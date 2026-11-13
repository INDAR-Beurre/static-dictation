//! pill: animated layer-shell pill (dark rounded bar + glowing teal waveform)
//! shown at the top-center of the screen while dictation is recording.
//!
//! Contract with main.rs / vecmentor / tsar:
//! - `PillHandle` is the public facade. Cloneable, Send, 'static — pass it anywhere.
//! - One instance per process. `show()` / `hide()` / `update(rms_level, speaking)` are
//!   non-blocking, thread-safe, cheap: they only push into an internal channel; the
//!   render thread (this module owns it) does all Wayland + drawing work.
//! - When the pill is visible, `update()` MUST be called at ~30 Hz with the current
//!   RMS level (0.0..1.0, can exceed for loud speech — clamp internally) so the
//!   animation is lively. `speaking` = current VAD voice flag.
//! - `show()` should be called right when recording starts, `hide()` when it finalizes.
//! - The animation: symmetric equalizer-style bars (like the reference image),
//!   teal/cyan color, gentle glow, smooth level smoothing, peak-hold with decay.
//!   Pill is dark, rounded, ~44-64 px tall, ~240-320 px wide, top-center with margin.
//! - Dropping all clones shuts the render thread down gracefully. The surface is
//!   destroyed on thread exit, so an abrupt `main` exit is fine too.
//!
//! This file is the facade: it embeds `vecmentor` (wave physics) and `tsar` (Wayland
//! layer-shell rendering). Both are owned by different subagents; this module wires
//! them together and must compile when they do.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod vecmentor;
mod canvas;
mod tsar;

/// Public handle to the pill. Clone and send freely.
#[derive(Clone)]
pub struct PillHandle {
    inner: Arc<PillInner>,
}

struct PillInner {
    tx: crossbeam_channel::Sender<Ctl>,
    /// Set false by the render thread (via its exit guard) when it is gone for
    /// any reason. Dead-thread sends would otherwise pile up in the unbounded
    /// channel forever while main never learns the pill is dead.
    alive: Arc<AtomicBool>,
}

/// Commands sent main → render thread.
pub enum Ctl {
    Show,
    Hide,
    Update { level: f32, speaking: bool },
}

impl PillHandle {
    /// Spawn the render thread. Call once at startup. Errors (returns None) if
    /// Wayland is unavailable — the tool then works fine without the pill.
    pub fn try_spawn() -> Option<Self> {
        let (tx, rx) = crossbeam_channel::unbounded::<Ctl>();
        let params = vecmentor::wave_params();
        let alive = Arc::new(AtomicBool::new(true));
        match tsar::spawn_render_thread(params, tx.clone(), rx, alive.clone()) {
            Ok(()) => Some(Self { inner: Arc::new(PillInner { tx, alive }) }),
            Err(e) => {
                eprintln!("static-dictation: pill UI unavailable ({e}); running headless");
                None
            }
        }
    }

    /// Non-blocking show request. Cheap no-op once the render thread is dead.
    pub fn show(&self) {
        if !self.inner.alive() {
            return;
        }
        let _ = self.inner.tx.send(Ctl::Show);
    }

    /// Non-blocking hide request. Cheap no-op once the render thread is dead.
    pub fn hide(&self) {
        if !self.inner.alive() {
            return;
        }
        let _ = self.inner.tx.send(Ctl::Hide);
    }

    /// Non-blocking animation frame. Call at ~30 Hz while visible.
    /// `level` is 0.0..1.0 RMS, over 1.0 = clipped/loud speech (we clamp).
    /// Cheap no-op once the render thread is dead.
    pub fn update(&self, level: f32, speaking: bool) {
        if !self.inner.alive() {
            return;
        }
        let _ = self
            .inner
            .tx
            .send(Ctl::Update { level: level.clamp(0.0, 2.0), speaking });
    }
}

impl PillInner {
    fn alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctl_channel_roundtrip() {
        // The facade is thin; verify the channel command flow that
        // show/hide/update rely on, without needing Wayland at all.
        let (tx, rx) = crossbeam_channel::unbounded::<Ctl>();
        tx.send(Ctl::Show).unwrap();
        tx.send(Ctl::Update { level: 1.5, speaking: true }).unwrap();
        tx.send(Ctl::Hide).unwrap();
        assert!(matches!(rx.recv(), Ok(Ctl::Show)));
        assert!(matches!(
            rx.recv(),
            Ok(Ctl::Update { level: 1.5, speaking: true })
        ));
        assert!(matches!(rx.recv(), Ok(Ctl::Hide)));
    }

    /// After the render thread dies (alive flag false), show/update/hide must
    /// complete without sending — no unbounded queue growth on a dead thread.
    #[test]
    fn dead_thread_ctls_are_noops() {
        let (tx, rx) = crossbeam_channel::unbounded::<Ctl>();
        let alive = Arc::new(AtomicBool::new(true));
        let handle = PillHandle {
            inner: Arc::new(PillInner { tx, alive: alive.clone() }),
        };
        // Sanity: while alive, a send goes through.
        handle.show();
        assert!(matches!(rx.try_recv(), Ok(Ctl::Show)));
        // Simulate render-thread exit (what tsar's AliveGuard does).
        alive.store(false, Ordering::Release);
        handle.show();
        handle.update(0.5, true);
        handle.hide();
        assert!(rx.try_recv().is_err(), "no Ctl may be sent after death");
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Live visual check (run with --ignored, needs a real compositor + `grim`
    /// screenshotting mid-run): spawns the real render thread, sends Show plus
    /// animated Updates at ~30 Hz for 3 s, hides, and drops the handle.
    /// shortcut: dev harness for the pill-visibility hunt, delete once proven live.
    #[test]
    #[ignore]
    fn pill_show_visible_for_3s() {
        let p = PillHandle::try_spawn().expect("pill spawn");
        p.show();
        let t0 = Instant::now();
        let mut i = 0u32;
        while t0.elapsed() < Duration::from_secs(3) {
            let level = 0.5 + 0.5 * (i as f32 / 9.0).sin(); // sweep so bars animate
            p.update(level, i % 40 < 30);
            i += 1;
            std::thread::sleep(Duration::from_millis(33));
        }
        p.hide();
        std::thread::sleep(Duration::from_millis(400));
        drop(p);
    }
}
