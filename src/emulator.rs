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
//! The run loop: a CPU, the bus it drives, and the things that stop it.

use crate::bus::Bus;
use crate::cpu::{Cpu, StepResult};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
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
}

impl Emulator {
    pub fn new(
        cpu: Box<dyn Cpu + Send>,
        bus: Box<dyn Bus + Send>,
        shutdown: Arc<AtomicBool>,
    ) -> Self {
        Emulator { cpu, bus, shutdown, cycle_limit: None, trace: None, throttle: None }
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

        // wake the frontend, mirroring the C++ cpu thread calling
        // Console::Stop() when its Run() returns for any reason
        self.shutdown.store(true, Ordering::SeqCst);

        // the trace is a diff artifact; a truncated one silently reads as a
        // successful short run, so flush before we return
        if let Some(t) = self.trace.as_mut() {
            let _ = t.flush();
        }

        reason
    }

    fn run_inner(&mut self) -> ExitReason {
        loop {
            if self.shutdown.load(Ordering::SeqCst) {
                return ExitReason::Shutdown;
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
            // state the instruction starts from -- the same point the C++
            // oracle emits at.
            if let Some(t) = self.trace.as_mut() {
                let _ = self.cpu.trace_line(t);
            }

            match self.cpu.step(&mut *self.bus) {
                StepResult::Ok => {}
                StepResult::Halted => return ExitReason::Halted,
                StepResult::BadOpcode => return ExitReason::BadOpcode,
                StepResult::InfiniteLoop => return ExitReason::InfiniteLoop,
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

    pub fn dump(&self) {
        self.cpu.dump();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::Bus;

    struct NullBus;
    impl Bus for NullBus {
        fn read8(&mut self, _addr: u32) -> u8 {
            0
        }
        fn write8(&mut self, _addr: u32, _val: u8) {}
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
        // matches the C++ oracle: -l 100000 yields 99999 traced instructions
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
