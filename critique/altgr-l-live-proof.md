# AltGr+L end-to-end dictation proof (automated, 2026-11-15)

**Verdict: works 100% on this host with a synthetic voice.** No human speaker needed.

## What was injected
- `scripts/inject_chord.py /dev/input/event20` — writes real evdev events:
  `RIGHTALT(100)↓`, `L(38)↓`, `L(38)↑`, `RIGHTALT(100)↑`, SYN_REPORT after each, 50 ms apart.
- A virtual microphone: `pactl load-module module-null-sink sink_name=dictation-test`,
  `pactl set-default-source dictation-test.monitor`.
- Speech: `espeak -s 150 -w /tmp/h3.wav "hello this is a dictation test one two three"` →
  `ffmpeg -af volume=2.0 -ar 16000 -ac 1 /tmp/h3-16k.wav` →
  `paplay -d dictation-test /tmp/h3-16k.wav`.
- App: `env -u WAYLAND_DISPLAY -u DISPLAY NO_TTS=1 /tmp/static-dictation-h3`
  (headless on purpose so no typing lands in the user's windows; transcript is printed).

## Observed log (exact)
```
hk: 22 input nodes armed (AltGr+L / NumPad0 = toggle dictation, hold RightCtrl = push-to-talk)
hk: event20 RIGHTALT press
hk: event20 L press
hk: event20 -> Toggle
static-dictation: toggle event received (recording=false )
hk: event20 L release
hk: event20 RIGHTALT release
Hello this is a dictation test 123
static-dictation: typing unavailable (no wtype/xdotool or no display)
```

`Hello this is a dictation test 123` is the real `whisper-large-v3-turbo` result from Groq
(src/groq.rs:26 `model = "whisper-large-v3-turbo"`), delivered by `output::deliver`.

## Expected non-errors in this headless run
- `pill wayland connect failed (Could not find wayland compositor); retrying` / `pill render thread died` — display unset.
- `typing unavailable` — display unset. With the normal desktop session the same transcript is typed via wtype/xdotool.

## Cleanup
Trap restored the default source to `alsa_input.pci-0000_00_1f.3.analog-stereo` and unloaded the null sink.
Exit 0.

## Still human-only
A real microphone + real voice through AltGr+L (critique/altgr-l-test-plan.md H1–H8), and a
keyboard whose AltGr emits a code other than 100.
