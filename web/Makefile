# vim: ts=4:sw=4:expandtab:
#
# The machines with front panels as a WebAssembly module (src/lib.rs), its
# check under node, and the pages that are their frontends (page/): the
# 703's and the Altair 8800's. Needs the wasm32-unknown-unknown target
# (rustup target add) and node; the check and the 703's page also need the
# guests: make -C ../test ray703-basic and make -C ../rex. The 8800's page
# fetches its BASIC tape (tools/fetch-roms.py), and says so on its status
# line when the site was built without it; the BASIC part of the check is
# skipped without it.

WASM := target/wasm32-unknown-unknown/release/emu_web.wasm
SITE := build/site
GUESTS := ../roms/703/basic.bin ../rex/build/rex.img
TAPE := ../roms/altair8800/basic8k40.tap
COMMON := page/common/machine.js page/common/page.css

.PHONY: all check site serve clean $(WASM)

all: $(WASM)

$(WASM):
	cargo build --release --target wasm32-unknown-unknown

check: $(WASM)
	node check.mjs $(WASM) $(GUESTS) $(wildcard $(TAPE))

# Each page and everything it fetches, in a directory of its own to serve
# as it is, and an index of the two.
site: $(WASM)
	rm -rf $(SITE)
	mkdir -p $(SITE)/ray703 $(SITE)/altair8800
	cp page/index.html $(SITE)/
	cp page/ray703/index.html page/ray703/page.js $(COMMON) $(GUESTS) $(SITE)/ray703/
	cp $(WASM) $(SITE)/ray703/emu.wasm
	cp page/altair8800/index.html page/altair8800/page.js $(COMMON) $(wildcard $(TAPE)) $(SITE)/altair8800/
	cp $(WASM) $(SITE)/altair8800/emu.wasm

# A page has to come over http, not file:, to fetch its files.
serve: site
	python3 -m http.server --directory $(SITE) --bind 127.0.0.1 8703

clean:
	cargo clean
	rm -rf build
