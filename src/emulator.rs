// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! The run loop: a CPU, the bus it drives, and the things that stop it.

use crate::bus::{Bus, Endian};
use crate::console::{PanelCommand, PanelState};
use crate::cpu::{Cpu, StepResult};
use crate::debug::{DebugEvent, DebugOp, DebugReply, DebugRequest, DebugSink, StopReason};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Why the run loop stopped.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExitReason {
    Shutdown,
    CycleLimit,
    Halted,
    BadOpcode,
    InfiniteLoop,
}

/// One command on the run loop's control channel. The front panel window
/// and the debug port share the channel -- a `Receiver` has one consumer,
/// and the halted wait blocks on it, so anything that wants to move a
/// halted machine has to arrive here.
pub enum Control {
    Panel(PanelCommand),
    Debug(DebugRequest),
}

/// What a HLT, a bad opcode or a dead branch-to-self do.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StopPolicy {
    /// End the run. The headless machines: there is nothing to resume from.
    Exit,
    /// Halt, and wait for RUN from the panel or the debugger. A dead loop
    /// keeps running -- it is the authentic idle at the end of a program,
    /// and HALT is the way out.
    Halt,
}

/// The most the debugger may read in one request, so a mistyped length
/// cannot stall the machine.
const PEEK_LIMIT: u32 = 256;

/// What the throttle wants done after one more instruction. Split out of
/// [`Throttle::pace`] as a pure function of the numbers so the arithmetic,
/// the sleep threshold and the re-anchor rule are testable without touching
/// a wall clock.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Pace {
    /// Virtual time and wall time agree closely enough; keep stepping.
    Continue,
    /// Virtual time leads wall time by at least the sleep granularity.
    Sleep(Duration),
    /// Wall time leads virtual time by so much (a host stall, a debugger, a
    /// laptop asleep) that catching up would be a sprint at unthrottled
    /// speed. Forget the past and re-anchor at now instead.
    ReAnchor,
}

/// One instruction takes single-digit microseconds on the machines here --
/// far below what an OS sleep can express -- so accumulate a lead of at
/// least a millisecond before sleeping it off.
const SLEEP_GRANULARITY: Duration = Duration::from_millis(1);
/// How far wall time may lead virtual time before the throttle re-anchors
/// rather than sprinting to catch up.
const REANCHOR_LAG: Duration = Duration::from_millis(100);

fn pace_decision(hz: u64, total_cycles: u64, elapsed: Duration) -> Pace {
    // Virtual elapsed time is recomputed from the running total every time,
    // in u128, so integer division truncates once rather than accumulating
    // drift step by step -- any clock rate is exact to the nanosecond.
    let virtual_ns = total_cycles as u128 * 1_000_000_000 / hz as u128;
    let elapsed_ns = elapsed.as_nanos();
    if virtual_ns >= elapsed_ns {
        let lead = virtual_ns - elapsed_ns;
        if lead >= SLEEP_GRANULARITY.as_nanos() {
            Pace::Sleep(Duration::from_nanos(lead as u64))
        } else {
            Pace::Continue
        }
    } else if elapsed_ns - virtual_ns >= REANCHOR_LAG.as_nanos() {
        Pace::ReAnchor
    } else {
        Pace::Continue
    }
}

/// Paces the run loop to a real clock rate, fed by `Cpu::last_step_cycles`.
struct Throttle {
    hz: u64,
    /// Cycles executed since `anchor`.
    total_cycles: u64,
    anchor: Instant,
}

impl Throttle {
    fn new(hz: u64) -> Self {
        Throttle { hz, total_cycles: 0, anchor: Instant::now() }
    }

    fn pace(&mut self, cycles: u32) {
        self.total_cycles += cycles as u64;
        match pace_decision(self.hz, self.total_cycles, self.anchor.elapsed()) {
            Pace::Continue => {}
            // Sleeping the whole lead lands wall time on virtual time; the
            // lead never much exceeds the granularity, so the shutdown flag
            // is still checked every millisecond or so.
            Pace::Sleep(d) => std::thread::sleep(d),
            Pace::ReAnchor => {
                self.anchor = Instant::now();
                self.total_cycles = 0;
            }
        }
    }
}

/// Owns the whole machine. The `Emulator` moves onto the CPU thread wholesale;
/// only lightweight handles (the shutdown flag, console channels, the Kaypro
/// framebuffer) cross the thread boundary.
///
/// The C++ ownership cycle is gone: nothing here holds a back-reference. The
/// loop borrows two disjoint fields of one owner, which the borrow checker
/// accepts.
pub struct Emulator {
    cpu: Box<dyn Cpu + Send>,
    bus: Box<dyn Bus + Send>,
    shutdown: Arc<AtomicBool>,
    /// Replaces the C++ global `g_cycle_limit`. Counts *instructions*, not
    /// clocks, and is decremented once per `step()`.
    cycle_limit: Option<i64>,
    trace: Option<Box<dyn Write + Send>>,
    throttle: Option<Throttle>,
    /// Commands from the panel window and the debug port, when the machine
    /// has either. Without a channel nothing can ever halt the machine, so
    /// the run loop never waits.
    control: Option<Receiver<Control>>,
    stop_policy: StopPolicy,
    /// The panel's shared lamp state, when the machine has one. The run
    /// loop is the only place that knows whether the machine is halted
    /// (HLT and bad opcodes halt to the panel too, not just the switch),
    /// so it publishes every run-state change here for the HALT
    /// indicator's red lens.
    panel: Option<PanelState>,
    run_state: RunState,
    /// Where stop events and the exit go, when a debug port is attached.
    sink: Option<DebugSink>,
    /// PCs, in the core's own units, that halt the machine when reached.
    breakpoints: Vec<u32>,
    /// Instructions still owed to a SINGLE COMMAND or a `step N`. While
    /// this is non-zero a halted machine keeps stepping.
    pending_steps: u32,
    /// Instructions executed since the run began.
    executed: u64,
    /// Why the machine is halted, for `status`; None while running.
    stop_reason: Option<StopReason>,
}

/// Whether the machine is executing instructions or sitting halted at the
/// front panel waiting for RUN.
#[derive(Copy, Clone, PartialEq, Eq)]
enum RunState {
    Running,
    Halted,
}

impl Emulator {
    pub fn new(
        cpu: Box<dyn Cpu + Send>,
        bus: Box<dyn Bus + Send>,
        shutdown: Arc<AtomicBool>,
    ) -> Self {
        Emulator {
            cpu,
            bus,
            shutdown,
            cycle_limit: None,
            trace: None,
            throttle: None,
            control: None,
            stop_policy: StopPolicy::Exit,
            panel: None,
            run_state: RunState::Running,
            sink: None,
            breakpoints: Vec::new(),
            pending_steps: 0,
            executed: 0,
            stop_reason: None,
        }
    }

    /// Wire in the control channel the panel window and the debug port
    /// send on. Changes nothing else: whether the machine starts halted
    /// and what HLT does are `set_halted` and `set_stop_policy`.
    pub fn set_control(&mut self, control: Option<Receiver<Control>>) {
        self.control = control;
    }

    pub fn set_stop_policy(&mut self, policy: StopPolicy) {
        self.stop_policy = policy;
    }

    /// Start halted. A machine with a panel does, as a real one did at
    /// power-on: the operator presses RUN. Only meaningful with a control
    /// channel -- without one nothing could ever press it.
    pub fn set_halted(&mut self, halted: bool) {
        let state = if halted { RunState::Halted } else { RunState::Running };
        self.set_run_state(state);
    }

    pub fn set_debug_sink(&mut self, sink: Option<DebugSink>) {
        self.sink = sink;
    }

    /// Wire in the panel's lamp state so run-state changes reach its HALT
    /// indicator. Publishes the current state immediately, so the order of
    /// this and `set_halted` doesn't matter.
    pub fn set_panel_state(&mut self, panel: Option<PanelState>) {
        self.panel = panel;
        self.set_run_state(self.run_state);
    }

    /// Every run-state change comes through here so the panel's HALT lens
    /// always shows the truth.
    fn set_run_state(&mut self, state: RunState) {
        self.run_state = state;
        if let Some(p) = &self.panel {
            p.set_halted(state == RunState::Halted);
        }
    }

    pub fn set_cycle_limit(&mut self, limit: Option<i64>) {
        self.cycle_limit = limit;
    }

    /// Pace the run loop to `hz` clock cycles per second of wall time, as
    /// reported by the core's `last_step_cycles`. `None` (the default) runs
    /// flat out, as every machine here always has.
    pub fn set_throttle(&mut self, hz: Option<u64>) {
        self.throttle = hz.map(Throttle::new);
    }

    pub fn set_trace(&mut self, trace: Option<Box<dyn Write + Send>>) {
        self.trace = trace;
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut *self.bus);
    }

    pub fn run(&mut self) -> ExitReason {
        let reason = self.run_inner();

        // the debugger hears why before the flag pulls the process down
        if let Some(sink) = &self.sink {
            sink.event(DebugEvent::Exit(reason));
        }

        // wake the frontend, mirroring the C++ cpu thread calling
        // Console::Stop() when its Run() returns for any reason
        self.shutdown.store(true, Ordering::SeqCst);

        // a truncated trace silently reads as a successful short run, so
        // flush before we return
        if let Some(t) = self.trace.as_mut() {
            let _ = t.flush();
        }

        reason
    }

    /// Whether the machine is sitting still: halted with no step budget
    /// left. This is when the run loop waits on the channel instead of
    /// polling it.
    fn waiting(&self) -> bool {
        self.run_state == RunState::Halted && self.pending_steps == 0
    }

    /// Drain pending commands. While waiting this blocks on the channel
    /// (with a timeout so the shutdown flag stays responsive); otherwise it
    /// only picks up what has already arrived. `Err(())` means every sender
    /// has dropped -- without that, a halted wait would spin hot on
    /// Disconnected until the shutdown flag caught up.
    fn pump_commands(&mut self) -> Result<(), ()> {
        loop {
            // The command is moved out of this scoped match before any
            // handler runs, ending the borrow of self.control: a
            // `while let ... recv()` would hold it across the body and
            // conflict with the handlers' `&mut self`.
            let cmd = {
                let rx = self.control.as_ref().unwrap();
                if self.waiting() {
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(c) => Some(c),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => return Err(()),
                    }
                } else {
                    match rx.try_recv() {
                        Ok(c) => Some(c),
                        Err(TryRecvError::Empty) => None,
                        Err(TryRecvError::Disconnected) => return Err(()),
                    }
                }
            };
            match cmd {
                None => return Ok(()),
                Some(Control::Panel(cmd)) => self.handle_panel(&cmd),
                Some(Control::Debug(req)) => self.handle_debug(req),
            }
            // A step budget executes before the next command is looked at:
            // two SINGLE COMMANDs queued together are two instructions.
            if self.pending_steps > 0 {
                return Ok(());
            }
        }
    }

    /// Every way into the halted state that the debugger should hear
    /// about: records why, lights the lens, and reports the PC the machine
    /// stopped in front of.
    fn stop(&mut self, reason: StopReason) {
        self.pending_steps = 0;
        self.set_run_state(RunState::Halted);
        self.stop_reason = Some(reason);
        if let Some(sink) = &self.sink {
            sink.event(DebugEvent::Stopped { reason, pc: self.cpu.pc() });
        }
    }

    /// RUN: from halted, or from a step budget, which it supersedes.
    fn resume(&mut self) {
        self.pending_steps = 0;
        self.stop_reason = None;
        self.set_run_state(RunState::Running);
    }

    /// HALT. A machine that was moving -- running, or working through a
    /// step budget -- stops and says so; one already still stays that way
    /// without a second event.
    fn halt(&mut self) {
        if !self.waiting() {
            self.stop(StopReason::Request);
        }
    }

    /// Owe `n` instructions to a halted machine. "Each actuation of the
    /// switch executes one instruction ... then halts" (5-3) -- pressed
    /// while running, that means one more instruction and then the halt.
    fn step(&mut self, n: u32) {
        self.pending_steps = n;
        self.stop_reason = None;
        self.set_run_state(RunState::Halted);
    }

    /// The master reset (5-3), and back to the halted state: every
    /// operating procedure is RESET, key the registers, RUN -- a reset
    /// that left the machine free-running from word 0 would make that
    /// flow impossible.
    fn reset_and_halt(&mut self) {
        self.cpu.reset(&mut *self.bus);
        self.stop(StopReason::Reset);
    }

    /// Apply one panel command.
    fn handle_panel(&mut self, cmd: &PanelCommand) {
        match cmd {
            PanelCommand::Run => self.resume(),
            PanelCommand::Halt => self.halt(),
            PanelCommand::SingleCommand => self.step(1),
            PanelCommand::Reset => self.reset_and_halt(),
            // Everything else is data entry the core owns.
            cmd => self.cpu.panel_command(&mut *self.bus, cmd),
        }
    }

    /// Service one debugger request and send its reply. Memory goes
    /// through the bus's peek, never its read: nothing the debugger looks
    /// at may change what the guest sees.
    fn handle_debug(&mut self, req: DebugRequest) {
        let reply = match req.op {
            DebugOp::Status => self.status(),
            DebugOp::Halt => {
                self.halt();
                self.status()
            }
            DebugOp::Run => {
                self.resume();
                DebugReply::Ok
            }
            DebugOp::Step(0) => DebugReply::Err("step count must be at least 1".into()),
            DebugOp::Step(n) => {
                self.step(n);
                DebugReply::Ok
            }
            DebugOp::Reset => {
                self.reset_and_halt();
                DebugReply::Ok
            }
            DebugOp::Regs => DebugReply::Registers(self.cpu.registers()),
            DebugOp::GetReg(name) => self.register(&name),
            DebugOp::SetReg(name, value) => {
                if self.cpu.set_register(&name, value) {
                    self.register(&name)
                } else {
                    DebugReply::Err(format!("unknown register {name}"))
                }
            }
            DebugOp::Peek { addr, len } => self.peek(addr, len),
            DebugOp::Poke { addr, bytes } => {
                for (i, &b) in bytes.iter().enumerate() {
                    self.bus.poke8(addr.wrapping_add(i as u32), b);
                }
                DebugReply::Ok
            }
            DebugOp::PeekWords { addr, len } => {
                let unit = self.cpu.addressing();
                let bytes = addr.wrapping_mul(unit.unit_bytes);
                match self.peek(bytes, len.saturating_mul(2)) {
                    DebugReply::Bytes(b) => DebugReply::Words(
                        b.chunks(2)
                            .map(|w| match unit.endian {
                                Endian::Big => u16::from_be_bytes([w[0], w[1]]),
                                Endian::Little => u16::from_le_bytes([w[0], w[1]]),
                            })
                            .collect(),
                    ),
                    other => other,
                }
            }
            DebugOp::PokeWords { addr, words } => {
                let unit = self.cpu.addressing();
                let mut a = addr.wrapping_mul(unit.unit_bytes);
                for w in words {
                    let [first, second] = match unit.endian {
                        Endian::Big => w.to_be_bytes(),
                        Endian::Little => w.to_le_bytes(),
                    };
                    self.bus.poke8(a, first);
                    self.bus.poke8(a.wrapping_add(1), second);
                    a = a.wrapping_add(2);
                }
                DebugReply::Ok
            }
            DebugOp::AddBreak(pc) => {
                if !self.breakpoints.contains(&pc) {
                    self.breakpoints.push(pc);
                }
                DebugReply::Ok
            }
            DebugOp::RemoveBreak(pc) => match self.breakpoints.iter().position(|&b| b == pc) {
                Some(i) => {
                    self.breakpoints.remove(i);
                    DebugReply::Ok
                }
                None => DebugReply::Err(format!("no breakpoint at {pc:04x}")),
            },
            DebugOp::Breaks => {
                let mut b = self.breakpoints.clone();
                b.sort_unstable();
                DebugReply::Breaks(b)
            }
        };
        // a client that hung up before its reply arrived is not an error
        let _ = req.reply.send(reply);
    }

    fn status(&self) -> DebugReply {
        DebugReply::Status {
            halted: self.waiting(),
            reason: if self.waiting() { self.stop_reason } else { None },
            pc: self.cpu.pc(),
            insns: self.executed,
        }
    }

    fn register(&self, name: &str) -> DebugReply {
        match self.cpu.registers().into_iter().find(|r| r.name == name) {
            Some(r) => DebugReply::Register(r),
            None => DebugReply::Err(format!("unknown register {name}")),
        }
    }

    fn peek(&self, addr: u32, len: u32) -> DebugReply {
        if len > PEEK_LIMIT {
            return DebugReply::Err(format!("at most {PEEK_LIMIT} bytes per request"));
        }
        let mut bytes = Vec::with_capacity(len as usize);
        for i in 0..len {
            let a = addr.wrapping_add(i);
            match self.bus.peek8(a) {
                Some(b) => bytes.push(b),
                None => return DebugReply::Err(format!("no memory at {a:04x}")),
            }
        }
        DebugReply::Bytes(bytes)
    }

    fn run_inner(&mut self) -> ExitReason {
        loop {
            if self.shutdown.load(Ordering::SeqCst) {
                return ExitReason::Shutdown;
            }

            if self.control.is_some() {
                // every sender is gone: the frontend's shutdown store is in flight
                if self.pump_commands().is_err() {
                    return ExitReason::Shutdown;
                }
                if self.waiting() {
                    // Halted: no limit decrement, no trace line, no step.
                    // Loop to wait on the channel again.
                    continue;
                }
                // A step budget falls through and executes one instruction
                // at a time through the ordinary body below -- so traces,
                // the instruction limit and the throttle see it like any
                // other step -- with run_state still Halted.
            }

            // Mirrors the C++ decrement-then-test exactly, including its
            // off-by-one: a limit of N executes N-1 instructions, because the
            // Nth iteration decrements to zero and exits before stepping.
            if let Some(limit) = self.cycle_limit.as_mut() {
                if *limit > 0 {
                    *limit -= 1;
                    if *limit == 0 {
                        println!("cycle limit reached, exiting");
                        return ExitReason::CycleLimit;
                    }
                }
            }

            // Emitted before the instruction runs, so the line describes the
            // state the instruction starts from.
            if let Some(t) = self.trace.as_mut() {
                let _ = self.cpu.trace_line(t);
            }

            let result = self.cpu.step(&mut *self.bus);
            self.executed += 1;
            let stopped = match (result, self.stop_policy) {
                (StepResult::Ok, _) => false,
                (StepResult::Halted, StopPolicy::Exit) => return ExitReason::Halted,
                (StepResult::Halted, StopPolicy::Halt) => {
                    // There is a RUN switch, so a HLT halts to it instead
                    // of ending the process.
                    println!("halted; RUN resumes");
                    self.stop(StopReason::Hlt);
                    true
                }
                (StepResult::BadOpcode, StopPolicy::Exit) => return ExitReason::BadOpcode,
                (StepResult::BadOpcode, StopPolicy::Halt) => {
                    // Halting makes a mistyped hand entry recoverable
                    // instead of fatal.
                    println!("bad opcode; halted");
                    self.stop(StopReason::BadOpcode);
                    true
                }
                // Headless this is the nothing-can-ever-change exit.
                (StepResult::InfiniteLoop, StopPolicy::Exit) => return ExitReason::InfiniteLoop,
                (StepResult::InfiniteLoop, StopPolicy::Halt) => false,
            };

            // A step budget counts down, and a breakpoint is checked on the
            // PC the machine now stands in front of -- so RUN from a
            // breakpoint executes the instruction there, no skip needed.
            if !stopped {
                if self.pending_steps > 0 {
                    self.pending_steps -= 1;
                    if self.pending_steps == 0 {
                        self.stop(StopReason::Step);
                    } else if self.at_breakpoint() {
                        self.stop(StopReason::Break);
                    }
                } else if self.run_state == RunState::Running && self.at_breakpoint() {
                    self.stop(StopReason::Break);
                }
            }

            if self.throttle.is_some() {
                match self.cpu.last_step_cycles() {
                    // 0 is the trait default: this core does not count
                    // cycles, so a throttle would either freeze or lie.
                    // Say so once and run uncapped.
                    0 => {
                        eprintln!(
                            "throttle: this cpu core does not report cycle counts; running unthrottled"
                        );
                        self.throttle = None;
                    }
                    n => self.throttle.as_mut().unwrap().pace(n),
                }
            }
        }
    }

    fn at_breakpoint(&self) -> bool {
        !self.breakpoints.is_empty() && self.breakpoints.contains(&self.cpu.pc())
    }

    pub fn dump(&self) {
        self.cpu.dump();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::Bus;
    use crate::cpu::Register;
    use crate::debug::Outbound;

    struct NullBus;
    impl Bus for NullBus {
        fn read8(&mut self, _addr: u32) -> u8 {
            0
        }
        fn write8(&mut self, _addr: u32, _val: u8) {}
    }

    /// Flat memory whose byte at `a` reads `a`, except one address that
    /// stands in for a device register and refuses to be peeked.
    struct PeekBus {
        mem: Vec<u8>,
    }
    const UNPEEKABLE: u32 = 0xdead;
    impl PeekBus {
        fn new() -> Self {
            PeekBus { mem: (0..0x10000).map(|a| a as u8).collect() }
        }
    }
    impl Bus for PeekBus {
        fn read8(&mut self, addr: u32) -> u8 {
            self.mem[(addr & 0xffff) as usize]
        }
        fn write8(&mut self, addr: u32, val: u8) {
            self.mem[(addr & 0xffff) as usize] = val;
        }
        fn peek8(&self, addr: u32) -> Option<u8> {
            (addr != UNPEEKABLE).then(|| self.mem[(addr & 0xffff) as usize])
        }
    }

    /// Counts steps so we can assert the cycle-limit arithmetic.
    struct CountingCpu {
        steps: Arc<std::sync::atomic::AtomicU64>,
        stop_after: Option<u64>,
    }

    impl Cpu for CountingCpu {
        fn reset(&mut self, _bus: &mut dyn Bus) {}
        fn step(&mut self, _bus: &mut dyn Bus) -> StepResult {
            let n = self.steps.fetch_add(1, Ordering::SeqCst) + 1;
            match self.stop_after {
                Some(limit) if n >= limit => StepResult::Halted,
                _ => StepResult::Ok,
            }
        }
        fn dump(&self) {}
        fn trace_line(&self, out: &mut dyn Write) -> std::io::Result<()> {
            writeln!(out, "step")
        }
    }

    fn emulator_with(stop_after: Option<u64>) -> (Emulator, Arc<std::sync::atomic::AtomicU64>) {
        let steps = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let cpu = CountingCpu { steps: Arc::clone(&steps), stop_after };
        let emu = Emulator::new(Box::new(cpu), Box::new(NullBus), Arc::new(AtomicBool::new(false)));
        (emu, steps)
    }

    #[test]
    fn cycle_limit_executes_n_minus_one_instructions() {
        // the historical semantics, kept: -l 100000 yields 99999 instructions
        let (mut emu, steps) = emulator_with(None);
        emu.set_cycle_limit(Some(100));
        assert_eq!(emu.run(), ExitReason::CycleLimit);
        assert_eq!(steps.load(Ordering::SeqCst), 99);
    }

    #[test]
    fn one_trace_line_per_instruction() {
        let (mut emu, steps) = emulator_with(None);
        emu.set_cycle_limit(Some(50));
        let buf: Vec<u8> = Vec::new();
        // capture into a shared buffer we can inspect afterwards
        struct Shared(Arc<std::sync::Mutex<Vec<u8>>>);
        impl Write for Shared {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        drop(buf);
        let sink = Arc::new(std::sync::Mutex::new(Vec::new()));
        emu.set_trace(Some(Box::new(Shared(Arc::clone(&sink)))));
        emu.run();

        let out = sink.lock().unwrap();
        let lines = out.iter().filter(|&&c| c == b'\n').count() as u64;
        assert_eq!(lines, steps.load(Ordering::SeqCst));
        assert_eq!(lines, 49);
    }

    #[test]
    fn no_cycle_limit_runs_until_the_cpu_stops() {
        let (mut emu, steps) = emulator_with(Some(10));
        assert_eq!(emu.run(), ExitReason::Halted);
        assert_eq!(steps.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn run_sets_the_shutdown_flag_so_the_frontend_wakes() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let steps = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let cpu = CountingCpu { steps, stop_after: Some(1) };
        let mut emu =
            Emulator::new(Box::new(cpu), Box::new(NullBus), Arc::clone(&shutdown));
        emu.run();
        assert!(shutdown.load(Ordering::SeqCst));
    }

    /// A cpu whose step results follow a script, recording steps, resets
    /// and forwarded panel commands -- the harness for the run-state tests.
    /// mpsc delivers everything queued before the sender dropped, so most
    /// tests pre-queue commands, drop the sender, and let Disconnected end
    /// the run with ExitReason::Shutdown deterministically.
    struct ScriptedCpu {
        steps: Arc<std::sync::atomic::AtomicU64>,
        resets: Arc<std::sync::atomic::AtomicU64>,
        commands: Arc<std::sync::Mutex<Vec<PanelCommand>>>,
        results: Vec<StepResult>,
        /// A settable register, for the debug request path.
        v: u32,
    }

    /// The panel rig doubles as the debug rig: the sink's client channel
    /// is `events`, and `debug()` puts a request on the control channel.
    struct PanelRig {
        emu: Emulator,
        tx: std::sync::mpsc::Sender<Control>,
        steps: Arc<std::sync::atomic::AtomicU64>,
        resets: Arc<std::sync::atomic::AtomicU64>,
        commands: Arc<std::sync::Mutex<Vec<PanelCommand>>>,
        panel: PanelState,
        events: std::sync::mpsc::Receiver<Outbound>,
    }

    impl ScriptedCpu {
        fn build(results: Vec<StepResult>) -> PanelRig {
            let steps = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let resets = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
            let cpu = ScriptedCpu {
                steps: Arc::clone(&steps),
                resets: Arc::clone(&resets),
                commands: Arc::clone(&commands),
                results,
                v: 0,
            };
            let mut emu =
                Emulator::new(Box::new(cpu), Box::new(PeekBus::new()), Arc::new(AtomicBool::new(false)));
            let (tx, rx) = std::sync::mpsc::channel();
            emu.set_control(Some(rx));
            emu.set_stop_policy(StopPolicy::Halt);
            emu.set_halted(true);
            let panel = PanelState::new();
            emu.set_panel_state(Some(panel.clone()));
            let sink = DebugSink::new();
            let (ev_tx, events) = std::sync::mpsc::channel();
            sink.attach(ev_tx);
            emu.set_debug_sink(Some(sink));
            PanelRig { emu, tx, steps, resets, commands, panel, events }
        }
    }

    /// Send one debug request; the reply arrives on the returned channel.
    fn debug(tx: &std::sync::mpsc::Sender<Control>, op: DebugOp) -> std::sync::mpsc::Receiver<DebugReply> {
        let (reply, rx) = std::sync::mpsc::channel();
        tx.send(Control::Debug(DebugRequest { op, reply })).unwrap();
        rx
    }

    /// The next event from the sink, within a generous bound.
    fn next_event(events: &std::sync::mpsc::Receiver<Outbound>) -> DebugEvent {
        match events.recv_timeout(Duration::from_secs(10)).expect("an event") {
            Outbound::Event(e) => e,
            other => panic!("expected an event, got {other:?}"),
        }
    }

    impl Cpu for ScriptedCpu {
        fn reset(&mut self, _bus: &mut dyn Bus) {
            self.resets.fetch_add(1, Ordering::SeqCst);
        }
        fn step(&mut self, _bus: &mut dyn Bus) -> StepResult {
            let n = self.steps.fetch_add(1, Ordering::SeqCst) as usize;
            self.results.get(n).copied().unwrap_or(StepResult::Ok)
        }
        fn dump(&self) {}
        fn trace_line(&self, out: &mut dyn Write) -> std::io::Result<()> {
            writeln!(out, "step")
        }
        fn panel_command(&mut self, _bus: &mut dyn Bus, cmd: &PanelCommand) {
            self.commands.lock().unwrap().push(*cmd);
        }
        /// Each step advances one unit, so the PC is the step count.
        fn pc(&self) -> u32 {
            self.steps.load(Ordering::SeqCst) as u32
        }
        fn registers(&self) -> Vec<Register> {
            vec![Register::new("PC", self.pc(), 16), Register::new("V", self.v, 16)]
        }
        fn set_register(&mut self, name: &str, value: u32) -> bool {
            if name == "V" {
                self.v = value & 0xffff;
                true
            } else {
                false
            }
        }
    }

    // -- the debugger's side of the run loop ----------------------------------

    /// The stop policy is explicit state, not inferred from the channel: a
    /// machine can carry a control channel and still exit on HLT. The sink
    /// hears the exit before the flag pulls the process down.
    #[test]
    fn hlt_exits_under_the_exit_policy_even_with_a_control_channel() {
        let PanelRig { mut emu, tx, steps, events, .. } =
            ScriptedCpu::build(vec![StepResult::Halted]);
        emu.set_stop_policy(StopPolicy::Exit);
        emu.set_halted(false);
        assert_eq!(emu.run(), ExitReason::Halted);
        assert_eq!(steps.load(Ordering::SeqCst), 1);
        assert_eq!(next_event(&events), DebugEvent::Exit(ExitReason::Halted));
        drop(tx);
    }

    #[test]
    fn debug_halt_stops_a_running_machine_and_reports_it() {
        let PanelRig { mut emu, tx, steps, events, .. } = ScriptedCpu::build(vec![]);
        emu.set_halted(false);
        let runner = std::thread::spawn(move || emu.run());
        let reply = debug(&tx, DebugOp::Halt).recv().unwrap();
        let DebugReply::Status { halted: true, reason: Some(StopReason::Request), pc, insns } = reply
        else {
            panic!("halt should reply with a halted status, got {reply:?}");
        };
        assert_eq!(pc as u64, insns);
        assert_eq!(next_event(&events), DebugEvent::Stopped { reason: StopReason::Request, pc });
        let stopped_at = steps.load(Ordering::SeqCst);
        // a second halt of a still machine is not a second event
        assert!(matches!(debug(&tx, DebugOp::Halt).recv().unwrap(), DebugReply::Status { .. }));
        assert!(events.try_recv().is_err());
        assert_eq!(steps.load(Ordering::SeqCst), stopped_at, "halted means halted");
        drop(tx);
        assert_eq!(runner.join().unwrap(), ExitReason::Shutdown);
    }

    #[test]
    fn step_n_executes_n_and_stops() {
        let PanelRig { mut emu, tx, steps, events, .. } = ScriptedCpu::build(vec![]);
        let runner = std::thread::spawn(move || emu.run());
        assert_eq!(debug(&tx, DebugOp::Step(3)).recv().unwrap(), DebugReply::Ok);
        assert_eq!(next_event(&events), DebugEvent::Stopped { reason: StopReason::Step, pc: 3 });
        assert_eq!(steps.load(Ordering::SeqCst), 3);
        assert_eq!(
            debug(&tx, DebugOp::Status).recv().unwrap(),
            DebugReply::Status { halted: true, reason: Some(StopReason::Step), pc: 3, insns: 3 }
        );
        assert_eq!(
            debug(&tx, DebugOp::Step(0)).recv().unwrap(),
            DebugReply::Err("step count must be at least 1".into())
        );
        drop(tx);
        assert_eq!(runner.join().unwrap(), ExitReason::Shutdown);
    }

    /// A breakpoint stops the machine in front of its instruction -- the
    /// reported PC is the breakpoint -- and RUN executes that instruction
    /// and carries on past it.
    #[test]
    fn a_breakpoint_halts_before_its_instruction_and_run_executes_it() {
        let PanelRig { mut emu, tx, steps, events, .. } = ScriptedCpu::build(vec![]);
        let runner = std::thread::spawn(move || emu.run());
        assert_eq!(debug(&tx, DebugOp::AddBreak(2)).recv().unwrap(), DebugReply::Ok);
        assert_eq!(debug(&tx, DebugOp::AddBreak(2)).recv().unwrap(), DebugReply::Ok, "idempotent");
        assert_eq!(debug(&tx, DebugOp::Breaks).recv().unwrap(), DebugReply::Breaks(vec![2]));
        assert_eq!(debug(&tx, DebugOp::Run).recv().unwrap(), DebugReply::Ok);
        assert_eq!(next_event(&events), DebugEvent::Stopped { reason: StopReason::Break, pc: 2 });
        assert_eq!(steps.load(Ordering::SeqCst), 2);

        assert_eq!(debug(&tx, DebugOp::Run).recv().unwrap(), DebugReply::Ok);
        let DebugReply::Status { halted: true, reason: Some(StopReason::Request), pc, .. } =
            debug(&tx, DebugOp::Halt).recv().unwrap()
        else {
            panic!("halt should report a halted machine");
        };
        assert!(pc > 2, "RUN went past the breakpoint");
        assert_eq!(next_event(&events), DebugEvent::Stopped { reason: StopReason::Request, pc });
        assert_eq!(debug(&tx, DebugOp::RemoveBreak(2)).recv().unwrap(), DebugReply::Ok);
        assert_eq!(
            debug(&tx, DebugOp::RemoveBreak(2)).recv().unwrap(),
            DebugReply::Err("no breakpoint at 0002".into())
        );
        drop(tx);
        assert_eq!(runner.join().unwrap(), ExitReason::Shutdown);
    }

    #[test]
    fn a_halt_mid_step_budget_cancels_it() {
        let PanelRig { mut emu, tx, steps, events, .. } = ScriptedCpu::build(vec![]);
        let runner = std::thread::spawn(move || emu.run());
        assert_eq!(debug(&tx, DebugOp::Step(u32::MAX)).recv().unwrap(), DebugReply::Ok);
        let DebugReply::Status { halted: true, reason: Some(StopReason::Request), pc, .. } =
            debug(&tx, DebugOp::Halt).recv().unwrap()
        else {
            panic!("halt should report a halted machine");
        };
        assert_eq!(next_event(&events), DebugEvent::Stopped { reason: StopReason::Request, pc });
        let stopped_at = steps.load(Ordering::SeqCst);
        assert!(stopped_at < u32::MAX as u64);
        // the budget is gone: nothing more executes
        assert!(events.recv_timeout(Duration::from_millis(250)).is_err());
        assert_eq!(steps.load(Ordering::SeqCst), stopped_at);
        drop(tx);
        assert_eq!(runner.join().unwrap(), ExitReason::Shutdown);
    }

    /// A halted machine answers without stepping: the requests are queued
    /// before the sender drops, and the run ends with nothing executed.
    #[test]
    fn debug_requests_are_serviced_while_halted() {
        let PanelRig { mut emu, tx, steps, .. } = ScriptedCpu::build(vec![]);
        let regs = debug(&tx, DebugOp::Regs);
        let one = debug(&tx, DebugOp::GetReg("V".into()));
        let none = debug(&tx, DebugOp::GetReg("W".into()));
        drop(tx);
        assert_eq!(emu.run(), ExitReason::Shutdown);
        assert_eq!(steps.load(Ordering::SeqCst), 0);
        assert_eq!(
            regs.recv().unwrap(),
            DebugReply::Registers(vec![Register::new("PC", 0u32, 16), Register::new("V", 0u32, 16)])
        );
        assert_eq!(one.recv().unwrap(), DebugReply::Register(Register::new("V", 0u32, 16)));
        assert_eq!(none.recv().unwrap(), DebugReply::Err("unknown register W".into()));
    }

    /// Registers and memory go through the core's and the bus's debugger
    /// accessors: a set reads back, a peek refuses a device register, and
    /// the word forms honour the core's addressing.
    #[test]
    fn set_register_and_peek_go_through_the_request_path() {
        let PanelRig { mut emu, tx, .. } = ScriptedCpu::build(vec![]);
        let set = debug(&tx, DebugOp::SetReg("V".into(), 0x1234));
        let bad_set = debug(&tx, DebugOp::SetReg("W".into(), 1));
        let bytes = debug(&tx, DebugOp::Peek { addr: 0x10, len: 3 });
        let refused = debug(&tx, DebugOp::Peek { addr: UNPEEKABLE - 2, len: 4 });
        let too_many = debug(&tx, DebugOp::Peek { addr: 0, len: PEEK_LIMIT + 1 });
        let poke = debug(&tx, DebugOp::Poke { addr: 0x20, bytes: vec![0xaa, 0xbb] });
        let poked = debug(&tx, DebugOp::Peek { addr: 0x20, len: 2 });
        // the scripted core is byte-addressed big-endian, the trait default
        let words = debug(&tx, DebugOp::PeekWords { addr: 0x30, len: 2 });
        let pokew = debug(&tx, DebugOp::PokeWords { addr: 0x40, words: vec![0x1122] });
        let pokedw = debug(&tx, DebugOp::Peek { addr: 0x40, len: 2 });
        drop(tx);
        assert_eq!(emu.run(), ExitReason::Shutdown);
        assert_eq!(set.recv().unwrap(), DebugReply::Register(Register::new("V", 0x1234u32, 16)));
        assert_eq!(bad_set.recv().unwrap(), DebugReply::Err("unknown register W".into()));
        assert_eq!(bytes.recv().unwrap(), DebugReply::Bytes(vec![0x10, 0x11, 0x12]));
        assert_eq!(refused.recv().unwrap(), DebugReply::Err("no memory at dead".into()));
        assert!(matches!(too_many.recv().unwrap(), DebugReply::Err(_)));
        assert_eq!(poke.recv().unwrap(), DebugReply::Ok);
        assert_eq!(poked.recv().unwrap(), DebugReply::Bytes(vec![0xaa, 0xbb]));
        assert_eq!(words.recv().unwrap(), DebugReply::Words(vec![0x3031, 0x3233]));
        assert_eq!(pokew.recv().unwrap(), DebugReply::Ok);
        assert_eq!(pokedw.recv().unwrap(), DebugReply::Bytes(vec![0x11, 0x22]));
    }

    /// HLT under the halt policy is a stop event the debugger can inspect
    /// from; a reset is reported the same way and leaves the machine halted.
    #[test]
    fn hlt_and_reset_are_reported_as_stops() {
        let PanelRig { mut emu, tx, resets, events, .. } =
            ScriptedCpu::build(vec![StepResult::Halted, StepResult::BadOpcode]);
        let runner = std::thread::spawn(move || emu.run());
        assert_eq!(debug(&tx, DebugOp::Run).recv().unwrap(), DebugReply::Ok);
        assert_eq!(next_event(&events), DebugEvent::Stopped { reason: StopReason::Hlt, pc: 1 });
        assert_eq!(debug(&tx, DebugOp::Run).recv().unwrap(), DebugReply::Ok);
        assert_eq!(next_event(&events), DebugEvent::Stopped { reason: StopReason::BadOpcode, pc: 2 });
        assert_eq!(debug(&tx, DebugOp::Reset).recv().unwrap(), DebugReply::Ok);
        assert_eq!(next_event(&events), DebugEvent::Stopped { reason: StopReason::Reset, pc: 2 });
        assert_eq!(resets.load(Ordering::SeqCst), 1);
        drop(tx);
        assert_eq!(runner.join().unwrap(), ExitReason::Shutdown);
        assert_eq!(next_event(&events), DebugEvent::Exit(ExitReason::Shutdown));
    }

    /// With a control channel the machine starts halted and executes
    /// nothing until told to; a dropped sender ends the run cleanly.
    #[test]
    fn panel_machine_starts_halted() {
        let PanelRig { mut emu, tx, steps, .. } = ScriptedCpu::build(vec![]);
        drop(tx);
        assert_eq!(emu.run(), ExitReason::Shutdown);
        assert_eq!(steps.load(Ordering::SeqCst), 0);
    }

    /// The HALT indicator is a red light, so every run-state change must
    /// reach the panel state: lit at power-on, doused by RUN, relit when a
    /// HLT halts to the panel. The HLT leg runs the emulator on a thread, as
    /// it really runs, because a queued RUN can't outlive the channel: the
    /// run loop treats a disconnected sender as shutdown before stepping.
    #[test]
    fn the_halt_lens_follows_the_run_state() {
        let PanelRig { mut emu, tx, panel, steps, .. } =
            ScriptedCpu::build(vec![StepResult::Halted]);
        assert!(panel.halted(), "a panel machine powers on halted");
        emu.handle_panel(&PanelCommand::Run);
        assert!(!panel.halted(), "RUN douses the lens");
        emu.handle_panel(&PanelCommand::Halt);
        assert!(panel.halted(), "the HALT switch lights it");

        // a HLT instruction halts to the panel, and the lens shows it
        emu.handle_panel(&PanelCommand::Run);
        assert!(!panel.halted());
        let runner = std::thread::spawn(move || emu.run());
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !panel.halted() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(panel.halted(), "HLT relights the lens");
        assert_eq!(steps.load(Ordering::SeqCst), 1);
        drop(tx);
        assert_eq!(runner.join().unwrap(), ExitReason::Shutdown);
    }

    #[test]
    fn single_command_steps_exactly_once_each() {
        let PanelRig { mut emu, tx, steps, .. } = ScriptedCpu::build(vec![]);
        tx.send(Control::Panel(PanelCommand::Halt)).unwrap();
        tx.send(Control::Panel(PanelCommand::SingleCommand)).unwrap();
        tx.send(Control::Panel(PanelCommand::SingleCommand)).unwrap();
        drop(tx);
        assert_eq!(emu.run(), ExitReason::Shutdown);
        assert_eq!(steps.load(Ordering::SeqCst), 2);
    }

    /// A HLT with a panel becomes a halted state, and a later SINGLE
    /// COMMAND executes again -- the process does not exit.
    #[test]
    fn hlt_halts_to_the_panel_and_resumes() {
        let PanelRig { mut emu, tx, steps, .. } =
            ScriptedCpu::build(vec![StepResult::Halted, StepResult::Ok]);
        tx.send(Control::Panel(PanelCommand::Run)).unwrap();
        tx.send(Control::Panel(PanelCommand::SingleCommand)).unwrap();
        tx.send(Control::Panel(PanelCommand::SingleCommand)).unwrap();
        drop(tx);
        assert_eq!(emu.run(), ExitReason::Shutdown);
        assert_eq!(steps.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn bad_opcode_halts_to_the_panel() {
        let PanelRig { mut emu, tx, steps, .. } =
            ScriptedCpu::build(vec![StepResult::BadOpcode, StepResult::Ok]);
        tx.send(Control::Panel(PanelCommand::Run)).unwrap();
        tx.send(Control::Panel(PanelCommand::SingleCommand)).unwrap();
        tx.send(Control::Panel(PanelCommand::SingleCommand)).unwrap();
        drop(tx);
        assert_eq!(emu.run(), ExitReason::Shutdown);
        assert_eq!(steps.load(Ordering::SeqCst), 2);
    }

    /// An InfiniteLoop result keeps running with a panel attached -- HALT
    /// is the way out of an idle loop now. The cycle limit proves it ran.
    #[test]
    fn infinite_loop_keeps_running_with_a_panel() {
        let PanelRig { mut emu, tx, steps, .. } = ScriptedCpu::build(vec![StepResult::InfiniteLoop; 100]);
        tx.send(Control::Panel(PanelCommand::Run)).unwrap();
        emu.set_cycle_limit(Some(50));
        assert_eq!(emu.run(), ExitReason::CycleLimit);
        assert_eq!(steps.load(Ordering::SeqCst), 49);
        drop(tx);
    }

    #[test]
    fn reset_resets_the_cpu_and_stays_halted() {
        let PanelRig { mut emu, tx, steps, resets, .. } = ScriptedCpu::build(vec![]);
        tx.send(Control::Panel(PanelCommand::Reset)).unwrap();
        drop(tx);
        assert_eq!(emu.run(), ExitReason::Shutdown);
        assert_eq!(resets.load(Ordering::SeqCst), 1);
        assert_eq!(steps.load(Ordering::SeqCst), 0);
    }

    /// Data-entry commands reach the core's panel_command with the bus.
    #[test]
    fn entry_commands_are_forwarded_to_the_core() {
        let PanelRig { mut emu, tx, steps, commands, .. } = ScriptedCpu::build(vec![]);
        tx.send(Control::Panel(PanelCommand::TogglePcBit(3))).unwrap();
        tx.send(Control::Panel(PanelCommand::Enter)).unwrap();
        drop(tx);
        assert_eq!(emu.run(), ExitReason::Shutdown);
        assert_eq!(steps.load(Ordering::SeqCst), 0);
        assert_eq!(*commands.lock().unwrap(), vec![PanelCommand::TogglePcBit(3), PanelCommand::Enter]);
    }

    /// Halted time must not consume the instruction limit: the machine
    /// sits halted well past what a decrementing wait loop would burn
    /// through, then a single command still executes under the limit.
    /// Asserts on exit reason and step count, never elapsed time.
    #[test]
    fn halted_waiting_does_not_consume_the_cycle_limit() {
        let PanelRig { mut emu, tx, steps, .. } = ScriptedCpu::build(vec![]);
        emu.set_cycle_limit(Some(2));
        let handle = std::thread::spawn(move || emu.run());
        // > 3 recv_timeout periods of halted waiting
        std::thread::sleep(Duration::from_millis(350));
        tx.send(Control::Panel(PanelCommand::SingleCommand)).unwrap();
        drop(tx);
        assert_eq!(handle.join().unwrap(), ExitReason::Shutdown);
        assert_eq!(steps.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn pace_decision_small_lead_keeps_going() {
        // 100 cycles at 1 MHz = 100 us of virtual time against 50 us of wall
        // time: a 50 us lead, well under the sleep granularity.
        let d = pace_decision(1_000_000, 100, Duration::from_micros(50));
        assert_eq!(d, Pace::Continue);
    }

    #[test]
    fn pace_decision_sleeps_off_a_full_lead() {
        // 2000 cycles at 1 MHz = 2 ms virtual against 500 us wall: sleep the
        // 1.5 ms difference exactly.
        let d = pace_decision(1_000_000, 2000, Duration::from_micros(500));
        assert_eq!(d, Pace::Sleep(Duration::from_micros(1500)));
    }

    #[test]
    fn pace_decision_reanchors_after_a_host_stall() {
        // 1 ms of virtual time against 200 ms of wall time: the host stalled;
        // don't sprint to catch up.
        let d = pace_decision(1_000_000, 1000, Duration::from_millis(200));
        assert_eq!(d, Pace::ReAnchor);
        // ...but a lag under the threshold just keeps going.
        let d = pace_decision(1_000_000, 1000, Duration::from_millis(50));
        assert_eq!(d, Pace::Continue);
    }

    #[test]
    fn pace_decision_is_exact_for_awkward_clock_rates() {
        // The 703's 4/7 MHz doesn't divide anything evenly. One second of
        // cycles must map to one second of virtual time to the nanosecond
        // (571429 cycles / 571429 Hz), not drift with per-step rounding.
        let hz = 571_429;
        let d = pace_decision(hz, hz, Duration::from_secs(1));
        assert_eq!(d, Pace::Continue);
        let d = pace_decision(hz, hz, Duration::from_millis(998));
        assert_eq!(d, Pace::Sleep(Duration::from_millis(2)));
    }

    #[test]
    fn throttling_a_core_that_reports_no_cycles_runs_uncapped() {
        // CountingCpu inherits the trait's default last_step_cycles() of 0.
        // At 1 Hz a working throttle would take ~50 s for 50 steps; the
        // 0-report must disable it on the first step instead.
        let (mut emu, steps) = emulator_with(Some(50));
        emu.set_throttle(Some(1));
        let start = std::time::Instant::now();
        assert_eq!(emu.run(), ExitReason::Halted);
        assert_eq!(steps.load(Ordering::SeqCst), 50);
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn preset_shutdown_flag_stops_before_stepping() {
        let shutdown = Arc::new(AtomicBool::new(true));
        let steps = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let cpu = CountingCpu { steps: Arc::clone(&steps), stop_after: None };
        let mut emu = Emulator::new(Box::new(cpu), Box::new(NullBus), shutdown);
        assert_eq!(emu.run(), ExitReason::Shutdown);
        assert_eq!(steps.load(Ordering::SeqCst), 0);
    }
}
