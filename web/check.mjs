// vim: ts=4:sw=4:expandtab:
// Copyright (c) 2026 Travis Geiselbrecht; MIT, see LICENSE.
//
// The wasm 703 under node, no browser: Tiny BASIC from a core image, and
// REX booted off its platter through the panel's switches. Every check
// fails the run; WEB_CONTROL=no-run leaves RUN unpressed, and must FAIL.
//
//   node web/check.mjs WASM BASIC_BIN REX_IMG
import fs from 'node:fs';

const [, , wasmPath, basicPath, rexPath] = process.argv;
const control = process.env.WEB_CONTROL ?? '';
const bytes = fs.readFileSync(wasmPath);

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
    check(e[kind](0) === 1, `${kind} took ${imagePath}`);
    const slice = Math.round(e.clock_hz() / 60);
    const m = { e, paper: '' };
    m.frame = () => {
        if (e.run(slice) !== 0) check(false, 'the run went on');
        const n = e.drain();
        if (n) m.paper += Buffer.from(new Uint8Array(e.memory.buffer, e.drained_ptr(), n)).toString('latin1');
    };
    m.frames = n => { for (let i = 0; i < n; i++) m.frame(); };
    // the guest's characters carry bit 7 and its lines end CR LF
    m.text = () => m.paper.replace(/[\x80-\xff]/g, c => String.fromCharCode(c.charCodeAt(0) & 0x7f)).replace(/\r/g, '');
    // Pace on the guest falling silent for a second: a 703 drops keys
    // typed while it is busy (AGENTS.md, Tiny BASIC).
    m.quiet = (limit = 60) => {
        let last = m.paper.length, still = 0;
        for (let i = 0; i < limit * 60 && still < 60; i++) {
            m.frame();
            still = m.paper.length === last ? still + 1 : 0;
            last = m.paper.length;
        }
    };
    m.type = line => {
        for (const c of line) { e.key(c.charCodeAt(0)); m.frames(9); }
        m.quiet();
    };
    return m;
}

const basic = await machine(basicPath, 'boot_core');
if (control !== 'no-run') basic.e.panel(0);
basic.quiet();
check(basic.text().includes('TINY BASIC'), 'BASIC prints its banner');
for (const l of ['10 FOR I=1 TO 5\r', '20 PRINT I*I\r', '30 NEXT I\r', 'RUN\r']) basic.type(l);
check(/RUN\n1\n4\n9\n16\n25\n/.test(basic.text()), 'the program prints the squares');

const rex = await machine(rexPath, 'boot_disc');
const e = rex.e;
const hex = v => v.toString(16).padStart(4, '0');
rex.frames(60);
check(e.halted() === 1 && e.pc_lamps() === 0 && rex.paper === '', 'power-on: halted at 0, nothing printed');
check(e.rack_online(0) === 1 && e.rack_online(1) === 0, 'unit 0 ONLINE, unit 1 empty');
if (control !== 'no-run') e.panel(0);
rex.quiet();
check(e.halted() === 0 && rex.text().includes('REX 703 UP'), 'RUN boots REX');
check(e.rack_reads(0) > 0, `the boot read the disc (${e.rack_reads(0)} reads)`);
const duty = Array.from({ length: 16 }, (_, i) => e.pc_duty(i));
check(duty.every(d => d >= 0 && d <= 1) && duty.some(d => d > 0), 'the PC lamps have duties');
e.panel(1); rex.frames(2);
const at = e.pc_lamps();
rex.frames(30);
check(e.halted() === 1 && e.pc_lamps() === at, `HALT holds the PC at ${hex(at)}`);
e.panel(0); rex.frames(30);
rex.type('RUN HELLO\r');
check(/RUN HELLO\nHELLO FROM WORD \d+\n/.test(rex.text()), 'a program off the disc runs');
console.log('PASS');
