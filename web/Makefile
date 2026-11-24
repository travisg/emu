# vim: ts=4:sw=4:expandtab:
#
# The 703 as a WebAssembly module (src/lib.rs), its check under node, and
# the page that is its frontend (page/). Needs the wasm32-unknown-unknown
# target (rustup target add) and node; the check and the page also need
# the guests: make -C ../test ray703-basic and make -C ../rex.

WASM := target/wasm32-unknown-unknown/release/emu703_web.wasm
SITE := build/site
GUESTS := ../roms/703/basic.bin ../rex/build/rex.img

.PHONY: all check site serve clean $(WASM)

all: $(WASM)

$(WASM):
	cargo build --release --target wasm32-unknown-unknown

check: $(WASM)
	node check.mjs $(WASM) $(GUESTS)

# The page and everything it fetches, in one directory to serve as it is.
site: $(WASM)
	rm -rf $(SITE)
	mkdir -p $(SITE)
	cp page/index.html page/page.js page/page.css $(SITE)/
	cp $(WASM) $(SITE)/emu703.wasm
	cp $(GUESTS) $(SITE)/

# A page has to come over http, not file:, to fetch its files.
serve: site
	python3 -m http.server --directory $(SITE) --bind 127.0.0.1 8703

clean:
	cargo clean
	rm -rf build
