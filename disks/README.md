# disks

The disk and floppy images the machines mount. **Nothing in this directory is
tracked**, for the same reason as `roms/` — see that README and
`tools/rom-manifest.txt`.

- `mbasic-games.img` — the Kaypro II floppy, mounted by `emu -s kaypro`.
  `tools/fetch-roms.py` puts it here.
- `ray703-boot.img` — a 703 disc that boots. `make -C test ray703-boot-disc`
  builds it, putting the boot sector in sector 0 of track 0 where the disc
  controller's LOAD button reads it:

      ./target/debug/emu -s ray703-load -r disks/ray703-boot.img

- `ray703-disc0.img`..`ray703-disc3.img` — the 703's four 74601 disc units. A
  file that isn't here is a drive that was never installed, and stays silent.
  `make -C test ray703-blank-disc` formats unit 0, which is what the disc
  exerciser writes to; it will not overwrite a disc that already exists,
  since one a guest has written to is data rather than a build product.

  An image must be exactly 770,048 bytes (385,024 words), and writes go
  through to the file. `tools/mkdisc703.py` is what makes them.
