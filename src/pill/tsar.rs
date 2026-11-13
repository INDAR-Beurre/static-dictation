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

use crate::pill::vecmentor::{self, WaveParams};

/// Ctl is defined in pill.rs; re-import for convenience.
use crate::pill::Ctl;

/// Spawn the render thread; see module doc for requirements.
pub fn spawn_render_thread(
    params: WaveParams,
    tx: crossbeam_channel::Sender<Ctl>,
    rx: crossbeam_channel::Receiver<Ctl>,
) -> anyhow::Result<()> {
    let _ = (params, tx, rx);
    unimplemented!("subagent T: spawn wayland render thread")
}
