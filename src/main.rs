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

use emu::console::sdl::SdlFrontend;
use emu::console::serial::SerialSpec;
use emu::console::terminal::TerminalFrontend;
use emu::console::windows::WindowsFrontend;
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
    /// `--disk PATH`, once per drive: the disk images, in place of the
    /// fixed name under disks/, the first drive's first.
    disks: Vec<PathBuf>,
    /// `--serial SPEC`, given once per spare serial port: their host ends.
    serial: Vec<SerialSpec>,
    /// `--debug PATH`: listen on a Unix socket for a debugger.
    debug: Option<PathBuf>,
    /// `--halt`: start halted, waiting for RUN from the panel or the port.
    halt: bool,
    /// `--restore PATH`: start from a snapshot instead of from reset.
    restore: Option<PathBuf>,
}

/// The help text: `-h` prints it on stdout, a bad option on stderr.
fn usage(argv0: &str) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    writeln!(out, "usage: {argv0} [-s system[-variant...]] [-r rom] [options]").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "systems (default 6809, or the --launch menu when run bare on a terminal):").unwrap();
    for s in registry::SYSTEMS {
        writeln!(out, "  {:<12}{}", s.name, s.title).unwrap();
        let clock = s.clock_hz.map(|hz| format!(" at {}", registry::fmt_hz(hz))).unwrap_or_default();
        writeln!(out, "  {:<12}cpu {}{clock}, rom {}", "", s.cpu, s.default_rom).unwrap();
        for (token, what) in s.variants {
            writeln!(out, "  {:<12}{:<18}{what}", "", format!("{}-{token}", s.name)).unwrap();
        }
        if let Some(disks) = s.disks {
            writeln!(out, "  {:<12}{:<18}{disks}", "", "--disk").unwrap();
        }
        for port in s.serial_ports {
            writeln!(out, "  {:<12}{:<18}{port}", "", "--serial").unwrap();
        }
    }
    writeln!(out, "  panel and tty combine with another variant in any order, e.g. ray703-panel-ptb").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "options:").unwrap();
    for (opt, what) in [
        ("-s, --system NAME", "the machine to run, from the list above"),
        ("-r, --rom FILE", "a rom, tape or image in place of the system's default"),
        ("-l, --limit N", "stop after N-1 instructions"),
        ("-t, --trace FILE", "write one line of cpu state per instruction to FILE"),
        ("--throttle [HZ]", "pace to HZ; bare, the machine's own clock (default)"),
        ("--no-throttle", "run flat out"),
        ("--fast-io", "devices finish i/o at once, not at real speed"),
        ("--disk FILE", "mount FILE in place of the default disk (above); again for the next drive"),
        ("--serial pty|tcp:PORT", "a spare serial port (above) on a pty or localhost port; once per port"),
        ("--serial window", "a 703 teletype's port in a teletype window of its own"),
        ("--debug SOCKET", "debug port on a unix socket, for tools/emudbg.py"),
        ("--halt", "start halted; needs a panel or --debug to run"),
        ("--restore FILE", "start from a snapshot of the same system"),
        ("-L, --launch", "pick the machine and what to load from a menu (bare emu on a terminal)"),
        ("-c, --cpu TYPE", "accepted and ignored: the system picks the cpu"),
        ("-h, --help", "this help"),
    ] {
        writeln!(out, "  {opt:<24}{what}").unwrap();
    }
    writeln!(out).unwrap();
    writeln!(out, "Devices keep real time at any --throttle rate.").unwrap();
    writeln!(out, "Ctrl-D exits; Ctrl-C goes to the guest.").unwrap();
    out
}

/// Mirrors the C++ `getopt_long` handling, including `--cpu` being accepted
/// and ignored (the cpu is chosen by the system).
fn parse_args(argv: Vec<String>) -> Result<Args, ()> {
    let argv0 = argv.first().cloned().unwrap_or_else(|| "emu".to_string());

    // same default as main.cpp
    let mut args = Args {
        system: "6809".to_string(),
        rom: None,
        limit: None,
        trace: None,
        throttle: ThrottleArg::Unset,
        fast_io: false,
        disks: Vec::new(),
        serial: Vec::new(),
        debug: None,
        halt: false,
        restore: None,
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
                print!("{}", usage(&argv0));
                std::process::exit(0);
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
                args.disks.push(PathBuf::from(v));
            }
            "--serial" => {
                let v = value(&mut i).ok_or(())?;
                match SerialSpec::parse(&v) {
                    Ok(spec) => args.serial.push(spec),
                    Err(e) => {
                        eprintln!("--serial: {e}");
                        return Err(());
                    }
                }
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
            "--restore" => {
                let v = value(&mut i).ok_or(())?;
                args.restore = Some(PathBuf::from(v));
            }
            _ => {
                eprintln!("unknown option '{raw}'");
                eprint!("{}", usage(&argv0));
                return Err(());
            }
        }
        i += 1;
    }

    Ok(args)
}

/// `--launch` anywhere on the line, or no arguments at all on a terminal:
/// run the picker, and stand what it chose in for the flag, ahead of the
/// rest of the line, which so gets the last word on anything both name.
/// With no arguments and no terminal -- a script, a pipe -- it is the 6809
/// as ever. `Err` is the exit status to leave with.
fn launch(mut argv: Vec<String>) -> Result<Vec<String>, ExitCode> {
    use std::io::IsTerminal;
    let bare = argv.len() == 1 && std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    match argv.iter().position(|a| a == "--launch" || a == "-L") {
        Some(at) => {
            argv.remove(at);
        }
        None if bare => {}
        None => return Ok(argv),
    }
    let extra = argv.split_off(1);
    match emu::launcher::run(std::path::Path::new("."), &argv[0], extra) {
        Ok(Some(chosen)) => {
            // the line that starts the same thing again without the picker
            let words: Vec<String> = chosen.iter().map(|a| emu::launcher::picker::quote(a)).collect();
            println!("$ {} {}", argv[0], words.join(" "));
            argv.extend(chosen);
            Ok(argv)
        }
        Ok(None) => Err(ExitCode::SUCCESS),
        Err(e) => {
            eprintln!("--launch: {e}");
            Err(ExitCode::FAILURE)
        }
    }
}

fn main() -> ExitCode {
    let argv = match launch(std::env::args().collect()) {
        Ok(argv) => argv,
        Err(code) => return code,
    };
    let Ok(args) = parse_args(argv) else {
        return ExitCode::FAILURE;
    };

    // Find the system we're supposed to run.
    let Some(desc) = registry::find(&args.system) else {
        eprintln!("unknown system '{}', aborting", args.system);
        return ExitCode::FAILURE;
    };
    if args.serial.len() > desc.serial_ports.len() {
        match desc.serial_ports.len() {
            0 => eprintln!("--serial: system '{}' has no spare serial port", desc.name),
            n => eprintln!("--serial: system '{}' takes it at most {n} times, and it was given {}", desc.name, args.serial.len()),
        }
        return ExitCode::FAILURE;
    }

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
        disks: args.disks,
        serial: args.serial,
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
        // a panel, teletypes or both, in windows of their own
        Some(display) => match WindowsFrontend::new(tx, display) {
            Ok(f) => Box::new(f),
            Err(e) => {
                eprintln!("error initializing SDL: {e}");
                return ExitCode::FAILURE;
            }
        },
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
    emu.set_system(&args.system);
    emu.set_cycle_limit(args.limit);
    emu.set_throttle(throttle_hz);
    emu.set_panel_state(machine.panel);
    emu.set_debug_sink(sink.clone());
    // The run loop's control channel: the factory made one for a window
    // with controls on it -- a panel, or the Kaypro's drive doors -- and
    // the window holds a clone of the sender inside the display; the debug
    // port shares it or gets one of its own.
    let has_window_controls = machine.control.is_some();
    let (ctl_tx, ctl_rx) = match machine.control {
        Some(registry::ControlChannel { tx, rx }) => (tx, rx),
        None => mpsc::channel(),
    };
    if has_window_controls || args.debug.is_some() {
        emu.set_control(Some(ctl_rx));
    }
    if has_panel || args.debug.is_some() {
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
    // after the reset, which would otherwise undo it
    if let Some(path) = &args.restore {
        if let Err(e) = std::fs::read(path).map_err(|e| e.to_string()).and_then(|data| emu.restore(&data)) {
            eprintln!("--restore {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        println!("restored {}", path.display());
    }
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
