// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! What there is to run: each machine's launch modes from the registry,
//! resolved against the files actually present under a root directory.
//!
//! Pure apart from reading directories, so any front end -- the terminal
//! picker today -- can be built over it.

use crate::system::registry::{Files, Media, SystemDescriptor, SYSTEMS};
use std::path::{Path, PathBuf};

/// One thing the launcher can start: a mode and the file it was given.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub variant: &'static str,
    /// The mode's description, from the registry.
    pub what: &'static str,
    /// For `-r`, relative to the root; `None` runs the default rom.
    pub rom: Option<PathBuf>,
    /// For `--disk`, relative to the root.
    pub disk: Option<PathBuf>,
    /// The file this machine runs when nothing is given.
    pub is_default: bool,
}

/// A machine and what was found for it.
pub struct Entry {
    pub desc: &'static SystemDescriptor,
    pub items: Vec<Item>,
    /// Modes that found nothing, and fixed files that are not there, each as
    /// a line saying what is wanted and how to come by it.
    pub missing: Vec<String>,
}

/// Every machine in the registry, against what is under `root`.
pub fn scan(root: &Path) -> Vec<Entry> {
    SYSTEMS.iter().map(|desc| scan_one(root, desc)).collect()
}

fn scan_one(root: &Path, desc: &'static SystemDescriptor) -> Entry {
    let mut items = Vec::new();
    let mut missing = Vec::new();
    let absent: Vec<&str> =
        desc.launch.needs.iter().copied().filter(|f| !root.join(f).is_file()).collect();
    for mode in desc.launch.modes {
        // a mode that picks no rom runs the default one, so needs it
        let mut wants: Vec<&str> = absent.clone();
        if !matches!(mode.media, Media::Rom(_)) && !root.join(desc.default_rom).is_file() {
            wants.push(desc.default_rom);
        }
        if !wants.is_empty() {
            missing.push(format!("{}: needs {} ({})", mode.what, wants.join(", "), mode.fetch));
            continue;
        }
        let item = |rom, disk, is_default| Item { variant: mode.variant, what: mode.what, rom, disk, is_default };
        match &mode.media {
            Media::Fixed => items.push(item(None, None, mode.variant.is_empty())),
            Media::Rom(files) | Media::Disk(files) => {
                let found = find(root, files);
                if found.is_empty() {
                    missing.push(format!("{}: none in {} ({})", mode.what, describe(files), mode.fetch));
                }
                for path in found {
                    items.push(match mode.media {
                        Media::Rom(_) => {
                            let is_default = mode.variant.is_empty() && path == Path::new(desc.default_rom);
                            item(Some(path), None, is_default)
                        }
                        _ => item(None, Some(path), false),
                    });
                }
            }
        }
    }
    Entry { desc, items, missing }
}

/// The files `files` matches under `root`, as paths relative to it, sorted
/// within each directory and in the directories' order.
pub fn find(root: &Path, files: &Files) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in files.dirs {
        let Ok(read) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        let mut here: Vec<PathBuf> = read
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_lowercase();
                let ext_ok = Path::new(&name)
                    .extension()
                    .is_some_and(|x| files.exts.iter().any(|want| x == *want));
                // metadata() follows a symlink, so a linked image counts
                let size_ok = e.path().metadata().is_ok_and(|m| m.is_file() && files.size.allows(m.len()));
                ext_ok && name.contains(&files.name_has.to_lowercase()) && size_ok
            })
            .map(|e| Path::new(dir).join(e.file_name()))
            .collect();
        here.sort();
        out.extend(here);
    }
    out
}

/// `disks/*.img, rex/build/*.img`, for a line saying none was found.
fn describe(files: &Files) -> String {
    let mut globs = Vec::new();
    for dir in files.dirs {
        for ext in files.exts {
            let stem = if files.name_has.is_empty() { "*".to_string() } else { format!("*{}*", files.name_has) };
            globs.push(format!("{dir}/{stem}.{ext}"));
        }
    }
    globs.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::registry::FileSize;

    struct TempRoot(PathBuf);
    impl TempRoot {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("emu-launcher-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            TempRoot(dir)
        }
        fn put(&self, path: &str, len: usize) {
            let p = self.0.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, vec![0u8; len]).unwrap();
        }
    }
    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn entry<'a>(entries: &'a [Entry], name: &str) -> &'a Entry {
        entries.iter().find(|e| e.desc.name == name).unwrap()
    }

    #[test]
    fn find_filters_by_extension_name_and_size_in_directory_order() {
        let root = TempRoot::new("find");
        root.put("b/z.IMG", 10);
        root.put("a/y.img", 10);
        root.put("a/x.img", 10);
        root.put("a/short.img", 9);
        root.put("a/x.bin", 10);
        root.put("a/sub.img/inner", 10); // a directory with the extension
        let files = Files { dirs: &["b", "a", "missing"], exts: &["img"], name_has: "", size: FileSize::Exactly(10) };
        let got = find(&root.0, &files);
        assert_eq!(got, ["b/z.IMG", "a/x.img", "a/y.img"].map(PathBuf::from));
        let files = Files { name_has: "Y", ..files };
        assert_eq!(find(&root.0, &files), [PathBuf::from("a/y.img")]);
    }

    #[test]
    fn an_empty_root_offers_nothing_and_says_what_is_wanted() {
        let root = TempRoot::new("empty");
        let entries = scan(&root.0);
        assert_eq!(entries.len(), SYSTEMS.len());
        for e in &entries {
            assert!(e.items.is_empty(), "{}: {:?}", e.desc.name, e.items);
            assert_eq!(e.missing.len(), e.desc.launch.modes.len(), "{}", e.desc.name);
        }
        let kaypro = entry(&entries, "kaypro");
        assert!(kaypro.missing[0].contains("roms/kaypro/kayproii_u43.bin"), "{:?}", kaypro.missing);
        assert!(kaypro.missing[0].contains("roms/kaypro/kayproii_u47.bin"), "{:?}", kaypro.missing);
    }

    #[test]
    fn the_703_sorts_its_files_into_modes_by_kind_and_size() {
        let root = TempRoot::new("703");
        let platter = crate::dev::disc74601::IMAGE_BYTES;
        root.put("roms/703/demo.bin", 100);
        root.put("roms/703/basic.bin", 100);
        root.put("rex/build/rex.bin", 100); // REX runs from its platter
        root.put("roms/703/tape.tape", 50);
        root.put("rex/build/rex.img", platter);
        root.put("disks/ray703-boot.img", platter);
        root.put("disks/mbasic-games.img", crate::dev::wd1793::IMAGE_SIZE as usize);
        let e = scan(&root.0);
        let ray = entry(&e, "ray703");
        let got: Vec<(&str, Option<&str>, bool)> = ray
            .items
            .iter()
            .map(|i| (i.variant, i.rom.as_deref().and_then(Path::to_str), i.is_default))
            .collect();
        assert_eq!(
            got,
            [
                ("", Some("roms/703/basic.bin"), false),
                ("", Some("roms/703/demo.bin"), true),
                ("ptb", Some("roms/703/tape.tape"), false),
                ("load", Some("disks/ray703-boot.img"), false),
                ("load", Some("rex/build/rex.img"), false),
            ]
        );
        assert!(ray.missing.is_empty(), "{:?}", ray.missing);
    }

    #[test]
    fn a_disk_mode_needs_the_default_rom_and_passes_its_file_as_the_disk() {
        let root = TempRoot::new("kaypro");
        let floppy = crate::dev::wd1793::IMAGE_SIZE as usize;
        root.put("disks/mbasic-games.img", floppy);
        root.put("disks/other.img", floppy);
        root.put("roms/kaypro/kayproii_u43.bin", 2048);
        assert!(entry(&scan(&root.0), "kaypro").items.is_empty(), "the boot rom is missing");
        root.put("roms/kaypro/kayproii_u47.bin", 4096);
        let e = scan(&root.0);
        let kaypro = entry(&e, "kaypro");
        let disks: Vec<_> = kaypro.items.iter().map(|i| (i.rom.clone(), i.disk.clone().unwrap())).collect();
        assert_eq!(
            disks,
            [(None, PathBuf::from("disks/mbasic-games.img")), (None, PathBuf::from("disks/other.img"))]
        );
    }

    #[test]
    fn a_fixed_mode_is_one_item_once_its_rom_is_there() {
        let root = TempRoot::new("rc2014");
        root.put("roms/rc2014/24886009.BIN", 65536);
        root.put("disks/mbasic-games.img", 204800); // too small to be a card
        let e = scan(&root.0);
        let rc = entry(&e, "rc2014");
        assert_eq!(rc.items.len(), 1);
        assert_eq!(rc.items[0].rom, None);
        assert!(rc.items[0].is_default);
        assert_eq!(rc.missing.len(), 1, "no card: {:?}", rc.missing);
        assert!(rc.missing[0].contains("disks/*.img"), "{:?}", rc.missing);
    }
}
