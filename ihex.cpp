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
#include "ihex.h"

#include <iostream>
#include <cctype>

using namespace std;

iHex::iHex()
{
}

iHex::~iHex()
{
    Close();
}

int iHex::Open(const string &name)
{
    Close();

    mFile.open(name.c_str(), ios::in);
    if (!mFile.is_open()) {
        return -1;
    }

    return 0;
}

void iHex::Close()
{
    mFile.close();
}

void iHex::SetCallback(const iHexCallback &cb)
{
    mParseCallback = cb;
}

static unsigned int hexnibble(char c)
{
    if (!isxdigit(c)) {
        return 0;
    }
    return (c >= '0' && c <= '9') ? (c - '0') :
           (c >= 'a' && c <= 'f') ? (c - 'a' + 10) :
           (c - 'A' + 10);
}

static unsigned int readhex8(const string &line, size_t offset)
{
    unsigned int val = hexnibble(line[offset]) << 4 | hexnibble(line[offset+1]);
    return val;
}

static unsigned int readhex16(const string &line, size_t offset)
{
    unsigned int val = (readhex8(line, offset) << 8) | readhex8(line, offset + 2);
    return val;
}

int iHex::Parse()
{
    if (!mFile.is_open())
        return -1;

    bool done = false;
    int err = 0;
    uint32_t extAddress = 0;
    while (!done && mFile.good()) {
        string line;
        getline(mFile, line);

        //cout << "line: " << line <<endl;

        size_t pos = 0;

        /* starts with : */
        if (line[pos++] != ':') {
            err = -1;
            break;
        }

        /* length byte is next */
        size_t length = readhex8(line, pos);
        pos += 2;

        uint16_t address = readhex16(line, pos);
        pos += 4;

        unsigned int type = readhex8(line, pos);
        pos += 2;

        //cout << "length " << length << " address " << address << " type " << type << endl;

        switch (type) {
            case 0: { // data record
                uint8_t *data = new uint8_t[length];

                for (size_t i = 0; i < length; i++) {
                    data[i] = readhex8(line, pos);
                    pos += 2;
                }

                unsigned int checksum = readhex8(line, pos);
                (void)checksum;
                //cout << "checksum " << checksum << endl;

                if (mParseCallback)
                    mParseCallback(data, extAddress + address, length);

                delete[] data;
                break;
            }
            case 1: // end of file
                done = true;
                break;
            case 2 ... 5: // unhandled
                cerr << "unhandled record type " << type << endl;
                return -1;
        }
    }

    return err;
}
