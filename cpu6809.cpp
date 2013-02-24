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

#include "cpu6809.h"
#include "system.h"

using namespace std;

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

	//mPC = 0xfffe;
	mPC = 0; // XXX incorrect
}

int Cpu6809::Run()
{

	bool done = false;
	while (!done) {
		uint8_t opcode;

		opcode = mSys->MemRead8(mPC);
		cout << "opcode " << hex << (unsigned int)opcode << endl;

		done = true;

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
