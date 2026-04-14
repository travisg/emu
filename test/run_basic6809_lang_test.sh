#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

EMU_BIN="$ROOT_DIR/build-emu/emu"
ROM_FILE="$ROOT_DIR/roms/6809/BASIC.HEX"
PROGRAM_FILE="$SCRIPT_DIR/basic6809_lang_test.bas"
LOG_FILE="${1:-$SCRIPT_DIR/basic6809_lang_test.log}"

if [[ ! -x "$EMU_BIN" ]]; then
    echo "error: emulator binary not found at $EMU_BIN" >&2
    echo "build first with: make" >&2
    exit 1
fi

if [[ ! -f "$ROM_FILE" ]]; then
    echo "error: rom file not found at $ROM_FILE" >&2
    exit 1
fi

if ! command -v script >/dev/null 2>&1; then
    echo "error: 'script' command is required" >&2
    exit 1
fi

if ! command -v perl >/dev/null 2>&1; then
    echo "error: 'perl' command is required" >&2
    exit 1
fi

TMP_INPUT=$(mktemp)
TMP_CRLF=$(mktemp)
trap 'rm -f "$TMP_INPUT" "$TMP_CRLF"' EXIT

{
    echo "NEW"
    cat "$PROGRAM_FILE"
    echo "RUN"
} > "$TMP_INPUT"

perl -pe 's/\n/\r/g' "$TMP_INPUT" > "$TMP_CRLF"
printf '\004' >> "$TMP_CRLF"

script -qfec "$EMU_BIN -s 6809 -r $ROM_FILE" "$LOG_FILE" < "$TMP_CRLF"

if grep -q "BASIC LANGUAGE TEST PASS" "$LOG_FILE" \
    && ! grep -Eq '^[[:space:]]*FAIL[[:space:]]' "$LOG_FILE" \
    && ! grep -Eq '^\?.*ERROR' "$LOG_FILE"; then
    echo "PASS: 6809 BASIC language test"
    exit 0
fi

echo "FAIL: 6809 BASIC language test (see $LOG_FILE)" >&2
exit 1
