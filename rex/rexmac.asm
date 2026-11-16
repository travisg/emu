; vim: ts=8:sw=8:expandtab:
;
; The macros a REX deck is written with: the file after rexapi.asm.
;
; The 703 branches by skipping.  A skip passes over one word when its
; condition holds, so "go there if it does not" is the skip and a jump,
; and "go there if it does" has to skip a jump around the jump -- and
; which of the two a SAZ and a JMP make is read wrong about as often as
; it is read right.  These say which is meant.  Two words each, except
; the ones marked three, whose condition the machine has no skip for the
; opposite of.
;
; What is tested is what the skips test: ACR for the first four, and the
; last compare (CMW, CMB, CLB) for the rest, which is a signed compare
; of ACR with the operand -- "less" is ACR less than what it was compared
; with.

JNZ             MACRO   TARGET          ; ACR is not zero
                SAZ
                JMP     TARGET
                ENDM
JZ              MACRO   TARGET          ; ACR is zero (three words)
                SAZ
                JMP     $+2
                JMP     TARGET
                ENDM
JMI             MACRO   TARGET          ; ACR is minus
                SAP
                JMP     TARGET
                ENDM
JPL             MACRO   TARGET          ; ACR is zero or more
                SAM
                JMP     TARGET
                ENDM
JNE             MACRO   TARGET          ; not equal
                SEQ
                JMP     TARGET
                ENDM
JEQ             MACRO   TARGET          ; equal
                SNE
                JMP     TARGET
                ENDM
JGE             MACRO   TARGET          ; not less
                SLS
                JMP     TARGET
                ENDM
JLE             MACRO   TARGET          ; not greater
                SGR
                JMP     TARGET
                ENDM
JGT             MACRO   TARGET          ; greater
                SLE
                JMP     TARGET
                ENDM
JLT             MACRO   TARGET          ; less (three words)
                SLS
                JMP     $+2
                JMP     TARGET
                ENDM

; A subroutine that is not in this page: the kernel's vector, from a
; module, or another page of the kernel's.  The page selection is for the
; jump and nothing else, so the two cannot be parted.
CALL            MACRO   ROUTINE
                SMB     ROUTINE
                JSX     ROUTINE
                ENDM

; A text, behind the two byte addresses that say where it is -- its first
; byte and one past its last, which is what a print routine is handed the
; address of.  The label is the descriptor's.  The text need not be an
; even number of characters.
MSG             MACRO   TEXT
                WORD    .T*2,.T*2+LEN(TEXT)
.T              STR     TEXT
                ENDM
