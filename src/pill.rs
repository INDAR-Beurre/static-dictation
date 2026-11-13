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
        match tsar::spawn_render_thread(params, tx.clone(), rx) {
            Ok(()) => Some(Self { inner: Arc::new(PillInner { tx }) }),
            Err(e) => {
                eprintln!("static-dictation: pill UI unavailable ({e}); running headless");
                None
            }
        }
    }

    /// Non-blocking show request.
    pub fn show(&self) {
        let _ = self.inner.tx.send(Ctl::Show);
    }

    /// Non-blocking hide request.
    pub fn hide(&self) {
        let _ = self.inner.tx.send(Ctl::Hide);
    }

    /// Non-blocking animation frame. Call at ~30 Hz while visible.
    /// `level` is 0.0..1.0 RMS, over 1.0 = clipped/loud speech (we clamp).
    pub fn update(&self, level: f32, speaking: bool) {
        let _ = self
            .inner
            .tx
            .send(Ctl::Update { level: level.clamp(0.0, 2.0), speaking });
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
}
