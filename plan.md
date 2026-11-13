# Dictation-RS — v1 Plan

## Goal

A **normal dictation tool** for now: no bar at the top (that's the next step), no pill-shaped menu yet.
Super lightweight, always-cloud: audio → **Groq API** (`whisper-large-v3-turbo`) → text out.
Built in **Rust**. Privacy: audio is only sent while dictation is active. Works perfectly.

## Architecture (v1)

Single binary `static-dictation`. Binary + 4 thin services, all behind
structs in one crate, no async, no tokio — plain threads, channels, poll loops.
"Super pedigree": cloud-first, zero local models, luxury simplicity.

```
main → config (env GROQ_API_KEY)
     → hotkey Monitor (evdev raw: /dev/input/event*) # global, all keyboards
         on hit → [Re]Acquire mic + start streaming … (toggle / PTT)
     → Capture Service (cpal, 16k mono f32, ringbuf)
     → VAD/endpointer service (30ms frames energy+ZCR, 700ms silence → finalize)
     → Groq Client (reqwest blocking multipart, whisper-large-v3-turbo)
     → Output service:
         Wayland/ X11: wl-copy / xclip type / wtype         (Wayland by default, X11 fallback)
         Stdout fallback: always prints line to stdout too
     → Feedback: espeak-ng "[OK] done" on success, error phrase on failure (config-off via env NO_TTS=1)
```

## Hotkeys v1 (be super PED (this is /PED, push-everyday-D))

- us layout: RightCtrl pressed (vaeg)
  = **Toggle** dictation: Press → start grabbing mic; release does nothing…
  Wait — v1 keeps it simple: **RightCtrl = toggle**; Dictation light in console.

Let me correct: user's push-to-talk is the super+D mean?

**Decision for v1** — "super plus D"-like behavior comes from the toggle/PTT pair:
- **RightCtrl** → toggle hold-to-use (ptt) as chosen default: hold key down = record; release = finalize.
- **NumPad0** → toggle mode (press once = start, press again = stop). Also: RightCtrl long-press (>600ms) does not fall to app.

**Super+D**-style: user said like "super plus D, and like, it should work perfectly like that too",
Assume they mean Windows-style Win+H dictate key combo. On Linux evdev we can't cleanly
emulate "Super+D" as one toggle unless we track a Super modifier bit ourselves — Super is
just a normal key at evdev level. v1: track Left/Right Super state; when Super is held
**and D** is pressed → toggle dictation. Releases pass through untouched.

## Rules

- No new logic beyond P0 list; no settings UI, no config file.
- Everything eprintln!-logged, no tracing/log crates
- Tests: coarse integration test only, off by default (needs mic), fine grained for VAD segmented
  (`-- ` `cargo test` unit tests only for pure logic: resampler, VAD params, hotkey state machine)
- `cargo test` must pass offline

