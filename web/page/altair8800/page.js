// vim: ts=4:sw=4:expandtab:
// Copyright (c) 2026 Travis Geiselbrecht; MIT, see LICENSE.
//
// The Altair 8800's page: BASIC's tape, through the bootstrap. The rest
// is machine.js's.

import { Machine, fetchBytes, powerOnFrom } from './machine.js';

const TAPE = 'basic8k40.tap';

const machine = new Machine({
    panel: document.getElementById('panel'),
    paper: document.getElementById('paper'),
    tabs: document.getElementById('teletypes'),
    status: document.getElementById('status'),
    halted: 'Stopped. Press RUN on the panel.',
});
let tape = null;

async function powerOn() {
    tape ??= await fetchBytes(TAPE);
    await machine.powerOn(tape, x => x.boot_altair8800(0), ['Console']);
}

powerOnFrom(document.getElementById('controls'), machine, powerOn);
