	.title	6809 rom

	.area rom (ABS)
	.org 0x8000

rombase:
	; immediate
	adda #1
	addb #1
	addd #1

	; direct page
	adda *1
	addb *1
	addd *1

	; extended addressing
	adda +0x1
	addb +0x1
	addd +0x1

	; indexed
	adda ,x     ; no offset
	adda 1,x    ; 5 bit offset
	adda 100,x  ; 8 bit offset
	adda 1024,x ; 16 bit offset
	adda a,x    ; register offsets
	adda b,x
	adda d,x
	adda ,x+    ; auto increment 1
	adda ,x++   ; auto increment 2
	adda ,-x    ; auto decrement 1
	adda ,--x   ; auto decrement 2
	adda 1,pc    ; 8 bit offset from pc
	adda 1024,pc ; 16 bit offset from pc

	; indirect indexed
	adda [0x1234] ; absolute indirect
	adda [,x]     ; no offset
	adda [1,x]    ; 5 bit offset
	adda [100,x]  ; 8 bit offset
	adda [1024,x] ; 16 bit offset
	adda [a,x]    ; register offsets
	adda [b,x]
	adda [d,x]
	adda [,x++]   ; auto increment 2
	adda [,--x]   ; auto decrement 2
	adda [1,pc]    ; 8 bit offset from pc
	adda [1024,pc] ; 16 bit offset from pc

	bra	.

unhandled_vector:
	bra	.

	.area vectab (ABS)
	.org 0xfff0

	.word unhandled_vector	; reserved
	.word unhandled_vector	; swi3
	.word unhandled_vector	; swi2
	.word unhandled_vector	; firq
	.word unhandled_vector	; irq
	.word unhandled_vector	; swi
	.word unhandled_vector	; nmi
	.word rombase			; reset


