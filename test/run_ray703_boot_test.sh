#!/usr/bin/env bash
set -euo pipefail

# End-to-end test for the disc controller's LOAD button: build a bootable
# disc image with the one-sector boot program in sector 0, track 0, boot it
# with the ray703-load subsystem, and check that the guest printed its banner
# and halted. This is the 706-era boot ritual with the subsystem standing in
# for the operator's finger on the button.

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

EMU_BIN="${EMU_BIN:-$ROOT_DIR/target/debug/emu}"
BOOT_BIN="$ROOT_DIR/roms/703/boot.bin"
LOG_FILE="${1:-$SCRIPT_DIR/ray703_boot_test.log}"

if [[ ! -x "$EMU_BIN" ]]; then
    echo "error: emulator binary not found at $EMU_BIN" >&2
    echo "build first with: cargo build" >&2
    exit 1
fi

if [[ ! -f "$BOOT_BIN" ]]; then
    echo "error: boot sector not found at $BOOT_BIN" >&2
    echo "build it with: make -C test ray703-boot" >&2
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

WORK=$(mktemp -d)
FIFO=$(mktemp -u)
mkfifo "$FIFO"
: > "$LOG_FILE"
trap 'rm -f "$FIFO"; rm -rf "$WORK"' EXIT

# A platter-sized image with the boot program in sector 0 and nothing else --
# the guard is real: the LOAD button reads one 94-byte sector, so a program
# that outgrew it would boot truncated.
DISC_IMG="$WORK/boot-disc.img"
python3 - "$BOOT_BIN" "$DISC_IMG" <<'EOF'
import sys
boot = open(sys.argv[1], 'rb').read()
assert len(boot) <= 94, f'boot sector is {len(boot)} bytes; the LOAD button reads 94'
image = bytearray(770048)
image[:len(boot)] = boot
open(sys.argv[2], 'wb').write(image)
EOF

# The fifo is held open for the emulator's whole life -- closing it looks
# like ctrl-d -- but nothing is typed: the guest boots, prints and halts.
script -qfec "$EMU_BIN -s ray703-load -r $DISC_IMG" "$LOG_FILE" < "$FIFO" >/dev/null 2>&1 &
EMU_PID=$!
exec 3>"$FIFO"

wait_for '703 BOOT' || true

exec 3>&-
wait "$EMU_PID" || true

if grep -q 'LOAD pressed' "$LOG_FILE" \
    && grep -q '703 BOOT' "$LOG_FILE" \
    && grep -q 'stopping, Halted' "$LOG_FILE"; then
    echo "PASS: ray703 boot test ($EMU_BIN)"
    exit 0
fi

echo "FAIL: ray703 boot test (see $LOG_FILE)" >&2
exit 1
