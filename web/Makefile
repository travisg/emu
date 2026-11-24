# vim: ts=4:sw=4:expandtab:
#
# The 703 as a WebAssembly module (src/lib.rs), and its check under node.
# Needs the wasm32-unknown-unknown target (rustup target add) and node; the
# check also needs the guests: make -C ../test ray703-basic and make -C ../rex.

WASM := target/wasm32-unknown-unknown/release/emu703_web.wasm

.PHONY: all check clean $(WASM)

all: $(WASM)

$(WASM):
	cargo build --release --target wasm32-unknown-unknown

check: $(WASM)
	node check.mjs $(WASM) ../roms/703/basic.bin ../rex/build/rex.img

clean:
	cargo clean
