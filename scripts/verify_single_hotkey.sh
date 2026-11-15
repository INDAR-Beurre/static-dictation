#!/usr/bin/env bash
# Live proof: AltGr+L is the only hotkey; the whole chord is consumed.
set -u
cd /home/alex/.t3/scratch/2026-11-13-can-you-please-like-i-e1185665
NODE=${1:-/dev/input/event9}
ORIG=$(pactl get-default-source)
pactl load-module module-null-sink sink_name=hkonly-sink >/dev/null 2>&1
pactl set-default-source hkonly-sink.monitor
restore() {
    pactl set-default-source "$ORIG" >/dev/null 2>&1
    pactl unload-module module-null-sink >/dev/null 2>&1
}
trap restore EXIT
rm -f /tmp/hkonly.log /tmp/hkonly.stdout /tmp/hkonly-reemit.txt /tmp/hkonly-watch.log
python3 scripts/watch_reemit.py 10 /tmp/hkonly-reemit.txt >/tmp/hkonly-watch.log 2>&1 &
WATCH=$!
cp target/release/static-dictation /tmp/static-dictation-hk
env -u WAYLAND_DISPLAY -u DISPLAY NO_TTS=1 timeout 10 /tmp/static-dictation-hk \
    >/tmp/hkonly.stdout 2>/tmp/hkonly.log &
BIN=$!
sleep 2
for k in 97 82 110; do
    python3 scripts/inject_ptt.py "$NODE" "$k" 0.05 >/dev/null
    sleep 0.4
done
sleep 0.6
python3 scripts/inject_chord.py "$NODE" >/dev/null
sleep 0.6
# Hold AltGr and tap L twice: still exactly one dictation action, and no "l" typed.
python3 - "$NODE" <<'PY'
import os, struct, sys, time
fd = os.open(sys.argv[1], os.O_WRONLY)
def e(code, value):
    t = time.time()
    return struct.pack("llHHi", int(t), int((t % 1) * 1e6), 0x01, code, value)
def syn():
    t = time.time()
    return struct.pack("llHHi", int(t), int((t % 1) * 1e6), 0, 0, 0)
for code, value in [(100, 1), (38, 1), (38, 0), (38, 1), (38, 0), (100, 0)]:
    os.write(fd, e(code, value) + syn())
    time.sleep(0.05)
os.close(fd)
PY
sleep 1.5
wait "$WATCH" || true
wait "$BIN" || true
echo "=== stdout ==="; cat /tmp/hkonly.stdout
echo "=== hk log (hotkey lines) ==="; grep -E "armed|->|toggle event|hk:" /tmp/hkonly.log
echo "=== re-emit EV_KEY (code,value) ==="; cat /tmp/hkonly-reemit.txt
echo "=== watcher ==="; cat /tmp/hkonly-watch.log
