# disks

The disk and floppy images the machines mount. **Nothing in this directory is
tracked**, for the same reason as `roms/` — see that README and
`tools/rom-manifest.txt`.

- `mbasic-games.img` — the Kaypro II floppy, mounted by `emu -s kaypro`.
  `tools/fetch-roms.py` puts it here.
- `ray703-disc0.img`..`ray703-disc3.img` — the 703's four 74601 disc units. A
  file that isn't here is a drive that was never installed, and stays silent.
  Make a blank one with:

      python3 -c "open('disks/ray703-disc0.img','wb').truncate(770048)"

  An image must be exactly 770,048 bytes (385,024 words), and writes go
  through to the file.
