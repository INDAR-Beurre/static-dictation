#!/usr/bin/env bash
# Reusable live-capture harness for the Wayland dictation pill.
# Usage: scripts/capture_pill.sh [HOLD_SECONDS] [SHOT_COUNT] [EVDEV_NODE] [KEYCODE]
# HOLD_SECONDS: RightCtrl push-to-talk hold duration (default 5; keep <=5, real input node)
# SHOT_COUNT:   number of evenly spaced grim shots while held (default 10)
# EVDEV_NODE:   keyboard evdev node (default /dev/input/event20)
# KEYCODE:      RightCtrl (default 97)
set -euo pipefail

HOLD="${1:-5}"
COUNT="${2:-10}"
NODE="${3:-/dev/input/event20}"
KEY="${4:-97}"

PROJ="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$PROJ/target/release/static-dictation"
OUT="$PROJ/live-proof"
LOG="$OUT/harness.log"

mkdir -p "$OUT"
TS=$(date +%s)
CAPDIR="$OUT/cap-$TS"
mkdir -p "$CAPDIR"

# (a) start binary fresh — pkill any strays first, best-effort
pkill -f "target/release/static-dictation" 2>/dev/null || true
sleep 0.5
env -C "$PROJ" \
  WAYLAND_DISPLAY=wayland-0 \
  XDG_RUNTIME_DIR=/run/user/1000 \
  "$BIN" >"$LOG" 2>&1 &
BINPID=$!

# (b) wait for pill to be live
sleep 2
if ! kill -0 "$BINPID" 2>/dev/null; then
  echo "ERROR: binary died on startup; log follows:" >&2
  cat "$LOG" >&2
  exit 1
fi

cleanup() {
  kill "$BINPID" 2>/dev/null || true
  wait "$BINPID" 2>/dev/null || true
}
trap cleanup EXIT

# (c) start PTT hold in background, (d) capture evenly spaced shots during the hold
python3 "$PROJ/scripts/inject_ptt.py" "$NODE" "$KEY" "$HOLD" &
INJPID=$!

# capture loop: shots spaced evenly across the hold window
DELAY=$(python3 -c "print(max(0.05, ($HOLD - 1.0) / max(1, $COUNT - 1)))")
for i in $(seq 1 "$COUNT"); do
  grim "$CAPDIR/shot-$(printf '%02d' "$i").png"
  [ "$i" -lt "$COUNT" ] && sleep "$DELAY"
done

wait "$INJPID"
# (e) one post-release shot
sleep 0.3
grim "$CAPDIR/shot-released.png"

# (f) binary killed via EXIT trap

# (g) list captured files
echo "Captured $COUNT held shots + 1 post-release into $CAPDIR:"
ls -la "$CAPDIR"
