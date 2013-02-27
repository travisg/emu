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

#define COND_A  0x0
#define COND_N  0x1
#define COND_HI 0x2
#define COND_LS 0x3
#define COND_CC 0x4
#define COND_CS 0x5
#define COND_NE 0x6
#define COND_EQ 0x7
#define COND_VC 0x8
#define COND_VS 0x9
#define COND_PL 0xa
#define COND_MI 0xb
#define COND_GE 0xc
#define COND_LT 0xd
#define COND_GT 0xe
#define COND_LE 0xf

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
	BADOP,
	ADD,
	ABX,
	BRA,
	LD,
	ST,
};

struct opdecode {
	const char *name;
	addrMode mode;
	int width; // 1 or 2
	op op;
	regnum targetreg;
	union {
		unsigned int cond;
		bool store;
	};
};

// opcode table, 0x10 extended opcodes are 0x100 -> 0x1ff, 0x11 is 0x200 -> 0x2ff
static const opdecode ops[256 * 3] = {
// alu ops
[0x8b] = { "adda", IMMEDIATE, 1, ADD, REG_A, { 0 } },
[0xcb] = { "addb", IMMEDIATE, 1, ADD, REG_B, { 0 } },
[0xc3] = { "addd", IMMEDIATE, 2, ADD, REG_D, { 0 } },

[0x9b] = { "adda", DIRECT, 1, ADD, REG_A, { 0 } },
[0xdb] = { "addb", DIRECT, 1, ADD, REG_B, { 0 } },
[0xd3] = { "addd", DIRECT, 2, ADD, REG_D, { 0 } },

[0xbb] = { "adda", EXTENDED, 1, ADD, REG_A, { 0 } },
[0xfb] = { "addb", EXTENDED, 1, ADD, REG_B, { 0 } },
[0xf3] = { "addd", EXTENDED, 2, ADD, REG_D, { 0 } },

[0xab] = { "adda", INDEXED, 1, ADD, REG_A, { 0 } },
[0xeb] = { "addb", INDEXED, 1, ADD, REG_B, { 0 } },
[0xe3] = { "addd", INDEXED, 2, ADD, REG_D, { 0 } },

// misc
[0x3a] = { "abx",  IMPLIED, 2, ABX, REG_X, { 0 } },

// loads
[0x86] = { "lda",  IMMEDIATE, 1, LD, REG_A, { .store = false } },
[0xc6] = { "ldb",  IMMEDIATE, 1, LD, REG_B, { .store = false } },
[0xcc] = { "ldd",  IMMEDIATE, 2, LD, REG_D, { .store = false } },
[0x1ce] = { "lds",  IMMEDIATE, 2, LD, REG_S, { .store = false } },
[0xce] = { "ldu",  IMMEDIATE, 2, LD, REG_U, { .store = false } },
[0x8e] = { "ldx",  IMMEDIATE, 2, LD, REG_X, { .store = false } },
[0x18e] = { "ldy",  IMMEDIATE, 2, LD, REG_Y, { .store = false } },

[0x96] = { "lda",  DIRECT, 1, LD, REG_A, { .store = false } },
[0xd6] = { "ldb",  DIRECT, 1, LD, REG_B, { .store = false } },
[0xdc] = { "ldd",  DIRECT, 2, LD, REG_D, { .store = false } },
[0x1de] = { "lds",  DIRECT, 2, LD, REG_S, { .store = false } },
[0xde] = { "ldu",  DIRECT, 2, LD, REG_U, { .store = false } },
[0x9e] = { "ldx",  DIRECT, 2, LD, REG_X, { .store = false } },
[0x19e] = { "ldy",  DIRECT, 2, LD, REG_Y, { .store = false } },

[0xa6] = { "lda",  INDEXED, 1, LD, REG_A, { .store = false } },
[0xe6] = { "ldb",  INDEXED, 1, LD, REG_B, { .store = false } },
[0xec] = { "ldd",  INDEXED, 2, LD, REG_D, { .store = false } },
[0x1ee] = { "lds",  INDEXED, 2, LD, REG_S, { .store = false } },
[0xee] = { "ldu",  INDEXED, 2, LD, REG_U, { .store = false } },
[0xae] = { "ldx",  INDEXED, 2, LD, REG_X, { .store = false } },
[0x1ae] = { "ldy",  INDEXED, 2, LD, REG_Y, { .store = false } },

[0xb6] = { "lda",  EXTENDED, 1, LD, REG_A, { .store = false } },
[0xf6] = { "ldb",  EXTENDED, 1, LD, REG_B, { .store = false } },
[0xfc] = { "ldd",  EXTENDED, 2, LD, REG_D, { .store = false } },
[0x1fe] = { "lds",  EXTENDED, 2, LD, REG_S, { .store = false } },
[0xfe] = { "ldu",  EXTENDED, 2, LD, REG_U, { .store = false } },
[0xbe] = { "ldx",  EXTENDED, 2, LD, REG_X, { .store = false } },
[0x1be] = { "ldy",  EXTENDED, 2, LD, REG_Y, { .store = false } },

// stores
[0x97] = { "sta",  DIRECT, 1, ST, REG_A, { .store = true } },
[0xd7] = { "stb",  DIRECT, 1, ST, REG_B, { .store = true } },
[0xdd] = { "std",  DIRECT, 2, ST, REG_D, { .store = true } },
[0xdf] = { "stu",  DIRECT, 2, ST, REG_U, { .store = true } },
[0x9f] = { "stx",  DIRECT, 2, ST, REG_X, { .store = true } },
[0x19f] = { "sty",  DIRECT, 2, ST, REG_Y, { .store = true } },

[0xb7] = { "sta",  EXTENDED, 1, ST, REG_A, { .store = true } },
[0xf7] = { "stb",  EXTENDED, 1, ST, REG_B, { .store = true } },
[0xfd] = { "std",  EXTENDED, 2, ST, REG_D, { .store = true } },
[0xff] = { "stu",  EXTENDED, 2, ST, REG_U, { .store = true } },
[0xbf] = { "stx",  EXTENDED, 2, ST, REG_X, { .store = true } },
[0x1bf] = { "sty",  EXTENDED, 2, ST, REG_Y, { .store = true } },

[0xa7] = { "sta",  INDEXED, 1, ST, REG_A, { .store = true } },
[0xe7] = { "stb",  INDEXED, 1, ST, REG_B, { .store = true } },
[0xed] = { "std",  INDEXED, 2, ST, REG_D, { .store = true } },
[0xef] = { "stu",  INDEXED, 2, ST, REG_U, { .store = true } },
[0xaf] = { "stx",  INDEXED, 2, ST, REG_X, { .store = true } },
[0x1af] = { "sty",  INDEXED, 2, ST, REG_Y, { .store = true } },

// branches
[0x20] = { "bra",  BRANCH, 1, BRA, REG_A, { .cond = COND_A } },
[0x21] = { "brn",  BRANCH, 1, BRA, REG_A, { .cond = COND_N } },
[0x22] = { "bhi",  BRANCH, 1, BRA, REG_A, { .cond = COND_HI } },
[0x23] = { "bls",  BRANCH, 1, BRA, REG_A, { .cond = COND_LS } },
[0x24] = { "bcc",  BRANCH, 1, BRA, REG_A, { .cond = COND_CC } },
[0x25] = { "bcs",  BRANCH, 1, BRA, REG_A, { .cond = COND_CS } },
[0x26] = { "bne",  BRANCH, 1, BRA, REG_A, { .cond = COND_NE } },
[0x27] = { "beq",  BRANCH, 1, BRA, REG_A, { .cond = COND_EQ } },
[0x28] = { "bvc",  BRANCH, 1, BRA, REG_A, { .cond = COND_VC } },
[0x29] = { "bvs",  BRANCH, 1, BRA, REG_A, { .cond = COND_VS } },
[0x2a] = { "bpl",  BRANCH, 1, BRA, REG_A, { .cond = COND_PL } },
[0x2b] = { "bmi",  BRANCH, 1, BRA, REG_A, { .cond = COND_MI } },
[0x2c] = { "bge",  BRANCH, 1, BRA, REG_A, { .cond = COND_GE } },
[0x2d] = { "blt",  BRANCH, 1, BRA, REG_A, { .cond = COND_LT } },
[0x2e] = { "bgt",  BRANCH, 1, BRA, REG_A, { .cond = COND_GT } },
[0x2f] = { "ble",  BRANCH, 1, BRA, REG_A, { .cond = COND_LE } },
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
	mU = mS = 0;
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

		cout << "opcode ";

		const opdecode *op = &ops[opcode];

		// see if it's an extended opcode
		if (opcode == 0x10) {
			opcode = mSys->MemRead8(mPC++);
			op = &ops[opcode + 0x100];
			cout << hex << 0x10 << " ";
		} else if (opcode == 0x11) {
			opcode = mSys->MemRead8(mPC++);
			op = &ops[opcode + 0x200];
			cout << hex << 0x11 << " ";
		} else {
			op = &ops[opcode];
		}

		uint8_t temp8;
		uint16_t temp16;
		int arg = 0;

		cout << hex << (unsigned int)opcode << " " << op->name;

		if (op->op == BADOP) {
			cout << endl;
			cerr << "unhandled opcode" << endl;
			assert(0);
		}

		// get the addressing mode
		switch (op->mode) {
			case IMPLIED:
				cout << " IMP";
				break;
			case IMMEDIATE:
				cout << " IMM";
				if (op->width == 1)
					arg = mSys->MemRead8(mPC++);
				else {
					arg = mSys->MemRead16(mPC);
					mPC += 2;
				}
				break;
			case DIRECT:
				cout << " DIR";
				temp8 = mSys->MemRead8(mPC++);
				temp16 = (mDP << 8) | temp8;
				if (op->store) {
					arg = temp16;
				} else {
					if (op->width == 1)
						arg = mSys->MemRead8(temp16);
					else
						arg = mSys->MemRead16(temp16); // XXX doesn't handle wraparound
				}
				break;
			case EXTENDED:
				cout << " EXT";
				temp16 = mSys->MemRead16(mPC);
				mPC += 2;
				if (op->store) {
					arg = temp16;
				} else {
					if (op->width == 1)
						arg = mSys->MemRead8(temp16);
					else
						arg = mSys->MemRead16(temp16);
				}
				break;
			case BRANCH:
				cout << " BRA";
				if (op->width == 1)
					arg = SignExtend(mSys->MemRead8(mPC++));
				else {
					arg = SignExtend(mSys->MemRead16(mPC));
					mPC += 2;
				}
				break;
			case INDEXED: {
				temp8 = mSys->MemRead8(mPC++);
				cout << " IDX word 0x" << hex << (unsigned int)temp8;

				// parse the index word
				int off = 0;
				int prepostinc = 0;
				uint16_t *reg = NULL;
				uint16_t zero = 0;
				bool indirect = !!BIT(temp8, 4);
				if (BIT(temp8, 7) == 0) {
					// 5 bit offset
					off = SignExtend(BITS(temp8, 4, 0), 4);
					indirect = false; // no indirect for this moe
				} else {
					switch (BITS(temp8, 3, 0)) {
						case 0x0: // ,R+
							prepostinc = 1;
							indirect = false; // no indirect for this moe
							break;
						case 0x1: // ,R++
							prepostinc = 2;
							break;
						case 0x2: // ,-R
							prepostinc = -1;
							indirect = false; // no indirect for this mode
							break;
						case 0x3: // ,--R
							prepostinc = -2;
							break;
						case 0x4: // ,R
							break;
						case 0x5: // B,R
							off = SignExtend(mB);
							break;
						case 0x6: // A,R
							off = SignExtend(mA);
							break;
						case 0x8: // n,R (8 bit offset)
							temp8 = mSys->MemRead8(mPC++);
							off = SignExtend(temp8);
							break;
						case 0x9: // n,R (16 bit offset)
							temp16 = mSys->MemRead16(mPC);
							mPC += 2;
							off = SignExtend(temp16);
							break;
						case 0xb: // D,R
							off = SignExtend(mD);
							break;
						case 0xc: // n,PC (8 bit offset)
							temp8 = mSys->MemRead8(mPC++);
							off = SignExtend(temp8);
							reg = &mPC;
							break;
						case 0xd: // n,PC (16 bit offset)
							temp16 = mSys->MemRead16(mPC);
							mPC += 2;
							off = SignExtend(temp16);
							reg = &mPC;
							break;
						case 0xf: // [n] (16 bit absolute indirect)
							temp16 = mSys->MemRead16(mPC);
							mPC += 2;
							off = SignExtend(temp16);
							reg = &zero;
							indirect = true;
							break;
						default:
							// unhandled mode 0x7, 0xa, 0xe (6309 modes for E, F, and W register)
							cerr << "unhandled indexed addressing mode" << endl;
							assert(0);
					}
				}

				cout << " offset " << dec << off << " prepostinc " << prepostinc;

				// register we're offsetting from in the usual case
				if (!reg) {
					switch (BITS_SHIFT(temp8, 6, 5)) {
						case 0: reg = &mX; break;
						case 1: reg = &mY; break;
						case 2: reg = &mU; break;
						case 3: reg = &mS; break;
					}
				}

				// handle predecrement
				if (prepostinc < 0)
					*reg += prepostinc;

				// compute offset
				uint16_t addr = *reg + off;

				// handle postincrement
				if (prepostinc > 0)
					*reg += prepostinc;

				cout << " addr 0x" << hex << addr;

				// if we're indirecting, load the address from addr
				if (indirect) {
					addr = mSys->MemRead16(addr);
					cout << " [addr] 0x" << hex << addr;
				}

				if (!op->store) {
					if (op->width == 1) {
						arg = mSys->MemRead8(addr);
					} else {
						arg = mSys->MemRead16(addr);
					}
				} else {
					arg = addr;
				}
				break;
			}
			default:
				cerr << "unhandled addressing mode" << endl;
				assert(0);
		}

		cout << " arg 0x" << hex << arg;

		// decode on our table based opcode
		switch (op->op) {
			case ADD: // adda,addb,addd
				temp16 = GetReg(op->targetreg);
				temp16 += arg;
				PutReg(op->targetreg, temp16);
				break;
			case ABX: // X += B
				PutReg(REG_X, GetReg(REG_X) + GetReg(REG_B));
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
			case LD: // lda,ldb,ldd,lds,ldu,ldx,ldy
				PutReg(op->targetreg, arg);
				break;
			case ST: // sta,stb,std,sts,stu,stx,sty
				if (op->width == 1)
					mSys->MemWrite8(arg, GetReg(op->targetreg));
				else
					mSys->MemWrite16(arg, GetReg(op->targetreg));
				break;
			case BADOP:
			default:
				cerr << "unhandled opcode " << op->op << endl;
				done = true;
		}

		cout << endl;

		Dump();
	}

	return 0;
}

void Cpu6809::Dump()
{
	char str[256];

	snprintf(str, sizeof(str), "A 0x%02x B 0x%02x D 0x%04x X 0x%04x Y 0x%04x U 0x%04x S 0x%04x DP 0x%02x CC 0x%02x PC 0x%04x", 
		mA, mB, mD, mX, mY, mU, mS, mDP, mCC, mPC);

	cout << str << endl;
}

uint16_t Cpu6809::GetReg(regnum r)
{
	switch (r) {
		case REG_X: return mX;
		case REG_Y: return mY;
		case REG_U: return mU;
		case REG_S: return mS;
		case REG_A: return mA;
		case REG_B: return mB;
		case REG_D: return mD;
		case REG_PC: return mPC;
	}
}

void Cpu6809::PutReg(regnum r, uint16_t val)
{
	switch (r) {
		case REG_X: mX = val; break;
		case REG_Y: mY = val; break;
		case REG_U: mU = val; break;
		case REG_S: mS = val; break;
		case REG_A: mA = val; break;
		case REG_B: mB = val; break;
		case REG_D: mD = val; break;
		case REG_PC: mPC = val; break;
	}
}

int Cpu6809::RegWidth(regnum r)
{
	switch (r) {
		default:
		case REG_A:
		case REG_B:
			return 1;
		case REG_X:
		case REG_Y:
		case REG_U:
		case REG_S:
		case REG_D:
		case REG_PC:
			return 2;
	}

}


