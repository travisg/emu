# Emu: Terminal-driven Vintage System Emulator

A terminal-driven emulator for several vintage computer systems, featuring a lightweight interactive console loop.

## Supported Systems

- **Motorola 6809**: A powerful 8-bit (with 16-bit features) microprocessor.
- **MITS Altair 680**: A compact 6800-based computer.
- **Kaypro II**: A Z80-based CP/M machine.

## Prerequisites

To build and run the emulator, you will need:
- `clang` / `clang++`
- `make`
- `objdump` (Linux) or `otool` (Darwin)

## Building the Project

Build the emulator from the repository root:

```bash
make
```

This will produce the following outputs in the `build-emu/` directory:
- `emu`: The main emulator binary.
- `emu.lst`: A disassembly listing of the emulator itself.

To clean the build artifacts:

```bash
make clean
```

Or for a full cleanup:

```bash
make spotless
```

## Running the Emulator

### Basic Usage

Run the default system (currently 6809):

```bash
./build-emu/emu
```

### Command Line Options

Show the help message and a list of supported systems:

```bash
./build-emu/emu -h
```

Run a specific system with a custom ROM:

```bash
# 6809 with BASIC
./build-emu/emu -s 6809 -r test/BASIC.HEX

# Altair 680 with mits680b ROM
./build-emu/emu -s altair680 -r mits680b.bin

# Kaypro II with system ROM
./build-emu/emu -s kaypro -r rom/kaypro/kayproii_u47.bin
```

### Console Interaction

The emulator runs in a raw terminal mode. 
- Use **`Ctrl-D`** to exit the interactive console loop and shut down the emulator cleanly.

## Project Structure

- `cpu/`: CPU core implementations (6800, 6809, Z80).
- `dev/`: Emulated devices (Memory, UARTs, etc.).
- `system/`: Concrete system implementations and the system factory.
- `console.cpp`: Terminal handling and interactive loop.
- `libihex/`: Submodule for Intel HEX file support.
- `test/`: ROMs, test scripts, and assembly code for verification.
