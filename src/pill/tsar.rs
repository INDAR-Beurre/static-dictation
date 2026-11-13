//! tsar: Wayland layer-shell renderer owning the pill window (overlay layer, top-center).
//!
//! Owned-by: subagent T. Do not edit the pub API; fill in the `unimplemented!()` bodies.
//! This is the only file that talks to Wayland, using smithay-client-toolkit 0.21.
//!
//! Contract (FROZEN):
//! - `spawn_render_thread(params, tx, rx) -> anyhow::Result<()>` — spawns the render
//!   thread, binds Wayland, creates an overlay layer surface (top-center, anchored
//!   TOP only, margin ~48 px, size SURF_W x SURF_H from canvas.rs, keyboard
//!   interactivity None), and drives the
//!   animation at compositor frame cadence. `rx` receives `Ctl` (the same enum defined
//!   in pill.rs — reference as `crate::pill::Ctl`). `tx` clone is handed in so the
//!   thread can detect client hangup (all senders dropped). Return Ok once the thread
//!   is started and Wayland bound OK; Err if Wayland is unavailable (pill.rs then
//!   makes the whole tool run headless gracefully).
//! - The thread: drains `rx` (non-blocking at frame cadence), updates vecmentor `Sim`
//!   via `Sim::push(level, &bands, speaking)` each frame + canvas::render_pill rasterizes.
//! - `Ctl::Show` → map the surface (fade in, draw frame loop until Hide); `Ctl::Hide`
//!   → fade out to a fully transparent frame; the surface stays mapped forever
//!   (never unmap: a null-buffer commit resets wlroots' configured state and
//!   the next draw dies with protocol error 2 — see loop comment in step 3).
//!   `Ctl::Update{level,bands,speaking}` → advance sim.
//! - Use `SlotPool` (smithay_client_toolkit::shm::slot::SlotPool) for buffers; ARGB8888;
//!   damage full surface each frame; use frame callbacks for cadence (request next
//!   frame only while visible; use `std::thread::sleep` ~33ms fallback if no frame
//!   callback arrives, e.g. on compositors that don't send them).
//! - Graceful exit: when `rx` disconnects (all PillHandle clones dropped) or thread
//!   panics, the Wayland objects are dropped → surface destroyed. No leave-behind.
//!
//! Sketch of the working pieces (from local SCT 0.21.1 sources — trust these):
//! ```ignore
//! use smithay_client_toolkit::{
//!     compositor::{CompositorState},
//!     delegate_registry, registry::RegistryState, registry_handlers,
//!     shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler,
//!                        LayerSurface, LayerSurfaceConfigure},
//!     shm::{slot::SlotPool, Shm, ShmHandler},
//!     delegate_dispatch2,
//! };
//! use wayland_client::{globals::registry_queue_init, protocol::{wl_shm, wl_surface},
//!                     Connection, Dispatch, QueueHandle};
//!
//! // main thread body:
//! let conn = Connection::connect_to_env()?;                    // Err if no Wayland
//! let (globals, mut event_queue) = registry_queue_init(&conn)?;
//! let qh = event_queue.handle();
//! let compositor = CompositorState::bind(&globals, &qh)?;       // provides create_surface
//! let layer_shell = LayerShell::bind(&globals, &qh)?;           // zwlr_layer_shell_v1
//! let shm = Shm::bind(&globals, &qh)?;                          // wl_shm
//! let surface = compositor.create_surface(&qh);
//! let layer = layer_shell.create_layer_surface(&qh, surface, Layer::Top,
//!                                             Some("static-dictation"), None);
//! layer.set_anchor(Anchor::TOP);
//! layer.set_size(320, 64);
//! layer.set_margin(-14, 0, 0, 0);         // top-center: anchor TOP only + WIDTH set
//! layer.set_keyboard_interactivity(KeyboardInteractivity::None);
//! layer.commit();                        // then wait for first configure event
//! ```
//! Handler struct must impl: `RegistryState` via `delegate_registry!(State)` +
//! `registry_handlers![OutputState, SeatState]` NO — not needed for pill (no input) —
//! only `ProvidesRegistryState` if you use globals-bound helpers; minimal set:
//! `CompositorHandler` (frame callback), `LayerShellHandler` (configure/closed),
//! `ShmHandler` (shm_state), then `delegate_dispatch2!(State)`.
//! Draw each frame basically like the simple_layer example (SlotPool::create_buffer →
//! canvas → damage_buffer → frame(qh, FrameCallbackData(...)) → attach_to → commit).

use std::io::Write as _;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::delegate_registry;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::client as wayland_client;
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{delegate_dispatch2, registry_handlers};
use wayland_client::backend::WaylandError;
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{wl_output, wl_region, wl_shm, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};

use crate::pill::canvas::{GROW_MAX, SURF_H, SURF_W};
use crate::pill::vecmentor::{Sim, WaveParams};
use crate::pill::Ctl;

/// Wait cap for each dispatch round. The channel cannot wake the Wayland socket,
/// so an unbounded dispatch would stall Hide/exit until the next frame event.
/// 16 ms keeps Ctl latency invisible and idle CPU near zero (one wakeup / 16 ms);
/// it is also the fade-ramp tick (see `step_alpha`).
const TICK: Duration = Duration::from_millis(16);

/// Total fade duration: ~160 ms from alpha 0↔1 at the 16 ms TICK (10 ticks).
const FADE_SECS: f32 = 0.16;

/// One tick of the alpha ramp toward `target` (0.0 or 1.0). Pure math, no
/// Wayland. Fade-in is ease-out (fast start, gentle settle); fade-out is
/// ease-in (gentle start, quick finish); never crosses the target. A Show
/// arriving mid-fade-out just reverses the ramp from the current alpha.
fn step_alpha(cur: f32, target: f32, dt_secs: f32) -> f32 {
    if dt_secs <= 0.0 || cur == target {
        return cur;
    }
    let cur = cur.clamp(0.0, 1.0);
    // Both curves advance a linear progress p by dt/FADE_SECS per tick and are
    // inverted from `cur` so the ramp always finishes in exactly FADE_SECS.
    if target >= cur {
        // Fade-in, ease-out: cur = 1 - (1 - p)^2.
        let p = (1.0 - (1.0 - cur).sqrt() + dt_secs / FADE_SECS).min(1.0);
        1.0 - (1.0 - p) * (1.0 - p)
    } else {
        // Fade-out, ease-in: cur = 1 - p^2.
        let p = ((1.0 - cur).sqrt() + dt_secs / FADE_SECS).min(1.0);
        (1.0 - p * p).max(0.0)
    }
}

/// Spring constants for the entrance bounce, derived at compile time.
///
/// The grow spring is a damped 2nd-order oscillator driving scale factor
/// g(t): g" = -k*(g-target) - c*g', i.e. ω_n = √k (natural frequency),
/// ζ = c/(2√k) (damping ratio). We pick the FEEL, then derive k and c so the
/// two stay consistent:
///
/// - Entrance: ω_n = 22 rad/s (period ≈ 286 ms — one wiggle per overshoot
///   swing), ζ = 0.35 (underdamped → TWO clearly visible overshoot wiggles;
///   cold-start peak 1+e^(-ζπ/√(1-ζ²)) ≈ 1.31 rides the 1.25 canvas ceiling
///   for ~3 ticks — that flat-top IS the Dynamic-Island "stretch" pop),
///   settling inside 1% for good by ≈ 440 ms, well under the ~600 ms budget.
///   A small initial velocity kick (SPRING_KICK) launches it out of the bar.
/// - Hide contraction: ω_n = 24 rad/s, ζ = 0.80 — a faster, firmer,
///   near-critically-damped shrink (overshoot factor e^(-0.8π/0.6) ≈ ×0.015,
///   invisible); reaches grow < 0.02 (snapped to 0) in 12 ticks ≈ 192 ms,
///   matching the required 180-220 ms. The alpha fade runs in parallel.
///
/// Semi-implicit Euler at dt = 16 ms is unconditionally stable here
/// (dt·ω_n ≪ 2), so the integrator never adds oscillation of its own.
const SPRING_OMEGA_SHOW: f32 = 22.0;
const SPRING_ZETA_SHOW: f32 = 0.35;
const SPRING_OMEGA_HIDE: f32 = 24.0;
const SPRING_ZETA_HIDE: f32 = 0.80;
const SPRING_KICK: f32 = 2.0;

/// One semi-implicit Euler step of the grow spring toward `target`.
/// Semi-implicit (a.k.a. symplectic) Euler — update velocity first with the new
/// position's force, then position with the NEW velocity — is stable for this
/// dt and gives a livelier, more natural bounce than explicit Euler.
/// Clamps grow to [0, 1.25] (the same ceiling canvas.rs clamps at) so a big
/// overshoot can never ask canvas for an out-of-range scale.
fn step_grow(cur: f32, vel: f32, target: f32, dt_secs: f32, omega: f32, zeta: f32) -> (f32, f32) {
    if dt_secs <= 0.0 {
        return (cur, vel);
    }
    let k = omega * omega; // ω_n²
    let c = 2.0 * zeta * omega; // 2·ζ·ω_n
    let vel = vel + (-k * (cur - target) - c * vel) * dt_secs;
    // Clamp both ends: [0, 1.25] matches the canvas's internal clamp, so an
    // overshoot can never ask canvas for an out-of-range scale, and a Hide
    // retarget from a mid-flight bounce can never dip below collapsed.
    let cur = (cur + vel * dt_secs).clamp(0.0, GROW_MAX);
    (cur, vel)
}

/// One full spring tick on the app state: re-aims at the current target
/// (1.0 visible / 0.0 hidden) with the phase's ω_n/ζ, and snaps fully-hidden
/// grow to exactly 0 once it is visually gone (< 0.02) — so a later Show
/// restarts the bounce from a clean collapsed state, and the alpha fade plus
/// hide-contraction can finish in parallel without blocking each other.
fn step_grow_tick(grow: &mut f32, vel: &mut f32, visible: bool, dt_secs: f32) {
    let target = if visible { 1.0 } else { 0.0 };
    if (*grow - target).abs() > 0.002 {
        let (omega, zeta) = if visible {
            (SPRING_OMEGA_SHOW, SPRING_ZETA_SHOW)
        } else {
            (SPRING_OMEGA_HIDE, SPRING_ZETA_HIDE)
        };
        let (g, v) = step_grow(*grow, *vel, target, dt_secs, omega, zeta);
        *grow = g;
        *vel = v;
        // Visually gone: clamp to exact 0 (grow < 0.02 rule) so the surface's
        // last frame is the fully-collapsed one while alpha fades in parallel.
        if !visible && *grow < 0.02 {
            *grow = 0.0;
            *vel = 0.0;
        }
    } else if *grow != target {
        // Spring has settled inside tolerance: land exactly on target so the
        // loop's idle condition (grow == target) holds without float dust.
        *grow = target;
        *vel = 0.0;
    }
}

/// Spawn the render thread; see module doc for requirements.
pub fn spawn_render_thread(
    params: WaveParams,
    tx: crossbeam_channel::Sender<Ctl>,
    rx: crossbeam_channel::Receiver<Ctl>,
    alive: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    // Requirement: flush before binding so main's banner is ordered before any
    // error output this thread might print on early return.
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();

    std::thread::Builder::new()
        .name("pill-render".into())
        .spawn(move || {
            // Cleared on EVERY exit path — error returns, Disconnected, panic —
            // by the guard's Drop (panics unwind through the closure, dropping it).
            let _guard = AliveGuard { flag: alive };
            if let Err(e) = render_thread(params, tx, rx) {
                eprintln!("static-dictation: pill render thread died: {e:#}");
            }
        })?;
    Ok(())
}

/// Sets the shared alive flag false when dropped; the facade (pill.rs) uses the
/// flag to stop sending Ctls into a channel whose reader is gone.
struct AliveGuard {
    flag: Arc<AtomicBool>,
}

impl Drop for AliveGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

/// Loop architecture (chosen): each round is (1) drain `rx` non-blocking,
/// (2) draw if dirty, (3) bounded wait for Wayland events (prepare_read +
/// sleep up to TICK, then read + dispatch_pending). Cadence is tick-driven
/// (16 ms); while visible the Ctl stream (audio sim, 16 ms) drives redraws.
/// Never busy-spins: every round sleeps or dispatches.
fn render_thread(
    params: WaveParams,
    tx: crossbeam_channel::Sender<Ctl>,
    rx: crossbeam_channel::Receiver<Ctl>,
) -> anyhow::Result<()> {
    // The handed-in `tx` is deliberately dropped here: holding it would keep the
    // channel connected forever and rx would never observe Disconnected. Exit is
    // driven by rx.try_recv() == Disconnected, i.e. all PillHandle clones dropped.
    drop(tx);

    // shortcut: EAGAIN on connect happens when the compositor's accept queue briefly
    // stalls; a single attempt would permanently lose the pill. Retry ~5×/500ms.
    let mut last_err = None;
    let conn = 'c: {
        for _ in 0..5 {
            match Connection::connect_to_env() {
                Ok(c) => break 'c c,
                Err(e) => {
                    eprintln!("static-dictation: pill wayland connect failed ({e}); retrying");
                    last_err = Some(e);
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        }
        return Err(last_err.expect("at least one connect attempt").into());
    };
    let (globals, mut event_queue) = match registry_queue_init(&conn) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("static-dictation: pill step registry_init failed: {e}");
            return Err(e.into());
        }
    };
    let qh = event_queue.handle();

    // Each bind can fail (protocol missing) — pill.rs falls back headless on Err.
    let compositor = match CompositorState::bind(&globals, &qh) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("static-dictation: pill step bind_compositor failed: {e}");
            return Err(e.into());
        }
    };
    let layer_shell = match LayerShell::bind(&globals, &qh) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("static-dictation: pill step bind_layer_shell failed: {e}");
            return Err(e.into());
        }
    };
    let shm = match Shm::bind(&globals, &qh) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("static-dictation: pill step bind_shm failed: {e}");
            return Err(e.into());
        }
    };

    let surface = compositor.create_surface(&qh);
    // shortcut: output=None is ambiguous on Mango (dead space above eDP-1) — bind
    // the first real output so the pill anchors to the panel, not nowhere.
    let output = OutputState::new(&globals, &qh).outputs().next();
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface,
        Layer::Top,
        Some("static-dictation"),
        output.as_ref(),
    );
    layer.set_anchor(Anchor::TOP);
    layer.set_size(SURF_W, SURF_H);
    // Negative top margin: the 104-logical-px surface starts 14 px above the
    // screen (world y −14..90) so the canvas can slide its capsule between the
    // collapsed clock-pill position (core at world y 15) and the full island
    // hugging the top edge (top at world y 6) purely via `grow` — the
    // compositor geometry stays static (Mango layer animations would fight a
    // per-tick margin). A static negative margin is proven fine on Mango.
    layer.set_margin(-14, 0, 0, 0);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);

    // WHY regions: the pill is a floating overlay — it must never eat clicks and
    // must never claim opaque pixels it doesn't have.
    // - Input: the default input region is the whole surface ("infinite"); a
    //   top-center pill would otherwise steal pointer events from whatever is
    //   below it. set_input_region with a real, EMPTY region (not the None form
    //   — None means "infinite"!) makes the overlay click-through. Both region
    //   requests are double-buffered, so they must be issued BEFORE the first
    //   commit to cover the surface's whole life.
    let region = compositor.wl_compositor().create_region(&qh, ());
    layer.wl_surface().set_input_region(Some(&region));
    region.destroy();
    // - Opaque: the protocol DEFAULT is already the empty region, which is what
    //   a pill with transparent corners + halo needs (alpha blit, not opaque
    //   copy). Make it explicit anyway so a later edit can't silently regress it
    //   into Mango's opaque-copy fast path with garbage corner pixels.
    layer.wl_surface().set_opaque_region(None);

    layer.commit();

    // Pool borrows shm only during construction; then shm moves into the app.
    // Byte headroom = logical w*h*4 bytes × 4: covers a scale-2 buffer (×4 area)
    // for ARGB8888 in one allocation; draw() re-creates per-frame buffers from
    // the same pool.
    let pool = match SlotPool::new((SURF_W * SURF_H * 4 * 4) as usize, &shm) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("static-dictation: pill step slot_pool failed: {e}");
            return Err(e.into());
        }
    };

    let mut app = PillApp {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        layer,
        sim: Sim::new(params),
        started: Instant::now(),
        visible: false,
        need_redraw: true, // first Hide commit once configured
        width: SURF_W,
        height: SURF_H,
        configured: false,
        exited: false,
        scale: 1,
        // Fade state: start fully transparent; Show ramps toward 1.0.
        alpha: 0.0,
        // Entrance bounce: grow starts collapsed; the first Show kicks it.
        grow: 0.0,
        grow_vel: 0.0,
        pool,
    };

    // Wait for the first configure (it tells us the mapped size) before drawing.
    // The queue is already live; app.configured is set by the LayerShellHandler.
    while !app.configured {
        event_queue.blocking_dispatch(&mut app)?;
    }

    while !app.exited {
        // 1) Drain all pending Ctl messages (never blocks).
        loop {
            match rx.try_recv() {
                Ok(ctl) => app.handle_ctl(ctl),
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    app.exited = true;
                    break;
                }
            }
        }
        if app.exited {
            break;
        }

        // 2) Dispatch pending Wayland events BEFORE any commit: a commit racing
        // an un-acked configure is a protocol error ("wrong configure serial" on
        // Mango; the surface gets killed). SCT acks configure during dispatch, so
        // dispatch-first guarantees acks always precede our commits.
        wait_and_dispatch(&conn, &mut event_queue, &mut app, TICK)?;
        if app.exited {
            break;
        }

        // 2b) Advance the fade ramp one tick; sets need_redraw while alpha is
        // still moving toward its target (so fading-out keeps drawing until
        // alpha bottoms at 0 — transparent frame, surface still mapped).
        app.tick_fade();

        // 3) Redraw when dirty or mid-fade. Fade-out keeps drawing each tick
        // until alpha bottoms out at 0 — a fully transparent frame (master_alpha
        // 0 zeroes every pixel's alpha) — and the surface STAYS MAPPED.
        // shortcut: never unmaps — a null-buffer commit resets wlroots' configured
        // state, and the next draw commits pre-configure → protocol error 2 kills
        // the thread; if the idle pill ever needs to free memory, revisit
        if app.need_redraw || app.alpha != app.fade_target() {
            app.need_redraw = false;
            app.draw();
        }

        // 3b) Any commit above (draw/unmap) must reach the socket NOW: the
        // wayland client buffers writes and only flushes during dispatch. Without
        // this the compositor sees nothing until the next dispatch round.
        if conn.flush().is_err() {
            // EPIPE later surfaces as a read error in wait_and_dispatch; exit soon.
        }
    }

    // Dropping `app` (and `conn`) destroys the layer surface: the pill disappears.
    Ok(())
}

/// One bounded wait round. std has no non-blocking poll, so: reserve a read guard
/// (never losing socket events), sleep up to TICK, then read + dispatch.
fn wait_and_dispatch(
    _conn: &Connection,
    event_queue: &mut EventQueue<PillApp>,
    app: &mut PillApp,
    timeout: Duration,
) -> anyhow::Result<()> {
    // prepare_read reserves the socket read; if None, events are already queued.
    let Some(guard) = event_queue.prepare_read() else {
        event_queue.dispatch_pending(app)?;
        return Ok(());
    };
    // No safe non-blocking poll of the fd exists in std alone, so: sleep the full
    // timeout, then read + dispatch. Worst-case frame latency is TICK (16 ms) —
    // effectively frame-driven while mapped — and idle CPU is one 16 ms sleep per
    // round, near 0%. TICK is also the fade-ramp step, so the tick keeps the
    // alpha state machine moving even without frame callbacks.
    std::thread::sleep(timeout);
    match guard.read() {
        Ok(_) => {}
        // WouldBlock = no Wayland events during the sleep (normal while unmapped —
        // compositors send no frame callbacks to hidden surfaces). Not an error.
        Err(WaylandError::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {}
        Err(e) => return Err(e.into()),
    }
    event_queue.dispatch_pending(app)?;
    Ok(())
}

struct PillApp {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    layer: LayerSurface,
    sim: Sim,
    started: Instant,
    visible: bool,
    need_redraw: bool,
    width: u32,
    height: u32,
    configured: bool,
    exited: bool,
    // Compositor-preferred buffer scale (updated by scale_factor_changed).
    scale: i32,
    // Current master alpha (0.0..=1.0), eased toward fade_target each tick.
    alpha: f32,
    // Entrance/hide bounce: scale factor and its velocity, spring-driven
    // toward `grow_target` each tick (0.0 collapsed .. 1.0 full; may overshoot
    // to at most 1.25 — canvas clamps there too). Hide uses a faster,
    // near-critically-damped spring than Show.
    grow: f32,
    grow_vel: f32,
    pool: SlotPool,
}

impl PillApp {
    /// Where the alpha ramp is heading: 1.0 while visible, else 0.0.
    fn fade_target(&self) -> f32 {
        if self.visible {
            1.0
        } else {
            0.0
        }
    }

    /// Advance the fade ramp one TICK; called every loop round before drawing.
    fn tick_fade(&mut self) {
        let target = self.fade_target();
        if self.alpha != target {
            self.alpha = step_alpha(self.alpha, target, TICK.as_secs_f32());
            self.need_redraw = true;
        }
        // Bounce spring: one semi-implicit Euler step per tick. Show and Hide
        // pick their own ω_n/ζ; a mid-flight retarget just re-aims the same
        // spring from its current state (no jump). While |grow - target| is
        // still visible we keep redrawing; once it has settled (and the alpha
        // ramp is done) the loop idles as before.
        let target = if self.visible { 1.0 } else { 0.0 };
        if (self.grow - target).abs() > 0.002 {
            step_grow_tick(
                &mut self.grow,
                &mut self.grow_vel,
                self.visible,
                TICK.as_secs_f32(),
            );
            self.need_redraw = true;
        }
    }

    fn handle_ctl(&mut self, ctl: Ctl) {
        match ctl {
            Ctl::Show => {
                self.visible = true;
                self.need_redraw = true;
                // Bounce entrance: start collapsed with a small outward velocity
                // kick so the spring leaps out of the top bar. If a Show lands
                // mid-shrink, don't restart from 0 — re-aim the running spring
                // from wherever it is (never jump, no state reset). grow < 0.02
                // means already snapped to 0 (see step_grow_tick).
                if self.grow < 0.02 {
                    self.grow = 0.0;
                    self.grow_vel = SPRING_KICK;
                }
            }
            Ctl::Hide => {
                self.visible = false;
                self.need_redraw = true;
                // Shrink immediately with the near-critically-damped hide
                // spring (chosen in tick_fade); alpha fades in parallel. The
                // clamp in step_grow means grow < 0.02 reads as "gone".
            }
            Ctl::Update {
                level,
                bands,
                speaking,
            } => {
                self.sim.push(level, &bands, speaking);
                if self.visible || self.alpha > 0.0 {
                    self.need_redraw = true;
                }
            }
        }
    }

    fn draw(&mut self) {
        let width = self.width;
        let height = self.height;
        // Render at the compositor's preferred scale (Mango: 2): buffer pixels =
        // logical size × scale. The configure size stays logical; the canvas is
        // resolution-independent so it just gets more pixels.
        let scale = self.scale.max(1);
        let stride = width as i32 * scale * 4;

        let (buffer, canvas) = match self.pool.create_buffer(
            width as i32 * scale,
            height as i32 * scale,
            stride,
            wl_shm::Format::Argb8888,
        ) {
            Ok(bc) => bc,
            Err(e) => {
                // Buffers not yet released by the compositor: set the retry flag
                // so the loop redraws next TICK (16 ms). No frame callback is
                // pending at this call site, so "the next frame retries" would
                // never fire and the pill would freeze until the next Ctl.
                eprintln!("static-dictation: pill create_buffer failed: {e}");
                self.need_redraw = true;
                return;
            }
        };

        crate::pill::canvas::render_pill(
            canvas,
            width * scale as u32,
            height * scale as u32,
            self.sim.bars(),
            self.started.elapsed().as_secs_f32(),
            self.alpha,
            self.grow,
        );

        self.layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32 * scale, height as i32 * scale);
        // No frame callback here — see frame() handler: callbacks self-sustain a
        // dispatch flood on Mango (~190 ms/round) and starve the fade tick.
        buffer.attach_to(self.layer.wl_surface()).ok();
        self.layer.commit();
    }
}

impl CompositorHandler for PillApp {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        // Mango advertises scale 2. Render at that scale (canvas multiplies),
        // and record it so draw() sizes buffers correctly; otherwise the
        // compositor may refuse to map a scale-1 buffer on a scale-2 output.
        if new_factor > 0 {
            self.scale = new_factor;
            surface.set_buffer_scale(new_factor);
            self.need_redraw = true;
        }
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        // The render loop is TICK-driven (16 ms) — every redraw there covers the
        // fade cadence, and Update Ctls arrive at 16 ms granularity anyway.
        // Requesting a callback inside draw() would self-sustain a frame flood:
        // callback -> frame() -> draw() -> callback, which starves the loop's
        // other duties (callbacks arrive ~5-6 ms after commit, stretching a
        // ~320 ms fade into ~2 s). So: never request callbacks; draw() is only
        // called by the loop.
        //
        // CONTRACT (enforced by the `frame_callback_*` tests below): the draw
        // path must NEVER issue a frame request (`.frame(..)` /
        // `FrameCallbackData` / `wl_callback`). Under slow compositors the
        // callback arrives ~5-6 ms after commit, so a
        // callback→request-frame→callback loop ran at 180+ Hz and starved the
        // 16 ms tick_fade, stretching a ~320 ms fade into ~2 s (~6×). A frame()
        // callback arrival may ONLY set need_redraw (as here); the 16 ms TICK
        // re-arms every redraw.
        if self.visible || self.alpha > 0.0 {
            self.need_redraw = true;
        }
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        let _ = output; // anchor pill to first output; no debug info needed
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for PillApp {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exited = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        self.width = NonZeroU32::new(configure.new_size.0).map_or(SURF_W, NonZeroU32::get);
        self.height = NonZeroU32::new(configure.new_size.1).map_or(SURF_H, NonZeroU32::get);
        self.configured = true;
        // NEVER commit from inside the configure handler: Mango may send several
        // configures back-to-back (bunched), and SCT acks each one as it is
        // dispatched; a commit interleaved between acks desyncs the serial
        // ("wrong configure serial" → surface killed). Instead mark dirty and
        // let the render loop draw+commit AFTER the whole batch is dispatched.
        // The first commit is still a real buffer: need_redraw was initialized
        // true — the loop draws before Mango ever sees any null-buffer commit,
        // so the "Mango never maps a first-null-commit surface" trap is dodged
        // and the surface still gets its initial buffer on the first tick.
        self.need_redraw = true;
    }
}

impl ShmHandler for PillApp {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl OutputHandler for PillApp {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

delegate_registry!(PillApp);

impl ProvidesRegistryState for PillApp {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

delegate_dispatch2!(PillApp);

// wl_region has no events; wayland_client still requires a Dispatch impl so the
// queue can own it. Without this the empty input-region object fails to
// dispatch and the requests never take effect (SCT examples do the same).
impl Dispatch<wl_region::WlRegion, ()> for PillApp {
    fn event(
        _state: &mut Self,
        _region: &wl_region::WlRegion,
        _event: wl_region::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICK_SECS: f32 = TICK.as_secs_f32();

    /// Sum 16 ms steps until the ramp settles; return (steps_taken, final_alpha).
    fn run_ramp(from: f32, target: f32, max_steps: usize) -> (usize, f32) {
        let mut cur = from;
        let mut steps = 0;
        while cur != target && steps < max_steps {
            cur = step_alpha(cur, target, TICK_SECS);
            steps += 1;
        }
        (steps, cur)
    }

    #[test]
    fn fade_in_reaches_one_in_about_160ms() {
        let (steps, end) = run_ramp(0.0, 1.0, 100);
        assert_eq!(end, 1.0, "must land exactly on target");
        // 160 ms / 16 ms = 10 ticks; allow one tick of slack for float rounding.
        assert!((10..=11).contains(&steps), "took {steps} ticks");
    }

    #[test]
    fn fade_out_reaches_zero_in_about_160ms() {
        let (steps, end) = run_ramp(1.0, 0.0, 100);
        assert_eq!(end, 0.0, "must land exactly on target");
        assert!((10..=11).contains(&steps), "took {steps} ticks");
    }

    #[test]
    fn never_overshoots_or_undershoots() {
        // Fade-in: monotonic non-decreasing, never above 1.0.
        let mut cur = 0.0f32;
        for _ in 0..50 {
            let next = step_alpha(cur, 1.0, TICK_SECS);
            assert!(next >= cur && next <= 1.0, "in {cur} out {next}");
            cur = next;
        }
        // Fade-out: monotonic non-increasing, never below 0.0.
        let mut cur = 1.0f32;
        for _ in 0..50 {
            let next = step_alpha(cur, 0.0, TICK_SECS);
            assert!(next <= cur && next >= 0.0, "in {cur} out {next}");
            cur = next;
        }
    }

    #[test]
    fn reverses_direction_mid_ramp() {
        // Half-way into fade-in, flip to fade-out: must glide down from the
        // current value (not jump) and still reach 0.0 in ~the remaining ramp.
        let mut cur = 0.0f32;
        for _ in 0..5 {
            cur = step_alpha(cur, 1.0, TICK_SECS);
        }
        assert!(cur > 0.0 && cur < 1.0);
        let top = cur;
        let (steps, end) = run_ramp(top, 0.0, 100);
        assert_eq!(end, 0.0);
        assert!(steps <= 11, "reverse ramp took {steps} ticks");
        // And back mid-fade-out to fade-in again.
        cur = end;
        for _ in 0..3 {
            cur = step_alpha(cur, 0.0, TICK_SECS);
        }
        let (steps, end) = run_ramp(cur, 1.0, 100);
        assert_eq!(end, 1.0);
        assert!(steps <= 11, "second reverse took {steps} ticks");
    }

    #[test]
    fn degenerate_inputs_are_stable() {
        assert_eq!(step_alpha(0.5, 0.5, TICK_SECS), 0.5);
        assert_eq!(step_alpha(0.4, 1.0, 0.0), 0.4); // dt=0: no movement
        assert_eq!(step_alpha(0.4, 1.0, -0.016), 0.4); // negative dt: no movement
        assert!(step_alpha(2.0, 0.0, TICK_SECS) <= 1.0);
        assert!(step_alpha(-0.5, 1.0, TICK_SECS) >= 0.0);
    }

    // ---- grow (bounce) spring tests ----

    const EPS: f32 = 1e-4;

    #[test]
    fn entrance_bounces_with_two_overshoots_and_settles() {
        let mut grow = 0.0f32;
        let mut vel = SPRING_KICK;
        let mut excursions = 0u32;
        let mut above = false;
        let mut peaked_above_1_05 = false;
        let mut second_peak = 0.0f32;
        let mut last_outside_tick = 0usize;
        for i in 0..50 {
            step_grow_tick(&mut grow, &mut vel, true, TICK_SECS);
            assert!(grow.is_finite(), "tick {i}: grow not finite");
            assert!(grow >= 0.0, "tick {i}: grow dipped below 0");
            if grow > 1.0 && !above {
                excursions += 1;
            }
            if excursions == 2 && grow > 1.0 {
                second_peak = second_peak.max(grow);
            }
            above = grow > 1.0;
            peaked_above_1_05 |= grow > 1.05;
            if (grow - 1.0).abs() >= 0.01 {
                last_outside_tick = i;
            }
        }
        // TWO visible overshoot wiggles: two separate excursions above 1.0,
        // the first one clearly visible (past 1.05).
        assert_eq!(excursions, 2, "want exactly two overshoot wiggles");
        assert!(peaked_above_1_05, "first overshoot must clear 1.05");
        // The bouncy character the user asked for: the second excursion is
        // still clearly visible (measured peak 1.0201) — never detune flatter.
        assert!(second_peak > 1.01, "second overshoot too flat: {second_peak}");
        // Settle budget: the last excursion outside 1% must end by tick 36
        // ((36+1)·16 ms = 592 ms ≤ the 600 ms budget); measured: tick 28 = 464 ms.
        assert!(
            last_outside_tick <= 36,
            "still wobbling at tick {last_outside_tick}"
        );
        assert!((grow - 1.0).abs() < 0.01, "end value {grow} drifted");
    }

    #[test]
    fn hide_contraction_snaps_within_180_220ms() {
        let mut grow = 1.0f32;
        let mut vel = 0.0f32;
        let mut ticks = 0;
        while grow > 0.0 && ticks < 25 {
            step_grow_tick(&mut grow, &mut vel, false, TICK_SECS);
            ticks += 1;
            assert!(
                grow.is_finite() && grow >= 0.0,
                "tick {ticks}: grow={grow}"
            );
        }
        assert!(ticks < 25, "grow never snapped (stuck at {grow})");
        assert_eq!(grow, 0.0, "must snap to exactly 0 (grow<0.02 rule)");
        // Timing budget: 180-220 ms (measured: 12 ticks = 192 ms).
        let ms = (ticks as u128) * TICK.as_millis();
        assert!(
            (180..=220).contains(&ms),
            "hide contraction took {ms} ms ({ticks} ticks)"
        );
    }

    #[test]
    fn show_mid_hide_retargets_without_nans_or_range_violations() {
        for hide_ticks in [0usize, 1, 4, 8, 12, 16, 20] {
            let mut grow = 1.0f32;
            let mut vel = 0.0f32;
            for _ in 0..hide_ticks {
                step_grow_tick(&mut grow, &mut vel, false, TICK_SECS);
            }
            // Mid-flight Show: spring re-targets from current state (no jump,
            // no reset). Also cover Hide mid-Show after 10 entrance ticks.
            for round in 0..60 {
                step_grow_tick(&mut grow, &mut vel, true, TICK_SECS);
                assert!(
                    grow.is_finite() && (0.0..=GROW_MAX).contains(&grow),
                    "hide={hide_ticks} tick {round}: grow={grow} vel={vel}"
                );
            }
            assert!((grow - 1.0).abs() < 0.01, "hide={hide_ticks}: end {grow}");
            // And the mirror case: Hide arriving mid-Show, sweeping over the
            // launch, the 1.25 overshoot plateau, and the way down.
            for show_ticks in [0usize, 2, 6, 10, 14] {
                let mut grow = 1.0f32;
                let mut vel = SPRING_KICK;
                for _ in 0..show_ticks {
                    step_grow_tick(&mut grow, &mut vel, true, TICK_SECS);
                }
                for round in 0..30 {
                    step_grow_tick(&mut grow, &mut vel, false, TICK_SECS);
                    assert!(
                        grow.is_finite() && (0.0..=GROW_MAX).contains(&grow),
                        "hide@show{show_ticks} tick {round}: grow={grow} vel={vel}"
                    );
                }
                assert_eq!(grow, 0.0);
            }
        }
    }

    #[test]
    fn draw_passes_grow_to_render_pill_by_construction() {
        // The draw() call site must thread the spring state through to
        // render_pill (compile-level assertion, by construction):
        // `render_pill(.., self.alpha, self.grow)` — see `PillApp::draw`.
        let (g, _v) = step_grow(
            0.5,
            0.0,
            1.0,
            TICK_SECS,
            SPRING_OMEGA_SHOW,
            SPRING_ZETA_SHOW,
        );
        assert!((g - 0.5).abs() < 0.5 + EPS);
    }

    // ---- regression guard: the draw path must never request frame callbacks ----
    //
    // CONTRACT (see the frame() handler prose above): draw() and the loop must
    // never issue a frame request. History: a past regression called
    // wl_surface.frame() from the draw path; under slow compositors (Mango on
    // this machine) the callback arrived ~5-6 ms after commit, so a
    // callback→request-frame→callback loop ran at 180+ Hz and starved the
    // 16 ms tick_fade — a ~320 ms fade stretched to ~2 s (~6×). Fixed in
    // d07b8c0: frame()/callback handling only sets need_redraw; the 16 ms
    // TICK re-arms every redraw.
    //
    // A real seam test would need a live PillApp, which owns SlotPool and
    // LayerSurface (unconstructible off-Wayland) — driving it would require a
    // refactor, which is out of scope. So these are deliberately crude
    // grep-style source-contract tests: they catch exactly the regression that
    // shipped and need no refactor.

    /// Source of tsar.rs, for the contract tests below.
    const TSAR_SRC: &str = include_str!("tsar.rs");

    /// Strip `//` line comments so the tests' own literals (this prose, the
    /// token lists) cannot self-match the contract assertions.
    fn strip_line_comments(src: &str) -> String {
        src.lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<Vec<&str>>()
            .join("\n")
    }

    /// Slice from `start_marker` up to the next function or impl boundary, so
    /// the scan covers the whole body. Panics if a marker is missing: the
    /// contract must fail loudly if the code it guards is renamed away.
    fn body_until_next_fn<'a>(src: &'a str, start_marker: &str) -> &'a str {
        let start = src
            .find(start_marker)
            .unwrap_or_else(|| panic!("contract anchor {start_marker:?} missing"));
        let rest = &src[start + start_marker.len()..];
        let end = [rest.find("\nfn "), rest.find("\n    fn ")]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(rest.len());
        &rest[..end]
    }

    #[test]
    fn draw_path_never_requests_frame_callbacks() {
        let code = strip_line_comments(TSAR_SRC);
        // Whole live (pre-test-module) code: none of the regression tokens may
        // exist anywhere outside the test module.
        let live = &code[..code.find("#[cfg(test)]").expect("test module anchor")];
        for token in [".frame(", "FrameCallbackData", "wl_callback"] {
            assert!(
                !live.contains(token),
                "frame-callback regression: token {token:?} reappeared in live code"
            );
        }
        // And the draw() body specifically — the site where it shipped.
        let draw = body_until_next_fn(&code, "fn draw(&mut self)");
        for token in [".frame(", "FrameCallbackData", "wl_callback"] {
            assert!(!draw.contains(token), "draw() must never contain {token:?}");
        }
    }

    #[test]
    fn frame_callback_sets_only_need_redraw() {
        let code = strip_line_comments(TSAR_SRC);
        let handler = body_until_next_fn(&code, "fn frame(");
        // The callback's only side effect is arming the tick-driven redraw.
        assert!(
            handler.contains("need_redraw = true"),
            "frame() must set need_redraw (the 16 ms tick re-arms rendering)"
        );
        for token in ["self.draw(", "commit(", ".frame("] {
            assert!(
                !handler.contains(token),
                "frame() must not {token:?} — draw/commit belongs to the tick loop"
            );
        }
    }

    #[test]
    fn frame_callback_starve_contract_is_documented() {
        // The explanatory prose must survive refactors: without it the next
        // "why not frame callbacks for cadence?" question gets re-answered
        // wrong. Both phrasings carry the rule and the WHY.
        assert!(
            TSAR_SRC.contains("starves") && TSAR_SRC.contains("never request callbacks"),
            "the frame-callback starvation explanation in the frame() handler was removed"
        );
        assert!(
            TSAR_SRC.contains("stretching a ~320 ms fade into ~2 s"),
            "the measured regression numbers (~320 ms → ~2 s) were removed"
        );
    }
}
