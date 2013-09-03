// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2013 Travis Geiselbrecht
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
#include <cstdio>
#include <iostream>
#include <unistd.h>
#include <getopt.h>
#include <fcntl.h>
#include <termios.h>
#include <getopt.h>

#include "system09.h"
#include <boost/scoped_ptr.hpp>

using namespace std;

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

    fcntl(0, F_SETFL, O_NONBLOCK);

    t = oldstdout;
    t.c_lflag = ISIG; // no output processing
    // Don't interpret various control characters, pass them through instead
    t.c_cc[VINTR] = t.c_cc[VQUIT] = t.c_cc[VSUSP] = '\0';
    tcsetattr(1, TCSANOW, &t);
}

static void usage(char **argv)
{
    fprintf(stderr, "usage: %s [-h] [-c/--cpu cpu type] [-s/--system system]\n", argv[0]);

    exit(1);
}

int main(int argc, char **argv)
{
    string romOption;
    string cpuOption;
    string systemOption = "6809";

    // read in any overriding configuration from the command line
    for(;;) {
        int c;
        int option_index = 0;

        static struct option long_options[] = {
            {"help", 0, 0, 'h'},
            {"cpu", 1, 0, 'c'},
            {"rom", 1, 0, 'r'},
            {"system", 1, 0, 's'},
            {0, 0, 0, 0},
        };

        c = getopt_long(argc, argv, "c:hr:s:", long_options, &option_index);
        if(c == -1)
            break;

        switch(c) {
            case 'c':
                printf("cpu option: '%s'\n", optarg);
                cpuOption = optarg;
                break;
            case 'r':
                printf("rom option: '%s'\n", optarg);
                romOption = optarg;
                break;
            case 's':
                printf("system option: '%s'\n", optarg);
                systemOption = optarg;
                break;
            case 'h':
            default:
                usage(argv);
                break;
        }
    }

    setconsole();

    boost::scoped_ptr<System> sys(System::Factory(systemOption));
    if (sys == NULL) {
        fprintf(stderr, "error creating system, aborting\n");
        return 1;
    }

    if (cpuOption != "") {
        if (sys->SetCpu(cpuOption) < 0) {
            fprintf(stderr, "error setting cpu, aborting\n");
            return 1;
        }
    }
    if (romOption != "") {
        if (sys->SetRom(romOption) < 0) {
            fprintf(stderr, "error setting rom, aborting\n");
            return 1;
        }
    }

    if (sys->Init() < 0) {
        fprintf(stderr, "error initializing system, aborting\n");
        return 1;
    }
    sys->Run();

    return 0;
}

