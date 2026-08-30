#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Populate roms/ and disks/ from tools/rom-manifest.txt.

None of the images the emulator boots are in the repo: they are third-party
ROM dumps and a CP/M floppy, and their licensing does not permit
redistribution. The manifest records what each one is and what it hashes to,
and this fetches them into place.

Two sources, in order. A local archive -- $EMU_ROM_ARCHIVE, defaulting to the
Dropbox tech_docs tree -- holds them all and is checked first; entries with a
public URL fall back to downloading. Either way the sha256 has to match before
the file is kept, so a half-finished download or a wrong dump is caught here
rather than as a machine that boots to garbage.

A URL ending in "#member" names a file inside a zip to extract, which is how
the 6809 BASIC image is published.

An image already in place is verified, never overwritten: a mismatch is
reported and the file left alone, because the copy on disk may be the good one
and the manifest stale.
"""

import hashlib
import io
import os
import shutil
import sys
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

MANIFEST = Path(__file__).resolve().parent / "rom-manifest.txt"
ROOT = MANIFEST.parent.parent
DEFAULT_ARCHIVE = "/storage/cloud/dropbox/tech_docs"


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def read_manifest():
    entries = []
    for lineno, line in enumerate(MANIFEST.read_text().splitlines(), 1):
        # Only a whole-line comment: a "#" inside a field is the zip member a
        # URL ends with, and stripping it would silently fetch the zip itself.
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        fields = line.split()
        if len(fields) != 3:
            sys.exit(f"{MANIFEST}:{lineno}: expected 'path sha256 url'")
        path, digest, url = fields
        entries.append((path, digest, None if url == "-" else url))
    return entries


def fetch_from_archive(archive, relpath, dest, digest):
    src = Path(archive) / relpath
    # The archive keeps some entries as symlinks into the document tree, so
    # copy the contents rather than the link.
    if not src.is_file():
        return None
    shutil.copyfile(src, dest)
    if sha256(dest) == digest:
        return f"copied from {src}"
    dest.unlink()
    return None


def fetch_from_url(url, dest, digest):
    url, _, member = url.partition("#")
    try:
        with urllib.request.urlopen(url) as response:
            body = response.read()
    except (urllib.error.URLError, OSError) as e:
        return None, f"download failed: {e}"

    if member:
        try:
            body = zipfile.ZipFile(io.BytesIO(body)).read(member)
        except (zipfile.BadZipFile, KeyError) as e:
            return None, f"{url} does not hold {member}: {e}"

    dest.write_bytes(body)
    if sha256(dest) == digest:
        where = f"{member} in {url}" if member else url
        return f"downloaded from {where}", None
    dest.unlink()
    return None, f"downloaded from {url} but the hash did not match"


def main():
    archive = os.environ.get("EMU_ROM_ARCHIVE", DEFAULT_ARCHIVE)
    missing = []

    for relpath, digest, url in read_manifest():
        dest = ROOT / relpath
        if dest.exists():
            if sha256(dest) == digest:
                print(f"ok       {relpath}")
            else:
                print(f"MISMATCH {relpath}: on disk but the hash differs; leaving it alone")
                missing.append(relpath)
            continue

        dest.parent.mkdir(parents=True, exist_ok=True)

        how = fetch_from_archive(archive, relpath, dest, digest)
        why = None
        if how is None and url is not None:
            how, why = fetch_from_url(url, dest, digest)

        if how is not None:
            print(f"fetched  {relpath} ({how})")
        else:
            print(f"MISSING  {relpath}" + (f": {why}" if why else ""))
            missing.append(relpath)

    if missing:
        print()
        print(f"{len(missing)} image(s) not in place:")
        for relpath in missing:
            print(f"  {relpath}")
        print()
        print(f"The archive searched was {archive}; set EMU_ROM_ARCHIVE to point")
        print("somewhere else, or put the files in place by hand.")
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
