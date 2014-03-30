; vim: ts=4 sw=4 expandtab:
    .title  6809 rom

stacktop = 0x8000

; uart registers
uart_base   = 0x8000
uart_rbr    = uart_base + 0 ; receiver buffer
uart_thr    = uart_base + 0 ; transmitter hold
uart_ier    = uart_base + 1 ; interrupt enable
uart_iir    = uart_base + 2 ; interrupt ident
uart_fcr    = uart_base + 2 ; fifo control
uart_lcr    = uart_base + 3 ; line control
uart_mcr    = uart_base + 4 ; modem control
uart_lsr    = uart_base + 5 ; line status
uart_msr    = uart_base + 6 ; modem status
uart_scr    = uart_base + 7 ; scratch
uart_dll    = uart_base + 0 ; divisor latch low
uart_dlm    = uart_base + 1 ; divisor latch high

; start of rom
    .area rom (ABS)
    .org 0xc000

rombase:
    lds     #stacktop

; configure the uart
    bsr     uart_config

romloop:
    ldx     #0x0
    bsr     memtest_bank
    ldx     #0x4000
    bsr     memtest_bank
    ldx     #0x8000
    bsr     memtest_bank
    ldx     #0x8800
    bsr     memtest_bank
    ldx     #0x9000
    bsr     memtest_bank
    ldx     #0x9800
    bsr     memtest_bank
    ldx     #0xa000
    bsr     memtest_bank
    ldx     #0xa800
    bsr     memtest_bank
    ldx     #0xb000
    bsr     memtest_bank
    ldx     #0xb800
    bsr     memtest_bank

    ldx     #0xc000
    bsr     memtest_bank
    ldx     #0xd000
    bsr     memtest_bank
    ldx     #0xe000
    bsr     memtest_bank
    ldx     #0xf000
    bsr     memtest_bank

    bra     romloop

    ; test a bank of memory, address passed in X
memtest_bank:
    ; write value out and read it back
    lda     #0x1

.memtest_bank_loop:
    sta     ,x
    ldb     ,x
    lsla
    bne     .memtest_bank_loop

    rts

uart_config:
    ; config for n81
    lda     #0x13 ; n81, DLAB
    sta     uart_lcr

    ; divisor bits active, div 2 (57600 at 1.8Mhz)
    lda     #0x2
    sta     uart_dll
    lda     #0
    sta     uart_dlm

    ; clear divisor bits
    lda     #0x03; n81
    sta     uart_lcr

    ; disable fifo
    lda     #0x0
    sta     uart_fcr

    ; disable interrupts
    sta     uart_ier

    rts

uartwrite:
    ldb     uart_lcr
    andb    #0x20    ; test if transmitter hold is empty
    bne     uartwrite
    sta     uart_thr
    rts

unhandled_vector:
    bra     .

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


