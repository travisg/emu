// vim: ts=4:sw=4:expandtab:
// Copyright (c) 2026 Travis Geiselbrecht; MIT, see LICENSE.
//
// The wasm module under node, no browser, driven as the pages drive it:
// the panels' switches pressed by clicking their pixels, the panel and
// paper rendered every frame, and the verdicts read off the pixels as well
// as the teletypes' output.
//
// The 703: Tiny BASIC from a core image, then REX off its platter, a key
// at the multiplexer's first teletype bringing it a shell on its own
// paper, then the module's own limits -- an empty core refused, the
// platen's stops, and the kept output bounded for a page that never
// drains it. The Altair 8800: a program in a memory image this file
// assembles, run from the panel, echoing what is typed, and EXAMINE
// showing a byte on the data lamps; then, given its tape, 8K BASIC loaded
// through the bootstrap, a line run on it and a sense switch read. Every
// check fails the run; WEB_CONTROL=no-run leaves the 703's RUN unpressed
// and =no-run-8800 the 8800's, and each must FAIL.
//
//   node web/check.mjs WASM BASIC_BIN REX_IMG [ALTAIR8800_TAPE]
import fs from 'node:fs';

const [, , wasmPath, basicPath, rexPath, tapePath] = process.argv;
const control = process.env.WEB_CONTROL ?? '';
const bytes = fs.readFileSync(wasmPath);

// Switch centers in the 703 panel's pixels: BUTTONS and CONTROLS_Y in
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

// A machine booted from `image` by `boot(exports)`, with a frame at a
// time and each teletype's output collected.
async function machine(image, boot, what) {
    const e = await instance();
    const p = e.staging(image.length) >>> 0;
    new Uint8Array(e.memory.buffer, p, image.length).set(image);
    check(boot(e) === 1, `${what} booted`);
    const slice = Math.round(e.clock_hz() / 60);
    const pw = e.panel_width(), ph = e.panel_height();
    const paperBytes = e.paper_width() * e.paper_height() * 4;
    const m = { e, outs: Array.from({ length: e.terminals() }, () => ''), panel: null, paper: null };
    m.frame = () => {
        if (e.run(slice) !== 0) check(false, 'the run went on');
        const ptr = e.panel_render() >>> 0;
        m.panel = new Uint8Array(e.memory.buffer, ptr, pw * ph * 4).slice();
        const pp = e.paper_render(0, 0) >>> 0;
        if (pp) m.paper = new Uint8Array(e.memory.buffer, pp, paperBytes).slice();
        for (let n = 0; n < m.outs.length; n++) {
            const k = e.drain(n);
            if (k) m.outs[n] += Buffer.from(new Uint8Array(e.memory.buffer, e.drained_ptr() >>> 0, k)).toString('latin1');
        }
    };
    m.frames = n => { for (let i = 0; i < n; i++) m.frame(); };
    // teletype n's paper as it is drawn now, forced
    m.paperOf = n => {
        const pp = e.paper_render(n, 1) >>> 0;
        return new Uint8Array(e.memory.buffer, pp, paperBytes).slice();
    };
    // the console's paper, drawn as a page shows it
    m.paper = m.paperOf(0);
    m.click = ([x, y]) => e.panel_click(x, y, 0);
    m.pixel = ([x, y]) => Array.from(m.panel.subarray((y * pw + x) * 4, (y * pw + x) * 4 + 3));
    // the guest's lines end CR LF
    m.text = (n = 0) => m.outs[n].replace(/\r/g, '');
    // Pace on teletype n falling silent for a second: a 703 drops keys
    // typed while it is busy (AGENTS.md, Tiny BASIC).
    m.quiet = (n = 0, limit = 60) => {
        let last = m.outs[n].length, still = 0;
        for (let i = 0; i < limit * 60 && still < 60; i++) {
            m.frame();
            still = m.outs[n].length === last ? still + 1 : 0;
            last = m.outs[n].length;
        }
    };
    m.type = (line, n = 0) => {
        for (const c of line) { e.key(n, c.charCodeAt(0)); m.frames(9); }
        m.quiet(n);
    };
    // Frames until teletype n's output matches, or `limit` seconds.
    m.until = (re, n = 0, limit = 30) => {
        for (let i = 0; i < limit * 60 && !re.test(m.text(n)); i++) m.frame();
        return re.test(m.text(n));
    };
    return m;
}

// ink: paper pixels well darker than the newsprint
function ink(paper) {
    let n = 0;
    for (let i = 0; i < paper.length; i += 4) if (paper[i] < 0xc0) n++;
    return n;
}

// The HALT lens: lit is a bright red, unlit a deep maroon. The Altair's
// LEDs are the same red.
const lensLit = ([r, g, b]) => r > 0xc0 && g < 0x60 && b < 0x60;

const read = path => new Uint8Array(fs.readFileSync(path));

// -- the 703 ----------------------------------------------------------------

const basic = await machine(read(basicPath), e => e.boot_core(0, 0), basicPath);
check(basic.e.terminals() === 4, 'the 703 has its console and the multiplexer\'s three teletypes');
basic.frames(2);
check(basic.e.paper_render(0, 0) === 0 && [1, 2, 3].every(n => basic.e.paper_unseen(n) === 0),
    'power-on: no paper has anything new to draw');
check(lensLit(basic.pixel(HALT)), 'power-on: the HALT lens is lit');
check(ink(basic.paper) === 0, 'power-on: blank paper');
if (control !== 'no-run') basic.click(RUN);
basic.quiet();
check(basic.text().includes('TINY BASIC'), 'BASIC prints its banner');
check(!lensLit(basic.pixel(HALT)), 'running: the HALT lens is out');
check(ink(basic.paper) > 500, `the banner is on the paper (${ink(basic.paper)} pixels of ink)`);
check(basic.e.paper_can_roll(0, 1) === 0 && basic.e.paper_wheel(0, 1) === 0, 'a short page has nowhere to roll');
for (const l of ['10 FOR I=1 TO 30\r', '20 PRINT I*I\r', '30 NEXT I\r', 'RUN\r']) basic.type(l);
check(/RUN\n1\n4\n9\n16\n25\n/.test(basic.text()) && basic.text().includes('\n900\n'), 'the program prints the squares');
{
    const b = basic.e;
    check(b.paper_can_roll(0, -1) === 0 && b.paper_wheel(0, -1) === 0, 'at the live end the platen rolls no further forward');
    check(b.paper_can_roll(0, 1) === 1 && b.paper_wheel(0, 1) === 1, 'a full page rolls back');
    let rolls = 0;
    while (b.paper_wheel(0, 1)) rolls++;
    check(rolls > 0 && rolls < 100 && b.paper_can_roll(0, 1) === 0, `and stops at its first line (${rolls} more notches)`);
    check(b.paper_can_roll(0, -1) === 1, 'from where it rolls forward again');
    check(b.paper_wheel(4, 1) === 0 && b.paper_render(4, 1) === 0, 'a teletype past the last is none');
}

const rex = await machine(read(rexPath), e => e.boot_disc(0, 0), rexPath);
const e = rex.e;
const hex = v => v.toString(16).padStart(4, '0');
rex.frames(60);
check(e.halted() === 1 && e.pc_lamps() === 0 && rex.outs[0] === '', 'power-on: halted at 0, nothing printed');
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

// Teletype 1, on the multiplexer: blank until a key brings it a shell,
// which prints on its paper and no other.
check(ink(rex.paperOf(1)) === 0 && ink(rex.paperOf(2)) === 0, 'teletypes 1 and 2 start blank');
check(e.paper_unseen(1) === 0, 'and teletype 1 has printed nothing unseen');
const consoleBefore = rex.outs[0].length;
rex.e.key(1, 0x0d);
check(rex.until(/REX>/, 1, 60), 'Return at teletype 1 brings it a shell');
check(/REX 703 TERMINAL 1\n/.test(rex.text(1)) && !rex.outs[0].slice(consoleBefore).includes('TERMINAL'),
    'its banner names it, on its own teletype and not the console');
check(e.paper_unseen(1) === 1, 'teletype 1 has printed unseen');
check(ink(rex.paperOf(1)) > 200, `its shell is on its paper (${ink(rex.paperOf(1))} pixels of ink)`);
check(e.paper_unseen(1) === 0, 'and once drawn it is seen');
check(ink(rex.paperOf(2)) === 0 && rex.outs[2] === '', 'teletype 2 is still blank');
rex.type('DIR 0:\r', 1);
check(/HELLO/.test(rex.text(1)), 'teletype 1\'s shell lists the disc');

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
{
    const loop = await machine(read(basicPath), e => e.boot_core(0, 1), `${basicPath} under Fast I/O`);
    loop.click(RUN); loop.quiet();
    for (const l of ['10 PRINT "THE QUICK BROWN FOX"\r', '20 GOTO 10\r']) loop.type(l);
    for (const c of 'RUN\r') loop.e.key(0, c.charCodeAt(0));
    const slice = Math.round(loop.e.clock_hz() / 60);
    // twenty seconds for the kept output to reach its cap and the allocator
    // to settle, then thirty more that must not grow the memory
    for (let i = 0; i < 1200; i++) loop.e.run(slice);
    const memory = loop.e.memory.buffer.byteLength;
    for (let i = 0; i < 1800; i++) loop.e.run(slice);
    const kept = loop.e.drain(0);
    check(kept === 64 * 1024, `the kept output stops at its cap (${kept} bytes)`);
    check(loop.e.memory.buffer.byteLength === memory, 'and the memory stops growing');
}

// -- the Altair 8800 --------------------------------------------------------

// Switch and lamp centers in the 8800 panel's pixels, after col_x,
// command_x and the row constants in src/console/panel8800.rs.
const col = i => 200 + i * 52 + (5 - Math.floor((15 - i) / 3)) * 14;
const command = i => col(0) + i * 104 + 26;
const COMMAND_Y = 362, TOGGLE_Y = 262, DATA_Y = 96;
const STOP = [command(0), COMMAND_Y - 12], RUN8800 = [command(0), COMMAND_Y + 12];
const EXAMINE = [command(2), COMMAND_Y - 12];
const WAIT = [40, 176];
const toggle = bit => [col(15 - bit), TOGGLE_Y];
const dataLamp = bit => [col(15 - bit), DATA_Y];

// The program: the 2SIO's port 0 programmed, a banner printed from MSG,
// then every character read echoed. MSG is '8800 READY'.
const MSG = 0x30;
// prettier-ignore
const PROGRAM = [
    0x3e, 0x03, 0xd3, 0x10,         // 0000 mvi a,3; out 10h   master reset
    0x3e, 0x15, 0xd3, 0x10,         // 0004 mvi a,15h; out 10h 8 bits, 1 stop
    0x21, MSG, 0x00,                // 0008 lxi h,MSG
    0x7e, 0xb7, 0xca, 0x16, 0x00,   // 000b mov a,m; ora a; jz 0016h
    0xd3, 0x11, 0x23,               // 0010 out 11h; inx h
    0xc3, 0x0b, 0x00,               // 0013 jmp 000bh
    0xdb, 0x10, 0x0f,               // 0016 in 10h; rrc       receive data?
    0xd2, 0x16, 0x00,               // 0019 jnc 0016h
    0xdb, 0x11, 0xd3, 0x11,         // 001c in 11h; out 11h
    0xc3, 0x16, 0x00,               // 0020 jmp 0016h
];
const image = new Uint8Array(MSG + 16);
image.set(PROGRAM);
image.set(Buffer.from('8800 READY\r\n\0', 'latin1'), MSG);

const a = await machine(image, e => e.boot_altair8800(1), 'the 8800 image');
check(a.e.terminals() === 1 && a.e.pc_lamps() === 0, 'the 8800 has one teletype and no 703 lamps');
a.frames(2);
check(a.e.halted() === 1 && lensLit(a.pixel(WAIT)), 'power-on: stopped, WAIT lit');
if (control !== 'no-run-8800') a.click(RUN8800);
a.frames(10);
check(a.text().includes('8800 READY\n'), 'RUN runs the program, which prints its banner');
check(a.e.halted() === 0 && !lensLit(a.pixel(WAIT)), 'running: WAIT is out');
check(ink(a.paper) > 200, `the banner is on the paper (${ink(a.paper)} pixels of ink)`);
for (const c of 'HI\r') a.e.key(0, c.charCodeAt(0));
a.frames(10);
check(a.outs[0].endsWith('READY\r\nHI\r'), 'what is typed is echoed, its Return a carriage return alone');
a.click(STOP); a.frames(2);
check(a.e.halted() === 1 && lensLit(a.pixel(WAIT)), 'STOP stops it, WAIT lit');
a.click(toggle(5)); a.click(toggle(4)); a.click(EXAMINE); a.frames(2);
// '8' is 0x38: D5, D4 and D3
const lit = [7, 6, 5, 4, 3, 2, 1, 0].map(b => lensLit(a.pixel(dataLamp(b))) ? 1 : 0).join('');
check(lit === '00111000', `EXAMINE at the toggles' 0030 shows its '8' on the data lamps (${lit})`);
check((await instance()).boot_altair8800(1) === 0, 'boot_altair8800 refuses nothing staged');

if (tapePath) {
    const b = await machine(read(tapePath), e => e.boot_altair8800(0), tapePath);
    b.click(RUN8800);
    check(b.until(/MEMORY SIZE\? $/), 'the tape loads BASIC, which asks for the memory size');
    const answer = (keys, prompt, what) => {
        for (const c of keys) b.e.key(0, c.charCodeAt(0));
        check(b.until(prompt), what);
    };
    answer('\r', /TERMINAL WIDTH\? $/, 'and the terminal width');
    answer('\r', /WANT SIN-COS-TAN-ATN\? $/, 'and whether to keep the functions');
    answer('Y\r', /BYTES FREE\n[^]*\nOK\n$/, 'and says what it has free');
    answer('PRINT 6*7\r', /PRINT 6\*7\n 42 \nOK\n$/, 'BASIC runs a line');
    // A8 is sense switch 0, which INP(255) reads
    b.click(toggle(8)); b.frame();
    answer('PRINT INP(255)\r', /PRINT INP\(255\)\n 1 \nOK\n$/, 'INP(255) reads the sense switches, A8 up');
} else {
    console.log('skip: no Altair 8800 tape, so no BASIC');
}
console.log('PASS');
