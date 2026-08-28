#!/usr/bin/env bash
# Rebuild the durable master transcript from the per-page files.
#
#   ./sync-transcript.sh <page-dir> <header-file> <output> <total-pages>
#
# Per-page files are what the transcribing agents write, one apiece, so that
# several can work at once without fighting over one file. This concatenates
# them in page order and stamps the progress line in the header.
#
# <total-pages> is how many pages the document has: 54 for X-RAY, 28 for the
# relocating loader. It is an argument rather than a constant because the point
# of the progress line is to say what is still missing, and a total that
# silently belongs to another document would say the opposite.
set -euo pipefail
[[ $# -eq 4 ]] || { echo "usage: $0 <page-dir> <header-file> <output> <total-pages>" >&2; exit 2; }
pages=$(find "$1" -name 'page-*.txt' | wc -l)
{
    sed "s/^Transcription progress:.*/Transcription progress: $pages of $4 scanned pages./" "$2"
    for f in $(find "$1" -name 'page-*.txt' | sort); do echo; cat "$f"; done
} > "$3"
echo "$3: $pages pages"
