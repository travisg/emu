#!/usr/bin/env bash
set -euo pipefail

# End-to-end test for the Raytheon 703: boot the demo image, type a line at it,
# and check that the machine echoed the line back folded to upper case and then
# halted. That exercises the whole stack -- the core, the interrupt system, the
# DIO channel, the teletype device and the terminal frontend -- because the
# demo's echo runs entirely out of a level 0 interrupt service routine.

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

EMU_BIN="${EMU_BIN:-$ROOT_DIR/target/debug/emu}"
ROM_FILE="$ROOT_DIR/roms/703/demo.bin"
LOG_FILE="${1:-$SCRIPT_DIR/ray703_demo_test.log}"

# Typed in lower case; the demo folds it, so finding the upper case version in
# the log proves the guest produced it. The terminal is in raw mode by then, so
# nothing else can be echoing.
PHRASE='ray703 echo test pass'
EXPECT='RAY703 ECHO TEST PASS'

if [[ ! -x "$EMU_BIN" ]]; then
    echo "error: emulator binary not found at $EMU_BIN" >&2
    echo "build first with: cargo build" >&2
    exit 1
fi

if [[ ! -f "$ROM_FILE" ]]; then
    echo "error: demo image not found at $ROM_FILE" >&2
    echo "build it with: make -C test ray703-demo" >&2
    exit 1
fi

if ! command -v script >/dev/null 2>&1; then
    echo "error: 'script' command is required" >&2
    exit 1
fi

# Wait for a string to appear in the live log rather than sleeping a fixed
# amount. The emulator only starts listening once it has opened the terminal
# and put it in raw mode, and anything typed before that is swallowed by the
# line discipline -- which looks exactly like a broken emulator.
wait_for() {
    local pattern="$1" tries=0
    while (( tries < 200 )); do
        if grep -q -- "$pattern" "$LOG_FILE" 2>/dev/null; then
            return 0
        fi
        sleep 0.1
        (( tries += 1 ))
    done
    echo "error: timed out waiting for '$pattern' (see $LOG_FILE)" >&2
    return 1
}

FIFO=$(mktemp -u)
mkfifo "$FIFO"
: > "$LOG_FILE"
trap 'rm -f "$FIFO"' EXIT

# -f flushes after every write, which is what makes wait_for work at all.
script -qfec "$EMU_BIN -s ray703" "$LOG_FILE" < "$FIFO" >/dev/null 2>&1 &
EMU_PID=$!

# Hold the write end open for the emulator's whole life: closing it looks like
# ctrl-d and shuts the machine down mid-test.
exec 3>"$FIFO"

# A timeout here is not fatal on its own -- the checks below report what
# actually reached the log, which says more than "timed out" does.
if wait_for 'RAYTHEON 703 READY'; then
    printf '%s\r' "$PHRASE" >&3
    wait_for "$EXPECT" || true
    printf '.' >&3          # the demo halts on a period
fi

# Closing the write end is a ctrl-d, so the emulator exits even if the guest
# never reached its HLT.
exec 3>&-
wait "$EMU_PID" || true

if grep -q 'RAYTHEON 703 READY' "$LOG_FILE" \
    && grep -q -- "$EXPECT" "$LOG_FILE" \
    && grep -q 'stopping, Halted' "$LOG_FILE"; then
    echo "PASS: ray703 demo test ($EMU_BIN)"
    exit 0
fi

echo "FAIL: ray703 demo test (see $LOG_FILE)" >&2
exit 1
