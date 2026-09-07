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

use emu::console::ray703::Ray703Frontend;
use emu::console::sdl::SdlFrontend;
use emu::console::terminal::TerminalFrontend;
use emu::console::{ConsoleEndpoint, ConsoleFrontend};
use emu::debug::server::{spawn_unix, DebugServer};
use emu::debug::{DebugSink, TapWriter};
use emu::emulator::{Emulator, StopPolicy};
use emu::system::registry;
use std::io::BufWriter;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

/// What `--throttle` asked for. `RealTime` means "the machine's own clock
/// rate, from the registry"; an explicit rate gives slow motion (or fast
/// motion) for free; `Flat` is `--no-throttle`, running uncapped. `Unset`
/// means neither flag was given, which is real time on any machine whose
/// registry entry names a clock rate -- every one of them today.
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
    /// `--disk PATH`: the disk image, in place of the fixed name under disks/.
    disk: Option<PathBuf>,
    /// `--debug PATH`: listen on a Unix socket for a debugger.
    debug: Option<PathBuf>,
    /// `--halt`: start halted, waiting for RUN from the panel or the port.
    halt: bool,
}

fn usage(argv0: &str) {
    eprintln!("usage: {argv0} [-h] [-c/--cpu cpu type] [-s/--system system] [-r/--rom romfile] [-l/--limit limit] [-t/--trace tracefile] [--throttle [hz]] [--no-throttle] [--fast-io] [--disk image] [--debug socket] [--halt]");
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
    eprintln!("note: a machine with a known clock rate (shown above) runs at it by default.");
    eprintln!("note: --throttle paces the cpu to that rate, or to an explicit rate in Hz (--throttle N or --throttle=N).");
    eprintln!("note: --no-throttle runs flat out, overriding the real-time default.");
    eprintln!("note: device periods follow the throttle rate, so a slow-motion cpu keeps a real-time terminal.");
    eprintln!("note: --fast-io makes devices complete i/o instantly instead of at period rates (currently: the 703 teletype's 10 chars/sec). Independent of --throttle.");
    eprintln!("note: --disk PATH mounts an image in place of the fixed one under disks/ (the kaypro's floppy; the 703 mounts its four units by name).");
    eprintln!("note: --debug PATH listens on a unix socket for a debugger (halt/run/step, registers, memory, breakpoints, the console); tools/emudbg.py talks to it. HLT then halts to the debugger instead of exiting.");
    eprintln!("note: --halt starts the machine halted; needs a front panel or --debug to start it.");
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
        disk: None,
        debug: None,
        halt: false,
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
                    // The resolved rate is announced once the machine is
                    // built, so nothing is printed here.
                    Some(hz) => args.throttle = ThrottleArg::Hz(hz),
                    None => args.throttle = ThrottleArg::RealTime,
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
            "--disk" => {
                let v = value(&mut i).ok_or(())?;
                println!("disk option: '{v}'");
                args.disk = Some(PathBuf::from(v));
            }
            "--debug" => {
                let v = value(&mut i).ok_or(())?;
                args.debug = Some(PathBuf::from(v));
            }
            "--halt" => {
                if attached.is_some() {
                    eprintln!("--halt takes no value");
                    return Err(());
                }
                args.halt = true;
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
    let mut endpoint = ConsoleEndpoint::new(rx, Box::new(std::io::stdout()));
    // The debugger's keyboard: a second sender on the same channel the
    // frontend feeds, taken before the frontend takes `tx`.
    let keys_tx = tx.clone();
    // The debugger's tap on the serial output goes in before the factory
    // runs, so a factory that retargets the output (the teletype window)
    // cannot displace it.
    let sink = args.debug.as_ref().map(|_| DebugSink::new());
    if let Some(sink) = &sink {
        endpoint.set_tap(Box::new(TapWriter(sink.clone())));
    }

    // Build the machine object
    let (_, subsystem) = registry::split_name(&args.system);
    let opts = registry::MachineOpts {
        fast_io: args.fast_io,
        disk: args.disk,
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
        Some(display @ emu::console::Display::Ray703 { .. }) => {
            match Ray703Frontend::new(tx, display) {
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
    // --throttle (the machine's own clock, from the registry), which is also
    // what nothing given resolves to -- a machine runs at the rate its real
    // counterpart ran at unless told otherwise. A machine whose registry entry
    // names no clock falls back to whatever rate its factory chose, and runs
    // flat out if that is none either.
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
        ThrottleArg::Unset => desc.clock_hz.or(machine.throttle_hz),
    };

    // Devices measure their periods in cycles, so they need the rate those
    // cycles are being issued at to keep a tenth of a second a tenth of a
    // second: a machine held to 10 kHz would otherwise take its teletype down
    // with it, minutes to the character. Set after the throttle is resolved
    // rather than through `MachineOpts`, because a machine's own default rate
    // is only known once the factory has built it.
    match throttle_hz {
        Some(hz) => {
            machine.bus.set_device_pacing_hz(hz);
            println!("throttling cpu to {hz} Hz");
        }
        None => println!("cpu is unthrottled"),
    }

    let has_panel = machine.panel.is_some();
    if args.halt && !has_panel && args.debug.is_none() {
        eprintln!("--halt: nothing could start the machine again; add a front panel or --debug");
        return ExitCode::FAILURE;
    }
    let addressing = machine.cpu.addressing();
    let mut emu = Emulator::new(machine.cpu, machine.bus, Arc::clone(&shutdown));
    emu.set_cycle_limit(args.limit);
    emu.set_throttle(throttle_hz);
    emu.set_panel_state(machine.panel);
    emu.set_debug_sink(sink.clone());
    // The run loop's control channel: the factory made one for a panel
    // (its window holds a clone of the sender inside the display), and
    // the debug port shares it or gets one of its own.
    let (ctl_tx, ctl_rx) = match machine.control {
        Some(registry::ControlChannel { tx, rx }) => (tx, rx),
        None => mpsc::channel(),
    };
    if has_panel || args.debug.is_some() {
        emu.set_control(Some(ctl_rx));
        // Either has a RUN switch, so a HLT halts to it instead of ending
        // the process. A panel machine starts halted, as a real one did at
        // power-on; a debug machine only on request.
        emu.set_stop_policy(StopPolicy::Halt);
        emu.set_halted(has_panel || args.halt);
    }
    let server = args.debug.as_ref().map(|_| DebugServer {
        control: ctl_tx.clone(),
        keys: keys_tx,
        sink: sink.clone().unwrap_or_default(),
        shutdown: Arc::clone(&shutdown),
        banner: format!(
            "emu debug port: system={} cpu={} unit={} endian={}",
            desc.name,
            desc.cpu,
            addressing.unit_bytes,
            match addressing.endian {
                emu::bus::Endian::Big => "big",
                emu::bus::Endian::Little => "little",
            }
        ),
    });
    // Main's own sender goes away so that, with no debug port, closing the
    // panel window disconnects the channel and ends the run.
    drop(ctl_tx);
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
    } else if args.halt {
        println!("halted; run from the debugger to start");
    }

    // The listener comes up before the cpu thread, so a client that
    // connects as soon as the socket exists finds the machine still halted
    // if --halt asked for that.
    let server_thread = match (server, &args.debug) {
        (Some(server), Some(path)) => match spawn_unix(path.clone(), server) {
            Ok(handle) => {
                println!("debug port on {}", path.display());
                Some(handle)
            }
            Err(e) => {
                eprintln!("error opening debug socket '{}': {e}", path.display());
                return ExitCode::FAILURE;
            }
        },
        _ => None,
    };

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
    if let Some(server) = server_thread {
        // it sees the flag within a poll and removes the socket file
        let _ = server.join();
    }

    ExitCode::SUCCESS
}
