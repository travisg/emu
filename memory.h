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
#pragma once

#include <boost/utility.hpp>

#include <stdint.h>
#include <sys/types.h>

class MemoryDevice : boost::noncopyable {
public:
	MemoryDevice() {}
	virtual ~MemoryDevice() {}

	virtual uint8_t ReadByte(size_t address) = 0;
	virtual void WriteByte(size_t address, uint8_t val) = 0;

};

class Memory : public MemoryDevice {
public:
	Memory();
	virtual ~Memory();

	int Alloc(size_t len);

	// simple accessors, assumes bounds checking somewhere else
	virtual uint8_t ReadByte(size_t address) { return mMem[address]; }
	virtual void WriteByte(size_t address, uint8_t val) { mMem[address] = val; }

private:
	uint8_t *mMem;
	size_t mSize;
};
