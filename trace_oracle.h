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
#pragma once

#include <cstdio>

// Golden instruction-trace oracle, enabled by --trace <path>. Null when
// tracing is off.
//
// This is deliberately separate from trace.h: TRACEF prefixes every line with
// __PRETTY_FUNCTION__:__LINE__, which would make the golden trace churn
// whenever unrelated lines move, and LOCAL_TRACE is compile-time while this is
// runtime.
//
// Each core writes exactly one line per instruction, from the cpu thread only,
// at the same boundary g_cycle_limit decrements at -- that boundary is this
// codebase's definition of "one instruction", and is what the Rust port's
// step() must preserve. The trace performs no bus accesses of its own (in
// particular it must never peek the opcode: a read of a UART data register has
// side effects, so a traced run would diverge from an untraced one).
extern FILE *g_trace_file;
