// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2014 Travis Geiselbrecht
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
#include "console.h"

#include <cstdio>
#include <iostream>
#include <unistd.h>
#include <fcntl.h>
#include <termios.h>

static struct termios oldstdin;
static struct termios oldstdout;

static void resetconsole(void)
{
    tcsetattr(0, TCSANOW, &oldstdin);
    tcsetattr(1, TCSANOW, &oldstdout);
}

static void setconsole(void)
{
    struct termios t;

    tcgetattr(0, &oldstdin);
    tcgetattr(1, &oldstdout);

    atexit(&resetconsole);

    t = oldstdin;
    t.c_lflag = ISIG; // no input processing
    // Don't interpret various control characters, pass them through instead
    t.c_cc[VINTR] = t.c_cc[VQUIT] = t.c_cc[VSUSP] = '\0';
    t.c_cc[VMIN]  = 0; // nonblocking read
    t.c_cc[VTIME] = 0; // nonblocking read
    tcsetattr(0, TCSANOW, &t);

//    fcntl(0, F_SETFL, O_NONBLOCK);

    t = oldstdout;
    t.c_lflag = ISIG; // no output processing
    // Don't interpret various control characters, pass them through instead
    t.c_cc[VINTR] = t.c_cc[VQUIT] = t.c_cc[VSUSP] = '\0';
    tcsetattr(1, TCSANOW, &t);
}

Console::Console()
{
    setconsole();
}

Console::~Console()
{
    resetconsole();
}

int Console::Run()
{
    for (;;) {
        int c = getchar();

        if (c == 0x4) {
            return -1;
        } else if (c == EOF) {
            return -1;
        } else {
            std::lock_guard<std::mutex> lck(mLock);
            mInBuffer.push(c);
        }
    }
}

void Console::Putchar(char c)
{
#if 1
    putchar(c);
    fflush(stdout);
#else
    std::lock_guard<std::mutex> lck(mLock);
    mOutBuffer.push(c);
#endif
}

int Console::GetNextChar()
{
    std::lock_guard<std::mutex> lck(mLock);

    int nc = -1;
    if (!mInBuffer.empty()) {
        nc = mInBuffer.front();
        mInBuffer.pop();
    }

    return nc;
}

