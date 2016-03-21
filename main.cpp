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
#include <memory>
#include <cstdio>
#include <iostream>
#include <unistd.h>
#include <getopt.h>
#include <fcntl.h>
#include <termios.h>

#include "system/system09.h"
#include "console.h"

using namespace std;

static void usage(char **argv)
{
    fprintf(stderr, "usage: %s [-h] [-c/--cpu cpu type] [-s/--system system] [-r/--rom romfile]\n", argv[0]);

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

    // create a console object to pass to the system
    Console console;

    std::unique_ptr<System> sys(System::Factory(systemOption, console));
    if (!sys) {
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

    // start system thread
    sys->RunThreaded();

    // enter the main console run loop
    console.Run();

    printf("exiting run\n");

    sys->ShutdownThreaded();

    printf("main system thread stopped\n");

    return 0;
}

