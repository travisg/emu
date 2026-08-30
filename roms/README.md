# roms

The ROM images the emulator boots. **Nothing in this directory is tracked** —
the third-party images are dumps and builds whose licensing does not permit
redistribution, so they are fetched rather than committed.

```bash
tools/fetch-roms.py    # third-party images, from the local archive or a URL
make -C test ray703    # the Raytheon 703 guests, built from source in this tree
```

`tools/rom-manifest.txt` says what each third-party image is and what it
hashes to. The fetch script looks in `$EMU_ROM_ARCHIVE` (default
`/storage/cloud/dropbox/tech_docs`) first and falls back to a download for the
entries that have a public home; anything it cannot find it names, to be put
in place by hand.

The 703 is the exception that needs no fetching: it has no period ROM at all,
and every guest image under `703/` is assembled out of `test/703/` by
`tools/asm703.py`.
