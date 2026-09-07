# fonts/

Typefaces vendored for the emulator's windows. Nothing here is linked or
parsed at build time -- `tools/mkfont703.py` rasterizes a face once into a
bitmap atlas that is checked in as Rust source, so the runtime needs no font
library.

## TTY33MA-Book.ttf

The Teletype Model 33's type wheel, as struck on paper: developed from
photographs of real output by the teletypewriter-fonts project.

- Source: https://github.com/rand-projects/teletypewriter-fonts
  (`tty/typewheel/tty33/TTY33MA-Book.ttf`, commit `02cfd79`)
- License: SIL Open Font License 1.1, in `OFL.txt` alongside
- Used by: `src/console/tty33_font.rs` (generated), the 703 teletype window

The Book weight is the as-printed rendering; the repo also carries a Light
weight and an `lc` variant with invented lowercase, neither of which a
Model 33 ever printed.
