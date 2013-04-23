; vim: ts=4 sw=4 expandtab:
    .title  6809 rom

puthere = 0x1234
stacktop = 0x8000

; uart registers
uart_status  = 0xa000 ; read
uart_control = 0xa000 ; write
uart_data    = 0xa001 ; read/write

; start of rom
    .area rom (ABS)
    .org 0xc000

rombase:
    lds     #stacktop


    lda     #'a
    bsr     uartwrite
    lda     #'b
    bsr     uartwrite
    lda     #'\n
    bsr     uartwrite
    bra     .

uartwrite:
    sta     uart_data
    rts

asmtest:
    lda     #1
    lda     somedata
    ldd     somedata+1

    ldx     somedata+1

    sta     puthere
    std     puthere

    abx

    ; immediate
    adda    #1
    addb    #1
    addd    #1

    ; direct page
    adda    *1
    addb    *1
    addd    *1

    ; extended addressing
    adda    +0x1
    addb    +0x1
    addd    +0x1

    ; indexed
    adda    1,x    ; 5 bit offset
    adda    -11,x    ; 5 bit offset
    adda    ,x     ; no offset
    adda    100,x  ; 8 bit offset
    adda    -100,x  ; 8 bit offset
    adda    1024,x ; 16 bit offset
    adda    -1024,x ; 16 bit offset
    adda    a,x    ; register offsets
    adda    b,x
    adda    d,x
    adda    ,x+    ; auto increment 1
    adda    ,x++   ; auto increment 2
    adda    ,-x    ; auto decrement 1
    adda    ,--x   ; auto decrement 2
    adda    1,pc    ; 8 bit offset from pc
    adda    1024,pc ; 16 bit offset from pc

    ; indirect indexed
    adda    [0x1234] ; absolute indirect
    adda    [,x]     ; no offset
    adda    [1,x]    ; 5 bit offset
    adda    [100,x]  ; 8 bit offset
    adda    [1024,x] ; 16 bit offset
    adda    [a,x]    ; register offsets
    adda    [b,x]
    adda    [d,x]
    adda    [,x++]   ; auto increment 2
    adda    [,--x]   ; auto decrement 2
    adda    [1,pc]    ; 8 bit offset from pc
    adda    [1024,pc] ; 16 bit offset from pc

    bra     .

unhandled_vector:
    bra     .

somedata:
    .byte   0x99
    .word   0x1024

    .area   vectab (ABS)
    .org    0xfff0

vectab:
    .word   unhandled_vector  ; reserved
    .word   unhandled_vector  ; swi3
    .word   unhandled_vector  ; swi2
    .word   unhandled_vector  ; firq
    .word   unhandled_vector  ; irq
    .word   unhandled_vector  ; swi
    .word   unhandled_vector  ; nmi
    .word   rombase           ; reset


