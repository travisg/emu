// vim: ts=4:sw=4:expandtab:
// Copyright (c) 2026 Travis Geiselbrecht; MIT, see LICENSE.
//
// The wasm 703 under node, no browser, driven as the page drives it: the
// panel's switches pressed by clicking their pixels, the panel and paper
// rendered every frame, and the verdicts read off the pixels as well as
// the teletype's output. Tiny BASIC from a core image, then REX off its
// platter, then the module's own limits: an empty core refused, the
// platen's stops, and the kept output bounded for a page that never
// drains it. Every check fails the run; WEB_CONTROL=no-run leaves RUN
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
async function instance() {
    return (await WebAssembly.instantiate(bytes, {})).instance.exports;
}

async function machine(imagePath, kind, fastIO = 0) {
    const e = await instance();
    const image = fs.readFileSync(imagePath);
    const p = e.staging(image.length) >>> 0;
    new Uint8Array(e.memory.buffer, p, image.length).set(image);
    check(e[kind](0, fastIO) === 1, `${kind} took ${imagePath}`);
    const slice = Math.round(e.clock_hz() / 60);
    const pw = e.panel_width(), ph = e.panel_height();
    const m = { e, out: '', panel: null, paper: null };
    m.frame = () => {
        if (e.run(slice) !== 0) check(false, 'the run went on');
        const ptr = e.panel_render() >>> 0;
        m.panel = new Uint8Array(e.memory.buffer, ptr, pw * ph * 4).slice();
        const pp = e.paper_render() >>> 0;
        if (pp) m.paper = new Uint8Array(e.memory.buffer, pp, e.paper_width() * e.paper_height() * 4).slice();
        const n = e.drain();
        if (n) m.out += Buffer.from(new Uint8Array(e.memory.buffer, e.drained_ptr() >>> 0, n)).toString('latin1');
    };
    m.frames = n => { for (let i = 0; i < n; i++) m.frame(); };
    m.click = ([x, y]) => e.panel_click(x, y, 0);
    m.pixel = ([x, y]) => Array.from(m.panel.subarray((y * pw + x) * 4, (y * pw + x) * 4 + 3));
    // the guest's lines end CR LF
    m.text = () => m.out.replace(/\r/g, '');
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
check(basic.e.paper_can_roll(1) === 0 && basic.e.paper_wheel(1) === 0, 'a short page has nowhere to roll');
for (const l of ['10 FOR I=1 TO 30\r', '20 PRINT I*I\r', '30 NEXT I\r', 'RUN\r']) basic.type(l);
check(/RUN\n1\n4\n9\n16\n25\n/.test(basic.text()) && basic.text().includes('\n900\n'), 'the program prints the squares');
{
    const b = basic.e;
    check(b.paper_can_roll(-1) === 0 && b.paper_wheel(-1) === 0, 'at the live end the platen rolls no further forward');
    check(b.paper_can_roll(1) === 1 && b.paper_wheel(1) === 1, 'a full page rolls back');
    let rolls = 0;
    while (b.paper_wheel(1)) rolls++;
    check(rolls > 0 && rolls < 100 && b.paper_can_roll(1) === 0, `and stops at its first line (${rolls} more notches)`);
    check(b.paper_can_roll(-1) === 1, 'from where it rolls forward again');
}

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

// The knob, both ways from MB: ENTER and DISPLAY light only on MB.
const ENTER = [800, 252];
const entryLit = () => rex.pixel(ENTER)[0] > 0xe0;
check(entryLit(), 'the knob starts on MB, ENTER lit');
e.panel_knob(1); rex.frames(1);
check(!entryLit(), 'the knob turns off MB');
e.panel_knob(-1); rex.frames(1);
check(entryLit(), 'and back onto it');

// An empty core is no machine.
check((await instance()).boot_core(0, 0) === 0, 'boot_core refuses an empty core');

// A page never drains: under Fast I/O, a loop printing for a few seconds
// with nothing drained leaves the kept output at its cap.
const loop = await machine(basicPath, 'boot_core', 1);
loop.click(RUN); loop.quiet();
for (const l of ['10 PRINT "THE QUICK BROWN FOX"\r', '20 GOTO 10\r']) loop.type(l);
for (const c of 'RUN\r') loop.e.key(c.charCodeAt(0));
const slice = Math.round(loop.e.clock_hz() / 60);
// twenty seconds for the kept output to reach its cap and the allocator to
// settle, then thirty more that must not grow the memory
for (let i = 0; i < 1200; i++) loop.e.run(slice);
const memory = loop.e.memory.buffer.byteLength;
for (let i = 0; i < 1800; i++) loop.e.run(slice);
const kept = loop.e.drain();
check(kept === 64 * 1024, `the kept output stops at its cap (${kept} bytes)`);
check(loop.e.memory.buffer.byteLength === memory, 'and the memory stops growing');
console.log('PASS');
