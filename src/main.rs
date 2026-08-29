// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Permission is hereby granted, free of charge, to any person obtaining
 * a copy of this software and associated documentation files
 * (the "Software"), to deal in the Software without restriction,
 * including without limitation the rights to use, copy, modify, merge,
 * publish, distribute, sublicense, and/or sell copies of the Software,
 * and to permit persons to whom the Software is furnished to do so,
 * subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
 * IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
 * CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
 * TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
 * SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */
//! Entry point: parse args, build the machine, run the CPU on its own thread
//! while the console frontend owns the main thread.

use emu::console::sdl::SdlFrontend;
use emu::console::terminal::TerminalFrontend;
use emu::console::{ConsoleEndpoint, ConsoleFrontend};
use emu::emulator::Emulator;
use emu::system::registry;
use std::io::BufWriter;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

/// What `--throttle` asked for. `RealTime` means "the machine's own clock
/// rate, from the registry"; an explicit rate gives slow motion (or fast
/// motion) for free.
enum ThrottleArg {
    Off,
    RealTime,
    Hz(u64),
}

struct Args {
    system: String,
    rom: Option<PathBuf>,
    limit: Option<i64>,
    trace: Option<PathBuf>,
    throttle: ThrottleArg,
}

fn usage(argv0: &str) {
    eprintln!("usage: {argv0} [-h] [-c/--cpu cpu type] [-s/--system system] [-r/--rom romfile] [-l/--limit limit] [-t/--trace tracefile] [--throttle [hz]]");
    eprintln!();
    eprintln!("valid systems:");
    for s in registry::SYSTEMS {
        eprint!("  {:-10} cpu: {:-4} default rom: {}", s.name, s.cpu, s.default_rom);
        if let Some(hz) = s.clock_hz {
            eprint!("  clock: {hz} Hz");
        }
        eprintln!();
    }
    eprintln!();
    eprintln!("note: system may include a subsystem suffix like '6809-obc'.");
    eprintln!("note: cpu is currently selected by system; --cpu is accepted but ignored.");
    eprintln!("note: --trace writes one line of cpu state per instruction to tracefile.");
    eprintln!("note: --throttle paces the cpu to its real clock rate (shown above), or to an explicit rate in Hz.");
}

/// Mirrors the C++ `getopt_long` handling, including `--cpu` being accepted
/// and ignored (the cpu is chosen by the system).
fn parse_args() -> Result<Args, ()> {
    let argv: Vec<String> = std::env::args().collect();
    let argv0 = argv.first().cloned().unwrap_or_else(|| "emu".to_string());

    // same default as main.cpp
    let mut args = Args {
        system: "6809".to_string(),
        rom: None,
        limit: None,
        trace: None,
        throttle: ThrottleArg::Off,
    };

    let mut i = 1;
    while i < argv.len() {
        let arg = argv[i].as_str();

        // returns the value for an option that takes one
        let value = |i: &mut usize| -> Option<String> {
            *i += 1;
            argv.get(*i).cloned()
        };

        match arg {
            "-h" | "--help" => {
                usage(&argv0);
                return Err(());
            }
            "-c" | "--cpu" => {
                let v = value(&mut i).ok_or(())?;
                println!("cpu option: '{v}'");
            }
            "-r" | "--rom" => {
                let v = value(&mut i).ok_or(())?;
                println!("rom option: '{v}'");
                args.rom = Some(PathBuf::from(v));
            }
            "-s" | "--system" => {
                let v = value(&mut i).ok_or(())?;
                println!("system option: '{v}'");
                args.system = v;
            }
            "-l" | "--limit" => {
                let v = value(&mut i).ok_or(())?;
                let n: i64 = v.parse().map_err(|_| ())?;
                println!("cycle limit set to: {n}");
                args.limit = Some(n);
            }
            "-t" | "--trace" => {
                let v = value(&mut i).ok_or(())?;
                println!("tracing instructions to: '{v}'");
                args.trace = Some(PathBuf::from(v));
            }
            "--throttle" => {
                // The rate is optional: bare --throttle means the machine's
                // own clock. There are no positional arguments, so a next
                // argument that parses as a number is the rate.
                match argv.get(i + 1).and_then(|v| v.parse::<u64>().ok()) {
                    Some(0) => {
                        eprintln!("--throttle: rate must be nonzero");
                        return Err(());
                    }
                    Some(hz) => {
                        i += 1;
                        println!("throttling to {hz} Hz");
                        args.throttle = ThrottleArg::Hz(hz);
                    }
                    None => {
                        println!("throttling to the system's own clock rate");
                        args.throttle = ThrottleArg::RealTime;
                    }
                }
            }
            _ => {
                eprintln!("unknown option '{arg}'");
                usage(&argv0);
                return Err(());
            }
        }
        i += 1;
    }

    Ok(args)
}

fn main() -> ExitCode {
    let Ok(args) = parse_args() else {
        return ExitCode::FAILURE;
    };

    // Find the system we're supposed to run.
    let Some(desc) = registry::find(&args.system) else {
        eprintln!("unknown system '{}', aborting", args.system);
        return ExitCode::FAILURE;
    };

    // Load the ROM
    let rom = args.rom.unwrap_or_else(|| PathBuf::from(desc.default_rom));
    println!("rom is {}", rom.display());

    // Create the console.
    let shutdown = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let endpoint = ConsoleEndpoint::new(rx, Box::new(std::io::stdout()));

    // Build the machine object
    let (_, subsystem) = registry::split_name(&args.system);
    let machine = match (desc.factory)(&rom, endpoint, subsystem) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error initializing system: {e}");
            return ExitCode::FAILURE;
        }
    };

    // The frontend is chosen by what the machine returned to display, not by
    // its name. Build it before the cpu thread starts so an SDL failure is a
    // clean exit rather than a spawned thread with nowhere to go.
    let mut frontend: Box<dyn ConsoleFrontend> = match machine.display {
        Some(display @ emu::console::Display::CharCell { .. }) => {
            match SdlFrontend::new(tx, display) {
                Ok(f) => Box::new(f),
                Err(e) => {
                    eprintln!("error initializing SDL: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
        Some(emu::console::Display::Panel703 { .. }) => {
            // no factory builds this yet; the panel frontend is next
            eprintln!("error: no frontend for the 703 panel yet");
            return ExitCode::FAILURE;
        }
        None => Box::new(TerminalFrontend::new(tx)),
    };

    // Throttle precedence: an explicit rate beats a bare --throttle (the
    // machine's own clock, from the registry), which beats the machine
    // factory's default; everything else runs flat out, as always.
    let throttle_hz = match args.throttle {
        ThrottleArg::Hz(hz) => Some(hz),
        ThrottleArg::RealTime => match desc.clock_hz {
            Some(hz) => Some(hz),
            None => {
                eprintln!(
                    "--throttle: system '{}' has no known clock rate; give one explicitly",
                    desc.name
                );
                return ExitCode::FAILURE;
            }
        },
        ThrottleArg::Off => machine.throttle_hz,
    };

    let mut emu = Emulator::new(machine.cpu, machine.bus, Arc::clone(&shutdown));
    emu.set_cycle_limit(args.limit);
    emu.set_throttle(throttle_hz);
    if let Some(path) = args.trace {
        match std::fs::File::create(&path) {
            Ok(f) => emu.set_trace(Some(Box::new(BufWriter::new(f)))),
            Err(e) => {
                eprintln!("error opening trace file '{}': {e}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }
    emu.reset();

    // The whole emulator moves onto the cpu thread; only the shutdown flag and
    // the keystroke channel cross the boundary.
    println!("Starting system thread");
    let cpu_thread = std::thread::spawn(move || {
        let reason = emu.run();
        println!("system thread stopping, {reason:?}");
        reason
    });

    frontend.run(Arc::clone(&shutdown));

    println!("exiting run");
    shutdown.store(true, Ordering::SeqCst);
    let _ = cpu_thread.join();
    println!("main system thread stopped");

    ExitCode::SUCCESS
}
