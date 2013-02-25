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
#include <iostream>
#include <cstdio>
#include <assert.h>

#include "cpu6809.h"
#include "system.h"
#include "bits.h"

using namespace std;

/* exceptions */
#define EXC_RESET 0x1
#define EXC_NMI   0x2
#define EXC_SWI   0x4
#define EXC_IRQ   0x8
#define EXC_FIRQ  0x10
#define EXC_SWI2  0x20
#define EXC_SWI3  0x40

/* decode table */
enum addrMode {
	UNKNOWN,
	IMPLIED,
	IMMEDIATE,
	DIRECT,
	EXTENDED,
	INDEXED,
	BRANCH,
};

enum op {
	ADDA,
	ADDB,
	ADDD,
	BRA,
};

struct opdecode {
	const char *name;
	addrMode mode;
	int width; // 1 or 2
	op op;
	union {
		unsigned int cond;
	};
};

static const opdecode ops[256] = {
[0x8b] = { "adda", IMMEDIATE, 1, ADDA },
[0xcb] = { "addb", IMMEDIATE, 1, ADDB },
[0xc3] = { "addd", IMMEDIATE, 2, ADDD },

[0x9b] = { "adda", DIRECT, 1, ADDA },
[0xdb] = { "addb", DIRECT, 1, ADDB },
[0xd3] = { "addd", DIRECT, 2, ADDD },

[0xbb] = { "adda", EXTENDED, 1, ADDA },
[0xfb] = { "addb", EXTENDED, 1, ADDB },
[0xf3] = { "addd", EXTENDED, 2, ADDD },

[0xab] = { "adda", INDEXED, 1, ADDA },
[0xeb] = { "addb", INDEXED, 1, ADDB },
[0xe3] = { "addd", INDEXED, 2, ADDD },

[0x20] = { "bra",  BRANCH, 1, BRA },
[0x21] = { "brn",  BRANCH, 1, BRA },
[0x22] = { "bhi",  BRANCH, 1, BRA },
[0x23] = { "bls",  BRANCH, 1, BRA },
[0x24] = { "bcc",  BRANCH, 1, BRA },
[0x25] = { "bcs",  BRANCH, 1, BRA },
[0x26] = { "bne",  BRANCH, 1, BRA },
[0x27] = { "beq",  BRANCH, 1, BRA },
[0x28] = { "bvc",  BRANCH, 1, BRA },
[0x29] = { "bvs",  BRANCH, 1, BRA },
[0x2a] = { "bpl",  BRANCH, 1, BRA },
[0x2b] = { "bmi",  BRANCH, 1, BRA },
[0x2c] = { "bge",  BRANCH, 1, BRA },
[0x2d] = { "blt",  BRANCH, 1, BRA },
[0x2e] = { "bgt",  BRANCH, 1, BRA },
[0x2f] = { "ble",  BRANCH, 1, BRA },
};

Cpu6809::Cpu6809()
{
}

Cpu6809::~Cpu6809()
{
}

void Cpu6809::Reset()
{
	// put all the registers into their default values
	mA = mB = mX = mY = 0;
	mUP = mSP = 0;
	mDP = 0;
	mCC = 0;

	mPC = 0;

	// put the cpu in reset
	mException = EXC_RESET;
}

int Cpu6809::Run()
{

	bool done = false;
	while (!done) {
		uint8_t opcode;

		if (mException) {
			if (mException & EXC_RESET) {
				// reset, branch to the reset vector
				mPC = mSys->MemRead16(0xfffe);
				mException = 0; // clear the rest of the pending irqs
			}
			assert(!mException);
		}

		// fetch the first byte of the opcode
		opcode = mSys->MemRead8(mPC++);

		cout << "opcode " << hex << (unsigned int)opcode << " " << ops[opcode].name;

		uint8_t temp8;
		uint16_t temp16;
		int arg;

		// get the addressing mode
		switch (ops[opcode].mode) {
			case IMMEDIATE:
				cout << " IMM ";
				if (ops[opcode].width == 1)
					arg = mSys->MemRead8(mPC++);
				else {
					arg = mSys->MemRead16(mPC);
					mPC += 2;
				}
				break;
			case DIRECT:
				cout << " DIR ";
				temp8 = mSys->MemRead8(mPC++);
				if (ops[opcode].width == 1)
					arg = mSys->MemRead8(mDP << 8 | temp8);
				else
					arg = mSys->MemRead16(mDP << 8 | temp8); // XXX doesn't handle wraparound
				break;
			case EXTENDED:
				cout << " EXT ";
				temp16 = mSys->MemRead16(mPC);
				mPC += 2;
				if (ops[opcode].width == 1)
					arg = mSys->MemRead8(temp16);
				else
					arg = mSys->MemRead16(temp16);
				break;
			case BRANCH:
				cout << " BRA ";
				if (ops[opcode].width == 1)
					arg = SignExtend(mSys->MemRead8(mPC++));
				else {
					arg = SignExtend(mSys->MemRead16(mPC));
					mPC += 2;
				}
				break;
			case INDEXED:
				cout << " IDX ";
				temp8 = mSys->MemRead8(mPC++);

				// parse the index word
				if (BIT(temp8, 7) == 0) {
					// 5 bit offset
				} else {
					assert(0);
				}

				if (ops[opcode].width == 1)
					arg = mSys->MemRead8(mDP << 8 | temp8);
				else
					arg = mSys->MemRead16(mDP << 8 | temp8); // XXX doesn't handle wraparound
				break;
			default:
				assert(0);
				;
		}

		cout << " arg 0x" << hex << arg;

		// decode on our table based opcode
		switch (ops[opcode].op) {
			case ADDA: // adda
				mA += arg;
				break;
			case ADDB: // addb
				mB += arg;
				break;
			case ADDD: // addd
				mD += arg;
				break;
			case BRA: { // branch
				bool takebranch = true;

				cout << " branch with arg " << dec << arg << endl;

				// XXX test conditions

				if (takebranch) {
					mPC += arg;
					if (arg == -2) {
						cerr << "infinite loop detected, aborting" << endl;
						done = true;
					}
				}
			}
		}

		cout << endl;

		Dump();
	}

	return 0;
}

void Cpu6809::Dump()
{
	char str[256];

	snprintf(str, sizeof(str), "A 0x%02x B 0x%02x D 0x%04x X 0x%04x Y 0x%04x UP 0x%04x SP 0x%04x DP 0x%02x CC 0x%02x PC 0x%04x", 
		mA, mB, mD, mX, mY, mUP, mSP, mDP, mCC, mPC);

	cout << str << endl;
}
