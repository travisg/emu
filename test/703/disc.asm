; vim: ts=8:sw=8:expandtab:
;
; Raytheon 703 disc exerciser.
;
; Drives the 74601 fixed-head disc (706 User's Manual section 5-9) through a
; write, a read back and a verify, and reports DISC TEST PASS or DISC TEST
; FAIL on the teletype before halting.  The end-to-end test greps for the
; verdict and the clean halt; there is no keyboard input at all.
;
; The span is chosen to exercise the controller's one interesting rule: 94
; words starting at track 2, sector 127 run off the end of the track, and
; 5-9.4 says the transfer carries into sector 0 of track 3 by itself.
;
; Two interrupt levels are live, which makes this the first guest in the tree
; with more than one.  The teletype completes on level 0 exactly as the demo's
; does; the disc completes on level 1, and its service routine does nothing
; but collect the status word and raise a flag for the main line -- the disc
; is a DMA device, so by the time it interrupts the data has already moved
; and there is nothing to feed it (5-9.5.4/5).  The main line's waits are the
; period idiom: a load-test-jump spin on a cell the service routine owns,
; interrupts unmasked.
;
; Everything lives in word page 0 -- the demo already exercises the high-page
; addressing dance, and this listing is about the disc.

; ---------------------------------------------------------------- levels 0-1
                ORG     0
                JMP     START           ; word 0: clobbered by the PCR save
                WORD    SERV            ; level 0 linkage address
                WORD    0               ; level 0 machine status save
                WORD    0
                WORD    0               ; word 4: level 1 PCR save
                WORD    DSERV           ; level 1 linkage address
                WORD    0               ; level 1 machine status save
                WORD    0

; ---------------------------------------------------------------- start up
                ORG     X'40'

; Print the banner the way the demo does: hand SEND the first character under
; MSK -- SEND is not re-entrant, and the completion interrupt from its own DOT
; could otherwise arrive before it returned -- and let level 0 do the rest.
START           MSK
                ENB     0
                ENB     1
                LDW     BANADR
                STW     OUTP
                LDW     BANEND
                STW     OUTE
                JSX     SEND
                UNM
WBAN            LDW     OUTP            ; let the banner drain before the
                SAZ                     ; verdict machinery reuses the printer
                JMP     WBAN

; Fill the source buffer: word i holds i XOR X'A5C3', a pattern with no two
; words alike and both byte halves busy.
                LDX     N94M1
FILL            CXA                     ; A <- i
                ORE     PATK
                STW     *BUF1
                DXS     1
                JMP     FILL

; ---------------------------------------------------------------- write
; The 5-9.5 command sequence: memory address, track and sector, then unit,
; count and go.  DBUSY is raised before the starting DOT so the completion
; cannot race the wait loop; the DOTs themselves need no masking, because the
; only interrupts that can land between them belong to the idle teletype and
; touch nothing of the disc's.
                LDW     ONE
                STW     DBUSY
                LDW     BUF1AW
                DOT     1,1             ; set memory address (5-9.5.2)
                LDW     TRKSEC
                DOT     1,2             ; set track and sector (5-9.5.3)
                LDW     N94
                DOT     1,4             ; unit 0, 94 words, write (5-9.5.4)
WWRITE          LDW     DBUSY
                SAZ
                JMP     WWRITE
                LDW     DSTAT           ; all-zero status is a clean op
                SAZ                     ; (Table 5-29)
                JMP     FAIL

; ---------------------------------------------------------------- read back
                LDW     ONE
                STW     DBUSY
                LDW     BUF2AW
                DOT     1,1             ; set memory address (5-9.5.2)
                LDW     TRKSEC
                DOT     1,2             ; set track and sector (5-9.5.3)
                LDW     N94
                DOT     1,6             ; unit 0, 94 words, read (5-9.5.5)
WREAD           LDW     DBUSY
                SAZ
                JMP     WREAD
                LDW     DSTAT
                SAZ
                JMP     FAIL

; The read landed next to the source, so compare the two buffers word for
; word.  An interrupt between the CMW and its skip is harmless: the machine
; status the hardware saves and INR restores carries the comparison
; indicators (3-3).
                LDX     N94M1
COMP            LDW     *BUF1
                CMW     *BUF2
                SEQ
                JMP     FAIL
                DXS     1
                JMP     COMP

; ---------------------------------------------------------------- verify
; 5-9.5.6: the same operation as the read except no data enters core.  The
; track and sector went stale when the read advanced them, so they are
; reissued; no memory address is set, which is the point -- a verify that
; wrote anything would be caught by the recomparison below.
                LDW     ONE
                STW     DBUSY
                LDW     TRKSEC
                DOT     1,2             ; set track and sector (5-9.5.3)
                LDW     N94
                DOT     1,7             ; unit 0, 94 words, verify (5-9.5.6)
WVERIFY         LDW     DBUSY
                SAZ
                JMP     WVERIFY
                LDW     DSTAT
                SAZ
                JMP     FAIL

; Recompare the read buffer against the recomputed pattern: proves the verify
; moved nothing, and checks the data against its definition rather than
; against another buffer the same bug could have corrupted.
                LDX     N94M1
VCOMP           CXA
                ORE     PATK
                CMW     *BUF2
                SEQ
                JMP     FAIL
                DXS     1
                JMP     VCOMP

; ---------------------------------------------------------------- verdict
PASS            LDW     PASADR
                STW     OUTP
                LDW     PASEND
                JMP     REPORT
FAIL            LDW     FAILADR
                STW     OUTP
                LDW     FAILEND
REPORT          STW     OUTE
                MSK                     ; SEND is not re-entrant; see START
                JSX     SEND
                UNM
DRAIN           LDW     OUTP            ; let the printer finish the verdict
                SAZ
                JMP     DRAIN
                HLT

; ---------------------------------------------------------------- output
; The demo's transmitter, unchanged: hand the next character to the printer,
; or mark the transmission idle if there is none left.
SEND            STX     SRET
                LDW     OUTP
                CMW     OUTE
                SNE
                JMP     SDONE
                CAX
                ADD     ONE
                STW     OUTP            ; advance before the DOT: the
                                        ; completion can arrive on the very
                                        ; next instruction
                LDB     *0
                DOT     14,14           ; teletype, write a character
                LDX     SRET
                JMP     *0
SDONE           CLR
                STW     OUTP
                LDX     SRET
                JMP     *0

; ------------------------------------------------------- level 0 service
; Only the printer can interrupt on level 0 -- the keyboard is never armed --
; so the service routine is the demo's with the receive half gone.
SERV            STW     SAVEA
                STX     SAVEX
                JSX     SEND
                LDW     SAVEA
                LDX     SAVEX
                INR     0

; ------------------------------------------------------- level 1 service
; The disc's completion.  Collect unit 0's status -- the collect is the whole
; conversation, the data moved under DMA before the interrupt was raised --
; and flag the main line.  A clean operation reads all zero, completion
; having set bits 0 and 1 false (5-9.5.5).
DSERV           STW     DSAVEA
                STX     DSAVEX
                DIN     1,0             ; status of unit 0 (5-9.7)
                STW     DSTAT
                CLR
                STW     DBUSY
                LDW     DSAVEA
                LDX     DSAVEX
                INR     1

; ---------------------------------------------------------------- data
OUTP            WORD    0               ; byte address of the next character;
OUTE            WORD    0               ; zero means the printer is idle
SRET            WORD    0
SAVEA           WORD    0               ; level 0's register saves
SAVEX           WORD    0
DSAVEA          WORD    0               ; level 1's register saves
DSAVEX          WORD    0
DBUSY           WORD    0               ; set before each command, cleared
                                        ; by DSERV at its completion -- the
                                        ; wait loops spin on it with SAZ,
                                        ; which skips on *zero*, so the flag
                                        ; reads as busy, not as done
DSTAT           WORD    0               ; the status DSERV collected
ONE             WORD    1
PATK            WORD    X'A5C3'
N94M1           WORD    93
N94             WORD    94              ; unit 0 in bits 0-1, count in 2-15
                                        ; (5-9.5.4) -- bit 0 is the MSB, so a
                                        ; plain 94 is unit 0
TRKSEC          WORD    X'087F'         ; track 2 in bits 0-5, sector 127 in
                                        ; bits 7-15 (5-9.5.3)
BUF1AW          WORD    BUF1            ; DOT 1,1 takes a word address
BUF2AW          WORD    BUF2
BANADR          WORD    BANNER*2        ; the teletype wants byte addresses
BANEND          WORD    BANSTOP*2
PASADR          WORD    PASMSG*2
PASEND          WORD    PASSTOP*2
FAILADR         WORD    FAILMSG*2
FAILEND         WORD    FAILSTOP*2

BANNER          TEXT    "703 DISC EXERCISER\r\n"
BANSTOP         EQU     $
PASMSG          TEXT    "DISC TEST PASS\r\n"
PASSTOP         EQU     $
FAILMSG         TEXT    "DISC TEST FAIL\r\n"
FAILSTOP        EQU     $

BUF1            RES     94              ; the pattern written to the disc
BUF2            RES     94              ; what came back
