; Conversational FORTRAN, the compiler for a 4K machine, DN 393295 rev B
;
; Written by tools/dis703.py. A line marked "not run" is an
; instruction only by being reached from one that ran.

A0040    EQU     X'0040'
A0044    EQU     X'0044'
A0046    EQU     X'0046'
A005A    EQU     X'005A'

         ORG     X'0080'
; Where the compiler starts: PTIOS's loader enters it at X'80', the tape's last record being 8080.
; It runs over PTIOS's resident words, below X'80', and compiles into memory from X'D00' up.
J0080    CLR                      ; 0080 0100
         SML     2                ; 0081 0082
         STW     D0B35            ; 0082 7335
         LDX     D0225            ; 0083 9225
         STX     D0209            ; 0084 6209
         LDW     A005A            ; 0085 805A
         SUB     D01F6            ; 0086 B1F6
         STW     D0208            ; 0087 7208
         STW     D0222            ; 0088 7222
S0089    LDW     D0147            ; 0089 8147
         SML     2                ; 008A 0082
         STW     D0AD7            ; 008B 72D7
         JSX     A0044            ; 008C 2044
         DATA    X'8AD7'          ; 008D 8AD7  argument 1 of 008C; '\nW'
E008E    SML     2                ; 008E 0082
         LDW     D0B46            ; 008F 8346
         SML     2                ; 0090 0082
         STW     D0A0C            ; 0091 720C
         CLR                      ; 0092 0100
         SML     2                ; 0093 0082
         STW     D0A10            ; 0094 7210
         STW     D078D            ; 0095 778D
         STW     D020A            ; 0096 720A
         LLB     X'29'            ; 0097 0629
         SML     2                ; 0098 0082
         STW     D09D3            ; 0099 71D3
         LDW     D0767            ; 009A 8767
         STW     D0224            ; 009B 7224
         STW     D0302            ; 009C 7302
         STW     D0793            ; 009D 7793
         SML     2                ; 009E 0082
         STW     D0B38            ; 009F 7338
         STW     D020B            ; 00A0 720B
         LDW     D0223            ; 00A1 8223
         STW     D02A7            ; 00A2 72A7
         LDW     D03CF            ; 00A3 83CF
         STW     D03CE            ; 00A4 73CE
         JSX     A0046            ; 00A5 2046
         DATA    X'8AD7'          ; 00A6 8AD7  argument 1 of 00A5; '\nW'
; The command, read off the teletype through PTIOS's X'44' and waited for at its X'46'. Its first letter is
; I (start again at X'80'), P (PREP: statements typed at the keyboard, each checked as it comes, an error
; printed with an arrow under its column; at END it pauses at 0B07, punches the statements it accepted and pauses
; again), B (punches what has been compiled as an absolute tape, records from X'CFD' and from X'1EFD', where
; the symbols are), E (to PTIOS's loader, through X'40') or G (compile from unit 2). L as the second letter, or
; the third after C, lists the source as it is read; C reads on past END. The letters were seen in the runs'
; sessions, but C, tried once by hand (the reader ran out of tape); what P and B punch was read off tapes torn
; off in sessions by hand.
E00A7    LDB     D020F            ; 00A7 541E
         CLB     X'C9'            ; 00A8 07C9
         SNE                      ; 00A9 0870
         JMP     J0080            ; 00AA 1080
         CLB     X'D0'            ; 00AB 07D0
         SNE                      ; 00AC 0870
         JMP     J00C1            ; 00AD 10C1
         CLB     X'C2'            ; 00AE 07C2
         SNE                      ; 00AF 0870
         JMP     J00C8            ; 00B0 10C8
         CLB     X'C5'            ; 00B1 07C5
         SNE                      ; 00B2 0870
         JMP     A0040            ; 00B3 1040
         CLB     X'C7'            ; 00B4 07C7
         SEQ                      ; 00B5 0860
         JMP     S0089            ; 00B6 1089  not run
J00B7    LDB     D020F+1          ; 00B7 541F
         CLB     X'C3'            ; 00B8 07C3
         SEQ                      ; 00B9 0860
         JMP     J00BD            ; 00BA 10BD
         STW     D0793            ; 00BB 7793  not run
         LDB     D0210            ; 00BC 5420  not run
J00BD    CLB     X'CC'            ; 00BD 07CC
         SNE                      ; 00BE 0870
         STW     D0224            ; 00BF 7224
         JMP     J00FD            ; 00C0 10FD
J00C1    LDW     D0767            ; 00C1 8767
         STW     D020A            ; 00C2 720A
         CLR                      ; 00C3 0100
         LLB     X'7B'            ; 00C4 067B
         SML     2                ; 00C5 0082
         STW     D09D3            ; 00C6 71D3
         JMP     J00B7            ; 00C7 10B7
J00C8    JSX     S00DA            ; 00C8 20DA
J00C9    LDW     D0209            ; 00C9 8209
         STW     D00CE            ; 00CA 70CE
         SML     2                ; 00CB 0082
         JSX     S0B4B            ; 00CC 234B
         DATA    X'0CFD'          ; 00CD 0CFD  argument 1 of 00CC
D00CE    DATA    X'8000'          ; 00CE 8000  argument 2 of 00CC; LDW 0; written by 00CA
         LDW     D0208            ; 00CF 8208
         STW     D00D6            ; 00D0 70D6
         LDW     D0222            ; 00D1 8222
         ADD     D01F5            ; 00D2 A1F5
         STW     D00D7            ; 00D3 70D7
         SML     2                ; 00D4 0082
         JSX     S0B4B            ; 00D5 234B
D00D6    DATA    X'0000'          ; 00D6 0000  argument 1 of 00D5; written by 00D0
D00D7    DATA    X'0000'          ; 00D7 0000  argument 2 of 00D5; written by 00D3
         JSX     S00DA            ; 00D8 20DA
J00D9    JMP     S0089            ; 00D9 1089
S00DA    LDW     D020F            ; 00DA 820F
         CLB     X'D4'            ; 00DB 07D4
         SEQ                      ; 00DC 0860
         JMP     *0               ; 00DD 1800
         SML     2                ; 00DE 0082  not run
         JMP     J09EC            ; 00DF 11EC  not run
J00E0    LDW     D0210            ; 00E0 8210
         SAZ                      ; 00E1 0800
         JMP     J00E4            ; 00E2 10E4
         JMP     J00EF            ; 00E3 10EF
J00E4    STW     D021F            ; 00E4 721F
         LDW     D0124            ; 00E5 8124
         STW     D021E            ; 00E6 721E
         JSX     S014B            ; 00E7 214B
         JMP     J0122            ; 00E8 1122
         CAX                      ; 00E9 0130
         LDW     *2               ; 00EA 8802
         SAM                      ; 00EB 0820
         JMP     J022F            ; 00EC 122F
J00ED    LDW     D0211            ; 00ED 8211
         STW     *2               ; 00EE 7802
J00EF    JSX     S01BA            ; 00EF 21BA
         CLB     X'20'            ; 00F0 0720
         SEQ                      ; 00F1 0860
         JMP     J023F            ; 00F2 123F  not run
         LDX     D07B8            ; 00F3 97B8
         LDB     *0               ; 00F4 5800
         STW     D0205            ; 00F5 7205
         LDW     D0793            ; 00F6 8793
         SAM                      ; 00F7 0820
         JMP     J00FF            ; 00F8 10FF  not run
         LDW     D020A            ; 00F9 820A
         SML     2                ; 00FA 0082
         SAP                      ; 00FB 0810
         JSX     X'02A8'          ; 00FC 22A8  lands at 0AA8 = S0AA8 under the selection in force
J00FD    SML     2                ; 00FD 0082
         JSX     S095A            ; 00FE 215A
J00FF    LDW     D01FE            ; 00FF 81FE
         STW     D0776            ; 0100 7776
         CLR                      ; 0101 0100
         STW     D0226            ; 0102 7226
         STW     D0303            ; 0103 7303
         STW     D0346            ; 0104 7346
         STW     D02A5            ; 0105 72A5
         STW     D0215            ; 0106 7215
         STW     D0535            ; 0107 7535
         STW     D0210            ; 0108 7210
         STW     D064D            ; 0109 764D
         LDX     D0209            ; 010A 9209
         STW     *0               ; 010B 7800
         STX     D0203            ; 010C 6203
         LDW     D0208            ; 010D 8208
         STW     D0211            ; 010E 7211
J010F    JSX     S07AA            ; 010F 27AA
         JSX     S071B            ; 0110 271B
J0111    JMP     J0118            ; 0111 1118
J0112    LDW     D0148            ; 0112 8148
         STW     D054F            ; 0113 754F
         LDW     D01F6            ; 0114 81F6
         STW     D064C            ; 0115 764C
         STX     D0212            ; 0116 6212
         JMP     J0568            ; 0117 1568
J0118    LDW     D0210            ; 0118 8210
         SLL     3                ; 0119 0A13
         ADD     D0210            ; 011A A210
         ADD     D0210            ; 011B A210
         ADD     D0459            ; 011C A459
         STW     D0210            ; 011D 7210
         SAM                      ; 011E 0820
         JMP     J010F            ; 011F 110F
J0120    JSX     S0241            ; 0120 2241
         DATA    X'D3CC'          ; 0121 D3CC  argument 1 of 0120; 'SL'
J0122    LDX     D0209            ; 0122 9209
         IXS     3                ; 0123 0403
D0124    DATA    X'3002'          ; 0124 3002  STB 2; read by 00E5 0127 0758 075F
         STX     D0209            ; 0125 6209
         DXS     3                ; 0126 0503
         LDW     D0124            ; 0127 8124
         STW     *0               ; 0128 7800
         LDW     D0210            ; 0129 8210
         STW     *1               ; 012A 7801
         JMP     J00ED            ; 012B 10ED
J012C    JSX     S01BA            ; 012C 21BA
         CLB     X'20'            ; 012D 0720
         SEQ                      ; 012E 0860
         JMP     J023F            ; 012F 123F  not run
         LDX     D02A7            ; 0130 92A7
         LDW     *0               ; 0131 8800
         CMW     D0210            ; 0132 F210
         SEQ                      ; 0133 0860
         JMP     J00E0            ; 0134 10E0
         JSX     S0774            ; 0135 2774
D0136    DATA    X'0834'          ; 0136 0834  argument 1 of 0135; read by 0349
         LDX     D02A7            ; 0137 92A7
         LDW     *1               ; 0138 8801
         IXS     2                ; 0139 0402
D013A    DATA    X'0004'          ; 013A 0004  read by 013D 02FE
         STX     D02A7            ; 013B 62A7
         STW     D02A9            ; 013C 72A9
         ADD     D013A            ; 013D A13A
         JSX     S0779            ; 013E 2779
         JSX     S0519            ; 013F 2519
         JMP     J012C            ; 0140 112C  not run
J0141    LDX     D02A9            ; 0141 92A9
         LDW     D0208            ; 0142 8208
         JMP     J0337            ; 0143 1337
S0144    LDW     D0203            ; 0144 8203
         STW     D0209            ; 0145 7209
         JMP     J00FD            ; 0146 10FD
D0147    DATA    X'020F'          ; 0147 020F  read by 0089
D0148    DATA    X'012C'          ; 0148 012C  read by 0112
D0149    DATA    X'0000'          ; 0149 0000  written by 014D; read by 0161 0170
S014A    CLR                      ; 014A 0100
S014B    STW     D0218            ; 014B 7218
         LDW     D0207            ; 014C 8207
S014D    STX     D0149            ; 014D 6149
         STW     D0217            ; 014E 7217
         LDW     D01F9            ; 014F 81F9
         LDX     D0209            ; 0150 9209
         STW     *0               ; 0151 7800
         STW     *1               ; 0152 7801
         CLR                      ; 0153 0100
J0154    ADD     D0217            ; 0154 A217
         STW     D0217            ; 0155 7217
         CAX                      ; 0156 0130
         LDW     *1               ; 0157 8801
         CMW     D021F            ; 0158 F21F
         SNE                      ; 0159 0870
         JMP     J0163            ; 015A 1163
J015B    LDW     *0               ; 015B 8800
J015C    SAP                      ; 015C 0810
         JMP     J0154            ; 015D 1154
         SRL     12               ; 015E 0A0C
         SAZ                      ; 015F 0800
         JMP     J0154            ; 0160 1154
         LDX     D0149            ; 0161 9149
         JSX     *0               ; 0162 2800
J0163    LDW     *0               ; 0163 8800
         CMB     D021E+1          ; 0164 443D
         SEQ                      ; 0165 0860
         JMP     J015C            ; 0166 115C
         LDW     D0218            ; 0167 8218
         SAZ                      ; 0168 0800
         JMP     J016E            ; 0169 116E
         LDW     *2               ; 016A 8802
         CMW     D0220            ; 016B F220
         SEQ                      ; 016C 0860
         JMP     J015B            ; 016D 115B  not run
J016E    CXA                      ; 016E 0140
         AND     D0200            ; 016F E200
         LDX     D0149            ; 0170 9149
         JSX     *1               ; 0171 2801
S0172    STX     D01B9            ; 0172 61B9
         CLR                      ; 0173 0100
         STW     D021C            ; 0174 721C
         LDW     D0205            ; 0175 8205
         CLB     X'C0'            ; 0176 07C0
         SGR                      ; 0177 0880
         JMP     J0227            ; 0178 1227
         CLB     X'C8'            ; 0179 07C8
         SGR                      ; 017A 0880
         JMP     J0182            ; 017B 1182
         CLB     X'CE'            ; 017C 07CE
         SGR                      ; 017D 0880
         JMP     J0183            ; 017E 1183
         CLB     X'DA'            ; 017F 07DA
         SLE                      ; 0180 0890
         JMP     J0227            ; 0181 1227
J0182    ORI     D0589            ; 0182 C589
J0183    STW     D021E            ; 0183 721E
         LDW     D0201            ; 0184 8201
         STW     D021F            ; 0185 721F
         STW     D0220            ; 0186 7220
J0187    LDW     D07B8            ; 0187 87B8
         STW     D01C1            ; 0188 71C1
         LDW     D0212            ; 0189 8212
         SAZ                      ; 018A 0800
         JMP     J019D            ; 018B 119D
         JSX     S07AA            ; 018C 27AA
J018D    CLB     X'C0'            ; 018D 07C0
         SGR                      ; 018E 0880
         JMP     J019F            ; 018F 119F
         CLB     X'DA'            ; 0190 07DA
         SLE                      ; 0191 0890
         JMP     J019F            ; 0192 119F
J0193    LDW     D021C            ; 0193 821C
         CLB     4                ; 0194 0704
         SNE                      ; 0195 0870
         JMP     J0187            ; 0196 1187
         ADD     D01F5            ; 0197 A1F5
         STW     D021C            ; 0198 721C
         CAX                      ; 0199 0130
         LDW     D0205            ; 019A 8205
         STB     *X'43D'          ; 019B 3C3D
         JMP     J0187            ; 019C 1187
J019D    JSX     S07B3            ; 019D 27B3
         JMP     J018D            ; 019E 118D
J019F    CLB     X'BD'            ; 019F 07BD
         SNE                      ; 01A0 0870
         JMP     J01A7            ; 01A1 11A7
         LDW     D0212            ; 01A2 8212
         SAZ                      ; 01A3 0800
         JMP     J01AD            ; 01A4 11AD
J01A5    JSX     S071B            ; 01A5 271B
         JMP     J0193            ; 01A6 1193  not run
J01A7    LDW     D0212            ; 01A7 8212
         STW     D0226            ; 01A8 7226
         SAZ                      ; 01A9 0800
         JSX     S01C3            ; 01AA 21C3
J01AB    LDX     D01B9            ; 01AB 91B9
         JMP     J01B6            ; 01AC 11B6
J01AD    CLR                      ; 01AD 0100
         STW     D0218            ; 01AE 7218
         LDW     D020E            ; 01AF 820E
         JSX     S014D            ; 01B0 214D
         JMP     J01A5            ; 01B1 11A5
         CAX                      ; 01B2 0130
         LDX     *3               ; 01B3 9803
         IXS     0                ; 01B4 0400
         JMP     J07F7            ; 01B5 17F7
J01B6    CLR                      ; 01B6 0100
         STW     D0212            ; 01B7 7212
         JMP     S01BA            ; 01B8 11BA
D01B9    DATA    X'0000'          ; 01B9 0000  written by 0172 01BA; read by 01AB 01BF
S01BA    STX     D01B9            ; 01BA 61B9
         LDW     D0205            ; 01BB 8205
         CLB     X'A0'            ; 01BC 07A0
         SNE                      ; 01BD 0870
         JSX     S07AA            ; 01BE 27AA
         LDX     D01B9            ; 01BF 91B9
         JSX     *0               ; 01C0 2800
D01C1    DATA    X'0000'          ; 01C1 0000  written by 0188 07AC; read by 0A27
D01C2    DATA    X'0000'          ; 01C2 0000  written by 01C3; read by 01F2
S01C3    STX     D01C2            ; 01C3 61C2
         LDW     D020B            ; 01C4 820B
         SAM                      ; 01C5 0820
         JMP     *0               ; 01C6 1800
         JSX     S0519            ; 01C7 2519
J01C8    JMP     J01F1            ; 01C8 11F1
J01C9    LDX     D0302            ; 01C9 9302
         LDW     D0208            ; 01CA 8208
         SML     2                ; 01CB 0082
         JMP     J0B0D            ; 01CC 130D
S01CD    LDW     D0207            ; 01CD 8207
         JMP     J01D0            ; 01CE 11D0
J01CF    ADD     D0217            ; 01CF A217
J01D0    STW     D0217            ; 01D0 7217
         CAX                      ; 01D1 0130
         LDW     *0               ; 01D2 8800
         AND     D01FF            ; 01D3 E1FF
         ORE     D01FF            ; 01D4 D1FF
         SAZ                      ; 01D5 0800
         JMP     J01EB            ; 01D6 11EB
         LDW     *3               ; 01D7 8803
         SAZ                      ; 01D8 0800
         JMP     J01EB            ; 01D9 11EB
         STX     D020D            ; 01DA 620D
         LDW     D0209            ; 01DB 8209
         STW     *3               ; 01DC 7803
         JSX     S0413            ; 01DD 2413
         LDX     D0209            ; 01DE 9209
         ORI     D0767            ; 01DF C767
         STW     *0               ; 01E0 7800
         AND     D0200            ; 01E1 E200
         ADD     D0209            ; 01E2 A209
         STW     D0209            ; 01E3 7209
         CMW     D0208            ; 01E4 F208
         CAX                      ; 01E5 0130
         CLR                      ; 01E6 0100
         SLE                      ; 01E7 0890
         JMP     J0786            ; 01E8 1786  not run
         STW     *0               ; 01E9 7800
         LDX     D020D            ; 01EA 920D
J01EB    LDW     *0               ; 01EB 8800
         SAP                      ; 01EC 0810
         JMP     J01CF            ; 01ED 11CF
         SRL     12               ; 01EE 0A0C
         SAZ                      ; 01EF 0800
         JMP     J01CF            ; 01F0 11CF
J01F1    STW     D020B            ; 01F1 720B
         LDX     D01C2            ; 01F2 91C2
         JSX     *0               ; 01F3 2800
D01F4    DATA    X'0000'          ; 01F4 0000  read by 0442 048F 04E0
D01F5    DATA    X'0001'          ; 01F5 0001  read by 00D2 0197 0250 0258 ...
D01F6    DATA    X'0002'          ; 01F6 0002  read by 0086 0114
D01F7    DATA    X'0003'          ; 01F7 0003  read by 038E
D01F8    DATA    X'0007'          ; 01F8 0007  read by 05B4
D01F9    DATA    X'000F'          ; 01F9 000F  read by 014F 0423 0722 0786
         DATA    X'0024'          ; 01FA 0024
D01FB    DATA    X'00AD'          ; 01FB 00AD  read by 05F3 06F0
D01FC    DATA    X'0200'          ; 01FC 0200  read by 0400
D01FD    DATA    X'0400'          ; 01FD 0400  read by 02E4 03C6 050B 079B
D01FE    DATA    X'0800'          ; 01FE 0800  read by 00FF 04AA 04EF 0687 ...
D01FF    DATA    X'6200'          ; 01FF 6200  STX X'200'; read by 01D3 01D4
D0200    DATA    X'7FFF'          ; 0200 7FFF  STW *X'7FF'; read by 016F 01E1 0420
D0201    DATA    X'A0A0'          ; 0201 A0A0  '  '; read by 0184
D0202    DATA    X'0000'          ; 0202 0000  written by 047F 0484; read by 045E
D0203    DATA    X'0000'          ; 0203 0000  written by 010C; read by 0144 0A1D
D0204    DATA    X'0000'          ; 0204 0000  written by 0688; read by 049B 04B2
D0205    DATA    X'0020'          ; 0205 0020  written by 00F5 07BA; read by 0175 019A 01BB 0244 ...
D0206    DATA    X'0000'          ; 0206 0000  written by 06C7; read by 06D7
D0207    DATA    X'0846'          ; 0207 0846  read by 014C 01CD 07FD
D0208    DATA    X'0000'          ; 0208 0000  written by 0087 0784 0AB9 0AE7; read by 00CF 010D 0142 01CA ...
D0209    DATA    X'0D02'          ; 0209 0D02  written by 0084 0125 0145 01E3 ...; read by 00C9 010A 0122 0150 ...
D020A    DATA    X'0000'          ; 020A 0000  written by 0096 00C2; read by 00F9 0519 077C 07C3 ...
D020B    DATA    X'0000'          ; 020B 0000  written by 00A0 01F1; read by 01C4
D020C    DATA    X'0000'          ; 020C 0000  written by 0421 0429; read by 0425 042D
D020D    DATA    X'0000'          ; 020D 0000  written by 01DA 0393 03D9 03ED ...; read by 01EA 02DF 03C2 03FE ...
D020E    DATA    X'088B'          ; 020E 088B  read by 01AF
D020F    DATA    X'0000'          ; 020F 0000  written by 04EA; read by 00A7 00B7 00DA 04F1
D0210    DATA    X'0000'          ; 0210 0000  written by 0108 011D; read by 00BC 00E0 0118 011A ...
D0211    DATA    X'0000'          ; 0211 0000  written by 010E 035C; read by 00ED 035A
D0212    DATA    X'0000'          ; 0212 0000  written by 0116 01B7; read by 0189 01A2 01A7
D0213    DATA    X'0000'          ; 0213 0000  written by 0682; read by 0440 0447 0473 04A4 ...
D0214    DATA    X'0000'          ; 0214 0000  written by 0685; read by 0448 04BC
D0215    DATA    X'0000'          ; 0215 0000  written by 0106 072C; read by 072A
D0216    DATA    X'0000'          ; 0216 0000  written by 04AB 068B; read by 0454 048C 049E 04A9 ...
D0217    DATA    X'0000'          ; 0217 0000  written by 014E 0155 01D0; read by 0154 01CF
D0218    DATA    X'0000'          ; 0218 0000  written by 014B 01AE; read by 0167
D0219    DATA    X'0000'          ; 0219 0000  written by 07DD 07F2; read by 0471 07D5 07EB
D021A    DATA    X'0000'          ; 021A 0000  written by 0710; read by 0713
D021B    DATA    X'0000'          ; 021B 0000  written by 0251 0259 034C 0424 ...; read by 0257 0263 035F 042A ...
D021C    DATA    X'0000'          ; 021C 0000  written by 0174 0198 034E 0416 ...; read by 0193 0354 0363 041B ...
D021D    DATA    X'0000'          ; 021D 0000  written by 05E5; read by 05EA
D021E    DATA    X'0000'          ; 021E 0000  written by 00E6 0183 026B 05C8 ...; read by 0164 0273 02F6 033B ...
D021F    DATA    X'0000'          ; 021F 0000  written by 00E4 0185 05C6 06C2 ...; read by 0158 02F9 033E 03E4 ...
D0220    DATA    X'0000'          ; 0220 0000  written by 0186 06BA 06D5 06E3 ...; read by 016B 02FB 0340 03E6 ...
D0221    DATA    X'0000'          ; 0221 0000  written by 05A9 05B8 06B2 06D0 ...; read by 03FA 0402 0407 040C ...
D0222    DATA    X'0000'          ; 0222 0000  written by 0088; read by 00D1 0AE2
D0223    DATA    X'0846'          ; 0223 0846  read by 00A1 07FB
D0224    DATA    X'0000'          ; 0224 0000  written by 009B 00BF; read by 0979 09FB
D0225    DATA    X'0D02'          ; 0225 0D02  read by 0083
D0226    DATA    X'0000'          ; 0226 0000  written by 0102 01A8 0485 04B1; read by 065A
; The report of an error the compiler finds in a statement: a JSX with the code behind it. The errors session
; printed ID MR NA CN DD MX from here, and DN LA SL OP RT, which come from elsewhere.
J0227    JSX     S0241            ; 0227 2241
         DATA    X'C9C4'          ; 0228 C9C4  argument 1 of 0227; 'ID'
J0229    JSX     S0241            ; 0229 2241
         DATA    X'CDD2'          ; 022A CDD2  argument 1 of 0229; 'MR'
J022B    JSX     S0241            ; 022B 2241
         DATA    X'CEC1'          ; 022C CEC1  argument 1 of 022B; 'NA'
J022D    JSX     S0241            ; 022D 2241
         DATA    X'C3CE'          ; 022E C3CE  argument 1 of 022D; 'CN'
J022F    JSX     S0241            ; 022F 2241
         DATA    X'C4C4'          ; 0230 C4C4  argument 1 of 022F; 'DD'
J0231    JSX     S0241            ; 0231 2241  not run
         DATA    X'CDCC'          ; 0232 CDCC  argument 1 of 0231; 'ML'
J0233    JSX     S0241            ; 0233 2241  not run
         DATA    X'CEC9'          ; 0234 CEC9  argument 1 of 0233; 'NI'
J0235    JSX     S0241            ; 0235 2241  not run
         DATA    X'C4D3'          ; 0236 C4D3  argument 1 of 0235; 'DS'
J0237    JSX     S0241            ; 0237 2241  not run
         DATA    X'C3CD'          ; 0238 C3CD  argument 1 of 0237; 'CM'
         JSX     S0241            ; 0239 2241  not run
         DATA    X'CCCC'          ; 023A CCCC  argument 1 of 0239; 'LL'
J023B    JSX     S0241            ; 023B 2241
         DATA    X'CDD8'          ; 023C CDD8  argument 1 of 023B; 'MX'
J023D    JSX     S0241            ; 023D 2241  not run
         DATA    X'CDC3'          ; 023E CDC3  argument 1 of 023D; 'MC'
J023F    JSX     S0241            ; 023F 2241  not run
         DATA    X'D4D2'          ; 0240 D4D2  argument 1 of 023F; 'TR'
S0241    SML     2                ; 0241 0082
         JMP     J09F8            ; 0242 11F8
J0243    JSX     S01C3            ; 0243 21C3
J0244    LDW     D0205            ; 0244 8205
         CLB     X'A8'            ; 0245 07A8
         SNE                      ; 0246 0870
         JMP     J024E            ; 0247 124E
         JSX     S0774            ; 0248 2774
         DATA    X'0832'          ; 0249 0832  argument 1 of 0248
         JMP     J0279            ; 024A 1279
         JSX     S07AA            ; 024B 27AA
         JSX     S07AA            ; 024C 27AA
         JMP     J0243            ; 024D 1243
J024E    JSX     S0774            ; 024E 2774
         DATA    X'0836'          ; 024F 0836  argument 1 of 024E
         LDW     D01F5            ; 0250 81F5
         STW     D021B            ; 0251 721B
         LDW     D0208            ; 0252 8208
         STW     D0301            ; 0253 7301
         JSX     S0779            ; 0254 2779
J0255    JSX     S07AA            ; 0255 27AA
         JSX     S074A            ; 0256 274A
         LDW     D021B            ; 0257 821B
         ADD     D01F5            ; 0258 A1F5
         STW     D021B            ; 0259 721B
         LDW     D0205            ; 025A 8205
         CLB     X'AC'            ; 025B 07AC
         SNE                      ; 025C 0870
         JMP     J0255            ; 025D 1255
         JSX     S0522            ; 025E 2522
         CLB     X'AC'            ; 025F 07AC
         SNE                      ; 0260 0870
         JSX     S07AA            ; 0261 27AA
         LDX     D0301            ; 0262 9301
         LDW     D021B            ; 0263 821B
         STW     *0               ; 0264 7800
         JSX     S0528            ; 0265 2528
         JSX     S0794            ; 0266 2794
         JMP     J00E0            ; 0267 10E0
         JSX     S01C3            ; 0268 21C3
J0269    JSX     S051D            ; 0269 251D
         JSX     S0550            ; 026A 2550
         STW     D021E            ; 026B 721E
         CAX                      ; 026C 0130
         LDW     *0               ; 026D 8800
         SLC     4                ; 026E 0A54
         SAP                      ; 026F 0810
         JMP     J027B            ; 0270 127B
         JSX     S0774            ; 0271 2774
         DATA    X'0838'          ; 0272 0838  argument 1 of 0271
J0273    LDW     D021E            ; 0273 821E
         JSX     S0794            ; 0274 2794
         JSX     S074A            ; 0275 274A
         JSX     S07AA            ; 0276 27AA
         JSX     S074A            ; 0277 274A
         JSX     S07AA            ; 0278 27AA
J0279    JSX     S074A            ; 0279 274A
         JMP     J00E0            ; 027A 10E0
J027B    JSX     S0774            ; 027B 2774
         DATA    X'083A'          ; 027C 083A  argument 1 of 027B
         JMP     J0273            ; 027D 1273
         JSX     S01C3            ; 027E 21C3
J027F    JSX     S074E            ; 027F 274E
         CAX                      ; 0280 0130
         LDW     *1               ; 0281 8801
         STW     D02A8            ; 0282 72A8
         STX     D02A5            ; 0283 62A5
         JSX     S0536            ; 0284 2536
         STW     D0293            ; 0285 7293
         JSX     S0513            ; 0286 2513
         JMP     J023D            ; 0287 123D  not run
         JSX     S0536            ; 0288 2536
         STW     D0297            ; 0289 7297
         JSX     S0513            ; 028A 2513
J028B    JMP     J02A2            ; 028B 12A2
         DATA    X'2536'          ; 028C 2536  JSX X'536'
J028D    STW     D0295            ; 028D 7295
         JSX     S0774            ; 028E 2774
         DATA    X'082E'          ; 028F 082E  argument 1 of 028E
         JSX     S0774            ; 0290 2774
         DATA    X'0830'          ; 0291 0830  argument 1 of 0290
         JSX     S0791            ; 0292 2791
D0293    DATA    X'0000'          ; 0293 0000  argument 1 of 0292; written by 0285
         JSX     S0791            ; 0294 2791
D0295    DATA    X'0000'          ; 0295 0000  argument 1 of 0294; written by 028D
         JSX     S0791            ; 0296 2791
D0297    DATA    X'0000'          ; 0297 0000  argument 1 of 0296; written by 0289
         STW     D02A9            ; 0298 72A9
         JSX     S0779            ; 0299 2779
         LDX     D02A7            ; 029A 92A7
         DXS     2                ; 029B 0502
         LDW     D02A8            ; 029C 82A8
         STW     *0               ; 029D 7800
         LDW     D02A9            ; 029E 82A9
         STW     *1               ; 029F 7801
         STX     D02A7            ; 02A0 62A7
         JMP     J00E0            ; 02A1 10E0
J02A2    LDW     D02A4            ; 02A2 82A4
         JMP     J028D            ; 02A3 128D
D02A4    DATA    X'0CFE'          ; 02A4 0CFE  read by 02A2
D02A5    DATA    X'0000'          ; 02A5 0000  written by 0105 0283; read by 0727
D02A6    DATA    X'0000'          ; 02A6 0000  written by 0747; read by 0745
D02A7    DATA    X'0846'          ; 02A7 0846  written by 00A2 013B 02A0; read by 0130 0137 029A 07FC
D02A8    DATA    X'0000'          ; 02A8 0000  written by 0282; read by 029C
D02A9    DATA    X'0000'          ; 02A9 0000  written by 013C 0298; read by 0141 029E
         JSX     S01C3            ; 02AA 21C3
J02AB    JSX     S0774            ; 02AB 2774
         DATA    X'082A'          ; 02AC 082A  argument 1 of 02AB
J02AD    LDW     D0205            ; 02AD 8205
         CLB     X'DA'            ; 02AE 07DA
         SNE                      ; 02AF 0870
         JSX     S02BE            ; 02B0 22BE  not run
         STB     D02BC            ; 02B1 3578
         JSX     S02BE            ; 02B2 22BE
J02B3    STB     D02BC+1          ; 02B3 3579
         JSX     S02BE            ; 02B4 22BE
J02B5    STB     D02BA            ; 02B5 3574
         JSX     S02BE            ; 02B6 22BE
J02B7    STB     D02BA+1          ; 02B7 3575
         JSX     S02BE            ; 02B8 22BE
J02B9    JSX     S0774            ; 02B9 2774
D02BA    DATA    X'0000'          ; 02BA 0000  argument 1 of 02B9; written by 02B5 02B7
         JSX     S0774            ; 02BB 2774
D02BC    DATA    X'0000'          ; 02BC 0000  argument 1 of 02BB; written by 02B1 02B3
         JMP     J012C            ; 02BD 112C
S02BE    LDW     D0205            ; 02BE 8205
         CLB     X'20'            ; 02BF 0720
         SEQ                      ; 02C0 0860
         JMP     S07AA            ; 02C1 17AA  not run
         LLB     X'A0'            ; 02C2 06A0
         JMP     *0               ; 02C3 1800
         JSX     S0774            ; 02C4 2774
         DATA    X'0828'          ; 02C5 0828  argument 1 of 02C4
         JMP     J02AD            ; 02C6 12AD
         JSX     S0779            ; 02C7 2779
         CLR                      ; 02C8 0100
         STW     D0301            ; 02C9 7301
         JSX     S0779            ; 02CA 2779
         STW     D0302            ; 02CB 7302
         STW     D0303            ; 02CC 7303
         JSX     S0172            ; 02CD 2172
         JSX     S014A            ; 02CE 214A
         JMP     J02F5            ; 02CF 12F5  not run
         CAX                      ; 02D0 0130
         LDW     *3               ; 02D1 8803
         SAZ                      ; 02D2 0800
         JMP     J022F            ; 02D3 122F  not run
J02D4    LDW     D0302            ; 02D4 8302
         ORI     D0767            ; 02D5 C767
         STW     *3               ; 02D6 7803
         JSX     S0774            ; 02D7 2774
         DATA    X'0842'          ; 02D8 0842  argument 1 of 02D7
         LDW     D0205            ; 02D9 8205
         CLB     X'A8'            ; 02DA 07A8
         SEQ                      ; 02DB 0860
         JMP     J00EF            ; 02DC 10EF  not run
         JSX     S07AA            ; 02DD 27AA
J02DE    JSX     S03D5            ; 02DE 23D5
         LDX     D020D            ; 02DF 920D
         LDW     D0208            ; 02E0 8208
         ORI     D0767            ; 02E1 C767
         STW     *3               ; 02E2 7803
         LDW     *0               ; 02E3 8800
         ORI     D01FD            ; 02E4 C1FD
         STW     *0               ; 02E5 7800
         JSX     S0779            ; 02E6 2779
         LDX     D0301            ; 02E7 9301
         IXS     1                ; 02E8 0401
D02E9    DATA    X'4300'          ; 02E9 4300  CMB X'300'; read by 02F7 033C
         STX     D0301            ; 02EA 6301
         JSX     S0513            ; 02EB 2513
J02EC    JMP     J02EE            ; 02EC 12EE
         JMP     J02DE            ; 02ED 12DE
J02EE    JSX     S0522            ; 02EE 2522
         JSX     S0519            ; 02EF 2519
         JMP     J00EF            ; 02F0 10EF  not run
J02F1    LDX     D0302            ; 02F1 9302
         LDW     D0301            ; 02F2 8301
         STW     *1               ; 02F3 7801
         JMP     J00EF            ; 02F4 10EF
J02F5    LDX     D0209            ; 02F5 9209  not run
         LDW     D021E            ; 02F6 821E  not run
         ORI     D02E9            ; 02F7 C2E9  not run
         STW     *0               ; 02F8 7800  not run
         LDW     D021F            ; 02F9 821F  not run
         STW     *1               ; 02FA 7801  not run
         LDW     D0220            ; 02FB 8220  not run
         STW     *2               ; 02FC 7802  not run
         CXA                      ; 02FD 0140  not run
         ADD     D013A            ; 02FE A13A  not run
         STW     D0209            ; 02FF 7209  not run
         JMP     J02D4            ; 0300 12D4  not run
D0301    DATA    X'0000'          ; 0301 0000  written by 0253 02C9 02EA 0324 ...; read by 0262 02E7 02F2 032F ...
D0302    DATA    X'0000'          ; 0302 0000  written by 009C 02CB; read by 01C9 02D4 02F1 0307
D0303    DATA    X'0000'          ; 0303 0000  written by 0103 02CC; read by 03F5
         JSX     S01C3            ; 0304 21C3
J0305    JSX     S0774            ; 0305 2774
         DATA    X'0846'          ; 0306 0846  argument 1 of 0305
         LDW     D0302            ; 0307 8302
         SAM                      ; 0308 0820
         JMP     J030C            ; 0309 130C
         JSX     S0241            ; 030A 2241
         DATA    X'D2D4'          ; 030B D2D4  argument 1 of 030A; 'RT'
J030C    JSX     S0779            ; 030C 2779
         JMP     J00E0            ; 030D 10E0
         JSX     S01C3            ; 030E 21C3
J030F    JSX     S0172            ; 030F 2172
         JSX     S014A            ; 0310 214A
         JMP     J033A            ; 0311 133A
J0312    STW     D0339            ; 0312 7339
         LDW     D0205            ; 0313 8205
         CLB     X'A8'            ; 0314 07A8
         SEQ                      ; 0315 0860
         JMP     J031A            ; 0316 131A  not run
         JSX     S07AA            ; 0317 27AA
         STX     D0346            ; 0318 6346
         JSX     S0550            ; 0319 2550
J031A    JSX     S0774            ; 031A 2774
         DATA    X'0840'          ; 031B 0840  argument 1 of 031A
         LDW     D0339            ; 031C 8339
         JSX     S0779            ; 031D 2779
         LDW     D0346            ; 031E 8346
         SAZ                      ; 031F 0800
         JMP     J0322            ; 0320 1322
         JMP     J0396            ; 0321 1396  not run
J0322    CLR                      ; 0322 0100
         STW     D0346            ; 0323 7346
         STW     D0301            ; 0324 7301
         LDW     D0208            ; 0325 8208
         STW     D0339            ; 0326 7339
         JSX     S0779            ; 0327 2779
J0328    LDW     D064B            ; 0328 864B
         CLB     2                ; 0329 0702
         SNE                      ; 032A 0870
         JMP     J0333            ; 032B 1333
         LDW     D045D            ; 032C 845D
         JSX     S0794            ; 032D 2794
         JSX     S07E6            ; 032E 27E6
         LDW     D0301            ; 032F 8301
         ADD     D01F5            ; 0330 A1F5
         STW     D0301            ; 0331 7301
         JMP     J0328            ; 0332 1328
J0333    JSX     S0519            ; 0333 2519
         JMP     J012C            ; 0334 112C  not run
J0335    LDX     D0339            ; 0335 9339
         LDW     D0301            ; 0336 8301
J0337    STW     *0               ; 0337 7800
         JMP     J012C            ; 0338 112C
D0339    DATA    X'0000'          ; 0339 0000  written by 0312 0326; read by 031C 0335
J033A    LDX     D0209            ; 033A 9209
         LDW     D021E            ; 033B 821E
         ORI     D02E9            ; 033C C2E9
         STW     *0               ; 033D 7800
         LDW     D021F            ; 033E 821F
         STW     *1               ; 033F 7801
         LDW     D0220            ; 0340 8220
         STW     *2               ; 0341 7802
         CLR                      ; 0342 0100
         STW     *3               ; 0343 7803
         CXA                      ; 0344 0140
         IXS     4                ; 0345 0404
D0346    DATA    X'0000'          ; 0346 0000  written by 0104 0318 0323; read by 031E 0662
         STX     D0209            ; 0347 6209
         JMP     J0312            ; 0348 1312
         LDW     D0136            ; 0349 8136
         JSX     S0779            ; 034A 2779
         JSX     S0779            ; 034B 2779
         STW     D021B            ; 034C 721B
         JSX     S051D            ; 034D 251D
J034E    STW     D021C            ; 034E 721C
         JSX     S07B3            ; 034F 27B3
         CLB     X'20'            ; 0350 0720
         SNE                      ; 0351 0870
         JMP     J0363            ; 0352 1363
         SLL     8                ; 0353 0A18
         ORI     D021C            ; 0354 C21C
         JSX     S0779            ; 0355 2779
         JSX     S07B3            ; 0356 27B3
         CLB     X'20'            ; 0357 0720
         SEQ                      ; 0358 0860
         JMP     J034E            ; 0359 134E
J035A    LDW     D0211            ; 035A 8211
         SUB     D01F5            ; 035B B1F5
         STW     D0211            ; 035C 7211
         JSX     S0519            ; 035D 2519
         JMP     J00E0            ; 035E 10E0  not run
J035F    LDX     D021B            ; 035F 921B
         LDW     D0208            ; 0360 8208
         STW     *1               ; 0361 7801
         JMP     J00E0            ; 0362 10E0
J0363    LDW     D021C            ; 0363 821C
         JSX     S0779            ; 0364 2779
         JMP     J035A            ; 0365 135A
         LDW     D0398            ; 0366 8398
         JMP     J0369            ; 0367 1369
         LDW     D039D            ; 0368 839D
J0369    STW     D036C            ; 0369 736C
         JSX     S01C3            ; 036A 21C3
J036B    JSX     S0774            ; 036B 2774
D036C    DATA    X'8000'          ; 036C 8000  argument 1 of 036B; LDW 0; written by 0369
         JSX     S051D            ; 036D 251D
         JSX     S0528            ; 036E 2528
         JSX     S0794            ; 036F 2794
         JSX     S0513            ; 0370 2513
J0371    JMP     J0389            ; 0371 1389
         JSX     S074A            ; 0372 274A
         JSX     S0522            ; 0373 2522
J0374    CLB     X'20'            ; 0374 0720
         SNE                      ; 0375 0870
         JMP     J0386            ; 0376 1386  not run
         JSX     S0550            ; 0377 2550
         STW     D0382            ; 0378 7382
         CAX                      ; 0379 0130
         LDW     *0               ; 037A 8800
         SLL     4                ; 037B 0A14
         LDX     D0399            ; 037C 9399
         SAP                      ; 037D 0810
         LDX     D039A            ; 037E 939A
         CXA                      ; 037F 0140
         JSX     S0779            ; 0380 2779
         JSX     S0791            ; 0381 2791
D0382    DATA    X'0000'          ; 0382 0000  argument 1 of 0381; written by 0378
J0383    JSX     S0513            ; 0383 2513
J0384    JMP     J0386            ; 0384 1386
         JMP     J0374            ; 0385 1374
J0386    JSX     S0774            ; 0386 2774
         DATA    X'00A9'          ; 0387 00A9  argument 1 of 0386
         JMP     J012C            ; 0388 112C
J0389    JSX     S0522            ; 0389 2522
         JSX     S0519            ; 038A 2519
         JMP     J0390            ; 038B 1390  not run
J038C    LDX     D0208            ; 038C 9208
         LDW     *2               ; 038D 8802
         SUB     D01F7            ; 038E B1F7
         STW     *2               ; 038F 7802
J0390    JSX     S0172            ; 0390 2172
         JSX     S014A            ; 0391 214A
         JMP     J022B            ; 0392 122B  not run
         STW     D020D            ; 0393 720D
         JSX     S0779            ; 0394 2779
         JSX     S0413            ; 0395 2413
J0396    JSX     S0779            ; 0396 2779
         JMP     J012C            ; 0397 112C
D0398    DATA    X'009E'          ; 0398 009E  read by 0366
D0399    DATA    X'00A7'          ; 0399 00A7  read by 037C
D039A    DATA    X'00A5'          ; 039A 00A5  read by 037E
         JSX     S01C3            ; 039B 21C3
J039C    JSX     S0774            ; 039C 2774
D039D    DATA    X'00A3'          ; 039D 00A3  argument 1 of 039C; read by 0368
         JSX     S0774            ; 039E 2774
         DATA    X'0D01'          ; 039F 0D01  argument 1 of 039E
         JSX     S074A            ; 03A0 274A
         JMP     J0383            ; 03A1 1383
         DATA    X'23D5'          ; 03A2 23D5  JSX X'3D5'
         DATA    X'2513'          ; 03A3 2513  JSX X'513'
         DATA    X'1239'          ; 03A4 1239  JMP X'239'
         DATA    X'8205'          ; 03A5 8205  LDW X'205'
         DATA    X'07D8'          ; 03A6 07D8
         DATA    X'0860'          ; 03A7 0860
         DATA    X'1239'          ; 03A8 1239  JMP X'239'
         DATA    X'25A7'          ; 03A9 25A7  JSX X'5A7'
         DATA    X'8221'          ; 03AA 8221  LDW X'221'
         DATA    X'F3BB'          ; 03AB F3BB  's;'
         DATA    X'0880'          ; 03AC 0880
         DATA    X'1239'          ; 03AD 1239  JMP X'239'
         DATA    X'920D'          ; 03AE 920D  LDX X'20D'
         DATA    X'7803'          ; 03AF 7803  STW *3
         DATA    X'B1F6'          ; 03B0 B1F6  '1v'
         DATA    X'920A'          ; 03B1 920A  LDX X'20A'
         DATA    X'0500'          ; 03B2 0500
         DATA    X'F208'          ; 03B3 F208  CMW X'208'
         DATA    X'0880'          ; 03B4 0880
         DATA    X'7208'          ; 03B5 7208  STW X'208'
         DATA    X'0880'          ; 03B6 0880
         DATA    X'7222'          ; 03B7 7222  STW X'222'
         DATA    X'2513'          ; 03B8 2513  JSX X'513'
         DATA    X'10EF'          ; 03B9 10EF  JMP X'EF'
         DATA    X'13A2'          ; 03BA 13A2  JMP X'3A2'
         DATA    X'0C1C'          ; 03BB 0C1C
         SML     2                ; 03BC 0082
         LDW     D0B38            ; 03BD 8338
         SAM                      ; 03BE 0820
         JMP     J0237            ; 03BF 1237  not run
J03C0    JSX     S03D5            ; 03C0 23D5
         LDX     D03CE            ; 03C1 93CE
         LDW     D020D            ; 03C2 820D
         STW     *3               ; 03C3 7803
         CAX                      ; 03C4 0130
         LDW     *0               ; 03C5 8800
         ORI     D01FD            ; 03C6 C1FD
         STW     *0               ; 03C7 7800
         CLR                      ; 03C8 0100
         STW     *3               ; 03C9 7803
         STX     D03CE            ; 03CA 63CE
         JSX     S0513            ; 03CB 2513
J03CC    JMP     J00EF            ; 03CC 10EF
         JMP     J03C0            ; 03CD 13C0
D03CE    DATA    X'0B35'          ; 03CE 0B35  written by 00A4 03CA; read by 03C1
D03CF    DATA    X'0B35'          ; 03CF 0B35  read by 00A3
J03D0    JSX     S03D5            ; 03D0 23D5
         JSX     S0513            ; 03D1 2513
J03D2    JMP     J00EF            ; 03D2 10EF
         JMP     J03D0            ; 03D3 13D0
D03D4    DATA    X'0000'          ; 03D4 0000  written by 03D5; read by 0410
S03D5    STX     D03D4            ; 03D5 63D4
         JSX     S0172            ; 03D6 2172
         JSX     S014A            ; 03D7 214A
         JMP     J03E0            ; 03D8 13E0
         STW     D020D            ; 03D9 720D
         CAX                      ; 03DA 0130
         LDW     *0               ; 03DB 8800
         SLL     7                ; 03DC 0A17
         SAP                      ; 03DD 0810
         JMP     J022F            ; 03DE 122F  not run
         JMP     J03F1            ; 03DF 13F1
J03E0    LDX     D0209            ; 03E0 9209
         LDW     D021E            ; 03E1 821E
         ORI     D03EF            ; 03E2 C3EF
         STW     *0               ; 03E3 7800
         LDW     D021F            ; 03E4 821F
         STW     *1               ; 03E5 7801
         LDW     D0220            ; 03E6 8220
         STW     *2               ; 03E7 7802
         CLR                      ; 03E8 0100
         STW     *3               ; 03E9 7803
         LDW     D01F5            ; 03EA 81F5
         STW     *4               ; 03EB 7804
         STW     *5               ; 03EC 7805
         STX     D020D            ; 03ED 620D
         IXS     6                ; 03EE 0406
D03EF    DATA    X'6000'          ; 03EF 6000  STX 0; read by 03E2
         STX     D0209            ; 03F0 6209
J03F1    LDW     D0205            ; 03F1 8205
         CLB     X'A8'            ; 03F2 07A8
         SEQ                      ; 03F3 0860
         JMP     J0410            ; 03F4 1410
         LDW     D0303            ; 03F5 8303
         SAZ                      ; 03F6 0800
         JMP     J023D            ; 03F7 123D  not run
         JSX     S07AA            ; 03F8 27AA
         JSX     S070A            ; 03F9 270A
         LDW     D0221            ; 03FA 8221
         SRL     14               ; 03FB 0A0E
         SAZ                      ; 03FC 0800
         JMP     J0235            ; 03FD 1235  not run
         LDX     D020D            ; 03FE 920D
         LDW     *0               ; 03FF 8800
         ORI     D01FC            ; 0400 C1FC
         STW     *0               ; 0401 7800
         LDW     D0221            ; 0402 8221
         STW     *4               ; 0403 7804
         JSX     S0513            ; 0404 2513
J0405    JMP     J040E            ; 0405 140E
         JSX     S070A            ; 0406 270A
         LDW     D0221            ; 0407 8221
         SRL     14               ; 0408 0A0E
         SAZ                      ; 0409 0800
         JMP     J0235            ; 040A 1235  not run
         LDX     D020D            ; 040B 920D
         LDW     D0221            ; 040C 8221
         STW     *5               ; 040D 7805
J040E    JSX     S0522            ; 040E 2522
         JSX     S0413            ; 040F 2413
J0410    LDX     D03D4            ; 0410 93D4
         JSX     *0               ; 0411 2800
D0412    DATA    X'0000'          ; 0412 0000  written by 0413; read by 0432
S0413    STX     D0412            ; 0413 6412
         LDX     D020D            ; 0414 920D
         LDW     *5               ; 0415 8805
         STW     D021C            ; 0416 721C
         LDW     *0               ; 0417 8800
         SLL     4                ; 0418 0A14
         SAM                      ; 0419 0820
         JMP     J041F            ; 041A 141F
         LDW     D021C            ; 041B 821C
         SLL     1                ; 041C 0A11
         ADD     D021C            ; 041D A21C
         STW     D021C            ; 041E 721C
J041F    LDW     *4               ; 041F 8804
         AND     D0200            ; 0420 E200
         STW     D020C            ; 0421 720C
         CLR                      ; 0422 0100
         LDX     D01F9            ; 0423 91F9
J0424    STX     D021B            ; 0424 621B
         LDX     D020C            ; 0425 920C
         SXE                      ; 0426 0850
         ADD     D021C            ; 0427 A21C
         SRLD    1                ; 0428 0A21
         STX     D020C            ; 0429 620C
         LDX     D021B            ; 042A 921B
         DXS     1                ; 042B 0501
         JMP     J0424            ; 042C 1424
         LDX     D020C            ; 042D 920C
         SLLD    2                ; 042E 0A32
         SAZ                      ; 042F 0800
         JMP     J0235            ; 0430 1235  not run
         SLLD    14               ; 0431 0A3E
         LDX     D0412            ; 0432 9412
         JSX     *0               ; 0433 2800
         JSX     S0440            ; 0434 2440
         DATA    X'0800'          ; 0435 0800  argument 1 of 0434
         DATA    X'0085'          ; 0436 0085  argument 2 of 0434
         JSX     S0440            ; 0437 2440
         DATA    X'0802'          ; 0438 0802  argument 1 of 0437
         DATA    X'0087'          ; 0439 0087  argument 2 of 0437
         JSX     S0440            ; 043A 2440
         DATA    X'0804'          ; 043B 0804  argument 1 of 043A
         DATA    X'0089'          ; 043C 0089  argument 2 of 043A
         JSX     S0440            ; 043D 2440
         DATA    X'0806'          ; 043E 0806  argument 1 of 043D
         DATA    X'008B'          ; 043F 008B  argument 2 of 043D
S0440    LDW     D0213            ; 0440 8213
         SLL     4                ; 0441 0A14
         CMW     D01F4            ; 0442 F1F4
         LDW     *1               ; 0443 8801
         SLS                      ; 0444 0840
         LDW     *0               ; 0445 8800
         STW     D0457            ; 0446 7457
         LDW     D0213            ; 0447 8213
         ORE     D0214            ; 0448 D214
         SLL     4                ; 0449 0A14
         SAP                      ; 044A 0810
         JMP     J023B            ; 044B 123B
J044C    LDW     D07DA            ; 044C 87DA
         CLB     X'E'             ; 044D 070E
         SNE                      ; 044E 0870
         JMP     J0471            ; 044F 1471
J0450    JSX     S0726            ; 0450 2726
         STW     D0459            ; 0451 7459
         CAX                      ; 0452 0130
         LDW     *0               ; 0453 8800
         ORI     D0216            ; 0454 C216
         STW     *0               ; 0455 7800
J0456    JSX     S0774            ; 0456 2774
D0457    DATA    X'0000'          ; 0457 0000  argument 1 of 0456; written by 0446 047E 04A2; read by 047C
         JSX     S0791            ; 0458 2791
D0459    DATA    X'0000'          ; 0459 0000  argument 1 of 0458; written by 0451 047B 0723; read by 011C 0461 0619 076E
         JSX     S0791            ; 045A 2791
D045B    DATA    X'0000'          ; 045B 0000  argument 1 of 045A; written by 0462 0489 069A 07E1 ...; read by 0494 0506 0683 07DC ...
         JSX     S0791            ; 045C 2791
D045D    DATA    X'0000'          ; 045D 0000  argument 1 of 045C; written by 04ED 056B 0581 05CD ...; read by 032C 0496 04D5 04DE ...
         LDW     D0202            ; 045E 8202
         SAZ                      ; 045F 0800
         JMP     J0481            ; 0460 1481
         LDW     D0459            ; 0461 8459
         STW     D045B            ; 0462 745B
J0463    LDW     D064B            ; 0463 864B
         STW     D064C            ; 0464 764C
J0465    JSX     S07E6            ; 0465 27E6
         LDW     D064C            ; 0466 864C
         CLB     2                ; 0467 0702
         SEQ                      ; 0468 0860
         JMP     J0675            ; 0469 1675
         LDW     D064B            ; 046A 864B
         CLB     1                ; 046B 0701
         SEQ                      ; 046C 0860
         CLB     X'C'             ; 046D 070C
         SNE                      ; 046E 0870
         JMP     J065A            ; 046F 165A
         JMP     J0675            ; 0470 1675
J0471    LDX     D0219            ; 0471 9219
         LDW     *0               ; 0472 8800
         ORE     D0213            ; 0473 D213
         SLL     4                ; 0474 0A14
         SAP                      ; 0475 0810
         JMP     J0450            ; 0476 1450
         LDW     D064B            ; 0477 864B
         CLB     0                ; 0478 0700
         SEQ                      ; 0479 0860
         JMP     J0450            ; 047A 1450
         STX     D0459            ; 047B 6459
         LDW     D0457            ; 047C 8457
         ORI     D0767            ; 047D C767
         STW     D0457            ; 047E 7457
         STX     D0202            ; 047F 6202
         JMP     J0456            ; 0480 1456
J0481    LDW     D064B            ; 0481 864B
         STW     D07DA            ; 0482 77DA
         CLR                      ; 0483 0100
         STW     D0202            ; 0484 7202
         STW     D0226            ; 0485 7226
         JSX     S07E6            ; 0486 27E6
         JMP     J0465            ; 0487 1465
         JSX     S0726            ; 0488 2726
         STW     D045B            ; 0489 745B
         CAX                      ; 048A 0130
         LDW     *0               ; 048B 8800
         ORI     D0216            ; 048C C216
         STW     *0               ; 048D 7800
         SLL     4                ; 048E 0A14
         CMW     D01F4            ; 048F F1F4
         LDW     D0499            ; 0490 8499
         SLS                      ; 0491 0840
         LDW     D049A            ; 0492 849A
J0493    JSX     S0779            ; 0493 2779
         LDW     D045B            ; 0494 845B
         JSX     S0794            ; 0495 2794
         LDW     D045D            ; 0496 845D
         JSX     S0794            ; 0497 2794
         JMP     J0463            ; 0498 1463
D0499    DATA    X'0822'          ; 0499 0822  read by 0490
D049A    DATA    X'0808'          ; 049A 0808  read by 0492
         LDW     D0204            ; 049B 8204
         SAZ                      ; 049C 0800
         JMP     J04A4            ; 049D 14A4
         LDW     D0216            ; 049E 8216  not run
         LDX     D04AD            ; 049F 94AD  not run
         SAZ                      ; 04A0 0800  not run
         LDX     D04AF            ; 04A1 94AF  not run
J04A2    STX     D0457            ; 04A2 6457
         JMP     J044C            ; 04A3 144C
J04A4    LDW     D0213            ; 04A4 8213
         SLL     4                ; 04A5 0A14
         LDX     D04AE            ; 04A6 94AE
         SAP                      ; 04A7 0810
         JMP     J0233            ; 04A8 1233  not run
         LDW     D0216            ; 04A9 8216
         ORE     D01FE            ; 04AA D1FE
         STW     D0216            ; 04AB 7216
         JMP     J04A2            ; 04AC 14A2
D04AD    DATA    X'084A'          ; 04AD 084A  read by 049F
D04AE    DATA    X'084C'          ; 04AE 084C  read by 04A6
D04AF    DATA    X'084E'          ; 04AF 084E  read by 04A1
         CLR                      ; 04B0 0100
         STW     D0226            ; 04B1 7226
         LDW     D0204            ; 04B2 8204
         SAZ                      ; 04B3 0800
         JMP     J04BC            ; 04B4 14BC
         LDW     D0216            ; 04B5 8216
         LDX     D04C2            ; 04B6 94C2
         SAZ                      ; 04B7 0800
         LDX     D04C3            ; 04B8 94C3
J04B9    CXA                      ; 04B9 0140
         ORI     D0767            ; 04BA C767
         JMP     J0493            ; 04BB 1493
J04BC    LDW     D0214            ; 04BC 8214
         SLL     4                ; 04BD 0A14
         LDX     D04C5            ; 04BE 94C5
         SAP                      ; 04BF 0810
         LDX     D04C4            ; 04C0 94C4
         JMP     J04B9            ; 04C1 14B9
D04C2    DATA    X'080A'          ; 04C2 080A  read by 04B6
D04C3    DATA    X'0814'          ; 04C3 0814  read by 04B8
D04C4    DATA    X'0810'          ; 04C4 0810  read by 04C0
D04C5    DATA    X'080C'          ; 04C5 080C  read by 04BE
J04C6    LDW     D0205            ; 04C6 8205
         CLB     X'BD'            ; 04C7 07BD
         SNE                      ; 04C8 0870
         JMP     J022B            ; 04C9 122B
         LDW     D01F5            ; 04CA 81F5
         STW     D0301            ; 04CB 7301
         LDW     D04FD            ; 04CC 84FD
         JSX     S0779            ; 04CD 2779
         JSX     S0779            ; 04CE 2779
         JSX     S0779            ; 04CF 2779
         STW     D021C            ; 04D0 721C
J04D1    LDW     D064B            ; 04D1 864B
         CLB     8                ; 04D2 0708
         SNE                      ; 04D3 0870
         JMP     J04DC            ; 04D4 14DC
         LDW     D045D            ; 04D5 845D
         JSX     S0794            ; 04D6 2794
         LDW     D0301            ; 04D7 8301
         ADD     D01F5            ; 04D8 A1F5
         STW     D0301            ; 04D9 7301
         JSX     S07E6            ; 04DA 27E6
         JMP     J04D1            ; 04DB 14D1
J04DC    JSX     S0519            ; 04DC 2519
         JMP     J04E9            ; 04DD 14E9  not run
J04DE    LDX     D045D            ; 04DE 945D
         LDW     *3               ; 04DF 8803
         CMW     D01F4            ; 04E0 F1F4
         LDX     D021C            ; 04E1 921C
         SLS                      ; 04E2 0840
         STW     *3               ; 04E3 7803
         LDW     D0301            ; 04E4 8301
         STW     *2               ; 04E5 7802
         LDW     D045D            ; 04E6 845D
         SGR                      ; 04E7 0880
         JSX     S0779            ; 04E8 2779
J04E9    CLR                      ; 04E9 0100
J04EA    STW     D020F            ; 04EA 720F
         JSX     S0726            ; 04EB 2726
         LDX     D045D            ; 04EC 945D
         STW     D045D            ; 04ED 745D
         LDW     *0               ; 04EE 8800
         AND     D01FE            ; 04EF E1FE
         LDX     D045D            ; 04F0 945D
         ORI     D020F            ; 04F1 C20F
         ORI     *0               ; 04F2 C800
         STW     *0               ; 04F3 7800
         JSX     S0519            ; 04F4 2519
         JMP     J04F9            ; 04F5 14F9  not run
J04F6    LDW     D045D            ; 04F6 845D
         LDX     D021C            ; 04F7 921C
         STW     *1               ; 04F8 7801
J04F9    LDW     D064B            ; 04F9 864B
         LLB     0                ; 04FA 0600
         STW     D064D            ; 04FB 764D
         JMP     J0625            ; 04FC 1625
D04FD    DATA    X'0844'          ; 04FD 0844  read by 04CC
J04FE    LDW     D064C            ; 04FE 864C
         CLB     X'C'             ; 04FF 070C
         SNE                      ; 0500 0870
         JMP     J050D            ; 0501 150D
         JSX     S0774            ; 0502 2774
         DATA    X'083C'          ; 0503 083C  argument 1 of 0502
J0504    LDW     D045D            ; 0504 845D
         JSX     S0794            ; 0505 2794
         LDW     D045B            ; 0506 845B
         JSX     S0779            ; 0507 2779
         JSX     S07E6            ; 0508 27E6
         JSX     S0779            ; 0509 2779
         STW     D021C            ; 050A 721C
         LDW     D01FD            ; 050B 81FD
         JMP     J04EA            ; 050C 14EA
J050D    JSX     S0774            ; 050D 2774
         DATA    X'083E'          ; 050E 083E  argument 1 of 050D
         LDW     D045D            ; 050F 845D
         JSX     S0794            ; 0510 2794
         JSX     S07E6            ; 0511 27E6
         JMP     J0504            ; 0512 1504
S0513    LDW     D0205            ; 0513 8205
         CLB     X'AC'            ; 0514 07AC
         SNE                      ; 0515 0870
         IXS     1                ; 0516 0401
         JMP     *0               ; 0517 1800
         JMP     S07AA            ; 0518 17AA
S0519    LDW     D020A            ; 0519 820A
         SAM                      ; 051A 0820
         JMP     *1               ; 051B 1801
         JMP     *0               ; 051C 1800
S051D    LDW     D0205            ; 051D 8205
         CLB     X'A8'            ; 051E 07A8
         SEQ                      ; 051F 0860
         JMP     J0231            ; 0520 1231  not run
         JMP     S07AA            ; 0521 17AA
S0522    LDW     D0205            ; 0522 8205
         CLB     X'A9'            ; 0523 07A9
         SEQ                      ; 0524 0860
         JMP     J0229            ; 0525 1229
         JMP     S07AA            ; 0526 17AA
D0527    DATA    X'0000'          ; 0527 0000  written by 0528 0536; read by 0532
S0528    STX     D0527            ; 0528 6527
         LDW     D0534            ; 0529 8534
         STW     D0535            ; 052A 7535
         JMP     J0560            ; 052B 1560
J052C    LDX     D045D            ; 052C 945D
J052D    LDW     *0               ; 052D 8800
         SLL     4                ; 052E 0A14
         SAP                      ; 052F 0810
         JMP     J0233            ; 0530 1233  not run
         CXA                      ; 0531 0140
         LDX     D0527            ; 0532 9527
         JSX     *0               ; 0533 2800
D0534    DATA    X'052C'          ; 0534 052C  read by 0529
D0535    DATA    X'0000'          ; 0535 0000  written by 0107 052A 0552; read by 0625
S0536    STX     D0527            ; 0536 6527
         JSX     S0550            ; 0537 2550
         CAX                      ; 0538 0130
         JMP     J052D            ; 0539 152D
D053A    DATA    X'051A'          ; 053A 051A  read by 062A
D053B    DATA    X'061A'          ; 053B 061A  read by 062E
D053C    DATA    X'031C'          ; 053C 031C  read by 066E
D053D    DATA    X'041C'          ; 053D 041C  read by 0635
D053E    DATA    X'0000'          ; 053E 0000  read by 0665
D053F    DATA    X'0001'          ; 053F 0001  read by 063F
D0540    DATA    X'000C'          ; 0540 000C  read by 0660
D0541    DATA    X'0008'          ; 0541 0008  read by 0597
D0542    DATA    X'0006'          ; 0542 0006  read by 055D
D0543    DATA    X'0120'          ; 0543 0120  read by 064E
D0544    DATA    X'000A'          ; 0544 000A  read by 0578
D0545    DATA    X'070E'          ; 0545 070E  read by 0657
D0546    DATA    X'021E'          ; 0546 021E  read by 0672
         DATA    X'0649'          ; 0547 0649
         DATA    X'0488'          ; 0548 0488
         DATA    X'049B'          ; 0549 049B
         DATA    X'043A'          ; 054A 043A
         DATA    X'043D'          ; 054B 043D
         DATA    X'0434'          ; 054C 0434
         DATA    X'0437'          ; 054D 0437
         DATA    X'04B0'          ; 054E 04B0
D054F    DATA    X'0000'          ; 054F 0000  written by 0113 0550; read by 065E
S0550    STX     D054F            ; 0550 654F
         CLR                      ; 0551 0100
         STW     D0535            ; 0552 7535
         LLB     2                ; 0553 0602
         STW     D064C            ; 0554 764C
J0555    LDW     D0205            ; 0555 8205
         CLB     X'AD'            ; 0556 07AD
         SNE                      ; 0557 0870
         JMP     J064E            ; 0558 164E
         CLB     X'AB'            ; 0559 07AB
         SNE                      ; 055A 0870
         JMP     J059D            ; 055B 159D  not run
J055C    CLB     X'A8'            ; 055C 07A8
         LDW     D0542            ; 055D 8542
         SNE                      ; 055E 0870
         JMP     J064F            ; 055F 164F
J0560    JSX     S071B            ; 0560 271B
J0561    JMP     J05BD            ; 0561 15BD
J0562    CLB     X'AE'            ; 0562 07AE
         SNE                      ; 0563 0870
         JMP     J05BD            ; 0564 15BD
         CLB     X'D8'            ; 0565 07D8
         SNE                      ; 0566 0870
         JMP     J059F            ; 0567 159F
J0568    JSX     S0172            ; 0568 2172
         JSX     S014A            ; 0569 214A
         JMP     J0580            ; 056A 1580
         STW     D045D            ; 056B 745D
         LDW     D0205            ; 056C 8205
         CLB     X'A8'            ; 056D 07A8
         SEQ                      ; 056E 0860
         JMP     J057A            ; 056F 157A
         LDX     D045D            ; 0570 945D
         LDW     *0               ; 0571 8800
         SLC     6                ; 0572 0A56
         SAM                      ; 0573 0820
         JMP     J0649            ; 0574 1649  not run
         SLL     1                ; 0575 0A11
         SAP                      ; 0576 0810
         JMP     J0597            ; 0577 1597
         LDW     D0544            ; 0578 8544
         JMP     J0598            ; 0579 1598
J057A    LDX     D045D            ; 057A 945D
         LDW     *0               ; 057B 8800
         SLL     6                ; 057C 0A16
         SAP                      ; 057D 0810
         JMP     J0231            ; 057E 1231  not run
         JMP     J0625            ; 057F 1625
J0580    LDX     D0209            ; 0580 9209
         STX     D045D            ; 0581 645D
         LDW     D021F            ; 0582 821F
         STW     *1               ; 0583 7801
         LDW     D0220            ; 0584 8220
         STW     *2               ; 0585 7802
         LDW     D0767            ; 0586 8767
         STW     *3               ; 0587 7803
         IXS     3                ; 0588 0403
D0589    DATA    X'0800'          ; 0589 0800  read by 0182
         LDW     D0205            ; 058A 8205
         CLB     X'A8'            ; 058B 07A8
         LLB     X'43'            ; 058C 0643
         SNE                      ; 058D 0870
         IXS     1                ; 058E 0401
         LLB     X'30'            ; 058F 0630
         STX     D0209            ; 0590 6209
         SLL     8                ; 0591 0A18
         ORI     D021E            ; 0592 C21E
         LDX     D045D            ; 0593 945D
         STW     *0               ; 0594 7800
         SEQ                      ; 0595 0860
         JMP     J0625            ; 0596 1625
J0597    LDW     D0541            ; 0597 8541
J0598    ORI     D064D            ; 0598 C64D
         STW     D064B            ; 0599 764B
         SLL     8                ; 059A 0A18
         STW     D064D            ; 059B 764D
J059C    JSX     S07D3            ; 059C 27D3
J059D    JSX     S07AA            ; 059D 27AA
         JMP     J0555            ; 059E 1555
J059F    LDX     D07B8            ; 059F 97B8
         LDB     *0               ; 05A0 5800
         CLB     X'A7'            ; 05A1 07A7
         SEQ                      ; 05A2 0860
         JMP     J0568            ; 05A3 1568
         JSX     S05A7            ; 05A4 25A7  not run
         DATA    X'15C5'          ; 05A5 15C5  JMP X'5C5'
D05A6    DATA    X'0000'          ; 05A6 0000  written by 05A7; read by 05BB
S05A7    STX     D05A6            ; 05A7 65A6  not run
         CLR                      ; 05A8 0100  not run
         STW     D0221            ; 05A9 7221  not run
         JSX     S07AA            ; 05AA 27AA  not run
J05AB    JSX     S07AA            ; 05AB 27AA  not run
         CLB     X'20'            ; 05AC 0720  not run
         SNE                      ; 05AD 0870  not run
         JMP     J022D            ; 05AE 122D  not run
         CLB     X'A7'            ; 05AF 07A7  not run
         SNE                      ; 05B0 0870  not run
         JMP     J05BA            ; 05B1 15BA  not run
         CLB     X'B9'            ; 05B2 07B9  not run
         SLE                      ; 05B3 0890  not run
         SUB     D01F8            ; 05B4 B1F8  not run
         SRLD    4                ; 05B5 0A24  not run
         LDW     D0221            ; 05B6 8221  not run
         SLLD    4                ; 05B7 0A34  not run
         STW     D0221            ; 05B8 7221  not run
         JMP     J05AB            ; 05B9 15AB  not run
J05BA    JSX     S07AA            ; 05BA 27AA  not run
         LDX     D05A6            ; 05BB 95A6  not run
         JSX     *0               ; 05BC 2800  not run
J05BD    JSX     S070A            ; 05BD 270A
         CLB     X'AE'            ; 05BE 07AE
         SNE                      ; 05BF 0870
         JMP     J05D5            ; 05C0 15D5
         LDW     D021F            ; 05C1 821F
         ORI     D0220            ; 05C2 C220
         SAZ                      ; 05C3 0800
         JMP     J022D            ; 05C4 122D  not run
         LDW     D0221            ; 05C5 8221
         STW     D021F            ; 05C6 721F
         LDW     D05D3            ; 05C7 85D3
         STW     D021E            ; 05C8 721E
         JSX     S014B            ; 05C9 214B
         JMP     J05CC            ; 05CA 15CC
         JMP     J05FE            ; 05CB 15FE
J05CC    LDX     D0209            ; 05CC 9209
         STX     D045D            ; 05CD 645D
         LDW     D021E            ; 05CE 821E
         STW     *0               ; 05CF 7800
         LDW     D021F            ; 05D0 821F
         STW     *1               ; 05D1 7801
         IXS     2                ; 05D2 0402
D05D3    DATA    X'2101'          ; 05D3 2101  JSX X'101'; read by 05C7 0798
         JMP     J060A            ; 05D4 160A
J05D5    CLR                      ; 05D5 0100
         STW     D021B            ; 05D6 721B
         STW     D021C            ; 05D7 721C
         JSX     S07AA            ; 05D8 27AA
         JSX     S0710            ; 05D9 2710
         LDW     D0205            ; 05DA 8205
         CLB     X'C5'            ; 05DB 07C5
         SEQ                      ; 05DC 0860
         JMP     J05EC            ; 05DD 15EC
         JSX     S07AA            ; 05DE 27AA
         LDW     D0205            ; 05DF 8205
         CLB     X'AD'            ; 05E0 07AD
         SNE                      ; 05E1 0870
         JMP     J05E5            ; 05E2 15E5  not run
         CLB     X'AB'            ; 05E3 07AB
         CLR                      ; 05E4 0100
J05E5    STW     D021D            ; 05E5 721D
         SNE                      ; 05E6 0870
         JSX     S07AA            ; 05E7 27AA  not run
J05E8    JSX     S071B            ; 05E8 271B
J05E9    JMP     J0619            ; 05E9 1619
J05EA    LDW     D021D            ; 05EA 821D
         SLA     9                ; 05EB 0919
J05EC    LDW     D021B            ; 05EC 821B
         SNO                      ; 05ED 08A0
         CMP                      ; 05EE 0110  not run
         SUB     D021C            ; 05EF B21C
J05F0    SAZ                      ; 05F0 0800
         JMP     J060C            ; 05F1 160C
         JSX     S06EA            ; 05F2 26EA
         LDW     D01FB            ; 05F3 81FB
         ADD     D021E            ; 05F4 A21E
         STW     D021E            ; 05F5 721E
         LLB     0                ; 05F6 0600
         SAZ                      ; 05F7 0800
         JMP     J022D            ; 05F8 122D
         LDW     D021E            ; 05F9 821E
         ORI     D0609            ; 05FA C609
         STW     D021E            ; 05FB 721E
         JSX     S014A            ; 05FC 214A
         JMP     J0600            ; 05FD 1600
J05FE    STW     D045D            ; 05FE 745D
         JMP     J0625            ; 05FF 1625
J0600    LDX     D0209            ; 0600 9209
         STX     D045D            ; 0601 645D
         LDW     D021E            ; 0602 821E
         STW     *0               ; 0603 7800
         LDW     D021F            ; 0604 821F
         STW     *1               ; 0605 7801
         LDW     D0220            ; 0606 8220
         STW     *2               ; 0607 7802
         IXS     3                ; 0608 0403
D0609    DATA    X'3900'          ; 0609 3900  STB *X'100'; read by 05FA
J060A    STX     D0209            ; 060A 6209
         JMP     J0625            ; 060B 1625
J060C    STW     D021C            ; 060C 721C
         SAP                      ; 060D 0810
         JMP     J0614            ; 060E 1614
         CLR                      ; 060F 0100
         JSX     S06AA            ; 0610 26AA
         LDW     D021C            ; 0611 821C
         SUB     D01F5            ; 0612 B1F5
         JMP     J05F0            ; 0613 15F0
J0614    JSX     S06EA            ; 0614 26EA
         JSX     S06DA            ; 0615 26DA
         LDW     D021C            ; 0616 821C
         ADD     D01F5            ; 0617 A1F5
         JMP     J05F0            ; 0618 15F0
J0619    LDW     D0459            ; 0619 8459
         SRLD    1                ; 061A 0A21
         ADD     D021B            ; 061B A21B
         SRLD    2                ; 061C 0A22
         ADD     D021B            ; 061D A21B
         SRLD    13               ; 061E 0A2D
         STX     D021B            ; 061F 621B
         DXS     0                ; 0620 0500
         SAZ                      ; 0621 0800
         JMP     J022D            ; 0622 122D  not run
         JSX     S07AA            ; 0623 27AA
         JMP     J05E8            ; 0624 15E8
J0625    LDW     D0535            ; 0625 8535
         CAX                      ; 0626 0130
         SAZ                      ; 0627 0800
         JMP     *0               ; 0628 1800
         LDW     D0205            ; 0629 8205
         LDX     D053A            ; 062A 953A
         CLB     X'AB'            ; 062B 07AB
         SNE                      ; 062C 0870
         JMP     J0673            ; 062D 1673
         LDX     D053B            ; 062E 953B
         CLB     X'AD'            ; 062F 07AD
         SNE                      ; 0630 0870
         JMP     J0673            ; 0631 1673
         CLB     X'AA'            ; 0632 07AA
         SNE                      ; 0633 0870
         JMP     J066B            ; 0634 166B
         LDX     D053D            ; 0635 953D
         CLB     X'AF'            ; 0636 07AF
         SNE                      ; 0637 0870
         JMP     J0673            ; 0638 1673
         CLB     X'20'            ; 0639 0720
         SNE                      ; 063A 0870
         JMP     J0665            ; 063B 1665
         CLB     X'BD'            ; 063C 07BD
         SNE                      ; 063D 0870
         JMP     J0657            ; 063E 1657
         LDX     D053F            ; 063F 953F
         CLB     X'A9'            ; 0640 07A9
         SNE                      ; 0641 0870
         JMP     J0673            ; 0642 1673
         CLB     X'AC'            ; 0643 07AC
         SNE                      ; 0644 0870
         JMP     J0660            ; 0645 1660
         CLB     X'DE'            ; 0646 07DE
         SNE                      ; 0647 0870
         JMP     J0672            ; 0648 1672  not run
J0649    JSX     S0241            ; 0649 2241
         DATA    X'CFD0'          ; 064A CFD0  argument 1 of 0649; 'OP'
D064B    DATA    X'0000'          ; 064B 0000  written by 0599 064F 0658 0666 ...; read by 0328 0463 046A 0477 ...
D064C    DATA    X'0000'          ; 064C 0000  written by 0115 0464 0554 07E3 ...; read by 0466 04FE 0676 068C ...
D064D    DATA    X'0000'          ; 064D 0000  written by 0109 04FB 059B; read by 0598 0661 0667 06A0
J064E    LDW     D0543            ; 064E 8543
J064F    STW     D064B            ; 064F 764B
         JMP     J059C            ; 0650 159C
J0651    JSX     S07D3            ; 0651 27D3
         CLB     X'C'             ; 0652 070C
         SNE                      ; 0653 0870
         JMP     J0555            ; 0654 1555
         LDW     D0205            ; 0655 8205
         JMP     J055C            ; 0656 155C
J0657    LDW     D0545            ; 0657 8545
         STW     D064B            ; 0658 764B
         JMP     J059C            ; 0659 159C
J065A    LDW     D0226            ; 065A 8226
         SAZ                      ; 065B 0800
         JMP     J023F            ; 065C 123F  not run
         LDW     D045D            ; 065D 845D
         LDX     D054F            ; 065E 954F
         JSX     *0               ; 065F 2800
J0660    LDX     D0540            ; 0660 9540
         LDW     D064D            ; 0661 864D
         ORI     D0346            ; 0662 C346
         SAZ                      ; 0663 0800
         JMP     J0673            ; 0664 1673
J0665    LDW     D053E            ; 0665 853E
         STW     D064B            ; 0666 764B
         LDW     D064D            ; 0667 864D
         SAZ                      ; 0668 0800
         JMP     J0229            ; 0669 1229  not run
         JMP     J0680            ; 066A 1680
J066B    LDX     D07B8            ; 066B 97B8
         LDB     *0               ; 066C 5800
         CLB     X'AA'            ; 066D 07AA
         LDX     D053C            ; 066E 953C
         SEQ                      ; 066F 0860
         JMP     J0673            ; 0670 1673
         JSX     S07AA            ; 0671 27AA
J0672    LDX     D0546            ; 0672 9546
J0673    STX     D064B            ; 0673 664B
         JSX     S07AA            ; 0674 27AA
J0675    LDW     D064B            ; 0675 864B
         CMB     D064C+1          ; 0676 4499
         SLE                      ; 0677 0890
         JMP     J0651            ; 0678 1651
         SNE                      ; 0679 0870
         CLB     X'C'             ; 067A 070C
         SNE                      ; 067B 0870
         JMP     J0651            ; 067C 1651  not run
         CLB     1                ; 067D 0701
         SNE                      ; 067E 0870
         JMP     J0695            ; 067F 1695
J0680    LDX     D045D            ; 0680 945D
         LDW     *0               ; 0681 8800
         STW     D0213            ; 0682 7213
         LDX     D045B            ; 0683 945B
         LDW     *0               ; 0684 8800
         STW     D0214            ; 0685 7214
         ORE     D0213            ; 0686 D213
         AND     D01FE            ; 0687 E1FE
         STW     D0204            ; 0688 7204
         LDW     D0213            ; 0689 8213
         AND     D01FE            ; 068A E1FE
         STW     D0216            ; 068B 7216
         LDW     D064C            ; 068C 864C
         CLB     2                ; 068D 0702
         SNE                      ; 068E 0870
         JMP     J065A            ; 068F 165A
         SRL     8                ; 0690 0A08
         CAX                      ; 0691 0130
         SLM                      ; 0692 0040
         LDX     *X'547'          ; 0693 9D47
         JSX     *0               ; 0694 2800
J0695    LDW     D064C            ; 0695 864C
         CLB     6                ; 0696 0706
         SEQ                      ; 0697 0860
         JMP     J069D            ; 0698 169D
         LDW     D045D            ; 0699 845D  not run
         STW     D045B            ; 069A 745B  not run
         JSX     S07E6            ; 069B 27E6  not run
         JMP     J0625            ; 069C 1625  not run
J069D    CLB     X'C'             ; 069D 070C
         SLE                      ; 069E 0890
         JMP     J0680            ; 069F 1680
         LDW     D064D            ; 06A0 864D
         SAZ                      ; 06A1 0800
         JMP     J06A4            ; 06A2 16A4
         JMP     J065A            ; 06A3 165A
J06A4    SRL     8                ; 06A4 0A08
         CLB     X'A'             ; 06A5 070A
         SNE                      ; 06A6 0870
         JMP     J04FE            ; 06A7 14FE
         JMP     J04C6            ; 06A8 14C6
D06A9    DATA    X'0000'          ; 06A9 0000  written by 06AA 06DA 06EA; read by 06C5
S06AA    STX     D06A9            ; 06AA 66A9
         SRLD    1                ; 06AB 0A21
         ADD     D0221            ; 06AC A221
         SRLD    2                ; 06AD 0A22
         ADD     D0221            ; 06AE A221
         SRLD    12               ; 06AF 0A2C
         SLL     1                ; 06B0 0A11
         SRLD    1                ; 06B1 0A21
         STX     D0221            ; 06B2 6221
         SRLD    1                ; 06B3 0A21
         ADD     D0220            ; 06B4 A220
         SRLD    2                ; 06B5 0A22
         ADD     D0220            ; 06B6 A220
         SRLD    12               ; 06B7 0A2C
         SLL     1                ; 06B8 0A11
         SRLD    1                ; 06B9 0A21
         STX     D0220            ; 06BA 6220
         SRLD    1                ; 06BB 0A21
         ADD     D021F            ; 06BC A21F
         SRLD    2                ; 06BD 0A22
         ADD     D021F            ; 06BE A21F
         SRLD    12               ; 06BF 0A2C
J06C0    SLL     1                ; 06C0 0A11
         SRLD    1                ; 06C1 0A21
         STX     D021F            ; 06C2 621F
         SAZ                      ; 06C3 0800
         JMP     J06C7            ; 06C4 16C7
J06C5    LDX     D06A9            ; 06C5 96A9
         JSX     *0               ; 06C6 2800
J06C7    STW     D0206            ; 06C7 7206
         LDW     D021E            ; 06C8 821E
         ADD     D01F5            ; 06C9 A1F5
         STW     D021E            ; 06CA 721E
         LDW     D0221            ; 06CB 8221
         SLL     1                ; 06CC 0A11
         LDX     D0220            ; 06CD 9220
         SRCD    1                ; 06CE 0A61
         SRL     1                ; 06CF 0A01
         STW     D0221            ; 06D0 7221
         SRCD    14               ; 06D1 0A6E
         LDX     D021F            ; 06D2 921F
         SRCD    1                ; 06D3 0A61
         SRL     1                ; 06D4 0A01
         STW     D0220            ; 06D5 7220
         SLLD    2                ; 06D6 0A32
         LDW     D0206            ; 06D7 8206
         SRLD    1                ; 06D8 0A21
         JMP     J06C0            ; 06D9 16C0
S06DA    STX     D06A9            ; 06DA 66A9
         CLR                      ; 06DB 0100
         SML     2                ; 06DC 0082
         STW     D090D            ; 06DD 710D
         LDW     D021F            ; 06DE 821F
         JSX     S06E8            ; 06DF 26E8
         STW     D021F            ; 06E0 721F
         LDW     D0220            ; 06E1 8220
         JSX     S06E8            ; 06E2 26E8
         STW     D0220            ; 06E3 7220
         LDW     D0221            ; 06E4 8221
         JSX     S06E8            ; 06E5 26E8
         STW     D0221            ; 06E6 7221
         JMP     J06C5            ; 06E7 16C5
S06E8    SML     2                ; 06E8 0082
         JMP     J08E2            ; 06E9 10E2
S06EA    STX     D06A9            ; 06EA 66A9
         LDW     D021F            ; 06EB 821F
         ORI     D0220            ; 06EC C220
         ORI     D0221            ; 06ED C221
         SAZ                      ; 06EE 0800
         JMP     J06F3            ; 06EF 16F3
         SUB     D01FB            ; 06F0 B1FB  not run
         STW     D021E            ; 06F1 721E  not run
         JMP     J06C5            ; 06F2 16C5  not run
J06F3    LDW     D021F            ; 06F3 821F
J06F4    SLA     1                ; 06F4 0911
         SNO                      ; 06F5 08A0
         JMP     J06C5            ; 06F6 16C5
         LDW     D021E            ; 06F7 821E
         SUB     D01F5            ; 06F8 B1F5
         STW     D021E            ; 06F9 721E
         LDX     D0221            ; 06FA 9221
         SLLD    2                ; 06FB 0A32
         SLL     1                ; 06FC 0A11
         SRLD    1                ; 06FD 0A21
         STX     D0221            ; 06FE 6221
         SRLD    1                ; 06FF 0A21
         LDW     D0220            ; 0700 8220
         SRLD    14               ; 0701 0A2E
         SLL     1                ; 0702 0A11
         SRLD    1                ; 0703 0A21
         STX     D0220            ; 0704 6220
         SRLD    1                ; 0705 0A21
         LDW     D021F            ; 0706 821F
         SLLD    1                ; 0707 0A31
         STW     D021F            ; 0708 721F
         JMP     J06F4            ; 0709 16F4
S070A    CLR                      ; 070A 0100
         STW     D021E            ; 070B 721E
         STW     D021F            ; 070C 721F
         STW     D0220            ; 070D 7220
         STW     D0221            ; 070E 7221
         STW     D021C            ; 070F 721C
S0710    STX     D021A            ; 0710 621A
J0711    JSX     S071B            ; 0711 271B
J0712    JMP     J0715            ; 0712 1715
J0713    LDX     D021A            ; 0713 921A
         JSX     *0               ; 0714 2800
J0715    JSX     S06AA            ; 0715 26AA
         LDW     D021C            ; 0716 821C
         ADD     D01F5            ; 0717 A1F5
         STW     D021C            ; 0718 721C
         JSX     S07AA            ; 0719 27AA
         JMP     J0711            ; 071A 1711
S071B    LDW     D0205            ; 071B 8205
         CLB     X'AF'            ; 071C 07AF
         SGR                      ; 071D 0880
         JMP     *1               ; 071E 1801
         CLB     X'B9'            ; 071F 07B9
         SLE                      ; 0720 0890
         JMP     *1               ; 0721 1801
         AND     D01F9            ; 0722 E1F9
         STW     D0459            ; 0723 7459
         JMP     *0               ; 0724 1800
D0725    DATA    X'0000'          ; 0725 0000  written by 0726; read by 0738
S0726    STX     D0725            ; 0726 6725
         LDW     D02A5            ; 0727 82A5
         SAZ                      ; 0728 0800
         JMP     J0745            ; 0729 1745  not run
         LDW     D0215            ; 072A 8215
         ADD     D01F5            ; 072B A1F5
         STW     D0215            ; 072C 7215
J072D    STW     D021F            ; 072D 721F
         LDW     D0749            ; 072E 8749
         STW     D021E            ; 072F 721E
         JSX     S014B            ; 0730 214B
         JMP     J073A            ; 0731 173A
         STW     D0742            ; 0732 7742
J0733    LDX     D0742            ; 0733 9742
         SLLD    1                ; 0734 0A31
         LLB     X'30'            ; 0735 0630
         STB     *0               ; 0736 3800
         LDW     D0742            ; 0737 8742
         LDX     D0725            ; 0738 9725
         JSX     *0               ; 0739 2800
J073A    LDX     D0209            ; 073A 9209
         STX     D0742            ; 073B 6742
         LDW     D021E            ; 073C 821E
         STW     *0               ; 073D 7800
         LDW     D021F            ; 073E 821F
         STW     *1               ; 073F 7801
         STW     *2               ; 0740 7802
         IXS     3                ; 0741 0403
D0742    DATA    X'0000'          ; 0742 0000  written by 0732 073B; read by 0733 0737
         STX     D0209            ; 0743 6209
         JMP     J0733            ; 0744 1733
J0745    LDW     D02A6            ; 0745 82A6  not run
         ADD     D01F5            ; 0746 A1F5  not run
         STW     D02A6            ; 0747 72A6  not run
         JMP     J072D            ; 0748 172D  not run
D0749    DATA    X'30AE'          ; 0749 30AE  STB X'AE'; read by 072E
S074A    STX     D0778            ; 074A 6778
         JSX     S074E            ; 074B 274E
         JMP     J077A            ; 074C 177A
D074D    DATA    X'0000'          ; 074D 0000  written by 074E; read by 075C
S074E    STX     D074D            ; 074E 674D
         CLR                      ; 074F 0100
         STW     D021C            ; 0750 721C
J0751    JSX     S071B            ; 0751 271B
J0752    JMP     J076A            ; 0752 176A
J0753    LDW     D021C            ; 0753 821C
         SAZ                      ; 0754 0800
         JMP     J0757            ; 0755 1757
         JMP     J0120            ; 0756 1120
J0757    STW     D021F            ; 0757 721F
         LDW     D0124            ; 0758 8124
         STW     D021E            ; 0759 721E
         JSX     S014B            ; 075A 214B
         JMP     J075E            ; 075B 175E
J075C    LDX     D074D            ; 075C 974D
         JSX     *0               ; 075D 2800
J075E    LDX     D0209            ; 075E 9209
         LDW     D0124            ; 075F 8124
         STW     *0               ; 0760 7800
         LDW     D021C            ; 0761 821C
         STW     *1               ; 0762 7801
         LDW     D0767            ; 0763 8767
         STW     *2               ; 0764 7802
         CXA                      ; 0765 0140
         IXS     3                ; 0766 0403
D0767    DATA    X'8000'          ; 0767 8000  LDW 0; read by 009A 00C1 01DF 02D5 ...
         STX     D0209            ; 0768 6209
         JMP     J075C            ; 0769 175C
J076A    LDW     D021C            ; 076A 821C
         SLL     3                ; 076B 0A13
         ADD     D021C            ; 076C A21C
         ADD     D021C            ; 076D A21C
         ADD     D0459            ; 076E A459
         SAP                      ; 076F 0810
         JMP     J0120            ; 0770 1120  not run
         STW     D021C            ; 0771 721C
         JSX     S07AA            ; 0772 27AA
         JMP     J0751            ; 0773 1751
S0774    LDW     *0               ; 0774 8800
         IXS     1                ; 0775 0401
D0776    DATA    X'0800'          ; 0776 0800  written by 0100 07DB 07F5; read by 07D4 07EF
         JMP     S0779            ; 0777 1779
D0778    DATA    X'0000'          ; 0778 0000  written by 074A 0779 0794; read by 078F
S0779    STX     D0778            ; 0779 6778
J077A    LDX     D078D            ; 077A 978D
         DXS     0                ; 077B 0500
         LDX     D020A            ; 077C 920A
         IXS     0                ; 077D 0400
         JMP     J078E            ; 077E 178E
         LDX     D0208            ; 077F 9208
         STW     *0               ; 0780 7800
         CMW     D07F8            ; 0781 F7F8
         CXA                      ; 0782 0140
         SUB     D01F5            ; 0783 B1F5
         STW     D0208            ; 0784 7208
         SUB     D0209            ; 0785 B209
J0786    SUB     D01F9            ; 0786 B1F9
         SEQ                      ; 0787 0860
         SAM                      ; 0788 0820
         JMP     J078E            ; 0789 178E
         STW     D078D            ; 078A 778D  not run
         JSX     S0241            ; 078B 2241  not run
         DATA    X'D3DA'          ; 078C D3DA  argument 1 of 078B; 'SZ'
D078D    DATA    X'0000'          ; 078D 0000  written by 0095 078A; read by 077A
J078E    LDW     D0208            ; 078E 8208
         LDX     D0778            ; 078F 9778
         JSX     *0               ; 0790 2800
S0791    LDW     *0               ; 0791 8800
         IXS     1                ; 0792 0401
D0793    DATA    X'0000'          ; 0793 0000  written by 009D 00BB; read by 00F6 07C0
S0794    STX     D0778            ; 0794 6778
         CAX                      ; 0795 0130
         SAZ                      ; 0796 0800
         LDW     *0               ; 0797 8800
         CMW     D05D3            ; 0798 F5D3
         SNE                      ; 0799 0870
         JMP     J07A0            ; 079A 17A0
         AND     D01FD            ; 079B E1FD
         SAZ                      ; 079C 0800
         JMP     J07A3            ; 079D 17A3
         CXA                      ; 079E 0140
         JMP     J077A            ; 079F 177A
J07A0    CXA                      ; 07A0 0140
         ADD     D01F5            ; 07A1 A1F5
         JMP     J077A            ; 07A2 177A
J07A3    LDW     *0               ; 07A3 8800
         CLB     X'AE'            ; 07A4 07AE
         CXA                      ; 07A5 0140
         ORI     D0767            ; 07A6 C767
         SEQ                      ; 07A7 0860
         LDW     *3               ; 07A8 8803
         JMP     J077A            ; 07A9 177A
S07AA    STX     D07D2            ; 07AA 67D2
         LDW     D07B8            ; 07AB 87B8
         STW     D01C1            ; 07AC 71C1
J07AD    JSX     S07B3            ; 07AD 27B3
         CLB     X'A0'            ; 07AE 07A0
         SNE                      ; 07AF 0870
         JMP     J07AD            ; 07B0 17AD
         JMP     J07E4            ; 07B1 17E4
D07B2    DATA    X'0000'          ; 07B2 0000  written by 07B3; read by 07BE
S07B3    STX     D07B2            ; 07B3 67B2
         LDX     D07B8            ; 07B4 97B8
J07B5    CLR                      ; 07B5 0100
         LDB     *0               ; 07B6 5800
         IXS     1                ; 07B7 0401
D07B8    DATA    X'0000'          ; 07B8 0000  written by 07B9 0984; read by 00F3 0187 059F 066B ...
         STX     D07B8            ; 07B9 67B8
         STW     D0205            ; 07BA 7205
         CLB     X'20'            ; 07BB 0720
         SNE                      ; 07BC 0870
         JMP     J07C0            ; 07BD 17C0
J07BE    LDX     D07B2            ; 07BE 97B2
         JSX     *0               ; 07BF 2800
J07C0    LDX     D0793            ; 07C0 9793
         IXS     0                ; 07C1 0400
         JMP     J07BE            ; 07C2 17BE
         LDW     D020A            ; 07C3 820A  not run
         SML     2                ; 07C4 0082  not run
         SAP                      ; 07C5 0810  not run
         JSX     X'02A8'          ; 07C6 22A8  not run; lands at 0AA8 = S0AA8 under the selection in force
         SML     2                ; 07C7 0082  not run
         JSX     S095A            ; 07C8 215A  not run
         LDX     D07B8            ; 07C9 97B8  not run
         LDB     *5               ; 07CA 5805  not run
         CLB     X'A0'            ; 07CB 07A0  not run
         LDW     D07D0            ; 07CC 87D0  not run
         SNE                      ; 07CD 0870  not run
         JMP     J07BE            ; 07CE 17BE  not run
         IXS     6                ; 07CF 0406  not run
D07D0    DATA    X'0020'          ; 07D0 0020  read by 07CC
         JMP     J07B5            ; 07D1 17B5  not run
D07D2    DATA    X'0000'          ; 07D2 0000  written by 07AA 07D3 07E6; read by 07E4
S07D3    STX     D07D2            ; 07D3 67D2
         LDX     D0776            ; 07D4 9776
         LDW     D0219            ; 07D5 8219
         STW     *0               ; 07D6 7800
         LDW     D07DA            ; 07D7 87DA
         STW     *1               ; 07D8 7801
         IXS     2                ; 07D9 0402
D07DA    DATA    X'0000'          ; 07DA 0000  written by 0482 07DF 07F4; read by 044C 07D7 07ED
         STX     D0776            ; 07DB 6776
         LDW     D045B            ; 07DC 845B
         STW     D0219            ; 07DD 7219
         LDW     D064C            ; 07DE 864C
         STW     D07DA            ; 07DF 77DA
         LDW     D045D            ; 07E0 845D
         STW     D045B            ; 07E1 745B
         LDW     D064B            ; 07E2 864B
         STW     D064C            ; 07E3 764C
J07E4    LDX     D07D2            ; 07E4 97D2
         JSX     *0               ; 07E5 2800
S07E6    STX     D07D2            ; 07E6 67D2
         LDW     D045B            ; 07E7 845B
         STW     D045D            ; 07E8 745D
         LDW     D064C            ; 07E9 864C
         STW     D064B            ; 07EA 764B
         LDW     D0219            ; 07EB 8219
         STW     D045B            ; 07EC 745B
         LDW     D07DA            ; 07ED 87DA
         STW     D064C            ; 07EE 764C
         LDX     D0776            ; 07EF 9776
         DXS     2                ; 07F0 0502
         LDW     *0               ; 07F1 8800
         STW     D0219            ; 07F2 7219
         LDW     *1               ; 07F3 8801
         STW     D07DA            ; 07F4 77DA
         STX     D0776            ; 07F5 6776
         JMP     J07E4            ; 07F6 17E4
J07F7    JSX     S0774            ; 07F7 2774
D07F8    DATA    X'0828'          ; 07F8 0828  argument 1 of 07F7; read by 0781
         SML     2                ; 07F9 0082
         JSX     S0977            ; 07FA 2177
         LDW     D0223            ; 07FB 8223
         CMW     D02A7            ; 07FC F2A7
         LDX     D0207            ; 07FD 9207
         SML     2                ; 07FE 0082
         JMP     J090F            ; 07FF 110F
         DATA    X'0000'          ; 0800 0000
         DATA    X'0000'          ; 0801 0000
         DATA    X'0000'          ; 0802 0000
         DATA    X'0000'          ; 0803 0000
         DATA    X'0000'          ; 0804 0000
         DATA    X'0000'          ; 0805 0000
         DATA    X'0000'          ; 0806 0000
         DATA    X'0000'          ; 0807 0000
         DATA    X'0000'          ; 0808 0000
         DATA    X'0000'          ; 0809 0000
         DATA    X'0000'          ; 080A 0000
         DATA    X'0000'          ; 080B 0000
         DATA    X'0000'          ; 080C 0000
         DATA    X'0000'          ; 080D 0000
         DATA    X'0000'          ; 080E 0000
         DATA    X'0000'          ; 080F 0000
         DATA    X'0000'          ; 0810 0000
         DATA    X'0000'          ; 0811 0000
         DATA    X'0000'          ; 0812 0000
         DATA    X'0000'          ; 0813 0000
         DATA    X'0000'          ; 0814 0000
         DATA    X'0000'          ; 0815 0000
         DATA    X'0000'          ; 0816 0000
         DATA    X'0000'          ; 0817 0000
         DATA    X'0000'          ; 0818 0000
         DATA    X'0000'          ; 0819 0000
         DATA    X'0000'          ; 081A 0000
         DATA    X'0000'          ; 081B 0000
         DATA    X'0000'          ; 081C 0000
         DATA    X'0000'          ; 081D 0000
         DATA    X'0000'          ; 081E 0000
         DATA    X'0000'          ; 081F 0000
         DATA    X'0000'          ; 0820 0000
         DATA    X'0000'          ; 0821 0000
         DATA    X'0000'          ; 0822 0000
         DATA    X'0000'          ; 0823 0000
         DATA    X'0000'          ; 0824 0000
         DATA    X'0000'          ; 0825 0000
         DATA    X'0000'          ; 0826 0000
         DATA    X'0000'          ; 0827 0000
         DATA    X'0000'          ; 0828 0000
         DATA    X'0000'          ; 0829 0000
         DATA    X'0000'          ; 082A 0000
         DATA    X'0000'          ; 082B 0000
         DATA    X'0000'          ; 082C 0000
         DATA    X'0000'          ; 082D 0000
         DATA    X'0000'          ; 082E 0000
         DATA    X'0000'          ; 082F 0000
         DATA    X'0000'          ; 0830 0000
         DATA    X'0000'          ; 0831 0000
         DATA    X'0000'          ; 0832 0000
         DATA    X'0000'          ; 0833 0000
         DATA    X'0000'          ; 0834 0000
         DATA    X'0000'          ; 0835 0000
         DATA    X'0000'          ; 0836 0000
         DATA    X'0000'          ; 0837 0000
         DATA    X'0000'          ; 0838 0000
         DATA    X'0000'          ; 0839 0000
         DATA    X'0000'          ; 083A 0000
         DATA    X'0000'          ; 083B 0000
         DATA    X'0000'          ; 083C 0000
         DATA    X'0000'          ; 083D 0000
         DATA    X'0000'          ; 083E 0000
         DATA    X'0000'          ; 083F 0000
         DATA    X'0000'          ; 0840 0000
         DATA    X'0000'          ; 0841 0000
         DATA    X'0000'          ; 0842 0000
         DATA    X'0000'          ; 0843 0000
         DATA    X'0000'          ; 0844 0000
         DATA    X'0000'          ; 0845 0000
; The names the compiler knows, four words each: a code byte and five characters of name, then a word the
; compiler takes when it finds that name. The functions IAND IOR IEOR NOT ABS IABS SIGN ISIGN EXP ALOG SIN COS
; SQRT ATAN ISWCH FLOAT IFIX; after the word at 088A, the statements COMMON DIMENSION LOCAT FORMAT END RETURN
; GO GOTO IF CONTINUE CALL DO PAUSE REWIND BACKSPACE ENDFILE READ WRITE PRINT SUBROUTINE STOP. Read from the tape.
         DATA    X'43C9'          ; 0846 43C9  CMB X'3C9'
         DATA    X'C1CE'          ; 0847 C1CE  'AN'
         DATA    X'C4A0'          ; 0848 C4A0  'D '
         DATA    X'0818'          ; 0849 0818
         DATA    X'43C9'          ; 084A 43C9  CMB X'3C9'
         DATA    X'CFD2'          ; 084B CFD2  'OR'
         DATA    X'A0A0'          ; 084C A0A0  '  '
         DATA    X'081C'          ; 084D 081C
         DATA    X'43C9'          ; 084E 43C9  CMB X'3C9'
         DATA    X'C5CF'          ; 084F C5CF  'EO'
         DATA    X'D2A0'          ; 0850 D2A0  'R '
         DATA    X'081A'          ; 0851 081A
         DATA    X'43CE'          ; 0852 43CE  CMB X'3CE'
         DATA    X'CFD4'          ; 0853 CFD4  'OT'
         DATA    X'A0A0'          ; 0854 A0A0  '  '
         DATA    X'0816'          ; 0855 0816
         DATA    X'4BC1'          ; 0856 4BC1  CMB *X'3C1'
         DATA    X'C2D3'          ; 0857 C2D3  'BS'
         DATA    X'A0A0'          ; 0858 A0A0  '  '
         DATA    X'0820'          ; 0859 0820
         DATA    X'43C9'          ; 085A 43C9  CMB X'3C9'
         DATA    X'C1C2'          ; 085B C1C2  'AB'
         DATA    X'D3A0'          ; 085C D3A0  'S '
         DATA    X'081E'          ; 085D 081E
         DATA    X'4BD3'          ; 085E 4BD3  CMB *X'3D3'
         DATA    X'C9C7'          ; 085F C9C7  'IG'
         DATA    X'CEA0'          ; 0860 CEA0  'N '
         DATA    X'0826'          ; 0861 0826
         DATA    X'43C9'          ; 0862 43C9  CMB X'3C9'
         DATA    X'D3C9'          ; 0863 D3C9  'SI'
         DATA    X'C7CE'          ; 0864 C7CE  'GN'
         DATA    X'0824'          ; 0865 0824
         DATA    X'4BC5'          ; 0866 4BC5  CMB *X'3C5'
         DATA    X'D8D0'          ; 0867 D8D0  'XP'
         DATA    X'A0A0'          ; 0868 A0A0  '  '
         DATA    X'0095'          ; 0869 0095
         DATA    X'4BC1'          ; 086A 4BC1  CMB *X'3C1'
         DATA    X'CCCF'          ; 086B CCCF  'LO'
         DATA    X'C7A0'          ; 086C C7A0  'G '
         DATA    X'0097'          ; 086D 0097
         DATA    X'4BD3'          ; 086E 4BD3  CMB *X'3D3'
         DATA    X'C9CE'          ; 086F C9CE  'IN'
         DATA    X'A0A0'          ; 0870 A0A0  '  '
         DATA    X'008F'          ; 0871 008F
         DATA    X'4BC3'          ; 0872 4BC3  CMB *X'3C3'
         DATA    X'CFD3'          ; 0873 CFD3  'OS'
         DATA    X'A0A0'          ; 0874 A0A0  '  '
         DATA    X'0091'          ; 0875 0091
         DATA    X'4BD3'          ; 0876 4BD3  CMB *X'3D3'
         DATA    X'D1D2'          ; 0877 D1D2  'QR'
         DATA    X'D4A0'          ; 0878 D4A0  'T '
         DATA    X'008D'          ; 0879 008D
         DATA    X'4BC1'          ; 087A 4BC1  CMB *X'3C1'
         DATA    X'D4C1'          ; 087B D4C1  'TA'
         DATA    X'CEA0'          ; 087C CEA0  'N '
         DATA    X'0093'          ; 087D 0093
         DATA    X'43C9'          ; 087E 43C9  CMB X'3C9'
         DATA    X'D3D7'          ; 087F D3D7  'SW'
         DATA    X'C3C8'          ; 0880 C3C8  'CH'
         DATA    X'082C'          ; 0881 082C
         DATA    X'4BC6'          ; 0882 4BC6  CMB *X'3C6'
         DATA    X'CCCF'          ; 0883 CCCF  'LO'
         DATA    X'C1D4'          ; 0884 C1D4  'AT'
         DATA    X'0812'          ; 0885 0812
         DATA    X'43C9'          ; 0886 43C9  CMB X'3C9'
         DATA    X'C6C9'          ; 0887 C6C9  'FI'
         DATA    X'D8A0'          ; 0888 D8A0  'X '
         DATA    X'080E'          ; 0889 080E
         DATA    X'8474'          ; 088A 8474  LDW X'474'
         DATA    X'41C3'          ; 088B 41C3  CMB X'1C3'
         DATA    X'CFCD'          ; 088C CFCD  'OM'
         DATA    X'CDCF'          ; 088D CDCF  'MO'
         DATA    X'03BC'          ; 088E 03BC
         DATA    X'41C4'          ; 088F 41C4  CMB X'1C4'
         DATA    X'C9CD'          ; 0890 C9CD  'IM'
         DATA    X'C5CE'          ; 0891 C5CE  'EN'
         DATA    X'03D0'          ; 0892 03D0
         DATA    X'41CC'          ; 0893 41CC  CMB X'1CC'
         DATA    X'CFC3'          ; 0894 CFC3  'OC'
         DATA    X'C1D4'          ; 0895 C1D4  'AT'
         DATA    X'03A2'          ; 0896 03A2
         DATA    X'41C6'          ; 0897 41C6  CMB X'1C6'
         DATA    X'CFD2'          ; 0898 CFD2  'OR'
         DATA    X'CDC1'          ; 0899 CDC1  'MA'
         DATA    X'0349'          ; 089A 0349
         DATA    X'41C5'          ; 089B 41C5  CMB X'1C5'
         DATA    X'CEC4'          ; 089C CEC4  'ND'
         DATA    X'A0A0'          ; 089D A0A0  '  '
         DATA    X'FFFF'          ; 089E FFFF  CMW *X'7FF'
         DATA    X'41D2'          ; 089F 41D2  CMB X'1D2'
         DATA    X'C5D4'          ; 08A0 C5D4  'ET'
         DATA    X'D5D2'          ; 08A1 D5D2  'UR'
         DATA    X'0304'          ; 08A2 0304
         DATA    X'41C7'          ; 08A3 41C7  CMB X'1C7'
         DATA    X'CFA0'          ; 08A4 CFA0  'O '
         DATA    X'A0A0'          ; 08A5 A0A0  '  '
         DATA    X'024B'          ; 08A6 024B
         DATA    X'41C7'          ; 08A7 41C7  CMB X'1C7'
         DATA    X'CFD4'          ; 08A8 CFD4  'OT'
         DATA    X'CFA0'          ; 08A9 CFA0  'O '
         DATA    X'0243'          ; 08AA 0243
         DATA    X'41C9'          ; 08AB 41C9  CMB X'1C9'
         DATA    X'C6A0'          ; 08AC C6A0  'F '
         DATA    X'A0A0'          ; 08AD A0A0  '  '
         DATA    X'0268'          ; 08AE 0268
         DATA    X'41C3'          ; 08AF 41C3  CMB X'1C3'
         DATA    X'CFCE'          ; 08B0 CFCE  'ON'
         DATA    X'D4C9'          ; 08B1 D4C9  'TI'
         DATA    X'012C'          ; 08B2 012C
         DATA    X'41C3'          ; 08B3 41C3  CMB X'1C3'
         DATA    X'C1CC'          ; 08B4 C1CC  'AL'
         DATA    X'CCA0'          ; 08B5 CCA0  'L '
         DATA    X'030E'          ; 08B6 030E
         DATA    X'41C4'          ; 08B7 41C4  CMB X'1C4'
         DATA    X'CFA0'          ; 08B8 CFA0  'O '
         DATA    X'A0A0'          ; 08B9 A0A0  '  '
         DATA    X'027E'          ; 08BA 027E
         DATA    X'41D0'          ; 08BB 41D0  CMB X'1D0'
         DATA    X'C1D5'          ; 08BC C1D5  'AU'
         DATA    X'D3C5'          ; 08BD D3C5  'SE'
         DATA    X'02AA'          ; 08BE 02AA
         DATA    X'41D2'          ; 08BF 41D2  CMB X'1D2'
         DATA    X'C5D7'          ; 08C0 C5D7  'EW'
         DATA    X'C9CE'          ; 08C1 C9CE  'IN'
         DATA    X'0942'          ; 08C2 0942
         DATA    X'41C2'          ; 08C3 41C2  CMB X'1C2'
         DATA    X'C1C3'          ; 08C4 C1C3  'AC'
         DATA    X'CBD3'          ; 08C5 CBD3  'KS'
         DATA    X'0944'          ; 08C6 0944
         DATA    X'41C5'          ; 08C7 41C5  CMB X'1C5'
         DATA    X'CEC4'          ; 08C8 CEC4  'ND'
         DATA    X'C6C9'          ; 08C9 C6C9  'FI'
         DATA    X'0946'          ; 08CA 0946
         DATA    X'41D2'          ; 08CB 41D2  CMB X'1D2'
         DATA    X'C5C1'          ; 08CC C5C1  'EA'
         DATA    X'C4A0'          ; 08CD C4A0  'D '
         DATA    X'0366'          ; 08CE 0366
         DATA    X'41D7'          ; 08CF 41D7  CMB X'1D7'
         DATA    X'D2C9'          ; 08D0 D2C9  'RI'
         DATA    X'D4C5'          ; 08D1 D4C5  'TE'
         DATA    X'0368'          ; 08D2 0368
         DATA    X'41D0'          ; 08D3 41D0  CMB X'1D0'
         DATA    X'D2C9'          ; 08D4 D2C9  'RI'
         DATA    X'CED4'          ; 08D5 CED4  'NT'
         DATA    X'039B'          ; 08D6 039B
         DATA    X'41D3'          ; 08D7 41D3  CMB X'1D3'
         DATA    X'D5C2'          ; 08D8 D5C2  'UB'
         DATA    X'D2CF'          ; 08D9 D2CF  'RO'
         DATA    X'02C7'          ; 08DA 02C7
         DATA    X'41D3'          ; 08DB 41D3  CMB X'1D3'
         DATA    X'D4CF'          ; 08DC D4CF  'TO'
         DATA    X'D0A0'          ; 08DD D0A0  'P '
         DATA    X'02C4'          ; 08DE 02C4
         DATA    X'0000'          ; 08DF 0000
         DATA    X'0000'          ; 08E0 0000
D08E1    DATA    X'0000'          ; 08E1 0000  written by 08E2; read by 0908
J08E2    STX     D08E1            ; 08E2 60E1
         CAX                      ; 08E3 0130
         SLLD    1                ; 08E4 0A31
         LDW     D090A            ; 08E5 810A
         STW     D090B            ; 08E6 710B
         LDW     D090D            ; 08E7 810D
         JMP     J08EB            ; 08E8 10EB
J08E9    STX     D090B            ; 08E9 610B
         LDX     D090D            ; 08EA 910D
J08EB    SLCD    1                ; 08EB 0A71
         SUB     D090C            ; 08EC B10C
         SAM                      ; 08ED 0820
         ADD     D090C            ; 08EE A10C
         SLCD    1                ; 08EF 0A71
         SUB     D090C            ; 08F0 B10C
         SAM                      ; 08F1 0820
         ADD     D090C            ; 08F2 A10C
         SLCD    1                ; 08F3 0A71
         SUB     D090C            ; 08F4 B10C
         SAM                      ; 08F5 0820
         ADD     D090C            ; 08F6 A10C
         SLCD    1                ; 08F7 0A71
         SUB     D090C            ; 08F8 B10C
         SAM                      ; 08F9 0820
         ADD     D090C            ; 08FA A10C
         SLCD    1                ; 08FB 0A71
         SUB     D090C            ; 08FC B10C
         SAM                      ; 08FD 0820
         ADD     D090C            ; 08FE A10C
         STX     D090D            ; 08FF 610D
         LDX     D090B            ; 0900 910B
         DXS     1                ; 0901 0501
         JMP     J08E9            ; 0902 10E9
         LDX     D090D            ; 0903 910D
         SLCD    1                ; 0904 0A71
         SRL     1                ; 0905 0A01
         STW     D090D            ; 0906 710D
         CXA                      ; 0907 0140
         LDX     D08E1            ; 0908 90E1
         JSX     *0               ; 0909 2800
D090A    DATA    X'0002'          ; 090A 0002  read by 08E5 0972 0A2A 0A2D ...
D090B    DATA    X'0000'          ; 090B 0000  written by 08E6 08E9; read by 0900
D090C    DATA    X'800A'          ; 090C 800A  LDW X'A'; read by 08EC 08EE 08F0 08F2 ...
D090D    DATA    X'0000'          ; 090D 0000  written by 06DD 08FF 0906 0B4F; read by 08E7 08EA 0903 0B69
D090E    DATA    X'00F8'          ; 090E 00F8  read by 0928
J090F    CLR                      ; 090F 0100
         STW     D099F            ; 0910 719F
         CXA                      ; 0911 0140
         SNE                      ; 0912 0870
         JMP     J091E            ; 0913 111E
         STW     D0B34            ; 0914 7334
         LDW     D091C            ; 0915 811C
         STW     D09A3            ; 0916 71A3
         LLB     1                ; 0917 0601
         STB     D09A2+1          ; 0918 3345
         JSX     S0993            ; 0919 2193
         LDW     D0B34            ; 091A 8334
         JMP     J091E            ; 091B 111E
D091C    DATA    X'C4CE'          ; 091C C4CE  'DN'; read by 0915
J091D    ADD     D0B34            ; 091D A334
J091E    STW     D0B34            ; 091E 7334
         CAX                      ; 091F 0130
         LDW     *0               ; 0920 8800
         SAP                      ; 0921 0810
         JMP     J091D            ; 0922 111D
         CLB     2                ; 0923 0702
         SNE                      ; 0924 0870
         JMP     J0937            ; 0925 1137
J0926    SRC     8                ; 0926 0A48
         SAO                      ; 0927 0830
         AND     D090E            ; 0928 E10E
         SLC     8                ; 0929 0A58
         STW     *0               ; 092A 7800
         SRL     12               ; 092B 0A0C
         SAZ                      ; 092C 0800
         JMP     J091D            ; 092D 111D
         SML     1                ; 092E 0081
         JSX     S0519            ; 092F 2519
J0930    JMP     J0934            ; 0930 1134
J0931    JSX     S0B39            ; 0931 2339
J0932    SML     0                ; 0932 0080
         JSX     S0089            ; 0933 2089
J0934    JSX     S0AA8            ; 0934 22A8
         JSX     S0AE0            ; 0935 22E0
J0936    JMP     J0932            ; 0936 1132
J0937    LDW     *2               ; 0937 8802
         SAM                      ; 0938 0820
         JMP     J093E            ; 0939 113E
         LDW     D0941            ; 093A 8141
         STW     D09A3            ; 093B 71A3
         LDW     *1               ; 093C 8801
         JSX     S09D9            ; 093D 21D9
J093E    LDX     D0B34            ; 093E 9334
         LDW     *0               ; 093F 8800
         JMP     J0926            ; 0940 1126
D0941    DATA    X'CCC1'          ; 0941 CCC1  'LA'; read by 093A
         DATA    X'8156'          ; 0942 8156  LDW X'156'
         DATA    X'1147'          ; 0943 1147  JMP X'147'
         DATA    X'8157'          ; 0944 8157  LDW X'157'
         DATA    X'1147'          ; 0945 1147  JMP X'147'
         DATA    X'8158'          ; 0946 8158  LDW X'158'
         DATA    X'714F'          ; 0947 714F  STW X'14F'
         DATA    X'0080'          ; 0948 0080
         DATA    X'21C3'          ; 0949 21C3  JSX X'1C3'
         DATA    X'0081'          ; 094A 0081
         DATA    X'2774'          ; 094B 2774  JSX X'774'
         DATA    X'0848'          ; 094C 0848
         DATA    X'0081'          ; 094D 0081
         DATA    X'2774'          ; 094E 2774  JSX X'774'
         DATA    X'8000'          ; 094F 8000  LDW 0
         DATA    X'0081'          ; 0950 0081
         DATA    X'2528'          ; 0951 2528  JSX X'528'
         DATA    X'0081'          ; 0952 0081
         DATA    X'2794'          ; 0953 2794  JSX X'794'
         DATA    X'0080'          ; 0954 0080
         DATA    X'112C'          ; 0955 112C  JMP X'12C'
         DATA    X'004E'          ; 0956 004E
         DATA    X'0048'          ; 0957 0048
         DATA    X'0050'          ; 0958 0050
D0959    DATA    X'0000'          ; 0959 0000  written by 095A 09EC; read by 098C
S095A    STX     D0959            ; 095A 6159
J095B    SS3                      ; 095B 08F0
         JMP     J095B            ; 095C 115B  not run
         LDW     D099F            ; 095D 819F
         SAZ                      ; 095E 0800
         JSX     S0977            ; 095F 2177
         LDW     D0AA6            ; 0960 82A6
         STW     D099F            ; 0961 719F
         JSX     S099D            ; 0962 219D
         DATA    X'89D1'          ; 0963 89D1  argument 1 of 0962; LDW *X'1D1'
E0964    LDW     D0A10            ; 0964 8210
         ADD     D0AA6            ; 0965 A2A6
         STW     D0A10            ; 0966 7210
J0967    LDW     D09D1            ; 0967 81D1
         SAP                      ; 0968 0810
         JMP     J0967            ; 0969 1167  not run
         LDX     D09D5            ; 096A 91D5
         DXS     1                ; 096B 0501
         LLB     X'20'            ; 096C 0620
         STB     *1               ; 096D 3801
         CXA                      ; 096E 0140
         SUB     D09C8            ; 096F B1C8
         SAP                      ; 0970 0810
         JMP     J095B            ; 0971 115B  not run
         ADD     D090A            ; 0972 A10A
         SRL     1                ; 0973 0A01
         STW     D09A2            ; 0974 71A2
         JMP     J0982            ; 0975 1182
D0976    DATA    X'0000'          ; 0976 0000  written by 0977; read by 0980
S0977    STX     D0976            ; 0977 6176
         SML     0                ; 0978 0080
         LDW     D0224            ; 0979 8224
         SAP                      ; 097A 0810
         JMP     J0980            ; 097B 1180
         SML     1                ; 097C 0081
         JSX     S0519            ; 097D 2519
         JMP     J0980            ; 097E 1180  not run
J097F    JSX     S0993            ; 097F 2193
J0980    LDX     D0976            ; 0980 9176
         JSX     *0               ; 0981 2800
J0982    LDX     D09C8            ; 0982 91C8
         SML     1                ; 0983 0081
         STX     D07B8            ; 0984 67B8
         LDB     *0               ; 0985 5800
         CLB     X'C3'            ; 0986 07C3
         SNE                      ; 0987 0870
         JMP     J098E            ; 0988 118E
         CLB     X'AA'            ; 0989 07AA
         SNE                      ; 098A 0870
         JMP     J098E            ; 098B 118E  not run
J098C    LDX     D0959            ; 098C 9159
         JSX     *0               ; 098D 2800
J098E    SML     1                ; 098E 0081
         JSX     S0519            ; 098F 2519
         JSX     S0AA8            ; 0990 22A8  not run
J0991    JMP     J095B            ; 0991 115B
D0992    DATA    X'0000'          ; 0992 0000  written by 0995 09D9; read by 099B
S0993    SS0                      ; 0993 08C0
         JMP     *0               ; 0994 1800  not run
         STX     D0992            ; 0995 6192
J0996    JSX     S099D            ; 0996 219D
         DATA    X'89C9'          ; 0997 89C9  argument 1 of 0996; LDW *X'1C9'
J0998    LDW     D09C9            ; 0998 81C9
         SAP                      ; 0999 0810
         JMP     J0998            ; 099A 1198  not run
J099B    LDX     D0992            ; 099B 9192
         JSX     *0               ; 099C 2800
S099D    SML     0                ; 099D 0080  read by 09ED
         JMP     A0044            ; 099E 1044
D099F    DATA    X'0000'          ; 099F 0000  written by 0910 0961 0A18; read by 095D
D09A0    DATA    X'000F'          ; 09A0 000F  read by 0AA1
D09A1    DATA    X'00B0'          ; 09A1 00B0  read by 0AA2
D09A2    DATA    X'0000'          ; 09A2 0000  written by 0918 0974 09E8; read by 0AB2 0AB7
D09A3    DATA    X'0000'          ; 09A3 0000  written by 0916 093B 0B43
D09A4    DATA    X'0000'          ; 09A4 0000  written by 09E4 09E6
D09A5    DATA    X'0000'          ; 09A5 0000  written by 09E0 09E2
D09A6    DATA    X'0000'          ; 09A6 0000  written by 09DC 09DE
         DATA    X'0000'          ; 09A7 0000
         DATA    X'0000'          ; 09A8 0000
         DATA    X'0000'          ; 09A9 0000
         DATA    X'0000'          ; 09AA 0000
         DATA    X'0000'          ; 09AB 0000
         DATA    X'0000'          ; 09AC 0000
         DATA    X'0000'          ; 09AD 0000
         DATA    X'0000'          ; 09AE 0000
         DATA    X'0000'          ; 09AF 0000
         DATA    X'0000'          ; 09B0 0000
         DATA    X'0000'          ; 09B1 0000
         DATA    X'0000'          ; 09B2 0000
         DATA    X'0000'          ; 09B3 0000
         DATA    X'0000'          ; 09B4 0000
         DATA    X'0000'          ; 09B5 0000
         DATA    X'0000'          ; 09B6 0000
         DATA    X'0000'          ; 09B7 0000
         DATA    X'0000'          ; 09B8 0000
         DATA    X'0000'          ; 09B9 0000
         DATA    X'0000'          ; 09BA 0000
         DATA    X'0000'          ; 09BB 0000
         DATA    X'0000'          ; 09BC 0000
         DATA    X'0000'          ; 09BD 0000
         DATA    X'0000'          ; 09BE 0000
         DATA    X'0000'          ; 09BF 0000
         DATA    X'0000'          ; 09C0 0000
         DATA    X'0000'          ; 09C1 0000
         DATA    X'0000'          ; 09C2 0000
         DATA    X'0000'          ; 09C3 0000
         DATA    X'0000'          ; 09C4 0000
         DATA    X'0000'          ; 09C5 0000
         DATA    X'0000'          ; 09C6 0000
         DATA    X'FFFF'          ; 09C7 FFFF  CMW *X'7FF'
D09C8    DATA    X'1346'          ; 09C8 1346  JMP X'346'; read by 096F 0982 0A28
D09C9    DATA    X'09A3'          ; 09C9 09A3  read by 0998
         DATA    X'09A2'          ; 09CA 09A2
         DATA    X'003E'          ; 09CB 003E
         DATA    X'0000'          ; 09CC 0000
         DATA    X'0000'          ; 09CD 0000
         DATA    X'0000'          ; 09CE 0000
         DATA    X'0000'          ; 09CF 0000
         DATA    X'0000'          ; 09D0 0000
D09D1    DATA    X'09A3'          ; 09D1 09A3  read by 0967
         DATA    X'01FA'          ; 09D2 01FA
D09D3    DATA    X'0029'          ; 09D3 0029  written by 0099 00C6
         DATA    X'0000'          ; 09D4 0000
D09D5    DATA    X'0000'          ; 09D5 0000  read by 096A
         DATA    X'0000'          ; 09D6 0000
         DATA    X'0000'          ; 09D7 0000
         DATA    X'0000'          ; 09D8 0000
S09D9    STX     D0992            ; 09D9 6192
         STW     D0AA5            ; 09DA 72A5
         JSX     S0A8A            ; 09DB 228A
J09DC    STB     D09A6+1          ; 09DC 334D
         JSX     S0A8A            ; 09DD 228A
J09DE    STB     D09A6            ; 09DE 334C
         JSX     S0A8A            ; 09DF 228A
J09E0    STB     D09A5+1          ; 09E0 334B
         JSX     S0A8A            ; 09E1 228A
J09E2    STB     D09A5            ; 09E2 334A
         JSX     S0A8A            ; 09E3 228A
J09E4    STB     D09A4+1          ; 09E4 3349
         LLB     X'A0'            ; 09E5 06A0
         STB     D09A4            ; 09E6 3348
         LLB     4                ; 09E7 0604
         STB     D09A2+1          ; 09E8 3345
         SS0                      ; 09E9 08C0
         JMP     J099B            ; 09EA 119B  not run
         JMP     J0996            ; 09EB 1196
J09EC    STX     D0959            ; 09EC 6159  not run
         LDX     S099D            ; 09ED 919D  not run
J09EE    CLR                      ; 09EE 0100  not run
         DOT     14,14            ; 09EF 03EE  not run
J09F0    DIN     14,0             ; 09F0 02E0  not run
         SLL     7                ; 09F1 0A17  not run
         SAM                      ; 09F2 0820  not run
         JMP     J09F0            ; 09F3 11F0  not run
         DXS     1                ; 09F4 0501  not run
         JMP     J09EE            ; 09F5 11EE  not run
         DOT     14,0             ; 09F6 03E0  not run
         JMP     J098C            ; 09F7 118C  not run
J09F8    LDW     *0               ; 09F8 8800
         STW     D0A46            ; 09F9 7246
         SML     0                ; 09FA 0080
         LDW     D0224            ; 09FB 8224
         SAM                      ; 09FC 0820
         JMP     J0A11            ; 09FD 1211  not run
         SML     0                ; 09FE 0080
         LDW     D020A            ; 09FF 820A
         SAP                      ; 0A00 0810
         JMP     J0A11            ; 0A01 1211
         LDW     D0A0C            ; 0A02 820C
         CMW     D0A63            ; 0A03 F263
         SLS                      ; 0A04 0840
         JMP     J0A0E            ; 0A05 120E
         CAX                      ; 0A06 0130
         LDW     D0A10            ; 0A07 8210
         STW     *0               ; 0A08 7800
         LDW     D0A46            ; 0A09 8246
         STW     *1               ; 0A0A 7801
         IXS     2                ; 0A0B 0402
D0A0C    DATA    X'0A4F'          ; 0A0C 0A4F  written by 0091 0A0D; read by 0A02 0B3C
         STX     D0A0C            ; 0A0D 620C
J0A0E    SML     0                ; 0A0E 0080
         JSX     S0144            ; 0A0F 2144
D0A10    DATA    X'0000'          ; 0A10 0000  argument 1 of 0A0F; written by 0094 0966; read by 0964 0A07
J0A11    SML     0                ; 0A11 0080
         LDW     D0205            ; 0A12 8205
         CLB     X'20'            ; 0A13 0720
         SNE                      ; 0A14 0870
         JMP     J0A1B            ; 0A15 121B
         JSX     S0977            ; 0A16 2177
         CLR                      ; 0A17 0100
         STW     D099F            ; 0A18 719F
         JSX     S0A23            ; 0A19 2223
         JMP     J0A0E            ; 0A1A 120E
J0A1B    JSX     S0A23            ; 0A1B 2223
         SML     0                ; 0A1C 0080
         LDW     D0203            ; 0A1D 8203
         SML     0                ; 0A1E 0080
         STW     D0209            ; 0A1F 7209
         SML     0                ; 0A20 0080
         JMP     J00FF            ; 0A21 10FF
D0A22    DATA    X'0000'          ; 0A22 0000  written by 0A23; read by 0A41
S0A23    STX     D0A22            ; 0A23 6222
         LDW     D0A46            ; 0A24 8246
         STW     D0A64            ; 0A25 7264
         SML     0                ; 0A26 0080
         LDW     D01C1            ; 0A27 81C1
         SUB     D09C8            ; 0A28 B1C8
         STW     D0A45            ; 0A29 7245
         ADD     D090A            ; 0A2A A10A
         SRL     1                ; 0A2B 0A01
         STW     D0A89            ; 0A2C 7289
         SUB     D090A            ; 0A2D B10A
         CAX                      ; 0A2E 0130
         LDW     D0A44            ; 0A2F 8244
         SLM                      ; 0A30 0040
J0A31    STW     *X'265'          ; 0A31 7A65
         DXS     1                ; 0A32 0501
         JMP     J0A31            ; 0A33 1231
         SGM                      ; 0A34 0050
         LDW     D0A45            ; 0A35 8245
         ADD     D0A43            ; 0A36 A243
         CAX                      ; 0A37 0130
         LLB     X'DE'            ; 0A38 06DE
         STB     *0               ; 0A39 3800
         LLB     X'A0'            ; 0A3A 06A0
         STB     *1               ; 0A3B 3801
         JSX     S099D            ; 0A3C 219D
         DATA    X'8A47'          ; 0A3D 8A47  argument 1 of 0A3C; LDW *X'247'
J0A3E    LDW     D0A47            ; 0A3E 8247
         SAP                      ; 0A3F 0810
         JMP     J0A3E            ; 0A40 123E  not run
         LDX     D0A22            ; 0A41 9222
         JSX     *0               ; 0A42 2800
D0A43    DATA    X'14C8'          ; 0A43 14C8  JMP X'4C8'; read by 0A36
D0A44    DATA    X'AEAE'          ; 0A44 AEAE  '..'; read by 0A2F
D0A45    DATA    X'0000'          ; 0A45 0000  written by 0A29; read by 0A35
D0A46    DATA    X'0000'          ; 0A46 0000  written by 09F9 0B3A 0B47; read by 0A09 0A24 0B3B
D0A47    DATA    X'0A64'          ; 0A47 0A64  read by 0A3E
         DATA    X'0A89'          ; 0A48 0A89
         DATA    X'003E'          ; 0A49 003E
         DATA    X'0000'          ; 0A4A 0000
         DATA    X'0000'          ; 0A4B 0000
         DATA    X'0000'          ; 0A4C 0000
         DATA    X'0000'          ; 0A4D 0000
         DATA    X'0000'          ; 0A4E 0000
         DATA    X'0000'          ; 0A4F 0000
         DATA    X'0000'          ; 0A50 0000
         DATA    X'0000'          ; 0A51 0000
         DATA    X'0000'          ; 0A52 0000
         DATA    X'0000'          ; 0A53 0000
         DATA    X'0000'          ; 0A54 0000
         DATA    X'0000'          ; 0A55 0000
         DATA    X'0000'          ; 0A56 0000
         DATA    X'0000'          ; 0A57 0000
         DATA    X'0000'          ; 0A58 0000
         DATA    X'0000'          ; 0A59 0000
         DATA    X'0000'          ; 0A5A 0000
         DATA    X'0000'          ; 0A5B 0000
         DATA    X'0000'          ; 0A5C 0000
         DATA    X'0000'          ; 0A5D 0000
         DATA    X'0000'          ; 0A5E 0000
         DATA    X'0000'          ; 0A5F 0000
         DATA    X'0000'          ; 0A60 0000
         DATA    X'0000'          ; 0A61 0000
         DATA    X'0000'          ; 0A62 0000
D0A63    DATA    X'0A63'          ; 0A63 0A63  read by 0A03
D0A64    DATA    X'0000'          ; 0A64 0000  written by 0A25
         DATA    X'0000'          ; 0A65 0000
         DATA    X'0000'          ; 0A66 0000
         DATA    X'0000'          ; 0A67 0000
         DATA    X'0000'          ; 0A68 0000
         DATA    X'0000'          ; 0A69 0000
         DATA    X'0000'          ; 0A6A 0000
         DATA    X'0000'          ; 0A6B 0000
         DATA    X'0000'          ; 0A6C 0000
         DATA    X'0000'          ; 0A6D 0000
         DATA    X'0000'          ; 0A6E 0000
         DATA    X'0000'          ; 0A6F 0000
         DATA    X'0000'          ; 0A70 0000
         DATA    X'0000'          ; 0A71 0000
         DATA    X'0000'          ; 0A72 0000
         DATA    X'0000'          ; 0A73 0000
         DATA    X'0000'          ; 0A74 0000
         DATA    X'0000'          ; 0A75 0000
         DATA    X'0000'          ; 0A76 0000
         DATA    X'0000'          ; 0A77 0000
         DATA    X'0000'          ; 0A78 0000
         DATA    X'0000'          ; 0A79 0000
         DATA    X'0000'          ; 0A7A 0000
         DATA    X'0000'          ; 0A7B 0000
         DATA    X'0000'          ; 0A7C 0000
         DATA    X'0000'          ; 0A7D 0000
         DATA    X'0000'          ; 0A7E 0000
         DATA    X'0000'          ; 0A7F 0000
         DATA    X'0000'          ; 0A80 0000
         DATA    X'0000'          ; 0A81 0000
         DATA    X'0000'          ; 0A82 0000
         DATA    X'0000'          ; 0A83 0000
         DATA    X'0000'          ; 0A84 0000
         DATA    X'0000'          ; 0A85 0000
         DATA    X'0000'          ; 0A86 0000
         DATA    X'0000'          ; 0A87 0000
         DATA    X'0000'          ; 0A88 0000
D0A89    DATA    X'0000'          ; 0A89 0000  written by 0A2C
S0A8A    LDW     D0AA5            ; 0A8A 82A5
         STW     D0AA4            ; 0A8B 72A4
         SAZ                      ; 0A8C 0800
         JMP     J0A90            ; 0A8D 1290
         LLB     X'A0'            ; 0A8E 06A0
         JMP     *0               ; 0A8F 1800
J0A90    SLA     1                ; 0A90 0911
         ADD     D0AA4            ; 0A91 A2A4
         STW     D0AA5            ; 0A92 72A5
         SRA     4                ; 0A93 0904
         ADD     D0AA5            ; 0A94 A2A5
         SRA     4                ; 0A95 0904
         ADD     D0AA5            ; 0A96 A2A5
         SRA     4                ; 0A97 0904
         ADD     D0AA5            ; 0A98 A2A5
         ADD     D0AA6            ; 0A99 A2A6
         SRA     5                ; 0A9A 0905
         STW     D0AA5            ; 0A9B 72A5
         SLL     3                ; 0A9C 0A13
         ADD     D0AA5            ; 0A9D A2A5
         ADD     D0AA5            ; 0A9E A2A5
         CMP                      ; 0A9F 0110
         ADD     D0AA4            ; 0AA0 A2A4
         AND     D09A0            ; 0AA1 E1A0
         ORI     D09A1            ; 0AA2 C1A1
         JMP     *0               ; 0AA3 1800
D0AA4    DATA    X'0000'          ; 0AA4 0000  written by 0A8B; read by 0A91 0AA0
D0AA5    DATA    X'0000'          ; 0AA5 0000  written by 09DA 0A92 0A9B; read by 0A8A 0A94 0A96 0A98 ...
D0AA6    DATA    X'0001'          ; 0AA6 0001  read by 0960 0965 0A99 0AB5 ...
D0AA7    DATA    X'0000'          ; 0AA7 0000  written by 0AA8; read by 0AD1
S0AA8    STX     D0AA7            ; 0AA8 62A7
         SML     0                ; 0AA9 0080
         LDW     D0208            ; 0AAA 8208
         STW     D0AD3            ; 0AAB 72D3
         SML     0                ; 0AAC 0080
         SUB     D0209            ; 0AAD B209
         SUB     D0AD4            ; 0AAE B2D4
         SAP                      ; 0AAF 0810
         JSX     S0AE0            ; 0AB0 22E0  not run
         LDX     D0AD3            ; 0AB1 92D3
         LDW     D09A2            ; 0AB2 81A2
         STW     *0               ; 0AB3 7800
         CXA                      ; 0AB4 0140
         SUB     D0AA6            ; 0AB5 B2A6
         STW     D0AD5            ; 0AB6 72D5
         SUB     D09A2            ; 0AB7 B1A2
         SML     0                ; 0AB8 0080
         STW     D0208            ; 0AB9 7208
         STW     D0AC4            ; 0ABA 72C4
         LDW     D0AD6            ; 0ABB 82D6
         STW     D0AC0            ; 0ABC 72C0
J0ABD    LDX     D0AC0            ; 0ABD 92C0
         LDW     *0               ; 0ABE 8800
         IXS     1                ; 0ABF 0401
D0AC0    DATA    X'0000'          ; 0AC0 0000  written by 0ABC 0AC1; read by 0ABD
         STX     D0AC0            ; 0AC1 62C0
         LDX     D0AC4            ; 0AC2 92C4
         IXS     1                ; 0AC3 0401
D0AC4    DATA    X'0000'          ; 0AC4 0000  written by 0ABA 0AC5; read by 0AC2
         STX     D0AC4            ; 0AC5 62C4
         STW     *0               ; 0AC6 7800
         CXA                      ; 0AC7 0140
         CMW     D0AD5            ; 0AC8 F2D5
         SEQ                      ; 0AC9 0860
         JMP     J0ABD            ; 0ACA 12BD
         LDW     *0               ; 0ACB 8800
         CLB     X'20'            ; 0ACC 0720
         SLLD    1                ; 0ACD 0A31
         LLB     X'A0'            ; 0ACE 06A0
         SNE                      ; 0ACF 0870
         STB     *1               ; 0AD0 3801
         LDX     D0AA7            ; 0AD1 92A7
         JSX     *0               ; 0AD2 2800
D0AD3    DATA    X'0000'          ; 0AD3 0000  written by 0AAB 0AE5 0B06; read by 0AB1 0AEB
D0AD4    DATA    X'0064'          ; 0AD4 0064  read by 0AAE
D0AD5    DATA    X'0000'          ; 0AD5 0000  written by 0AB6; read by 0AC8
D0AD6    DATA    X'09A3'          ; 0AD6 09A3  read by 0ABB
D0AD7    DATA    X'020F'          ; 0AD7 020F  written by 008B
         DATA    X'01F6'          ; 0AD8 01F6
         DATA    X'001B'          ; 0AD9 001B
         DATA    X'0000'          ; 0ADA 0000
         DATA    X'0000'          ; 0ADB 0000
         DATA    X'0000'          ; 0ADC 0000
         DATA    X'0000'          ; 0ADD 0000
         DATA    X'0000'          ; 0ADE 0000
D0ADF    DATA    X'0000'          ; 0ADF 0000  written by 0AE0; read by 0AEC
S0AE0    STX     D0ADF            ; 0AE0 62DF
         SML     0                ; 0AE1 0080
         LDW     D0222            ; 0AE2 8222
         SML     0                ; 0AE3 0080
         LDX     D0208            ; 0AE4 9208
         STX     D0AD3            ; 0AE5 62D3
         SML     0                ; 0AE6 0080
         STW     D0208            ; 0AE7 7208
         STW     D0B03            ; 0AE8 7303
         JSX     S0B07            ; 0AE9 2307
J0AEA    LDW     D0B03            ; 0AEA 8303
         CMW     D0AD3            ; 0AEB F2D3
         LDX     D0ADF            ; 0AEC 92DF
         SGR                      ; 0AED 0880
         JMP     J0B04            ; 0AEE 1304
         CAX                      ; 0AEF 0130
         STW     D0AFC            ; 0AF0 72FC
         SUB     *0               ; 0AF1 B800
         STW     D0AFB            ; 0AF2 72FB
         SUB     D0AA6            ; 0AF3 B2A6
         STW     D0B03            ; 0AF4 7303
         JSX     S099D            ; 0AF5 219D
         DATA    X'8AFB'          ; 0AF6 8AFB  argument 1 of 0AF5; '\n{'
J0AF7    LDW     D0AFB            ; 0AF7 82FB
         SAP                      ; 0AF8 0810
         JMP     J0AF7            ; 0AF9 12F7  not run
         JMP     J0AEA            ; 0AFA 12EA
D0AFB    DATA    X'0000'          ; 0AFB 0000  written by 0AF2; read by 0AF7
D0AFC    DATA    X'0000'          ; 0AFC 0000  written by 0AF0
         DATA    X'005E'          ; 0AFD 005E
         DATA    X'0000'          ; 0AFE 0000
         DATA    X'0000'          ; 0AFF 0000
         DATA    X'0000'          ; 0B00 0000
         DATA    X'0000'          ; 0B01 0000
         DATA    X'0000'          ; 0B02 0000
D0B03    DATA    X'0000'          ; 0B03 0000  written by 0AE8 0AF4; read by 0AEA
J0B04    SML     0                ; 0B04 0080
         LDW     D0208            ; 0B05 8208
         STW     D0AD3            ; 0B06 72D3
; The operator's pause: SENSE 3 up, then down. PREP comes here before it punches and after (errors session).
S0B07    SS3                      ; 0B07 08F0
         JMP     J0B0A            ; 0B08 130A
         JMP     S0B07            ; 0B09 1307  not run
J0B0A    SS3                      ; 0B0A 08F0
         JMP     J0B0A            ; 0B0B 130A  not run
         JMP     *0               ; 0B0C 1800
J0B0D    IXS     0                ; 0B0D 0400
         STW     D0CFD            ; 0B0E 74FD
         LDW     D0B38            ; 0B0F 8338
         SAP                      ; 0B10 0810
         JMP     J0B32            ; 0B11 1332
         LDW     D0B35            ; 0B12 8335
         SML     0                ; 0B13 0080
         LDX     D0209            ; 0B14 9209
         STX     D0B34            ; 0B15 6334
         SAZ                      ; 0B16 0800
         CAX                      ; 0B17 0130
         STX     D0B35            ; 0B18 6335
         STX     D0B36            ; 0B19 6336
J0B1A    LDX     D0B38            ; 0B1A 9338
         LDW     *3               ; 0B1B 8803
         STW     D0B38            ; 0B1C 7338
         SML     0                ; 0B1D 0080
         STX     D020D            ; 0B1E 620D
         LDW     D0B36            ; 0B1F 8336
         STW     *3               ; 0B20 7803
         SML     1                ; 0B21 0081
         JSX     S0413            ; 0B22 2413
         ADD     D0B36            ; 0B23 A336
         STW     D0B36            ; 0B24 7336
         LDW     D0B38            ; 0B25 8338
         SAZ                      ; 0B26 0800
         JMP     J0B1A            ; 0B27 131A
         LDW     D0B36            ; 0B28 8336
         SUB     D0B34            ; 0B29 B334
         SAP                      ; 0B2A 0810
         JMP     J0B32            ; 0B2B 1332
         ORI     D0B37            ; 0B2C C337
         LDX     D0B34            ; 0B2D 9334
         STW     *0               ; 0B2E 7800
         LDW     D0B36            ; 0B2F 8336
         SML     0                ; 0B30 0080
         STW     D0209            ; 0B31 7209
J0B32    SML     0                ; 0B32 0080
         JSX     S01CD            ; 0B33 21CD
D0B34    DATA    X'0000'          ; 0B34 0000  argument 1 of 0B33; written by 0914 091E 0B15; read by 091A 091D 093E 0B29 ...
D0B35    DATA    X'0000'          ; 0B35 0000  argument 2 of 0B33; written by 0082 0B18; read by 0B12
D0B36    DATA    X'0000'          ; 0B36 0000  argument 3 of 0B33; written by 0B19 0B24; read by 0B1F 0B23 0B28 0B2F
D0B37    DATA    X'8000'          ; 0B37 8000  argument 4 of 0B33; LDW 0; read by 0B2C
D0B38    DATA    X'0000'          ; 0B38 0000  argument 5 of 0B33; written by 009F 0B1C; read by 03BD 0B0F 0B1A 0B25
S0B39    LDW     D0B46            ; 0B39 8346
         STW     D0A46            ; 0B3A 7246
J0B3B    LDW     D0A46            ; 0B3B 8246
         CMW     D0A0C            ; 0B3C F20C
         SEQ                      ; 0B3D 0860
         JMP     J0B41            ; 0B3E 1341
         SML     0                ; 0B3F 0080
         JMP     S0089            ; 0B40 1089
J0B41    CAX                      ; 0B41 0130
         LDW     *1               ; 0B42 8801
         STW     D09A3            ; 0B43 71A3
         LDW     *0               ; 0B44 8800
         IXS     2                ; 0B45 0402
D0B46    DATA    X'0A4F'          ; 0B46 0A4F  read by 008F 0B39
         STX     D0A46            ; 0B47 6246
         JSX     S09D9            ; 0B48 21D9
         JMP     J0B3B            ; 0B49 133B
D0B4A    DATA    X'0000'          ; 0B4A 0000  written by 0B4B; read by 0B75
S0B4B    STX     D0B4A            ; 0B4B 634A
         LDW     *0               ; 0B4C 8800
         STW     D0B7F            ; 0B4D 737F
         LDW     *1               ; 0B4E 8801
         STW     D090D            ; 0B4F 710D
         SUB     *0               ; 0B50 B800
         ADD     D0AA6            ; 0B51 A2A6
         STW     D0B87            ; 0B52 7387
         SLL     1                ; 0B53 0A11
         SUB     D090A            ; 0B54 B10A
         STW     D0B89            ; 0B55 7389
         CLR                      ; 0B56 0100
         STW     D0B8A            ; 0B57 738A
         LDX     *0               ; 0B58 9800
         SLLD    1                ; 0B59 0A31
J0B5A    CLR                      ; 0B5A 0100
         LDB     *0               ; 0B5B 5800
         ADD     D0B8A            ; 0B5C A38A
         STW     D0B8A            ; 0B5D 738A
         IXS     1                ; 0B5E 0401
         SLL     0                ; 0B5F 0A10  not run
         LDW     D0B89            ; 0B60 8389
         SUB     D0AA6            ; 0B61 B2A6
         STW     D0B89            ; 0B62 7389
         SAZ                      ; 0B63 0800
         JMP     J0B5A            ; 0B64 135A
         LDW     D0B8A            ; 0B65 838A
         SRL     8                ; 0B66 0A08
         ADD     D0B8A            ; 0B67 A38A
         AND     D0B8B            ; 0B68 E38B
         LDX     D090D            ; 0B69 910D
         STW     *0               ; 0B6A 7800
         JSX     S099D            ; 0B6B 219D
         DATA    X'8B77'          ; 0B6C 8B77  argument 1 of 0B6B; LDW *X'377'
J0B6D    LDW     D0B77            ; 0B6D 8377
         SAP                      ; 0B6E 0810
         JMP     J0B6D            ; 0B6F 136D  not run
         JSX     S099D            ; 0B70 219D
         DATA    X'8B7F'          ; 0B71 8B7F  argument 1 of 0B70; LDW *X'37F'
J0B72    LDW     D0B7F            ; 0B72 837F
         SAP                      ; 0B73 0810
         JMP     J0B72            ; 0B74 1372  not run
         LDX     D0B4A            ; 0B75 934A
         JSX     *2               ; 0B76 2802
D0B77    DATA    X'0B7F'          ; 0B77 0B7F  read by 0B6D
         DATA    X'81F5'          ; 0B78 81F5  LDW X'1F5'
         DATA    X'005E'          ; 0B79 005E
         DATA    X'0000'          ; 0B7A 0000
         DATA    X'0000'          ; 0B7B 0000
         DATA    X'0000'          ; 0B7C 0000
         DATA    X'0000'          ; 0B7D 0000
         DATA    X'0000'          ; 0B7E 0000
D0B7F    DATA    X'0000'          ; 0B7F 0000  written by 0B4D; read by 0B72
         DATA    X'8B87'          ; 0B80 8B87  LDW *X'387'
         DATA    X'005E'          ; 0B81 005E
         DATA    X'0000'          ; 0B82 0000
         DATA    X'0000'          ; 0B83 0000
         DATA    X'0000'          ; 0B84 0000
         DATA    X'0000'          ; 0B85 0000
         DATA    X'0000'          ; 0B86 0000
D0B87    DATA    X'0000'          ; 0B87 0000  written by 0B52
         DATA    X'0B87'          ; 0B88 0B87
D0B89    DATA    X'0000'          ; 0B89 0000  written by 0B55 0B62; read by 0B60
D0B8A    DATA    X'0000'          ; 0B8A 0000  written by 0B57 0B5D; read by 0B5C 0B65 0B67
D0B8B    DATA    X'00FF'          ; 0B8B 00FF  read by 0B68
         DATA    X'0000'          ; 0B8C 0000
         DATA    X'0000'          ; 0B8D 0000
         DATA    X'0000'          ; 0B8E 0000
         DATA    X'0000'          ; 0B8F 0000
         DATA    X'0000'          ; 0B90 0000
         DATA    X'0000'          ; 0B91 0000
         DATA    X'0000'          ; 0B92 0000
         DATA    X'0000'          ; 0B93 0000
         DATA    X'0000'          ; 0B94 0000
         DATA    X'0000'          ; 0B95 0000
         DATA    X'0000'          ; 0B96 0000
         DATA    X'0000'          ; 0B97 0000
         DATA    X'0000'          ; 0B98 0000
         DATA    X'0000'          ; 0B99 0000
         DATA    X'0000'          ; 0B9A 0000
         DATA    X'0000'          ; 0B9B 0000
         DATA    X'0000'          ; 0B9C 0000
         DATA    X'0000'          ; 0B9D 0000
         DATA    X'0000'          ; 0B9E 0000
         DATA    X'0000'          ; 0B9F 0000
         DATA    X'0000'          ; 0BA0 0000
         DATA    X'0000'          ; 0BA1 0000
         DATA    X'0000'          ; 0BA2 0000
         DATA    X'0000'          ; 0BA3 0000
         DATA    X'0000'          ; 0BA4 0000
         DATA    X'0000'          ; 0BA5 0000
         DATA    X'0000'          ; 0BA6 0000
         DATA    X'0000'          ; 0BA7 0000
         DATA    X'0000'          ; 0BA8 0000
         DATA    X'0000'          ; 0BA9 0000
         DATA    X'0000'          ; 0BAA 0000
         DATA    X'0000'          ; 0BAB 0000
         DATA    X'0000'          ; 0BAC 0000
         DATA    X'0000'          ; 0BAD 0000
         DATA    X'0000'          ; 0BAE 0000
         DATA    X'0000'          ; 0BAF 0000
         DATA    X'0000'          ; 0BB0 0000
         DATA    X'0000'          ; 0BB1 0000
         DATA    X'0000'          ; 0BB2 0000
         DATA    X'0000'          ; 0BB3 0000
         DATA    X'0000'          ; 0BB4 0000
         DATA    X'0000'          ; 0BB5 0000
         DATA    X'0000'          ; 0BB6 0000
         DATA    X'0000'          ; 0BB7 0000
         DATA    X'0000'          ; 0BB8 0000
         DATA    X'0000'          ; 0BB9 0000
         DATA    X'0000'          ; 0BBA 0000
         DATA    X'0000'          ; 0BBB 0000
         DATA    X'0000'          ; 0BBC 0000
         DATA    X'0000'          ; 0BBD 0000
         DATA    X'0000'          ; 0BBE 0000
         DATA    X'0000'          ; 0BBF 0000
         DATA    X'0000'          ; 0BC0 0000
         DATA    X'0000'          ; 0BC1 0000
         DATA    X'0000'          ; 0BC2 0000
         DATA    X'0000'          ; 0BC3 0000
         DATA    X'0000'          ; 0BC4 0000
         DATA    X'0000'          ; 0BC5 0000
         DATA    X'0000'          ; 0BC6 0000
         DATA    X'0000'          ; 0BC7 0000
         DATA    X'0000'          ; 0BC8 0000
         DATA    X'0000'          ; 0BC9 0000
         DATA    X'0000'          ; 0BCA 0000
         DATA    X'0000'          ; 0BCB 0000
         DATA    X'0000'          ; 0BCC 0000
         DATA    X'0000'          ; 0BCD 0000
         DATA    X'0000'          ; 0BCE 0000
         DATA    X'0000'          ; 0BCF 0000
         DATA    X'0000'          ; 0BD0 0000
         DATA    X'0000'          ; 0BD1 0000
         DATA    X'0000'          ; 0BD2 0000
         DATA    X'0000'          ; 0BD3 0000
         DATA    X'0000'          ; 0BD4 0000
         DATA    X'0000'          ; 0BD5 0000
         DATA    X'0000'          ; 0BD6 0000
         DATA    X'0000'          ; 0BD7 0000
         DATA    X'0000'          ; 0BD8 0000
         DATA    X'0000'          ; 0BD9 0000
         DATA    X'0000'          ; 0BDA 0000
         DATA    X'0000'          ; 0BDB 0000
         DATA    X'0000'          ; 0BDC 0000
         DATA    X'0000'          ; 0BDD 0000
         DATA    X'0000'          ; 0BDE 0000
         DATA    X'0000'          ; 0BDF 0000
         DATA    X'0000'          ; 0BE0 0000
         DATA    X'0000'          ; 0BE1 0000
         DATA    X'0000'          ; 0BE2 0000
         DATA    X'0000'          ; 0BE3 0000
         DATA    X'0000'          ; 0BE4 0000
         DATA    X'0000'          ; 0BE5 0000
         DATA    X'0000'          ; 0BE6 0000
         DATA    X'0000'          ; 0BE7 0000
         DATA    X'0000'          ; 0BE8 0000
         DATA    X'0000'          ; 0BE9 0000
         DATA    X'0000'          ; 0BEA 0000
         DATA    X'0000'          ; 0BEB 0000
         DATA    X'0000'          ; 0BEC 0000
         DATA    X'0000'          ; 0BED 0000
         DATA    X'0000'          ; 0BEE 0000
         DATA    X'0000'          ; 0BEF 0000
         DATA    X'0000'          ; 0BF0 0000
         DATA    X'0000'          ; 0BF1 0000
         DATA    X'0000'          ; 0BF2 0000
         DATA    X'0000'          ; 0BF3 0000
         DATA    X'0000'          ; 0BF4 0000
         DATA    X'0000'          ; 0BF5 0000
         DATA    X'0000'          ; 0BF6 0000
         DATA    X'0000'          ; 0BF7 0000
         DATA    X'0000'          ; 0BF8 0000
         DATA    X'0000'          ; 0BF9 0000
         DATA    X'0000'          ; 0BFA 0000
         DATA    X'0000'          ; 0BFB 0000
         DATA    X'0000'          ; 0BFC 0000
         DATA    X'0000'          ; 0BFD 0000
         DATA    X'0000'          ; 0BFE 0000
         DATA    X'0000'          ; 0BFF 0000
         DATA    X'0000'          ; 0C00 0000
         DATA    X'0000'          ; 0C01 0000
         DATA    X'0000'          ; 0C02 0000
         DATA    X'0000'          ; 0C03 0000
         DATA    X'0000'          ; 0C04 0000
         DATA    X'0000'          ; 0C05 0000
         DATA    X'0000'          ; 0C06 0000
         DATA    X'0000'          ; 0C07 0000
         DATA    X'0000'          ; 0C08 0000
         DATA    X'0000'          ; 0C09 0000
         DATA    X'0000'          ; 0C0A 0000
         DATA    X'0000'          ; 0C0B 0000
         DATA    X'0000'          ; 0C0C 0000
         DATA    X'0000'          ; 0C0D 0000
         DATA    X'0000'          ; 0C0E 0000
         DATA    X'0000'          ; 0C0F 0000
         DATA    X'0000'          ; 0C10 0000
         DATA    X'0000'          ; 0C11 0000
         DATA    X'0000'          ; 0C12 0000
         DATA    X'0000'          ; 0C13 0000
         DATA    X'0000'          ; 0C14 0000
         DATA    X'0000'          ; 0C15 0000
         DATA    X'0000'          ; 0C16 0000
         DATA    X'0000'          ; 0C17 0000
         DATA    X'0000'          ; 0C18 0000
         DATA    X'0000'          ; 0C19 0000
         DATA    X'0000'          ; 0C1A 0000
         DATA    X'0000'          ; 0C1B 0000
         DATA    X'0000'          ; 0C1C 0000
         DATA    X'0000'          ; 0C1D 0000
         DATA    X'0000'          ; 0C1E 0000
         DATA    X'0000'          ; 0C1F 0000
         DATA    X'0000'          ; 0C20 0000
         DATA    X'0000'          ; 0C21 0000
         DATA    X'0000'          ; 0C22 0000
         DATA    X'0000'          ; 0C23 0000
         DATA    X'0000'          ; 0C24 0000
         DATA    X'0000'          ; 0C25 0000
         DATA    X'0000'          ; 0C26 0000
         DATA    X'0000'          ; 0C27 0000
         DATA    X'0000'          ; 0C28 0000
         DATA    X'0000'          ; 0C29 0000
         DATA    X'0000'          ; 0C2A 0000
         DATA    X'0000'          ; 0C2B 0000
         DATA    X'0000'          ; 0C2C 0000
         DATA    X'0000'          ; 0C2D 0000
         DATA    X'0000'          ; 0C2E 0000
         DATA    X'0000'          ; 0C2F 0000
         DATA    X'0000'          ; 0C30 0000
         DATA    X'0000'          ; 0C31 0000
         DATA    X'0000'          ; 0C32 0000
         DATA    X'0000'          ; 0C33 0000
         DATA    X'0000'          ; 0C34 0000
         DATA    X'0000'          ; 0C35 0000
         DATA    X'0000'          ; 0C36 0000
         DATA    X'0000'          ; 0C37 0000
         DATA    X'0000'          ; 0C38 0000
         DATA    X'0000'          ; 0C39 0000
         DATA    X'0000'          ; 0C3A 0000
         DATA    X'0000'          ; 0C3B 0000
         DATA    X'0000'          ; 0C3C 0000
         DATA    X'0000'          ; 0C3D 0000
         DATA    X'0000'          ; 0C3E 0000
         DATA    X'0000'          ; 0C3F 0000
         DATA    X'0000'          ; 0C40 0000
         DATA    X'0000'          ; 0C41 0000
         DATA    X'0000'          ; 0C42 0000
         DATA    X'0000'          ; 0C43 0000
         DATA    X'0000'          ; 0C44 0000
         DATA    X'0000'          ; 0C45 0000
         DATA    X'0000'          ; 0C46 0000
         DATA    X'0000'          ; 0C47 0000
         DATA    X'0000'          ; 0C48 0000
         DATA    X'0000'          ; 0C49 0000
         DATA    X'0000'          ; 0C4A 0000
         DATA    X'0000'          ; 0C4B 0000
         DATA    X'0000'          ; 0C4C 0000
         DATA    X'0000'          ; 0C4D 0000
         DATA    X'0000'          ; 0C4E 0000
         DATA    X'0000'          ; 0C4F 0000
         DATA    X'0000'          ; 0C50 0000
         DATA    X'0000'          ; 0C51 0000
         DATA    X'0000'          ; 0C52 0000
         DATA    X'0000'          ; 0C53 0000
         DATA    X'0000'          ; 0C54 0000
         DATA    X'0000'          ; 0C55 0000
         DATA    X'0000'          ; 0C56 0000
         DATA    X'0000'          ; 0C57 0000
         DATA    X'0000'          ; 0C58 0000
         DATA    X'0000'          ; 0C59 0000
         DATA    X'0000'          ; 0C5A 0000
         DATA    X'0000'          ; 0C5B 0000
         DATA    X'0000'          ; 0C5C 0000
         DATA    X'0000'          ; 0C5D 0000
         DATA    X'0000'          ; 0C5E 0000
         DATA    X'0000'          ; 0C5F 0000
         DATA    X'0000'          ; 0C60 0000
         DATA    X'0000'          ; 0C61 0000
         DATA    X'0000'          ; 0C62 0000
         DATA    X'0000'          ; 0C63 0000
         DATA    X'0000'          ; 0C64 0000
         DATA    X'0000'          ; 0C65 0000
         DATA    X'0000'          ; 0C66 0000
         DATA    X'0000'          ; 0C67 0000
         DATA    X'0000'          ; 0C68 0000
         DATA    X'0000'          ; 0C69 0000
         DATA    X'0000'          ; 0C6A 0000
         DATA    X'0000'          ; 0C6B 0000
         DATA    X'0000'          ; 0C6C 0000
         DATA    X'0000'          ; 0C6D 0000
         DATA    X'0000'          ; 0C6E 0000
         DATA    X'0000'          ; 0C6F 0000
         DATA    X'0000'          ; 0C70 0000
         DATA    X'0000'          ; 0C71 0000
         DATA    X'0000'          ; 0C72 0000
         DATA    X'0000'          ; 0C73 0000
         DATA    X'0000'          ; 0C74 0000
         DATA    X'0000'          ; 0C75 0000
         DATA    X'0000'          ; 0C76 0000
         DATA    X'0000'          ; 0C77 0000
         DATA    X'0000'          ; 0C78 0000
         DATA    X'0000'          ; 0C79 0000
         DATA    X'0000'          ; 0C7A 0000
         DATA    X'0000'          ; 0C7B 0000
         DATA    X'0000'          ; 0C7C 0000
         DATA    X'0000'          ; 0C7D 0000
         DATA    X'0000'          ; 0C7E 0000
         DATA    X'0000'          ; 0C7F 0000
         DATA    X'0000'          ; 0C80 0000
         DATA    X'0000'          ; 0C81 0000
         DATA    X'0000'          ; 0C82 0000
         DATA    X'0000'          ; 0C83 0000
         DATA    X'0000'          ; 0C84 0000
         DATA    X'0000'          ; 0C85 0000
         DATA    X'0000'          ; 0C86 0000
         DATA    X'0000'          ; 0C87 0000
         DATA    X'0000'          ; 0C88 0000
         DATA    X'0000'          ; 0C89 0000
         DATA    X'0000'          ; 0C8A 0000
         DATA    X'0000'          ; 0C8B 0000
         DATA    X'0000'          ; 0C8C 0000
         DATA    X'0000'          ; 0C8D 0000
         DATA    X'0000'          ; 0C8E 0000
         DATA    X'0000'          ; 0C8F 0000
         DATA    X'0000'          ; 0C90 0000
         DATA    X'0000'          ; 0C91 0000
         DATA    X'0000'          ; 0C92 0000
         DATA    X'0000'          ; 0C93 0000
         DATA    X'0000'          ; 0C94 0000
         DATA    X'0000'          ; 0C95 0000
         DATA    X'0000'          ; 0C96 0000
         DATA    X'0000'          ; 0C97 0000
         DATA    X'0000'          ; 0C98 0000
         DATA    X'0000'          ; 0C99 0000
         DATA    X'0000'          ; 0C9A 0000
         DATA    X'0000'          ; 0C9B 0000
         DATA    X'0000'          ; 0C9C 0000
         DATA    X'0000'          ; 0C9D 0000
         DATA    X'0000'          ; 0C9E 0000
         DATA    X'0000'          ; 0C9F 0000
         DATA    X'0000'          ; 0CA0 0000
         DATA    X'0000'          ; 0CA1 0000
         DATA    X'0000'          ; 0CA2 0000
         DATA    X'0000'          ; 0CA3 0000
         DATA    X'0000'          ; 0CA4 0000
         DATA    X'0000'          ; 0CA5 0000
         DATA    X'0000'          ; 0CA6 0000
         DATA    X'0000'          ; 0CA7 0000
         DATA    X'0000'          ; 0CA8 0000
         DATA    X'0000'          ; 0CA9 0000
         DATA    X'0000'          ; 0CAA 0000
         DATA    X'0000'          ; 0CAB 0000
         DATA    X'0000'          ; 0CAC 0000
         DATA    X'0000'          ; 0CAD 0000
         DATA    X'0000'          ; 0CAE 0000
         DATA    X'0000'          ; 0CAF 0000
         DATA    X'0000'          ; 0CB0 0000
         DATA    X'0000'          ; 0CB1 0000
         DATA    X'0000'          ; 0CB2 0000
         DATA    X'0000'          ; 0CB3 0000
         DATA    X'0000'          ; 0CB4 0000
         DATA    X'0000'          ; 0CB5 0000
         DATA    X'0000'          ; 0CB6 0000
         DATA    X'0000'          ; 0CB7 0000
         DATA    X'0000'          ; 0CB8 0000
         DATA    X'0000'          ; 0CB9 0000
         DATA    X'0000'          ; 0CBA 0000
         DATA    X'0000'          ; 0CBB 0000
         DATA    X'0000'          ; 0CBC 0000
         DATA    X'0000'          ; 0CBD 0000
         DATA    X'0000'          ; 0CBE 0000
         DATA    X'0000'          ; 0CBF 0000
         DATA    X'0000'          ; 0CC0 0000
         DATA    X'0000'          ; 0CC1 0000
         DATA    X'0000'          ; 0CC2 0000
         DATA    X'0000'          ; 0CC3 0000
         DATA    X'0000'          ; 0CC4 0000
         DATA    X'0000'          ; 0CC5 0000
         DATA    X'0000'          ; 0CC6 0000
         DATA    X'0000'          ; 0CC7 0000
         DATA    X'0000'          ; 0CC8 0000
         DATA    X'0000'          ; 0CC9 0000
         DATA    X'0000'          ; 0CCA 0000
         DATA    X'0000'          ; 0CCB 0000
         DATA    X'0000'          ; 0CCC 0000
         DATA    X'0000'          ; 0CCD 0000
         DATA    X'0000'          ; 0CCE 0000
         DATA    X'0000'          ; 0CCF 0000
         DATA    X'0000'          ; 0CD0 0000
         DATA    X'0000'          ; 0CD1 0000
         DATA    X'0000'          ; 0CD2 0000
         DATA    X'0000'          ; 0CD3 0000
         DATA    X'0000'          ; 0CD4 0000
         DATA    X'0000'          ; 0CD5 0000
         DATA    X'0000'          ; 0CD6 0000
         DATA    X'0000'          ; 0CD7 0000
         DATA    X'0000'          ; 0CD8 0000
         DATA    X'0000'          ; 0CD9 0000
         DATA    X'0000'          ; 0CDA 0000
         DATA    X'0000'          ; 0CDB 0000
         DATA    X'0000'          ; 0CDC 0000
         DATA    X'0000'          ; 0CDD 0000
         DATA    X'0000'          ; 0CDE 0000
         DATA    X'0000'          ; 0CDF 0000
         DATA    X'0000'          ; 0CE0 0000
         DATA    X'0000'          ; 0CE1 0000
         DATA    X'0000'          ; 0CE2 0000
         DATA    X'0000'          ; 0CE3 0000
         DATA    X'0000'          ; 0CE4 0000
         DATA    X'0000'          ; 0CE5 0000
         DATA    X'0000'          ; 0CE6 0000
         DATA    X'0000'          ; 0CE7 0000
         DATA    X'0000'          ; 0CE8 0000
         DATA    X'0000'          ; 0CE9 0000
         DATA    X'0000'          ; 0CEA 0000
         DATA    X'0000'          ; 0CEB 0000
         DATA    X'0000'          ; 0CEC 0000
         DATA    X'0000'          ; 0CED 0000
         DATA    X'0000'          ; 0CEE 0000
         DATA    X'0000'          ; 0CEF 0000
         DATA    X'0000'          ; 0CF0 0000
         DATA    X'0000'          ; 0CF1 0000
         DATA    X'0000'          ; 0CF2 0000
         DATA    X'0000'          ; 0CF3 0000
         DATA    X'0000'          ; 0CF4 0000
         DATA    X'0000'          ; 0CF5 0000
         DATA    X'0000'          ; 0CF6 0000
         DATA    X'0000'          ; 0CF7 0000
         DATA    X'0000'          ; 0CF8 0000
         DATA    X'0000'          ; 0CF9 0000
         DATA    X'0000'          ; 0CFA 0000
         DATA    X'0000'          ; 0CFB 0000
         DATA    X'0000'          ; 0CFC 0000
D0CFD    DATA    X'0000'          ; 0CFD 0000  written by 0B0E
         DATA    X'2101'          ; 0CFE 2101  JSX X'101'
         DATA    X'0001'          ; 0CFF 0001
         DATA    X'2101'          ; 0D00 2101  JSX X'101'
         DATA    X'0003'          ; 0D01 0003
         DATA    X'0000'          ; 0D02 0000
         DATA    X'0000'          ; 0D03 0000
         DATA    X'00E7'          ; 0D04 00E7
         END
