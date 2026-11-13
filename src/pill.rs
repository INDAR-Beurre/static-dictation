//! pill: animated layer-shell pill (dark rounded bar + glowing teal waveform)
//! shown at the top-center of the screen while dictation is recording.
//!
//! Contract with main.rs / vecmentor / tsar:
//! - `PillHandle` is the public facade. Cloneable, Send, 'static — pass it anywhere.
//! - One instance per process. `show()` / `hide()` / `update(rms_level, bands, speaking)`
//!   are non-blocking, thread-safe, cheap: they only push into an internal channel; the
//!   render thread (this module owns it) does all Wayland + drawing work.
//! - When the pill is visible, `update()` MUST be called at ~30 Hz with the current
//!   RMS level (0.0..1.0, can exceed for loud speech — clamp internally) and the
//!   9 per-band spectral energies from `bands::energies` so the animation is lively.
//!   `speaking` = current VAD voice flag.
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

mod canvas;
mod tsar;
pub(crate) mod vecmentor;

#[cfg(test)]
mod preview;

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

/// Commands sent main → render thread. Internal plumbing: not part of the
/// facade API (main.rs drives the pill exclusively through `PillHandle`).
pub(crate) enum Ctl {
    Show,
    Hide,
    Update {
        level: f32,
        bands: [f32; 9],
        speaking: bool,
    },
}

impl PillHandle {
    /// Spawn the render thread. Call once at startup. Errors (returns None) if
    /// Wayland is unavailable — the tool then works fine without the pill.
    pub fn try_spawn() -> Option<Self> {
        let (tx, rx) = crossbeam_channel::unbounded::<Ctl>();
        let params = vecmentor::wave_params();
        let alive = Arc::new(AtomicBool::new(true));
        match tsar::spawn_render_thread(params, tx.clone(), rx, alive.clone()) {
            Ok(()) => Some(Self {
                inner: Arc::new(PillInner { tx, alive }),
            }),
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
    /// `level` is 0.0..2.0 RMS, over 1.0 = clipped/loud speech (clamped here to
    /// vecmentor's 0.0..2.0 input contract; vecmentor sanitizes NaN/inf too).
    /// `bands` is the 9 per-band spectral energies from `bands::energies`
    /// (array literal 9; keep in sync with bands.rs / vecmentor::NUM_BARS).
    /// Cheap no-op once the render thread is dead.
    pub fn update(&self, level: f32, bands: &[f32; 9], speaking: bool) {
        if !self.inner.alive() {
            return;
        }
        let _ = self.inner.tx.send(Ctl::Update {
            level: level.clamp(0.0, 2.0),
            bands: *bands,
            speaking,
        });
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
        let b = [0.1f32; 9];
        tx.send(Ctl::Show).unwrap();
        tx.send(Ctl::Update {
            level: 1.5,
            bands: b,
            speaking: true,
        })
        .unwrap();
        tx.send(Ctl::Hide).unwrap();
        assert!(matches!(rx.recv(), Ok(Ctl::Show)));
        assert!(matches!(
            rx.recv(),
            Ok(Ctl::Update {
                level: 1.5,
                ..
            })
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
            inner: Arc::new(PillInner {
                tx,
                alive: alive.clone(),
            }),
        };
        // Sanity: while alive, a send goes through.
        handle.show();
        assert!(matches!(rx.try_recv(), Ok(Ctl::Show)));
        // Simulate render-thread exit (what tsar's AliveGuard does).
        alive.store(false, Ordering::Release);
        handle.show();
        handle.update(0.5, &[0.0; 9], true);
        handle.hide();
        assert!(rx.try_recv().is_err(), "no Ctl may be sent after death");
    }

    /// A dead render thread also drops its `rx`, closing the channel. Show/hide
    /// sent in that window must stay cheap no-ops — not block or panic (send on
    /// a closed crossbeam channel returns Err, never blocks; this test pins
    /// that behavior for the facade's `let _ = send`).
    #[test]
    fn dead_channel_sends_are_noops() {
        let (tx, rx) = crossbeam_channel::unbounded::<Ctl>();
        let handle = PillHandle {
            inner: Arc::new(PillInner {
                tx,
                alive: Arc::new(AtomicBool::new(true)),
            }),
        };
        drop(rx); // channel closed, as when the render thread has exited
        handle.show();
        handle.update(0.5, &[0.0; 9], true);
        handle.hide();
        // Nothing to assert beyond "no panic/block" — sends failed silently,
        // which is exactly the facade contract once the thread is gone.
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
            // Level: sine sweep 0.4..1.2.
            let level = 0.8 + 0.4 * (i as f32 / 7.0).sin();
            // Bands: one-hot band rotating across the 9 indices, with the
            // neighbors trailing at half energy so the sweep looks continuous.
            let hot = ((i / 4) % 9) as usize;
            let mut bands = [0.0f32; 9];
            bands[hot] = 1.0;
            bands[(hot + 8) % 9] = 0.5;
            bands[(hot + 1) % 9] = 0.5;
            p.update(level, &bands, !i.is_multiple_of(4)); // speaking ~75% of frames
            i += 1;
            std::thread::sleep(Duration::from_millis(33));
        }
        p.hide();
        std::thread::sleep(Duration::from_millis(400));
        drop(p);
    }
}
