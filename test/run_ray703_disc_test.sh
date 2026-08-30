#!/usr/bin/env bash
set -euo pipefail

# End-to-end test for the 703's 74601 disc: boot the exerciser image against a
# blank disc image and check that it printed DISC TEST PASS and halted, then
# check the disc image itself for the pattern the guest wrote. The guest does
# a write spanning the end of a track, a read back and a verify, entirely
# under interrupt control on two levels, so this exercises the core, both
# interrupt levels, the DIO channel, the DMA completion path, the teletype
# and the write-through to the host file at once.

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

EMU_BIN="${EMU_BIN:-$ROOT_DIR/target/debug/emu}"
ROM_FILE="$ROOT_DIR/roms/703/disc.bin"
LOG_FILE="${1:-$SCRIPT_DIR/ray703_disc_test.log}"

if [[ ! -x "$EMU_BIN" ]]; then
    echo "error: emulator binary not found at $EMU_BIN" >&2
    echo "build first with: cargo build" >&2
    exit 1
fi

if [[ ! -f "$ROM_FILE" ]]; then
    echo "error: disc exerciser image not found at $ROM_FILE" >&2
    echo "build it with: make -C test ray703-disc" >&2
    exit 1
fi

for tool in script python3; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "error: '$tool' command is required" >&2
        exit 1
    fi
done

# Wait for a string to appear in the live log rather than sleeping a fixed
# amount -- see run_ray703_demo_test.sh for why.
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

# The machine mounts disks/ray703-disc{0..3}.img under the current directory,
# so the emulator runs in a scratch directory holding a fresh blank image --
# exactly the platter size, all zeros. python3 makes the blank because
# truncate(1) does not exist everywhere the tests run.
WORK=$(mktemp -d)
FIFO=$(mktemp -u)
mkfifo "$FIFO"
: > "$LOG_FILE"
trap 'rm -f "$FIFO"; rm -rf "$WORK"' EXIT

mkdir "$WORK/disks"
DISC_IMG="$WORK/disks/ray703-disc0.img"
python3 -c "open('$DISC_IMG', 'wb').truncate(770048)"

# The fifo is held open for the emulator's whole life -- closing it looks like
# ctrl-d -- but nothing is ever typed: the guest runs to its HLT by itself.
(cd "$WORK" && exec script -qfec "$EMU_BIN -s ray703 -r $ROM_FILE" "$LOG_FILE") < "$FIFO" >/dev/null 2>&1 &
EMU_PID=$!
exec 3>"$FIFO"

wait_for '703 DISC EXERCISER' && wait_for 'DISC TEST PASS' || true

exec 3>&-
wait "$EMU_PID" || true

# The guest's own read-back only proves the round trip through the device;
# the image file holds the write-through, so check the pattern landed at the
# linear offset of track 2, sector 127 -- and ran over into track 3, sector 0,
# which is the 5-9.4 track continuation the guest's span exercises.
check_image() {
    python3 - "$DISC_IMG" <<'EOF'
import sys
data = open(sys.argv[1], 'rb').read()
off = (2 * 128 + 127) * 47 * 2
got = [int.from_bytes(data[off + 2 * i:off + 2 * i + 2], 'big') for i in range(94)]
want = [i ^ 0xA5C3 for i in range(94)]
assert got == want, f'pattern mismatch at byte {off}: {got[:4]} != {want[:4]}'
assert data[off + 188:off + 196] == bytes(8), 'wrote past the span'
EOF
}

if grep -q '703 DISC EXERCISER' "$LOG_FILE" \
    && grep -q 'DISC TEST PASS' "$LOG_FILE" \
    && grep -q 'stopping, Halted' "$LOG_FILE" \
    && check_image; then
    echo "PASS: ray703 disc test ($EMU_BIN)"
    exit 0
fi

echo "FAIL: ray703 disc test (see $LOG_FILE)" >&2
exit 1
