# static-dictation

Super-lightweight, always-on dictation for Linux. Hold-to-talk or toggle,
speech goes to Groq's `whisper-large-v3-turbo`, and the transcription is typed
directly into the focused window. Pure Rust, ~3.4 MiB release binary, no UI
by default.

## Build

```sh
cargo build --release
```

Requires Linux with evdev access to the keyboard devices and a
`/dev/uinput` the user can write (the `input` group). Audio via PulseAudio/
PipeWire. `wtype` (Wayland) or `xdotool` (X11) delivers the typed text.

## API key

```sh
mkdir -p ~/.config/static-dictation
printf '%s' "$GROQ_KEY" > ~/.config/static-dictation/api_key
chmod 600 ~/.config/static-dictation/api_key
```

`GROQ_API_KEY` in the environment overrides the file.

## Run

```sh
./target/release/static-dictation
```

| Shortcut | Action |
|---|---|
| `AltGr+L` | Toggle continuous dictation (the only shortcut) |

While toggled, speech is transcribed in chunks and typed into the focused
application. A voice gate requires ~300 ms of voiced audio before a Groq
request is spent, so room noise never produces output.

Set `GROQ_BASE_URL` to point at a different OpenAI-compatible endpoint.

## QA tooling

- `scripts/verify_single_hotkey.sh` — end-to-end AltGr+L proof with a null
  audio sink.
- `scripts/make_clock_morph.py` — 60-frame design video of the clock-pill to
  waveform morph.
- `STATIC_DICTATION_DUMP_FRAMES=<dir>` / `STATIC_DICTATION_DUMP_SPEECH=1` —
  offscreen frame dump for motion/colour measurement (QA only).
- `critique/` — the design rounds, acceptance measurements, and proof docs.
