#!/usr/bin/env bash
# vim: ts=4:sw=4:expandtab:
#
# Cut a page of a scanned Raytheon 700-series program listing into strips that
# are actually readable.
#
# The listings are landscape content printed on portrait pages, scanned at 600
# dpi as 1-bit CCITT. Viewed whole they look like mush -- that is downsampling,
# not the film. Rotated 90 degrees and cut into bands of about fifteen lines,
# they are crisp.
#
#   ./scanstrip.sh extract <pdf> <first> <last> <outdir>
#       pull the page images out at native resolution (pg-000.png upwards)
#
#   ./scanstrip.sh strips <page.png> <outdir>
#       cut one page into band-0.png .. band-3.png
#
# The crop geometry below is tuned for the X-RAY listing (DN 390779) and should
# transfer to the other 700-series listings, which came off the same printer.

set -euo pipefail

# The crop runs almost the full width of the sheet, for three reasons the
# original 560..5060 window missed. On the left, the assembly trailer -- CARDS,
# SYMBOLS, the error count -- is printed further out than the body columns, and
# it is the transcription's only independent check on how many cards there
# were. On the right sit the page number and the card sequence numbers
# ("BLD nnnn", "NP nnnnn"); the sequence numbers are not worth reading, but the
# page number beside them lets a transcriber confirm the page is the one they
# were asked for, and a page read twice or skipped is the worst failure here.
# In between, the widest comment lines ran past the old right edge.
readonly X=100
readonly W=5760
# A band is about fifteen source lines. The step is shorter than the height so
# that consecutive bands overlap by a line and a half -- without that, a line
# landing on a boundary is cut in half twice and read in neither.
readonly H=1250
readonly STEP=1000
# Sheets are not registered identically: some pages sit as much as 200 pixels
# lower than others, so the first band starts above where any of them begin and
# there is one band more than a page strictly needs. Missing content is far
# more expensive than reading a blank band.
readonly Y0=200
readonly BANDS=5
# Final width. Keep this in step with W: 5760 down to 2110 is the same 2.7:1
# downsample that was found to be legible, and widening one without the other
# is what makes a strip unreadable.
readonly OUT_W=2110
# Percent ink below which a band is blank page rather than content. Set low:
# a band holding two lines of a sparse page reads under 1%, and dropping one
# would be a silent hole in the transcript.
readonly INK_MIN=0.6

usage() {
    echo "usage: $0 extract <pdf> <first> <last> <outdir>" >&2
    echo "       $0 strips <page.png> <outdir>" >&2
    exit 2
}

case "${1:-}" in
extract)
    [[ $# -eq 5 ]] || usage
    mkdir -p "$5"
    pdfimages -f "$3" -l "$4" -png "$2" "$5/pg"
    echo "extracted $(ls "$5"/pg-*.png | wc -l) pages to $5"
    ;;
strips)
    [[ $# -eq 3 ]] || usage
    mkdir -p "$3"
    rm -f "$3"/band-*.png
    for band in $(seq 0 $((BANDS - 1))); do
        out="$3/band-$band.png"
        convert "$2" -rotate 90 \
            -crop "${W}x${H}+${X}+$((Y0 + band * STEP))" +repage \
            -resize "${OUT_W}x" "$out"
        ink=$(convert "$out" -format "%[fx:100*(1-mean)]" info:)
        # awk rather than bc: a blank band's mean rounds to 1 and the fx
        # formatter prints the difference in scientific notation, which bc
        # cannot parse. It errored and kept the band -- the safe direction, but
        # it buried the message in noise on every blank page.
        if awk -v i="$ink" -v m="$INK_MIN" 'BEGIN{exit !(i+0 < m+0)}'; then
            rm -f "$out"
        else
            printf '%s  (ink %.1f%%)\n' "$out" "$ink"
        fi
    done
    ;;
*)
    usage
    ;;
esac
