#!/usr/bin/env bash
set -euo pipefail

# End-to-end test for Tiny BASIC on the Raytheon 703: boot the image, type a
# program at the READY prompt, RUN it, answer its INPUT, and check the log
# for output only the guest could have produced -- which exercises the
# editor, the evaluator, the hardware multiply/divide, FOR/NEXT, GOSUB, the
# array, RND's bounds, and the interrupt-driven teletype in one sitting.
#
# Every line of input waits for the guest output that invites it: the guest
# drops keys typed while it is still processing the previous line, exactly
# as a busy 1968 machine dropped what a Model 33 fed it, so pacing on
# output is part of driving it correctly.

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

EMU_BIN="${EMU_BIN:-$ROOT_DIR/target/debug/emu}"
ROM_FILE="$ROOT_DIR/roms/703/basic.bin"
LOG_FILE="${1:-$SCRIPT_DIR/ray703_basic_test.log}"

if [[ ! -x "$EMU_BIN" ]]; then
    echo "error: emulator binary not found at $EMU_BIN" >&2
    echo "build first with: cargo build" >&2
    exit 1
fi

if [[ ! -f "$ROM_FILE" ]]; then
    echo "error: basic image not found at $ROM_FILE" >&2
    echo "build it with: make -C test ray703-basic" >&2
    exit 1
fi

if ! command -v script >/dev/null 2>&1; then
    echo "error: 'script' command is required" >&2
    exit 1
fi

# Wait for the Nth occurrence of a string in the live log rather than
# sleeping a fixed amount; the count is what lets the same prompt pace
# several lines of input.
wait_count() {
    local pattern="$1" want="${2:-1}" tries=0
    while (( tries < 200 )); do
        if (( $(grep -c -- "$pattern" "$LOG_FILE" 2>/dev/null || true) >= want )); then
            return 0
        fi
        sleep 0.1
        (( tries += 1 ))
    done
    echo "error: timed out waiting for $want x '$pattern' (see $LOG_FILE)" >&2
    return 1
}

# Type one line, after waiting for the Nth READY that invites it.
type_at_ready() {
    local nth="$1" line="$2"
    wait_count 'READY' "$nth" || return 1
    printf '%s\r' "$line" >&3
}

FIFO=$(mktemp -u)
mkfifo "$FIFO"
: > "$LOG_FILE"
trap 'rm -f "$FIFO"' EXIT

# -f flushes after every write, which is what makes wait_count work at all.
script -qfec "$EMU_BIN -s ray703 -r $ROM_FILE" "$LOG_FILE" < "$FIFO" >/dev/null 2>&1 &
EMU_PID=$!

# Hold the write end open for the emulator's whole life: closing it looks
# like ctrl-d and shuts the machine down mid-test.
exec 3>"$FIFO"

FAILED=0
run_session() {
    wait_count 'RAY703 TINY BASIC' 1 || return 1
    type_at_ready  1 '10 FOR I=1 TO 3'
    type_at_ready  2 '20 @(I)=I*I'
    type_at_ready  3 '30 NEXT I'
    type_at_ready  4 '40 PRINT "SQ";@(1);@(2);@(3)'
    type_at_ready  5 '50 IF RND(10)>0 IF RND(10)<11 GOSUB 100'
    type_at_ready  6 '60 INPUT X'
    type_at_ready  7 '70 PRINT "GOT";X;" ";0-5/1'
    type_at_ready  8 '80 END'
    type_at_ready  9 '100 PRINT "RNDOK"'
    type_at_ready 10 '110 RETURN'
    type_at_ready 11 'RUN'
    wait_count '? ' 1 || return 1       # INPUT's prompt
    printf '%s\r' '-6' >&3
    type_at_ready 12 'LIST'
    wait_count '20 @(I)=I\*I' 2 || return 1   # once typed, once listed
    type_at_ready 13 'PRINT 6*7;"OK"'
    type_at_ready 14 'BYE'
    return 0
}
run_session || FAILED=1

# Closing the write end is a ctrl-d, so the emulator exits even if the guest
# never reached its HLT.
exec 3>&-
wait "$EMU_PID" || true

if [[ "$FAILED" == 0 ]] \
    && grep -q 'SQ149' "$LOG_FILE" \
    && grep -q 'RNDOK' "$LOG_FILE" \
    && grep -q -- 'GOT-6 -5' "$LOG_FILE" \
    && grep -q '42OK' "$LOG_FILE" \
    && grep -q 'stopping, Halted' "$LOG_FILE"; then
    echo "PASS: ray703 basic test ($EMU_BIN)"
    exit 0
fi

echo "FAIL: ray703 basic test (see $LOG_FILE)" >&2
exit 1
