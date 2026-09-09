# disks

The disk and floppy images the machines mount. **Nothing in this directory is
tracked**, for the same reason as `roms/` — see that README and
`tools/rom-manifest.txt`.

- `mbasic-games.img` — the Kaypro II floppy, mounted by `emu -s kaypro`.
  `tools/fetch-roms.py` puts it here. `--disk PATH` mounts another image
  in its place; cpmtools' `kpii` format is this geometry, so
  `cpmcp -f kpii` onto a copy of it is how a CP/M program gets on a disk.
- `rc2014-cf.img` — the RC2014's compact flash card, mounted by
  `emu -s rc2014-cpm`: CP/M 2.2 with Grant Searle's CBIOS as the RC2014
  project publishes it, sixteen drives. `tools/fetch-roms.py` puts it here
  (128 MB, out of a small zip), `--disk PATH` mounts another card, and
  writes go through to it — what CP/M saves stays saved, so keep a copy if
  that matters. `test/run_rc2014_cpm_test.py` works on a scratch copy.
- `cpm/` — CP/M programs to put on a Kaypro floppy: the Z80 instruction
  exercisers `zexdoc.com` and `zexall.com` and the `prelim.com` in front of
  them, which `test/run_kaypro_zex_test.py` runs. `tools/fetch-roms.py`
  puts them here.
- `ray703-boot.img` — a 703 disc that boots. `make -C test ray703-boot-disc`
  builds it, putting the boot sector in sector 0 of track 0 where the disc
  controller's LOAD button reads it:

      ./target/debug/emu -s ray703-load -r disks/ray703-boot.img

- `ray703-disc0.img`..`ray703-disc3.img` — the 703's four 74601 disc units. A
  file that isn't here is a drive that was never installed, and stays silent.
  `make -C test ray703-blank-disc` formats unit 0, which is what the disc
  exerciser writes to; it will not overwrite a disc that already exists,
  since one a guest has written to is data rather than a build product.
  Unit 0 is also where REX's loader looks for its modules: `make -C rex disc`
  puts them on it under a catalogue in sector 1 (`tools/mkdisc703.py --add`),
  making the image if there is none and rewriting only the sectors the
  catalogue owns.

  An image must be exactly 770,048 bytes (385,024 words), and writes go
  through to the file. `tools/mkdisc703.py` is what makes them.
