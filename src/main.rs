// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Entry point: parse args, build the machine, run the CPU on its own thread
//! while the console frontend owns the main thread.

use emu::console::panel703::Panel703Frontend;
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
/// motion) for free; `Flat` is `--no-throttle`, running uncapped even on a
/// machine whose factory defaults real time on (the panel ones). `Unset`
/// means neither flag was given, which leaves that default in force.
enum ThrottleArg {
    Unset,
    Flat,
    RealTime,
    Hz(u64),
}

struct Args {
    system: String,
    rom: Option<PathBuf>,
    limit: Option<i64>,
    trace: Option<PathBuf>,
    throttle: ThrottleArg,
    /// `--fast-io`: devices complete instantly instead of at period rates.
    /// A different axis from `--throttle`, which paces the machine against the
    /// wall clock and with it what a second of device time is worth in cycles;
    /// this decides whether a device charges any time at all, so the two
    /// compose (a real-time CPU with an instant terminal is the useful panel
    /// combination) and fast-io outranks a pacing rate.
    fast_io: bool,
}

fn usage(argv0: &str) {
    eprintln!("usage: {argv0} [-h] [-c/--cpu cpu type] [-s/--system system] [-r/--rom romfile] [-l/--limit limit] [-t/--trace tracefile] [--throttle [hz]] [--no-throttle] [--fast-io]");
    eprintln!();
    eprintln!("valid systems:");
    for s in registry::SYSTEMS {
        eprint!(
            "  {:-10} cpu: {:-4} default rom: {}",
            s.name, s.cpu, s.default_rom
        );
        if let Some(hz) = s.clock_hz {
            eprint!("  clock: {hz} Hz");
        }
        eprintln!();
    }
    eprintln!();
    eprintln!("note: system may include a subsystem suffix like '6809-obc'.");
    eprintln!("note: cpu is currently selected by system; --cpu is accepted but ignored.");
    eprintln!("note: --trace writes one line of cpu state per instruction to tracefile.");
    eprintln!("note: --throttle paces the cpu to its real clock rate (shown above), or to an explicit rate in Hz (--throttle N or --throttle=N).");
    eprintln!("note: --no-throttle runs flat out, overriding the real-time default the panel machines carry.");
    eprintln!("note: device periods follow the throttle rate, so a slow-motion cpu keeps a real-time terminal.");
    eprintln!("note: --fast-io makes devices complete i/o instantly instead of at period rates (currently: the 703 teletype's 10 chars/sec). Independent of --throttle.");
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
        throttle: ThrottleArg::Unset,
        fast_io: false,
    };

    let mut i = 1;
    while i < argv.len() {
        let raw = argv[i].as_str();

        // getopt_long also took `--option=value`, and for the optional-rate
        // --throttle the attached form was the only way to name a rate at
        // all -- so peel it off and accept both spellings. Short options
        // never had it.
        let (arg, attached) = match raw.split_once('=') {
            Some((name, val)) if raw.starts_with("--") => (name, Some(val.to_string())),
            _ => (raw, None),
        };

        // returns the value for an option that takes one
        let value = |i: &mut usize| -> Option<String> {
            if attached.is_some() {
                return attached.clone();
            }
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
                // own clock. `--throttle=N` names one outright and a junk N
                // is an error; otherwise, since there are no positional
                // arguments, a next argument that parses as a number is the
                // rate.
                let rate = match &attached {
                    Some(v) => match v.parse::<u64>() {
                        Ok(hz) => Some(hz),
                        Err(_) => {
                            eprintln!("--throttle: '{v}' is not a rate in Hz");
                            return Err(());
                        }
                    },
                    None => match argv.get(i + 1).and_then(|v| v.parse::<u64>().ok()) {
                        Some(hz) => {
                            i += 1;
                            Some(hz)
                        }
                        None => None,
                    },
                };
                match rate {
                    Some(0) => {
                        eprintln!("--throttle: rate must be nonzero");
                        return Err(());
                    }
                    Some(hz) => {
                        println!("throttling to {hz} Hz");
                        args.throttle = ThrottleArg::Hz(hz);
                    }
                    None => {
                        println!("throttling to the system's own clock rate");
                        args.throttle = ThrottleArg::RealTime;
                    }
                }
            }
            "--no-throttle" => {
                if attached.is_some() {
                    eprintln!("--no-throttle takes no value");
                    return Err(());
                }
                println!("running flat out, machine default or not");
                args.throttle = ThrottleArg::Flat;
            }
            "--fast-io" => {
                if attached.is_some() {
                    eprintln!("--fast-io takes no value");
                    return Err(());
                }
                println!("devices will complete i/o instantly");
                args.fast_io = true;
            }
            _ => {
                eprintln!("unknown option '{raw}'");
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
    let opts = registry::MachineOpts {
        fast_io: args.fast_io,
    };
    let mut machine = match (desc.factory)(&rom, endpoint, subsystem, &opts) {
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
        Some(display @ emu::console::Display::Panel703 { .. }) => {
            match Panel703Frontend::new(tx, display) {
                Ok(f) => Box::new(f),
                Err(e) => {
                    eprintln!("error initializing SDL: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
        None => Box::new(TerminalFrontend::new(tx)),
    };

    // Throttle precedence: --no-throttle and an explicit rate beat a bare
    // --throttle (the machine's own clock, from the registry), which beats
    // the machine factory's default; nothing given runs flat out except on
    // the machines whose factory defaults real time (the panel ones).
    let throttle_hz = match args.throttle {
        ThrottleArg::Flat => None,
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
        ThrottleArg::Unset => machine.throttle_hz,
    };

    // Devices measure their periods in cycles, so they need the rate those
    // cycles are being issued at to keep a tenth of a second a tenth of a
    // second: a machine held to 10 kHz would otherwise take its teletype down
    // with it, minutes to the character. Set after the throttle is resolved
    // rather than through `MachineOpts`, because a machine's own default rate
    // is only known once the factory has built it.
    if let Some(hz) = throttle_hz {
        machine.bus.set_device_pacing_hz(hz);
    }

    let has_panel = machine.panel_control.is_some();
    let mut emu = Emulator::new(machine.cpu, machine.bus, Arc::clone(&shutdown));
    emu.set_cycle_limit(args.limit);
    emu.set_throttle(throttle_hz);
    emu.set_panel_control(machine.panel_control);
    emu.set_panel_state(machine.panel);
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
    if has_panel {
        // the machine starts halted, as a real one did at power-on
        println!("halted at the front panel; press RUN to start");
    }

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
