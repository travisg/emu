// vim: ts=4:sw=4:expandtab:
// Copyright (c) 2026 Travis Geiselbrecht; MIT, see LICENSE.
//
// The page's half of the 703: fetch the module and the software, boot a
// machine, and run it a sixtieth of a second at a time, putting the
// panel's and the paper's pixels on their canvases and passing clicks and
// keys back. The interface is web/src/lib.rs's module doc.

const FRAME_MS = 1000 / 60;
// a hidden tab or a stall runs no faster than this to catch up
const MAX_FRAMES_PER_TICK = 6;

const SOFTWARE = {
    rex: { file: 'rex.img', boot: 'boot_disc' },
    basic: { file: 'basic.bin', boot: 'boot_core' },
};

const panelCanvas = document.getElementById('panel');
const paperCanvas = document.getElementById('paper');
const panelCtx = panelCanvas.getContext('2d');
const paperCtx = paperCanvas.getContext('2d');
const status = document.getElementById('status');

async function fetchBytes(name) {
    const r = await fetch(name);
    if (!r.ok) throw new Error(`${name}: ${r.status} ${r.statusText}`);
    return new Uint8Array(await r.arrayBuffer());
}

const wasm = await fetchBytes('emu703.wasm');
const images = {};
let e = null;       // the running machine's exports
let slice = 0;      // machine cycles in a frame

// The time-of-day clock's count: tenths of a millisecond since local
// midnight, what the operator set it to.
function timeOfDay() {
    const d = new Date();
    return ((d.getHours() * 3600 + d.getMinutes() * 60 + d.getSeconds()) * 10000
        + d.getMilliseconds() * 10) >>> 0;
}

async function powerOn() {
    const which = SOFTWARE[document.getElementById('software').value];
    status.textContent = 'Loading...';
    images[which.file] ??= await fetchBytes(which.file);
    const image = images[which.file];
    // a fresh instance is a fresh machine
    const { instance } = await WebAssembly.instantiate(wasm, {});
    const x = instance.exports;
    const p = x.staging(image.length);
    new Uint8Array(x.memory.buffer, p, image.length).set(image);
    if (!x[which.boot](timeOfDay(), document.getElementById('fastio').checked ? 1 : 0)) {
        throw new Error(`${which.file} did not boot`);
    }
    panelCanvas.width = x.panel_width();
    panelCanvas.height = x.panel_height();
    paperCanvas.width = x.paper_width();
    paperCanvas.height = x.paper_height();
    slice = Math.round(x.clock_hz() / 60);
    e = x;
    // the first animation frame owes no time; draw the panel on it anyway
    owed = FRAME_MS;
}

function put(ctx, ptr, w, h) {
    // a view straight onto the module's memory, used before the next call
    ctx.putImageData(new ImageData(new Uint8ClampedArray(e.memory.buffer, ptr, w * h * 4), w, h), 0, 0);
}

let last = null, owed = 0;
function tick(now) {
    requestAnimationFrame(tick);
    if (!e) return;
    owed += Math.min(now - (last ?? now), MAX_FRAMES_PER_TICK * FRAME_MS);
    last = now;
    let panel = 0;
    // one panel render per machine frame: the lamps' filters are tuned
    // for sixty a second, whatever the display's rate
    for (; owed >= FRAME_MS; owed -= FRAME_MS) {
        e.run(slice);
        panel = e.panel_render();
    }
    if (panel) put(panelCtx, panel, panelCanvas.width, panelCanvas.height);
    const paper = e.paper_render();
    if (paper) put(paperCtx, paper, paperCanvas.width, paperCanvas.height);
    status.textContent = e.halted() ? 'Halted. Press RUN on the panel.' : '';
}

// -- the panel --------------------------------------------------------------

function panelPoint(ev) {
    const r = panelCanvas.getBoundingClientRect();
    return [
        Math.floor((ev.clientX - r.left) * panelCanvas.width / r.width),
        Math.floor((ev.clientY - r.top) * panelCanvas.height / r.height),
    ];
}

panelCanvas.addEventListener('mousedown', ev => {
    if (!e) return;
    const [x, y] = panelPoint(ev);
    e.panel_click(x, y, ev.button === 2 ? 1 : 0);
});
panelCanvas.addEventListener('contextmenu', ev => ev.preventDefault());
panelCanvas.addEventListener('keydown', ev => {
    if (!e || ev.ctrlKey || ev.metaKey || ev.altKey) return;
    if (ev.key === 'Tab' || ('1' <= ev.key && ev.key <= '4')) {
        e.panel_key(ev.key === 'Tab' ? 9 : ev.key.charCodeAt(0));
        ev.preventDefault();
    }
});

// -- the teletype -----------------------------------------------------------

// A key as the Model 33 sends it, after the SDL window's mapping
// (tty_keyboard_code and control_code in src/console): capitals only,
// Return is CR and shift-Return the LINE FEED key, both erase keys RUBOUT.
function teletypeCode(ev) {
    if (ev.metaKey || ev.altKey) return null;
    const k = ev.key;
    if (ev.ctrlKey) {
        if (k === ' ' || k === '@') return 0x00;
        if (k.length !== 1) return null;
        const c = k.toUpperCase().charCodeAt(0);
        return c >= 0x40 && c <= 0x5f ? c & 0x1f : null;
    }
    if (k === 'Enter') return ev.shiftKey ? 0x0a : 0x0d;
    if (k === 'Backspace' || k === 'Delete') return 0x7f;
    if (k === 'Escape') return 0x1b;
    if (k.length === 1) {
        const c = k.toUpperCase().charCodeAt(0);
        if (c >= 0x20 && c < 0x7f) return c;
    }
    return null;
}

paperCanvas.addEventListener('keydown', ev => {
    if (!e) return;
    const c = teletypeCode(ev);
    if (c === null) return;
    e.key(c);
    ev.preventDefault();
});
paperCanvas.addEventListener('wheel', ev => {
    if (!e || ev.deltaY === 0) return;
    // wheel up rolls the platen back into the scrollback
    e.paper_wheel(ev.deltaY < 0 ? 1 : -1);
    ev.preventDefault();
}, { passive: false });
paperCanvas.addEventListener('mousedown', () => paperCanvas.focus());

// -- power ------------------------------------------------------------------

document.getElementById('controls').addEventListener('submit', async ev => {
    ev.preventDefault();
    try {
        await powerOn();
    } catch (err) {
        status.textContent = String(err);
    }
});

try {
    await powerOn();
} catch (err) {
    status.textContent = String(err);
}
requestAnimationFrame(tick);
