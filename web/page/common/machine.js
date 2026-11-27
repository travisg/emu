// vim: ts=4:sw=4:expandtab:
// Copyright (c) 2026 Travis Geiselbrecht; MIT, see LICENSE.
//
// What the pages share: fetching the module and the software, booting a
// machine in a fresh instance, running it a sixtieth of a second at a
// time, the panel's pixels and clicks, and the teletypes -- one paper on
// show, with tabs when the machine has more than one teletype, keys and
// the wheel going to the one shown. The interface is web/src/lib.rs's
// module doc.

const FRAME_MS = 1000 / 60;
// a hidden tab or a stall runs no faster than this to catch up
const MAX_FRAMES_PER_TICK = 6;
// a mouse wheel's notch in pixels: a trackpad's run of small deltas adds
// up to notches, three lines of paper each
const NOTCH_PX = 40;

export async function fetchBytes(name) {
    const r = await fetch(name);
    if (!r.ok) throw new Error(`${name}: ${r.status} ${r.statusText}`);
    return new Uint8Array(await r.arrayBuffer());
}

// A key as the Model 33 sends it, after the SDL window's mapping
// (tty_keyboard_code and control_code in src/console): capitals only,
// Return is CR and shift-Return the LINE FEED key, both erase keys RUBOUT.
export function teletypeCode(ev) {
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

// A page's machine: the panel canvas, the paper canvas and the tabs over
// it, and the status line. `halted` is what the status line says while the
// panel's run lamp is lit.
export class Machine {
    constructor({ panel, paper, tabs, status, halted }) {
        this.panelCanvas = panel;
        this.paperCanvas = paper;
        this.panelCtx = panel.getContext('2d');
        this.paperCtx = paper.getContext('2d');
        this.tabs = tabs;
        this.status = status;
        this.haltedMsg = halted;
        this.wasm = null;   // the compiled module, one for every power-on
        this.e = null;      // the running machine's exports
        this.slice = 0;     // machine cycles in a frame
        this.last = null;
        this.owed = 0;
        this.shown = 0;     // the teletype on show
        this.wheelPx = 0;
        this.listen();
        requestAnimationFrame(t => this.tick(t));
    }

    // The status line is a live region: write it only when it changes.
    setStatus(msg) {
        if (this.status.textContent !== msg) this.status.textContent = msg;
    }

    // A fresh instance is a fresh machine: `image`'s bytes staged, then
    // `boot(exports)`, which says whether it took them. `names` are the
    // teletypes' tab labels, the console's first; one name is no tabs.
    async powerOn(image, boot, names) {
        // the old machine stops now, so nothing writes over what this says
        this.e = null;
        this.last = null;
        this.setStatus('Loading...');
        // compiled from bytes, so a server that doesn't know
        // application/wasm still serves it
        this.wasm ??= await WebAssembly.compile(await fetchBytes('emu.wasm'));
        const x = (await WebAssembly.instantiate(this.wasm, {})).exports;
        const p = x.staging(image.length) >>> 0;
        new Uint8Array(x.memory.buffer, p, image.length).set(image);
        if (!boot(x)) throw new Error('the machine did not take its software');
        this.panelCanvas.width = x.panel_width();
        this.panelCanvas.height = x.panel_height();
        this.paperCanvas.width = x.paper_width();
        this.paperCanvas.height = x.paper_height();
        this.slice = Math.round(x.clock_hz() / 60);
        this.e = x;
        this.makeTabs(names.slice(0, x.terminals()));
        this.select(0);
        // the first animation frame owes no time; draw the panel on it anyway
        this.owed = FRAME_MS;
    }

    put(ctx, ptr, w, h) {
        // a view straight onto the module's memory, used before the next call
        ctx.putImageData(new ImageData(new Uint8ClampedArray(this.e.memory.buffer, ptr >>> 0, w * h * 4), w, h), 0, 0);
    }

    tick(now) {
        requestAnimationFrame(t => this.tick(t));
        const e = this.e;
        if (!e) return;
        this.owed += Math.min(now - (this.last ?? now), MAX_FRAMES_PER_TICK * FRAME_MS);
        this.last = now;
        let panel = 0;
        // one panel render per machine frame: the lamps' filters are tuned
        // for sixty a second, whatever the display's rate
        for (; this.owed >= FRAME_MS; this.owed -= FRAME_MS) {
            e.run(this.slice);
            panel = e.panel_render();
        }
        if (panel) this.put(this.panelCtx, panel, this.panelCanvas.width, this.panelCanvas.height);
        this.drawPaper(false);
        this.buttons.forEach((b, n) => {
            if (n !== this.shown) b.classList.toggle('unseen', e.paper_unseen(n) === 1);
        });
        this.setStatus(e.halted() ? this.haltedMsg : '');
    }

    drawPaper(force) {
        const paper = this.e.paper_render(this.shown, force ? 1 : 0);
        if (paper) this.put(this.paperCtx, paper, this.paperCanvas.width, this.paperCanvas.height);
    }

    // -- the teletypes' tabs ------------------------------------------------

    makeTabs(names) {
        this.tabs.replaceChildren();
        this.tabs.hidden = names.length < 2;
        this.buttons = names.map((name, n) => {
            const b = document.createElement('button');
            b.type = 'button';
            b.role = 'tab';
            b.textContent = name;
            b.addEventListener('click', () => {
                this.select(n);
                this.paperCanvas.focus();
            });
            this.tabs.append(b);
            return b;
        });
    }

    select(n) {
        this.shown = n;
        this.wheelPx = 0;
        this.buttons.forEach((b, i) => {
            b.setAttribute('aria-selected', i === n ? 'true' : 'false');
            if (i === n) b.classList.remove('unseen');
        });
        this.paperCanvas.setAttribute('aria-label', `${this.buttons[n]?.textContent ?? 'The teletype'}'s paper`);
        if (this.e) this.drawPaper(true);
    }

    // -- input --------------------------------------------------------------

    panelPoint(ev) {
        const c = this.panelCanvas, r = c.getBoundingClientRect();
        return [
            Math.floor((ev.clientX - r.left) * c.width / r.width),
            Math.floor((ev.clientY - r.top) * c.height / r.height),
        ];
    }

    listen() {
        const panel = this.panelCanvas, paper = this.paperCanvas;
        panel.addEventListener('mousedown', ev => {
            if (!this.e) return;
            const [x, y] = this.panelPoint(ev);
            this.e.panel_click(x, y, ev.button === 2 ? 1 : 0);
        });
        panel.addEventListener('contextmenu', ev => ev.preventDefault());

        paper.addEventListener('keydown', ev => {
            if (!this.e) return;
            const c = teletypeCode(ev);
            if (c === null) return;
            this.e.key(this.shown, c);
            ev.preventDefault();
        });
        // The wheel rolls the platen a notch at a time, deltas added up into
        // notches, and wheel up is back into the scrollback. At either stop
        // the wheel is the page's again.
        paper.addEventListener('wheel', ev => {
            const e = this.e;
            if (!e || ev.deltaY === 0) return;
            if (!e.paper_can_roll(this.shown, ev.deltaY < 0 ? 1 : -1)) {
                this.wheelPx = 0;
                return;
            }
            ev.preventDefault();
            if (Math.sign(this.wheelPx) === -Math.sign(ev.deltaY)) this.wheelPx = 0;   // reversed
            const px = ev.deltaMode === 1 ? NOTCH_PX / 3 : ev.deltaMode === 2 ? NOTCH_PX * 8 : 1;
            this.wheelPx += ev.deltaY * px;
            const notches = Math.trunc(this.wheelPx / NOTCH_PX);
            if (notches) {
                e.paper_wheel(this.shown, -notches);
                this.wheelPx -= notches * NOTCH_PX;
            }
        }, { passive: false });
        paper.addEventListener('mousedown', () => paper.focus());
    }
}

// Run `powerOn` now and on the form's submit, its failures on the status
// line.
export function powerOnFrom(form, machine, powerOn) {
    const go = async () => {
        try {
            await powerOn();
        } catch (err) {
            machine.setStatus(String(err));
        }
    };
    form.addEventListener('submit', ev => {
        ev.preventDefault();
        go();
    });
    go();
}
