// vim: ts=4:sw=4:expandtab:
// Copyright (c) 2026 Travis Geiselbrecht; MIT, see LICENSE.
//
// The 703's page: the software to boot, Fast I/O, and the keys its panel
// takes. The rest is machine.js's.

import { Machine, fetchBytes, powerOnFrom } from './machine.js';

const SOFTWARE = {
    rex: { file: 'rex.img', boot: 'boot_disc' },
    basic: { file: 'basic.bin', boot: 'boot_core' },
};
// The console, then the multiplexer's three: REX serves them all, and a
// key at one of the three brings it a shell.
const TELETYPES = ['Console', 'Teletype 1', 'Teletype 2', 'Teletype 3'];

const machine = new Machine({
    panel: document.getElementById('panel'),
    paper: document.getElementById('paper'),
    tabs: document.getElementById('teletypes'),
    status: document.getElementById('status'),
    halted: 'Halted. Press RUN on the panel.',
});
const images = {};

// The time-of-day clock's count: tenths of a millisecond since local
// midnight, what the operator set it to.
function timeOfDay() {
    const d = new Date();
    return ((d.getHours() * 3600 + d.getMinutes() * 60 + d.getSeconds()) * 10000
        + d.getMilliseconds() * 10) >>> 0;
}

async function powerOn() {
    const name = document.getElementById('software').value;
    const which = SOFTWARE[name];
    images[which.file] ??= await fetchBytes(which.file);
    const fastIO = document.getElementById('fastio').checked ? 1 : 0;
    // only REX answers the multiplexer, so Tiny BASIC gets no tabs
    const teletypes = name === 'rex' ? TELETYPES : TELETYPES.slice(0, 1);
    await machine.powerOn(images[which.file], x => x[which.boot](timeOfDay(), fastIO), teletypes);
}

// The arrows turn the knob and 1 to 4 flip the SENSE switches. Tab is the
// browser's, to move the focus on to the paper.
document.getElementById('panel').addEventListener('keydown', ev => {
    const e = machine.e;
    if (!e || ev.ctrlKey || ev.metaKey || ev.altKey) return;
    if (ev.key === 'ArrowLeft' || ev.key === 'ArrowRight') {
        e.panel_knob(ev.key === 'ArrowRight' ? 1 : -1);
    } else if ('1' <= ev.key && ev.key <= '4') {
        e.panel_key(ev.key.charCodeAt(0));
    } else {
        return;
    }
    ev.preventDefault();
});

powerOnFrom(document.getElementById('controls'), machine, powerOn);
