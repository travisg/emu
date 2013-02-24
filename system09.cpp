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

#include "memory.h"
#include "system09.h"
#include "cpu6809.h"
#include "ihex.h"

using namespace std;

// a simple 6809 based system
System09::System09()
:	mCpu(0),
	mMem(0)
{
}

System09::~System09()
{
	delete mCpu;
	delete mMem;
}

void System09::iHexParseCallbackStatic(void *context, const uint8_t *ptr, size_t offset, size_t len)
{
	System09 *s = static_cast<System09 *>(context);
	s->iHexParseCallback(ptr, offset, len);
}

void System09::iHexParseCallback(const uint8_t *ptr, size_t offset, size_t len)
{
	//cout << "parsecallback " << (void *)ptr << " offset 0x" << hex << offset << " length 0x" << len << endl;

	for (int i = 0; i < len; i++) {
		//cout << "putting byte 0x" << hex << (unsigned int)ptr[i] << " at address " << offset + i << endl;
		mMem->WriteByte(offset + i, ptr[i]);
	}
}

int System09::Init()
{
	cout << "initializing a 6809 based system" << endl;

	// create a bank of memory
	mMem = new Memory();
	mMem->Alloc(65536);
	
	// create a 6809 based cpu
	mCpu = new Cpu6809();
	mCpu->SetSystem(this);
	mCpu->Reset();

	// preload some stuff into memory
	iHex hex;
	hex.Open("test.hex");

	hex.SetCallback(&System09::iHexParseCallbackStatic, this);
	hex.Parse();
	
	return 0;
}

int System09::Run()
{
	cout << "starting main run loop" << endl;

	return mCpu->Run();
}

uint8_t System09::MemRead8(size_t address)
{
	if (address < 0x10000)
		return mMem->ReadByte(address);
	return 0;
}

void System09::MemWrite8(size_t address, uint8_t val)
{
	if (address < 0x10000)
		mMem->WriteByte(address, val);
}

