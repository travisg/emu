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

# Left margin to skip, and the width that covers address through comment. The
# "BLD nnnn" sequence numbers past that are not worth reading.
readonly X=560
readonly W=4500
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
# Final width. 4500 down to 1650 is a 2.7:1 downsample and still legible.
readonly OUT_W=1650
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
        if (( $(echo "$ink < $INK_MIN" | bc -l) )); then
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
