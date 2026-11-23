; Conversational FORTRAN, the run-time library, DN 394002 rev B
;
; Written by tools/dis703.py. A line marked "not run" is an
; instruction only by being reached from one that ran.

A0000    EQU     X'0000'
A0040    EQU     X'0040'
A0044    EQU     X'0044'
A0CFD    EQU     X'0CFD'

         ORG     X'0080'
; Where the library starts: PTIOS's loader enters it at X'80', the tape's last record being 8080.
; It is loaded over the compiler and runs the program the compiler left from X'D00' up.
; The program it runs is interpreted: it sits from X'CFD' up, where the compiler's B punches it from, and no word past
; the library's last ran while the library ran one (all the sessions).
E0080    SML     2                ; 0080 0082
         JMP     J0AD5            ; 0081 12D5
         DATA    X'10FF'          ; 0082 10FF  JMP X'FF'
         DATA    X'0082'          ; 0083 0082
         DATA    X'2063'          ; 0084 2063  JSX X'63'
         DATA    X'00AA'          ; 0085 00AA
         DATA    X'2110'          ; 0086 2110  JSX X'110'
         DATA    X'00AA'          ; 0087 00AA
         DATA    X'210E'          ; 0088 210E  JSX X'10E'
         DATA    X'00AA'          ; 0089 00AA
         DATA    X'21A3'          ; 008A 21A3  JSX X'1A3'
         DATA    X'00AA'          ; 008B 00AA
         DATA    X'219D'          ; 008C 219D  JSX X'19D'
         DATA    X'00E5'          ; 008D 00E5
         DATA    X'2273'          ; 008E 2273  JSX X'273'
         DATA    X'00E5'          ; 008F 00E5
         DATA    X'22DE'          ; 0090 22DE  JSX X'2DE'
         DATA    X'00E5'          ; 0091 00E5
         DATA    X'22DC'          ; 0092 22DC  JSX X'2DC'
         DATA    X'00E5'          ; 0093 00E5
         DATA    X'235B'          ; 0094 235B  JSX X'35B'
         DATA    X'00E5'          ; 0095 00E5
         DATA    X'23DE'          ; 0096 23DE  JSX X'3DE'
         DATA    X'00E5'          ; 0097 00E5
         DATA    X'245F'          ; 0098 245F  JSX X'45F'
         DATA    X'0BBD'          ; 0099 0BBD
         DATA    X'0000'          ; 009A 0000
         DATA    X'0B11'          ; 009B 0B11
         DATA    X'0083'          ; 009C 0083
         DATA    X'24D3'          ; 009D 24D3  JSX X'4D3'
         DATA    X'0B17'          ; 009E 0B17
J009F    JMP     J0698            ; 009F 1698
         DATA    X'0B11'          ; 00A0 0B11
         DATA    X'0083'          ; 00A1 0083
         DATA    X'24D5'          ; 00A2 24D5  JSX X'4D5'
         DATA    X'0B17'          ; 00A3 0B17
J00A4    JMP     J0661            ; 00A4 1661
         DATA    X'0B1D'          ; 00A5 0B1D
J00A6    JMP     J0782            ; 00A6 1782
         DATA    X'0B1D'          ; 00A7 0B1D
J00A8    JMP     J06D0            ; 00A8 16D0
         DATA    X'0729'          ; 00A9 0729
J00AA    SML     2                ; 00AA 0082
         LDX     D09E9            ; 00AB 91E9
         LDW     *1               ; 00AC 8801
         STW     D00C5            ; 00AD 70C5
         SML     2                ; 00AE 0082
         LDX     D09E8            ; 00AF 91E8
         DXS     2                ; 00B0 0502
         LDX     *0               ; 00B1 9800
         IXS     0                ; 00B2 0400
         LDX     *0               ; 00B3 9800
         LDW     *0               ; 00B4 8800
         AND     D0522            ; 00B5 E522
         STW     D052A            ; 00B6 752A
         LDW     *1               ; 00B7 8801
         STW     D052B            ; 00B8 752B
         LDW     *2               ; 00B9 8802
         STW     D052C            ; 00BA 752C
         SML     2                ; 00BB 0082
         LDX     D09E8            ; 00BC 91E8
         DXS     3                ; 00BD 0503
         LDX     *0               ; 00BE 9800
         IXS     0                ; 00BF 0400
         LDX     *0               ; 00C0 9800
         LDW     *0               ; 00C1 8800
         AND     D0522            ; 00C2 E522
         STW     *0               ; 00C3 7800
         STX     D00C6            ; 00C4 60C6
D00C5    DATA    X'0000'          ; 00C5 0000  run: an instruction stored here; written by 00AD
D00C6    DATA    X'0000'          ; 00C6 0000  written by 00C4
J00C7    SML     2                ; 00C7 0082
         LDX     D09E8            ; 00C8 91E8
         DXS     1                ; 00C9 0501
         LDX     *0               ; 00CA 9800
         IXS     0                ; 00CB 0400
         LDX     *0               ; 00CC 9800
J00CD    LDW     D052A            ; 00CD 852A
         STW     *0               ; 00CE 7800
         LDW     D052B            ; 00CF 852B
         STW     *1               ; 00D0 7801
         LDW     D052C            ; 00D1 852C
         STW     *2               ; 00D2 7802
         SML     2                ; 00D3 0082
         LDW     D09E9            ; 00D4 81E9
         SAP                      ; 00D5 0810
         JMP     J00D9            ; 00D6 10D9
J00D7    SML     2                ; 00D7 0082
         JMP     J0B02            ; 00D8 1302
J00D9    SML     2                ; 00D9 0082
         LDW     D0AD3            ; 00DA 82D3
         SAM                      ; 00DB 0820
         JMP     J00D7            ; 00DC 10D7
         SML     2                ; 00DD 0082
         STX     D09EA            ; 00DE 61EA
         JSX     S05DA            ; 00DF 25DA
         SML     2                ; 00E0 0082
         JSX     S0BA4            ; 00E1 23A4
         DATA    X'0646'          ; 00E2 0646  argument 1 of 00E1
         DATA    X'0007'          ; 00E3 0007  argument 2 of 00E1
         JMP     J00D7            ; 00E4 10D7
J00E5    SML     2                ; 00E5 0082
         LDX     D09E9            ; 00E6 91E9
         LDW     *1               ; 00E7 8801
         STW     D00F3            ; 00E8 70F3
         SML     2                ; 00E9 0082
         LDX     D09E8            ; 00EA 91E8
         DXS     3                ; 00EB 0503
J00EC    LDX     *0               ; 00EC 9800
         IXS     0                ; 00ED 0400
         JMP     J00EC            ; 00EE 10EC  not run
         LDW     *0               ; 00EF 8800
         AND     D0522            ; 00F0 E522
         STW     *0               ; 00F1 7800
         STX     D00F4            ; 00F2 60F4
D00F3    DATA    X'0000'          ; 00F3 0000  run: an instruction stored here; written by 00E8
D00F4    DATA    X'0000'          ; 00F4 0000  written by 00F2
J00F5    SML     2                ; 00F5 0082
         LDX     D09E8            ; 00F6 91E8
         DXS     2                ; 00F7 0502
J00F8    LDX     *0               ; 00F8 9800
         IXS     0                ; 00F9 0400
         JMP     J00F8            ; 00FA 10F8  not run
         STX     D00FD            ; 00FB 60FD
         JSX     S04CA            ; 00FC 24CA
D00FD    DATA    X'8000'          ; 00FD 8000  argument 1 of 00FC; LDW 0; written by 00FB
J00FE    JMP     J00D7            ; 00FE 10D7
         DATA    X'0082'          ; 00FF 0082
         DATA    X'11EE'          ; 0100 11EE  JMP X'1EE'
D0101    DATA    X'E000'          ; 0101 E000  AND 0; read by 0663
D0102    DATA    X'B000'          ; 0102 B000  SUB 0; read by 069A
D0103    DATA    X'0028'          ; 0103 0028  read by 0711
D0104    DATA    X'0000'          ; 0104 0000  written by 0762; read by 067F 06A1 06AE 06BD ...
D0105    DATA    X'0000'          ; 0105 0000  written by 0680; read by 0682 06D7 078E
D0106    DATA    X'0000'          ; 0106 0000  written by 066D 0763; read by 075D
D0107    DATA    X'24EA'          ; 0107 24EA  JSX X'4EA'; read by 0110
D0108    DATA    X'24FC'          ; 0108 24FC  JSX X'4FC'; read by 010E
S0109    DXS     1                ; 0109 0501
         JMP     J010B            ; 010A 110B
J010B    STX     D0530            ; 010B 6530
         LDW     D052B            ; 010C 852B
         JMP     J014E            ; 010D 114E
S010E    LDW     D0108            ; 010E 8108
         JMP     J0111            ; 010F 1111
S0110    LDW     D0107            ; 0110 8107
J0111    STW     D014A            ; 0111 714A
         STX     D0530            ; 0112 6530
         LDX     *0               ; 0113 9800
         LDW     *1               ; 0114 8801
         STW     D0533            ; 0115 7533
         LDW     *2               ; 0116 8802
         STW     D0534            ; 0117 7534
         LDW     D052A            ; 0118 852A
         SUB     *0               ; 0119 B800
         SAP                      ; 011A 0810
         JMP     J011E            ; 011B 111E
         LDX     D014B            ; 011C 914B
         JMP     J0122            ; 011D 1122
J011E    LDX     *0               ; 011E 9800
         STX     D052A            ; 011F 652A
         LDX     D016D            ; 0120 916D
         CMP                      ; 0121 0110
J0122    STX     D0536            ; 0122 6536
         CAX                      ; 0123 0130
         DXS     X'10'            ; 0124 0510
         JMP     J0128            ; 0125 1128
         LLB     X'20'            ; 0126 0620
         JMP     J012B            ; 0127 112B
J0128    LLB     X'2F'            ; 0128 062F
         DXS     X'F'             ; 0129 050F
         JMP     J012C            ; 012A 112C
J012B    IXS     X'30'            ; 012B 0430
J012C    CAX                      ; 012C 0130
         STB     D013A+1          ; 012D 3275
         CXA                      ; 012E 0140
         STB     D013B+1          ; 012F 3277
         LDX     D0536            ; 0130 9536
         LDW     *0               ; 0131 8800
         LDX     *1               ; 0132 9801
         SAP                      ; 0133 0810
         DXS     1                ; 0134 0501
         JMP     J0137            ; 0135 1137
         SUB     D0520            ; 0136 B520
J0137    STW     D0535            ; 0137 7535
         SLLD    1                ; 0138 0A31
         LDW     D0535            ; 0139 8535
D013A    SRAD    0                ; 013A 0920  written by 012D
D013B    SRAD    0                ; 013B 0920  written by 012F
         SAP                      ; 013C 0810
         IXS     2                ; 013D 0402
         JMP     J0144            ; 013E 1144
         STX     D0535            ; 013F 6535
         DXS     2                ; 0140 0502
         JMP     J0145            ; 0141 1145
         ADD     D0520            ; 0142 A520
         JMP     J0145            ; 0143 1145
J0144    STX     D0535            ; 0144 6535
J0145    LDX     D0536            ; 0145 9536
         STW     *0               ; 0146 7800
         LDW     D0535            ; 0147 8535
         SRL     1                ; 0148 0A01
         STW     *1               ; 0149 7801
D014A    JSX     A0000            ; 014A 2000  written by 0111
D014B    DATA    X'0533'          ; 014B 0533  argument 1 of 014A; read by 011C
J014C    SNO                      ; 014C 08A0
         JMP     J0184            ; 014D 1184
J014E    SAM                      ; 014E 0820
         JMP     J0156            ; 014F 1156
         LDX     D052C            ; 0150 952C
         DXS     1                ; 0151 0501
         ADD     D0520            ; 0152 A520
         CMP                      ; 0153 0110
         SNO                      ; 0154 08A0
         JMP     J0199            ; 0155 1199  not run
J0156    SAZ                      ; 0156 0800
         JMP     J0161            ; 0157 1161
         LDW     D052B            ; 0158 852B
         AND     D051E            ; 0159 E51E
         ORI     D052C            ; 015A C52C
         LDX     D0525            ; 015B 9525
         STX     D052C            ; 015C 652C
         STW     D052B            ; 015D 752B
         SAZ                      ; 015E 0800
         JMP     J0166            ; 015F 1166  not run
         JMP     J0194            ; 0160 1194
J0161    SLL     1                ; 0161 0A11
         SAP                      ; 0162 0810
         JMP     J0182            ; 0163 1182
         LDX     D052A            ; 0164 952A
         JMP     J0172            ; 0165 1172
J0166    SAP                      ; 0166 0810  not run
         CMP                      ; 0167 0110  not run
         LDX     D052A            ; 0168 952A  not run
         DXS     X'F'             ; 0169 050F  not run
         JMP     J016E            ; 016A 116E  not run
J016B    JSX     S0495            ; 016B 2495  not run
         DATA    X'1182'          ; 016C 1182  JMP X'182'
D016D    DATA    X'052B'          ; 016D 052B  read by 0120
J016E    STX     D052A            ; 016E 652A  not run
J016F    SLL     1                ; 016F 0A11
         SAP                      ; 0170 0810
         JMP     J0175            ; 0171 1175
J0172    DXS     1                ; 0172 0501
         JMP     J016F            ; 0173 116F
         JMP     J016B            ; 0174 116B  not run
J0175    LLB     X'30'            ; 0175 0630
         ADD     D052A            ; 0176 A52A
         STX     D052A            ; 0177 652A
         SUB     D052A            ; 0178 B52A
         STB     D017D+1          ; 0179 32FB
         LDX     D052C            ; 017A 952C
         SLLD    1                ; 017B 0A31
         LDW     D052B            ; 017C 852B
D017D    SLAD    0                ; 017D 0930  written by 0179
         STW     D052B            ; 017E 752B
         CXA                      ; 017F 0140
         SRL     1                ; 0180 0A01
         STW     D052C            ; 0181 752C
J0182    LDX     D0530            ; 0182 9530
         JMP     *1               ; 0183 1801
J0184    LDX     D052C            ; 0184 952C
         SAP                      ; 0185 0810
         JMP     J018B            ; 0186 118B
         IXS     1                ; 0187 0401  not run
         ADD     D0520            ; 0188 A520  not run
         IXS     0                ; 0189 0400  not run
         LDX     D0525            ; 018A 9525  not run
J018B    SRAD    1                ; 018B 0921
         ORE     D051E            ; 018C D51E
         STW     D052B            ; 018D 752B
         CXA                      ; 018E 0140
         SAP                      ; 018F 0810
         SUB     D0524            ; 0190 B524
         STW     D052C            ; 0191 752C
J0192    LDW     D052A            ; 0192 852A
         ADD     D0520            ; 0193 A520
J0194    STW     D052A            ; 0194 752A
         LLB     0                ; 0195 0600
         SAZ                      ; 0196 0800
         JSX     S04DF            ; 0197 24DF  not run
         JMP     J0182            ; 0198 1182
J0199    SRA     1                ; 0199 0901  not run
         STW     D052B            ; 019A 752B  not run
         JMP     J0192            ; 019B 1192  not run
D019C    DATA    X'8031'          ; 019C 8031  LDW X'31'; read by 0216
S019D    STX     D0530            ; 019D 6530
         LDX     *0               ; 019E 9800
         LDW     D0526            ; 019F 8526
         STW     D052F            ; 01A0 752F
         SUB     *0               ; 01A1 B800
         JMP     J01A8            ; 01A2 11A8
S01A3    STX     D0530            ; 01A3 6530
         LDX     *0               ; 01A4 9800
         LDW     D0528            ; 01A5 8528
         STW     D052F            ; 01A6 752F
         ADD     *0               ; 01A7 A800
J01A8    ADD     D052A            ; 01A8 A52A
         STW     D052A            ; 01A9 752A
         LDW     *1               ; 01AA 8801
         LDX     *2               ; 01AB 9802
         JMP     J01B3            ; 01AC 11B3
S01AD    LDW     D0520            ; 01AD 8520
         STW     D052F            ; 01AE 752F
         STX     D0530            ; 01AF 6530
         LDX     *0               ; 01B0 9800
         LDW     *0               ; 01B1 8800
         LDX     *1               ; 01B2 9801
J01B3    ORE     D052B            ; 01B3 D52B
         STW     D0537            ; 01B4 7537
         ORE     D052B            ; 01B5 D52B
         SAP                      ; 01B6 0810
         JMP     J01E1            ; 01B7 11E1  not run
         STW     D0535            ; 01B8 7535
         STX     D0536            ; 01B9 6536
J01BA    LDW     D052B            ; 01BA 852B
         SAP                      ; 01BB 0810
         JSX     S050B            ; 01BC 250B
J01BD    LDX     D052F            ; 01BD 952F
         DXS     1                ; 01BE 0501
         JMP     J020C            ; 01BF 120C
         STW     D0533            ; 01C0 7533
         LDW     D052C            ; 01C1 852C
         AND     D0536            ; 01C2 E536
         STW     D052B            ; 01C3 752B
         LDW     D0535            ; 01C4 8535
         JSX     S0518            ; 01C5 2518
J01C6    LDX     D052B            ; 01C6 952B
         STW     D052B            ; 01C7 752B
         STX     D0534            ; 01C8 6534
         LDW     D0533            ; 01C9 8533
         STW     D052C            ; 01CA 752C
         LDW     D0536            ; 01CB 8536
         JSX     S0518            ; 01CC 2518
J01CD    LDX     D0533            ; 01CD 9533
         STW     D0533            ; 01CE 7533
         STX     D052C            ; 01CF 652C
         LDW     D0535            ; 01D0 8535
         JSX     S0518            ; 01D1 2518
J01D2    ADD     D0534            ; 01D2 A534
         AND     D051F            ; 01D3 E51F
         STW     D052C            ; 01D4 752C
         LDW     D052B            ; 01D5 852B
         SNO                      ; 01D6 08A0
         ADD     D0520            ; 01D7 A520
J01D8    STW     D052B            ; 01D8 752B
         LDW     D052F            ; 01D9 852F
         SAO                      ; 01DA 0830
         JMP     J01EA            ; 01DB 11EA
J01DC    LDW     D0537            ; 01DC 8537
         SAP                      ; 01DD 0810
         JSX     S050B            ; 01DE 250B
J01DF    LDX     D0530            ; 01DF 9530
         JMP     *1               ; 01E0 1801
J01E1    DXS     1                ; 01E1 0501  not run
         ADD     D0520            ; 01E2 A520  not run
         CMP                      ; 01E3 0110  not run
         STW     D0535            ; 01E4 7535  not run
         CXA                      ; 01E5 0140  not run
         INV                      ; 01E6 0120  not run
         AND     D051F            ; 01E7 E51F  not run
         STW     D0536            ; 01E8 7536  not run
         JMP     J01BA            ; 01E9 11BA  not run
J01EA    LDX     D052C            ; 01EA 952C
         SLLD    1                ; 01EB 0A31
         LDW     D052B            ; 01EC 852B
         SAP                      ; 01ED 0810
         JMP     J01F5            ; 01EE 11F5
         SLLD    1                ; 01EF 0A31
         SAP                      ; 01F0 0810
         JMP     J0204            ; 01F1 1204
         SAZ                      ; 01F2 0800
         JMP     J01FB            ; 01F3 11FB
         JMP     J0203            ; 01F4 1203
J01F5    SRLD    1                ; 01F5 0A21
         STW     D052B            ; 01F6 752B
         CLR                      ; 01F7 0100
         SRLD    1                ; 01F8 0A21
         LDW     D0520            ; 01F9 8520
         JMP     J0201            ; 01FA 1201
J01FB    STW     D052B            ; 01FB 752B
         CLR                      ; 01FC 0100
         SRCD    3                ; 01FD 0A63
         ORI     D0533            ; 01FE C533
         SLCD    2                ; 01FF 0A72
         LDW     D0527            ; 0200 8527
J0201    STX     D052C            ; 0201 652C
         ADD     D052A            ; 0202 A52A
J0203    STW     D052A            ; 0203 752A
J0204    LDW     D052A            ; 0204 852A
         LLB     0                ; 0205 0600
         SAZ                      ; 0206 0800
J0207    JSX     S0209            ; 0207 2209  not run
         JMP     J01DC            ; 0208 11DC
S0209    SAP                      ; 0209 0810  not run
         JMP     S0495            ; 020A 1495  not run
         JMP     S04DF            ; 020B 14DF  not run
J020C    LDW     D0535            ; 020C 8535
         SLL     1                ; 020D 0A11
         SAP                      ; 020E 0810
         JMP     J022F            ; 020F 122F
         SXE                      ; 0210 0850  not run
         JMP     J0207            ; 0211 1207  not run
         SAZ                      ; 0212 0800  not run
         JMP     J0216            ; 0213 1216  not run
         LLB     X'3F'            ; 0214 063F  not run
         JMP     J021D            ; 0215 121D  not run
J0216    LDX     D019C            ; 0216 919C  not run
J0217    SLL     1                ; 0217 0A11  not run
         SAP                      ; 0218 0810  not run
         JMP     J021C            ; 0219 121C  not run
         IXS     1                ; 021A 0401  not run
         JMP     J0217            ; 021B 1217  not run
J021C    CXA                      ; 021C 0140  not run
J021D    STB     D0222+1          ; 021D 3445  not run
         STB     D022A+1          ; 021E 3455  not run
         LDX     D0536            ; 021F 9536  not run
         SLLD    1                ; 0220 0A31  not run
         LDW     D0535            ; 0221 8535  not run
D0222    DATA    X'0930'          ; 0222 0930  written by 021D
         DATA    X'7535'          ; 0223 7535  STW X'535'
         DATA    X'0140'          ; 0224 0140
         DATA    X'0A01'          ; 0225 0A01
         DATA    X'7536'          ; 0226 7536  STW X'536'
         DATA    X'952C'          ; 0227 952C  LDX X'52C'
         DATA    X'0A31'          ; 0228 0A31
         DATA    X'852B'          ; 0229 852B  LDW X'52B'
D022A    DATA    X'0930'          ; 022A 0930  written by 021E
         DATA    X'752B'          ; 022B 752B  STW X'52B'
         DATA    X'0140'          ; 022C 0140
         DATA    X'0A01'          ; 022D 0A01
         DATA    X'752C'          ; 022E 752C  STW X'52C'
J022F    LDW     D052B            ; 022F 852B
         SUB     D0535            ; 0230 B535
         STW     D0538            ; 0231 7538
         SAP                      ; 0232 0810
         JMP     J0242            ; 0233 1242
         CAX                      ; 0234 0130
         LDW     D052C            ; 0235 852C
         SUB     D0536            ; 0236 B536
         SAP                      ; 0237 0810
         DXS     1                ; 0238 0501  not run
         JMP     J023F            ; 0239 123F
         STW     D052B            ; 023A 752B  not run
         CLR                      ; 023B 0100  not run
         STW     D052C            ; 023C 752C  not run
         LDW     D051F            ; 023D 851F  not run
         JMP     J0254            ; 023E 1254  not run
J023F    STX     D052B            ; 023F 652B
         AND     D051F            ; 0240 E51F
         STW     D052C            ; 0241 752C
J0242    LDW     D0535            ; 0242 8535
         JSX     S0584            ; 0243 2584
J0244    LDX     D0538            ; 0244 9538
         DXS     0                ; 0245 0500
         ORI     D051E            ; 0246 C51E
         STW     D0538            ; 0247 7538
         LDW     D052B            ; 0248 852B
         STW     D0533            ; 0249 7533
         LDW     D0536            ; 024A 8536
         CMP                      ; 024B 0110
         JSX     S053F            ; 024C 253F
J024D    LDW     D052B            ; 024D 852B
         ADD     D0533            ; 024E A533
         STW     D052B            ; 024F 752B
J0250    SAM                      ; 0250 0820
         JMP     J0258            ; 0251 1258
         LDW     D0538            ; 0252 8538  not run
         SUB     D0520            ; 0253 B520  not run
J0254    STW     D0538            ; 0254 7538  not run
         JSX     S04EA            ; 0255 24EA  not run
         DATA    X'0535'          ; 0256 0535  argument 1 of 0255
         JMP     J0250            ; 0257 1250  not run
J0258    LDW     D0535            ; 0258 8535
         JSX     S0584            ; 0259 2584
J025A    SNO                      ; 025A 08A0
         JMP     J0265            ; 025B 1265  not run
         CAX                      ; 025C 0130
         AND     D0536            ; 025D E536
         SUB     D052B            ; 025E B52B
         CMP                      ; 025F 0110
         SAP                      ; 0260 0810
         DXS     1                ; 0261 0501  not run
J0262    STX     D052C            ; 0262 652C
         LDW     D0538            ; 0263 8538
         JMP     J01D8            ; 0264 11D8
J0265    LDX     D051F            ; 0265 951F  not run
         JMP     J0262            ; 0266 1262  not run
         DATA    X'7312'          ; 0267 7312  STW X'312'
         DATA    X'6102'          ; 0268 6102  STX X'102'
         DATA    X'515E'          ; 0269 515E  LDB X'15E'
         DATA    X'515E'          ; 026A 515E  LDB X'15E'
         DATA    X'4498'          ; 026B 4498  CMB X'498'
         DATA    X'4498'          ; 026C 4498  CMB X'498'
         DATA    X'236A'          ; 026D 236A  JSX X'36A'
         DATA    X'2A1D'          ; 026E 2A1D  JSX *X'21D'
         DATA    X'3215'          ; 026F 3215  STB X'215'
         DATA    X'3215'          ; 0270 3215  STB X'215'
         DATA    X'3B8F'          ; 0271 3B8F  STB *X'38F'
         DATA    X'3B69'          ; 0272 3B69  STB *X'369'
         STX     D052F            ; 0273 652F
         LDX     *0               ; 0274 9800
         LDW     *0               ; 0275 8800
         ADD     D0523            ; 0276 A523
         SRC     1                ; 0277 0A41
         ADD     D051E            ; 0278 A51E
         AND     D051F            ; 0279 E51F
         STW     D052A            ; 027A 752A
         LDW     *1               ; 027B 8801
         SAP                      ; 027C 0810
         JMP     J028D            ; 027D 128D  not run
         LDX     *2               ; 027E 9802
         SNO                      ; 027F 08A0
         JMP     J0286            ; 0280 1286
         SLLD    1                ; 0281 0A31
         SRL     1                ; 0282 0A01
         SRLD    1                ; 0283 0A21
         SLL     1                ; 0284 0A11
         SRLD    1                ; 0285 0A21
J0286    STW     D0536            ; 0286 7536
         STX     D0537            ; 0287 6537
         STW     D052C            ; 0288 752C
         LDX     D051E            ; 0289 951E
         SLCD    4                ; 028A 0A74
         DXS     2                ; 028B 0502
         JMP     J028F            ; 028C 128F
J028D    JSX     S0495            ; 028D 2495  not run
         DATA    X'12B4'          ; 028E 12B4  JMP X'2B4'
J028F    SLM                      ; 028F 0040
         LDW     *X'26D'          ; 0290 8A6D
         STW     D0535            ; 0291 7535
         LDW     *X'267'          ; 0292 8A67
         JSX     S053F            ; 0293 253F
J0294    LDW     D052B            ; 0294 852B
         ADD     D0535            ; 0295 A535
         LDX     D0536            ; 0296 9536
         STX     D052B            ; 0297 652B
         LDX     D0537            ; 0298 9537
         STX     D052C            ; 0299 652C
         SNO                      ; 029A 08A0
         JMP     J02B6            ; 029B 12B6  not run
         STW     D0535            ; 029C 7535
         JSX     S0584            ; 029D 2584
J029E    ADD     D0535            ; 029E A535
         SRL     1                ; 029F 0A01
         STW     D0535            ; 02A0 7535
         LDX     D0536            ; 02A1 9536
         STX     D052B            ; 02A2 652B
         LDX     D0537            ; 02A3 9537
         STX     D052C            ; 02A4 652C
         JSX     S0584            ; 02A5 2584
J02A6    STW     D0536            ; 02A6 7536
         CLR                      ; 02A7 0100
         STW     D052C            ; 02A8 752C
         LDW     D0535            ; 02A9 8535
         JSX     S0584            ; 02AA 2584
J02AB    LDW     D0535            ; 02AB 8535
J02AC    ADD     D0536            ; 02AC A536
         LDX     D052C            ; 02AD 952C
         SRLD    1                ; 02AE 0A21
         STW     D052B            ; 02AF 752B
         CXA                      ; 02B0 0140
         SAP                      ; 02B1 0810
         SUB     D0524            ; 02B2 B524
         STW     D052C            ; 02B3 752C
J02B4    LDX     D052F            ; 02B4 952F
         JMP     *1               ; 02B5 1801
J02B6    IXS     1                ; 02B6 0401  not run
         JMP     J02B4            ; 02B7 12B4  not run
         JMP     J02AC            ; 02B8 12AC  not run
         DATA    X'0080'          ; 02B9 0080
D02BA    DATA    X'6487'          ; 02BA 6487  STX X'487'; read by 0301
         DATA    X'76A9'          ; 02BB 76A9  STW X'6A9'
         DATA    X'006E'          ; 02BC 006E
         DATA    X'5A5D'          ; 02BD 5A5D  LDB *X'25D'
         DATA    X'4895'          ; 02BE 4895  CMB *X'95'
         DATA    X'0074'          ; 02BF 0074
         DATA    X'9800'          ; 02C0 9800  LDX *0
         DATA    X'6D48'          ; 02C1 6D48  STX *X'548'
         DATA    X'007A'          ; 02C2 007A
         DATA    X'4444'          ; 02C3 4444  CMB X'444'
         DATA    X'1CE1'          ; 02C4 1CE1  JMP *X'4E1'
         DATA    X'FFFF'          ; 02C5 FFFF  CMW *X'7FF'
         DATA    X'007E'          ; 02C6 007E
         DATA    X'AAAA'          ; 02C7 AAAA  '**'
         DATA    X'5560'          ; 02C8 5560  LDB X'560'
         DATA    X'0071'          ; 02C9 0071
         DATA    X'6522'          ; 02CA 6522  STX X'522'
         DATA    X'61FA'          ; 02CB 61FA  STX X'1FA'
         DATA    X'0077'          ; 02CC 0077
         DATA    X'A504'          ; 02CD A504  ADD X'504'
         DATA    X'365C'          ; 02CE 365C  STB X'65C'
         DATA    X'007C'          ; 02CF 007C
         DATA    X'5555'          ; 02D0 5555  LDB X'555'
         DATA    X'1C38'          ; 02D1 1C38  JMP *X'438'
         DATA    X'007F'          ; 02D2 007F
         DATA    X'8000'          ; 02D3 8000  LDW 0
         DATA    X'003A'          ; 02D4 003A
D02D5    DATA    X'8957'          ; 02D5 8957  LDW *X'157'; read by 0307
         DATA    X'0080'          ; 02D6 0080
         DATA    X'8000'          ; 02D7 8000  LDW 0
         DATA    X'0000'          ; 02D8 0000
         DATA    X'007F'          ; 02D9 007F
         DATA    X'0000'          ; 02DA 0000
         DATA    X'0001'          ; 02DB 0001
         LLB     2                ; 02DC 0602
         JMP     J02DF            ; 02DD 12DF
         CLR                      ; 02DE 0100
J02DF    STX     D0532            ; 02DF 6532
         STW     D0538            ; 02E0 7538
         LDX     *0               ; 02E1 9800
         STX     D02E4            ; 02E2 62E4
         JSX     S04C1            ; 02E3 24C1
D02E4    DATA    X'0000'          ; 02E4 0000  argument 1 of 02E3; written by 02E2
J02E5    SAM                      ; 02E5 0820
         JMP     J02ED            ; 02E6 12ED
         LDW     D0538            ; 02E7 8538  not run
         SAZ                      ; 02E8 0800  not run
         JMP     J02EC            ; 02E9 12EC  not run
         LLB     4                ; 02EA 0604  not run
         STW     D0538            ; 02EB 7538  not run
J02EC    JSX     S050B            ; 02EC 250B  not run
J02ED    LDX     D052A            ; 02ED 952A
         DXS     X'8E'            ; 02EE 058E
         JMP     J032E            ; 02EF 132E  not run
         IXS     X'E'             ; 02F0 040E
         JMP     J0314            ; 02F1 1314  not run
         CAX                      ; 02F2 0130
         LLB     X'B1'            ; 02F3 06B1
         ADD     D052A            ; 02F4 A52A
         STB     D02F7+1          ; 02F5 35EF
         CLR                      ; 02F6 0100
D02F7    SLLD    0                ; 02F7 0A30  written by 02F5
         STW     D0537            ; 02F8 7537
         LLB     X'B0'            ; 02F9 06B0
         ADD     D052A            ; 02FA A52A
         JSX     S04D3            ; 02FB 24D3
J02FC    LDX     D052C            ; 02FC 952C
         STX     D0536            ; 02FD 6536
         STW     D052C            ; 02FE 752C
         LDW     D0537            ; 02FF 8537
         STW     D052B            ; 0300 752B
         LDW     D02BA            ; 0301 82BA
         JSX     S0584            ; 0302 2584
J0303    ADD     D0538            ; 0303 A538
         STW     D0538            ; 0304 7538
         LDW     D052B            ; 0305 852B
         STW     D0535            ; 0306 7535
         LDW     D02D5            ; 0307 82D5
         JSX     S053F            ; 0308 253F
J0309    JSX     S04EA            ; 0309 24EA
         DATA    X'0535'          ; 030A 0535  argument 1 of 0309
J030B    LDX     D0526            ; 030B 9526
         STX     D052A            ; 030C 652A
         SAM                      ; 030D 0820
         JMP     J0314            ; 030E 1314
         JSX     S04EA            ; 030F 24EA  not run
         DATA    X'02BA'          ; 0310 02BA  argument 1 of 030F
         LDW     D0538            ; 0311 8538  not run
         SUB     D0520            ; 0312 B520  not run
         STW     D0538            ; 0313 7538  not run
J0314    LDW     D0538            ; 0314 8538
         SAO                      ; 0315 0830
         JMP     J031C            ; 0316 131C
         JSX     S010E            ; 0317 210E  not run
         DATA    X'02B9'          ; 0318 02B9  argument 1 of 0317
         JSX     S050B            ; 0319 250B  not run
         LDW     D0538            ; 031A 8538  not run
         ADD     D0520            ; 031B A520  not run
J031C    SRLD    2                ; 031C 0A22
         IXS     0                ; 031D 0400
         JMP     J0322            ; 031E 1322
         JSX     S0499            ; 031F 2499
         DATA    X'82BC'          ; 0320 82BC  argument 1 of 031F; LDW X'2BC'
J0321    JMP     J0328            ; 0321 1328
J0322    JSX     S0499            ; 0322 2499
         DATA    X'82C9'          ; 0323 82C9  argument 1 of 0322; LDW X'2C9'
J0324    JSX     S010E            ; 0324 210E
         DATA    X'02D6'          ; 0325 02D6  argument 1 of 0324
J0326    JSX     S010E            ; 0326 210E
         DATA    X'02D9'          ; 0327 02D9  argument 1 of 0326
J0328    LDW     D0538            ; 0328 8538
         SRC     3                ; 0329 0A43
         SAP                      ; 032A 0810
         JSX     S050B            ; 032B 250B  not run
J032C    LDX     D0532            ; 032C 9532
         JMP     *1               ; 032D 1801
J032E    JSX     S0495            ; 032E 2495  not run
         DATA    X'132C'          ; 032F 132C  JMP X'32C'
         DATA    X'0081'          ; 0330 0081
         DATA    X'6487'          ; 0331 6487  STX X'487'
         DATA    X'76A9'          ; 0332 76A9  STW X'6A9'
         DATA    X'0080'          ; 0333 0080
         DATA    X'6487'          ; 0334 6487  STX X'487'
         DATA    X'76A9'          ; 0335 76A9  STW X'6A9'
         DATA    X'007D'          ; 0336 007D
         DATA    X'7F56'          ; 0337 7F56  STW *X'756'
         DATA    X'7535'          ; 0338 7535  STW X'535'
         DATA    X'007E'          ; 0339 007E
         DATA    X'7D6D'          ; 033A 7D6D  STW *X'56D'
         DATA    X'6BF2'          ; 033B 6BF2  STX *X'3F2'
         DATA    X'007F'          ; 033C 007F
         DATA    X'5BD8'          ; 033D 5BD8  LDB *X'3D8'
         DATA    X'3284'          ; 033E 3284  STB X'284'
         DATA    X'007F'          ; 033F 007F
         DATA    X'76B1'          ; 0340 76B1  STW X'6B1'
         DATA    X'4E0B'          ; 0341 4E0B  CMB *X'60B'
         DATA    X'0080'          ; 0342 0080
         DATA    X'4780'          ; 0343 4780  CMB X'780'
         DATA    X'1757'          ; 0344 1757  JMP X'757'
         DATA    X'0080'          ; 0345 0080
         DATA    X'525E'          ; 0346 525E  LDB X'25E'
         DATA    X'1F46'          ; 0347 1F46  JMP *X'746'
         DATA    X'0080'          ; 0348 0080
         DATA    X'5C02'          ; 0349 5C02  LDB *X'402'
         DATA    X'4F8B'          ; 034A 4F8B  CMB *X'78B'
         DATA    X'8333'          ; 034B 8333  LDW X'333'
         DATA    X'007D'          ; 034C 007D
         DATA    X'71C7'          ; 034D 71C7  STW X'1C7'
         DATA    X'0E39'          ; 034E 0E39
         DATA    X'007E'          ; 034F 007E
         DATA    X'B6DB'          ; 0350 B6DB  '6['
         DATA    X'36DB'          ; 0351 36DB  STB X'6DB'
         DATA    X'007E'          ; 0352 007E
         DATA    X'6666'          ; 0353 6666  STX X'666'
         DATA    X'3333'          ; 0354 3333  STB X'333'
         DATA    X'FFFF'          ; 0355 FFFF  CMW *X'7FF'
         DATA    X'007F'          ; 0356 007F
         DATA    X'AAAA'          ; 0357 AAAA  '**'
         DATA    X'5555'          ; 0358 5555  LDB X'555'
D0359    DATA    X'0000'          ; 0359 0000  written by 036C; read by 03AE
D035A    DATA    X'0000'          ; 035A 0000  written by 035F; read by 03A8
         STX     D0532            ; 035B 6532
         LDX     *0               ; 035C 9800
         LDW     *0               ; 035D 8800
         SUB     D0523            ; 035E B523
         STW     D035A            ; 035F 735A
         SAP                      ; 0360 0810
         JMP     J0369            ; 0361 1369  not run
         STX     D0366            ; 0362 6366
         JSX     S04C1            ; 0363 24C1
         DATA    X'0523'          ; 0364 0523  argument 1 of 0363
J0365    JSX     S019D            ; 0365 219D
D0366    DATA    X'0000'          ; 0366 0000  argument 1 of 0365; written by 0362
J0367    LDW     D052B            ; 0367 852B
         JMP     J036C            ; 0368 136C
J0369    STX     D036B            ; 0369 636B  not run
         JSX     S04C1            ; 036A 24C1  not run
D036B    DATA    X'0000'          ; 036B 0000  argument 1 of 036A; written by 0369
J036C    STW     D0359            ; 036C 7359
         SAP                      ; 036D 0810
         JSX     S050B            ; 036E 250B  not run
         LDX     D052A            ; 036F 952A
         SLM                      ; 0370 0040
         DXS     X'7E'            ; 0371 057E
         JMP     *X'375'          ; 0372 1B75
         STW     D03A7            ; 0373 73A7  not run
         JMP     J03A1            ; 0374 13A1  not run
         DATA    X'0A01'          ; 0375 0A01
         DATA    X'0A01'          ; 0376 0A01
         DATA    X'137B'          ; 0377 137B  JMP X'37B'
J0378    JSX     S04C1            ; 0378 24C1
         DATA    X'0333'          ; 0379 0333  argument 1 of 0378
J037A    JMP     J03AE            ; 037A 13AE
         DATA    X'E44A'          ; 037B E44A  AND X'44A'
         DATA    X'7533'          ; 037C 7533  STW X'533'
         DATA    X'0A01'          ; 037D 0A01
         DATA    X'A533'          ; 037E A533  ADD X'533'
         DATA    X'0A55'          ; 037F 0A55
         DATA    X'A34B'          ; 0380 A34B  ADD X'34B'
         DATA    X'73A7'          ; 0381 73A7  STW X'3A7'
         DATA    X'852C'          ; 0382 852C  LDW X'52C'
         DATA    X'7535'          ; 0383 7535  STW X'535'
         DATA    X'852B'          ; 0384 852B  LDW X'52B'
         DATA    X'7534'          ; 0385 7534  STW X'534'
         DATA    X'8533'          ; 0386 8533  LDW X'533'
         DATA    X'253F'          ; 0387 253F  JSX X'53F'
         DATA    X'8534'          ; 0388 8534  LDW X'534'
         DATA    X'752C'          ; 0389 752C  STW X'52C'
         DATA    X'8533'          ; 038A 8533  LDW X'533'
         DATA    X'2518'          ; 038B 2518  JSX X'518'
         DATA    X'06A1'          ; 038C 06A1
         DATA    X'B52A'          ; 038D B52A  SUB X'52A'
         DATA    X'24D3'          ; 038E 24D3  JSX X'4D3'
         DATA    X'A524'          ; 038F A524  ADD X'524'
         DATA    X'952C'          ; 0390 952C  LDX X'52C'
         DATA    X'753A'          ; 0391 753A  STW X'53A'
         DATA    X'653B'          ; 0392 653B  STX X'53B'
         DATA    X'8535'          ; 0393 8535  LDW X'535'
         DATA    X'752C'          ; 0394 752C  STW X'52C'
         DATA    X'8534'          ; 0395 8534  LDW X'534'
         DATA    X'752B'          ; 0396 752B  STW X'52B'
         DATA    X'06B3'          ; 0397 06B3
         DATA    X'A52A'          ; 0398 A52A  ADD X'52A'
         DATA    X'24D3'          ; 0399 24D3  JSX X'4D3'
         DATA    X'9526'          ; 039A 9526  LDX X'526'
         DATA    X'6539'          ; 039B 6539  STX X'539'
         DATA    X'0504'          ; 039C 0504
         DATA    X'652A'          ; 039D 652A  STX X'52A'
         DATA    X'2109'          ; 039E 2109  JSX X'109'
         DATA    X'219D'          ; 039F 219D  JSX X'19D'
         DATA    X'0539'          ; 03A0 0539
J03A1    JSX     S0499            ; 03A1 2499  not run
         DATA    X'834C'          ; 03A2 834C  argument 1 of 03A1; LDW X'34C'
         LDW     D03A7            ; 03A3 83A7  not run
         SAM                      ; 03A4 0820  not run
         JMP     J03A8            ; 03A5 13A8  not run
         JSX     S0110            ; 03A6 2110  not run
D03A7    DATA    X'0000'          ; 03A7 0000  argument 1 of 03A6; written by 0373; read by 03A3
J03A8    LDW     D035A            ; 03A8 835A  not run
         SAP                      ; 03A9 0810  not run
         JMP     J03AE            ; 03AA 13AE  not run
         JSX     S050B            ; 03AB 250B  not run
         JSX     S0110            ; 03AC 2110  not run
         DATA    X'0330'          ; 03AD 0330  argument 1 of 03AC
J03AE    LDW     D0359            ; 03AE 8359
         SAP                      ; 03AF 0810
         JSX     S050B            ; 03B0 250B  not run
         JMP     J032C            ; 03B1 132C
         DATA    X'0077'          ; 03B2 0077
         DATA    X'5B05'          ; 03B3 5B05  LDB *X'305'
         DATA    X'582E'          ; 03B4 582E  LDB *X'2E'
         DATA    X'007A'          ; 03B5 007A
         DATA    X'4444'          ; 03B6 4444  CMB X'444'
         DATA    X'2222'          ; 03B7 2222  JSX X'222'
         DATA    X'007C'          ; 03B8 007C
         DATA    X'5555'          ; 03B9 5555  LDB X'555'
         DATA    X'2AAB'          ; 03BA 2AAB  JSX *X'2AB'
         DATA    X'007E'          ; 03BB 007E
         DATA    X'5555'          ; 03BC 5555  LDB X'555'
         DATA    X'2AAB'          ; 03BD 2AAB  JSX *X'2AB'
         DATA    X'0080'          ; 03BE 0080
         DATA    X'4000'          ; 03BF 4000  CMB 0
         DATA    X'0000'          ; 03C0 0000
         DATA    X'0081'          ; 03C1 0081
         DATA    X'4000'          ; 03C2 4000  CMB 0
         DATA    X'0000'          ; 03C3 0000
D03C4    DATA    X'83C6'          ; 03C4 83C6  LDW X'3C6'; read by 041E
D03C5    DATA    X'007D'          ; 03C5 007D  read by 0416
         DATA    X'0081'          ; 03C6 0081
         DATA    X'4000'          ; 03C7 4000  CMB 0
         DATA    X'0000'          ; 03C8 0000
         DATA    X'0081'          ; 03C9 0081
         DATA    X'4885'          ; 03CA 4885  CMB *X'85'
         DATA    X'408B'          ; 03CB 408B  CMB X'8B'
         DATA    X'0081'          ; 03CC 0081
         DATA    X'522D'          ; 03CD 522D  LDB X'22D'
         DATA    X'3C79'          ; 03CE 3C79  STB *X'479'
         DATA    X'0081'          ; 03CF 0081
         DATA    X'5D1E'          ; 03D0 5D1E  LDB *X'51E'
         DATA    X'4A28'          ; 03D1 4A28  CMB *X'228'
         DATA    X'0081'          ; 03D2 0081
         DATA    X'6984'          ; 03D3 6984  STX *X'184'
         DATA    X'531C'          ; 03D4 531C  LDB X'31C'
         DATA    X'0081'          ; 03D5 0081
         DATA    X'7791'          ; 03D6 7791  STW X'791'
         DATA    X'2BBF'          ; 03D7 2BBF  JSX *X'3BF'
D03D8    DATA    X'58B9'          ; 03D8 58B9  LDB *X'B9'; read by 0405
         DATA    X'05FE'          ; 03D9 05FE
D03DA    DATA    X'FA02'          ; 03DA FA02  CMW *X'202'; read by 040A 0425
         DATA    X'6442'          ; 03DB 6442  STX X'442'
         DATA    X'13EB'          ; 03DC 13EB  JMP X'3EB'
D03DD    DATA    X'0110'          ; 03DD 0110  read by 03DE
         LDW     D03DD            ; 03DE 83DD
         STW     D0442            ; 03DF 7442
         STX     D0532            ; 03E0 6532
         LDX     *0               ; 03E1 9800
         STX     D03E4            ; 03E2 63E4
         JSX     S04C1            ; 03E3 24C1
D03E4    DATA    X'0000'          ; 03E4 0000  argument 1 of 03E3; written by 03E2 03E5; read by 0430
J03E5    STW     D03E4            ; 03E5 73E4
         LDX     D052A            ; 03E6 952A
         DXS     X'88'            ; 03E7 0588
         JMP     J043E            ; 03E8 143E  not run
         SAP                      ; 03E9 0810
         JSX     S050B            ; 03EA 250B  not run
         LDX     D052A            ; 03EB 952A
         DXS     X'80'            ; 03EC 0580
         JMP     J03F6            ; 03ED 13F6
         CLR                      ; 03EE 0100  not run
         STW     D0538            ; 03EF 7538  not run
         LLB     X'F'             ; 03F0 060F  not run
         IXS     2                ; 03F1 0402  not run
         JMP     J0418            ; 03F2 1418  not run
         LLB     X'8C'            ; 03F3 068C  not run
         SUB     D052A            ; 03F4 B52A  not run
         JMP     J0416            ; 03F5 1416  not run
J03F6    CAX                      ; 03F6 0130
         LLB     X'B1'            ; 03F7 06B1
         ADD     D052A            ; 03F8 A52A
         STB     D03FB+1          ; 03F9 37F7
         CLR                      ; 03FA 0100
D03FB    SLLD    0                ; 03FB 0A30  written by 03F9
         STW     D0538            ; 03FC 7538
         LLB     X'B0'            ; 03FD 06B0
         ADD     D052A            ; 03FE A52A
         JSX     S04D3            ; 03FF 24D3
J0400    LDX     D052C            ; 0400 952C
         STX     D0536            ; 0401 6536
         STW     D052C            ; 0402 752C
         LDW     D0538            ; 0403 8538
         STW     D052B            ; 0404 752B
         LDW     D03D8            ; 0405 83D8
         JSX     S0584            ; 0406 2584
J0407    STW     D0538            ; 0407 7538
         LDW     D052B            ; 0408 852B
         STW     D0535            ; 0409 7535
         LDW     D03DA            ; 040A 83DA
         JSX     S053F            ; 040B 253F
J040C    JSX     S04EA            ; 040C 24EA
         DATA    X'0535'          ; 040D 0535  argument 1 of 040C
J040E    SAM                      ; 040E 0820
         JMP     J0415            ; 040F 1415
         JSX     S04EA            ; 0410 24EA  not run
         DATA    X'03D8'          ; 0411 03D8  argument 1 of 0410
         LDW     D0538            ; 0412 8538  not run
         SUB     D0520            ; 0413 B520  not run
         STW     D0538            ; 0414 7538  not run
J0415    LLB     X'C'             ; 0415 060C
J0416    LDX     D03C5            ; 0416 93C5
         STX     D052A            ; 0417 652A
J0418    STB     D041A+1          ; 0418 3035
         LDW     D052B            ; 0419 852B
D041A    SRL     0                ; 041A 0A00  written by 0418; read by 0422
         STW     D0533            ; 041B 7533
         SLL     1                ; 041C 0A11
         ADD     D0533            ; 041D A533
         ADD     D03C4            ; 041E A3C4
         STW     D0429            ; 041F 7429
         STW     D042C            ; 0420 742C
         LLB     X'3F'            ; 0421 063F
         ORE     D041A            ; 0422 D41A
         JSX     S04D3            ; 0423 24D3
J0424    JSX     S0109            ; 0424 2109
J0425    LDW     D03DA            ; 0425 83DA
         JSX     S0499            ; 0426 2499
         DATA    X'83B2'          ; 0427 83B2  argument 1 of 0426; LDW X'3B2'
J0428    JSX     S01A3            ; 0428 21A3
D0429    DATA    X'0000'          ; 0429 0000  argument 1 of 0428; written by 041F
J042A    LDX     D0442            ; 042A 9442
         JSX     *0               ; 042B 2800
D042C    DATA    X'0000'          ; 042C 0000  written by 0420
J042D    LDW     D0538            ; 042D 8538
         ADD     D0523            ; 042E A523
         STW     D052A            ; 042F 752A
         LDX     D03E4            ; 0430 93E4
         DXS     0                ; 0431 0500
         JMP     J043A            ; 0432 143A
         JSX     S04CA            ; 0433 24CA  not run
         DATA    X'0539'          ; 0434 0539  argument 1 of 0433
         JSX     S04C1            ; 0435 24C1  not run
         DATA    X'0523'          ; 0436 0523  argument 1 of 0435
         JSX     S019D            ; 0437 219D  not run
         DATA    X'0539'          ; 0438 0539  argument 1 of 0437
         JMP     J043D            ; 0439 143D  not run
J043A    LLB     0                ; 043A 0600
         SAZ                      ; 043B 0800
J043C    JSX     S04DF            ; 043C 24DF  not run
J043D    JMP     J032C            ; 043D 132C
J043E    SAM                      ; 043E 0820  not run
         JMP     J043C            ; 043F 143C  not run
         JSX     S0495            ; 0440 2495  not run
         DATA    X'143D'          ; 0441 143D  JMP X'43D'
D0442    DATA    X'0000'          ; 0442 0000  written by 03DF; read by 042A
         DATA    X'0080'          ; 0443 0080
         DATA    X'58B9'          ; 0444 58B9  LDB *X'B9'
         DATA    X'05FE'          ; 0445 05FE
D0446    DATA    X'8433'          ; 0446 8433  LDW X'433'; read by 046A
         DATA    X'007F'          ; 0447 007F
         DATA    X'7852'          ; 0448 7852  STW *X'52'
         DATA    X'1434'          ; 0449 1434  JMP X'434'
D044A    DATA    X'F000'          ; 044A F000  CMW 0; read by 0467
         DATA    X'007F'          ; 044B 007F
         DATA    X'49A5'          ; 044C 49A5  CMB *X'1A5'
         DATA    X'4422'          ; 044D 4422  CMB X'422'
         DATA    X'0000'          ; 044E 0000
         DATA    X'007E'          ; 044F 007E
         DATA    X'445E'          ; 0450 445E  CMB X'45E'
         DATA    X'1D04'          ; 0451 1D04  JMP *X'504'
D0452    DATA    X'007D'          ; 0452 007D  read by 047F
         DATA    X'71C7'          ; 0453 71C7  STW X'1C7'
         DATA    X'0E39'          ; 0454 0E39
         DATA    X'007E'          ; 0455 007E
         DATA    X'4924'          ; 0456 4924  CMB *X'124'
         DATA    X'4925'          ; 0457 4925  CMB *X'125'
         DATA    X'007E'          ; 0458 007E
         DATA    X'6666'          ; 0459 6666  STX X'666'
         DATA    X'3333'          ; 045A 3333  STB X'333'
         DATA    X'FFFF'          ; 045B FFFF  CMW *X'7FF'
         DATA    X'007F'          ; 045C 007F
         DATA    X'5555'          ; 045D 5555  LDB X'555'
         DATA    X'2AAB'          ; 045E 2AAB  JSX *X'2AB'
         STX     D0532            ; 045F 6532
         LDX     *0               ; 0460 9800
         STX     D0463            ; 0461 6463
         JSX     S04C1            ; 0462 24C1
D0463    DATA    X'0000'          ; 0463 0000  argument 1 of 0462; written by 0461
J0464    SAZ                      ; 0464 0800
         SAP                      ; 0465 0810
         JMP     J0491            ; 0466 1491  not run
         AND     D044A            ; 0467 E44A
         STW     D0538            ; 0468 7538
         SLC     6                ; 0469 0A56
         ADD     D0446            ; 046A A446
         STW     D0487            ; 046B 7487
         LDW     D0538            ; 046C 8538
         ORE     D052B            ; 046D D52B
         STW     D052B            ; 046E 752B
         LDW     D0538            ; 046F 8538
         LDX     D052C            ; 0470 952C
         SLLD    1                ; 0471 0A31
         ORI     D052B            ; 0472 C52B
         SRLD    1                ; 0473 0A21
         STW     D0535            ; 0474 7535
         CXA                      ; 0475 0140
         SRL     1                ; 0476 0A01
         STW     D0536            ; 0477 7536
         LLB     X'32'            ; 0478 0632
         JSX     S04D3            ; 0479 24D3
J047A    JSX     S01AD            ; 047A 21AD
         DATA    X'0535'          ; 047B 0535  argument 1 of 047A
J047C    LDW     D052A            ; 047C 852A
         SUB     D0526            ; 047D B526
         STW     D0538            ; 047E 7538
         LDW     D0452            ; 047F 8452
         STW     D052A            ; 0480 752A
         JSX     S0499            ; 0481 2499
         DATA    X'8452'          ; 0482 8452  argument 1 of 0481; LDW X'452'
J0483    LDW     D052A            ; 0483 852A
         ADD     D0520            ; 0484 A520
         STW     D052A            ; 0485 752A
         JSX     S010E            ; 0486 210E
D0487    DATA    X'0000'          ; 0487 0000  argument 1 of 0486; written by 046B
J0488    JSX     S04CA            ; 0488 24CA
         DATA    X'0539'          ; 0489 0539  argument 1 of 0488
J048A    JSX     S05CC            ; 048A 25CC
         DATA    X'0538'          ; 048B 0538  argument 1 of 048A
J048C    JSX     S01A3            ; 048C 21A3
         DATA    X'0443'          ; 048D 0443  argument 1 of 048C
J048E    JSX     S0110            ; 048E 2110
         DATA    X'0539'          ; 048F 0539  argument 1 of 048E
J0490    JMP     J032C            ; 0490 132C
J0491    INV                      ; 0491 0120  not run
         STW     D052B            ; 0492 752B  not run
         JSX     S04DF            ; 0493 24DF  not run
         DATA    X'1490'          ; 0494 1490  JMP X'490'
S0495    CLR                      ; 0495 0100  not run
         STW     D052A            ; 0496 752A  not run
         STW     D052C            ; 0497 752C  not run
         JMP     J0513            ; 0498 1513  not run
S0499    STX     D0531            ; 0499 6531
         LDX     *0               ; 049A 9800
         STX     D04A6            ; 049B 64A6
         SAP                      ; 049C 0810
         JMP     J04A3            ; 049D 14A3
         JSX     S0109            ; 049E 2109
J049F    JSX     S04CA            ; 049F 24CA
         DATA    X'053C'          ; 04A0 053C  argument 1 of 049F
J04A1    JSX     S01A3            ; 04A1 21A3
         DATA    X'053C'          ; 04A2 053C  argument 1 of 04A1
J04A3    JSX     S04CA            ; 04A3 24CA
         DATA    X'0539'          ; 04A4 0539  argument 1 of 04A3
J04A5    JSX     S01A3            ; 04A5 21A3
D04A6    DATA    X'0000'          ; 04A6 0000  argument 1 of 04A5; written by 049B
J04A7    LDX     *0               ; 04A7 9800
J04A8    IXS     3                ; 04A8 0403
         STX     D04AB            ; 04A9 64AB
         JSX     S0110            ; 04AA 2110
D04AB    DATA    X'0000'          ; 04AB 0000  argument 1 of 04AA; written by 04A9; read by 04AE
J04AC    JSX     S01A3            ; 04AC 21A3
         DATA    X'0539'          ; 04AD 0539  argument 1 of 04AC
J04AE    LDX     D04AB            ; 04AE 94AB
         LDW     *3               ; 04AF 8803
         SAM                      ; 04B0 0820
         JMP     J04A8            ; 04B1 14A8
         ADD     D0520            ; 04B2 A520
         SAP                      ; 04B3 0810
         JMP     J04BF            ; 04B4 14BF
         IXS     4                ; 04B5 0404
         STX     D04B8            ; 04B6 64B8
         JSX     S0110            ; 04B7 2110
D04B8    DATA    X'0000'          ; 04B8 0000  argument 1 of 04B7; written by 04B6
J04B9    JSX     S01A3            ; 04B9 21A3
         DATA    X'0539'          ; 04BA 0539  argument 1 of 04B9
J04BB    JSX     S01A3            ; 04BB 21A3
         DATA    X'053C'          ; 04BC 053C  argument 1 of 04BB
J04BD    JSX     S0110            ; 04BD 2110
         DATA    X'053C'          ; 04BE 053C  argument 1 of 04BD
J04BF    LDX     D0531            ; 04BF 9531
         JMP     *1               ; 04C0 1801
S04C1    STX     D052E            ; 04C1 652E
         LDX     *0               ; 04C2 9800
         LDW     *0               ; 04C3 8800
         AND     D0522            ; 04C4 E522
         STW     D052A            ; 04C5 752A
         LDW     *2               ; 04C6 8802
         STW     D052C            ; 04C7 752C
         LDW     *1               ; 04C8 8801
         JMP     J04F6            ; 04C9 14F6
S04CA    STX     D052E            ; 04CA 652E
         LDX     *0               ; 04CB 9800
         LDW     D052A            ; 04CC 852A
         STW     *0               ; 04CD 7800
         LDW     D052C            ; 04CE 852C
         STW     *2               ; 04CF 7802
         LDW     D052B            ; 04D0 852B
         STW     *1               ; 04D1 7801
         JMP     J05D6            ; 04D2 15D6
S04D3    STX     D052E            ; 04D3 652E
         STB     D04D8+1          ; 04D4 31B1
         LDX     D052C            ; 04D5 952C
         SLLD    1                ; 04D6 0A31
         LDW     D052B            ; 04D7 852B
D04D8    SRL     0                ; 04D8 0A00  written by 04D4
         SLL     1                ; 04D9 0A11
         SRLD    1                ; 04DA 0A21
         STW     D052B            ; 04DB 752B
         STX     D052C            ; 04DC 652C
J04DD    LDX     D052E            ; 04DD 952E
         JMP     *0               ; 04DE 1800
S04DF    STX     D0529            ; 04DF 6529  not run
         LDX     D052B            ; 04E0 952B  not run
         LDW     D0522            ; 04E1 8522  not run
         STW     D052A            ; 04E2 752A  not run
         LDW     D051F            ; 04E3 851F  not run
         STW     D052B            ; 04E4 752B  not run
         STW     D052C            ; 04E5 752C  not run
         IXS     0                ; 04E6 0400  not run
         JSX     S050B            ; 04E7 250B  not run
         LDX     D0529            ; 04E8 9529  not run
         JMP     *0               ; 04E9 1800  not run
S04EA    STX     D052E            ; 04EA 652E
         LDX     *0               ; 04EB 9800
         LDW     *1               ; 04EC 8801
         ADD     D052C            ; 04ED A52C
         AND     D051F            ; 04EE E51F
         STW     D052C            ; 04EF 752C
         LDW     *0               ; 04F0 8800
         SNO                      ; 04F1 08A0
         ADD     D0520            ; 04F2 A520
         SNO                      ; 04F3 08A0
         JMP     J04F8            ; 04F4 14F8  not run
J04F5    ADD     D052B            ; 04F5 A52B
J04F6    STW     D052B            ; 04F6 752B
         JMP     J05D6            ; 04F7 15D6
J04F8    ORE     D052B            ; 04F8 D52B  not run
         ADD     D051E            ; 04F9 A51E  not run
         ORE     D051E            ; 04FA D51E  not run
         JMP     J04F6            ; 04FB 14F6  not run
         STX     D052E            ; 04FC 652E
         LDX     *0               ; 04FD 9800
         LDW     D052C            ; 04FE 852C
         SUB     *1               ; 04FF B801
         SAM                      ; 0500 0820
         JMP     J0507            ; 0501 1507
         AND     D051F            ; 0502 E51F
         STW     D052C            ; 0503 752C
         LDW     *0               ; 0504 8800
         INV                      ; 0505 0120
         JMP     J04F5            ; 0506 14F5
J0507    STW     D052C            ; 0507 752C
         LDW     D052B            ; 0508 852B
         SUB     *0               ; 0509 B800
         JMP     J04F6            ; 050A 14F6
S050B    LDW     D052C            ; 050B 852C
         CMP                      ; 050C 0110
         SAM                      ; 050D 0820
         JMP     J0515            ; 050E 1515
         AND     D051F            ; 050F E51F
         STW     D052C            ; 0510 752C
         LDW     D052B            ; 0511 852B
         INV                      ; 0512 0120
J0513    STW     D052B            ; 0513 752B
         JMP     *0               ; 0514 1800
J0515    LDW     D052B            ; 0515 852B
         CMP                      ; 0516 0110
         JMP     J0513            ; 0517 1513
S0518    STX     D052E            ; 0518 652E
         CAX                      ; 0519 0130
         LDW     D052B            ; 051A 852B
         SXE                      ; 051B 0850
         ADD     D052C            ; 051C A52C
         JMP     J0547            ; 051D 1547
D051E    DATA    X'8000'          ; 051E 8000  LDW 0; read by 0159 018C 0246 0278 ...
D051F    DATA    X'7FFF'          ; 051F 7FFF  STW *X'7FF'; read by 01D3 01E7 023D 0240 ...
D0520    DATA    X'0001'          ; 0520 0001  read by 0136 0142 0152 0188 ...
D0521    DATA    X'0002'          ; 0521 0002  read by 058A 0798
D0522    DATA    X'00FF'          ; 0522 00FF  read by 00B5 00C2 00F0 04C4 ...
D0523    DATA    X'0081'          ; 0523 0081  read by 0276 035E 042E
D0524    DATA    X'4000'          ; 0524 4000  CMB 0; read by 0190 02B2
D0525    DATA    X'0000'          ; 0525 0000  read by 015B 018A 05E7
D0526    DATA    X'0080'          ; 0526 0080  read by 019F 030B 047D 05F6
D0527    DATA    X'FFFF'          ; 0527 FFFF  CMW *X'7FF'; read by 0200 0698 06D0 0782
D0528    DATA    X'FF80'          ; 0528 FF80  CMW *X'780'; read by 01A5
D0529    DATA    X'0000'          ; 0529 0000  written by 04DF; read by 04E8
D052A    DATA    X'0000'          ; 052A 0000  written by 00B6 011F 016E 0177 ...; read by 00CD 0118 0164 0168 ...
D052B    DATA    X'0000'          ; 052B 0000  written by 00B8 015D 017E 018D ...; read by 00CF 010C 0158 017C ...
D052C    DATA    X'0000'          ; 052C 0000  written by 00BA 015C 0181 0191 ...; read by 00D1 0150 015A 017A ...
         DATA    X'0000'          ; 052D 0000
D052E    DATA    X'0000'          ; 052E 0000  written by 04C1 04CA 04D3 04EA ...; read by 04DD 05D6
D052F    DATA    X'0000'          ; 052F 0000  written by 01A0 01A6 01AE 0273 ...; read by 01BD 01D9 02B4 0744
D0530    DATA    X'0000'          ; 0530 0000  written by 010B 0112 019D 01A3 ...; read by 0182 01DF
D0531    DATA    X'0000'          ; 0531 0000  written by 0499; read by 04BF
D0532    DATA    X'0000'          ; 0532 0000  written by 02DF 035B 03E0 045F ...; read by 032C
D0533    DATA    X'0000'          ; 0533 0000  written by 0115 01C0 01CE 0249 ...; read by 01C9 01CD 01FE 024E ...
D0534    DATA    X'0000'          ; 0534 0000  written by 0117 01C8 0589; read by 01D2 0591 0594 0599 ...
D0535    DATA    X'0000'          ; 0535 0000  written by 0137 013F 0144 01B8 ...; read by 0139 0147 01C4 01D0 ...
D0536    DATA    X'0000'          ; 0536 0000  written by 0122 01B9 01E8 0286 ...; read by 0130 0145 01C2 01CB ...
D0537    DATA    X'0000'          ; 0537 0000  written by 01B4 0287 02F8 06EA ...; read by 01DC 0298 02A3 02FF ...
D0538    DATA    X'0000'          ; 0538 0000  written by 0231 0247 0254 02E0 ...; read by 0244 0252 0263 02E7 ...
D0539    DATA    X'0000'          ; 0539 0000  written by 0766; read by 076B
D053A    DATA    X'0000'          ; 053A 0000  written by 078D 07E9; read by 07DF
         DATA    X'0000'          ; 053B 0000
D053C    DATA    X'0000'          ; 053C 0000  written by 07C3; read by 07D1
         DATA    X'0000'          ; 053D 0000
         DATA    X'0000'          ; 053E 0000
S053F    STX     D052E            ; 053F 652E
         SAP                      ; 0540 0810
         JMP     J0579            ; 0541 1579
J0542    LDX     D052C            ; 0542 952C
J0543    STW     D052C            ; 0543 752C
         SXE                      ; 0544 0850
         JMP     J0547            ; 0545 1547
         CLR                      ; 0546 0100
J0547    SRLD    1                ; 0547 0A21
         SXE                      ; 0548 0850
         ADD     D052C            ; 0549 A52C
         SRLD    1                ; 054A 0A21
         SXE                      ; 054B 0850
         ADD     D052C            ; 054C A52C
         SRLD    1                ; 054D 0A21
         SXE                      ; 054E 0850
         ADD     D052C            ; 054F A52C
         SRLD    1                ; 0550 0A21
         SXE                      ; 0551 0850
         ADD     D052C            ; 0552 A52C
         SRLD    1                ; 0553 0A21
         SXE                      ; 0554 0850
         ADD     D052C            ; 0555 A52C
         SRLD    1                ; 0556 0A21
         SXE                      ; 0557 0850
         ADD     D052C            ; 0558 A52C
         SRLD    1                ; 0559 0A21
         SXE                      ; 055A 0850
         ADD     D052C            ; 055B A52C
         SRLD    1                ; 055C 0A21
         SXE                      ; 055D 0850
         ADD     D052C            ; 055E A52C
         SRLD    1                ; 055F 0A21
         SXE                      ; 0560 0850
         ADD     D052C            ; 0561 A52C
         SRLD    1                ; 0562 0A21
         SXE                      ; 0563 0850
         ADD     D052C            ; 0564 A52C
         SRLD    1                ; 0565 0A21
         SXE                      ; 0566 0850
         ADD     D052C            ; 0567 A52C
         SRLD    1                ; 0568 0A21
         SXE                      ; 0569 0850
         ADD     D052C            ; 056A A52C
         SRLD    1                ; 056B 0A21
         SXE                      ; 056C 0850
         ADD     D052C            ; 056D A52C
         SRLD    1                ; 056E 0A21
         SXE                      ; 056F 0850
         ADD     D052C            ; 0570 A52C
         SRLD    1                ; 0571 0A21
         SLA     0                ; 0572 0910
         SXE                      ; 0573 0850
         SUB     D052C            ; 0574 B52C
         STW     D052B            ; 0575 752B
         CXA                      ; 0576 0140
         SRL     1                ; 0577 0A01
         JMP     J05BD            ; 0578 15BD
J0579    CAX                      ; 0579 0130
         LDW     D052C            ; 057A 852C
         SAM                      ; 057B 0820
         JMP     J0543            ; 057C 1543
         CMP                      ; 057D 0110  not run
         STW     D052C            ; 057E 752C  not run
         CXA                      ; 057F 0140  not run
         SNO                      ; 0580 08A0  not run
         JMP     J0542            ; 0581 1542  not run
         CMP                      ; 0582 0110  not run
         JMP     J0542            ; 0583 1542  not run
S0584    STX     D052E            ; 0584 652E
         STW     D0535            ; 0585 7535
         SAP                      ; 0586 0810
         CMP                      ; 0587 0110  not run
         ADD     D051E            ; 0588 A51E
         STW     D0534            ; 0589 7534
         LDW     D0521            ; 058A 8521
         STW     D0533            ; 058B 7533
         LDX     D052C            ; 058C 952C
         SLLD    1                ; 058D 0A31
         LDW     D052B            ; 058E 852B
         SAP                      ; 058F 0810
         JMP     J05BF            ; 0590 15BF  not run
         SUB     D0534            ; 0591 B534
         SAP                      ; 0592 0810
         JMP     J05CA            ; 0593 15CA  not run
J0594    ADD     D0534            ; 0594 A534
         JMP     J0598            ; 0595 1598
J0596    STX     D0533            ; 0596 6533
         LDX     D052C            ; 0597 952C
J0598    SLCD    1                ; 0598 0A71
         SUB     D0534            ; 0599 B534
         SAM                      ; 059A 0820
         ADD     D0534            ; 059B A534
         SLCD    1                ; 059C 0A71
         SUB     D0534            ; 059D B534
         SAM                      ; 059E 0820
         ADD     D0534            ; 059F A534
         SLCD    1                ; 05A0 0A71
         SUB     D0534            ; 05A1 B534
         SAM                      ; 05A2 0820
         ADD     D0534            ; 05A3 A534
         SLCD    1                ; 05A4 0A71
         SUB     D0534            ; 05A5 B534
         SAM                      ; 05A6 0820
         ADD     D0534            ; 05A7 A534
         SLCD    1                ; 05A8 0A71
         SUB     D0534            ; 05A9 B534
         SAM                      ; 05AA 0820
         ADD     D0534            ; 05AB A534
         STX     D052C            ; 05AC 652C
         LDX     D0533            ; 05AD 9533
         DXS     1                ; 05AE 0501
         JMP     J0596            ; 05AF 1596
         LDX     D052C            ; 05B0 952C
         SLCD    1                ; 05B1 0A71
         SRL     1                ; 05B2 0A01
         IXS     0                ; 05B3 0400
         CMP                      ; 05B4 0110  not run
         STW     D052B            ; 05B5 752B
         CXA                      ; 05B6 0140
         ORE     D0535            ; 05B7 D535
         ADD     D051E            ; 05B8 A51E
         CXA                      ; 05B9 0140
         AND     D051F            ; 05BA E51F
         SNO                      ; 05BB 08A0
         CMP                      ; 05BC 0110  not run
J05BD    STW     D052C            ; 05BD 752C
J05BE    JMP     J04DD            ; 05BE 14DD
J05BF    CXA                      ; 05BF 0140  not run
         CMP                      ; 05C0 0110  not run
         CAX                      ; 05C1 0130  not run
         SAZ                      ; 05C2 0800  not run
         LDW     D0520            ; 05C3 8520  not run
         ADD     D052B            ; 05C4 A52B  not run
         CMP                      ; 05C5 0110  not run
         SUB     D0534            ; 05C6 B534  not run
         ORE     D051E            ; 05C7 D51E  not run
         SAP                      ; 05C8 0810  not run
         JMP     J0594            ; 05C9 1594  not run
J05CA    LDW     D0535            ; 05CA 8535  not run
         JMP     J05BE            ; 05CB 15BE  not run
S05CC    STX     D052E            ; 05CC 652E
         LDX     *0               ; 05CD 9800
         LDW     *0               ; 05CE 8800
         STW     D052B            ; 05CF 752B
         SAZ                      ; 05D0 0800
         LDW     D05D8            ; 05D1 85D8
         STW     D052A            ; 05D2 752A
         CLR                      ; 05D3 0100
         STW     D052C            ; 05D4 752C
         JSX     S0109            ; 05D5 2109
J05D6    LDX     D052E            ; 05D6 952E
         JMP     *1               ; 05D7 1801
D05D8    DATA    X'008F'          ; 05D8 008F  read by 05D1
D05D9    DATA    X'0000'          ; 05D9 0000  written by 05DA; read by 0621
S05DA    STX     D05D9            ; 05DA 65D9
         SML     2                ; 05DB 0082
         LDW     D09EA            ; 05DC 81EA
         STW     D05DF            ; 05DD 75DF
         JSX     S04C1            ; 05DE 24C1
D05DF    DATA    X'8000'          ; 05DF 8000  argument 1 of 05DE; LDW 0; written by 05DD
J05E0    LDW     D052B            ; 05E0 852B
         LDX     D064D            ; 05E1 964D
         SAP                      ; 05E2 0810
         LDX     D064E            ; 05E3 964E
         STX     D0646            ; 05E4 6646
         SAP                      ; 05E5 0810
         JSX     S050B            ; 05E6 250B
J05E7    LDX     D0525            ; 05E7 9525
         STX     D064F            ; 05E8 664F
         SAZ                      ; 05E9 0800
         JMP     J0623            ; 05EA 1623
         JSX     S04CA            ; 05EB 24CA  not run
         DATA    X'8650'          ; 05EC 8650  argument 1 of 05EB; LDW X'650'
J05ED    LDW     D065E            ; 05ED 865E
         STW     D0607            ; 05EE 7607
J05EF    JSX     S04C1            ; 05EF 24C1
         DATA    X'8653'          ; 05F0 8653  argument 1 of 05EF; LDW X'653'
J05F1    JSX     S01A3            ; 05F1 21A3
         DATA    X'8650'          ; 05F2 8650  argument 1 of 05F1; LDW X'650'
J05F3    JSX     S04CA            ; 05F3 24CA
         DATA    X'8650'          ; 05F4 8650  argument 1 of 05F3; LDW X'650'
J05F5    LDW     D052A            ; 05F5 852A
         SUB     D0526            ; 05F6 B526
         SAP                      ; 05F7 0810
         CLR                      ; 05F8 0100
         CMP                      ; 05F9 0110
         ADD     D065B            ; 05FA A65B
         SAP                      ; 05FB 0810
         CLR                      ; 05FC 0100  not run
         ORI     D065C            ; 05FD C65C
         STW     D0602            ; 05FE 7602
         ORI     D065F            ; 05FF C65F
         STW     D060A            ; 0600 760A
         LDW     D052B            ; 0601 852B
D0602    DATA    X'0000'          ; 0602 0000  run: an instruction stored here; written by 05FE
         ORI     D0659            ; 0603 C659
         LDX     D0607            ; 0604 9607
         STB     *0               ; 0605 3800
         IXS     1                ; 0606 0401
D0607    DATA    X'0000'          ; 0607 0000  written by 05EE 0608; read by 0604 0613
         STX     D0607            ; 0608 6607
         AND     D065B            ; 0609 E65B
D060A    DATA    X'0000'          ; 060A 0000  run: an instruction stored here; written by 0600
         STW     D052B            ; 060B 752B
         CLR                      ; 060C 0100
         STW     D052C            ; 060D 752C
         JSX     S050B            ; 060E 250B
J060F    JSX     S0110            ; 060F 2110
         DATA    X'8650'          ; 0610 8650  argument 1 of 060F; LDW X'650'
J0611    JSX     S04CA            ; 0611 24CA
         DATA    X'8650'          ; 0612 8650  argument 1 of 0611; LDW X'650'
J0613    LDW     D0607            ; 0613 8607
         CMW     D065D            ; 0614 F65D
         SEQ                      ; 0615 0860
         JMP     J05EF            ; 0616 15EF
         SML     3                ; 0617 0083
         JSX     S0CA7            ; 0618 24A7
         LDW     D064F            ; 0619 864F
         SML     2                ; 061A 0082
         JSX     S0B51            ; 061B 2351
         LDX     D0660            ; 061C 9660
         LDW     *0               ; 061D 8800
         STB     D064B+1          ; 061E 3497
         LDW     *1               ; 061F 8801
         STW     D064C            ; 0620 764C
         LDX     D05D9            ; 0621 95D9
         JSX     *0               ; 0622 2800
J0623    JSX     S04CA            ; 0623 24CA
         DATA    X'8650'          ; 0624 8650  argument 1 of 0623; LDW X'650'
J0625    JSX     S010E            ; 0625 210E
         DATA    X'8523'          ; 0626 8523  argument 1 of 0625; LDW X'523'
J0627    LDW     D052B            ; 0627 852B
         SAM                      ; 0628 0820
         JMP     J0634            ; 0629 1634
         JSX     S04C1            ; 062A 24C1
         DATA    X'8650'          ; 062B 8650  argument 1 of 062A; LDW X'650'
J062C    JSX     S010E            ; 062C 210E
         DATA    X'8656'          ; 062D 8656  argument 1 of 062C; LDW X'656'
J062E    LDW     D052B            ; 062E 852B
         SAP                      ; 062F 0810
         JMP     J063C            ; 0630 163C  not run
         JSX     S04C1            ; 0631 24C1
         DATA    X'8650'          ; 0632 8650  argument 1 of 0631; LDW X'650'
J0633    JMP     J05ED            ; 0633 15ED
J0634    LDW     D064F            ; 0634 864F
         ADD     D0520            ; 0635 A520
         STW     D064F            ; 0636 764F
         JSX     S04C1            ; 0637 24C1
         DATA    X'8650'          ; 0638 8650  argument 1 of 0637; LDW X'650'
J0639    JSX     S01A3            ; 0639 21A3
         DATA    X'8656'          ; 063A 8656  argument 1 of 0639; LDW X'656'
J063B    JMP     J0623            ; 063B 1623
J063C    LDW     D064F            ; 063C 864F  not run
         SUB     D0520            ; 063D B520  not run
         STW     D064F            ; 063E 764F  not run
         JSX     S04C1            ; 063F 24C1  not run
         DATA    X'8650'          ; 0640 8650  argument 1 of 063F; LDW X'650'
         JSX     S01A3            ; 0641 21A3  not run
         DATA    X'8653'          ; 0642 8653  argument 1 of 0641; LDW X'653'
         JSX     S04CA            ; 0643 24CA  not run
         DATA    X'8650'          ; 0644 8650  argument 1 of 0643; LDW X'650'
         JMP     J062C            ; 0645 162C  not run
D0646    DATA    X'0000'          ; 0646 0000  written by 05E4; read by 07A1 07BD
D0647    DATA    X'0000'          ; 0647 0000  written by 0CC5
         DATA    X'0000'          ; 0648 0000
         DATA    X'0000'          ; 0649 0000
         DATA    X'0000'          ; 064A 0000
D064B    DATA    X'0000'          ; 064B 0000  written by 061E 0CCD; read by 0CAB
D064C    DATA    X'0000'          ; 064C 0000  written by 0620
D064D    DATA    X'A0AE'          ; 064D A0AE  ' .'; read by 05E1
D064E    DATA    X'ADAE'          ; 064E ADAE  '-.'; read by 05E3
D064F    DATA    X'0000'          ; 064F 0000  written by 05E8 0636 063E 0CCA; read by 0619 0634 063C 0795 ...
         DATA    X'0000'          ; 0650 0000
         DATA    X'0000'          ; 0651 0000
         DATA    X'0000'          ; 0652 0000
         DATA    X'0084'          ; 0653 0084
         DATA    X'5000'          ; 0654 5000  LDB 0
         DATA    X'0000'          ; 0655 0000
         DATA    X'007D'          ; 0656 007D
         DATA    X'6666'          ; 0657 6666  STX X'666'
         DATA    X'3334'          ; 0658 3334  STB X'334'
D0659    DATA    X'00B0'          ; 0659 00B0  read by 0603
D065A    DATA    X'0006'          ; 065A 0006  read by 07C1
D065B    DATA    X'000F'          ; 065B 000F  read by 05FA 0609 0765
D065C    DATA    X'0900'          ; 065C 0900  read by 05FD
D065D    DATA    X'0C97'          ; 065D 0C97  read by 0614
D065E    DATA    X'0C8E'          ; 065E 0C8E  read by 05ED
D065F    DATA    X'0010'          ; 065F 0010  read by 05FF
D0660    DATA    X'0B8C'          ; 0660 0B8C  read by 061C
J0661    CLR                      ; 0661 0100
         STW     D077F            ; 0662 777F
         LDW     D0101            ; 0663 8101
J0664    SML     2                ; 0664 0082
         LDX     D09EA            ; 0665 91EA
         ORI     *0               ; 0666 C800
         SLC     4                ; 0667 0A54
         STW     D0778            ; 0668 7778
         SML     2                ; 0669 0082
         LDX     D09EB            ; 066A 91EB
         LDW     *2               ; 066B 8802
         SLL     1                ; 066C 0A11
         STW     D0106            ; 066D 7106
         LDW     D077F            ; 066E 877F
         SAP                      ; 066F 0810
         JSX     S0710            ; 0670 2710
J0671    LDW     D0781            ; 0671 8781
         STW     D0751            ; 0672 7751
         CLR                      ; 0673 0100
         STW     D0771            ; 0674 7771
J0675    LDW     D0520            ; 0675 8520
         STW     D0775            ; 0676 7775
         STW     D0772            ; 0677 7772
         JSX     S075C            ; 0678 275C
J0679    SML     2                ; 0679 0082
         JSX     S0B6B            ; 067A 236B
J067B    JMP     J067F            ; 067B 167F
J067C    JSX     S0739            ; 067C 2739
J067D    STW     D0775            ; 067D 7775
         STW     D0772            ; 067E 7772
J067F    LDW     D0104            ; 067F 8104
         STW     D0105            ; 0680 7105
         JSX     S075C            ; 0681 275C
J0682    LDW     D0105            ; 0682 8105
         CLB     X'C8'            ; 0683 07C8
         SNE                      ; 0684 0870
         JMP     J069C            ; 0685 169C
         CLB     X'A7'            ; 0686 07A7
         SNE                      ; 0687 0870
         JMP     J06AE            ; 0688 16AE  not run
         CLB     X'D8'            ; 0689 07D8
         SNE                      ; 068A 0870
         JMP     J06B6            ; 068B 16B6
         CLB     X'AF'            ; 068C 07AF
         SNE                      ; 068D 0870
         JMP     J06B9            ; 068E 16B9  not run
         CLB     X'C9'            ; 068F 07C9
         SNE                      ; 0690 0870
         JMP     J06CD            ; 0691 16CD
         CLB     X'C6'            ; 0692 07C6
         SEQ                      ; 0693 0860
         CLB     X'C5'            ; 0694 07C5
         SNE                      ; 0695 0870
         JMP     J06BD            ; 0696 16BD
         JMP     S07F6            ; 0697 17F6  not run
J0698    LDW     D0527            ; 0698 8527
         STW     D077F            ; 0699 777F
         LDW     D0102            ; 069A 8102
         JMP     J0664            ; 069B 1664
J069C    LDW     D077F            ; 069C 877F
         SAP                      ; 069D 0810
         JMP     J06A5            ; 069E 16A5  not run
J069F    JSX     S0756            ; 069F 2756
J06A0    JMP     J0733            ; 06A0 1733
J06A1    LDW     D0104            ; 06A1 8104
         JSX     S0746            ; 06A2 2746
J06A3    JSX     S075C            ; 06A3 275C
J06A4    JMP     J069F            ; 06A4 169F
J06A5    JSX     S0756            ; 06A5 2756  not run
         JMP     J0733            ; 06A6 1733  not run
         DATA    X'2752'          ; 06A7 2752  JSX X'752'
         DATA    X'9106'          ; 06A8 9106  LDX X'106'
         DATA    X'3800'          ; 06A9 3800  STB *0
         DATA    X'0401'          ; 06AA 0401
         DATA    X'0A10'          ; 06AB 0A10
         DATA    X'6106'          ; 06AC 6106  STX X'106'
         DATA    X'16A5'          ; 06AD 16A5  JMP X'6A5'
J06AE    LDW     D0104            ; 06AE 8104  not run
J06AF    JSX     S0746            ; 06AF 2746  not run
         JSX     S075C            ; 06B0 275C  not run
         CLB     X'A7'            ; 06B1 07A7  not run
         SEQ                      ; 06B2 0860  not run
         JMP     J06AF            ; 06B3 16AF  not run
         JSX     S075C            ; 06B4 275C  not run
         JMP     J0733            ; 06B5 1733  not run
J06B6    JSX     S07EE            ; 06B6 27EE
         DATA    X'06A0'          ; 06B7 06A0  argument 1 of 06B6
J06B8    JMP     J0733            ; 06B8 1733
J06B9    JSX     S0756            ; 06B9 2756  not run
         JMP     J0733            ; 06BA 1733  not run
         DATA    X'2710'          ; 06BB 2710  JSX X'710'
         DATA    X'16B9'          ; 06BC 16B9  JMP X'6B9'
J06BD    LDW     D0104            ; 06BD 8104
         CLB     X'C6'            ; 06BE 07C6
         SML     2                ; 06BF 0082
         SNE                      ; 06C0 0870
         JMP     X'0340'          ; 06C1 1340  not run; lands at 0B40 = J0B40 under the selection in force
         SML     1                ; 06C2 0081
         JSX     S0739            ; 06C3 2739
J06C4    STW     D0773            ; 06C4 7773
         LDW     D0104            ; 06C5 8104
         CLB     X'AE'            ; 06C6 07AE
         SEQ                      ; 06C7 0860
         JSX     S07F6            ; 06C8 27F6  not run
         JSX     S075C            ; 06C9 275C
J06CA    JSX     S0739            ; 06CA 2739
J06CB    STW     D0774            ; 06CB 7774
         JMP     J0737            ; 06CC 1737
J06CD    JSX     S0739            ; 06CD 2739
J06CE    STW     D0773            ; 06CE 7773
         JMP     J0737            ; 06CF 1737
J06D0    LDW     D0527            ; 06D0 8527
         STW     D0771            ; 06D1 7771
         LDW     D0773            ; 06D2 8773
         STW     D0772            ; 06D3 7772
         LDW     D077F            ; 06D4 877F
         SAP                      ; 06D5 0810
         JMP     J06E9            ; 06D6 16E9
         LDW     D0105            ; 06D7 8105
         CLB     X'C9'            ; 06D8 07C9
         SEQ                      ; 06D9 0860
         JSX     S07F8            ; 06DA 27F8  not run
         SML     2                ; 06DB 0082
         LDX     D09EA            ; 06DC 91EA
         LDW     *0               ; 06DD 8800
         SML     2                ; 06DE 0082
         JSX     S0B51            ; 06DF 2351
J06E0    JSX     S0756            ; 06E0 2756
J06E1    JMP     J072E            ; 06E1 172E
J06E2    LDW     D06E8            ; 06E2 86E8
         SUB     D0772            ; 06E3 B772
         CAX                      ; 06E4 0130
         LDB     *0               ; 06E5 5800
         JSX     S0746            ; 06E6 2746
J06E7    JMP     J06E0            ; 06E7 16E0
D06E8    DATA    X'171B'          ; 06E8 171B  JMP X'71B'; read by 06E2
J06E9    CLR                      ; 06E9 0100
         STW     D0537            ; 06EA 7537
J06EB    STB     D0538            ; 06EB 3270
J06EC    LDW     D070F            ; 06EC 870F
         SAZ                      ; 06ED 0800
         JMP     J06F1            ; 06EE 16F1  not run
         JSX     S0756            ; 06EF 2756
J06F0    JMP     J0706            ; 06F0 1706
J06F1    JSX     S0752            ; 06F1 2752
J06F2    CLB     X'AD'            ; 06F2 07AD
         SNE                      ; 06F3 0870
         JMP     J06EB            ; 06F4 16EB
         CLB     X'A0'            ; 06F5 07A0
         SEQ                      ; 06F6 0860
         CLB     X'AB'            ; 06F7 07AB
         SNE                      ; 06F8 0870
         JMP     J06EC            ; 06F9 16EC
         LDX     D070F            ; 06FA 970F
         SXE                      ; 06FB 0850
         JMP     J06FF            ; 06FC 16FF  not run
         JSX     S0765            ; 06FD 2765
J06FE    JMP     J06EC            ; 06FE 16EC
J06FF    JSX     S0765            ; 06FF 2765  not run
         JSX     S0752            ; 0700 2752  not run
         CLB     X'A0'            ; 0701 07A0  not run
         SEQ                      ; 0702 0860  not run
         CLB     X'20'            ; 0703 0720  not run
         SEQ                      ; 0704 0860  not run
         JMP     J06FF            ; 0705 16FF  not run
J0706    LDW     D0537            ; 0706 8537
         LDX     D0538            ; 0707 9538
         IXS     0                ; 0708 0400
         CMP                      ; 0709 0110
         SML     2                ; 070A 0082
         LDX     D09EA            ; 070B 91EA
         STW     *0               ; 070C 7800
         SML     3                ; 070D 0083
         JMP     J0C4F            ; 070E 144F
D070F    DATA    X'0000'          ; 070F 0000  written by 0B44; read by 06EC 06FA
S0710    STX     D052F            ; 0710 652F
         LDW     D0103            ; 0711 8103
         LDX     D077F            ; 0712 977F
         IXS     0                ; 0713 0400
         JMP     J071C            ; 0714 171C
         LDX     D0751            ; 0715 9751
         LLB     X'A0'            ; 0716 06A0
         STB     *0               ; 0717 3800
         STB     *1               ; 0718 3801
         CXA                      ; 0719 0140
         SUB     D0780            ; 071A B780
         SRL     1                ; 071B 0A01
J071C    STW     D077E            ; 071C 777E
         JSX     A0044            ; 071D 2044
         DATA    X'8776'          ; 071E 8776  argument 1 of 071D; LDW X'776'
J071F    LDW     D0776            ; 071F 8776
         SAP                      ; 0720 0810
         JMP     J071F            ; 0721 171F  not run
         LDX     D077A            ; 0722 977A
         DXS     1                ; 0723 0501
         LLB     X'20'            ; 0724 0620
         STB     *1               ; 0725 3801
         LDW     D0781            ; 0726 8781
         STW     D0751            ; 0727 7751
         JMP     J0744            ; 0728 1744
J0729    LDW     D077F            ; 0729 877F
         SAM                      ; 072A 0820
         JSX     S0710            ; 072B 2710
J072C    SML     2                ; 072C 0082
         JMP     J0B4D            ; 072D 134D
J072E    LDW     D0775            ; 072E 8775
         SUB     D0520            ; 072F B520
         STW     D0775            ; 0730 7775
         SAZ                      ; 0731 0800
         JMP     J0737            ; 0732 1737
J0733    LDW     D0104            ; 0733 8104
         CLB     X'A9'            ; 0734 07A9
         SEQ                      ; 0735 0860
         JMP     J0675            ; 0736 1675
J0737    SML     2                ; 0737 0082
         JMP     J0B42            ; 0738 1342
S0739    STX     D052F            ; 0739 652F
         CLR                      ; 073A 0100
         STW     D0537            ; 073B 7537
         LDW     D0104            ; 073C 8104
J073D    SML     2                ; 073D 0082
         JSX     S0B6B            ; 073E 236B
J073F    JMP     J0743            ; 073F 1743
J0740    JSX     S0765            ; 0740 2765
J0741    JSX     S075C            ; 0741 275C
J0742    JMP     J073D            ; 0742 173D
J0743    LDW     D0537            ; 0743 8537
J0744    LDX     D052F            ; 0744 952F
         JMP     *0               ; 0745 1800
S0746    STX     D052F            ; 0746 652F
         LDX     D0751            ; 0747 9751
         STB     *0               ; 0748 3800
J0749    CXA                      ; 0749 0140
         ADD     D0520            ; 074A A520
         STW     D0751            ; 074B 7751
         CMW     D0755            ; 074C F755
         SLS                      ; 074D 0840
         JMP     S07FA            ; 074E 17FA  not run
         LDB     *0               ; 074F 5800
         JMP     J0744            ; 0750 1744
D0751    DATA    X'0000'          ; 0751 0000  written by 0672 0727 074B; read by 0715 0747 0753
S0752    STX     D052F            ; 0752 652F
         LDX     D0751            ; 0753 9751
         JMP     J0749            ; 0754 1749
D0755    DATA    X'194F'          ; 0755 194F  JMP *X'14F'; read by 074C
S0756    LDW     D0772            ; 0756 8772
         SUB     D0520            ; 0757 B520
         STW     D0772            ; 0758 7772
         SAP                      ; 0759 0810
         JMP     *0               ; 075A 1800
         JMP     *1               ; 075B 1801
S075C    STX     D052E            ; 075C 652E
         LDX     D0106            ; 075D 9106
         CLR                      ; 075E 0100
         DXS     1                ; 075F 0501
         SLL     0                ; 0760 0A10
         LDB     *0               ; 0761 5800
         STW     D0104            ; 0762 7104
         STX     D0106            ; 0763 6106
         JMP     J04DD            ; 0764 14DD
S0765    AND     D065B            ; 0765 E65B
         STW     D0539            ; 0766 7539
         LDW     D0537            ; 0767 8537
         SLL     3                ; 0768 0A13
         ADD     D0537            ; 0769 A537
         ADD     D0537            ; 076A A537
         ADD     D0539            ; 076B A539
         STW     D0537            ; 076C 7537
         SAM                      ; 076D 0820
         JMP     *0               ; 076E 1800
         LLB     X'CD'            ; 076F 06CD  not run
         JMP     J07FB            ; 0770 17FB  not run
D0771    DATA    X'0000'          ; 0771 0000  written by 0674 06D1 0783; read by 0B47
D0772    DATA    X'0000'          ; 0772 0000  written by 0677 067E 06D3 0758 ...; read by 06E3 0756
D0773    DATA    X'0000'          ; 0773 0000  written by 06C4 06CE; read by 06D2 0784 079B 07BF
D0774    DATA    X'0000'          ; 0774 0000  written by 06CB; read by 0799 07AF 07B1 07B8 ...
D0775    DATA    X'0000'          ; 0775 0000  written by 0676 067D 0730; read by 072E
D0776    DATA    X'0C65'          ; 0776 0C65  read by 071F
         DATA    X'077E'          ; 0777 077E
D0778    DATA    X'0000'          ; 0778 0000  written by 0668
         DATA    X'0000'          ; 0779 0000
D077A    DATA    X'0000'          ; 077A 0000  read by 0722
         DATA    X'0000'          ; 077B 0000
         DATA    X'0000'          ; 077C 0000
         DATA    X'0000'          ; 077D 0000
D077E    DATA    X'0001'          ; 077E 0001  written by 071C
D077F    DATA    X'0000'          ; 077F 0000  written by 0662 0699; read by 066E 069C 06D4 0712 ...
D0780    DATA    X'18C8'          ; 0780 18C8  JMP *X'C8'; read by 071A
D0781    DATA    X'18CA'          ; 0781 18CA  JMP *X'CA'; read by 0671 0726
J0782    LDW     D0527            ; 0782 8527
         STW     D0771            ; 0783 7771
         LDW     D0773            ; 0784 8773
         STW     D0772            ; 0785 7772
         LDW     D077F            ; 0786 877F
         SML     2                ; 0787 0082
         SAP                      ; 0788 0810
         JMP     X'03C6'          ; 0789 13C6  lands at 0BC6 = J0BC6 under the selection in force
         SML     1                ; 078A 0081
         JSX     S05DA            ; 078B 25DA
         LDW     D07E8            ; 078C 87E8
         STW     D053A            ; 078D 753A
         LDW     D0105            ; 078E 8105
         CLB     X'C5'            ; 078F 07C5
         SNE                      ; 0790 0870
         JMP     J07BD            ; 0791 17BD
         CLB     X'C6'            ; 0792 07C6
         SEQ                      ; 0793 0860
         JSX     S07F8            ; 0794 27F8  not run
         LDW     D064F            ; 0795 864F
         SAP                      ; 0796 0810
         CLR                      ; 0797 0100  not run
         ADD     D0521            ; 0798 A521
         ADD     D0774            ; 0799 A774
         CMP                      ; 079A 0110
         ADD     D0773            ; 079B A773
         STW     D0772            ; 079C 7772
         SAP                      ; 079D 0810
         JSX     S07FA            ; 079E 27FA  not run
         JSX     S07EE            ; 079F 27EE
         DATA    X'06A0'          ; 07A0 06A0  argument 1 of 079F
J07A1    LDB     D0646            ; 07A1 548C
         JSX     S0746            ; 07A2 2746
J07A3    LDW     D064F            ; 07A3 864F
         SAP                      ; 07A4 0810
         CLR                      ; 07A5 0100  not run
         STW     D0772            ; 07A6 7772
         JSX     S07EE            ; 07A7 27EE
         DATA    X'27DE'          ; 07A8 27DE  argument 1 of 07A7; JSX X'7DE'
J07A9    LLB     X'AE'            ; 07A9 06AE
         JSX     S0746            ; 07AA 2746
J07AB    LDW     D064F            ; 07AB 864F
         SAM                      ; 07AC 0820
         CLR                      ; 07AD 0100
         CMP                      ; 07AE 0110
         CMW     D0774            ; 07AF F774
         SLE                      ; 07B0 0890
         LDW     D0774            ; 07B1 8774  not run
         STW     D0772            ; 07B2 7772
         JSX     S07EE            ; 07B3 27EE
         DATA    X'06B0'          ; 07B4 06B0  argument 1 of 07B3
J07B5    LDW     D064F            ; 07B5 864F
         SAM                      ; 07B6 0820
         CLR                      ; 07B7 0100
         ADD     D0774            ; 07B8 A774
         STW     D0772            ; 07B9 7772
         JSX     S07EE            ; 07BA 27EE
         DATA    X'27DE'          ; 07BB 27DE  argument 1 of 07BA; JSX X'7DE'
J07BC    JMP     J072E            ; 07BC 172E
J07BD    LDB     D0646            ; 07BD 548C
         JSX     S0746            ; 07BE 2746
J07BF    LDW     D0773            ; 07BF 8773
         SUB     D0774            ; 07C0 B774
         SUB     D065A            ; 07C1 B65A
         STW     D0772            ; 07C2 7772
         STW     D053C            ; 07C3 753C
         SAP                      ; 07C4 0810
         JSX     S07FA            ; 07C5 27FA  not run
         JSX     S07EE            ; 07C6 27EE
         DATA    X'27DE'          ; 07C7 27DE  argument 1 of 07C6; JSX X'7DE'
J07C8    LLB     X'AE'            ; 07C8 06AE
         JSX     S0746            ; 07C9 2746
J07CA    LDW     D0774            ; 07CA 8774
         STW     D0772            ; 07CB 7772
         JSX     S07EE            ; 07CC 27EE
         DATA    X'27DE'          ; 07CD 27DE  argument 1 of 07CC; JSX X'7DE'
J07CE    LLB     X'C5'            ; 07CE 06C5
         JSX     S0746            ; 07CF 2746
J07D0    LDW     D064F            ; 07D0 864F
         SUB     D053C            ; 07D1 B53C
         SML     2                ; 07D2 0082
         JSX     S0B51            ; 07D3 2351
         LDX     D07ED            ; 07D4 97ED
         LDB     *0               ; 07D5 5800
         JSX     S0746            ; 07D6 2746
J07D7    LDX     D07ED            ; 07D7 97ED
         LDB     *1               ; 07D8 5801
         JSX     S0746            ; 07D9 2746
J07DA    LDX     D07ED            ; 07DA 97ED
         LDB     *2               ; 07DB 5802
         JSX     S0746            ; 07DC 2746
J07DD    JMP     J072E            ; 07DD 172E
         STX     D052F            ; 07DE 652F
         LDX     D053A            ; 07DF 953A
         LDB     *0               ; 07E0 5800
         CLB     X'A0'            ; 07E1 07A0
         SNE                      ; 07E2 0870
         JMP     J07EB            ; 07E3 17EB  not run
         CLB     X'AD'            ; 07E4 07AD
         SNE                      ; 07E5 0870
         JMP     J07EB            ; 07E6 17EB  not run
         IXS     1                ; 07E7 0401
D07E8    DATA    X'0C8E'          ; 07E8 0C8E  read by 078C
         STX     D053A            ; 07E9 653A
         JMP     J0744            ; 07EA 1744
J07EB    LLB     X'B0'            ; 07EB 06B0  not run
         JMP     J0744            ; 07EC 1744  not run
D07ED    DATA    X'1719'          ; 07ED 1719  JMP X'719'; read by 07D4 07D7 07DA
S07EE    STX     D0532            ; 07EE 6532
         LDW     *0               ; 07EF 8800
         STW     D07F3            ; 07F0 77F3
J07F1    JSX     S0756            ; 07F1 2756
J07F2    JMP     J032C            ; 07F2 132C
D07F3    DATA    X'0000'          ; 07F3 0000  run: an instruction stored here; written by 07F0
J07F4    JSX     S0746            ; 07F4 2746
J07F5    JMP     J07F1            ; 07F5 17F1
S07F6    LLB     X'C4'            ; 07F6 06C4  not run
         JMP     J07FB            ; 07F7 17FB  not run
S07F8    LLB     X'D4'            ; 07F8 06D4  not run
         JMP     J07FB            ; 07F9 17FB  not run
S07FA    LLB     X'D7'            ; 07FA 06D7  not run
J07FB    SML     2                ; 07FB 0082  not run
         JMP     J0BBF            ; 07FC 13BF  not run
         DATA    X'0000'          ; 07FD 0000
         DATA    X'0000'          ; 07FE 0000
         DATA    X'0000'          ; 07FF 0000
         DATA    X'0B11'          ; 0800 0B11
J0801    JMP     J0850            ; 0801 1050
         DATA    X'0B11'          ; 0802 0B11
J0803    JMP     J0865            ; 0803 1065
         DATA    X'0B11'          ; 0804 0B11
J0805    JMP     J0B2D            ; 0805 132D
         DATA    X'0B11'          ; 0806 0B11
         DATA    X'1334'          ; 0807 1334  JMP X'334'
         DATA    X'0B17'          ; 0808 0B17
J0809    JMP     J086A            ; 0809 106A
         DATA    X'0B17'          ; 080A 0B17
J080B    JMP     J086E            ; 080B 106E
         DATA    X'0B17'          ; 080C 0B17
J080D    JMP     J087B            ; 080D 107B
         DATA    X'0B25'          ; 080E 0B25
J080F    JMP     J080D            ; 080F 100D
         DATA    X'0B17'          ; 0810 0B17
J0811    JMP     J088D            ; 0811 108D
         DATA    X'0B25'          ; 0812 0B25
J0813    JMP     J0811            ; 0813 1011
         DATA    X'0B17'          ; 0814 0B17
J0815    JMP     J0898            ; 0815 1098
         DATA    X'0B11'          ; 0816 0B11
J0817    JMP     J08B0            ; 0817 10B0
         DATA    X'0B0B'          ; 0818 0B0B
J0819    JMP     J08B4            ; 0819 10B4
         DATA    X'0B0B'          ; 081A 0B0B
J081B    JMP     J08C0            ; 081B 10C0
         DATA    X'0B0B'          ; 081C 0B0B
J081D    JMP     J08C3            ; 081D 10C3
         DATA    X'0B11'          ; 081E 0B11
J081F    JMP     J08C6            ; 081F 10C6
         DATA    X'0B25'          ; 0820 0B25
J0821    JMP     J08CB            ; 0821 10CB
         DATA    X'0B17'          ; 0822 0B17
J0823    JMP     J08D0            ; 0823 10D0
         DATA    X'0B27'          ; 0824 0B27
J0825    JMP     J08E6            ; 0825 10E6
         DATA    X'0B27'          ; 0826 0B27
J0827    JMP     J08EE            ; 0827 10EE
         DATA    X'0829'          ; 0828 0829
J0829    JMP     J08F6            ; 0829 10F6
         DATA    X'082B'          ; 082A 082B
J082B    JMP     J08FF            ; 082B 10FF
         DATA    X'082D'          ; 082C 082D
J082D    JMP     J090D            ; 082D 110D
         DATA    X'0B17'          ; 082E 0B17
J082F    JMP     J091A            ; 082F 111A
         DATA    X'0B0B'          ; 0830 0B0B
J0831    JMP     J091E            ; 0831 111E
         DATA    X'0B1D'          ; 0832 0B1D
J0833    JMP     J0940            ; 0833 1140
         DATA    X'0B1D'          ; 0834 0B1D
J0835    JMP     J094B            ; 0835 114B
         DATA    X'0B1D'          ; 0836 0B1D
J0837    JMP     J094E            ; 0837 114E
         DATA    X'0B1D'          ; 0838 0B1D
J0839    JMP     J0965            ; 0839 1165
         DATA    X'0B1D'          ; 083A 0B1D
         DATA    X'1175'          ; 083B 1175  JMP X'175'
         DATA    X'0B11'          ; 083C 0B11
J083D    JMP     J0978            ; 083D 1178
         DATA    X'0B0B'          ; 083E 0B0B
J083F    JMP     J099B            ; 083F 119B
         DATA    X'0B17'          ; 0840 0B17
J0841    JMP     J09CF            ; 0841 11CF
J0842    LDX     D09E9            ; 0842 91E9
         JMP     J0A08            ; 0843 1208
         DATA    X'0B1D'          ; 0844 0B1D
J0845    JMP     J0A22            ; 0845 1222
         DATA    X'0B1D'          ; 0846 0B1D
J0847    JMP     J0A48            ; 0847 1248
         DATA    X'0B17'          ; 0848 0B17
         DATA    X'1257'          ; 0849 1257  JMP X'257'
         DATA    X'0B11'          ; 084A 0B11
         DATA    X'1262'          ; 084B 1262  JMP X'262'
         DATA    X'0B11'          ; 084C 0B11
J084D    JMP     J0A89            ; 084D 1289
         DATA    X'0B11'          ; 084E 0B11
         DATA    X'12A8'          ; 084F 12A8  JMP X'2A8'
J0850    LDX     D09EB            ; 0850 91EB
         LDW     *0               ; 0851 8800
         LDX     D09EC            ; 0852 91EC
         ADD     *0               ; 0853 A800
J0854    LDX     D09EA            ; 0854 91EA
         STW     *0               ; 0855 7800
         LDX     D09E9            ; 0856 91E9
         DXS     0                ; 0857 0500
         JMP     J0B02            ; 0858 1302
         LDX     D0AD3            ; 0859 92D3
         DXS     0                ; 085A 0500
         JMP     J0B02            ; 085B 1302
         LDX     D09E8            ; 085C 91E8
         DXS     4                ; 085D 0504
J085E    STX     D09E8            ; 085E 61E8
         JSX     S0B51            ; 085F 2351
         JSX     S0BA4            ; 0860 23A4
         DATA    X'0B8B'          ; 0861 0B8B  argument 1 of 0860
         DATA    X'0003'          ; 0862 0003  argument 2 of 0860
J0863    LDX     D09E8            ; 0863 91E8
         JMP     J0B05            ; 0864 1305
J0865    LDX     D09EB            ; 0865 91EB
         LDW     *0               ; 0866 8800
         LDX     D09EC            ; 0867 91EC
         SUB     *0               ; 0868 B800
         JMP     J0854            ; 0869 1054
J086A    LDX     D09EB            ; 086A 91EB
J086B    LDW     *0               ; 086B 8800
         CMP                      ; 086C 0110
         JMP     J0870            ; 086D 1070
J086E    LDX     D09EB            ; 086E 91EB
J086F    LDW     *0               ; 086F 8800
J0870    LDX     D09EA            ; 0870 91EA
         STW     *0               ; 0871 7800
         LDX     D09E9            ; 0872 91E9
         DXS     0                ; 0873 0500
         JMP     J0AFF            ; 0874 12FF
         LDX     D0AD3            ; 0875 92D3
         DXS     0                ; 0876 0500
         JMP     J0AFF            ; 0877 12FF
         LDX     D09E8            ; 0878 91E8
         DXS     3                ; 0879 0503
         JMP     J085E            ; 087A 105E
J087B    LDX     D09EB            ; 087B 91EB
         LDW     *0               ; 087C 8800
         AND     D0CF5            ; 087D E4F5
         SUB     D0AC0            ; 087E B2C0
         SAP                      ; 087F 0810
         JMP     J088A            ; 0880 108A  not run
         CMP                      ; 0881 0110
         ADD     D0A42            ; 0882 A242
         SAP                      ; 0883 0810
         CLR                      ; 0884 0100  not run
         ORI     D088C            ; 0885 C08C
         STW     D0888            ; 0886 7088
         LDW     *1               ; 0887 8801
D0888    DATA    X'0000'          ; 0888 0000  run: an instruction stored here; written by 0886
         JMP     J0870            ; 0889 1070
J088A    CLR                      ; 088A 0100  not run
         JMP     J0870            ; 088B 1070  not run
D088C    DATA    X'0900'          ; 088C 0900  read by 0885
J088D    LDX     D09EB            ; 088D 91EB
         STX     D0891            ; 088E 6091
         SML     1                ; 088F 0081
         JSX     S05CC            ; 0890 25CC
D0891    DATA    X'8000'          ; 0891 8000  argument 1 of 0890; LDW 0; written by 088E
J0892    LDX     D09EA            ; 0892 91EA
         STX     D0896            ; 0893 6096
         SML     1                ; 0894 0081
         JSX     S04CA            ; 0895 24CA
D0896    DATA    X'8000'          ; 0896 8000  argument 1 of 0895; LDW 0; written by 0893
J0897    JMP     J08A4            ; 0897 10A4
J0898    LDX     D09EB            ; 0898 91EB
J0899    LDW     *2               ; 0899 8802
         STW     D0AC4            ; 089A 72C4
         LDW     *1               ; 089B 8801
         STW     D0AC3            ; 089C 72C3
         LDW     *0               ; 089D 8800
         LDX     D09EA            ; 089E 91EA
         STW     *0               ; 089F 7800
         LDW     D0AC3            ; 08A0 82C3
         STW     *1               ; 08A1 7801
         LDW     D0AC4            ; 08A2 82C4
         STW     *2               ; 08A3 7802
J08A4    LDW     D0AD3            ; 08A4 82D3
         SAM                      ; 08A5 0820
         JMP     J0AFF            ; 08A6 12FF
         LDW     D09E9            ; 08A7 81E9
         SAM                      ; 08A8 0820
         JMP     J0AFF            ; 08A9 12FF
         SML     1                ; 08AA 0081
         JSX     S05DA            ; 08AB 25DA
         JSX     S0BA4            ; 08AC 23A4
         DATA    X'0646'          ; 08AD 0646  argument 1 of 08AC
         DATA    X'0007'          ; 08AE 0007  argument 2 of 08AC
         JMP     J0AFF            ; 08AF 12FF
J08B0    LDX     D09EC            ; 08B0 91EC
         LDW     *0               ; 08B1 8800
         INV                      ; 08B2 0120
         JMP     J0916            ; 08B3 1116
J08B4    LDW     D08BF            ; 08B4 80BF
J08B5    STW     D08B9            ; 08B5 70B9
         LDX     D09ED            ; 08B6 91ED
         LDW     *0               ; 08B7 8800
         LDX     D09EC            ; 08B8 91EC
D08B9    DATA    X'0000'          ; 08B9 0000  run: an instruction stored here; written by 08B5
         LDX     D09EB            ; 08BA 91EB
         STW     *0               ; 08BB 7800
J08BC    LDX     D09E8            ; 08BC 91E8
         DXS     5                ; 08BD 0505
         JMP     J0B04            ; 08BE 1304
D08BF    DATA    X'E800'          ; 08BF E800  AND *0; read by 08B4
J08C0    LDW     D08C2            ; 08C0 80C2
         JMP     J08B5            ; 08C1 10B5
D08C2    DATA    X'D800'          ; 08C2 D800  ORE *0; read by 08C0
J08C3    LDW     D08C5            ; 08C3 80C5
         JMP     J08B5            ; 08C4 10B5
D08C5    DATA    X'C800'          ; 08C5 C800  ORI *0; read by 08C3
J08C6    LDX     D09EC            ; 08C6 91EC
         LDW     *0               ; 08C7 8800
         SAP                      ; 08C8 0810
         CMP                      ; 08C9 0110
         JMP     J0916            ; 08CA 1116
J08CB    LDX     D09EB            ; 08CB 91EB
         LDW     *1               ; 08CC 8801
         SAM                      ; 08CD 0820
         JMP     J0815            ; 08CE 1015  not run
         JMP     J0823            ; 08CF 1023
J08D0    LDX     D09EB            ; 08D0 91EB
J08D1    LDW     *2               ; 08D1 8802
         CMP                      ; 08D2 0110
         SAM                      ; 08D3 0820
         JMP     J08E2            ; 08D4 10E2
         AND     D0A19            ; 08D5 E219
         STW     D0AC4            ; 08D6 72C4
         LDW     *1               ; 08D7 8801
         INV                      ; 08D8 0120
J08D9    STW     D0AC3            ; 08D9 72C3
         LDW     *0               ; 08DA 8800
         LDX     D09EA            ; 08DB 91EA
         STW     *0               ; 08DC 7800
         LDW     D0AC3            ; 08DD 82C3
         STW     *1               ; 08DE 7801
         LDW     D0AC4            ; 08DF 82C4
         STW     *2               ; 08E0 7802
         JMP     J0AFF            ; 08E1 12FF
J08E2    STW     D0AC4            ; 08E2 72C4
         LDW     *1               ; 08E3 8801
         CMP                      ; 08E4 0110
         JMP     J08D9            ; 08E5 10D9
J08E6    JSX     S0B29            ; 08E6 2329
J08E7    LDX     D09EB            ; 08E7 91EB
         LDW     *0               ; 08E8 8800
         LDX     D09EC            ; 08E9 91EC
         ORE     *0               ; 08EA D800
         SAM                      ; 08EB 0820
         JMP     J086F            ; 08EC 106F  not run
         JMP     J086B            ; 08ED 106B
J08EE    JSX     S0B29            ; 08EE 2329
J08EF    LDX     D09EB            ; 08EF 91EB
         LDW     *1               ; 08F0 8801
         LDX     D09EC            ; 08F1 91EC
         ORE     *1               ; 08F2 D801
         SAM                      ; 08F3 0820
         JMP     J0899            ; 08F4 1099  not run
         JMP     J08D1            ; 08F5 10D1
; STOP: prints the STOP at 0A46, then leaves through PTIOS's X'40' for its loader.
J08F6    JSX     S0B90            ; 08F6 2390
         DATA    X'0A46'          ; 08F7 0A46  argument 1 of 08F6
         LDW     D09E8            ; 08F8 81E8
         SUB     D0A87            ; 08F9 B287
         STW     D08FC            ; 08FA 70FC
         JSX     S0B90            ; 08FB 2390
D08FC    DATA    X'8000'          ; 08FC 8000  argument 1 of 08FB; LDW 0; written by 08FA
J08FD    SML     0                ; 08FD 0080
         JMP     A0040            ; 08FE 1040
; PAUSE: prints the PAUS at 0A44 and waits for SENSE 3 up, then down (statements session).
J08FF    JSX     S0B90            ; 08FF 2390
         DATA    X'0A44'          ; 0900 0A44  argument 1 of 08FF
         LDW     D09E8            ; 0901 81E8
         SUB     D0A87            ; 0902 B287
         STW     D0905            ; 0903 7105
         JSX     S0B90            ; 0904 2390
D0905    DATA    X'8000'          ; 0905 8000  argument 1 of 0904; LDW 0; written by 0903
J0906    SS3                      ; 0906 08F0
         JMP     J0909            ; 0907 1109
         JMP     J0906            ; 0908 1106  not run
J0909    SS3                      ; 0909 08F0
         JMP     J0909            ; 090A 1109  not run
         JMP     J0AFF            ; 090B 12FF
D090C    DATA    X'0000'          ; 090C 0000  written by 0BA8
J090D    LDX     D09EC            ; 090D 91EC
         LDW     *0               ; 090E 8800
         AND     D0A88            ; 090F E288
         SLL     4                ; 0910 0A14
         ADD     D0919            ; 0911 A119
         STW     D0914            ; 0912 7114
         CLR                      ; 0913 0100
D0914    DATA    X'0000'          ; 0914 0000  run: an instruction stored here; written by 0912
         DATA    X'8241'          ; 0915 8241  LDW X'241'
J0916    LDX     D09EB            ; 0916 91EB
         STW     *0               ; 0917 7800
         JMP     J0B02            ; 0918 1302
D0919    DATA    X'08C0'          ; 0919 08C0  read by 0911
J091A    JSX     S0B29            ; 091A 2329
J091B    LDX     D09EB            ; 091B 91EB
         LDW     *0               ; 091C 8800
         JMP     J0928            ; 091D 1128
J091E    LDX     D09EA            ; 091E 91EA
         LDW     *0               ; 091F 8800
         LDX     D09EB            ; 0920 91EB
         ADD     *0               ; 0921 A800
         LDX     D09EA            ; 0922 91EA
         STW     *0               ; 0923 7800
         LDX     D09EC            ; 0924 91EC
         CMW     *0               ; 0925 F800
         SLE                      ; 0926 0890
         JMP     J0937            ; 0927 1137
J0928    LDX     D0AD4            ; 0928 92D4
         DXS     0                ; 0929 0500
         JMP     J08BC            ; 092A 10BC
         LDX     D09E8            ; 092B 91E8
         DXS     5                ; 092C 0505
         STX     D09E8            ; 092D 61E8
         LDX     D0936            ; 092E 9136
         STX     D0B8A            ; 092F 638A
J0930    JSX     S0B51            ; 0930 2351
         JSX     S0BA4            ; 0931 23A4
         DATA    X'0B8A'          ; 0932 0B8A  argument 1 of 0931
         DATA    X'0004'          ; 0933 0004  argument 2 of 0931
         LDX     D09E8            ; 0934 91E8
         JMP     J0B04            ; 0935 1304
D0936    DATA    X'C4CF'          ; 0936 C4CF  'DO'; read by 092E
J0937    LDX     D09ED            ; 0937 91ED
         LDW     D0AD4            ; 0938 82D4
         SAM                      ; 0939 0820
         JMP     J0B04            ; 093A 1304
         JSX     S0BA4            ; 093B 23A4
         DATA    X'0AC9'          ; 093C 0AC9  argument 1 of 093B
         DATA    X'0001'          ; 093D 0001  argument 2 of 093B
         LDX     D09ED            ; 093E 91ED
         JMP     J0B04            ; 093F 1304
J0940    LDW     D094D            ; 0940 814D
         STW     D0B8A            ; 0941 738A
         LDX     D09EA            ; 0942 91EA
J0943    LDX     *2               ; 0943 9802
         LDW     D0AD4            ; 0944 82D4
         SAM                      ; 0945 0820
         JMP     J0B04            ; 0946 1304
         STX     D09E8            ; 0947 61E8
         LDX     D09EA            ; 0948 91EA
         LDW     *1               ; 0949 8801
         JMP     J0930            ; 094A 1130
J094B    LDX     D09EA            ; 094B 91EA
         JMP     J0B04            ; 094C 1304
D094D    DATA    X'C7CF'          ; 094D C7CF  'GO'; read by 0940
J094E    LDW     D09E8            ; 094E 81E8
         SUB     D09EA            ; 094F B1EA
         CAX                      ; 0950 0130
         DXS     1                ; 0951 0501
J0952    LDX     *0               ; 0952 9800
         IXS     0                ; 0953 0400
         JMP     J0952            ; 0954 1152  not run
         LDW     *0               ; 0955 8800
         CMW     D0A3D            ; 0956 F23D
         SGR                      ; 0957 0880
         LDW     D0A41            ; 0958 8241  not run
         SUB     D09EA            ; 0959 B1EA
         CMW     D0A3D            ; 095A F23D
         SLS                      ; 095B 0840
         LDW     D0A43            ; 095C 8243  not run
         ADD     D09EA            ; 095D A1EA
         CMP                      ; 095E 0110
         ADD     D09E8            ; 095F A1E8
         CAX                      ; 0960 0130
         DXS     1                ; 0961 0501
         LDX     *0               ; 0962 9800
         STX     D09EA            ; 0963 61EA
         JMP     J0833            ; 0964 1033
J0965    LDX     D09EA            ; 0965 91EA
         LDW     *0               ; 0966 8800
         LDX     D0974            ; 0967 9174
         STX     D0B8A            ; 0968 638A
         LDX     D09E8            ; 0969 91E8
         DXS     4                ; 096A 0504
         SAZ                      ; 096B 0800
         JMP     J096F            ; 096C 116F
         LDX     *1               ; 096D 9801
         JMP     J0972            ; 096E 1172
J096F    SAM                      ; 096F 0820
         DXS     2                ; 0970 0502
         LDX     *2               ; 0971 9802
J0972    STX     D09EA            ; 0972 61EA
         JMP     J0943            ; 0973 1143
D0974    DATA    X'C9C6'          ; 0974 C9C6  'IF'; read by 0967
         DATA    X'91EA'          ; 0975 91EA  LDX X'1EA'
         DATA    X'8801'          ; 0976 8801  LDW *1
         DATA    X'1167'          ; 0977 1167  JMP X'167'
J0978    LDX     D09EA            ; 0978 91EA
         LDW     *0               ; 0979 8800
         LDX     D09EB            ; 097A 91EB
         CMW     *4               ; 097B F804
         SLE                      ; 097C 0890
         JMP     J0993            ; 097D 1193  not run
         SUB     D0A41            ; 097E B241
         SAP                      ; 097F 0810
         JMP     J0998            ; 0980 1198  not run
         STW     D0AC2            ; 0981 72C2
         LDX     *0               ; 0982 9800
         SLLD    4                ; 0983 0A34
         DXS     0                ; 0984 0500
         JMP     J098A            ; 0985 118A  not run
         LDW     D0AC2            ; 0986 82C2
         SLL     1                ; 0987 0A11
         ADD     D0AC2            ; 0988 A2C2
         STW     D0AC2            ; 0989 72C2
J098A    LDX     D09EB            ; 098A 91EB
         LDW     *3               ; 098B 8803
         CAX                      ; 098C 0130
         SAP                      ; 098D 0810
         LDW     *0               ; 098E 8800  not run
         ADD     D0AC2            ; 098F A2C2
         LDX     D09EC            ; 0990 91EC
         STW     *0               ; 0991 7800
         JMP     J0B02            ; 0992 1302
J0993    JSX     S0B90            ; 0993 2390  not run
         DATA    X'0ABE'          ; 0994 0ABE  argument 1 of 0993
J0995    CLR                      ; 0995 0100  not run
         STW     D0AC2            ; 0996 72C2  not run
         JMP     J098A            ; 0997 118A  not run
J0998    JSX     S0B90            ; 0998 2390  not run
         DATA    X'0AC5'          ; 0999 0AC5  argument 1 of 0998
         JMP     J0995            ; 099A 1195  not run
J099B    LDX     D09EA            ; 099B 91EA
         LDW     *0               ; 099C 8800
         LDX     D09EC            ; 099D 91EC
         CMW     *5               ; 099E F805
         IXS     4                ; 099F 0404
D09A0    DATA    X'A0B0'          ; 09A0 A0B0  ' 0'; read by 0B87
         STX     D09A8            ; 09A1 61A8
         SLE                      ; 09A2 0890
         JMP     J09C7            ; 09A3 11C7  not run
         SUB     D0A41            ; 09A4 B241
         SAP                      ; 09A5 0810
         JMP     J09CC            ; 09A6 11CC  not run
         JSX     S0BB0            ; 09A7 23B0
D09A8    DATA    X'8000'          ; 09A8 8000  argument 1 of 09A7; LDW 0; written by 09A1
         STW     D0AC2            ; 09A9 72C2
         LDX     D09EB            ; 09AA 91EB
         LDW     *0               ; 09AB 8800
         LDX     D09EC            ; 09AC 91EC
         CMW     *4               ; 09AD F804
         SLE                      ; 09AE 0890
         JMP     J09C7            ; 09AF 11C7  not run
         SUB     D0A41            ; 09B0 B241
         SAP                      ; 09B1 0810
         JMP     J09CC            ; 09B2 11CC  not run
         ADD     D0AC2            ; 09B3 A2C2
         STW     D0AC2            ; 09B4 72C2
         LDX     D09EC            ; 09B5 91EC
         LDX     *0               ; 09B6 9800
         SLLD    4                ; 09B7 0A34
         DXS     0                ; 09B8 0500
         JMP     J09BE            ; 09B9 11BE  not run
         LDW     D0AC2            ; 09BA 82C2
         SLL     1                ; 09BB 0A11
         ADD     D0AC2            ; 09BC A2C2
         STW     D0AC2            ; 09BD 72C2
J09BE    LDX     D09EC            ; 09BE 91EC
         LDW     *3               ; 09BF 8803
         CAX                      ; 09C0 0130
         SAP                      ; 09C1 0810
         LDW     *0               ; 09C2 8800  not run
         ADD     D0AC2            ; 09C3 A2C2
         LDX     D09ED            ; 09C4 91ED
         STW     *0               ; 09C5 7800
         JMP     J08BC            ; 09C6 10BC
J09C7    JSX     S0B90            ; 09C7 2390  not run
         DATA    X'0ABE'          ; 09C8 0ABE  argument 1 of 09C7
J09C9    CLR                      ; 09C9 0100  not run
         STW     D0AC2            ; 09CA 72C2  not run
         JMP     J09BE            ; 09CB 11BE  not run
J09CC    JSX     S0B90            ; 09CC 2390  not run
         DATA    X'0AC5'          ; 09CD 0AC5  argument 1 of 09CC
         JMP     J09C9            ; 09CE 11C9  not run
J09CF    LDW     D0AD4            ; 09CF 82D4
         SAM                      ; 09D0 0820
         JMP     J09E3            ; 09D1 11E3
         LDX     D09EA            ; 09D2 91EA
         LDW     *0               ; 09D3 8800
         SRCD    8                ; 09D4 0A68
         LLB     X'A0'            ; 09D5 06A0
         SLCD    8                ; 09D6 0A78
         STW     D0B8C            ; 09D7 738C
         LDW     *1               ; 09D8 8801
         STW     D0B8D            ; 09D9 738D
         LDW     *2               ; 09DA 8802
         STW     D0B8E            ; 09DB 738E
         LDW     D0A55            ; 09DC 8255
         STW     D0B8A            ; 09DD 738A
         LDW     D0A56            ; 09DE 8256
         STW     D0B8B            ; 09DF 738B
         JSX     S0BA4            ; 09E0 23A4
         DATA    X'0B8A'          ; 09E1 0B8A  argument 1 of 09E0
         DATA    X'0005'          ; 09E2 0005  argument 2 of 09E0
J09E3    LDX     D09EA            ; 09E3 91EA
         LDX     *3               ; 09E4 9803
         STX     D09E9            ; 09E5 61E9
         LDX     *0               ; 09E6 9800
         JMP     *0               ; 09E7 1800
D09E8    DATA    X'0000'          ; 09E8 0000  written by 085E 092D 0947 0A4D ...; read by 00AF 00BC 00C8 00EA ...
D09E9    DATA    X'0000'          ; 09E9 0000  written by 09E5 0A2A 0A3E 0B08; read by 00AB 00D4 00E6 0842 ...
D09EA    DATA    X'0000'          ; 09EA 0000  written by 00DE 0963 0972 0B22; read by 05DC 0665 06DC 070B ...
D09EB    DATA    X'0000'          ; 09EB 0000  written by 0A30 0A32 0B1C 0BDE ...; read by 066A 0850 0865 086A ...
D09EC    DATA    X'0000'          ; 09EC 0000  written by 0A10 0A25 0A36 0A98 ...; read by 0852 0867 08B0 08B8 ...
D09ED    DATA    X'0000'          ; 09ED 0000  written by 0B10; read by 08B6 0937 093E 09C4
         DATA    X'81E8'          ; 09EE 81E8  LDW X'1E8'
         DATA    X'B287'          ; 09EF B287  SUB X'287'
         DATA    X'B1EB'          ; 09F0 B1EB  '1k'
         DATA    X'B241'          ; 09F1 B241  SUB X'241'
         DATA    X'71ED'          ; 09F2 71ED  STW X'1ED'
         DATA    X'71E8'          ; 09F3 71E8  STW X'1E8'
         DATA    X'91EB'          ; 09F4 91EB  LDX X'1EB'
         DATA    X'0501'          ; 09F5 0501
         DATA    X'11F9'          ; 09F6 11F9  JMP X'1F9'
         DATA    X'91E9'          ; 09F7 91E9  LDX X'1E9'
         DATA    X'1801'          ; 09F8 1801  JMP *1
         DATA    X'61EB'          ; 09F9 61EB  STX X'1EB'
         DATA    X'91ED'          ; 09FA 91ED  LDX X'1ED'
         DATA    X'0401'          ; 09FB 0401
D09FC    DATA    X'8000'          ; 09FC 8000  LDW 0; read by 0AE4 0BB9
         DATA    X'61ED'          ; 09FD 61ED  STX X'1ED'
         DATA    X'9800'          ; 09FE 9800  LDX *0
         DATA    X'0400'          ; 09FF 0400
         DATA    X'9800'          ; 0A00 9800  LDX *0
         DATA    X'0140'          ; 0A01 0140
         DATA    X'91E9'          ; 0A02 91E9  LDX X'1E9'
         DATA    X'0401'          ; 0A03 0401
         DATA    X'0A10'          ; 0A04 0A10
         DATA    X'61E9'          ; 0A05 61E9  STX X'1E9'
         DATA    X'7800'          ; 0A06 7800  STW *0
         DATA    X'11F4'          ; 0A07 11F4  JMP X'1F4'
J0A08    LDW     *1               ; 0A08 8801
         CMW     D09EB            ; 0A09 F1EB
         SEQ                      ; 0A0A 0860
         JMP     J0BBD            ; 0A0B 13BD  not run
         CMP                      ; 0A0C 0110
         ADD     D09E8            ; 0A0D A1E8
         SUB     D0A88            ; 0A0E B288
         STW     *2               ; 0A0F 7802
         STW     D09EC            ; 0A10 71EC
J0A11    LDX     D09EB            ; 0A11 91EB
         DXS     1                ; 0A12 0501
         JMP     J0A32            ; 0A13 1232
         LDW     D09E9            ; 0A14 81E9
         AND     D0A19            ; 0A15 E219
         SUB     D0A41            ; 0A16 B241
         CAX                      ; 0A17 0130
         JMP     J0B04            ; 0A18 1304
D0A19    DATA    X'7FFF'          ; 0A19 7FFF  STW *X'7FF'; read by 08D5 0A15
D0A1A    DATA    X'0B8A'          ; 0A1A 0B8A  written by 0BA6; read by 0BAB
         DATA    X'090C'          ; 0A1B 090C
         DATA    X'003E'          ; 0A1C 003E
         DATA    X'0000'          ; 0A1D 0000
         DATA    X'0000'          ; 0A1E 0000
         DATA    X'0000'          ; 0A1F 0000
         DATA    X'0000'          ; 0A20 0000
         DATA    X'0000'          ; 0A21 0000
J0A22    LDW     D09E8            ; 0A22 81E8
         SUB     D09EA            ; 0A23 B1EA
         SUB     D0A87            ; 0A24 B287
         STW     D09EC            ; 0A25 71EC
         CAX                      ; 0A26 0130
         SUB     D0A41            ; 0A27 B241
         LDX     *0               ; 0A28 9800
         LDX     *3               ; 0A29 9803
         STX     D09E9            ; 0A2A 61E9
         STW     *2               ; 0A2B 7802
         LDW     *1               ; 0A2C 8801
         CMW     D09EA            ; 0A2D F1EA
         SEQ                      ; 0A2E 0860
         JMP     J0BBD            ; 0A2F 13BD
         STW     D09EB            ; 0A30 71EB  not run
         JMP     J0A11            ; 0A31 1211  not run
J0A32    STX     D09EB            ; 0A32 61EB
         LDX     D09EC            ; 0A33 91EC
         IXS     1                ; 0A34 0401
D0A35    DATA    X'CDD3'          ; 0A35 CDD3  the message for a function the program refers to and the library does not have, such as TANH (errors session)  --  'MS'; read by 0BBD
         STX     D09EC            ; 0A36 61EC
         LDX     *0               ; 0A37 9800
         IXS     0                ; 0A38 0400
         LDX     *0               ; 0A39 9800
         CXA                      ; 0A3A 0140
         LDX     D09E9            ; 0A3B 91E9
         DXS     1                ; 0A3C 0501
D0A3D    DATA    X'0000'          ; 0A3D 0000  read by 0956 095A 0B52
         STX     D09E9            ; 0A3E 61E9
         STW     *0               ; 0A3F 7800
         JMP     J0A11            ; 0A40 1211
D0A41    DATA    X'0001'          ; 0A41 0001  read by 0958 097E 09A4 09B0 ...
D0A42    DATA    X'000F'          ; 0A42 000F  read by 0882 0BDD 0C17 0C1C
D0A43    DATA    X'FFFF'          ; 0A43 FFFF  CMW *X'7FF'; read by 095C 0C5D
         DATA    X'D0C1'          ; 0A44 D0C1  'PA'
         DATA    X'D5D3'          ; 0A45 D5D3  'US'
         DATA    X'D3D4'          ; 0A46 D3D4  'ST'
         DATA    X'CFD0'          ; 0A47 CFD0  'OP'
J0A48    LDX     D09EA            ; 0A48 91EA
         LDX     *2               ; 0A49 9802
         LDW     D0AD4            ; 0A4A 82D4
         SAM                      ; 0A4B 0820
         JMP     J0B04            ; 0A4C 1304
         STX     D09E8            ; 0A4D 61E8
         JSX     S0BA4            ; 0A4E 23A4
         DATA    X'0A52'          ; 0A4F 0A52  argument 1 of 0A4E
         DATA    X'0003'          ; 0A50 0003  argument 2 of 0A4E
         JMP     J0863            ; 0A51 1063
         DATA    X'D2C5'          ; 0A52 D2C5  'RE'
         DATA    X'D4D5'          ; 0A53 D4D5  'TU'
         DATA    X'D2CE'          ; 0A54 D2CE  'RN'
D0A55    DATA    X'C3C1'          ; 0A55 C3C1  'CA'; read by 09DC
D0A56    DATA    X'CCCC'          ; 0A56 CCCC  'LL'; read by 09DE
         DATA    X'91EB'          ; 0A57 91EB  LDX X'1EB'
         DATA    X'8800'          ; 0A58 8800  LDW *0
         DATA    X'0A14'          ; 0A59 0A14
         DATA    X'0081'          ; 0A5A 0081
         DATA    X'7778'          ; 0A5B 7778  STW X'778'
         DATA    X'81EA'          ; 0A5C 81EA  LDW X'1EA'
         DATA    X'34BF'          ; 0A5D 34BF  STB X'4BF'
         DATA    X'0080'          ; 0A5E 0080
         DATA    X'2000'          ; 0A5F 2000  JSX 0
         DATA    X'8776'          ; 0A60 8776  LDW X'776'
         DATA    X'12FF'          ; 0A61 12FF  JMP X'2FF'
         DATA    X'91EB'          ; 0A62 91EB  LDX X'1EB'
         DATA    X'8800'          ; 0A63 8800  LDW *0
         DATA    X'91EC'          ; 0A64 91EC  LDX X'1EC'
         DATA    X'9800'          ; 0A65 9800  LDX *0
         DATA    X'0500'          ; 0A66 0500
         DATA    X'1273'          ; 0A67 1273  JMP X'273'
         DATA    X'0810'          ; 0A68 0810
         DATA    X'0110'          ; 0A69 0110
         DATA    X'B287'          ; 0A6A B287  SUB X'287'
         DATA    X'0820'          ; 0A6B 0820
         DATA    X'1272'          ; 0A6C 1272  JMP X'272'
         DATA    X'A241'          ; 0A6D A241  ADD X'241'
         DATA    X'0820'          ; 0A6E 0820
         DATA    X'1274'          ; 0A6F 1274  JMP X'274'
         DATA    X'8219'          ; 0A70 8219  LDW X'219'
         DATA    X'1286'          ; 0A71 1286  JMP X'286'
D0A72    DATA    X'0100'          ; 0A72 0100  written by 0CA7; read by 0CCE
         DATA    X'0800'          ; 0A73 0800
         DATA    X'8241'          ; 0A74 8241  LDW X'241'
         DATA    X'71ED'          ; 0A75 71ED  STW X'1ED'
         DATA    X'0140'          ; 0A76 0140
         DATA    X'0400'          ; 0A77 0400
         DATA    X'0110'          ; 0A78 0110
         DATA    X'B241'          ; 0A79 B241  SUB X'241'
         DATA    X'0810'          ; 0A7A 0810
         DATA    X'1285'          ; 0A7B 1285  JMP X'285'
         DATA    X'71EC'          ; 0A7C 71EC  STW X'1EC'
         DATA    X'91EB'          ; 0A7D 91EB  LDX X'1EB'
         DATA    X'6281'          ; 0A7E 6281  STX X'281'
         DATA    X'81ED'          ; 0A7F 81ED  LDW X'1ED'
         DATA    X'23B0'          ; 0A80 23B0  JSX X'3B0'
         DATA    X'8000'          ; 0A81 8000  LDW 0
         DATA    X'71ED'          ; 0A82 71ED  STW X'1ED'
         DATA    X'81EC'          ; 0A83 81EC  LDW X'1EC'
         DATA    X'1279'          ; 0A84 1279  JMP X'279'
         DATA    X'81ED'          ; 0A85 81ED  LDW X'1ED'
         DATA    X'1054'          ; 0A86 1054  JMP X'54'
D0A87    DATA    X'0002'          ; 0A87 0002  read by 08F9 0902 0A24
D0A88    DATA    X'0003'          ; 0A88 0003  read by 090F 0A0E
J0A89    SML     1                ; 0A89 0081
         JSX     S04C1            ; 0A8A 24C1
         DATA    X'8523'          ; 0A8B 8523  argument 1 of 0A8A; LDW X'523'
J0A8C    LDW     D09EB            ; 0A8C 81EB
         JSX     S0CED            ; 0A8D 24ED
         LDX     D09EC            ; 0A8E 91EC
         LDW     *0               ; 0A8F 8800
J0A90    SAZ                      ; 0A90 0800
         JMP     J0A95            ; 0A91 1295
         LDX     D09EA            ; 0A92 91EA
         SML     0                ; 0A93 0080
         JMP     J00CD            ; 0A94 10CD
J0A95    SAP                      ; 0A95 0810
         JMP     J0AA0            ; 0A96 12A0  not run
         SUB     D0A41            ; 0A97 B241
         STW     D09EC            ; 0A98 71EC
         LDW     D09EB            ; 0A99 81EB
         STW     D0A9D            ; 0A9A 729D
         SML     0                ; 0A9B 0080
         JSX     S01A3            ; 0A9C 21A3
D0A9D    DATA    X'8000'          ; 0A9D 8000  argument 1 of 0A9C; LDW 0; written by 0A9A
J0A9E    LDW     D09EC            ; 0A9E 81EC
         JMP     J0A90            ; 0A9F 1290
J0AA0    ADD     D0A41            ; 0AA0 A241  not run
         STW     D09EC            ; 0AA1 71EC  not run
         LDW     D09EB            ; 0AA2 81EB  not run
         STW     D0AA6            ; 0AA3 72A6  not run
         SML     0                ; 0AA4 0080  not run
         JSX     S019D            ; 0AA5 219D  not run
D0AA6    DATA    X'8000'          ; 0AA6 8000  argument 1 of 0AA5; LDW 0; written by 0AA3
         JMP     J0A9E            ; 0AA7 129E  not run
         DATA    X'81EC'          ; 0AA8 81EC  LDW X'1EC'
         DATA    X'24ED'          ; 0AA9 24ED  JSX X'4ED'
         DATA    X'72B6'          ; 0AAA 72B6  STW X'2B6'
         DATA    X'81EB'          ; 0AAB 81EB  LDW X'1EB'
         DATA    X'24ED'          ; 0AAC 24ED  JSX X'4ED'
         DATA    X'72B3'          ; 0AAD 72B3  STW X'2B3'
         DATA    X'81EA'          ; 0AAE 81EA  LDW X'1EA'
         DATA    X'72B9'          ; 0AAF 72B9  STW X'2B9'
         DATA    X'72BC'          ; 0AB0 72BC  STW X'2BC'
         DATA    X'0081'          ; 0AB1 0081
         DATA    X'245F'          ; 0AB2 245F  JSX X'45F'
         DATA    X'8000'          ; 0AB3 8000  LDW 0
         DATA    X'0080'          ; 0AB4 0080
         DATA    X'21A3'          ; 0AB5 21A3  JSX X'1A3'
         DATA    X'8000'          ; 0AB6 8000  LDW 0
         DATA    X'0081'          ; 0AB7 0081
         DATA    X'24CA'          ; 0AB8 24CA  JSX X'4CA'
         DATA    X'8000'          ; 0AB9 8000  LDW 0
         DATA    X'0080'          ; 0ABA 0080
         DATA    X'23DE'          ; 0ABB 23DE  JSX X'3DE'
         DATA    X'8000'          ; 0ABC 8000  LDW 0
         DATA    X'1292'          ; 0ABD 1292  JMP X'292'
         DATA    X'D3CF'          ; 0ABE D3CF  'SO'
D0ABF    DATA    X'000A'          ; 0ABF 000A  read by 0B81
D0AC0    DATA    X'0080'          ; 0AC0 0080  read by 087E
D0AC1    DATA    X'0000'          ; 0AC1 0000  written by 0B59 0B79; read by 0AE5 0B77
D0AC2    DATA    X'0000'          ; 0AC2 0000  written by 0981 0989 0996 09A9 ...; read by 0986 0988 098F 09B3 ...
D0AC3    DATA    X'0000'          ; 0AC3 0000  written by 089C 08D9; read by 08A0 08DD
D0AC4    DATA    X'0000'          ; 0AC4 0000  written by 089A 08D6 08E2; read by 08A2 08DF
         DATA    X'D3D5'          ; 0AC5 D3D5  'SU'
D0AC6    DATA    X'0000'          ; 0AC6 0000  written by 0BC2
         DATA    X'A0A0'          ; 0AC7 A0A0  '  '
         DATA    X'A0A0'          ; 0AC8 A0A0  '  '
         DATA    X'AAAA'          ; 0AC9 AAAA  '**'
D0ACA    DATA    X'0AC1'          ; 0ACA 0AC1  written by 0ADD; read by 0AE1
         DATA    X'0520'          ; 0ACB 0520
         DATA    X'001B'          ; 0ACC 001B
         DATA    X'0000'          ; 0ACD 0000
         DATA    X'0000'          ; 0ACE 0000
         DATA    X'0000'          ; 0ACF 0000
         DATA    X'0000'          ; 0AD0 0000
         DATA    X'0000'          ; 0AD1 0000
D0AD2    DATA    X'0000'          ; 0AD2 0000  written by 0B55 0B84; read by 0B74
D0AD3    DATA    X'0000'          ; 0AD3 0000  written by 0AEB 0AF3 0AF9; read by 00DA 0859 0875 08A4
D0AD4    DATA    X'0000'          ; 0AD4 0000  written by 0AE8 0AF2 0AF8; read by 0928 0938 0944 09CF ...
J0AD5    SGM                      ; 0AD5 0050
         LDW     A0CFD            ; 0AD6 84FD
         STW     D09E8            ; 0AD7 71E8
J0AD8    JSX     S0B90            ; 0AD8 2390
         DATA    X'0AC7'          ; 0AD9 0AC7  argument 1 of 0AD8
J0ADA    SS3                      ; 0ADA 08F0
         JMP     J0ADA            ; 0ADB 12DA  not run
         LLB     X'A'             ; 0ADC 060A
         STB     D0ACA            ; 0ADD 3594
         SML     0                ; 0ADE 0080
         JSX     A0044            ; 0ADF 2044
         DATA    X'8ACA'          ; 0AE0 8ACA  argument 1 of 0ADF; '\nJ'
J0AE1    LDW     D0ACA            ; 0AE1 82CA
         SAP                      ; 0AE2 0810
         JMP     J0AE1            ; 0AE3 12E1  not run
         LDX     D09FC            ; 0AE4 91FC
         LDB     D0AC1            ; 0AE5 5582
         CLB     X'C3'            ; 0AE6 07C3
         SNE                      ; 0AE7 0870
         STX     D0AD4            ; 0AE8 62D4
         CLB     X'D6'            ; 0AE9 07D6
         SNE                      ; 0AEA 0870
         STX     D0AD3            ; 0AEB 62D3
         CLB     X'C5'            ; 0AEC 07C5
         SNE                      ; 0AED 0870
         JMP     J08FD            ; 0AEE 10FD  not run
         CLB     X'C2'            ; 0AEF 07C2
         SEQ                      ; 0AF0 0860
         JMP     J0AF4            ; 0AF1 12F4
         STX     D0AD4            ; 0AF2 62D4
         STX     D0AD3            ; 0AF3 62D3
J0AF4    CLB     X'CE'            ; 0AF4 07CE
         SEQ                      ; 0AF5 0860
         JMP     J0AFA            ; 0AF6 12FA
         CLR                      ; 0AF7 0100
         STW     D0AD4            ; 0AF8 72D4
         STW     D0AD3            ; 0AF9 72D3
J0AFA    CLB     X'C7'            ; 0AFA 07C7
         SEQ                      ; 0AFB 0860
         JMP     J0AD8            ; 0AFC 12D8
         LDX     D09E8            ; 0AFD 91E8
         JMP     J0B04            ; 0AFE 1304
J0AFF    LDX     D09E8            ; 0AFF 91E8
         DXS     3                ; 0B00 0503
         JMP     J0B04            ; 0B01 1304
J0B02    LDX     D09E8            ; 0B02 91E8
         DXS     4                ; 0B03 0504
J0B04    STX     D09E8            ; 0B04 61E8
J0B05    SS3                      ; 0B05 08F0
         JMP     J0AD8            ; 0B06 12D8  not run
         LDX     *0               ; 0B07 9800
         STX     D09E9            ; 0B08 61E9
         LDX     *0               ; 0B09 9800
         JMP     *0               ; 0B0A 1800
         LDX     D09E8            ; 0B0B 91E8
         DXS     4                ; 0B0C 0504
         LDX     *0               ; 0B0D 9800
         IXS     0                ; 0B0E 0400
         LDX     *0               ; 0B0F 9800  not run
         STX     D09ED            ; 0B10 61ED
J0B11    LDX     D09E8            ; 0B11 91E8
         DXS     3                ; 0B12 0503
         LDX     *0               ; 0B13 9800
         IXS     0                ; 0B14 0400
         LDX     *0               ; 0B15 9800  not run
         STX     D09EC            ; 0B16 61EC
J0B17    LDX     D09E8            ; 0B17 91E8
         DXS     2                ; 0B18 0502
         LDX     *0               ; 0B19 9800
         IXS     0                ; 0B1A 0400
         LDX     *0               ; 0B1B 9800
         STX     D09EB            ; 0B1C 61EB
J0B1D    LDX     D09E8            ; 0B1D 91E8
         DXS     1                ; 0B1E 0501
         LDX     *0               ; 0B1F 9800
         IXS     0                ; 0B20 0400
         LDX     *0               ; 0B21 9800
         STX     D09EA            ; 0B22 61EA
         LDX     D09E9            ; 0B23 91E9
         JMP     *1               ; 0B24 1801
J0B25    JSX     S0B29            ; 0B25 2329
J0B26    JMP     J0B17            ; 0B26 1317
J0B27    JSX     S0B29            ; 0B27 2329
J0B28    JMP     J0B11            ; 0B28 1311
S0B29    LDW     D09E8            ; 0B29 81E8
         SUB     D0A41            ; 0B2A B241
         STW     D09E8            ; 0B2B 71E8
         JMP     *0               ; 0B2C 1800
J0B2D    LDX     D09EC            ; 0B2D 91EC
         STX     D0B32            ; 0B2E 6332
         LDX     D09EB            ; 0B2F 91EB
         LDW     *0               ; 0B30 8800
         JSX     S0BB0            ; 0B31 23B0
D0B32    DATA    X'8000'          ; 0B32 8000  argument 1 of 0B31; LDW 0; written by 0B2E
         JMP     J0854            ; 0B33 1054
         DATA    X'91EB'          ; 0B34 91EB  LDX X'1EB'
         DATA    X'8800'          ; 0B35 8800  LDW *0
         DATA    X'0081'          ; 0B36 0081
         DATA    X'752C'          ; 0B37 752C  STW X'52C'
         DATA    X'090F'          ; 0B38 090F
         DATA    X'0081'          ; 0B39 0081
         DATA    X'752B'          ; 0B3A 752B  STW X'52B'
         DATA    X'91EC'          ; 0B3B 91EC  LDX X'1EC'
         DATA    X'8800'          ; 0B3C 8800  LDW *0
         DATA    X'0081'          ; 0B3D 0081
         DATA    X'2584'          ; 0B3E 2584  JSX X'584'
         DATA    X'1054'          ; 0B3F 1054  JMP X'54'
J0B40    LDW     D0A41            ; 0B40 8241  not run
         JMP     J0B43            ; 0B41 1343  not run
J0B42    CLR                      ; 0B42 0100
J0B43    SML     1                ; 0B43 0081
         STW     D070F            ; 0B44 770F
         STW     D0B4F            ; 0B45 734F
         SML     1                ; 0B46 0081
         LDW     D0771            ; 0B47 8771
         LDX     D09E8            ; 0B48 91E8
         DXS     2                ; 0B49 0502
         SAM                      ; 0B4A 0820
J0B4B    DXS     1                ; 0B4B 0501
         JMP     J0B04            ; 0B4C 1304
J0B4D    LDX     D09E8            ; 0B4D 91E8
         JMP     J0B4B            ; 0B4E 134B
D0B4F    DATA    X'0000'          ; 0B4F 0000  written by 0B45; read by 0C02 0C27 0C4F
D0B50    DATA    X'0000'          ; 0B50 0000  written by 0B51; read by 0B69
S0B51    STX     D0B50            ; 0B51 6350
         CMW     D0A3D            ; 0B52 F23D
         SAP                      ; 0B53 0810
         CMP                      ; 0B54 0110
         STW     D0AD2            ; 0B55 72D2
         LLB     X'AD'            ; 0B56 06AD
         SLS                      ; 0B57 0840
         LLB     X'A0'            ; 0B58 06A0
         STB     D0AC1            ; 0B59 3582
         SNE                      ; 0B5A 0870
         LLB     X'B0'            ; 0B5B 06B0  not run
         SEQ                      ; 0B5C 0860
         JSX     S0B73            ; 0B5D 2373
         STB     D0B8D+1          ; 0B5E 371B
         JSX     S0B73            ; 0B5F 2373
J0B60    STB     D0B8D            ; 0B60 371A
         JSX     S0B73            ; 0B61 2373
J0B62    STB     D0B8C+1          ; 0B62 3719
         JSX     S0B73            ; 0B63 2373
J0B64    STB     D0B8C            ; 0B64 3718
         JSX     S0B73            ; 0B65 2373
J0B66    STB     D0B8B+1          ; 0B66 3717
         JSX     S0B73            ; 0B67 2373
J0B68    STB     D0B8B            ; 0B68 3716
         LDX     D0B50            ; 0B69 9350
         JSX     *0               ; 0B6A 2800
S0B6B    CLB     X'AF'            ; 0B6B 07AF
         SGR                      ; 0B6C 0880
         JMP     *0               ; 0B6D 1800
         CLB     X'B9'            ; 0B6E 07B9
         SLE                      ; 0B6F 0890
         JMP     *0               ; 0B70 1800
         JMP     *1               ; 0B71 1801
D0B72    DATA    X'0000'          ; 0B72 0000  written by 0B73; read by 0B88
S0B73    STX     D0B72            ; 0B73 6372
         LDW     D0AD2            ; 0B74 82D2
         SAZ                      ; 0B75 0800
         JMP     J0B7C            ; 0B76 137C
         LDW     D0AC1            ; 0B77 82C1
         LLB     X'A0'            ; 0B78 06A0
         STB     D0AC1            ; 0B79 3582
         SRL     8                ; 0B7A 0A08
         JMP     *0               ; 0B7B 1800
J0B7C    SML     1                ; 0B7C 0081
         STW     D052C            ; 0B7D 752C
         CLR                      ; 0B7E 0100
         SML     1                ; 0B7F 0081
         STW     D052B            ; 0B80 752B
         LDW     D0ABF            ; 0B81 82BF
         SML     1                ; 0B82 0081
         JSX     S0584            ; 0B83 2584
J0B84    STW     D0AD2            ; 0B84 72D2
         SML     1                ; 0B85 0081
         LDW     D052B            ; 0B86 852B
         ORI     D09A0            ; 0B87 C1A0
         LDX     D0B72            ; 0B88 9372
         JSX     *0               ; 0B89 2800
D0B8A    DATA    X'A0A0'          ; 0B8A A0A0  '  '; written by 092F 0941 0968 09DD
D0B8B    DATA    X'A0A0'          ; 0B8B A0A0  '  '; written by 09DF 0B66 0B68
D0B8C    DATA    X'A0A0'          ; 0B8C A0A0  '  '; written by 09D7 0B62 0B64
D0B8D    DATA    X'A0A0'          ; 0B8D A0A0  '  '; written by 09D9 0B5E 0B60
D0B8E    DATA    X'0000'          ; 0B8E 0000  written by 09DB
D0B8F    DATA    X'0000'          ; 0B8F 0000  written by 0B90 0BA4 0BB0; read by 0B98 0BAE
S0B90    STX     D0B8F            ; 0B90 638F
         LDW     *0               ; 0B91 8800
         STW     D0B9A            ; 0B92 739A
         JSX     S0BA2            ; 0B93 23A2
         DATA    X'8B9A'          ; 0B94 8B9A  argument 1 of 0B93; LDW *X'39A'
J0B95    LDW     D0B9A            ; 0B95 839A
         SAP                      ; 0B96 0810
         JMP     J0B95            ; 0B97 1395  not run
J0B98    LDX     D0B8F            ; 0B98 938F
         JSX     *1               ; 0B99 2801
D0B9A    DATA    X'0000'          ; 0B9A 0000  written by 0B92; read by 0B95
         DATA    X'0521'          ; 0B9B 0521
         DATA    X'00DE'          ; 0B9C 00DE
         DATA    X'0000'          ; 0B9D 0000
         DATA    X'0000'          ; 0B9E 0000
         DATA    X'0000'          ; 0B9F 0000
         DATA    X'0000'          ; 0BA0 0000
         DATA    X'0000'          ; 0BA1 0000
S0BA2    SML     0                ; 0BA2 0080
         JMP     A0044            ; 0BA3 1044
S0BA4    STX     D0B8F            ; 0BA4 638F
         LDW     *0               ; 0BA5 8800
         STW     D0A1A            ; 0BA6 721A
         LDW     *1               ; 0BA7 8801
         STW     D090C            ; 0BA8 710C
         JSX     S0BA2            ; 0BA9 23A2
         DATA    X'8A1A'          ; 0BAA 8A1A  argument 1 of 0BA9; LDW *X'21A'
J0BAB    LDW     D0A1A            ; 0BAB 821A
         SAP                      ; 0BAC 0810
         JMP     J0BAB            ; 0BAD 13AB  not run
         LDX     D0B8F            ; 0BAE 938F
         JSX     *2               ; 0BAF 2802
S0BB0    STX     D0B8F            ; 0BB0 638F
         SML     1                ; 0BB1 0081
         STW     D052C            ; 0BB2 752C
         LDX     *0               ; 0BB3 9800
         LDW     *0               ; 0BB4 8800
         SML     1                ; 0BB5 0081
         JSX     S053F            ; 0BB6 253F
J0BB7    SML     1                ; 0BB7 0081
         LDW     D052B            ; 0BB8 852B
         AND     D09FC            ; 0BB9 E1FC
         SML     1                ; 0BBA 0081
         ORI     D052C            ; 0BBB C52C
         JMP     J0B98            ; 0BBC 1398
J0BBD    LDW     D0A35            ; 0BBD 8235
         JMP     J0BC2            ; 0BBE 13C2
J0BBF    SLL     8                ; 0BBF 0A18  not run
         LLB     X'C6'            ; 0BC0 06C6  not run
         SRC     8                ; 0BC1 0A48  not run
J0BC2    STW     D0AC6            ; 0BC2 72C6
         JSX     S0B90            ; 0BC3 2390
         DATA    X'0AC6'          ; 0BC4 0AC6  argument 1 of 0BC3
         JMP     J0AD8            ; 0BC5 12D8
J0BC6    LDW     D09EA            ; 0BC6 81EA
         STW     D0C4E            ; 0BC7 744E
         CLR                      ; 0BC8 0100
         STW     D0BF6            ; 0BC9 73F6
         STW     D0C5F            ; 0BCA 745F
         STW     D0C60            ; 0BCB 7460
         STW     D0C61            ; 0BCC 7461
         STW     D0C62            ; 0BCD 7462
         STW     D0C63            ; 0BCE 7463
         STW     D0C64            ; 0BCF 7464
         STW     D0C06            ; 0BD0 7406
J0BD1    STB     D0BF6            ; 0BD1 37EC
J0BD2    JSX     S0C26            ; 0BD2 2426
J0BD3    CLB     X'AD'            ; 0BD3 07AD
         SNE                      ; 0BD4 0870
         JMP     J0BD1            ; 0BD5 13D1
         CLB     X'A0'            ; 0BD6 07A0
         SEQ                      ; 0BD7 0860
         CLB     X'AB'            ; 0BD8 07AB
         SNE                      ; 0BD9 0870
         JMP     J0BD2            ; 0BDA 13D2
J0BDB    JSX     S0B6B            ; 0BDB 236B
J0BDC    JMP     J0BF7            ; 0BDC 13F7
J0BDD    AND     D0A42            ; 0BDD E242
         STW     D09EB            ; 0BDE 71EB
         LDW     D0C62            ; 0BDF 8462
         SUB     D0C63            ; 0BE0 B463
         STW     D0C62            ; 0BE1 7462
         SML     1                ; 0BE2 0081
         JSX     S04C1            ; 0BE3 24C1
         DATA    X'8C5F'          ; 0BE4 8C5F  argument 1 of 0BE3; LDW *X'45F'
J0BE5    SML     0                ; 0BE5 0080
         JSX     S01A3            ; 0BE6 21A3
         DATA    X'8653'          ; 0BE7 8653  argument 1 of 0BE6; LDW X'653'
J0BE8    SML     1                ; 0BE8 0081
         JSX     S04CA            ; 0BE9 24CA
         DATA    X'8C5F'          ; 0BEA 8C5F  argument 1 of 0BE9; LDW *X'45F'
J0BEB    SML     1                ; 0BEB 0081
         JSX     S05CC            ; 0BEC 25CC
         DATA    X'89EB'          ; 0BED 89EB  argument 1 of 0BEC; LDW *X'1EB'
J0BEE    SML     0                ; 0BEE 0080
         JSX     S0110            ; 0BEF 2110
         DATA    X'8C5F'          ; 0BF0 8C5F  argument 1 of 0BEF; LDW *X'45F'
J0BF1    SML     1                ; 0BF1 0081
         JSX     S04CA            ; 0BF2 24CA
         DATA    X'8C5F'          ; 0BF3 8C5F  argument 1 of 0BF2; LDW *X'45F'
J0BF4    JSX     S0C26            ; 0BF4 2426
J0BF5    JMP     J0BDB            ; 0BF5 13DB
D0BF6    DATA    X'0000'          ; 0BF6 0000  written by 0BC9 0BD1; read by 0C48
J0BF7    CLB     X'AE'            ; 0BF7 07AE
         LDX     D0A41            ; 0BF8 9241
         SNE                      ; 0BF9 0870
         STX     D0C63            ; 0BFA 6463
         SNE                      ; 0BFB 0870
         JMP     J0BF4            ; 0BFC 13F4
         CLB     X'A0'            ; 0BFD 07A0
         SEQ                      ; 0BFE 0860
         CLB     X'20'            ; 0BFF 0720
         SEQ                      ; 0C00 0860
         JMP     J0C07            ; 0C01 1407  page from the run
J0C02    LDW     D0B4F            ; 0C02 834F  not run
         SAZ                      ; 0C03 0800  not run
         JMP     J0C30            ; 0C04 1430  not run
         JMP     J0C24            ; 0C05 1424  not run
D0C06    DATA    X'0000'          ; 0C06 0000  written by 0BD0 0C13; read by 0C31
J0C07    CLB     X'C5'            ; 0C07 07C5
         SEQ                      ; 0C08 0860
         JMP     J0C24            ; 0C09 1424  not run
J0C0A    JSX     S0C26            ; 0C0A 2426
J0C0B    CLB     X'A0'            ; 0C0B 07A0
         SEQ                      ; 0C0C 0860
         CLB     X'AB'            ; 0C0D 07AB
         SNE                      ; 0C0E 0870
         JMP     J0C0A            ; 0C0F 140A
         CLB     X'AD'            ; 0C10 07AD
         SEQ                      ; 0C11 0860
         JMP     J0C15            ; 0C12 1415
         STB     D0C06            ; 0C13 300C  not run
         JMP     J0C0A            ; 0C14 140A  not run
J0C15    JSX     S0B6B            ; 0C15 236B
         JMP     J0C24            ; 0C16 1424  not run
J0C17    AND     D0A42            ; 0C17 E242
J0C18    STW     D0C64            ; 0C18 7464
         JSX     S0C26            ; 0C19 2426
J0C1A    JSX     S0B6B            ; 0C1A 236B
         JMP     J0C02            ; 0C1B 1402  not run
J0C1C    AND     D0A42            ; 0C1C E242
         STW     D09EB            ; 0C1D 71EB
         LDW     D0C64            ; 0C1E 8464
         SLL     3                ; 0C1F 0A13
         ADD     D0C64            ; 0C20 A464
         ADD     D0C64            ; 0C21 A464
         ADD     D09EB            ; 0C22 A1EB
         JMP     J0C18            ; 0C23 1418
J0C24    LLB     X'CD'            ; 0C24 06CD  not run
         JMP     J0BBF            ; 0C25 13BF  not run
S0C26    STX     D09EC            ; 0C26 61EC
         LDW     D0B4F            ; 0C27 834F
         SAZ                      ; 0C28 0800
         JMP     J0C2E            ; 0C29 142E  not run
         SML     1                ; 0C2A 0081
         JSX     S0756            ; 0C2B 2756
J0C2C    JMP     J0C30            ; 0C2C 1430
J0C2D    LDX     D09EC            ; 0C2D 91EC
J0C2E    SML     1                ; 0C2E 0081
         JMP     S0752            ; 0C2F 1752
J0C30    LDW     D0C64            ; 0C30 8464
         LDX     D0C06            ; 0C31 9406
         IXS     0                ; 0C32 0400
         CMP                      ; 0C33 0110  not run
J0C34    ADD     D0C62            ; 0C34 A462
         STW     D0C62            ; 0C35 7462
         SAM                      ; 0C36 0820
         JMP     J0C43            ; 0C37 1443
         SML     1                ; 0C38 0081
         JSX     S04C1            ; 0C39 24C1
         DATA    X'8C5F'          ; 0C3A 8C5F  argument 1 of 0C39; LDW *X'45F'
J0C3B    SML     0                ; 0C3B 0080
         JSX     S01A3            ; 0C3C 21A3
         DATA    X'8656'          ; 0C3D 8656  argument 1 of 0C3C; LDW X'656'
J0C3E    SML     1                ; 0C3E 0081
         JSX     S04CA            ; 0C3F 24CA
         DATA    X'8C5F'          ; 0C40 8C5F  argument 1 of 0C3F; LDW *X'45F'
J0C41    LDW     D0A41            ; 0C41 8241
         JMP     J0C34            ; 0C42 1434
J0C43    SAZ                      ; 0C43 0800
         JMP     J0C54            ; 0C44 1454  not run
         SML     1                ; 0C45 0081
         JSX     S04C1            ; 0C46 24C1
         DATA    X'8C5F'          ; 0C47 8C5F  argument 1 of 0C46; LDW *X'45F'
J0C48    LDW     D0BF6            ; 0C48 83F6
         SML     1                ; 0C49 0081
         SAP                      ; 0C4A 0810
         JSX     X'0D0B'          ; 0C4B 250B  lands at 050B = S050B under the selection in force
J0C4C    SML     1                ; 0C4C 0081
         JSX     S04CA            ; 0C4D 24CA
D0C4E    DATA    X'0000'          ; 0C4E 0000  argument 1 of 0C4D; written by 0BC7
J0C4F    LDW     D0B4F            ; 0C4F 834F
         SAZ                      ; 0C50 0800
         JMP     J0B40            ; 0C51 1340  not run
         SML     1                ; 0C52 0081
         JMP     J072E            ; 0C53 172E
J0C54    SML     1                ; 0C54 0081  not run
         JSX     S04C1            ; 0C55 24C1  not run
         DATA    X'8C5F'          ; 0C56 8C5F  argument 1 of 0C55; LDW *X'45F'
         SML     0                ; 0C57 0080  not run
         JSX     S01A3            ; 0C58 21A3  not run
         DATA    X'8653'          ; 0C59 8653  argument 1 of 0C58; LDW X'653'
         SML     1                ; 0C5A 0081  not run
         JSX     S04CA            ; 0C5B 24CA  not run
         DATA    X'8C5F'          ; 0C5C 8C5F  argument 1 of 0C5B; LDW *X'45F'
         LDW     D0A43            ; 0C5D 8243  not run
         JMP     J0C34            ; 0C5E 1434  not run
D0C5F    DATA    X'0000'          ; 0C5F 0000  written by 0BCA
D0C60    DATA    X'0000'          ; 0C60 0000  written by 0BCB
D0C61    DATA    X'0000'          ; 0C61 0000  written by 0BCC
D0C62    DATA    X'0000'          ; 0C62 0000  written by 0BCD 0BE1 0C35; read by 0BDF 0C34
D0C63    DATA    X'0000'          ; 0C63 0000  written by 0BCE 0BFA; read by 0BE0
D0C64    DATA    X'0000'          ; 0C64 0000  written by 0BCF 0C18; read by 0C1E 0C20 0C21 0C30
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
S0CA7    STX     D0A72            ; 0CA7 6272
         SLM                      ; 0CA8 0040
         CLR                      ; 0CA9 0100
         SML     1                ; 0CAA 0081
         LDB     D064B            ; 0CAB 5496
         CLB     X'B4'            ; 0CAC 07B4
         LDX     D0CD0            ; 0CAD 94D0
J0CAE    SML     1                ; 0CAE 0081
         LDB     *X'48E'          ; 0CAF 5C8E
         SLE                      ; 0CB0 0890
         ADD     D0A41            ; 0CB1 A241
         CLB     X'B9'            ; 0CB2 07B9
         SLE                      ; 0CB3 0890
         CLR                      ; 0CB4 0100  not run
         ORI     D0CD2            ; 0CB5 C4D2
         SML     1                ; 0CB6 0081
         STB     *X'48E'          ; 0CB7 3C8E
         DXS     1                ; 0CB8 0501
         JMP     J0CAE            ; 0CB9 14AE
         SGR                      ; 0CBA 0880
         JMP     J0CCB            ; 0CBB 14CB
         LDX     D0CD1            ; 0CBC 94D1  not run
J0CBD    SML     1                ; 0CBD 0081  not run
         LDB     *X'48D'          ; 0CBE 5C8D  not run
         SML     1                ; 0CBF 0081  not run
         STB     *X'48E'          ; 0CC0 3C8E  not run
         DXS     1                ; 0CC1 0501  not run
         JMP     J0CBD            ; 0CC2 14BD  not run
         LLB     X'B1'            ; 0CC3 06B1  not run
         SML     1                ; 0CC4 0081  not run
         STB     D0647            ; 0CC5 348E  not run
         SML     1                ; 0CC6 0081  not run
         LDW     D064F            ; 0CC7 864F  not run
         ADD     D0A41            ; 0CC8 A241  not run
         SML     1                ; 0CC9 0081  not run
         STW     D064F            ; 0CCA 764F  not run
J0CCB    LLB     X'A0'            ; 0CCB 06A0
         SML     1                ; 0CCC 0081
         STB     D064B            ; 0CCD 3496
         LDX     D0A72            ; 0CCE 9272
         JSX     *0               ; 0CCF 2800
D0CD0    DATA    X'0007'          ; 0CD0 0007  read by 0CAD
D0CD1    DATA    X'0008'          ; 0CD1 0008  read by 0CBC
D0CD2    DATA    X'00B0'          ; 0CD2 00B0  read by 0CB5
         DATA    X'0690'          ; 0CD3 0690
         DATA    X'14D6'          ; 0CD4 14D6  JMP X'4D6'
         DATA    X'06E0'          ; 0CD5 06E0
         DATA    X'91EA'          ; 0CD6 91EA  LDX X'1EA'
         DATA    X'0A18'          ; 0CD7 0A18
         DATA    X'C800'          ; 0CD8 C800  ORI *0
         DATA    X'0A54'          ; 0CD9 0A54
         DATA    X'74E6'          ; 0CDA 74E6  STW X'4E6'
         DATA    X'91EB'          ; 0CDB 91EB  LDX X'1EB'
         DATA    X'8803'          ; 0CDC 8803  LDW *3
         DATA    X'74E4'          ; 0CDD 74E4  STW X'4E4'
         DATA    X'23A2'          ; 0CDE 23A2  JSX X'3A2'
         DATA    X'8CE4'          ; 0CDF 8CE4  LDW *X'4E4'
         DATA    X'84E4'          ; 0CE0 84E4  LDW X'4E4'
         DATA    X'0810'          ; 0CE1 0810
         DATA    X'14E0'          ; 0CE2 14E0  JMP X'4E0'
         DATA    X'1302'          ; 0CE3 1302  JMP X'302'
         DATA    X'0000'          ; 0CE4 0000
         DATA    X'89EC'          ; 0CE5 89EC  LDW *X'1EC'
         DATA    X'0000'          ; 0CE6 0000
         DATA    X'0000'          ; 0CE7 0000
         DATA    X'0000'          ; 0CE8 0000
         DATA    X'0000'          ; 0CE9 0000
         DATA    X'0000'          ; 0CEA 0000
         DATA    X'0000'          ; 0CEB 0000
D0CEC    DATA    X'0000'          ; 0CEC 0000  written by 0CED; read by 0CF3
S0CED    STX     D0CEC            ; 0CED 64EC
         CAX                      ; 0CEE 0130
         LDW     *0               ; 0CEF 8800
         AND     D0CF5            ; 0CF0 E4F5
         STW     *0               ; 0CF1 7800
         CXA                      ; 0CF2 0140
         LDX     D0CEC            ; 0CF3 94EC
         JSX     *0               ; 0CF4 2800
D0CF5    DATA    X'00FF'          ; 0CF5 00FF  read by 087D 0CF0
         DATA    X'00BB'          ; 0CF6 00BB
         END
