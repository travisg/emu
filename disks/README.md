# disks

The disk and floppy images the machines mount. **Nothing in this directory is
tracked**, for the same reason as `roms/` — see that README and
`tools/rom-manifest.txt`.

- `mbasic-games.img` — the Kaypro II floppy, mounted in drive A by
  `emu -s kaypro`. `tools/fetch-roms.py` puts it here. `--disk PATH` mounts
  another image in its place, and clicking a drive in the window ejects it
  and picks the next one with a file chooser, which starts in this
  directory; cpmtools' `kpii` format is this geometry, so `cpmcp -f kpii`
  onto a copy of it is how a CP/M program gets on a disk. An image must be
  exactly 204,800 bytes (40 tracks x 10 sectors x 512 bytes). Writes go
  through to any other image, but this one is always mounted copy-on-write,
  whichever drive it goes in: a session on it keeps what it writes in
  memory and the file never changes, because it is a scan of a real disk
  and the fetch script never replaces a file that is already here. To keep
  what CP/M saves, copy it and mount the copy.
- `rc2014-cf.img` — the RC2014's compact flash card, mounted by
  `emu -s rc2014-cpm`: CP/M 2.2 with Grant Searle's CBIOS as the RC2014
  project publishes it, sixteen drives. `tools/fetch-roms.py` puts it here
  (128 MB, out of a small zip), `--disk PATH` mounts another card, and
  writes go through to it — what CP/M saves stays saved, so keep a copy if
  that matters. `test/run_rc2014_cpm_test.py` works on a scratch copy.
- `altair8800-cpm.dsk` — the Altair 8800's floppy, mounted in drive 0 by
  `emu -s altair8800-disk`: Burcon CP/M 2.2 for 56K, from Mike Douglas's
  archive at deramp.com, where many more Altair 8-inch disks are (Disk
  BASIC, Altair DOS, CP/M 1.4, 2.2B and 3, Zork, the games disk), all of
  which boot. `tools/fetch-roms.py` puts it here. Each `--disk PATH` fills
  the next of the four drives, the first in place of this one. An image is
  77 tracks of 32 sectors of 137 bytes, 337,568 bytes, or that padded by
  less than a sector (337,664 is common). Writes go through to the image —
  keep a copy if that matters; `test/run_altair8800_cpm_test.py` works on
  scratch copies. `altair8800-zork.dsk` beside it is Zork I-III on a
  bootable CP/M disk, also from deramp and the fetch script.
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
  A REX platter holds its filesystem -- a directory in sectors 1-8 and
  contiguous files after it, which `rex/fs703.py` describes and lists.
  REX's build makes one as `rex/build/rex.img` and writes nothing here.
  Under `load` the platter given to `-r` is unit 0 and `ray703-disc0.img`
  is not mounted, but `ray703-disc1.img`..`disc3.img` are: REX keeps its
  files on `ray703-disc1.img` when that has a directory, which
  `rex/fs703.py --dir disks/ray703-disc1.img` gives an empty platter,
  so what is saved there outlives every rebuild of `rex.img`.

  An image must be exactly 770,048 bytes (385,024 words), and writes go
  through to the file. `tools/mkdisc703.py` is what makes them.
