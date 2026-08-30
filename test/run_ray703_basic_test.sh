#!/usr/bin/env bash
set -euo pipefail

# A guest that dies early leaves the input pipe with no reader, and the next
# keystroke would then kill this script outright. Take the error instead, so
# a broken run reaches the checks at the bottom and reports what it found.
trap '' PIPE

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
#
# The session then does the one thing that is not typing at a prompt: it
# hammers the keyboard while a program runs. That interrupts the
# interpreter out in word page 1 or 2, which is the only way to reach the
# service routine's first memory reference in a page other than its own.

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

# Wait for the printer to stop.  The teletype is paced at its real ten
# characters a second, so a prompt reaches the log a character at a time and
# the last character of it lands while the guest is still inside the spin
# that waits for the line to drain -- typing on the strength of the text
# alone gets in ahead of the T.GETL that primes the line buffer, and those
# keystrokes are dropped exactly as the guest documents. What invites typing
# is not the prompt appearing but the silence after it, which is the cue the
# operator had too: the printer stopped moving.
# Two consecutive samples rather than one: a single quiet interval could be
# the host descheduling us mid-line rather than the guest being done.
wait_quiet() {
    local last='' size tries=0 stable=0
    while (( tries < 100 )); do
        size=$(wc -c < "$LOG_FILE")
        if [[ "$size" == "$last" ]]; then
            (( stable += 1 ))
            (( stable >= 2 )) && return 0
        else
            stable=0
        fi
        last="$size"
        sleep 0.15
        (( tries += 1 ))
    done
    echo "error: the guest never stopped printing (see $LOG_FILE)" >&2
    return 1
}

# Type one line, after waiting for the Nth READY that invites it.
type_at_ready() {
    local nth="$1" line="$2"
    wait_count 'READY' "$nth" || return 1
    wait_quiet || return 1
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
    wait_quiet || return 1
    printf '%s\r' '-6' >&3
    type_at_ready 12 'LIST'
    wait_count '20 @(I)=I\*I' 2 || return 1   # once typed, once listed
    type_at_ready 13 'PRINT 6*7;"OK"'

    # Keys struck while a program runs. Each one interrupts the interpreter
    # wherever it happens to be -- pages 1 and 2, running a program -- and
    # the service routine's first store resolves in that page unless it
    # leads with SMB, landing on live code instead of on T.SAVEA. Ctrl-C
    # then breaks back to READY and the evaluator has to still work.
    type_at_ready 14 'NEW'
    type_at_ready 15 '10 GOTO 10'
    type_at_ready 16 'RUN'
    wait_quiet || return 1
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        printf 'X' >&3
        sleep 0.15                      # the keyboard delivers ten a second
    done
    printf '\003' >&3                   # ctrl-c, which the driver flags
    wait_count 'BREAK AT 10' 1 || return 1
    type_at_ready 17 'PRINT 8*9;"KB"'
    type_at_ready 18 'BYE'
    # Wait for the halt before the caller closes the pipe. Four characters
    # take four tenths of a second to reach a teletype, and the ctrl-d that
    # closing it looks like would shut the machine down with BYE still on
    # its way in.
    wait_count 'stopping, Halted' 1 || return 1
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
    && grep -q '72KB' "$LOG_FILE" \
    && grep -q 'stopping, Halted' "$LOG_FILE"; then
    echo "PASS: ray703 basic test ($EMU_BIN)"
    exit 0
fi

echo "FAIL: ray703 basic test (see $LOG_FILE)" >&2
exit 1
