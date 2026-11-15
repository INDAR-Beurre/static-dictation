# Live-capture notes — Wayland dictation pill

## Environment (proven working 2026-11-14)
- Wayland, mango compositor. `WAYLAND_DISPLAY=wayland-0`, `XDG_RUNTIME_DIR=/run/user/1000`.
- Screen: eDP-1, 2256×1504 logical, scale 1.5.
- Keyboard PTT node: `/dev/input/event20` (RightCtrl = code 97). Node is also **grabbed by the
  binary** — our injector writes to it from outside the grab; works fine.
- `grim` at /usr/bin/grim, PIL 13.0.0 for post-processing.

## Exact commands that work

```bash
# full harness: start binary, hold PTT 5s, 10 grim shots, post-release shot, kill binary
bash scripts/capture_pill.sh 5 10

# custom: hold 3s, 8 shots, different node/key
bash scripts/capture_pill.sh 3 8 /dev/input/event20 97

# rebuild crops + GIF from the newest cap dir (or pass dir explicitly)
python3 scripts/build_gif.py
python3 scripts/build_gif.py live-proof/cap-<ts>
```

Harness flow (`scripts/capture_pill.sh`): pkill strays → env vars → `./target/release/static-dictation`
(log → `live-proof/harness.log`) → 2s wait → `scripts/inject_ptt.py /dev/input/event20 97 <hold>` in
background → `COUNT` grim shots spaced evenly over the hold → post-release shot → kill binary (trap EXIT).
Output: `live-proof/cap-<date +%s>/shot-NN.png` + `shot-released.png`.

`scripts/inject_ptt.py` (copy kept in `scripts/` because **/tmp is wiped by systemd**; /tmp/inject_ptt.py
was still present this session): opens node O_WRONLY, writes two 24-byte little-endian `input_event`s
(`struct.pack('llHHi', sec, usec, 0x01, code, value)` then a SYN event), sleeps `hold`, writes release.
Roughly **0.3 s per grim full-screen shot** — the pacing anchor for interpreting timelines.

## Geometry (pill region)
- Full screenshot: pill occupies **x 1010..1450, y 0..95** (top-center of 2256×1504).
- Precise active-pill blob: **x 1011..1244, y 0..84** (from frame diff bbox).
- Crops: `(1010, 0, 1450, 95)` resized ×2 → **880×190** (`crop-NN.png`, ~78 KB each).
- `clock pill` sits nearby top-center; released-state crop (`crop-released.png`) is the idle
  reference (mean luma 26.3, stddev 34.8) vs live (~30.1, stddev 26.6).

## Timing reality (measured, 5s hold / 10 shots)
- Pill appears ~0.3 s after PTT down (shot-01 = pre-appear, diff-vs-released score 0).
- Shots 02–07 all have the pill (score 228; the 38..440 px diff subregion is the animated level meter).
- Pill gone by shot-08 (~4.2 s in) — with grim's ~0.3 s/shot overhead, 5 s hold yields ~6 usable frames.
- For more animation frames: raise HOLD to 5 (cap for safety) and COUNT to 12, or accept 444 ms spacing.

## Gotchas
- **Real input node**: injector writes to `/dev/input/event20` — hold ≤ 5 s. Nothing typed leaks into
  windows (node is grabbed for AltGr+L only; RightCtrl injection is not forwarded to apps).
- **Silent hold is safe**: no voice detected → no network calls, zero cost.
- **/tmp wiped by systemd**: keep `scripts/inject_ptt.py` as the durable copy; the harness uses the
  project copy, never /tmp.
- **Stray binary**: harness `pkill -f "target/release/static-dictation"` first, best-effort; verify with
  `pgrep -f static-dictation` after runs (this session left none).
- **clock pill overlap**: the system clock pill renders in the same top-center strip; use the
  diff-vs-released bbox (x 1011..1244) to isolate the dictation pill.
- Binary log on stdout names armed nodes; look for `hk: event20 RIGHTCTRL press → PttDown` to confirm
  the injection landed (harness.log tail shows exactly this).
