; PTIOS, the Paper Tape I/O System, DN 393954 (1981)
;
; Written by tools/dis703.py. A line marked "not run" is an
; instruction only by being reached from one that ran.

R0000    EQU     X'0000'  ; the tape's 00E2, where it runs
R0007    EQU     X'0007'  ; the tape's 00E9, where it runs
R000B    EQU     X'000B'  ; the tape's 00ED, where it runs
R000F    EQU     X'000F'  ; the tape's 00F1, where it runs
R0023    EQU     X'0023'  ; the tape's 0105, where it runs
R0025    EQU     X'0025'  ; the tape's 0107, where it runs
R002B    EQU     X'002B'  ; the tape's 010D, where it runs
R0030    EQU     X'0030'  ; the tape's 0112, where it runs
R0033    EQU     X'0033'  ; the tape's 0115, where it runs
R0034    EQU     X'0034'  ; the tape's 0116, where it runs
R0038    EQU     X'0038'  ; the tape's 011A, where it runs
R0040    EQU     X'0040'  ; the tape's 0122, where it runs
R0042    EQU     X'0042'  ; the tape's 0124, where it runs
R0044    EQU     X'0044'  ; the tape's 0126, where it runs
R0047    EQU     X'0047'  ; the tape's 0129, where it runs
R004B    EQU     X'004B'  ; the tape's 012D, where it runs
R004E    EQU     X'004E'  ; the tape's 0130, where it runs
R004F    EQU     X'004F'  ; the tape's 0131, where it runs
R0051    EQU     X'0051'  ; the tape's 0133, where it runs
R0057    EQU     X'0057'  ; the tape's 0139, where it runs
R005B    EQU     X'005B'  ; the tape's 013D, where it runs
R005C    EQU     X'005C'  ; the tape's 013E, where it runs
R006B    EQU     X'006B'  ; the tape's 014D, where it runs
R006F    EQU     X'006F'  ; the tape's 0151, where it runs
R0072    EQU     X'0072'  ; the tape's 0154, where it runs
R0076    EQU     X'0076'  ; the tape's 0158, where it runs
R0078    EQU     X'0078'  ; the tape's 015A, where it runs
R007B    EQU     X'007B'  ; the tape's 015D, where it runs

         ORG     X'0080'
; The Paper Tape I/O System, as the museum's site ran it in place of X-RAY.
; It copies its words from X'E2' on to word 0 and runs them there.
         CLR                      ; 0080 0100
         LDX     D0162            ; 0081 9162
         SRL     0                ; 0082 0A00
         SRL     0                ; 0083 0A00
         SRL     0                ; 0084 0A00
         SRL     0                ; 0085 0A00
         SRL     0                ; 0086 0A00
         SRL     0                ; 0087 0A00
         SRL     0                ; 0088 0A00
         DXS     1                ; 0089 0501
         STX     D013C            ; 008A 613C
         LDX     D0163            ; 008B 9163
         SLM                      ; 008C 0040
J008D    LDW     *X'E2'           ; 008D 88E2
         STW     *0               ; 008E 7800
         DXS     1                ; 008F 0501
         JMP     J008D            ; 0090 108D
         JSX     R0044            ; 0091 2044
         DATA    X'80C0'          ; 0092 80C0  argument 1 of 0091; LDW X'C0'
J0093    JSX     R0044            ; 0093 2044
         DATA    X'80D0'          ; 0094 80D0  argument 1 of 0093; LDW X'D0'
         LDB     D00E0            ; 0095 51C0
         CLB     X'C1'            ; 0096 07C1
         SEQ                      ; 0097 0860
         JMP     J009E            ; 0098 109E
         LDB     D00E0+1          ; 0099 51C1
         CLB     X'CC'            ; 009A 07CC
         LDB     D00E0            ; 009B 51C0
         SNE                      ; 009C 0870
         JMP     R0078            ; 009D 1078
J009E    CLB     X'B9'            ; 009E 07B9
         SLE                      ; 009F 0890
         ADD     D0164            ; 00A0 A164  not run
         AND     D0165            ; 00A1 E165
         CLB     X'B'             ; 00A2 070B
         SLE                      ; 00A3 0890
         JMP     J00BD            ; 00A4 10BD  not run
         CAX                      ; 00A5 0130
         LDB     D00E0+1          ; 00A6 51C1
         CLB     X'BA'            ; 00A7 07BA
         SEQ                      ; 00A8 0860
         JMP     J00BD            ; 00A9 10BD  not run
         LDB     D00E1            ; 00AA 51C2
         CLB     X'D4'            ; 00AB 07D4
         SNE                      ; 00AC 0870
         JMP     J00B9            ; 00AD 10B9  not run
         CLB     X'D2'            ; 00AE 07D2
         SNE                      ; 00AF 0870
         JMP     J00BB            ; 00B0 10BB
         CLB     X'D0'            ; 00B1 07D0
         SEQ                      ; 00B2 0860
         JMP     J00BD            ; 00B3 10BD  not run
         LLB     X'C0'            ; 00B4 06C0
J00B5    STB     *0               ; 00B5 3800
         JSX     S0126            ; 00B6 2126
         DATA    X'80CA'          ; 00B7 80CA  argument 1 of 00B6; LDW X'CA'
         JMP     J0093            ; 00B8 1093
J00B9    LLB     X'E0'            ; 00B9 06E0  not run
         JMP     J00B5            ; 00BA 10B5  not run
J00BB    LLB     X'D0'            ; 00BB 06D0
         JMP     J00B5            ; 00BC 10B5
J00BD    JSX     R0044            ; 00BD 2044  not run
         DATA    X'80C5'          ; 00BE 80C5  argument 1 of 00BD; LDW X'C5'
         JMP     J0093            ; 00BF 1093  not run
         DATA    X'00D5'          ; 00C0 00D5
         DATA    X'0166'          ; 00C1 0166
         DATA    X'003E'          ; 00C2 003E
         DATA    X'0000'          ; 00C3 0000
         DATA    X'0000'          ; 00C4 0000
         DATA    X'00DA'          ; 00C5 00DA
         DATA    X'0167'          ; 00C6 0167
         DATA    X'003E'          ; 00C7 003E
         DATA    X'0000'          ; 00C8 0000
         DATA    X'0000'          ; 00C9 0000
         DATA    X'00CF'          ; 00CA 00CF
         DATA    X'0168'          ; 00CB 0168
         DATA    X'003E'          ; 00CC 003E
         DATA    X'0000'          ; 00CD 0000
         DATA    X'0000'          ; 00CE 0000
         DATA    X'CFCB'          ; 00CF CFCB  'OK'
         DATA    X'00E0'          ; 00D0 00E0
         DATA    X'0169'          ; 00D1 0169
         DATA    X'001B'          ; 00D2 001B
         DATA    X'0000'          ; 00D3 0000
         DATA    X'0000'          ; 00D4 0000
         DATA    X'C1D3'          ; 00D5 C1D3  'AS'
         DATA    X'D3C9'          ; 00D6 D3C9  'SI'
         DATA    X'C7CE'          ; 00D7 C7CE  'GN'
         DATA    X'A0C9'          ; 00D8 A0C9  ' I'
         DATA    X'ADCF'          ; 00D9 ADCF  '-O'
         DATA    X'D3C1'          ; 00DA D3C1  'SA'
         DATA    X'D9A0'          ; 00DB D9A0  'Y '
         DATA    X'C9D4'          ; 00DC C9D4  'IT'
         DATA    X'A0C1'          ; 00DD A0C1  ' A'
         DATA    X'C7C1'          ; 00DE C7C1  'GA'
         DATA    X'C9CE'          ; 00DF C9CE  'IN'
D00E0    DATA    X'0000'          ; 00E0 0000  read by 0095 0099 009B 00A6
D00E1    DATA    X'0000'          ; 00E1 0000  read by 00AA
         DATA    X'E0E0'          ; 00E2 E0E0  runs at 0000 = R0000; '``'; read by 0139
         DATA    X'E0E0'          ; 00E3 E0E0  runs at 0001; '``'
         DATA    X'E0E0'          ; 00E4 E0E0  runs at 0002; '``'
         DATA    X'E0E0'          ; 00E5 E0E0  runs at 0003; '``'
         DATA    X'E0E0'          ; 00E6 E0E0  runs at 0004; '``'
         DATA    X'E0E0'          ; 00E7 E0E0  runs at 0005; '``'
         DATA    X'E0E0'          ; 00E8 E0E0  runs at 0006; '``'
         DATA    X'0000'          ; 00E9 0000  runs at 0007 = R0007; written by 0134; read by 0118 013E 014B
         DATA    X'0000'          ; 00EA 0000  runs at 0008
         DATA    X'0000'          ; 00EB 0000  runs at 0009
         DATA    X'0000'          ; 00EC 0000  runs at 000A
         DATA    X'0001'          ; 00ED 0001  runs at 000B = R000B; read by 011C
         DATA    X'0000'          ; 00EE 0000  runs at 000C
         DATA    X'0000'          ; 00EF 0000  runs at 000D
         DATA    X'0000'          ; 00F0 0000  runs at 000E
         ORI     R0025            ; 00F1 C025  runs at 000F = R000F
         SRL     12               ; 00F2 0A0C  runs at 0010
         ORI     R006B            ; 00F3 C06B  runs at 0011
         SAO                      ; 00F4 0830  runs at 0012
         ORI     R005B            ; 00F5 C05B  runs at 0013
         STW     R0072            ; 00F6 7072  runs at 0014
         LDW     *0               ; 00F7 8800  runs at 0015
         SLL     1                ; 00F8 0A11  runs at 0016
         STW     R0034            ; 00F9 7034  runs at 0017
         LDX     *1               ; 00FA 9801  runs at 0018
         LDW     *0               ; 00FB 8800  runs at 0019
         SLL     1                ; 00FC 0A11  runs at 001A
         STW     R004E            ; 00FD 704E  runs at 001B
         LLB     X'10'            ; 00FE 0610  runs at 001C
         IXS     0                ; 00FF 0400  runs at 001D
         LLB     X'80'            ; 0100 0680  runs at 001E
         STB     X'0070'          ; 0101 3070  runs at 001F; byte 0 of R0038
         STB     X'0056'          ; 0102 3056  runs at 0020; byte 0 of R002B
         JSX     R006F            ; 0103 206F  runs at 0021
         DATA    X'00AF'          ; 0104 00AF  runs at 0022; argument 1 of 0103
         JSX     R006B            ; 0105 206B  runs at 0023 = R0023
         DATA    X'0081'          ; 0106 0081  runs at 0024; argument 1 of 0105
         CMB     X'0089'          ; 0107 4089  runs at 0025 = R0025; byte 1 of R0044; read by 00F1
         SLE                      ; 0108 0890  runs at 0026
         JMP     R0023            ; 0109 1023  runs at 0027
         LLB     X'10'            ; 010A 0610  runs at 0028
         SEQ                      ; 010B 0860  runs at 0029
         JMP     R005C            ; 010C 105C  runs at 002A
         JMP     R0030            ; 010D 1030  runs at 002B = R002B; written by 0102
         JSX     R006B            ; 010E 206B  runs at 002C
         DATA    X'009C'          ; 010F 009C  runs at 002D; argument 1 of 010E
         JSX     R006B            ; 0110 206B  runs at 002E
         DATA    X'009D'          ; 0111 009D  runs at 002F; argument 1 of 0110
         LDW     R004E            ; 0112 804E  runs at 0030 = R0030
         ADD     R0034            ; 0113 A034  runs at 0031
         STW     R004E            ; 0114 704E  runs at 0032
         JSX     R006B            ; 0115 206B  runs at 0033 = R0033
         DATA    X'0000'          ; 0116 0000  runs at 0034 = R0034; argument 1 of 0115; written by 00F9 011D; read by 0113 011B 0141
         CLB     X'8D'            ; 0117 078D  runs at 0035
         LDX     R0007            ; 0118 9007  runs at 0036
         SGR                      ; 0119 0880  runs at 0037
         JMP     R0042            ; 011A 1042  runs at 0038 = R0038; written by 0101
         LDW     R0034            ; 011B 8034  runs at 0039
         ADD     R000B            ; 011C A00B  runs at 003A
         STW     R0034            ; 011D 7034  runs at 003B
         SUB     R004E            ; 011E B04E  runs at 003C
         SAZ                      ; 011F 0800  runs at 003D
         JMP     R0033            ; 0120 1033  runs at 003E
         JMP     R005B            ; 0121 105B  runs at 003F
         LDW     *X'500'          ; 0122 8D00  runs at 0040 = R0040; written by 0133
         JMP     R0078            ; 0123 1078  runs at 0041
         SNE                      ; 0124 0870  runs at 0042 = R0042
         JMP     R005B            ; 0125 105B  runs at 0043
S0126    LLB     X'8A'            ; 0126 068A  read by 0107
         JMP     R0051            ; 0127 1051
         JMP     *1               ; 0128 1801  runs at 0046
         JSX     R0044            ; 0129 2044  runs at 0047 = R0047
         DATA    X'004B'          ; 012A 004B  runs at 0048; argument 1 of 0129
         LDX     R004B            ; 012B 904B  runs at 0049
         JMP     *0               ; 012C 1800  runs at 004A
         DATA    X'0000'          ; 012D 0000  runs at 004B = R004B; written by 015B; read by 012B
         DATA    X'8000'          ; 012E 8000  runs at 004C; LDW 0
         DATA    X'0049'          ; 012F 0049  runs at 004D
         DATA    X'0000'          ; 0130 0000  runs at 004E = R004E; written by 00FD 0114; read by 0112 011E 015E
         DATA    X'0000'          ; 0131 0000  runs at 004F = R004F; written by 0151; read by 0156
         DATA    X'0687'          ; 0132 0687  runs at 0050
         STB     X'0081'          ; 0133 3081  runs at 0051 = R0051; byte 1 of R0040
         STX     R0007            ; 0134 6007  runs at 0052
         LDX     *0               ; 0135 9800  runs at 0053
         LDW     *2               ; 0136 8802  runs at 0054
         SRC     4                ; 0137 0A44  runs at 0055
         STB     X'00AF'          ; 0138 30AF  runs at 0056; byte 1 of R0057
         LDB     X'0000'          ; 0139 5000  runs at 0057 = R0057; byte 0 of R0000; written by 0138
         STB     X'00D7'          ; 013A 30D7  runs at 0058; byte 1 of R006B
         JMP     R000F            ; 013B 100F  runs at 0059
D013C    DATA    X'0FFF'          ; 013C 0FFF  runs at 005A; written by 008A
         CLR                      ; 013D 0100  runs at 005B = R005B; read by 00F5 0149
         LDX     R0007            ; 013E 9007  runs at 005C = R005C
         LDX     *0               ; 013F 9800  runs at 005D
         STW     *3               ; 0140 7803  runs at 005E
         LDW     R0034            ; 0141 8034  runs at 005F
         STW     *4               ; 0142 7804  runs at 0060
         LDB     X'00E4'          ; 0143 50E4  runs at 0061; byte 0 of R0072
         CLB     3                ; 0144 0703  runs at 0062
         SNE                      ; 0145 0870  runs at 0063
         JSX     R006B            ; 0146 206B  runs at 0064
         SML     0                ; 0147 0080  runs at 0065; argument 1 of 0146; run when a skip passes over the JSX
         LDW     R006B            ; 0148 806B  runs at 0066
         ORI     R005B            ; 0149 C05B  runs at 0067
         STW     R0072            ; 014A 7072  runs at 0068
         LDX     R0007            ; 014B 9007  runs at 0069
         SNE                      ; 014C 0870  runs at 006A
         DIN     0,0              ; 014D 0200  runs at 006B = R006B; written by 013A; read by 00F3 0148
         SRCL    1                ; 014E 0AC1  runs at 006C
         SAM                      ; 014F 0820  runs at 006D
         JMP     R006B            ; 0150 106B  runs at 006E
         STX     R004F            ; 0151 604F  runs at 006F = R006F
         LDX     *0               ; 0152 9800  runs at 0070
         LDB     *0               ; 0153 5800  runs at 0071
         DATA    X'0000'          ; 0154 0000  runs at 0072 = R0072; run: an instruction stored here; written by 00F6 014A; read by 0143
         STB     *0               ; 0155 3800  runs at 0073
         LDX     R004F            ; 0156 904F  runs at 0074
         JSX     *1               ; 0157 2801  runs at 0075
         JSX     R0044            ; 0158 2044  runs at 0076 = R0076
         DATA    X'004B'          ; 0159 004B  runs at 0077; argument 1 of 0158
         LDW     R007B            ; 015A 807B  runs at 0078 = R0078
         STW     R004B            ; 015B 704B  runs at 0079
         JSX     R0044            ; 015C 2044  runs at 007A
         DATA    X'004B'          ; 015D 004B  runs at 007B = R007B; argument 1 of 015C; read by 015A
         LDW     R004E            ; 015E 804E  runs at 007C
         SAZ                      ; 015F 0800  runs at 007D
         JMP     R0047            ; 0160 1047  runs at 007E
         JMP     R0076            ; 0161 1076  runs at 007F
D0162    DATA    X'1F00'          ; 0162 1F00  JMP *X'700'; read by 0081
D0163    DATA    X'007F'          ; 0163 007F  read by 008B
D0164    DATA    X'0009'          ; 0164 0009  read by 00A0
D0165    DATA    X'000F'          ; 0165 000F  read by 00A1
         DATA    X'0005'          ; 0166 0005
         DATA    X'0006'          ; 0167 0006
         DATA    X'0001'          ; 0168 0001
         DATA    X'0002'          ; 0169 0002
         DATA    X'00FC'          ; 016A 00FC
         END
