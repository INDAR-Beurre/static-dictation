//! tsar: Wayland layer-shell renderer owning the pill window (overlay layer, top-center).
//!
//! Owned-by: subagent T. Do not edit the pub API; fill in the `unimplemented!()` bodies.
//! This is the only file that talks to Wayland, using smithay-client-toolkit 0.21.
//!
//! Contract (FROZEN):
//! - `spawn_render_thread(params, tx, rx) -> anyhow::Result<()>` — spawns the render
//!   thread, binds Wayland, creates an overlay layer surface (top-center, anchored
//!   TOP only, margin ~48 px, size 320x64, keyboard interactivity None), and drives the
//!   animation at compositor frame cadence. `rx` receives `Ctl` (the same enum defined
//!   in pill.rs — reference as `crate::pill::Ctl`). `tx` clone is handed in so the
//!   thread can detect client hangup (all senders dropped). Return Ok once the thread
//!   is started and Wayland bound OK; Err if Wayland is unavailable (pill.rs then
//!   makes the whole tool run headless gracefully).
//! - The thread: drains `rx` (non-blocking at frame cadence), updates vecmentor `Sim`
//!   via `Sim::push(level, speaking)` each frame + canvas::render_pill rasterizes.
//! - `Ctl::Show` → map the surface (draw frame loop until Hide); `Ctl::Hide` → unmap
//!   (attach transparent/zero buffer  then commit? NO — use `wl_surface.attach(None)`
//!   via `surface.attach(None::<&wl_buffer>)` then commit, or simply destroy + re-create
//!   on next Show; pick the simple/safe option). `Ctl::Update{level,speaking}` → advance sim.
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
//!     compositor::{CompositorState, FrameCallbackData},
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
//! layer.set_margin(48, 0, 0, 0);          // top-center TOM: anchor TOP only + WIDTH set
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

use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState, FrameCallbackData};
use smithay_client_toolkit::delegate_registry;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{delegate_dispatch2, registry_handlers};
use smithay_client_toolkit::reexports::client as wayland_client;
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{wl_buffer, wl_output, wl_shm, wl_surface};
use wayland_client::{Connection, EventQueue, QueueHandle};
use wayland_client::backend::WaylandError;

use crate::pill::vecmentor::{Sim, WaveParams};
use crate::pill::Ctl;

/// Fixed pill size. Layer-shell sizes are suggestions, but we own the surface and
/// always draw at this size; the first configure just confirms the mapped size.
const PILL_W: u32 = 320;
const PILL_H: u32 = 64;

/// Wait cap for each dispatch round. The channel cannot wake the Wayland socket,
/// so an unbounded dispatch would stall Hide/exit until the next frame event.
/// 50 ms keeps Hide latency invisible and idle CPU near zero (one wakeup / 50 ms).
const TICK: Duration = Duration::from_millis(50);

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
/// (2) draw/unmap if dirty, (3) bounded wait for Wayland events (prepare_read +
/// sleep up to TICK, then read + dispatch_pending). While visible the frame callback
/// arrives on the socket during the sleep and is dispatched after it, so cadence is
/// effectively frame-driven at ~20 fps (50 ms cap; compositors that don't send frame
/// callbacks behave identically). Never busy-spins: every round sleeps or dispatches.
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
    let output = OutputState::new(&globals, &qh)
        .outputs()
        .next();
    eprintln!(
        "static-dictation: pill using explicit output: {}",
        output.is_some()
    );
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface,
        Layer::Top,
        Some("static-dictation"),
        output.as_ref(),
    );
    layer.set_anchor(Anchor::TOP);
    layer.set_size(PILL_W, PILL_H);
    layer.set_margin(48, 0, 0, 0);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.commit();

    // Pool borrows shm only during construction; then shm moves into the app.
    let pool = match SlotPool::new((PILL_W * PILL_H * 4 * 4) as usize, &shm) {
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
        width: PILL_W,
        height: PILL_H,
        configured: false,
        exited: false,
        // True once any real buffer has been committed; unmap is only valid then
        // (Mango never maps a layer surface whose first commit is a null buffer).
        ever_drawn: false,
        scale: 1,
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

        // 3) Redraw when dirty. Hide when never-yet-drawn just skips drawing;
        // once mapped, hide = unmap (attach None + commit once). Clear the flag
        // BEFORE drawing: draw() may set it again (buffer-create failure) so the
        // loop retries on the next tick.
        if app.need_redraw {
            app.need_redraw = false;
            if app.visible {
                app.draw(&qh);
            } else if app.ever_drawn {
                app.unmap();
            }
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
    // timeout, then read + dispatch. Worst-case frame latency is TICK (50 ms) —
    // ~20 fps, fine for a pill — and idle CPU is one 50 ms sleep per round, ~0%.
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
    // True once any real buffer has been committed; unmap is only valid then
    // (Mango never maps a layer surface whose first commit is a null buffer).
    ever_drawn: bool,
    pool: SlotPool,
}

impl PillApp {
    fn handle_ctl(&mut self, ctl: Ctl) {
        match ctl {
            Ctl::Show => {
                self.visible = true;
                self.need_redraw = true;
            }
            Ctl::Hide => {
                self.visible = false;
                self.need_redraw = true;
            }
            Ctl::Update { level, speaking } => {
                self.sim.push(level, speaking);
                if self.visible {
                    self.need_redraw = true;
                }
            }
        }
    }

    /// Hide = unmap: attach no buffer and commit once. The surface object stays
    /// configured (cheap re-map on next Show: just draw again); the compositor
    /// removes it from the scene immediately.
    fn unmap(&mut self) {
        self.layer
            .wl_surface()
            .attach(None::<&wl_buffer::WlBuffer>, 0, 0);
        self.layer.commit();
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) {
        let width = self.width;
        let height = self.height;
        // Render at the compositor's preferred scale (Mango: 2): buffer pixels =
        // logical size × scale. The configure size stays logical; the canvas is
        // resolution-independent so it just gets more pixels.
        let scale = self.scale.max(1);
        let stride = width as i32 * scale * 4;
        self.ever_drawn = true;

        let (buffer, canvas) = match self.pool.create_buffer(
            width as i32 * scale,
            height as i32 * scale,
            stride,
            wl_shm::Format::Argb8888,
        ) {
            Ok(bc) => bc,
            Err(e) => {
                // Buffers not yet released by the compositor: set the retry flag
                // so the loop redraws next TICK (50 ms). No frame callback is
                // pending at this call site, so "the next frame retries" would
                // never fire and the pill would freeze until the next Ctl.
                eprintln!("static-dictation: pill create_buffer failed: {e}");
                self.need_redraw = true;
                return;
            }
        };

        crate::pill::canvas::render_pill(
            canvas,
            width as u32 * scale as u32,
            height as u32 * scale as u32,
            self.sim.bars(),
            self.started.elapsed().as_secs_f32(),
        );

        self.layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32 * scale, height as i32 * scale);
        self.layer
            .wl_surface()
            .frame(qh, FrameCallbackData(self.layer.wl_surface().clone()));
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
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        // Frame-callback cadence: redraw while visible. While hidden we never
        // request frame callbacks, so this only fires when mapped.
        if self.visible {
            self.draw(qh);
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
        self.width = NonZeroU32::new(configure.new_size.0).map_or(PILL_W, NonZeroU32::get);
        self.height = NonZeroU32::new(configure.new_size.1).map_or(PILL_H, NonZeroU32::get);
        self.configured = true;
        eprintln!(
            "static-dictation: pill configured {}x{} (visible={})",
            self.width, self.height, self.visible
        );
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
