#!/usr/bin/env bash
# Rebuild the durable master transcript from the per-page files.
#
#   ./sync-transcript.sh <page-dir> <header-file> <output>
#
# Per-page files are what the transcribing agents write, one apiece, so that
# several can work at once without fighting over one file. This concatenates
# them in page order and stamps the progress line in the header.
set -euo pipefail
[[ $# -eq 3 ]] || { echo "usage: $0 <page-dir> <header-file> <output>" >&2; exit 2; }
pages=$(find "$1" -name 'page-*.txt' | wc -l)
{
    sed "s/^Transcription progress:.*/Transcription progress: $pages of 54 scanned pages./" "$2"
    for f in $(find "$1" -name 'page-*.txt' | sort); do echo; cat "$f"; done
} > "$3"
echo "$3: $pages pages"
