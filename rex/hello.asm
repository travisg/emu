; vim: ts=8:sw=8:expandtab:
;
; HELLO -- a module for the loader to place.
;
; It prints where it landed and goes away: the banner, then its own
; address in decimal -- read from a word the loader relocated, so the
; number is the proof that relocation happened -- then a two-second
; sleep, long enough for a STAT to find it, and K.EXIT.  Assembled from
; word 0 with rexapi.asm in front of it, and every kind of word the
; object format carries is in here on purpose: memory references (W11),
; a word address and the divide's flat operand (W15), the byte addresses
; of the text (B16), a page selection of our own page (SMB, which the
; loader builds from wherever we land) and a reserved run (ILOC).  make
; -C rex reloc-check assembles this absolute at two bases and holds the
; reference loader's placement of the object to those images.
;
; Output is the mailbox protocol of the shell's SHPUTC against our own
; node, which CURT names, through the kernel's vector.

HELLO           SMB     HKMSGA          ; a selection of our own page: nothing
                LDW     HKMSGA          ; needs it, and the loader has to
                JSX     HMSG            ; manufacture it
                LDW     HKSELF          ; where we are, as the loader wrote it
                JSX     HDEC
                LDW     HKCRLA
                JSX     HMSG

; Sleep 120 ticks and go.  The delay and the state go into the node in
; one masked window; K.SWTCH returns when the scheduler next picks us.
                MSK
                SMB     CURT
                LDX     CURT
                LDW     HK120
                STW     *T.DLY
                LDW     HKSLP
                STW     *T.STA
                SMB     K.SWTCH
                JSX     K.SWTCH
                SMB     K.EXIT
                JMP     K.EXIT

; ---------------------------------------------------------------- output
; HPUTC: the character in ACR's low half.  Deposit under MSK, kick the
; printer, then masked-look/SWAI/SWTCH until SERV clears the cell.
HPUTC           SUBR
                AND     HK0FF
                MSK
                SMB     CURT
                LDX     CURT
                STW     *T.MBX
                SMB     K.KICK
                JSX     K.KICK
                UNM
HPCWT           MSK
                SMB     CURT
                LDX     CURT
                LDW     *T.MBX
                SAZ                     ; printed yet?
                JMP     HPCWB
                JMP     HPCWD
HPCWB           LDW     HKWAI           ; no: stand down until SERV says so
                STW     *T.STA
                SMB     K.SWTCH
                JSX     K.SWTCH
                JMP     HPCWT
HPCWD           UNM
                EXIT    HPUTC

; HPUTW: the byte window [ACR, HPWEND), a character at a time.
HPUTW           SUBR
                STW     HPWP
HPWL            LDW     HPWP
                CMW     HPWEND
                SNE
                JMP     HPWDN
                CAX
                CLR                     ; LDB replaces only the low half
                LDB     *0
                JSX     HPUTC
                LDW     HPWP
                ADD     HK1
                STW     HPWP
                JMP     HPWL
HPWDN           EXIT    HPUTW

; HMSG: ACR addresses a descriptor -- two byte addresses, start and end --
; and the text between them is printed.
HMSG            SUBR
                CAX
                LDW     *1
                STW     HPWEND
                LDW     *0
                JSX     HPUTW
                EXIT    HMSG

; HDEC: ACR in decimal on the hardware divide, the shell's way: the digits
; come out backwards, so they go into a buffer from its end and print
; forwards.  The 31-bit dividend is IXR:ACR with the sign duplicated into
; ACR's top bit, which the shift and the copy build.
HDEC            SUBR
                STW     HV
                LDW     HKDBE
                STW     HDP
HDL             LDW     HV
                SRL     15
                CAX
                LDW     HV
                DIV     HKTEN
                STW     HV              ; the quotient
                CXA                     ; the remainder is the digit
                ADD     HKZER
                LDX     HDP
                STB     *0
                LDW     HDP
                SUB     HK1
                STW     HDP
                LDW     HV
                SAZ                     ; nothing left of it?
                JMP     HDL
                LDW     HKDBE
                ADD     HK1
                STW     HPWEND
                LDW     HDP
                ADD     HK1
                JSX     HPUTW
                EXIT    HDEC

; ---------------------------------------------------------------- data
HPWEND          WORD    0               ; HPUTW's end of window
HPWP            WORD    0               ; ...and cursor
HV              WORD    0               ; HDEC's running value...
HDP             WORD    0               ; ...and where its next digit goes
HDB             RES     3               ; six digits, filled backwards
HK1             WORD    1
HK120           WORD    120
HK0FF           WORD    X'00FF'
HKTEN           WORD    10
HKZER           WORD    '0'
HKSLP           WORD    SSLP
HKWAI           WORD    SWAI
HKSELF          WORD    HELLO           ; a word address the loader relocates
HKDBE           WORD    HDB*2+5         ; the last byte of the digit buffer
HKMSGA          WORD    HKMSG           ; the banner descriptor's address
HKMSG           WORD    HKMSGT*2,HKMSGE*2
HKMSGT          TEXT    "HELLO FROM WORD "
HKMSGE          EQU     $
HKCRLA          WORD    HKCRL
HKCRL           WORD    HKCRLT*2,HKCRLE*2
HKCRLT          TEXT    "\r\n"
HKCRLE          EQU     $
                END     HELLO
