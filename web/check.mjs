// vim: ts=4:sw=4:expandtab:
// Copyright (c) 2026 Travis Geiselbrecht; MIT, see LICENSE.
//
// The wasm 703 under node, no browser, driven as the page drives it: the
// panel's switches pressed by clicking their pixels, the panel and paper
// rendered every frame, and the verdicts read off the pixels as well as
// the teletype's output. Tiny BASIC from a core image, then REX off its
// platter. Every check fails the run; WEB_CONTROL=no-run leaves RUN
// unpressed, and must FAIL.
//
//   node web/check.mjs WASM BASIC_BIN REX_IMG
import fs from 'node:fs';

const [, , wasmPath, basicPath, rexPath] = process.argv;
const control = process.env.WEB_CONTROL ?? '';
const bytes = fs.readFileSync(wasmPath);

// Switch centers in the panel's pixels: BUTTONS and CONTROLS_Y in
// src/console/panel703.rs.
const RUN = [256, 252], HALT = [172, 252], SINGLE = [472, 252];

function check(ok, what) {
    if (!ok) { console.log(`FAIL: ${what}`); process.exit(1); }
    console.log(`ok: ${what}`);
}

// One machine per module instance, as a page has.
async function machine(imagePath, kind) {
    const { instance } = await WebAssembly.instantiate(bytes, {});
    const e = instance.exports;
    const image = fs.readFileSync(imagePath);
    const p = e.staging(image.length);
    new Uint8Array(e.memory.buffer, p, image.length).set(image);
    check(e[kind](0, 0) === 1, `${kind} took ${imagePath}`);
    const slice = Math.round(e.clock_hz() / 60);
    const pw = e.panel_width(), ph = e.panel_height();
    const m = { e, out: '', panel: null, paper: null };
    m.frame = () => {
        if (e.run(slice) !== 0) check(false, 'the run went on');
        const ptr = e.panel_render();
        m.panel = new Uint8Array(e.memory.buffer, ptr, pw * ph * 4).slice();
        const pp = e.paper_render();
        if (pp) m.paper = new Uint8Array(e.memory.buffer, pp, e.paper_width() * e.paper_height() * 4).slice();
        const n = e.drain();
        if (n) m.out += Buffer.from(new Uint8Array(e.memory.buffer, e.drained_ptr(), n)).toString('latin1');
    };
    m.frames = n => { for (let i = 0; i < n; i++) m.frame(); };
    m.click = ([x, y]) => e.panel_click(x, y, 0);
    m.pixel = ([x, y]) => Array.from(m.panel.subarray((y * pw + x) * 4, (y * pw + x) * 4 + 3));
    // the guest's characters carry bit 7 and its lines end CR LF
    m.text = () => m.out.replace(/[\x80-\xff]/g, c => String.fromCharCode(c.charCodeAt(0) & 0x7f)).replace(/\r/g, '');
    // Pace on the guest falling silent for a second: a 703 drops keys
    // typed while it is busy (AGENTS.md, Tiny BASIC).
    m.quiet = (limit = 60) => {
        let last = m.out.length, still = 0;
        for (let i = 0; i < limit * 60 && still < 60; i++) {
            m.frame();
            still = m.out.length === last ? still + 1 : 0;
            last = m.out.length;
        }
    };
    m.type = line => {
        for (const c of line) { e.key(c.charCodeAt(0)); m.frames(9); }
        m.quiet();
    };
    // ink: paper pixels well darker than the newsprint
    m.ink = () => { let n = 0; for (let i = 0; i < m.paper.length; i += 4) if (m.paper[i] < 0xc0) n++; return n; };
    return m;
}

// The HALT lens: lit is a bright red, unlit a deep maroon.
const lensLit = ([r, g, b]) => r > 0xc0 && g < 0x60 && b < 0x60;

const basic = await machine(basicPath, 'boot_core');
basic.frames(2);
check(lensLit(basic.pixel(HALT)), 'power-on: the HALT lens is lit');
check(basic.ink() === 0, 'power-on: blank paper');
if (control !== 'no-run') basic.click(RUN);
basic.quiet();
check(basic.text().includes('TINY BASIC'), 'BASIC prints its banner');
check(!lensLit(basic.pixel(HALT)), 'running: the HALT lens is out');
check(basic.ink() > 500, `the banner is on the paper (${basic.ink()} pixels of ink)`);
for (const l of ['10 FOR I=1 TO 5\r', '20 PRINT I*I\r', '30 NEXT I\r', 'RUN\r']) basic.type(l);
check(/RUN\n1\n4\n9\n16\n25\n/.test(basic.text()), 'the program prints the squares');

const rex = await machine(rexPath, 'boot_disc');
const e = rex.e;
const hex = v => v.toString(16).padStart(4, '0');
rex.frames(60);
check(e.halted() === 1 && e.pc_lamps() === 0 && rex.out === '', 'power-on: halted at 0, nothing printed');
rex.click(RUN);
rex.quiet();
check(e.halted() === 0 && rex.text().includes('REX 703 UP'), 'RUN boots REX');
check(e.rack_reads(0) > 0, `the boot read the disc (${e.rack_reads(0)} reads)`);
rex.click(HALT); rex.frames(2);
const at = e.pc_lamps();
rex.frames(30);
check(e.halted() === 1 && e.pc_lamps() === at && lensLit(rex.pixel(HALT)), `HALT holds the PC at ${hex(at)}, lens lit`);
rex.click(SINGLE); rex.frames(2);
check(e.halted() === 1, 'SINGLE COMMAND steps and stays halted');
rex.click(RUN); rex.frames(30);
rex.type('RUN HELLO\r');
check(/RUN HELLO\nHELLO FROM WORD \d+\n/.test(rex.text()), 'a program off the disc runs');
console.log('PASS');
