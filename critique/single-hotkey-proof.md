# One dedicated key: AltGr+L, nothing else

User asked (m05613, answer "3." in m05646): "make it only the dictation key possible
and that key is only for dictation no other aczions".

User later reported (m05796): "it still prints "ł"" — pressing the chord still typed
the Polish letter. See "Root cause of ł" below.

## What changed

- `HotkeyEvent` now has exactly one variant, `Toggle`.
- `src/hk.rs` binds only `KEY_RIGHTALT`(100) + `KEY_L`(38). `KEY_RIGHTCTRL`(97),
  `KEY_KP0`(82), `KEY_INSERT`(110) were removed entirely — no PTT, no NumPad0.
  `src/hk.rs::only_altgr_l_is_bound` asserts they produce `None` on press and release.
- The chord is fully consumed, so it cannot cause any other app action:
  - **AltGr press is buffered, not re-emitted.** L discards the buffer; any other
    key flushes it first (so AltGr+A / AltGr+E still type ą / €). Apps see no
    level-3 modifier for the chord, and no synthetic release is needed.
  - Every L press / autorepeat / release while AltGr is down is swallowed
    (`src/hk.rs::l_event_hidden`), so no "l" is typed even if L is tapped twice
    during one hold.
  - The real AltGr keyup is swallowed when its press was never sent.
  - Exactly one `Toggle` per AltGr hold (`chord_fired`).
- Only watchers that actually grabbed a keyboard re-emit events; a read-only
  listener would otherwise duplicate keys the compositor already received.
- Grabbing now targets any keyboard with `KEY_L` (guarded by `KEY_A` so mice and
  touchpads are left alone), not just nodes that also advertise AltGr — a remapper
  clone can emit L without advertising AltGr.
- `src/main.rs`: single `HotkeyEvent::Toggle` arm, banner
  "static-dictation v1 — AltGr+L toggles dictation (the only shortcut)".
- stdin fallback: empty line / "toggle" toggles, "quit" exits. "start"/"stop" gone.
- Armed log: "hk: 20 input nodes armed (AltGr+L = dictation; no other hotkey)".

## Root cause of ł

The first version re-emitted the physical AltGr press to apps immediately, then
tried to cancel it with a synthetic `AltGr release` when L fired. That is fragile:
a duplicated press (a remapper clone, or AltGr delivered on another `/dev/input`
node) leaves one press unbalanced, so LevelThree stays down and the *next* plain L
types `ł`. A synthetic release for a press that never reached the virtual device
is silently dropped by the kernel, which makes the imbalance worse.

Fix: never emit a release for a press that was never sent. Buffer the press, discard
it on L, flush it on any other key. `src/hk.rs::chord_never_sends_altgr_or_l_to_apps`,
`other_level_three_keys_still_work`, `bare_altgr_tap_types_nothing` are regression tests.

## Live proof — `bash scripts/verify_single_hotkey.sh /dev/input/event9`

Injects REAL evdev events into the grabbed keyboard node and reads the tool's
uinput re-emit device (`scripts/watch_reemit.py`). Default audio source is
temporarily a silent null sink and restored on exit.

Injected, in order: `97` (RightCtrl), `82` (KP0), `110` (INSERT), then an AltGr+L
chord, then `AltGr↓ L↓ L↑ L↓ L↑ AltGr↑`, two plain `L` taps, `AltGr+A`, and a
final plain `L`.

Results (all five regression assertions PASS, script exits 0):

- Exactly two `hk: event9 -> Toggle` lines, one per chord; the double L tap inside
  the second chord produced no third toggle.
- Re-emit EV_KEY stream: `97,1 97,0 82,1 82,0 110,1 110,0` (inert keys), then
  `38,1 38,0` (plain L), `100,1 30,1 30,0 100,0` (level-3 char, balanced),
  `38,1 38,0` (plain L). **Zero `38,*` from either chord and zero stray `100,1`** —
  no "l", no "ł", no stuck modifier.
- The only AltGr press in the stream is the one flushed for AltGr+A, and it has a
  matching release.
- `static-dictation: toggle event received (recording=false )` then
  `(recording=true )` — the only action the tool performs.
- End-to-end dictation re-proved with this build: injecting the chord into
  `/dev/input/event9` with a null-sink voice sample produced
  `Hello this is a dictation test 1, 2, 3` (a real Groq
  `whisper-large-v3-turbo` call), headless so no text was typed into a window.

Gates: `cargo fmt --check` ok, `cargo test` 97 passed / 0 failed / 2 ignored,
`cargo clippy --all-targets -- -D warnings` clean, release binary 3,751,528 bytes
(3.58 MiB, all Rust, < 50 MB).

shortcut: a watcher that fails to grab its device is only a listener; if a host
grabs a keyboard without relaying it onward, keys from that node would be lost.
Upgrade with a per-node relay decision if that host appears.

Not checked: a real microphone/voice transcription (the injected sample is
synthetic speech), typing the transcript into a focused Wayland window, and
keyboards whose AltGr is not keycode 100.
