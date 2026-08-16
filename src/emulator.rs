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

/// Why the run loop stopped.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExitReason {
    Shutdown,
    CycleLimit,
    Halted,
    BadOpcode,
    InfiniteLoop,
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
}

impl Emulator {
    pub fn new(
        cpu: Box<dyn Cpu + Send>,
        bus: Box<dyn Bus + Send>,
        shutdown: Arc<AtomicBool>,
    ) -> Self {
        Emulator { cpu, bus, shutdown, cycle_limit: None, trace: None }
    }

    pub fn set_cycle_limit(&mut self, limit: Option<i64>) {
        self.cycle_limit = limit;
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
    fn preset_shutdown_flag_stops_before_stepping() {
        let shutdown = Arc::new(AtomicBool::new(true));
        let steps = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let cpu = CountingCpu { steps: Arc::clone(&steps), stop_after: None };
        let mut emu = Emulator::new(Box::new(cpu), Box::new(NullBus), shutdown);
        assert_eq!(emu.run(), ExitReason::Shutdown);
        assert_eq!(steps.load(Ordering::SeqCst), 0);
    }
}
