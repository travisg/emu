#!/usr/bin/env bash
set -euo pipefail

# End-to-end test for REX, the preemptive executive: boot it, start the three
# letter tasks from its shell and watch them run -- which is the scheduler,
# the line clock, the context switch, the sleep machinery and the mailbox
# teletype driver all working at once -- then run two Tiny BASIC sessions
# through the shell's BASIC command (a program entered and RUN, a silent
# GOTO loop broken with Ctrl-C via the kernel's break flag, BYE handing the
# console back, and a second session LISTing the program the heap kept),
# then drive the rest of the commands and shut the machine down with HALT.
# The letter tasks come up stopped, so nothing prints until this asks it to.
#
# Two firsts for a 703 harness, both deliberate:
#
#   --fast-io   the teletype's pacing is not what is under test, and the
#               scheduling slices stay real machine time regardless -- the
#               line clock ignores the flag. What it speeds up is the wall
#               clock time the tasks' sleeps take, not the intervals the
#               guest observes, so REX reports an uptime in machine seconds
#               that runs far ahead of the test's own wall clock.
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

# Wait until at least N lines carry the pattern -- one prompt to a line, so
# this counts prompts rather than merely finding the first one.
wait_count() {
    local pattern="$1" want="$2" tries=0
    while (( tries < 300 )); do
        if (( $(grep -c -- "$pattern" "$LOG_FILE" 2>/dev/null || echo 0) >= want )); then
            return 0
        fi
        sleep 0.1
        (( tries += 1 ))
    done
    echo "error: timed out waiting for $want of '$pattern' (see $LOG_FILE)" >&2
    return 1
}

# Wait for the printer to fall quiet: two stable samples of the log's size.
# REX has type-ahead -- input queues while it is busy -- so this is not here
# to keep keystrokes from being dropped; it is here so each command's output
# is matched against a log that has stopped moving. Only usable with the
# letter tasks stopped, since otherwise the log never stops growing, which
# is what the STOP below is for.
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
        last=$size
        sleep 0.15
        (( tries += 1 ))
    done
    return 0
}

PROMPT='REX> '

# Set once the letter tasks have been seen running, after START and before
# anything else is typed, which is the only window in which counting their
# letters means anything.
BACKGROUND_RAN=0

# What the letter tasks have printed since START set them going. The line
# carrying the echo of START is dropped, and what follows it until the next
# command is typed is prompts and letters -- neither the prompt nor the STOP
# that ends the window contains an A, a B or a C, so nothing here counts the
# shell's own text as though a task had printed it.
background() {
    sed -n '/START/,$p' "$LOG_FILE" | tail -n +2
}

# True once every letter task has run several times and the printer has
# changed hands repeatedly. Thresholds and alternation rather than exact
# counts: the tasks sleep for 30, 45 and 60 ticks, so they drift through
# every phase against each other and no fixed pattern is owed.
enough_output() {
    local w a b c runs
    w=$(background)
    a=$(printf %s "$w" | tr -cd 'A' | wc -c)
    b=$(printf %s "$w" | tr -cd 'B' | wc -c)
    c=$(printf %s "$w" | tr -cd 'C' | wc -c)
    runs=$(printf %s "$w" | tr -cd 'ABC' | tr -s 'ABC' | wc -c)
    (( a >= 5 && b >= 5 && c >= 5 && runs >= 9 ))
}

# What the letter tasks printed between B alone being started and the STOP
# that follows. The terminal echoes every command back and the letters land
# on whatever line the printer is on, so the prompt and the echoed command
# have to come out before anything is counted -- STOP and HALT are made of
# the letters being counted. This is the deterministic half of the
# STOP/START check: the STAT taken while a task is running is not matched
# against exactly, because a letter can land anywhere in a line the printer
# is sharing, which is the whole point of the executive.
after_start() {
    sed -n '/START B/,/STOP/p' "$LOG_FILE" | tail -n +2 \
        | sed -e 's/REX>//g' -e 's/STOP//g'
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

# A timeout anywhere here is not fatal on its own -- the checks at the end
# report what actually reached the log, which says more than "timed out".
if wait_for 'REX 703 UP'; then

    # Nothing is running yet, so this one is quiet and exact.
    wait_count "$PROMPT" 1 && wait_quiet && printf 'STAT\r' >&3

    # Set the three of them going and let them run.
    wait_count "$PROMPT" 2 && wait_quiet && printf 'START\r' >&3
    tries=0
    while (( tries < 300 )) && ! enough_output; do
        sleep 0.1
        (( tries += 1 ))
    done
    enough_output && BACKGROUND_RAN=1

    # Then stop them: with the printer quiet, every command's output can be
    # matched exactly, and wait_quiet becomes usable for pacing.
    wait_count "$PROMPT" 3 && printf 'STOP\r' >&3

    wait_count "$PROMPT" 4 && wait_quiet && printf 'STAT\r' >&3
    wait_count "$PROMPT" 5 && wait_quiet && printf 'HELP\r' >&3
    wait_count "$PROMPT" 6 && wait_quiet && printf 'ECHO SHELL OUTPUT OK\r' >&3
    wait_count "$PROMPT" 7 && wait_quiet && printf 'FROB\r' >&3

    # A BASIC session: the console changes hands, a program goes in and
    # runs, and Ctrl-C -- which never enters the queue; SERV raises the
    # kernel's break flag -- stops a loop that prints nothing and reads
    # nothing. Each line is paced on BASIC's own READY count, the way the
    # standalone test paces on its prompt.
    wait_count "$PROMPT" 8 && wait_quiet && printf 'BASIC\r' >&3
    wait_for 'TINY BASIC UNDER REX' || true
    wait_count 'READY' 1 && wait_quiet && printf '10 FOR I=1 TO 3\r' >&3
    wait_count 'READY' 2 && wait_quiet && printf '20 PRINT "SQ";I*I\r' >&3
    wait_count 'READY' 3 && wait_quiet && printf '30 NEXT I\r' >&3
    wait_count 'READY' 4 && wait_quiet && printf '50 GOTO 50\r' >&3
    wait_count 'READY' 5 && wait_quiet && printf 'RUN\r' >&3
    wait_for 'SQ9' && printf '\003' >&3
    wait_for 'BREAK AT 50' || true
    wait_count 'READY' 6 && wait_quiet && printf 'BYE\r' >&3

    # Back at the shell: BASIC's node shows OFF, and a second session
    # finds the program still in the heap -- BYE parks the task, it does
    # not reset it.
    wait_count "$PROMPT" 9 && wait_quiet && printf 'STAT\r' >&3
    wait_count "$PROMPT" 10 && wait_quiet && printf 'BASIC\r' >&3
    wait_count 'READY' 7 && wait_quiet && printf 'LIST\r' >&3
    wait_count 'READY' 8 && wait_quiet && printf 'BYE\r' >&3

    # One task back on its feet, and only that one.
    wait_count "$PROMPT" 11 && wait_quiet && printf 'START B\r' >&3
    wait_for 'BBB' || true

    wait_count "$PROMPT" 12 && printf 'STOP\r' >&3
    wait_count "$PROMPT" 13 && wait_quiet && printf 'HALT\r' >&3
    wait_for 'REX 703 DOWN' || true
    wait_for 'stopping, Halted' || true
fi

# Closing the write end is a ctrl-d, so the emulator exits even if the guest
# never reached its HLT.
exec 3>&-
wait "$EMU_PID" || true

# What the shell had to have printed: STAT's uptime and task table with the
# tasks stopped and the shell itself running, the command list, ECHO's line
# on a line of its own (the terminal's echo of the command that asked for it
# begins with the prompt instead), the refusal, and B alone back at work --
# and, since A and C stay stopped through all of it, that the two letters
# they would otherwise have printed never appear after the START.
if grep -q 'REX 703 UP' "$LOG_FILE" \
    && (( BACKGROUND_RAN )) \
    && grep -q 'UPTIME [0-9][0-9]* SEC' "$LOG_FILE" \
    && grep -q '^A  OFF' "$LOG_FILE" \
    && grep -q '^SH RUN' "$LOG_FILE" \
    && grep -q '^BA OFF' "$LOG_FILE" \
    && grep -q '^ID RUN' "$LOG_FILE" \
    && grep -q '^COMMANDS HELP STAT UPTIME' "$LOG_FILE" \
    && grep -q '^SHELL OUTPUT OK' "$LOG_FILE" \
    && grep -q '^WHAT' "$LOG_FILE" \
    && grep -q 'TINY BASIC UNDER REX' "$LOG_FILE" \
    && grep -q '^SQ9' "$LOG_FILE" \
    && grep -q 'BREAK AT 50' "$LOG_FILE" \
    && (( $(grep -c '20 PRINT "SQ"' "$LOG_FILE") >= 2 )) \
    && (( $(after_start | tr -cd 'B' | wc -c) >= 3 )) \
    && (( $(after_start | tr -cd 'AC' | wc -c) == 0 )) \
    && grep -q 'REX 703 DOWN' "$LOG_FILE" \
    && grep -q 'stopping, Halted' "$LOG_FILE"; then
    echo "PASS: ray703 rex test ($EMU_BIN)"
    exit 0
fi

echo "FAIL: ray703 rex test (see $LOG_FILE)" >&2
exit 1
