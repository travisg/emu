// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The debug port's line protocol: commands in, replies and events out.
//! The grammar is documented in the module doc of [`crate::debug`]; this
//! file is its parser and its formatter, and nothing here touches the
//! machine.

use super::{DebugEvent, DebugOp, DebugReply};
use crate::emulator::ExitReason;

/// One parsed command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Goes to the run loop.
    Machine(DebugOp),
    /// Keystrokes for the guest's console.
    Key(Vec<u8>),
    /// The sink's backlog of guest output.
    Backlog,
    Help,
    /// Close this connection; the machine keeps running.
    Quit,
    /// Shut the emulator down.
    Kill,
}

/// Hex, with or without a `0x`.
pub fn parse_hex(s: &str) -> Result<u32, String> {
    let digits = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
    u32::from_str_radix(digits, 16).map_err(|_| format!("'{s}' is not a hex number"))
}

/// Decimal, for counts.
pub fn parse_count(s: &str) -> Result<u32, String> {
    s.parse().map_err(|_| format!("'{s}' is not a count"))
}

fn parse_hex_max(s: &str, max: u32, what: &str) -> Result<u32, String> {
    let v = parse_hex(s)?;
    if v > max {
        return Err(format!("'{s}' is not a {what}"));
    }
    Ok(v)
}

/// Parse one line, its terminator already stripped.
pub fn parse(line: &str) -> Result<Command, String> {
    let line = line.trim_start();
    // `key` takes the rest of the line verbatim, spaces included
    if let Some(rest) = line.strip_prefix("key ").or_else(|| line.strip_prefix("KEY ")) {
        return Ok(Command::Key(unescape(rest)?));
    }
    let mut words = line.split_whitespace();
    let Some(verb) = words.next() else {
        return Err("empty command".into());
    };
    let verb = verb.to_ascii_lowercase();
    let args: Vec<&str> = words.collect();
    let arity = |n: usize| -> Result<(), String> {
        if args.len() == n {
            Ok(())
        } else {
            Err(format!("{verb} takes {n} argument{}", if n == 1 { "" } else { "s" }))
        }
    };
    let cmd = match verb.as_str() {
        "status" => {
            arity(0)?;
            Command::Machine(DebugOp::Status)
        }
        "halt" => {
            arity(0)?;
            Command::Machine(DebugOp::Halt)
        }
        "run" => {
            arity(0)?;
            Command::Machine(DebugOp::Run)
        }
        "step" => match args.as_slice() {
            [] => Command::Machine(DebugOp::Step(1)),
            [n] => Command::Machine(DebugOp::Step(parse_count(n)?)),
            _ => return Err("step takes at most one argument".into()),
        },
        "reset" => {
            arity(0)?;
            Command::Machine(DebugOp::Reset)
        }
        "regs" => {
            arity(0)?;
            Command::Machine(DebugOp::Regs)
        }
        "reg" => {
            arity(1)?;
            Command::Machine(DebugOp::GetReg(args[0].to_ascii_uppercase()))
        }
        "set" => {
            arity(2)?;
            Command::Machine(DebugOp::SetReg(args[0].to_ascii_uppercase(), parse_hex(args[1])?))
        }
        "mem" => {
            arity(2)?;
            Command::Machine(DebugOp::Peek { addr: parse_hex(args[0])?, len: parse_count(args[1])? })
        }
        "memw" => {
            arity(2)?;
            Command::Machine(DebugOp::PeekWords { addr: parse_hex(args[0])?, len: parse_count(args[1])? })
        }
        "write" => {
            let [addr, bytes @ ..] = args.as_slice() else {
                return Err("write takes an address and bytes".into());
            };
            let bytes = bytes
                .iter()
                .map(|b| parse_hex_max(b, 0xff, "byte").map(|v| v as u8))
                .collect::<Result<Vec<u8>, _>>()?;
            if bytes.is_empty() {
                return Err("write takes an address and bytes".into());
            }
            Command::Machine(DebugOp::Poke { addr: parse_hex(addr)?, bytes })
        }
        "writew" => {
            let [addr, words @ ..] = args.as_slice() else {
                return Err("writew takes an address and words".into());
            };
            let words = words
                .iter()
                .map(|w| parse_hex_max(w, 0xffff, "word").map(|v| v as u16))
                .collect::<Result<Vec<u16>, _>>()?;
            if words.is_empty() {
                return Err("writew takes an address and words".into());
            }
            Command::Machine(DebugOp::PokeWords { addr: parse_hex(addr)?, words })
        }
        "break" => {
            arity(1)?;
            Command::Machine(DebugOp::AddBreak(parse_hex(args[0])?))
        }
        "unbreak" => {
            arity(1)?;
            Command::Machine(DebugOp::RemoveBreak(parse_hex(args[0])?))
        }
        "breaks" => {
            arity(0)?;
            Command::Machine(DebugOp::Breaks)
        }
        "key" => Command::Key(Vec::new()),
        "backlog" => {
            arity(0)?;
            Command::Backlog
        }
        "help" | "?" => Command::Help,
        "quit" => {
            arity(0)?;
            Command::Quit
        }
        "kill" => {
            arity(0)?;
            Command::Kill
        }
        _ => return Err(format!("unknown command '{verb}'; try help")),
    };
    Ok(cmd)
}

/// Bytes as text: printable ASCII stands for itself, `\\ \r \n \t` for
/// what they name, `\xHH` for the rest. The one grammar serves the `key`
/// command and the `! out` event, so what a client sees is what it can
/// type back.
pub fn escape(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            b'\\' => s.push_str("\\\\"),
            b'\r' => s.push_str("\\r"),
            b'\n' => s.push_str("\\n"),
            b'\t' => s.push_str("\\t"),
            0x20..=0x7e => s.push(b as char),
            _ => s.push_str(&format!("\\x{b:02x}")),
        }
    }
    s
}

/// The inverse of [`escape`].
pub fn unescape(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        match chars.next() {
            Some('\\') => out.push(b'\\'),
            Some('r') => out.push(b'\r'),
            Some('n') => out.push(b'\n'),
            Some('t') => out.push(b'\t'),
            Some('e') => out.push(0x1b),
            Some('x') => {
                let hex: String = chars.by_ref().take(2).collect();
                match (hex.len() == 2).then(|| u8::from_str_radix(&hex, 16).ok()).flatten() {
                    Some(b) => out.push(b),
                    None => return Err(format!("bad escape '\\x{hex}'")),
                }
            }
            Some(other) => return Err(format!("bad escape '\\{other}'")),
            None => return Err("trailing backslash".into()),
        }
    }
    Ok(out)
}

fn hex_list<T: std::fmt::LowerHex>(items: &[T], width: usize) -> String {
    items.iter().map(|v| format!("{v:0width$x}")).collect::<Vec<_>>().join(" ")
}

/// One reply line, without its newline: `ok ...` or `err ...`.
pub fn format_reply(reply: &DebugReply) -> String {
    match reply {
        DebugReply::Ok => "ok".into(),
        DebugReply::Err(msg) => format!("err {msg}"),
        DebugReply::Status { halted, reason, pc, insns } => {
            let mut s = format!("ok state={}", if *halted { "halted" } else { "running" });
            if let Some(r) = reason {
                s.push_str(&format!(" reason={}", r.name()));
            }
            s.push_str(&format!(" pc={pc:04x} insns={insns}"));
            s
        }
        DebugReply::Registers(regs) => {
            let mut s = String::from("ok");
            for r in regs {
                s.push_str(&format!(" {}={}", r.name, r.hex()));
            }
            s
        }
        DebugReply::Register(r) => format!("ok {}={}", r.name, r.hex()),
        DebugReply::Bytes(b) if b.is_empty() => "ok".into(),
        DebugReply::Bytes(b) => format!("ok {}", hex_list(b, 2)),
        DebugReply::Words(w) if w.is_empty() => "ok".into(),
        DebugReply::Words(w) => format!("ok {}", hex_list(w, 4)),
        DebugReply::Breaks(b) if b.is_empty() => "ok".into(),
        DebugReply::Breaks(b) => format!("ok {}", hex_list(b, 4)),
    }
}

fn exit_name(reason: ExitReason) -> &'static str {
    match reason {
        ExitReason::Shutdown => "shutdown",
        ExitReason::CycleLimit => "limit",
        ExitReason::Halted => "halted",
        ExitReason::BadOpcode => "badop",
        ExitReason::InfiniteLoop => "loop",
    }
}

/// One event line, without its newline.
pub fn format_event(ev: &DebugEvent) -> String {
    match ev {
        DebugEvent::Stopped { reason, pc } => format!("! stopped reason={} pc={pc:04x}", reason.name()),
        DebugEvent::Exit(reason) => format!("! exit reason={}", exit_name(*reason)),
    }
}

/// Guest output as one event line.
pub fn format_output(bytes: &[u8]) -> String {
    format!("! out {}", escape(bytes))
}

/// What `help` prints, one `# ` line each.
pub const HELP: &[&str] = &[
    "status                 state, stop reason, pc, instruction count",
    "halt | run | reset     stop, resume, master reset (leaves it halted)",
    "step [N]               execute N instructions (default 1), then stop",
    "regs | reg NAME        every register, or one",
    "set NAME HEX           set a register",
    "mem ADDR LEN           LEN bytes at bus byte address ADDR",
    "memw ADDR LEN          LEN 16-bit words at core-unit address ADDR",
    "write ADDR B...        bytes to bus byte address ADDR",
    "writew ADDR W...       words to core-unit address ADDR",
    "break ADDR | unbreak ADDR | breaks",
    "key TEXT               keystrokes to the guest (\\r \\n \\t \\\\ \\xHH)",
    "backlog                the last 16 KiB of guest output",
    "quit                   close this connection; the machine runs on",
    "kill                   shut the emulator down",
    "hex addresses and values, decimal counts; events: ! stopped, ! out, ! exit",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::Register;
    use crate::debug::StopReason;

    #[test]
    fn every_command_parses() {
        let cases: Vec<(&str, Command)> = vec![
            ("status", Command::Machine(DebugOp::Status)),
            ("HALT", Command::Machine(DebugOp::Halt)),
            ("run", Command::Machine(DebugOp::Run)),
            ("step", Command::Machine(DebugOp::Step(1))),
            ("step 25", Command::Machine(DebugOp::Step(25))),
            ("reset", Command::Machine(DebugOp::Reset)),
            ("regs", Command::Machine(DebugOp::Regs)),
            ("reg ac", Command::Machine(DebugOp::GetReg("AC".into()))),
            ("set pc 0x1a4", Command::Machine(DebugOp::SetReg("PC".into(), 0x1a4))),
            ("mem 100 16", Command::Machine(DebugOp::Peek { addr: 0x100, len: 16 })),
            ("memw 40 4", Command::Machine(DebugOp::PeekWords { addr: 0x40, len: 4 })),
            ("write 80 ab CD", Command::Machine(DebugOp::Poke { addr: 0x80, bytes: vec![0xab, 0xcd] })),
            ("writew 40 1040", Command::Machine(DebugOp::PokeWords { addr: 0x40, words: vec![0x1040] })),
            ("break 1a4", Command::Machine(DebugOp::AddBreak(0x1a4))),
            ("unbreak 1a4", Command::Machine(DebugOp::RemoveBreak(0x1a4))),
            ("breaks", Command::Machine(DebugOp::Breaks)),
            ("key LIST\\r", Command::Key(b"LIST\r".to_vec())),
            ("key  two spaces", Command::Key(b" two spaces".to_vec())),
            ("key", Command::Key(Vec::new())),
            ("backlog", Command::Backlog),
            ("help", Command::Help),
            ("quit", Command::Quit),
            ("kill", Command::Kill),
        ];
        for (line, want) in cases {
            assert_eq!(parse(line), Ok(want), "{line}");
        }
    }

    #[test]
    fn counts_are_decimal_and_addresses_hex() {
        assert_eq!(parse("mem 10 10"), Ok(Command::Machine(DebugOp::Peek { addr: 0x10, len: 10 })));
        assert!(parse("step 0x10").is_err(), "a count is decimal");
        assert!(parse("mem zz 1").is_err());
        assert!(parse("write 80 100").is_err(), "a byte is at most ff");
        assert!(parse("writew 80 10000").is_err(), "a word is at most ffff");
        assert!(parse("write 80").is_err(), "a write needs bytes");
        assert!(parse("halt now").is_err(), "halt takes no argument");
        assert!(parse("").is_err());
        assert!(parse("frobnicate").unwrap_err().contains("help"));
    }

    #[test]
    fn escape_round_trips_control_bytes() {
        let bytes = b"READY\r\n\t\\ \x7f\x03\x1b\x00\xff".to_vec();
        let text = escape(&bytes);
        assert_eq!(text, "READY\\r\\n\\t\\\\ \\x7f\\x03\\x1b\\x00\\xff");
        assert_eq!(unescape(&text), Ok(bytes));
        assert_eq!(unescape("\\e"), Ok(vec![0x1b]), "\\e is accepted as a courtesy");
    }

    #[test]
    fn unescape_rejects_a_bad_escape() {
        assert!(unescape("\\q").is_err());
        assert!(unescape("\\x4").is_err());
        assert!(unescape("\\xzz").is_err());
        assert!(unescape("dangling\\").is_err());
    }

    #[test]
    fn replies_are_one_line() {
        let replies = [
            DebugReply::Ok,
            DebugReply::Err("no memory at dead".into()),
            DebugReply::Status { halted: true, reason: Some(StopReason::Break), pc: 0x1a4, insns: 12 },
            DebugReply::Status { halted: false, reason: None, pc: 0x40, insns: 0 },
            DebugReply::Registers(vec![Register::new("PC", 0x40u32, 15), Register::new("EX", 3u32, 5)]),
            DebugReply::Register(Register::new("NEG", 1u32, 1)),
            DebugReply::Bytes(vec![0xab, 0xcd]),
            DebugReply::Bytes(vec![]),
            DebugReply::Words(vec![0x1040]),
            DebugReply::Breaks(vec![0x40, 0x1a4]),
        ];
        let lines: Vec<String> = replies.iter().map(format_reply).collect();
        assert_eq!(
            lines,
            [
                "ok",
                "err no memory at dead",
                "ok state=halted reason=break pc=01a4 insns=12",
                "ok state=running pc=0040 insns=0",
                "ok PC=0040 EX=03",
                "ok NEG=1",
                "ok ab cd",
                "ok",
                "ok 1040",
                "ok 0040 01a4",
            ]
        );
        assert!(lines.iter().all(|l| !l.contains('\n')));
        assert_eq!(
            format_event(&DebugEvent::Stopped { reason: StopReason::Hlt, pc: 0x113 }),
            "! stopped reason=hlt pc=0113"
        );
        assert_eq!(format_event(&DebugEvent::Exit(ExitReason::CycleLimit)), "! exit reason=limit");
        assert_eq!(format_output(b"READY\r\n"), "! out READY\\r\\n");
    }
}
