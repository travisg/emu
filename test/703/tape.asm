; vim: ts=8:sw=8:expandtab:
;
; A program small enough to punch onto paper tape, for exercising the PTB
; bootstrap: it prints a line and halts.
;
; Build with asm703.py -t; see the makefile's ray703-tape target.  Run with
;
;     emu -s ray703-ptb -r roms/703/tape.tape
;
; PTB loads it at word X'100', which is where the machine presets the index
; register, and then goes back to waiting for tape.  It is a loader and
; nothing more -- on real hardware the operator pressed HALT and RESET at this
; point and keyed in a start address, which is a front panel this emulator
; does not have.  Nothing here uses interrupts, since PTB owns level 0 and the
; four words of its interrupt block are far below the load origin.

                ORG     X'100'

START           LDX     MSGLEN
LOOP            LDB     */MSGEND
                DOT     14,14
                IXS     1
                JMP     LOOP
                HLT

MSGLEN          WORD    (MSG-MSGEND)*2
MSG             TEXT    "TAPE LOADED.  \r\n"
MSGEND          EQU     $
