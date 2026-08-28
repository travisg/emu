#!/usr/bin/env bash
# Rebuild the per-page working files from the durable master transcript.
#
#   ./split-transcript.sh <master.txt> <page-dir>
#
# The inverse of sync-transcript.sh, and the reason the master is enough on its
# own. The per-page files are working state -- they live in a scratch directory
# so that several transcribing agents can each own one without fighting over a
# single file -- and scratch directories do not survive. Everything needed to
# recreate them is in the master, because sync-transcript.sh only concatenates:
# each page arrives under its own `====` rule followed by a `PAGE NN` line.
#
# So the durable pair is the master transcript plus the page images in the PDF.
# Anything else under a scratch path is regenerable and may be deleted freely.
set -euo pipefail
[[ $# -eq 2 ]] || { echo "usage: $0 <master.txt> <page-dir>" >&2; exit 2; }
master=$1
outdir=$2
mkdir -p "$outdir"

awk -v outdir="$outdir" '
    # A rule line opens a page; the PAGE line right after it names the number.
    /^=+$/          { rule = $0; pending = 1; next }
    pending && /^PAGE[ \t]+[0-9]+/ {
        n = $2
        out = sprintf("%s/page-%03d.txt", outdir, n)
        print rule > out
        print $0 > out
        pending = 0
        pages++
        next
    }
    # A rule with no PAGE after it is the header block above page 2; drop it,
    # sync-transcript.sh puts the header back from its own file.
    pending         { pending = 0 }
    out             { print > out }
    END             { printf "%d pages\n", pages }
' "$master"

# sync-transcript.sh separates pages with a blank line, which lands at the tail
# of the page above. Drop trailing blanks so the round trip is exact.
for f in "$outdir"/page-*.txt; do
    printf '%s\n' "$(< "$f")" > "$f.tmp" && mv "$f.tmp" "$f"
done

echo "$outdir: $(find "$outdir" -name 'page-*.txt' | wc -l) page files"
