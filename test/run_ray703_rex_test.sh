#!/usr/bin/env bash
set -euo pipefail

# End-to-end test for REX, the preemptive executive: boot it, watch the three
# tasks' letters appear and alternate -- which is the scheduler, the line
# clock, the context switch and the mailbox teletype driver all working at
# once -- then type the '.' that asks it to shut down and check for the
# down-message and the clean halt.
#
# Two firsts for a 703 harness, both deliberate:
#
#   --fast-io   the teletype's pacing is not what is under test, and the
#               scheduling slices stay real machine time regardless -- the
#               line clock ignores the flag. The tasks sleep between
#               letters, so what this speeds up is the wall-clock time the
#               sleeps take, not the intervals the guest observes.
#   -l          a hang guard. A wedged guest becomes "stopping, CycleLimit"
#               instead of a stuck test, and the verdict grep rejects it.
#               2e9 instructions is roughly ten times a normal run here.

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

EMU_BIN="${EMU_BIN:-$ROOT_DIR/target/debug/emu}"
ROM_FILE="$ROOT_DIR/roms/703/rex.bin"
LOG_FILE="${1:-$SCRIPT_DIR/ray703_rex_test.log}"

if [[ ! -x "$EMU_BIN" ]]; then
    echo "error: emulator binary not found at $EMU_BIN" >&2
    echo "build first with: cargo build" >&2
    exit 1
fi

if [[ ! -f "$ROM_FILE" ]]; then
    echo "error: rex image not found at $ROM_FILE" >&2
    echo "build it with: make -C test ray703-rex" >&2
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

# The task output since the banner: everything script(1) wrote before the
# banner is startup noise, and nothing after it but the tasks and the
# down-message can produce an A, a B or a C.
window() {
    sed -n '/REX 703 UP/,$p' "$LOG_FILE"
}

# True once every task has printed several letters and the stream has
# changed hands repeatedly. Thresholds and alternation rather than exact
# counts: the three tasks sleep for 30, 45 and 60 ticks, so they drift
# through every phase against each other and no fixed pattern is owed.
enough_output() {
    local w a b c runs
    w=$(window)
    a=$(printf %s "$w" | tr -cd 'A' | wc -c)
    b=$(printf %s "$w" | tr -cd 'B' | wc -c)
    c=$(printf %s "$w" | tr -cd 'C' | wc -c)
    runs=$(printf %s "$w" | tr -cd 'ABC' | tr -s 'ABC' | wc -c)
    (( a >= 5 && b >= 5 && c >= 5 && runs >= 9 ))
}

FIFO=$(mktemp -u)
mkfifo "$FIFO"
: > "$LOG_FILE"
trap 'rm -f "$FIFO"' EXIT

# -f flushes after every write, which is what makes the polling work at all.
# The image is named absolutely so this works from any directory.
script -qfec "$EMU_BIN -s ray703 -r $ROM_FILE --fast-io -l 2000000000" "$LOG_FILE" < "$FIFO" >/dev/null 2>&1 &
EMU_PID=$!

# Hold the write end open for the emulator's whole life: closing it looks
# like ctrl-d and shuts the machine down mid-test.
exec 3>"$FIFO"

# A timeout here is not fatal on its own -- the checks below report what
# actually reached the log, which says more than "timed out" does.
if wait_for 'REX 703 UP'; then
    tries=0
    while (( tries < 300 )) && ! enough_output; do
        sleep 0.1
        (( tries += 1 ))
    done
    printf '.' >&3            # ask REX to shut down
    wait_for 'REX 703 DOWN' || true
    wait_for 'stopping, Halted' || true
fi

# Closing the write end is a ctrl-d, so the emulator exits even if the guest
# never reached its HLT.
exec 3>&-
wait "$EMU_PID" || true

if grep -q 'REX 703 UP' "$LOG_FILE" \
    && enough_output \
    && grep -q 'REX 703 DOWN' "$LOG_FILE" \
    && grep -q 'stopping, Halted' "$LOG_FILE"; then
    echo "PASS: ray703 rex test ($EMU_BIN)"
    exit 0
fi

echo "FAIL: ray703 rex test (see $LOG_FILE)" >&2
exit 1
