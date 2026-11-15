# One dedicated key: AltGr+L, nothing else

User asked (m05613, answer "3." in m05646): "make it only the dictation key possible
and that key is only for dictation no other aczions".

## What changed

- `HotkeyEvent` now has exactly one variant, `Toggle`.
- `src/hk.rs` binds only `KEY_RIGHTALT`(100) + `KEY_L`(38). `KEY_RIGHTCTRL`(97),
  `KEY_KP0`(82), `KEY_INSERT`(110) were removed entirely — no PTT, no NumPad0.
  `src/hk.rs::only_altgr_l_is_bound` asserts they produce `None` on press and release.
- The chord is fully consumed, so it cannot cause any other app action:
  - AltGr press is re-emitted; a synthetic `AltGr release` cancels it the moment L
    fires, so no modifier stays held in the focused window.
  - Every L press / autorepeat / release while AltGr is down is swallowed
    (`src/hk.rs::l_event_hidden`), so no "l" is typed even if L is tapped twice
    during one hold.
  - The real AltGr keyup is swallowed; the stale-hold reset still clears the state.
  - Exactly one `Toggle` per AltGr hold (`chord_fired`).
- `src/main.rs`: single `HotkeyEvent::Toggle` arm, banner
  "static-dictation v1 — AltGr+L toggles dictation (the only shortcut)".
- stdin fallback: empty line / "toggle" toggles, "quit" exits. "start"/"stop" gone.
- Armed log: "hk: 20 input nodes armed (AltGr+L = dictation; no other hotkey)".

Known limit (shortcut): cancelling AltGr early means a *second* level-3 key pressed
while AltGr is still physically held types its base character. Fix by buffering the
AltGr press if that ever matters.

## Live proof — `bash scripts/verify_single_hotkey.sh /dev/input/event9`

Injects REAL evdev events into the grabbed keyboard node and reads the tool's
uinput re-emit device (`scripts/watch_reemit.py`). Default audio source is
temporarily a silent null sink and restored on exit.

Injected, in order: `97` (RightCtrl), `82` (KP0), `110` (INSERT), then an AltGr+L
chord, then `AltGr↓ L↓ L↑ L↓ L↑ AltGr↑`.

Results:

- Only two `hk: event9 -> Toggle` lines, one per chord; the double L tap inside
  the second chord produced no third toggle.
- Re-emit EV_KEY stream: `97,1 97,0 82,1 82,0 110,1 110,0 100,1 100,0 100,1 100,0`
  — inert keys pass through to apps untouched, AltGr press/release pair balanced,
  and **zero `38,*` events** (no "l" typed, no stuck modifier).
- `static-dictation: toggle event received (recording=false )` then
  `(recording=true )` — the only action the tool performs.

Gates: `cargo fmt --check` ok, `cargo test` 94 passed / 0 failed / 2 ignored,
`cargo clippy --all-targets -- -D warnings` clean, release binary 3,757,248 bytes
(3.58 MiB, all Rust, < 50 MB).

Not checked: a real microphone/voice transcription with this build (the earlier
end-to-end Groq proof is in `critique/altgr-l-live-proof.md`), and keyboards whose
AltGr is not keycode 100.
