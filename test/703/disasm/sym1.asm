; SYM I/PREP, the one-pass assembler, DN 390470 (7/10/70)
;
; Written by tools/dis703.py. A line marked "not run" is an
; instruction only by being reached from one that ran.

         ORG     X'05B0'
S05B0    JSX     S07B7            ; 05B0 27B7
         DATA    X'8001'          ; 05B1 8001  argument 1 of 05B0; LDW 1
         LDW     D06CE            ; 05B2 86CE
         STB     D05D1+1          ; 05B3 33A3
         LLB     X'16'            ; 05B4 0616
         STW     D05D2            ; 05B5 75D2
         SML     2                ; 05B6 0082
         LDW     D0824            ; 05B7 8024
         SAZ                      ; 05B8 0800
         JMP     J05C0            ; 05B9 15C0
         LLB     3                ; 05BA 0603
         SML     3                ; 05BB 0083
         ADD     D0DAC            ; 05BC A5AC
         STW     D05D2            ; 05BD 75D2
         LLB     2                ; 05BE 0602
         STB     D05D1+1          ; 05BF 33A3
J05C0    SML     3                ; 05C0 0083
         LDW     D0C6D            ; 05C1 846D
         STW     D0604            ; 05C2 7604
         SML     3                ; 05C3 0083
         LDW     D0DAA            ; 05C4 85AA
J05C5    SML     3                ; 05C5 0083
         STW     D0C6D            ; 05C6 746D
         CMW     D0604            ; 05C7 F604
         SEQ                      ; 05C8 0860
         JMP     J05D3            ; 05C9 15D3
J05CA    LDW     D0604            ; 05CA 8604
         SML     3                ; 05CB 0083
         STW     D0C6D            ; 05CC 746D
         JSX     S07B7            ; 05CD 27B7
         DATA    X'8002'          ; 05CE 8002  argument 1 of 05CD; LDW 2
         SML     3                ; 05CF 0083
         JMP     J0D85            ; 05D0 1585
D05D1    DATA    X'0000'          ; 05D1 0000  written by 05B3 05BF; read by 05FE
D05D2    DATA    X'0000'          ; 05D2 0000  written by 05B5 05BD 05FF; read by 05D3 05FD
J05D3    LDX     D05D2            ; 05D3 95D2
         LDW     *0               ; 05D4 8800
         SML     2                ; 05D5 0082
         STW     D081F            ; 05D6 701F
         SS0                      ; 05D7 08C0
         JMP     J05CA            ; 05D8 15CA
         SML     3                ; 05D9 0083
         JSX     S0ECA            ; 05DA 26CA
         SML     3                ; 05DB 0083
         JSX     S0EB4            ; 05DC 26B4
         JSX     S06D9            ; 05DD 26D9
         DATA    X'0EC0'          ; 05DE 0EC0  argument 1 of 05DD
         DATA    X'800E'          ; 05DF 800E  argument 2 of 05DD; LDW X'E'
J05E0    SML     2                ; 05E0 0082
         LDW     D0826            ; 05E1 8026
         SML     2                ; 05E2 0082
         STW     D0BE0            ; 05E3 73E0
J05E4    SML     2                ; 05E4 0082
         JSX     S0BD5            ; 05E5 23D5
         JMP     J05FA            ; 05E6 15FA
         CAX                      ; 05E7 0130
         LDW     *3               ; 05E8 8803
         SML     3                ; 05E9 0083
         CMW     D0C6D            ; 05EA F46D
         SEQ                      ; 05EB 0860
         JMP     J05E4            ; 05EC 15E4
         LDW     *2               ; 05ED 8802
         SML     2                ; 05EE 0082
         AND     D0814            ; 05EF E014
         SAZ                      ; 05F0 0800
         JMP     J05F3            ; 05F1 15F3
         JMP     J05E4            ; 05F2 15E4
J05F3    LDW     *0               ; 05F3 8800
         STW     D0606            ; 05F4 7606
         LDW     *1               ; 05F5 8801
         STW     D0607            ; 05F6 7607
         JSX     S06D9            ; 05F7 26D9
         DATA    X'0605'          ; 05F8 0605  argument 1 of 05F7
         DATA    X'8006'          ; 05F9 8006  argument 2 of 05F7; LDW 6
J05FA    JSX     S06D9            ; 05FA 26D9
         DATA    X'0EC7'          ; 05FB 0EC7  argument 1 of 05FA
         DATA    X'8001'          ; 05FC 8001  argument 2 of 05FA; LDW 1
J05FD    LDW     D05D2            ; 05FD 85D2
         ADD     D05D1            ; 05FE A5D1
         STW     D05D2            ; 05FF 75D2
         SML     3                ; 0600 0083
         LDW     D0C6D            ; 0601 846D
         ADD     D06CE            ; 0602 A6CE
         JMP     J05C5            ; 0603 15C5
D0604    DATA    X'0000'          ; 0604 0000  written by 05C2; read by 05C7 05CA
         DATA    X'A0A0'          ; 0605 A0A0  '  '
D0606    DATA    X'0000'          ; 0606 0000  written by 05F4
D0607    DATA    X'0000'          ; 0607 0000  written by 05F6
J0608    LDW     D061F            ; 0608 861F
         SUB     D0620            ; 0609 B620
         SRL     1                ; 060A 0A01
         STW     D0620            ; 060B 7620
         ADD     D0619            ; 060C A619
         SML     2                ; 060D 0082
         SUB     D0803            ; 060E B003
         SAM                      ; 060F 0820
         JMP     J0625            ; 0610 1625  not run
J0611    LDW     D0620            ; 0611 8620
         SML     3                ; 0612 0083
         STW     D0DEB            ; 0613 75EB
         ADD     D06CE            ; 0614 A6CE
         STW     D0617            ; 0615 7617
         JSX     S06AB            ; 0616 26AB
D0617    DATA    X'0000'          ; 0617 0000  argument 1 of 0616; written by 0615; read by 061B
         DATA    X'0DEB'          ; 0618 0DEB  argument 2 of 0616
D0619    DATA    X'8000'          ; 0619 8000  argument 3 of 0616; LDW 0; written by 061C 0663 087C; read by 060C 061A 0659
         LDW     D0619            ; 061A 8619
         ADD     D0617            ; 061B A617
         STW     D0619            ; 061C 7619
         SML     2                ; 061D 0082
         JMP     J0860            ; 061E 1060
D061F    DATA    X'003D'          ; 061F 003D  read by 0608
D0620    DATA    X'0000'          ; 0620 0000  written by 060B 0DE5; read by 0609 0611
D0621    DATA    X'001E'          ; 0621 001E  read by 0671
D0622    DATA    X'A0A0'          ; 0622 A0A0  '  '; read by 062E 06B0
D0623    DATA    X'0000'          ; 0623 0000  written by 087E; read by 0631 0662
J0624    STX     D06A6            ; 0624 66A6
J0625    JSX     S07B7            ; 0625 27B7
         DATA    X'8002'          ; 0626 8002  argument 1 of 0625; LDW 2
         LDW     D06A7            ; 0627 86A7
         SAZ                      ; 0628 0800
         JMP     J0631            ; 0629 1631  not run
         STX     D06A7            ; 062A 66A7
         STW     D06A4            ; 062B 76A4
         SML     3                ; 062C 0083
         JSX     S0DAF            ; 062D 25AF
         LDW     D0622            ; 062E 8622
         STW     D0684            ; 062F 7684
         STB     D0683+1          ; 0630 3507
J0631    LDW     D0623            ; 0631 8623
         STW     D0641            ; 0632 7641
         CAX                      ; 0633 0130
J0634    LDW     *0               ; 0634 8800
         STW     D0640            ; 0635 7640
         SLA     1                ; 0636 0911
         ADD     D06A8            ; 0637 A6A8
         STW     D0645            ; 0638 7645
         JSX     S074B            ; 0639 274B
         DATA    X'07E6'          ; 063A 07E6  argument 1 of 0639
         DATA    X'8001'          ; 063B 8001  argument 2 of 0639; LDW 1
J063C    LDW     D0641            ; 063C 8641
         ADD     D06CE            ; 063D A6CE
         STW     D0641            ; 063E 7641
         JSX     S06AB            ; 063F 26AB
D0640    DATA    X'0000'          ; 0640 0000  argument 1 of 063F; written by 0635; read by 0655
D0641    DATA    X'0000'          ; 0641 0000  argument 2 of 063F; written by 0632 063E 0657; read by 063C 0656
         DATA    X'8686'          ; 0642 8686  argument 3 of 063F; LDW X'686'
J0643    JSX     S074B            ; 0643 274B
         DATA    X'0682'          ; 0644 0682  argument 1 of 0643
D0645    DATA    X'8000'          ; 0645 8000  argument 2 of 0643; LDW 0; written by 0638 0672
J0646    LDW     D06A4            ; 0646 86A4
         ADD     D06CE            ; 0647 A6CE
         STW     D06A4            ; 0648 76A4
         JSX     S06AD            ; 0649 26AD
D064A    DATA    X'0000'          ; 064A 0000  argument 1 of 0649; read by 0653
J064B    JSX     S06AD            ; 064B 26AD
D064C    DATA    X'0000'          ; 064C 0000  argument 1 of 064B; read by 0651
J064D    JSX     S06AD            ; 064D 26AD
D064E    DATA    X'0000'          ; 064E 0000  argument 1 of 064D; read by 064F
J064F    LDB     D064E+1          ; 064F 549D
         STB     D0683+1          ; 0650 3507
         LDB     D064C+1          ; 0651 5499
         STB     D0684            ; 0652 3508
         LDB     D064A+1          ; 0653 5495
         STB     D0684+1          ; 0654 3509
         LDW     D0640            ; 0655 8640
         ADD     D0641            ; 0656 A641
         STW     D0641            ; 0657 7641
         CAX                      ; 0658 0130
         SUB     D0619            ; 0659 B619
         SAP                      ; 065A 0810
         JMP     J0634            ; 065B 1634
         LDW     D06A6            ; 065C 86A6
         SAZ                      ; 065D 0800
         JMP     J0665            ; 065E 1665
         JSX     S07B7            ; 065F 27B7  not run
         DATA    X'8003'          ; 0660 8003  argument 1 of 065F; LDW 3
         DOT     12,3             ; 0661 03C3  not run
         LDW     D0623            ; 0662 8623  not run
         STW     D0619            ; 0663 7619  not run
         JMP     J0611            ; 0664 1611  not run
J0665    JSX     S074B            ; 0665 274B
         DATA    X'07E6'          ; 0666 07E6  argument 1 of 0665
         DATA    X'8001'          ; 0667 8001  argument 2 of 0665; LDW 1
J0668    LDW     D06A5            ; 0668 86A5
         ADD     D06CE            ; 0669 A6CE
         STW     D06A5            ; 066A 76A5
         SAM                      ; 066B 0820
         JMP     J0674            ; 066C 1674
         JSX     S06AB            ; 066D 26AB
         DATA    X'001E'          ; 066E 001E  argument 1 of 066D
         DATA    X'0DEC'          ; 066F 0DEC  argument 2 of 066D
         DATA    X'8686'          ; 0670 8686  argument 3 of 066D; LDW X'686'
         LDW     D0621            ; 0671 8621
         STW     D0645            ; 0672 7645
         JMP     J0643            ; 0673 1643
J0674    JSX     S074B            ; 0674 274B
         DATA    X'06AA'          ; 0675 06AA  argument 1 of 0674
         DATA    X'8002'          ; 0676 8002  argument 2 of 0674; LDW 2
J0677    SML     3                ; 0677 0083
         JSX     S0DAF            ; 0678 25AF
         JSX     S07B7            ; 0679 27B7
         DATA    X'8003'          ; 067A 8003  argument 1 of 0679; LDW 3
         DOT     12,3             ; 067B 03C3
         CLR                      ; 067C 0100
         STW     D06A6            ; 067D 76A6
         LDW     D06A9            ; 067E 86A9
         STW     D06A5            ; 067F 76A5
         SML     2                ; 0680 0082
         JMP     J0835            ; 0681 1035
         DATA    X'0000'          ; 0682 0000
D0683    DATA    X'0000'          ; 0683 0000  written by 0630 0650
D0684    DATA    X'0000'          ; 0684 0000  written by 062F 0652 0654
         DATA    X'A08A'          ; 0685 A08A  ' \n'
         DATA    X'0000'          ; 0686 0000
         DATA    X'0000'          ; 0687 0000
         DATA    X'0000'          ; 0688 0000
         DATA    X'0000'          ; 0689 0000
         DATA    X'0000'          ; 068A 0000
         DATA    X'0000'          ; 068B 0000
         DATA    X'0000'          ; 068C 0000
         DATA    X'0000'          ; 068D 0000
         DATA    X'0000'          ; 068E 0000
         DATA    X'0000'          ; 068F 0000
         DATA    X'0000'          ; 0690 0000
         DATA    X'0000'          ; 0691 0000
         DATA    X'0000'          ; 0692 0000
         DATA    X'0000'          ; 0693 0000
         DATA    X'0000'          ; 0694 0000
         DATA    X'0000'          ; 0695 0000
         DATA    X'0000'          ; 0696 0000
         DATA    X'0000'          ; 0697 0000
         DATA    X'0000'          ; 0698 0000
         DATA    X'0000'          ; 0699 0000
         DATA    X'0000'          ; 069A 0000
         DATA    X'0000'          ; 069B 0000
         DATA    X'0000'          ; 069C 0000
         DATA    X'0000'          ; 069D 0000
         DATA    X'0000'          ; 069E 0000
         DATA    X'0000'          ; 069F 0000
         DATA    X'0000'          ; 06A0 0000
         DATA    X'0000'          ; 06A1 0000
         DATA    X'0000'          ; 06A2 0000
         DATA    X'0000'          ; 06A3 0000
D06A4    DATA    X'0000'          ; 06A4 0000  written by 062B 0648; read by 0646
D06A5    DATA    X'FFFE'          ; 06A5 FFFE  CMW *X'7FE'; written by 066A 067F; read by 0668
D06A6    DATA    X'0000'          ; 06A6 0000  written by 0624 067D; read by 065C
D06A7    DATA    X'0000'          ; 06A7 0000  written by 062A 086A; read by 0627
D06A8    DATA    X'0008'          ; 06A8 0008  read by 0637
D06A9    DATA    X'FFFE'          ; 06A9 FFFE  CMW *X'7FE'; read by 067E
         DATA    X'0087'          ; 06AA 0087
S06AB    SML     3                ; 06AB 0083
         JMP     J0EE9            ; 06AC 16E9
S06AD    STW     D07D7            ; 06AD 77D7
         SAZ                      ; 06AE 0800
         JMP     J06B2            ; 06AF 16B2
         LDW     D0622            ; 06B0 8622
         JMP     J06C2            ; 06B1 16C2
J06B2    SRA     1                ; 06B2 0901
         ADD     D07D7            ; 06B3 A7D7
         STW     D07D6            ; 06B4 77D6
         SRA     4                ; 06B5 0904
         ADD     D07D6            ; 06B6 A7D6
         SRA     4                ; 06B7 0904
         ADD     D07D6            ; 06B8 A7D6
         ADD     D06CE            ; 06B9 A6CE
         SRA     4                ; 06BA 0904
         STW     D07D6            ; 06BB 77D6
         SLL     3                ; 06BC 0A13
         ADD     D07D6            ; 06BD A7D6
         ADD     D07D6            ; 06BE A7D6
         CMP                      ; 06BF 0110
         ADD     D07D7            ; 06C0 A7D7
         ORI     D06C5            ; 06C1 C6C5
J06C2    STW     *0               ; 06C2 7800
         LDW     D07D6            ; 06C3 87D6
         JMP     *1               ; 06C4 1801
D06C5    DATA    X'A0B0'          ; 06C5 A0B0  ' 0'; read by 06C1
D06C6    DATA    X'0000'          ; 06C6 0000  written by 0719 075B 07B0; read by 071B
D06C7    DATA    X'0000'          ; 06C7 0000  written by 075E; read by 06E7 070C
D06C8    DATA    X'00F0'          ; 06C8 00F0  read by 0777
D06C9    DATA    X'0004'          ; 06C9 0004  read by 0783
D06CA    DATA    X'0204'          ; 06CA 0204  read by 070A
D06CB    DATA    X'0300'          ; 06CB 0300  read by 06E4 0707 0778
D06CC    DATA    X'0020'          ; 06CC 0020  read by 0751
D06CD    DATA    X'0010'          ; 06CD 0010  read by 0743
D06CE    DATA    X'0001'          ; 06CE 0001  read by 05B2 0602 0614 063D ...
D06CF    DATA    X'0000'          ; 06CF 0000  written by 0774; read by 06E1 0704 0741 077D
D06D0    DATA    X'0000'          ; 06D0 0000  written by 06F5 072C 0758 07AC; read by 06EE 06F3 0710 072A ...
D06D1    DATA    X'0000'          ; 06D1 0000  written by 0715; read by 071F
D06D2    DATA    X'000F'          ; 06D2 000F  read by 0742
D06D3    DATA    X'00E9'          ; 06D3 00E9  written by 079D; read by 0748 0AE4
D06D4    DATA    X'00EB'          ; 06D4 00EB  read by 06FD
D06D5    DATA    X'00EE'          ; 06D5 00EE  read by 06DA
D06D6    DATA    X'00EE'          ; 06D6 00EE  written by 07A3; read by 074C
D06D7    DATA    X'0000'          ; 06D7 0000  written by 06DB 06FE 0749 074D; read by 06E3 0706 0709 0776
D06D8    DATA    X'0000'          ; 06D8 0000  written by 06D9 06FC 0747 074B ...; read by 06EF 0711 0755 076D
S06D9    STX     D06D8            ; 06D9 66D8
         LDW     D06D5            ; 06DA 86D5
         STW     D06D7            ; 06DB 76D7
J06DC    JSX     S0763            ; 06DC 2763
J06DD    JSX     S077C            ; 06DD 277C
         CAX                      ; 06DE 0130
         LDW     D06F2            ; 06DF 86F2
         STW     *1               ; 06E0 7801
         LDW     D06CF            ; 06E1 86CF
         JSX     S0750            ; 06E2 2750
         LDW     D06D7            ; 06E3 86D7
         ORI     D06CB            ; 06E4 C6CB
         STW     D06EC            ; 06E5 76EC
         STW     D06FA            ; 06E6 76FA
         LDX     D06C7            ; 06E7 96C7
         CLB     X'C6'            ; 06E8 07C6
         SNE                      ; 06E9 0870
         DOT     12,2             ; 06EA 03C2
         LDB     *0               ; 06EB 5800
D06EC    DOT     0,0              ; 06EC 0300  written by 06E5
J06ED    JMP     J06ED            ; 06ED 16ED
         LDW     D06D0            ; 06EE 86D0
         LDX     D06D8            ; 06EF 96D8
         JMP     *2               ; 06F0 1802
         IXS     1                ; 06F1 0401
D06F2    DATA    X'06F1'          ; 06F2 06F1  the print routine's linkage constant, which the IXS in front skips: only a runaway print, its index gone negative, runs it (seen in the range and expression sessions)  --  read by 06DF
         LDW     D06D0            ; 06F3 86D0
         SUB     D06CE            ; 06F4 B6CE
         STW     D06D0            ; 06F5 76D0
         SAZ                      ; 06F6 0800
         JMP     J06F9            ; 06F7 16F9
         JMP     J0733            ; 06F8 1733
J06F9    LDB     *0               ; 06F9 5800
D06FA    DOT     0,0              ; 06FA 0300  written by 06E6
         JMP     J0741            ; 06FB 1741
J06FC    STX     D06D8            ; 06FC 66D8
         LDW     D06D4            ; 06FD 86D4
         STW     D06D7            ; 06FE 76D7
J06FF    JSX     S0763            ; 06FF 2763
J0700    JSX     S077C            ; 0700 277C
         CAX                      ; 0701 0130
         LDW     D0731            ; 0702 8731
         STW     *1               ; 0703 7801
         LDW     D06CF            ; 0704 86CF
         JSX     S0750            ; 0705 2750
         LDW     D06D7            ; 0706 86D7
         ORI     D06CB            ; 0707 C6CB
         STW     D070D            ; 0708 770D
         LDW     D06D7            ; 0709 86D7
         ORI     D06CA            ; 070A C6CA
         STW     D0713            ; 070B 7713
         LDX     D06C7            ; 070C 96C7
D070D    DOT     0,0              ; 070D 0300  written by 0708
         UNM                      ; 070E 00B0
J070F    JMP     J070F            ; 070F 170F
         LDW     D06D0            ; 0710 86D0
         LDX     D06D8            ; 0711 96D8
         JMP     *2               ; 0712 1802
D0713    DIN     0,0              ; 0713 0200  written by 070B
         MSK                      ; 0714 00A0
         STW     D06D1            ; 0715 76D1
         CLB     X'8A'            ; 0716 078A
         SEQ                      ; 0717 0860
         JMP     J071B            ; 0718 171B
         STW     D06C6            ; 0719 76C6
         JMP     J073A            ; 071A 173A
J071B    LDW     D06C6            ; 071B 86C6
         SAZ                      ; 071C 0800
         JMP     J071F            ; 071D 171F
         JMP     J073A            ; 071E 173A
J071F    LDW     D06D1            ; 071F 86D1
         CLB     X'8D'            ; 0720 078D
         SNE                      ; 0721 0870
         JMP     J0733            ; 0722 1733
         CLB     X'FF'            ; 0723 07FF
         SNE                      ; 0724 0870
         JMP     J07A7            ; 0725 17A7  not run
         CLB     X'DF'            ; 0726 07DF
         SNE                      ; 0727 0870
         JMP     J07AF            ; 0728 17AF  not run
         STB     *0               ; 0729 3800
         LDW     D06D0            ; 072A 86D0
         SUB     D06CE            ; 072B B6CE
         STW     D06D0            ; 072C 76D0
         SAZ                      ; 072D 0800
         JMP     J0730            ; 072E 1730
         JMP     J0733            ; 072F 1733
J0730    IXS     1                ; 0730 0401
D0731    DATA    X'0713'          ; 0731 0713  read by 0702
         JMP     J073A            ; 0732 173A
J0733    DOT     0,0              ; 0733 0300  written by 0779
         JSX     S077C            ; 0734 277C
         CAX                      ; 0735 0130
         LDW     *0               ; 0736 8800
         ADD     D06CE            ; 0737 A6CE
         STW     *0               ; 0738 7800
         JMP     J0741            ; 0739 1741
J073A    STX     D0746            ; 073A 6746
         JSX     S077C            ; 073B 277C
         CAX                      ; 073C 0130
         LDW     *0               ; 073D 8800
         SUB     D06CE            ; 073E B6CE
         STW     *0               ; 073F 7800
         LDX     D0746            ; 0740 9746
J0741    LDW     D06CF            ; 0741 86CF
         AND     D06D2            ; 0742 E6D2
         ORI     D06CD            ; 0743 C6CD
         STW     D0745            ; 0744 7745
D0745    INR     0                ; 0745 0010  written by 0744
D0746    DATA    X'0000'          ; 0746 0000  written by 073A; read by 0740
J0747    STX     D06D8            ; 0747 66D8
         LDW     D06D3            ; 0748 86D3
         STW     D06D7            ; 0749 76D7
         JMP     J06FF            ; 074A 16FF
S074B    STX     D06D8            ; 074B 66D8
         LDW     D06D6            ; 074C 86D6
         STW     D06D7            ; 074D 76D7
         JMP     J06DC            ; 074E 16DC
D074F    DATA    X'0000'          ; 074F 0000  written by 0750; read by 0760
S0750    STX     D074F            ; 0750 674F
         ORI     D06CC            ; 0751 C6CC
         STW     D0754            ; 0752 7754
         UNM                      ; 0753 00B0
D0754    ENB     0                ; 0754 0020  written by 0752
         LDX     D06D8            ; 0755 96D8
         LDW     *1               ; 0756 8801
         AND     D07D5            ; 0757 E7D5
         STW     D06D0            ; 0758 76D0
         STW     D0762            ; 0759 7762
         CLR                      ; 075A 0100
         STW     D06C6            ; 075B 76C6
         LDW     *0               ; 075C 8800
         SLL     1                ; 075D 0A11
         STW     D06C7            ; 075E 76C7
         JSX     S0776            ; 075F 2776
J0760    LDX     D074F            ; 0760 974F
         JSX     *0               ; 0761 2800
D0762    DATA    X'0000'          ; 0762 0000  written by 0759; read by 07A9
S0763    SRL     4                ; 0763 0A04
         CLB     X'E'             ; 0764 070E
         SNE                      ; 0765 0870
         JMP     J076F            ; 0766 176F
         CLB     X'D'             ; 0767 070D
         SNE                      ; 0768 0870
         JMP     J0771            ; 0769 1771
         CLB     X'C'             ; 076A 070C
         SNE                      ; 076B 0870
         JMP     J0773            ; 076C 1773
         LDX     D06D8            ; 076D 96D8  not run
         JMP     *2               ; 076E 1802  not run
J076F    LLB     0                ; 076F 0600
         JMP     J0774            ; 0770 1774
J0771    LLB     0                ; 0771 0600
         JMP     J0774            ; 0772 1774
J0773    LLB     0                ; 0773 0600
J0774    STW     D06CF            ; 0774 76CF
         JMP     *0               ; 0775 1800
S0776    LDW     D06D7            ; 0776 86D7
         AND     D06C8            ; 0777 E6C8
         ORI     D06CB            ; 0778 C6CB
         STW     J0733            ; 0779 7733
         JMP     *0               ; 077A 1800
D077B    DATA    X'0000'          ; 077B 0000  written by 077C; read by 0781
S077C    STX     D077B            ; 077C 677B
         LDX     D06CF            ; 077D 96CF
         CLR                      ; 077E 0100
J077F    DXS     1                ; 077F 0501
         JMP     J0783            ; 0780 1783  not run
         LDX     D077B            ; 0781 977B
         JSX     *0               ; 0782 2800
J0783    ADD     D06C9            ; 0783 A6C9  not run
         JMP     J077F            ; 0784 177F  not run
J0785    LLB     0                ; 0785 0600
         SLC     8                ; 0786 0A58
         CLB     X'D4'            ; 0787 07D4
         SNE                      ; 0788 0870
         JMP     J07A5            ; 0789 17A5
         CLB     X'D0'            ; 078A 07D0
         SNE                      ; 078B 0870
         JMP     J07A1            ; 078C 17A1
         CLB     X'CB'            ; 078D 07CB
         SNE                      ; 078E 0870
         JMP     J079F            ; 078F 179F
         CLB     X'C8'            ; 0790 07C8
         SNE                      ; 0791 0870
         JMP     J0798            ; 0792 1798
         CLB     X'CA'            ; 0793 07CA
         SNE                      ; 0794 0870
         JMP     J079A            ; 0795 179A
J0796    SML     2                ; 0796 0082
         JMP     J0835            ; 0797 1035
J0798    LLB     X'D1'            ; 0798 06D1
         JMP     J079C            ; 0799 179C
J079A    LLB     X'C6'            ; 079A 06C6
         JMP     J07A2            ; 079B 17A2
J079C    SML     1                ; 079C 0081
         STW     D06D3            ; 079D 76D3
         JMP     J0796            ; 079E 1796
J079F    LLB     X'EB'            ; 079F 06EB
         JMP     J079C            ; 07A0 179C
J07A1    LLB     X'EE'            ; 07A1 06EE
J07A2    SML     1                ; 07A2 0081
         STW     D06D6            ; 07A3 76D6
         JMP     J0796            ; 07A4 1796
J07A5    LLB     X'E9'            ; 07A5 06E9
         JMP     J079C            ; 07A6 179C
J07A7    LDW     D06D0            ; 07A7 86D0  not run
         ADD     D06CE            ; 07A8 A6CE  not run
         CMW     D0762            ; 07A9 F762  not run
         SLE                      ; 07AA 0890  not run
         JMP     J073A            ; 07AB 173A  not run
         STW     D06D0            ; 07AC 76D0  not run
         DXS     1                ; 07AD 0501  not run
         JMP     J073A            ; 07AE 173A  not run
J07AF    CLR                      ; 07AF 0100  not run
         STW     D06C6            ; 07B0 76C6  not run
         LDW     D07B5            ; 07B1 87B5  not run
         STW     D06D8            ; 07B2 76D8  not run
         UNM                      ; 07B3 00B0  not run
         JMP     J0733            ; 07B4 1733  not run
D07B5    DATA    X'0DCB'          ; 07B5 0DCB  read by 07B1
D07B6    DATA    X'0000'          ; 07B6 0000  written by 07B7; read by 07CF
S07B7    STX     D07B6            ; 07B7 67B6
         LDW     *0               ; 07B8 8800
J07B9    LDX     D07D4            ; 07B9 97D4
         SLL     4                ; 07BA 0A14
         SRCD    4                ; 07BB 0A64
         STW     D07D6            ; 07BC 77D6
         LDW     D07D4            ; 07BD 87D4
         SLC     4                ; 07BE 0A54
         STW     D07D4            ; 07BF 77D4
         LDW     D07D6            ; 07C0 87D6
         LDX     D07D5            ; 07C1 97D5
J07C2    DXS     1                ; 07C2 0501
         JMP     J07C2            ; 07C3 17C2
         SS3                      ; 07C4 08F0
         JMP     J07D1            ; 07C5 17D1
         SAZ                      ; 07C6 0800  not run
         JMP     J07B9            ; 07C7 17B9  not run
J07C8    LDW     D07D4            ; 07C8 87D4  not run
         SAZ                      ; 07C9 0800  not run
         JMP     J07CC            ; 07CA 17CC  not run
         JMP     J07C8            ; 07CB 17C8  not run
J07CC    LDX     D07D5            ; 07CC 97D5
J07CD    DXS     1                ; 07CD 0501
         JMP     J07CD            ; 07CE 17CD
         LDX     D07B6            ; 07CF 97B6
         JSX     *1               ; 07D0 2801
J07D1    SS3                      ; 07D1 08F0
         JMP     J07D1            ; 07D2 17D1  not run
         JMP     J07CC            ; 07D3 17CC
D07D4    DATA    X'F0F0'          ; 07D4 F0F0  'pp'; written by 07BF; read by 07B9 07BD 07C8
D07D5    DATA    X'7FFF'          ; 07D5 7FFF  STW *X'7FF'; read by 0757 07C1 07CC
D07D6    DATA    X'0000'          ; 07D6 0000  written by 06B4 06BB 07BC; read by 06B6 06B8 06BD 06BE ...
D07D7    DATA    X'0000'          ; 07D7 0000  written by 06AD; read by 06B3 06C0
J07D8    SML     3                ; 07D8 0083  not run
         LDW     D0C6C            ; 07D9 846C  not run
         SRC     8                ; 07DA 0A48  not run
         SML     3                ; 07DB 0083  not run
         STW     D0DEB            ; 07DC 75EB  not run
         JSX     S07B7            ; 07DD 27B7  not run
         DATA    X'8FFF'          ; 07DE 8FFF  argument 1 of 07DD; LDW *X'7FF'
         LDB     D07E6            ; 07DF 57CC  not run
         SML     3                ; 07E0 0083  not run
         STB     D0DFA            ; 07E1 33F4  not run
         JSX     S06D9            ; 07E2 26D9  not run
         DATA    X'0DEB'          ; 07E3 0DEB  argument 1 of 07E2
         DATA    X'801F'          ; 07E4 801F  argument 2 of 07E2; LDW X'1F'
         JMP     J0796            ; 07E5 1796  not run
D07E6    DATA    X'8D00'          ; 07E6 8D00  LDW *X'500'; read by 07DF
J07E7    LDW     D07F0            ; 07E7 87F0
         STW     D07F4            ; 07E8 77F4
         LDW     D07F1            ; 07E9 87F1
         STW     D07F6            ; 07EA 77F6
J07EB    JSX     S06AB            ; 07EB 26AB
         DATA    X'000B'          ; 07EC 000B  argument 1 of 07EB
         DATA    X'07F2'          ; 07ED 07F2  argument 2 of 07EB
         DATA    X'8000'          ; 07EE 8000  argument 3 of 07EB; LDW 0
         DATA    X'0000'          ; 07EF 0000
D07F0    DATA    X'03D9'          ; 07F0 03D9  read by 07E7
D07F1    DATA    X'02DD'          ; 07F1 02DD  read by 07E9
         DATA    X'0020'          ; 07F2 0020
         DATA    X'8004'          ; 07F3 8004  LDW 4
D07F4    DATA    X'03E9'          ; 07F4 03E9  written by 07E8
         DATA    X'1003'          ; 07F5 1003  JMP 3
D07F6    DATA    X'02ED'          ; 07F6 02ED  written by 07EA
         DATA    X'0800'          ; 07F7 0800
         DATA    X'0401'          ; 07F8 0401
         DATA    X'0010'          ; 07F9 0010
         DATA    X'0638'          ; 07FA 0638
         DATA    X'300A'          ; 07FB 300A  STB X'A'
         DATA    X'0010'          ; 07FC 0010
         DATA    X'0000'          ; 07FD 0000
         DATA    X'0000'          ; 07FE 0000
         DATA    X'0000'          ; 07FF 0000
; Where SYM I starts: the tape's last record is 8800.
         SML     2                ; 0800 0082
         JMP     J0835            ; 0801 1035
D0802    DATA    X'05B0'          ; 0802 05B0  read by 0874
D0803    DATA    X'0000'          ; 0803 0000  written by 0877 09B1; read by 060E 0990 09A3 09B5 ...
D0804    DATA    X'00C0'          ; 0804 00C0  read by 0973 0AB0 0ADA 0B4B ...
D0805    DATA    X'0002'          ; 0805 0002  read by 09A0 0BA2 0E43 0E5C ...
D0806    DATA    X'0005'          ; 0806 0005  read by 0B99
D0807    DATA    X'0001'          ; 0807 0001  read by 08B4 094C 0956 0B08 ...
D0808    DATA    X'0080'          ; 0808 0080  read by 0959 0970 0ADC 0B3C ...
D0809    DATA    X'00B0'          ; 0809 00B0  read by 0CD2 0EE6
D080A    DATA    X'0090'          ; 080A 0090  read by 0AAA 0B61 0B66
D080B    DATA    X'00A0'          ; 080B 00A0  read by 0AAD 0CFB
D080C    DATA    X'0006'          ; 080C 0006  read by 09B6 0D93
D080D    DATA    X'0004'          ; 080D 0004  read by 0876 08B6 0992 09B0 ...
D080E    DATA    X'0003'          ; 080E 0003  read by 0D62
D080F    DATA    X'0007'          ; 080F 0007  read by 09E1 0EE5
D0810    DATA    X'000B'          ; 0810 000B  read by 0CD4
D0811    DATA    X'0000'          ; 0811 0000  written by 0881 0CD7; read by 0D8A
D0812    DATA    X'000A'          ; 0812 000A  read by 0880
D0813    DATA    X'0000'          ; 0813 0000  written by 086C 0C35; read by 0B44
D0814    DATA    X'0200'          ; 0814 0200  read by 05EF 0A2F 0B22 0B9C
D0815    DATA    X'0000'          ; 0815 0000  read by 0B1E 0BE3
D0816    DATA    X'8000'          ; 0816 8000  LDW 0; read by 0B64 0CB8 0D6C 0EEB ...
         DATA    X'0180'          ; 0817 0180
D0818    DATA    X'0000'          ; 0818 0000  written by 09F1; read by 0960 0AD6
D0819    DATA    X'0040'          ; 0819 0040  read by 0A37 0A42 0A59
D081A    DATA    X'0002'          ; 081A 0002  read by 0A2E 0A31 0A7D 0B21
D081B    DATA    X'0001'          ; 081B 0001  read by 0A2C 0A74 0B1F
D081C    DATA    X'0004'          ; 081C 0004  read by 09AB 0B2A 0B87 0BA4 ...
D081D    DATA    X'0100'          ; 081D 0100  read by 0B85 0B9B
D081E    DATA    X'0000'          ; 081E 0000  written by 0A54; read by 0A5D
D081F    DATA    X'0000'          ; 081F 0000  written by 05D6 0962 0967 09BC ...; read by 0964 09E4 0A06 0A0A ...
D0820    DATA    X'0000'          ; 0820 0000  written by 0A56 0EDC; read by 0A78 0A89 0EDF
D0821    DATA    X'0000'          ; 0821 0000  written by 09F5 0A33 0A3C 0A43 ...; read by 08EC 08F7 09F4 0A32 ...
D0822    DATA    X'0000'          ; 0822 0000  written by 0A58 0ED8; read by 0A8F 0EDD
D0823    DATA    X'0000'          ; 0823 0000  written by 0974 0AB1 0ADD 0B9A ...; read by 0AF0 0BB5
D0824    DATA    X'0000'          ; 0824 0000  written by 0842 0873; read by 05B7 0A2B 0AFE 0B1D ...
D0825    DATA    X'0000'          ; 0825 0000  written by 0872 0953; read by 0943 0952
D0826    DATA    X'0000'          ; 0826 0000  written by 0875; read by 05E1 0993 0BAE 0D71
D0827    DATA    X'0000'          ; 0827 0000  written by 0871
D0828    DATA    X'0000'          ; 0828 0000  written by 084D 0870; read by 085D 0AEC 0B33 0D6E ...
S0829    SML     1                ; 0829 0081
         JMP     S06D9            ; 082A 16D9
S082B    SML     1                ; 082B 0081
         JMP     J06FC            ; 082C 16FC
S082D    SML     1                ; 082D 0081
         JMP     J0747            ; 082E 1747
S082F    SML     1                ; 082F 0081
         JMP     S074B            ; 0830 174B
J0831    SML     1                ; 0831 0081
         JMP     J0624            ; 0832 1624
S0833    SML     1                ; 0833 0081
         JMP     J0608            ; 0834 1608
J0835    JSX     S0829            ; 0835 2029
         DATA    X'0863'          ; 0836 0863  argument 1 of 0835
         DATA    X'8004'          ; 0837 8004  argument 2 of 0835; LDW 4
J0838    JSX     S082B            ; 0838 202B
         DATA    X'0865'          ; 0839 0865  argument 1 of 0838
         DATA    X'8002'          ; 083A 8002  argument 2 of 0838; LDW 2
J083B    JSX     S0829            ; 083B 2029
         DATA    X'0E1C'          ; 083C 0E1C  argument 1 of 083B
         DATA    X'8002'          ; 083D 8002  argument 2 of 083B; LDW 2
J083E    JSX     S0867            ; 083E 2067
         LDW     D0865            ; 083F 8065
         CLB     X'C1'            ; 0840 07C1
         SNE                      ; 0841 0870
         STW     D0824            ; 0842 7024
         SLC     8                ; 0843 0A58
         CLB     X'D8'            ; 0844 07D8
         SNE                      ; 0845 0870
         JMP     J0857            ; 0846 1057
         CLB     X'D3'            ; 0847 07D3
         SNE                      ; 0848 0870
         JMP     J085D            ; 0849 105D
         CLB     X'D0'            ; 084A 07D0
         SEQ                      ; 084B 0860
         JMP     J084F            ; 084C 104F
         STW     D0828            ; 084D 7028
         JMP     J0860            ; 084E 1060
J084F    CLB     X'C4'            ; 084F 07C4
         SNE                      ; 0850 0870
         JMP     J0835            ; 0851 1035  not run
         CLB     X'C3'            ; 0852 07C3
         SEQ                      ; 0853 0860
         JMP     J0835            ; 0854 1035  not run
         SML     1                ; 0855 0081
         JMP     J0785            ; 0856 1785
J0857    SLC     8                ; 0857 0A58
         CLB     X'C8'            ; 0858 07C8
         SML     1                ; 0859 0081
         SEQ                      ; 085A 0860
         JMP     X'0FEB'          ; 085B 17EB  lands at 07EB = J07EB under the selection in force
         JMP     X'0FE7'          ; 085C 17E7  page from the run; lands at 07E7 = J07E7 under the selection in force
J085D    LDW     D0828            ; 085D 8028
         SAZ                      ; 085E 0800
         JSX     S0833            ; 085F 2033
J0860    JSX     S0DCC            ; 0860 25CC
         JSX     S0885            ; 0861 2085
         JMP     J0926            ; 0862 1126
         DATA    X'8ABF'          ; 0863 8ABF  '\n?'
         DATA    X'BF8D'          ; 0864 BF8D  '?\r'
D0865    DATA    X'0000'          ; 0865 0000  written by 0D55; read by 083F 0D5B
D0866    DATA    X'0000'          ; 0866 0000  written by 0867; read by 0882
S0867    STX     D0866            ; 0867 6066
         CLR                      ; 0868 0100
         SML     1                ; 0869 0081
         STW     D06A7            ; 086A 76A7
         STW     D0DAA            ; 086B 75AA
         STW     D0813            ; 086C 7013
         STW     D0C6D            ; 086D 746D
         STW     D0E49            ; 086E 7649
         STW     D0DAD            ; 086F 75AD
         STW     D0828            ; 0870 7028
         STW     D0827            ; 0871 7027
         STW     D0825            ; 0872 7025
         STW     D0824            ; 0873 7024
         LDW     D0802            ; 0874 8002
         STW     D0826            ; 0875 7026
         SUB     D080D            ; 0876 B00D
         STW     D0803            ; 0877 7003
         LDW     D0E48            ; 0878 8648
         STW     D0E47            ; 0879 7647
         SRL     1                ; 087A 0A01
         SML     1                ; 087B 0081
         STW     D0619            ; 087C 7619
         SML     1                ; 087D 0081
         STW     D0623            ; 087E 7623
         STW     D0B6B            ; 087F 736B
         LDW     D0812            ; 0880 8012
         STW     D0811            ; 0881 7011
         LDX     D0866            ; 0882 9066
         JSX     *0               ; 0883 2800
D0884    DATA    X'0000'          ; 0884 0000  written by 0885; read by 089C
S0885    STX     D0884            ; 0885 6084
         CLR                      ; 0886 0100
         STW     D08A0            ; 0887 70A0
         JSX     S08D3            ; 0888 20D3
         LDW     D089E            ; 0889 809E
         CLB     X'AA'            ; 088A 07AA
         SNE                      ; 088B 0870
         JMP     J085D            ; 088C 105D
         CLB     X'A7'            ; 088D 07A7
         SNE                      ; 088E 0870
         JMP     J085D            ; 088F 105D
         JSX     S0920            ; 0890 2120
J0891    JMP     J0898            ; 0891 1098
J0892    JSX     S08A4            ; 0892 20A4
         LDW     D08CF            ; 0893 80CF
         STW     D08A1            ; 0894 70A1
         LDW     D08D0            ; 0895 80D0
         STW     D08A2            ; 0896 70A2
         STX     D08A0            ; 0897 60A0
J0898    JSX     S08E2            ; 0898 20E2
         CLB     X'FF'            ; 0899 07FF
         SNE                      ; 089A 0870
         JMP     J0C79            ; 089B 1479  not run
         LDX     D0884            ; 089C 9084
         JSX     *0               ; 089D 2800
D089E    DATA    X'0000'          ; 089E 0000  written by 08DA 09E3 0A05 0E0F ...; read by 0889 08B1 08C7 08FD ...
D089F    DATA    X'0000'          ; 089F 0000  written by 08D8; read by 09D0 0CE6
D08A0    DATA    X'0000'          ; 08A0 0000  written by 0887 0897; read by 0B0E 0C37
D08A1    DATA    X'0000'          ; 08A1 0000  written by 0894; read by 0C50
D08A2    DATA    X'0000'          ; 08A2 0000  written by 0896; read by 0C52
D08A3    DATA    X'0000'          ; 08A3 0000  written by 08A4; read by 08BE
S08A4    STX     D08A3            ; 08A4 60A3
         CLR                      ; 08A5 0100
         STW     D0905            ; 08A6 7105
         JSX     S08FC            ; 08A7 20FC
J08A8    JMP     J08AB            ; 08A8 10AB
J08A9    LLB     X'CC'            ; 08A9 06CC
         JMP     J0C63            ; 08AA 1463
J08AB    LDW     D0EC4            ; 08AB 86C4
         STW     D08CF            ; 08AC 70CF
         STW     D08D0            ; 08AD 70D0
J08AE    LDW     D0905            ; 08AE 8105
         ADD     D08D1            ; 08AF A0D1
         CAX                      ; 08B0 0130
         LDW     D089E            ; 08B1 809E
         STB     *0               ; 08B2 3800
         LDW     D0905            ; 08B3 8105
         ADD     D0807            ; 08B4 A007
         STW     D0905            ; 08B5 7105
         CMW     D080D            ; 08B6 F00D
         SLS                      ; 08B7 0840
         JMP     J08BC            ; 08B8 10BC
         JSX     S08C1            ; 08B9 20C1
         JMP     J08AE            ; 08BA 10AE
         JMP     J08BE            ; 08BB 10BE
J08BC    JSX     S08C1            ; 08BC 20C1
         JMP     J08BC            ; 08BD 10BC
J08BE    LDX     D08A3            ; 08BE 90A3
         JSX     *0               ; 08BF 2800
D08C0    DATA    X'0000'          ; 08C0 0000  written by 08C1; read by 08CB 08CD
S08C1    STX     D08C0            ; 08C1 60C0
         JSX     S08D3            ; 08C2 20D3
         JSX     S08FC            ; 08C3 20FC
J08C4    JMP     J08CD            ; 08C4 10CD
J08C5    JSX     S0908            ; 08C5 2108
J08C6    JMP     J08CD            ; 08C6 10CD
J08C7    LDW     D089E            ; 08C7 809E
         CLB     X'AE'            ; 08C8 07AE
         SNE                      ; 08C9 0870
         JMP     J08CD            ; 08CA 10CD  not run
         LDX     D08C0            ; 08CB 90C0
         JSX     *1               ; 08CC 2801
J08CD    LDX     D08C0            ; 08CD 90C0
         JSX     *0               ; 08CE 2800
D08CF    DATA    X'0000'          ; 08CF 0000  written by 08AC 0C51; read by 0893 092D 0998 09A7 ...
D08D0    DATA    X'0000'          ; 08D0 0000  written by 08AD 0C53; read by 0895 0936 099C 09A9 ...
D08D1    DATA    X'119E'          ; 08D1 119E  JMP X'19E'; read by 08AF
D08D2    DATA    X'0000'          ; 08D2 0000  written by 08D3; read by 08DF
S08D3    STX     D08D2            ; 08D3 60D2
         STW     D08DC            ; 08D4 70DC
         LDX     D0DCA            ; 08D5 95CA
         CLR                      ; 08D6 0100
         LDB     *1               ; 08D7 5801
         STW     D089F            ; 08D8 709F
         LDB     *0               ; 08D9 5800
         STW     D089E            ; 08DA 709E
         IXS     1                ; 08DB 0401
D08DC    DATA    X'0000'          ; 08DC 0000  written by 08D4 08FC 0908 0911 ...; read by 08DE 0906
         STX     D0DCA            ; 08DD 65CA
         LDW     D08DC            ; 08DE 80DC
         LDX     D08D2            ; 08DF 90D2
         JSX     *0               ; 08E0 2800
D08E1    DATA    X'0000'          ; 08E1 0000  written by 08E2; read by 08E5
S08E2    STX     D08E1            ; 08E2 60E1
J08E3    JSX     S0920            ; 08E3 2120
J08E4    JMP     J08E7            ; 08E4 10E7
J08E5    LDX     D08E1            ; 08E5 90E1
         JSX     *0               ; 08E6 2800
J08E7    JSX     S08D3            ; 08E7 20D3
         JMP     J08E3            ; 08E8 10E3
S08E9    STW     D08F2            ; 08E9 70F2
         LDW     *0               ; 08EA 8800
         AND     D08F3            ; 08EB E0F3
         AND     D0821            ; 08EC E021
         SAZ                      ; 08ED 0800
         IXS     1                ; 08EE 0401
         SGM                      ; 08EF 0050
         LDW     D08F2            ; 08F0 80F2
         JMP     *1               ; 08F1 1801
D08F2    DATA    X'0000'          ; 08F2 0000  written by 08E9 08F4; read by 08F0 08FA
D08F3    DATA    X'7FFF'          ; 08F3 7FFF  STW *X'7FF'; read by 08EB 08F6 0D56
S08F4    STW     D08F2            ; 08F4 70F2
         LDW     *0               ; 08F5 8800
         AND     D08F3            ; 08F6 E0F3
         AND     D0821            ; 08F7 E021
         SAZ                      ; 08F8 0800
         DXS     1                ; 08F9 0501
         LDW     D08F2            ; 08FA 80F2
         JMP     *2               ; 08FB 1802
S08FC    STW     D08DC            ; 08FC 70DC
         LDW     D089E            ; 08FD 809E
         CLB     X'C0'            ; 08FE 07C0
         SGR                      ; 08FF 0880
         JMP     J0904            ; 0900 1104
         CLB     X'DA'            ; 0901 07DA
         SGR                      ; 0902 0880
         JMP     J0906            ; 0903 1106
J0904    IXS     1                ; 0904 0401
D0905    DATA    X'0000'          ; 0905 0000  written by 08A6 08B5; read by 08AE 08B3
J0906    LDW     D08DC            ; 0906 80DC
         JMP     *0               ; 0907 1800
S0908    STW     D08DC            ; 0908 70DC
         LDW     D089E            ; 0909 809E
         CLB     X'AF'            ; 090A 07AF
         SGR                      ; 090B 0880
         JMP     J0904            ; 090C 1104
         CLB     X'B9'            ; 090D 07B9
         SLE                      ; 090E 0890
         JMP     J0904            ; 090F 1104
         JMP     J0906            ; 0910 1106
S0911    STW     D08DC            ; 0911 70DC
         LDW     D089E            ; 0912 809E
         CLB     X'AF'            ; 0913 07AF
         SGR                      ; 0914 0880
         JMP     J0904            ; 0915 1104
         CLB     X'C6'            ; 0916 07C6
         SLE                      ; 0917 0890
         JMP     J0904            ; 0918 1104  not run
         CLB     X'B9'            ; 0919 07B9
         SGR                      ; 091A 0880
         JMP     J0906            ; 091B 1106
         CLB     X'C0'            ; 091C 07C0
         SGR                      ; 091D 0880
         JMP     J0904            ; 091E 1104  not run
         JMP     J0906            ; 091F 1106
S0920    STW     D08DC            ; 0920 70DC
         LDW     D089E            ; 0921 809E
         CLB     X'A0'            ; 0922 07A0
         SNE                      ; 0923 0870
         JMP     J0906            ; 0924 1106
         JMP     J0904            ; 0925 1104
J0926    LDW     D0DCA            ; 0926 85CA
         STW     D0976            ; 0927 7176
         JSX     S08A4            ; 0928 20A4
         JSX     S08E2            ; 0929 20E2
         LDX     D0931            ; 092A 9131
J092B    LDW     *0               ; 092B 8800
         STW     D0DAD            ; 092C 75AD
         CMW     D08CF            ; 092D F0CF
         SNE                      ; 092E 0870
         JMP     J0935            ; 092F 1135
J0930    IXS     3                ; 0930 0403
D0931    DATA    X'0F07'          ; 0931 0F07  read by 092A
         SAZ                      ; 0932 0800
         JMP     J092B            ; 0933 112B
         JMP     J0955            ; 0934 1155
J0935    LDW     *1               ; 0935 8801
         CMW     D08D0            ; 0936 F0D0
         SEQ                      ; 0937 0860
         JMP     J0930            ; 0938 1130
         LDW     *2               ; 0939 8802
         SRLD    4                ; 093A 0A24
         STW     D09FF            ; 093B 71FF
         SLL     4                ; 093C 0A14
         STW     D0949            ; 093D 7149
         CLR                      ; 093E 0100
         SLLD    4                ; 093F 0A34
         LDX     D09FF            ; 0940 91FF
         SAZ                      ; 0941 0800
         CAX                      ; 0942 0130
         LDW     D0825            ; 0943 8025
         SAZ                      ; 0944 0800
         JMP     J094A            ; 0945 114A  not run
         SLM                      ; 0946 0040
         LDX     *X'177'          ; 0947 9977
         JSX     *0               ; 0948 2800
D0949    DATA    X'0000'          ; 0949 0000  written by 093D 095A 0AC3 0CC9 ...; read by 0C72 0C9F 0CC7 0CE3 ...
J094A    CXA                      ; 094A 0140  not run
         CLB     X'14'            ; 094B 0714  not run
         LDW     D0807            ; 094C 8007  not run
         SLE                      ; 094D 0890  not run
         JMP     J0952            ; 094E 1152  not run
         CMP                      ; 094F 0110  not run
         SEQ                      ; 0950 0860  not run
         JMP     J085D            ; 0951 105D  not run
J0952    ADD     D0825            ; 0952 A025  not run
         STW     D0825            ; 0953 7025  not run
         JMP     J085D            ; 0954 105D  not run
J0955    LDW     D0976            ; 0955 8176
         SUB     D0807            ; 0956 B007
         STW     D0DCA            ; 0957 75CA
         JSX     S08D3            ; 0958 20D3
         LDW     D0808            ; 0959 8008
         STW     D0949            ; 095A 7149
         JSX     S0D42            ; 095B 2542
         JSX     S08E2            ; 095C 20E2
         JSX     S08E9            ; 095D 20E9
         DATA    X'8004'          ; 095E 8004  argument 1 of 095D; LDW 4
         JMP     J0970            ; 095F 1170  not run
J0960    LDX     D0818            ; 0960 9018
         LDW     *1               ; 0961 8801
         STW     D081F            ; 0962 701F
         JSX     S0ACD            ; 0963 22CD
J0964    LDW     D081F            ; 0964 801F
         AND     D0CBC            ; 0965 E4BC
         ORI     D0977            ; 0966 C177
         STW     D081F            ; 0967 701F
         JSX     S0EB4            ; 0968 26B4
         JSX     S0AEA            ; 0969 22EA
         JSX     S0C56            ; 096A 2456
         LDW     D089E            ; 096B 809E
         CLB     X'FF'            ; 096C 07FF
         SNE                      ; 096D 0870
         JMP     J0C79            ; 096E 1479  not run
         JMP     J0CB1            ; 096F 14B1
J0970    LDW     D0808            ; 0970 8008  not run
         JSX     S08F4            ; 0971 20F4  not run
         DATA    X'8002'          ; 0972 8002  argument 1 of 0971; LDW 2
         LDW     D0804            ; 0973 8004  not run
         STW     D0823            ; 0974 7023  not run
         JMP     J0964            ; 0975 1164  not run
D0976    DATA    X'0000'          ; 0976 0000  written by 0927; read by 0955
D0977    DATA    X'2000'          ; 0977 2000  JSX 0; read by 0966
         DATA    X'0C87'          ; 0978 0C87
         DATA    X'0CCE'          ; 0979 0CCE
         DATA    X'0CDC'          ; 097A 0CDC
         DATA    X'0CE6'          ; 097B 0CE6
         DATA    X'0D07'          ; 097C 0D07
         DATA    X'0D0C'          ; 097D 0D0C
         DATA    X'0D0F'          ; 097E 0D0F
         DATA    X'0D21'          ; 097F 0D21
         DATA    X'0D32'          ; 0980 0D32
         DATA    X'0D3F'          ; 0981 0D3F
         DATA    X'0C37'          ; 0982 0C37
         DATA    X'0A99'          ; 0983 0A99
         DATA    X'0C0E'          ; 0984 0C0E
         DATA    X'0C24'          ; 0985 0C24
         DATA    X'0B85'          ; 0986 0B85
         DATA    X'0B87'          ; 0987 0B87
         DATA    X'0B70'          ; 0988 0B70
         DATA    X'0D65'          ; 0989 0D65
         DATA    X'0AC0'          ; 098A 0AC0
         DATA    X'085D'          ; 098B 085D
         DATA    X'0BE1'          ; 098C 0BE1
         DATA    X'0BE3'          ; 098D 0BE3
D098E    DATA    X'0000'          ; 098E 0000  written by 098F; read by 09A1 09B7
S098F    STX     D098E            ; 098F 618E
         LDX     D0803            ; 0990 9003
J0991    CXA                      ; 0991 0140
         ADD     D080D            ; 0992 A00D
         CMW     D0826            ; 0993 F026
         SNE                      ; 0994 0870
         JMP     J09A3            ; 0995 11A3
         CAX                      ; 0996 0130
         LDW     *0               ; 0997 8800
         CMW     D08CF            ; 0998 F0CF
         SEQ                      ; 0999 0860
         JMP     J0991            ; 099A 1191
         LDW     *1               ; 099B 8801
         CMW     D08D0            ; 099C F0D0
         SEQ                      ; 099D 0860
         JMP     J0991            ; 099E 1191
         CXA                      ; 099F 0140
         ADD     D0805            ; 09A0 A005
         LDX     D098E            ; 09A1 918E
         JSX     *0               ; 09A2 2800
J09A3    LDX     D0803            ; 09A3 9003
         LDW     D0C86            ; 09A4 8486
         SAZ                      ; 09A5 0800
         JMP     J0C3A            ; 09A6 143A  not run
         LDW     D08CF            ; 09A7 80CF
         STW     *0               ; 09A8 7800
         LDW     D08D0            ; 09A9 80D0
         STW     *1               ; 09AA 7801
         LDW     D081C            ; 09AB 801C
         STW     *2               ; 09AC 7802
         LDW     D0E0A            ; 09AD 860A
         STW     *3               ; 09AE 7803
         CXA                      ; 09AF 0140
         SUB     D080D            ; 09B0 B00D
         STW     D0803            ; 09B1 7003
         CMW     D0E47            ; 09B2 F647
         SGR                      ; 09B3 0880
         JSX     S0ACA            ; 09B4 22CA  not run
         LDW     D0803            ; 09B5 8003
         ADD     D080C            ; 09B6 A00C
         LDX     D098E            ; 09B7 918E
         JSX     *1               ; 09B8 2801
D09B9    DATA    X'0000'          ; 09B9 0000  written by 09BA; read by 0A34
S09BA    STX     D09B9            ; 09BA 61B9
         CLR                      ; 09BB 0100
         STW     D081F            ; 09BC 701F
         LDW     D089E            ; 09BD 809E
         CLB     X'D8'            ; 09BE 07D8
         SNE                      ; 09BF 0870
         JMP     J09D0            ; 09C0 11D0
         JSX     S08FC            ; 09C1 20FC
J09C2    JMP     J09EE            ; 09C2 11EE
J09C3    JSX     S0908            ; 09C3 2108
J09C4    JMP     J0A00            ; 09C4 1200
J09C5    CLB     X'A7'            ; 09C5 07A7
         SNE                      ; 09C6 0870
         JMP     J0A16            ; 09C7 1216
         CLB     X'A4'            ; 09C8 07A4
         SNE                      ; 09C9 0870
         JMP     J0A24            ; 09CA 1224
         CLB     X'AC'            ; 09CB 07AC
         SNE                      ; 09CC 0870
         JMP     J0A31            ; 09CD 1231  not run
S09CE    LLB     X'D0'            ; 09CE 06D0
         JMP     J0C63            ; 09CF 1463
J09D0    LDW     D089F            ; 09D0 809F
         CLB     X'A7'            ; 09D1 07A7
         SEQ                      ; 09D2 0860
         JMP     J09EE            ; 09D3 11EE  not run
         JSX     S08D3            ; 09D4 20D3
         JSX     S08D3            ; 09D5 20D3
J09D6    JSX     S0911            ; 09D6 2111
J09D7    JMP     J09DE            ; 09D7 11DE
J09D8    LDW     D089E            ; 09D8 809E
         CLB     X'A7'            ; 09D9 07A7
         SEQ                      ; 09DA 0860
         JMP     S09CE            ; 09DB 11CE  not run
         JSX     S08D3            ; 09DC 20D3
         JMP     J0A31            ; 09DD 1231
J09DE    LDW     D089E            ; 09DE 809E
         CLB     X'B9'            ; 09DF 07B9
         SLE                      ; 09E0 0890
         SUB     D080F            ; 09E1 B00F
         AND     D09ED            ; 09E2 E1ED
         STW     D089E            ; 09E3 709E
         LDW     D081F            ; 09E4 801F
         SLA     3                ; 09E5 0913
         SNO                      ; 09E6 08A0
         JMP     S09CE            ; 09E7 11CE  not run
         SLL     1                ; 09E8 0A11
         ADD     D089E            ; 09E9 A09E
         STW     D081F            ; 09EA 701F
         JSX     S08D3            ; 09EB 20D3
         JMP     J09D6            ; 09EC 11D6
D09ED    DATA    X'000F'          ; 09ED 000F  read by 09E2 0A04 0D28 0EE2
J09EE    JSX     S08A4            ; 09EE 20A4
         JSX     S098F            ; 09EF 218F
         SLL     0                ; 09F0 0A10
         STW     D0818            ; 09F1 7018
         CAX                      ; 09F2 0130
         LDW     *0               ; 09F3 8800
         ORI     D0821            ; 09F4 C021
         STW     D0821            ; 09F5 7021
         LDW     *1               ; 09F6 8801
         JSX     S08E9            ; 09F7 20E9
         DATA    X'8200'          ; 09F8 8200  argument 1 of 09F7; LDW X'200'
J09F9    JMP     J09FD            ; 09F9 11FD
J09FA    JSX     S08F4            ; 09FA 20F4
         DATA    X'8040'          ; 09FB 8040  argument 1 of 09FA; LDW X'40'
J09FC    SLL     1                ; 09FC 0A11
J09FD    STW     D081F            ; 09FD 701F
         JMP     J0A34            ; 09FE 1234
D09FF    DATA    X'0000'          ; 09FF 0000  written by 093B 0B3D 0B4C 0B62 ...; read by 0940 0B55
J0A00    JSX     S0908            ; 0A00 2108
J0A01    JMP     J0A03            ; 0A01 1203
J0A02    JMP     J0A31            ; 0A02 1231
J0A03    LDW     D089E            ; 0A03 809E
         AND     D09ED            ; 0A04 E1ED
         STW     D089E            ; 0A05 709E
         LDW     D081F            ; 0A06 801F
         SLA     3                ; 0A07 0913
         SNO                      ; 0A08 08A0
         JSX     S0AC6            ; 0A09 22C6  not run
         ADD     D081F            ; 0A0A A01F  argument 1 of 0A09; run when a skip passes over the JSX
         SNO                      ; 0A0B 08A0  argument 2 of 0A09; run when a skip passes over the JSX
         JSX     S0AC6            ; 0A0C 22C6  not run
         ADD     D081F            ; 0A0D A01F  argument 1 of 0A0C; run when a skip passes over the JSX
         SNO                      ; 0A0E 08A0  argument 2 of 0A0C; run when a skip passes over the JSX
         JSX     S0AC6            ; 0A0F 22C6  not run
         ADD     D089E            ; 0A10 A09E  argument 1 of 0A0F; run when a skip passes over the JSX
         SNO                      ; 0A11 08A0  argument 2 of 0A0F; run when a skip passes over the JSX
         JSX     S0AC6            ; 0A12 22C6  not run
         STW     D081F            ; 0A13 701F  argument 1 of 0A12; run when a skip passes over the JSX
         JSX     S08D3            ; 0A14 20D3  argument 2 of 0A12; run when a skip passes over the JSX
         JMP     J0A00            ; 0A15 1200
J0A16    JSX     S08D3            ; 0A16 20D3
         LDW     D089E            ; 0A17 809E
         STW     D081F            ; 0A18 701F
         JSX     S08D3            ; 0A19 20D3
         LDW     D089E            ; 0A1A 809E
         CLB     X'A7'            ; 0A1B 07A7
         SNE                      ; 0A1C 0870
         JMP     J09D8            ; 0A1D 11D8
         LDW     D081F            ; 0A1E 801F
         SLL     8                ; 0A1F 0A18
         ORI     D089E            ; 0A20 C09E
         STW     D081F            ; 0A21 701F
         JSX     S08D3            ; 0A22 20D3
         JMP     J09D8            ; 0A23 11D8
J0A24    JSX     S08D3            ; 0A24 20D3
         LDW     D0C6D            ; 0A25 846D
         JSX     S08F4            ; 0A26 20F4
         DATA    X'8040'          ; 0A27 8040  argument 1 of 0A26; LDW X'40'
         SLL     1                ; 0A28 0A11  not run
J0A29    STW     D081F            ; 0A29 701F
         CLR                      ; 0A2A 0100
         CMW     D0824            ; 0A2B F024
         LDW     D081B            ; 0A2C 801B
         SEQ                      ; 0A2D 0860
         LDW     D081A            ; 0A2E 801A
         ORI     D0814            ; 0A2F C014
         JMP     J0A32            ; 0A30 1232
J0A31    LDW     D081A            ; 0A31 801A
J0A32    ORI     D0821            ; 0A32 C021
         STW     D0821            ; 0A33 7021
J0A34    LDX     D09B9            ; 0A34 91B9
         JSX     *0               ; 0A35 2800
S0A36    STX     D0A39            ; 0A36 6239
         LDW     D0819            ; 0A37 8019
         JMP     J0A3C            ; 0A38 123C
D0A39    DATA    X'0000'          ; 0A39 0000  written by 0A36 0A3A; read by 0A52
S0A3A    STX     D0A39            ; 0A3A 6239
         CLR                      ; 0A3B 0100
J0A3C    STW     D0821            ; 0A3C 7021
         LDW     D089E            ; 0A3D 809E
         CLB     X'AF'            ; 0A3E 07AF
         SEQ                      ; 0A3F 0860
         JMP     J0A44            ; 0A40 1244
         JSX     S08D3            ; 0A41 20D3
         LDW     D0819            ; 0A42 8019
         STW     D0821            ; 0A43 7021
J0A44    LDW     D089E            ; 0A44 809E
         CLB     X'AD'            ; 0A45 07AD
         SNE                      ; 0A46 0870
         JMP     J0A61            ; 0A47 1261
         JSX     S09BA            ; 0A48 21BA
J0A49    LDW     D089E            ; 0A49 809E
         LDX     D0A5F            ; 0A4A 925F
         CLB     X'AB'            ; 0A4B 07AB
         SNE                      ; 0A4C 0870
         JMP     J0A54            ; 0A4D 1254
         LDX     D0A60            ; 0A4E 9260
         CLB     X'AD'            ; 0A4F 07AD
         SNE                      ; 0A50 0870
         JMP     J0A54            ; 0A51 1254
         LDX     D0A39            ; 0A52 9239
         JSX     *0               ; 0A53 2800
J0A54    STX     D081E            ; 0A54 601E
         LDW     D081F            ; 0A55 801F
         STW     D0820            ; 0A56 7020
         LDW     D0821            ; 0A57 8021
         STW     D0822            ; 0A58 7022
         AND     D0819            ; 0A59 E019
         STW     D0821            ; 0A5A 7021
         JSX     S08D3            ; 0A5B 20D3
         JSX     S09BA            ; 0A5C 21BA
         LDX     D081E            ; 0A5D 901E
         JMP     *0               ; 0A5E 1800
D0A5F    DATA    X'0A6C'          ; 0A5F 0A6C  read by 0A4A
D0A60    DATA    X'0A7F'          ; 0A60 0A7F  read by 0A4E
J0A61    JSX     S08D3            ; 0A61 20D3
         JSX     S09BA            ; 0A62 21BA
         LDW     D081F            ; 0A63 801F
         CMP                      ; 0A64 0110
         STW     D081F            ; 0A65 701F
         SAM                      ; 0A66 0820
         JMP     J0A49            ; 0A67 1249  not run
         JSX     S08F4            ; 0A68 20F4
         DATA    X'8002'          ; 0A69 8002  argument 1 of 0A68; LDW 2
J0A6A    JMP     J0A49            ; 0A6A 1249
         DATA    X'11CE'          ; 0A6B 11CE  JMP X'1CE'
J0A6C    JSX     S0A8E            ; 0A6C 228E
J0A6D    CLB     3                ; 0A6D 0703
         SNE                      ; 0A6E 0870
         JMP     J0A74            ; 0A6F 1274
         CLB     4                ; 0A70 0704
         SNE                      ; 0A71 0870
         JMP     J0A7D            ; 0A72 127D
J0A73    JSX     S09CE            ; 0A73 21CE
J0A74    LDW     D081B            ; 0A74 801B
J0A75    ORI     D0821            ; 0A75 C021
         STW     D0821            ; 0A76 7021
         LDW     D081F            ; 0A77 801F
         ADD     D0820            ; 0A78 A020
         SNO                      ; 0A79 08A0
         JSX     S0AC6            ; 0A7A 22C6  not run
         STW     D081F            ; 0A7B 701F  argument 1 of 0A7A; run when a skip passes over the JSX
         JMP     J0A49            ; 0A7C 1249  argument 2 of 0A7A; run when a skip passes over the JSX
J0A7D    LDW     D081A            ; 0A7D 801A
         JMP     J0A75            ; 0A7E 1275
J0A7F    LDW     D081F            ; 0A7F 801F
         CMP                      ; 0A80 0110
         STW     D081F            ; 0A81 701F
         JSX     S0A8E            ; 0A82 228E
J0A83    CLB     4                ; 0A83 0704
         SNE                      ; 0A84 0870
         JMP     J0A7D            ; 0A85 127D
         CLB     2                ; 0A86 0702
         SNE                      ; 0A87 0870
         JMP     J0A7D            ; 0A88 127D
         LDW     D0820            ; 0A89 8020
         ADD     D081F            ; 0A8A A01F
         SAM                      ; 0A8B 0820
         JMP     J0A74            ; 0A8C 1274
         JMP     J0A73            ; 0A8D 1273  not run
S0A8E    LDW     D0821            ; 0A8E 8021
         ADD     D0822            ; 0A8F A022
         AND     D0A98            ; 0A90 E298
         CLB     4                ; 0A91 0704
         SLE                      ; 0A92 0890
         JMP     J0A73            ; 0A93 1273
         CLB     1                ; 0A94 0701
         SLE                      ; 0A95 0890
         JMP     *0               ; 0A96 1800
         JMP     J0A73            ; 0A97 1273  not run
D0A98    DATA    X'001F'          ; 0A98 001F  read by 0A90
         JSX     S0B0D            ; 0A99 230D
         JSX     S0A3A            ; 0A9A 223A
J0A9B    JSX     S0AA5            ; 0A9B 22A5
         LDW     D089E            ; 0A9C 809E
         CLB     X'AC'            ; 0A9D 07AC
         SEQ                      ; 0A9E 0860
         JMP     J0C79            ; 0A9F 1479
         JSX     S08D3            ; 0AA0 20D3
         JSX     S0A3A            ; 0AA1 223A
         JMP     J0A9B            ; 0AA2 129B
D0AA3    DATA    X'0000'          ; 0AA3 0000  written by 0C7A 0CB9; read by 0AB3 0ABA
D0AA4    DATA    X'0000'          ; 0AA4 0000  written by 0AA5; read by 0AB7
S0AA5    STX     D0AA4            ; 0AA5 62A4
         JSX     S0EB4            ; 0AA6 26B4
         JSX     S08F4            ; 0AA7 20F4
         DATA    X'8004'          ; 0AA8 8004  argument 1 of 0AA7; LDW 4
J0AA9    JMP     J0AB9            ; 0AA9 12B9
J0AAA    LDW     D080A            ; 0AAA 800A
         JSX     S08F4            ; 0AAB 20F4
         DATA    X'8040'          ; 0AAC 8040  argument 1 of 0AAB; LDW X'40'
J0AAD    LDW     D080B            ; 0AAD 800B
J0AAE    JSX     S08F4            ; 0AAE 20F4
         DATA    X'8002'          ; 0AAF 8002  argument 1 of 0AAE; LDW 2
J0AB0    LDW     D0804            ; 0AB0 8004
J0AB1    STW     D0823            ; 0AB1 7023
         LDW     D081F            ; 0AB2 801F
         ORI     D0AA3            ; 0AB3 C2A3
J0AB4    STW     D081F            ; 0AB4 701F
         JSX     S0AEA            ; 0AB5 22EA
         JSX     S0C56            ; 0AB6 2456
         LDX     D0AA4            ; 0AB7 92A4
         JSX     *0               ; 0AB8 2800
J0AB9    JSX     S0ACD            ; 0AB9 22CD
         LDW     D0AA3            ; 0ABA 82A3
         SRL     4                ; 0ABB 0A04
         ORI     D0ABF            ; 0ABC C2BF
         ORI     D081F            ; 0ABD C01F
         JMP     J0AB4            ; 0ABE 12B4
D0ABF    DATA    X'3000'          ; 0ABF 3000  STB 0; read by 0ABC
         JSX     S0D16            ; 0AC0 2516
         LDW     D081F            ; 0AC1 801F
         SLL     8                ; 0AC2 0A18
         STW     D0949            ; 0AC3 7149
         JSX     S08D3            ; 0AC4 20D3
         JMP     J0D0F            ; 0AC5 150F
S0AC6    LLB     X'CD'            ; 0AC6 06CD
         JMP     J0C63            ; 0AC7 1463
S0AC8    LLB     X'C2'            ; 0AC8 06C2  not run
         JMP     J0C63            ; 0AC9 1463  not run
S0ACA    LLB     X'D6'            ; 0ACA 06D6  not run
         JMP     J0C63            ; 0ACB 1463  not run
D0ACC    DATA    X'0000'          ; 0ACC 0000  written by 0ACD; read by 0AE0
S0ACD    STX     D0ACC            ; 0ACD 62CC
         JSX     S08E9            ; 0ACE 20E9
         DATA    X'8004'          ; 0ACF 8004  argument 1 of 0ACE; LDW 4
J0AD0    JMP     J0AE0            ; 0AD0 12E0
J0AD1    JSX     S08F4            ; 0AD1 20F4
         DATA    X'8040'          ; 0AD2 8040  argument 1 of 0AD1; LDW X'40'
J0AD3    JMP     S09CE            ; 0AD3 11CE
J0AD4    LDW     D0AE2            ; 0AD4 82E2
         STW     D0EC1            ; 0AD5 76C1
         LDX     D0818            ; 0AD6 9018
         LDW     D0C6D            ; 0AD7 846D
         STW     *1               ; 0AD8 7801
         LDW     D081F            ; 0AD9 801F
         LDX     D0804            ; 0ADA 9004
         SAM                      ; 0ADB 0820
         LDX     D0808            ; 0ADC 9008
         STX     D0823            ; 0ADD 6023
         AND     D0CBC            ; 0ADE E4BC
         STW     D081F            ; 0ADF 701F
J0AE0    LDX     D0ACC            ; 0AE0 92CC
         JSX     *0               ; 0AE1 2800
D0AE2    DATA    X'AAA0'          ; 0AE2 AAA0  '* '; read by 0AD4
S0AE3    SML     1                ; 0AE3 0081
         LDW     D06D3            ; 0AE4 86D3
         CLB     X'EB'            ; 0AE5 07EB
         SEQ                      ; 0AE6 0860
         JMP     *0               ; 0AE7 1800
         JMP     *1               ; 0AE8 1801
D0AE9    DATA    X'0000'          ; 0AE9 0000  written by 0AEA; read by 0AFB
S0AEA    STX     D0AE9            ; 0AEA 62E9
         JSX     S0ECA            ; 0AEB 26CA
         LDW     D0828            ; 0AEC 8028
         SAZ                      ; 0AED 0800
         JMP     J0AFB            ; 0AEE 12FB
         JSX     S0AFD            ; 0AEF 22FD
J0AF0    LDW     D0823            ; 0AF0 8023
         JSX     S0E25            ; 0AF1 2625
         SS0                      ; 0AF2 08C0
         JMP     J0AF9            ; 0AF3 12F9
         JSX     S0AE3            ; 0AF4 22E3
J0AF5    JMP     J0AF9            ; 0AF5 12F9
J0AF6    JSX     S0829            ; 0AF6 2029
         DATA    X'0EC0'          ; 0AF7 0EC0  argument 1 of 0AF6
         DATA    X'800F'          ; 0AF8 800F  argument 2 of 0AF6; LDW X'F'
J0AF9    LDW     D0EC4            ; 0AF9 86C4
         STW     D0EC1            ; 0AFA 76C1
J0AFB    LDX     D0AE9            ; 0AFB 92E9
         JSX     *0               ; 0AFC 2800
S0AFD    LDW     D0C6D            ; 0AFD 846D
         ADD     D0824            ; 0AFE A024
         SAZ                      ; 0AFF 0800
         JMP     *0               ; 0B00 1800
         STX     D0C0D            ; 0B01 640D
         LDW     D0E47            ; 0B02 8647
         SRL     1                ; 0B03 0A01
         STW     D0DAC            ; 0B04 75AC
         JSX     S0E25            ; 0B05 2625
         LDW     D0E47            ; 0B06 8647
         SRL     1                ; 0B07 0A01
         ADD     D0807            ; 0B08 A007
         STW     D0B6B            ; 0B09 736B
         LDX     D0C0D            ; 0B0A 940D
         JMP     *0               ; 0B0B 1800
D0B0C    DATA    X'0000'          ; 0B0C 0000  written by 0B0D 0C4D; read by 0B11
S0B0D    STX     D0B0C            ; 0B0D 630C
         LDW     D08A0            ; 0B0E 80A0
         SAZ                      ; 0B0F 0800
         JMP     J0B13            ; 0B10 1313
J0B11    LDX     D0B0C            ; 0B11 930C
         JSX     *0               ; 0B12 2800
J0B13    JSX     S0C50            ; 0B13 2450
J0B14    LDW     D0C6D            ; 0B14 846D
         STW     D081F            ; 0B15 701F
         JSX     S098F            ; 0B16 218F
         JMP     J0B28            ; 0B17 1328
         JSX     S0B1B            ; 0B18 231B
         JMP     J0B11            ; 0B19 1311
D0B1A    DATA    X'0000'          ; 0B1A 0000  written by 0B1B; read by 0B26
S0B1B    STX     D0B1A            ; 0B1B 631A
         CAX                      ; 0B1C 0130
         LDW     D0824            ; 0B1D 8024
         CMW     D0815            ; 0B1E F015
         LDW     D081B            ; 0B1F 801B
         SEQ                      ; 0B20 0860
         LDW     D081A            ; 0B21 801A
         ORI     D0814            ; 0B22 C014
         STW     *0               ; 0B23 7800
         LDW     D081F            ; 0B24 801F
         STW     *1               ; 0B25 7801
         LDX     D0B1A            ; 0B26 931A
         JSX     *0               ; 0B27 2800
J0B28    CAX                      ; 0B28 0130
         LDW     *0               ; 0B29 8800
         ORE     D081C            ; 0B2A D01C
         SAZ                      ; 0B2B 0800
         JMP     J0B6E            ; 0B2C 136E
         LDW     *1               ; 0B2D 8801
         STW     D0B6A            ; 0B2E 736A
         CXA                      ; 0B2F 0140
         JSX     S0B1B            ; 0B30 231B
         AND     D0CBC            ; 0B31 E4BC
         STW     D0B6D            ; 0B32 736D
         LDW     D0828            ; 0B33 8028
         SAZ                      ; 0B34 0800
         JMP     J0B11            ; 0B35 1311
J0B36    LDW     D0B6A            ; 0B36 836A
         AND     D0CBC            ; 0B37 E4BC
         CMW     D0CBC            ; 0B38 F4BC
         SNE                      ; 0B39 0870
         JMP     J0B11            ; 0B3A 1311
         CAX                      ; 0B3B 0130
         LDW     D0808            ; 0B3C 8008
         STW     D09FF            ; 0B3D 71FF
         LDW     D0824            ; 0B3E 8024
         SAZ                      ; 0B3F 0800
         JMP     J0B42            ; 0B40 1342
         SLLD    1                ; 0B41 0A31
J0B42    CXA                      ; 0B42 0140
         ADD     D0B6B            ; 0B43 A36B
         SUB     D0813            ; 0B44 B013
         CAX                      ; 0B45 0130
         LDW     *0               ; 0B46 8800
         STW     D0B6A            ; 0B47 736A
         SRL     11               ; 0B48 0A0B
         SAZ                      ; 0B49 0800
         JMP     J0B58            ; 0B4A 1358
         LDW     D0804            ; 0B4B 8004
         STW     D09FF            ; 0B4C 71FF
         LDW     D081F            ; 0B4D 801F
         SRL     10               ; 0B4E 0A0A
         ORI     D0B6C            ; 0B4F C36C
J0B50    STW     *0               ; 0B50 7800
         LDW     D0824            ; 0B51 8024
         SAZ                      ; 0B52 0800
         JMP     J0B36            ; 0B53 1336
         DXS     1                ; 0B54 0501
         LDW     D09FF            ; 0B55 81FF
         STW     *0               ; 0B56 7800
         JMP     J0B36            ; 0B57 1336
J0B58    CLB     6                ; 0B58 0706
         SNE                      ; 0B59 0870
         JMP     J0B66            ; 0B5A 1366
         CLB     7                ; 0B5B 0707
         SNE                      ; 0B5C 0870
         JMP     J0B61            ; 0B5D 1361
         SRC     5                ; 0B5E 0A45
         ORI     D0B6D            ; 0B5F C36D
         JMP     J0B50            ; 0B60 1350
J0B61    LDW     D080A            ; 0B61 800A
         STW     D09FF            ; 0B62 71FF
         LDW     D081F            ; 0B63 801F
         ORI     D0816            ; 0B64 C016
         JMP     J0B50            ; 0B65 1350
J0B66    LDW     D080A            ; 0B66 800A
         STW     D09FF            ; 0B67 71FF
         LDW     D081F            ; 0B68 801F
         JMP     J0B50            ; 0B69 1350
D0B6A    DATA    X'0000'          ; 0B6A 0000  written by 0B2E 0B47; read by 0B36
D0B6B    DATA    X'002C'          ; 0B6B 002C  written by 087F 0B09; read by 0B43
D0B6C    DATA    X'0080'          ; 0B6C 0080  read by 0B4F
D0B6D    DATA    X'0000'          ; 0B6D 0000  written by 0B32; read by 0B5F
J0B6E    LLB     X'C4'            ; 0B6E 06C4
         JMP     J0C63            ; 0B6F 1463
         LDW     D0824            ; 0B70 8024
         ORI     D0C6D            ; 0B71 C46D
         SAZ                      ; 0B72 0800
         JMP     J08A9            ; 0B73 10A9
J0B74    JSX     S08A4            ; 0B74 20A4  not run
         LDW     D08CF            ; 0B75 80CF  not run
         STW     D081F            ; 0B76 701F  not run
         LDW     D0807            ; 0B77 8007  not run
         JSX     S0E25            ; 0B78 2625  not run
         LDW     D08D0            ; 0B79 80D0  not run
         JSX     S0E31            ; 0B7A 2631  not run
         LDW     D0EC4            ; 0B7B 86C4  not run
         JSX     S0E31            ; 0B7C 2631  not run
         LDW     D0EC4            ; 0B7D 86C4  not run
         JSX     S0E31            ; 0B7E 2631  not run
         LDW     D089E            ; 0B7F 809E  not run
         CLB     X'AC'            ; 0B80 07AC  not run
         SEQ                      ; 0B81 0860  not run
         JMP     J0C79            ; 0B82 1479  not run
         JSX     S08D3            ; 0B83 20D3  not run
         JMP     J0B74            ; 0B84 1374  not run
         LDW     D081D            ; 0B85 801D
         JMP     J0B88            ; 0B86 1388
         LDW     D081C            ; 0B87 801C
J0B88    STW     D0B96            ; 0B88 7396
J0B89    JSX     S08A4            ; 0B89 20A4
         JSX     S098F            ; 0B8A 218F
         SLL     0                ; 0B8B 0A10
         CAX                      ; 0B8C 0130
         LDW     *0               ; 0B8D 8800
         ORI     D0B96            ; 0B8E C396
         STW     *0               ; 0B8F 7800
         LDW     D089E            ; 0B90 809E
         CLB     X'AC'            ; 0B91 07AC
         SEQ                      ; 0B92 0860
         JMP     J0C79            ; 0B93 1479
         JSX     S08D3            ; 0B94 20D3  not run
         JMP     J0B89            ; 0B95 1389  not run
D0B96    DATA    X'0000'          ; 0B96 0000  written by 0B88 0BAA 0D74; read by 0B8E 0BCC 0BCD
D0B97    DATA    X'0000'          ; 0B97 0000  written by 0B98; read by 0B9E
S0B98    STX     D0B97            ; 0B98 6397
         LDW     D0806            ; 0B99 8006
         STW     D0823            ; 0B9A 7023
         LDW     D081D            ; 0B9B 801D
         ORI     D0814            ; 0B9C C014
         JSX     S0BA9            ; 0B9D 23A9
         LDX     D0B97            ; 0B9E 9397
         JSX     *0               ; 0B9F 2800
D0BA0    DATA    X'0000'          ; 0BA0 0000  written by 0BA1; read by 0BA6
S0BA1    STX     D0BA0            ; 0BA1 63A0
         LDW     D0805            ; 0BA2 8005
         STW     D0823            ; 0BA3 7023
         LDW     D081C            ; 0BA4 801C
         JSX     S0BA9            ; 0BA5 23A9
         LDX     D0BA0            ; 0BA6 93A0
         JSX     *0               ; 0BA7 2800
D0BA8    DATA    X'0000'          ; 0BA8 0000  written by 0BA9; read by 0BC3
S0BA9    STX     D0BA8            ; 0BA9 63A8
         STW     D0B96            ; 0BAA 7396
         LDW     D0824            ; 0BAB 8024
         SAZ                      ; 0BAC 0800
         JMP     J0BC3            ; 0BAD 13C3  not run
         LDW     D0826            ; 0BAE 8026
         STW     D0BE0            ; 0BAF 73E0
J0BB0    JSX     S0BC7            ; 0BB0 23C7
         JMP     J0BC3            ; 0BB1 13C3
         LDX     D0BE0            ; 0BB2 93E0
         LDW     *0               ; 0BB3 8800
         STW     D081F            ; 0BB4 701F
         LDW     D0823            ; 0BB5 8023
         JSX     S0E25            ; 0BB6 2625
         LDX     D0BE0            ; 0BB7 93E0
         LDW     *1               ; 0BB8 8801
         JSX     S0E31            ; 0BB9 2631
         LDW     D0EC4            ; 0BBA 86C4
         JSX     S0E31            ; 0BBB 2631
         LDW     D0EC4            ; 0BBC 86C4
         JSX     S0E31            ; 0BBD 2631
         LDX     D0BE0            ; 0BBE 93E0
         LDW     *3               ; 0BBF 8803
         STW     D081F            ; 0BC0 701F
         JSX     S0E31            ; 0BC1 2631
         JMP     J0BB0            ; 0BC2 13B0
J0BC3    LDX     D0BA8            ; 0BC3 93A8
         JSX     *0               ; 0BC4 2800
         DATA    X'0000'          ; 0BC5 0000
D0BC6    DATA    X'0000'          ; 0BC6 0000  written by 0BC7; read by 0BD0 0BD2
S0BC7    STX     D0BC6            ; 0BC7 63C6
J0BC8    JSX     S0BD5            ; 0BC8 23D5
         JMP     J0BD2            ; 0BC9 13D2
         CAX                      ; 0BCA 0130
         LDW     *2               ; 0BCB 8802
         AND     D0B96            ; 0BCC E396
         ORE     D0B96            ; 0BCD D396
         SAZ                      ; 0BCE 0800
         JMP     J0BC8            ; 0BCF 13C8
         LDX     D0BC6            ; 0BD0 93C6
         JSX     *1               ; 0BD1 2801
J0BD2    LDX     D0BC6            ; 0BD2 93C6
         JSX     *0               ; 0BD3 2800
D0BD4    DATA    X'0000'          ; 0BD4 0000  written by 0BD5; read by 0BDC 0BDE
S0BD5    STX     D0BD4            ; 0BD5 63D4
         LDW     D0BE0            ; 0BD6 83E0
         SUB     D080D            ; 0BD7 B00D
         STW     D0BE0            ; 0BD8 73E0
         CMW     D0803            ; 0BD9 F003
         SNE                      ; 0BDA 0870
         JMP     J0BDE            ; 0BDB 13DE
         LDX     D0BD4            ; 0BDC 93D4
         JSX     *1               ; 0BDD 2801
J0BDE    LDX     D0BD4            ; 0BDE 93D4
         JSX     *0               ; 0BDF 2800
D0BE0    DATA    X'0000'          ; 0BE0 0000  written by 05E3 0BAF 0BD8 0D72; read by 0BB2 0BB7 0BBE 0BD6 ...
         LDX     D0807            ; 0BE1 9007
         JMP     J0BE4            ; 0BE2 13E4
         LDX     D0815            ; 0BE3 9015
J0BE4    STX     D0C0C            ; 0BE4 640C
         JSX     S0A3A            ; 0BE5 223A
         LDW     D089E            ; 0BE6 809E
         LDX     D0C09            ; 0BE7 9409
         CLB     X'BD'            ; 0BE8 07BD
         SNE                      ; 0BE9 0870
         JMP     J0BF5            ; 0BEA 13F5  not run
         LDX     D0C0A            ; 0BEB 940A
         CLB     X'BC'            ; 0BEC 07BC
         SNE                      ; 0BED 0870
         JMP     J0BF5            ; 0BEE 13F5  not run
         LDX     D0C0B            ; 0BEF 940B
         CLB     X'BE'            ; 0BF0 07BE
         SNE                      ; 0BF1 0870
         JMP     J0BF5            ; 0BF2 13F5  not run
         LLB     X'C3'            ; 0BF3 06C3
         JMP     J0C63            ; 0BF4 1463
J0BF5    STX     J0C06            ; 0BF5 6406  not run
         STX     D0C00            ; 0BF6 6400  not run
         LDW     D081F            ; 0BF7 801F  not run
         STW     D0C0D            ; 0BF8 740D  not run
         JSX     S08D3            ; 0BF9 20D3  not run
         JSX     S0A3A            ; 0BFA 223A  not run
         LDW     D0C0D            ; 0BFB 840D  not run
         CMW     D081F            ; 0BFC F01F  not run
         LDW     D0C0C            ; 0BFD 840C  not run
         SAZ                      ; 0BFE 0800  not run
         JMP     J0C06            ; 0BFF 1406  not run
D0C00    DATA    X'0000'          ; 0C00 0000  written by 0BF6
         DATA    X'105D'          ; 0C01 105D  JMP X'5D'
         DATA    X'8025'          ; 0C02 8025  LDW X'25'
         DATA    X'A007'          ; 0C03 A007  ADD 7
         DATA    X'7025'          ; 0C04 7025  STW X'25'
         DATA    X'105D'          ; 0C05 105D  JMP X'5D'
J0C06    DATA    X'0000'          ; 0C06 0000  written by 0BF5
         DATA    X'1402'          ; 0C07 1402  JMP X'402'
         DATA    X'105D'          ; 0C08 105D  JMP X'5D'
D0C09    DATA    X'0860'          ; 0C09 0860  read by 0BE7
D0C0A    DATA    X'0840'          ; 0C0A 0840  read by 0BEB
D0C0B    DATA    X'0880'          ; 0C0B 0880  read by 0BEF
D0C0C    DATA    X'0000'          ; 0C0C 0000  written by 0BE4; read by 0BFD
D0C0D    DATA    X'0000'          ; 0C0D 0000  written by 0B01 0BF8; read by 0B0A 0BFB
         JSX     S0A3A            ; 0C0E 223A
         JSX     S08E9            ; 0C0F 20E9
         DATA    X'8002'          ; 0C10 8002  argument 1 of 0C0F; LDW 2
         JMP     S09CE            ; 0C11 11CE  not run
J0C12    LDW     D081F            ; 0C12 801F
         SAP                      ; 0C13 0810
         JMP     S09CE            ; 0C14 11CE  not run
         STW     D0DAD            ; 0C15 75AD
         STW     D0C62            ; 0C16 7462
         JSX     S0AFD            ; 0C17 22FD
J0C18    JSX     S0B0D            ; 0C18 230D
         JSX     S0C56            ; 0C19 2456
         CLR                      ; 0C1A 0100
         STW     D081F            ; 0C1B 701F
J0C1C    LDX     D0DAD            ; 0C1C 95AD
         DXS     1                ; 0C1D 0501
         JMP     J0C20            ; 0C1E 1420
         JMP     J0C79            ; 0C1F 1479
J0C20    STX     D0DAD            ; 0C20 65AD
         LDW     D0804            ; 0C21 8004
         JSX     S0E25            ; 0C22 2625
         JMP     J0C1C            ; 0C23 141C
         LDW     D0C6D            ; 0C24 846D
         SAZ                      ; 0C25 0800
         JMP     J0955            ; 0C26 1155  not run
         LDW     D0824            ; 0C27 8024
         SAZ                      ; 0C28 0800
         JMP     J0C2B            ; 0C29 142B
         JMP     J0955            ; 0C2A 1155
J0C2B    JSX     S0A3A            ; 0C2B 223A
         JSX     S08E9            ; 0C2C 20E9
         DATA    X'8002'          ; 0C2D 8002  argument 1 of 0C2C; LDW 2
         JMP     S09CE            ; 0C2E 11CE  not run
J0C2F    LDW     D081F            ; 0C2F 801F
         SAP                      ; 0C30 0810
         JMP     S09CE            ; 0C31 11CE  not run
         STW     D0DAA            ; 0C32 75AA
         STW     D0C6D            ; 0C33 746D
         AND     D0CBC            ; 0C34 E4BC
         STW     D0813            ; 0C35 7013
         JMP     J0C79            ; 0C36 1479
         LDW     D08A0            ; 0C37 80A0
         SAZ                      ; 0C38 0800
         JMP     J0C3C            ; 0C39 143C
J0C3A    LLB     X'C5'            ; 0C3A 06C5  not run
         JMP     J0C63            ; 0C3B 1463  not run
J0C3C    STX     D0C86            ; 0C3C 6486
         JSX     S0A3A            ; 0C3D 223A
         JSX     S08E9            ; 0C3E 20E9
         DATA    X'8003'          ; 0C3F 8003  argument 1 of 0C3E; LDW 3
         JMP     S09CE            ; 0C40 11CE  not run
J0C41    JSX     S0C50            ; 0C41 2450
J0C42    CLR                      ; 0C42 0100
         STW     D0C86            ; 0C43 7486
         JSX     S098F            ; 0C44 218F
         JMP     J0C4C            ; 0C45 144C
         CAX                      ; 0C46 0130
         LDW     D0821            ; 0C47 8021
         STW     *0               ; 0C48 7800
         LDW     D081F            ; 0C49 801F
         STW     *1               ; 0C4A 7801
         JMP     J0C79            ; 0C4B 1479
J0C4C    LDX     D0C4F            ; 0C4C 944F
         STX     D0B0C            ; 0C4D 630C
         JMP     J0B28            ; 0C4E 1328
D0C4F    DATA    X'0C79'          ; 0C4F 0C79  read by 0C4C
S0C50    LDW     D08A1            ; 0C50 80A1
         STW     D08CF            ; 0C51 70CF
         LDW     D08A2            ; 0C52 80A2
         STW     D08D0            ; 0C53 70D0
         JMP     *0               ; 0C54 1800
D0C55    DATA    X'0000'          ; 0C55 0000  written by 0C56; read by 0C60
S0C56    STX     D0C55            ; 0C56 6455
         LDW     D0C6D            ; 0C57 846D
         ADD     D0C62            ; 0C58 A462
         STW     D0C6D            ; 0C59 746D
         ORE     D0DAA            ; 0C5A D5AA
         SRL     11               ; 0C5B 0A0B
         SAZ                      ; 0C5C 0800
         JMP     S0ACA            ; 0C5D 12CA  not run
         LDW     D0807            ; 0C5E 8007
         STW     D0C62            ; 0C5F 7462
         LDX     D0C55            ; 0C60 9455
         JSX     *0               ; 0C61 2800
D0C62    DATA    X'0001'          ; 0C62 0001  written by 0C16 0C5F; read by 0C58
J0C63    STB     D0C6C+1          ; 0C63 30D9
         JSX     S0AE3            ; 0C64 22E3
         JMP     J0C6A            ; 0C65 146A  not run
J0C66    JSX     S0829            ; 0C66 2029
         DATA    X'0C6C'          ; 0C67 0C6C  argument 1 of 0C66
         DATA    X'8002'          ; 0C68 8002  argument 2 of 0C66; LDW 2
J0C69    JMP     J0860            ; 0C69 1060
J0C6A    SML     1                ; 0C6A 0081  not run
         JMP     J07D8            ; 0C6B 17D8  not run
D0C6C    DATA    X'0000'          ; 0C6C 0000  written by 0C63; read by 07D9
D0C6D    DATA    X'0000'          ; 0C6D 0000  written by 05C6 05CC 086D 0C33 ...; read by 05C1 05EA 0601 0A25 ...
D0C6E    DATA    X'0000'          ; 0C6E 0000  written by 0C6F; read by 0C77
S0C6F    STX     D0C6E            ; 0C6F 646E
         LDW     D0804            ; 0C70 8004
         STW     D0823            ; 0C71 7023
         LDW     D0949            ; 0C72 8149
         STW     D081F            ; 0C73 701F
         JSX     S0EB4            ; 0C74 26B4
         JSX     S0AEA            ; 0C75 22EA
         JSX     S0C56            ; 0C76 2456
         LDX     D0C6E            ; 0C77 946E
         JSX     *0               ; 0C78 2800
J0C79    CLR                      ; 0C79 0100
         STW     D0AA3            ; 0C7A 72A3
         LDW     D0EC4            ; 0C7B 86C4
         STW     D0EC2            ; 0C7C 76C2
         STW     D0EC3            ; 0C7D 76C3
         JMP     J085D            ; 0C7E 105D
D0C7F    DATA    X'0000'          ; 0C7F 0000  written by 0C80; read by 0C84
S0C80    STX     D0C7F            ; 0C80 647F
         JSX     S08E9            ; 0C81 20E9
         DATA    X'8002'          ; 0C82 8002  argument 1 of 0C81; LDW 2
         JMP     S09CE            ; 0C83 11CE  not run
J0C84    LDX     D0C7F            ; 0C84 947F
         JSX     *0               ; 0C85 2800
D0C86    DATA    X'0000'          ; 0C86 0000  written by 0C3C 0C43; read by 09A4
         JSX     S0CC2            ; 0C87 24C2
         JSX     S0A3A            ; 0C88 223A
         STX     D0CDB            ; 0C89 64DB
         JSX     S08F4            ; 0C8A 20F4
         DATA    X'8040'          ; 0C8B 8040  argument 1 of 0C8A; LDW X'40'
         JSX     S0AC8            ; 0C8C 22C8  not run
J0C8D    LDW     D0808            ; 0C8D 8008
         STW     D0823            ; 0C8E 7023
         LDW     D0C6D            ; 0C8F 846D
J0C90    STW     D0CBD            ; 0C90 74BD
         JSX     S0ACD            ; 0C91 22CD
         JSX     S08F4            ; 0C92 20F4
         DATA    X'8004'          ; 0C93 8004  argument 1 of 0C92; LDW 4
J0C94    JMP     J0C9D            ; 0C94 149D
J0C95    JSX     S08E9            ; 0C95 20E9
         DATA    X'8201'          ; 0C96 8201  argument 1 of 0C95; LDW X'201'
J0C97    JMP     J0CBE            ; 0C97 14BE
J0C98    LDW     D081F            ; 0C98 801F
         ORE     D0CBD            ; 0C99 D4BD
         SRL     11               ; 0C9A 0A0B
D0C9B    SAZ                      ; 0C9B 0800  read by 0CC8
         JSX     S0ACA            ; 0C9C 22CA  not run
J0C9D    LDW     D081F            ; 0C9D 801F
         AND     D0CBC            ; 0C9E E4BC
         ORI     D0949            ; 0C9F C149
         STW     D081F            ; 0CA0 701F
         JSX     S0EB4            ; 0CA1 26B4
         JSX     S0AEA            ; 0CA2 22EA
         JSX     S0B0D            ; 0CA3 230D
         JSX     S0C56            ; 0CA4 2456
         LDW     D0CDB            ; 0CA5 84DB
         SAP                      ; 0CA6 0810
         JMP     J0C79            ; 0CA7 1479  not run
         LDW     D0DAD            ; 0CA8 85AD
         CLB     X'D3'            ; 0CA9 07D3
         SEQ                      ; 0CAA 0860
         JMP     J0C79            ; 0CAB 1479
         LDW     D089E            ; 0CAC 809E
         CLB     X'AC'            ; 0CAD 07AC
         SEQ                      ; 0CAE 0860
         JMP     J0C79            ; 0CAF 1479
J0CB0    JSX     S08D3            ; 0CB0 20D3
J0CB1    JSX     S0A3A            ; 0CB1 223A
         LDW     D089E            ; 0CB2 809E
         CLB     X'AC'            ; 0CB3 07AC
         SEQ                      ; 0CB4 0860
         JMP     J0CB8            ; 0CB5 14B8
         JSX     S0AA5            ; 0CB6 22A5  not run
         JMP     J0CB0            ; 0CB7 14B0  not run
J0CB8    LDW     D0816            ; 0CB8 8016
         STW     D0AA3            ; 0CB9 72A3
         JSX     S0AA5            ; 0CBA 22A5
         JMP     J0C79            ; 0CBB 1479
D0CBC    DATA    X'07FF'          ; 0CBC 07FF  read by 0965 0ADE 0B31 0B37 ...
D0CBD    DATA    X'0000'          ; 0CBD 0000  written by 0C90; read by 0C99
J0CBE    LDW     D0804            ; 0CBE 8004
         STW     D0823            ; 0CBF 7023
         JMP     J0C9D            ; 0CC0 149D
D0CC1    DATA    X'0000'          ; 0CC1 0000  written by 0CC2; read by 0CCC
S0CC2    STX     D0CC1            ; 0CC2 64C1
         LDW     D089E            ; 0CC3 809E
         CLB     X'AA'            ; 0CC4 07AA
         SEQ                      ; 0CC5 0860
         JMP     J0CCC            ; 0CC6 14CC
         LDW     D0949            ; 0CC7 8149
         ORI     D0C9B            ; 0CC8 C49B
         STW     D0949            ; 0CC9 7149
         JSX     S08D3            ; 0CCA 20D3
         JSX     S08E2            ; 0CCB 20E2
J0CCC    LDX     D0CC1            ; 0CCC 94C1
         JSX     *0               ; 0CCD 2800
         CLR                      ; 0CCE 0100
         STW     D0CDB            ; 0CCF 74DB
         JSX     S0CC2            ; 0CD0 24C2
         JSX     S0A36            ; 0CD1 2236
         LDW     D0809            ; 0CD2 8009
         STW     D0823            ; 0CD3 7023
         LDW     D0810            ; 0CD4 8010
         JSX     S08E9            ; 0CD5 20E9
         DATA    X'8002'          ; 0CD6 8002  argument 1 of 0CD5; LDW 2
J0CD7    STW     D0811            ; 0CD7 7011
J0CD8    LDW     D0C6D            ; 0CD8 846D
         SLL     1                ; 0CD9 0A11
         JMP     J0C90            ; 0CDA 1490
D0CDB    DATA    X'0000'          ; 0CDB 0000  written by 0C89 0CCF; read by 0CA5
J0CDC    JSX     S0A3A            ; 0CDC 223A
         JSX     S0C80            ; 0CDD 2480
J0CDE    LDW     D081F            ; 0CDE 801F
         SRL     4                ; 0CDF 0A04
         SAZ                      ; 0CE0 0800
         JSX     S0AC6            ; 0CE1 22C6  not run
         LDW     D081F            ; 0CE2 801F  argument 1 of 0CE1; run when a skip passes over the JSX
         ORI     D0949            ; 0CE3 C149  argument 2 of 0CE1; run when a skip passes over the JSX
         STW     D0949            ; 0CE4 7149
         JMP     J0D0C            ; 0CE5 150C
         LDW     D089F            ; 0CE6 809F
         CLB     X'A0'            ; 0CE7 07A0
         SEQ                      ; 0CE8 0860
         JMP     J0CDC            ; 0CE9 14DC
         LDW     D089E            ; 0CEA 809E
         CLB     X'C4'            ; 0CEB 07C4
         SNE                      ; 0CEC 0870
         JMP     J0D00            ; 0CED 1500  not run
         LDW     D0949            ; 0CEE 8149
         SRL     8                ; 0CEF 0A08
         CLB     X'A'             ; 0CF0 070A
         SEQ                      ; 0CF1 0860
         JMP     J0CDC            ; 0CF2 14DC
         LDW     D089E            ; 0CF3 809E
         CLB     X'CC'            ; 0CF4 07CC
         SNE                      ; 0CF5 0870
         JMP     J0D03            ; 0CF6 1503
         CLB     X'D2'            ; 0CF7 07D2
         SEQ                      ; 0CF8 0860
         JMP     J0CDC            ; 0CF9 14DC
         LDW     D0949            ; 0CFA 8149  not run
         ORI     D080B            ; 0CFB C00B  not run
J0CFC    STW     D0949            ; 0CFC 7149
         JSX     S08D3            ; 0CFD 20D3
         JSX     S08E2            ; 0CFE 20E2
         JMP     J0CDC            ; 0CFF 14DC
J0D00    LDW     D0949            ; 0D00 8149  not run
         ORI     D0D06            ; 0D01 C506  not run
         JMP     J0CFC            ; 0D02 14FC  not run
J0D03    LDW     D0949            ; 0D03 8149
         ORI     D0808            ; 0D04 C008
         JMP     J0CFC            ; 0D05 14FC
D0D06    DATA    X'0020'          ; 0D06 0020  read by 0D01
         CLR                      ; 0D07 0100
         STW     D081F            ; 0D08 701F
         JSX     S0908            ; 0D09 2108
         JSX     S0A3A            ; 0D0A 223A  not run
J0D0B    JMP     J0CDE            ; 0D0B 14DE
J0D0C    JSX     S0B0D            ; 0D0C 230D
         JSX     S0C6F            ; 0D0D 246F
         JMP     J0C79            ; 0D0E 1479
J0D0F    JSX     S0D16            ; 0D0F 2516
         LDW     D081F            ; 0D10 801F
         AND     D0D15            ; 0D11 E515
         ORI     D0949            ; 0D12 C149
         STW     D0949            ; 0D13 7149
         JMP     J0D0C            ; 0D14 150C
D0D15    DATA    X'00FF'          ; 0D15 00FF  read by 0D11
S0D16    STX     D0DAD            ; 0D16 65AD
         JSX     S0A3A            ; 0D17 223A
         JSX     S0C80            ; 0D18 2480
         LDW     D081F            ; 0D19 801F
         SAP                      ; 0D1A 0810
         CMP                      ; 0D1B 0110  not run
         LLB     0                ; 0D1C 0600
         SAZ                      ; 0D1D 0800
         JSX     S0AC6            ; 0D1E 22C6
         LDX     D0DAD            ; 0D1F 95AD  argument 1 of 0D1E; run when a skip passes over the JSX
         JSX     *0               ; 0D20 2800  argument 2 of 0D1E; run when a skip passes over the JSX
         JSX     S0A3A            ; 0D21 223A
         JSX     S0C80            ; 0D22 2480
         LDW     D081F            ; 0D23 801F
         SRL     4                ; 0D24 0A04
         SAZ                      ; 0D25 0800
         JSX     S0AC6            ; 0D26 22C6
         LDW     D081F            ; 0D27 801F  argument 1 of 0D26; run when a skip passes over the JSX
         AND     D09ED            ; 0D28 E1ED  argument 2 of 0D26; run when a skip passes over the JSX
         SLL     4                ; 0D29 0A14
         ORI     D0949            ; 0D2A C149
         STW     D0949            ; 0D2B 7149
         LDW     D089E            ; 0D2C 809E
         CLB     X'AC'            ; 0D2D 07AC
         SEQ                      ; 0D2E 0860
         JMP     S09CE            ; 0D2F 11CE  not run
         JSX     S08D3            ; 0D30 20D3
         JMP     J0CDC            ; 0D31 14DC
         DATA    X'230D'          ; 0D32 230D  JSX X'30D'
         DATA    X'8149'          ; 0D33 8149  LDW X'149'
         DATA    X'060F'          ; 0D34 060F
         DATA    X'7149'          ; 0D35 7149  STW X'149'
         DATA    X'246F'          ; 0D36 246F  JSX X'46F'
         DATA    X'223A'          ; 0D37 223A  JSX X'23A'
         DATA    X'0100'          ; 0D38 0100
         DATA    X'72A3'          ; 0D39 72A3  STW X'2A3'
         DATA    X'20F4'          ; 0D3A 20F4  JSX X'F4'
         DATA    X'8040'          ; 0D3B 8040  LDW X'40'
         DATA    X'12C8'          ; 0D3C 12C8  JMP X'2C8'
         DATA    X'22A5'          ; 0D3D 22A5  JSX X'2A5'
         DATA    X'1479'          ; 0D3E 1479  JMP X'479'
         DATA    X'2542'          ; 0D3F 2542  JSX X'542'
         DATA    X'1479'          ; 0D40 1479  JMP X'479'
D0D41    DATA    X'0000'          ; 0D41 0000  written by 0D42; read by 0D52
S0D42    STX     D0D41            ; 0D42 6541
         JSX     S0B0D            ; 0D43 230D
         JSX     S0A3A            ; 0D44 223A
         JSX     S08F4            ; 0D45 20F4
         DATA    X'8040'          ; 0D46 8040  argument 1 of 0D45; LDW X'40'
         JMP     S0AC8            ; 0D47 12C8  not run
J0D48    JSX     S08F4            ; 0D48 20F4
         DATA    X'8002'          ; 0D49 8002  argument 1 of 0D48; LDW 2
         JMP     J0D54            ; 0D4A 1554  not run
J0D4B    JSX     S08F4            ; 0D4B 20F4
         DATA    X'8001'          ; 0D4C 8001  argument 1 of 0D4B; LDW 1
         JMP     J0D5E            ; 0D4D 155E  not run
J0D4E    JSX     S0ACD            ; 0D4E 22CD
         JSX     S0EB4            ; 0D4F 26B4
J0D50    JSX     S0AEA            ; 0D50 22EA
         JSX     S0C56            ; 0D51 2456
J0D52    LDX     D0D41            ; 0D52 9541
         JSX     *0               ; 0D53 2800
J0D54    LDW     D081F            ; 0D54 801F  not run
         STW     D0865            ; 0D55 7065  not run
         AND     D08F3            ; 0D56 E0F3  not run
         SRL     10               ; 0D57 0A0A  not run
         ORI     D0949            ; 0D58 C149  not run
         STW     D0949            ; 0D59 7149  not run
         JSX     S0C6F            ; 0D5A 246F  not run
         LDW     D0865            ; 0D5B 8065  not run
         STW     D081F            ; 0D5C 701F  not run
         JMP     J0D52            ; 0D5D 1552  not run
J0D5E    LDW     D0824            ; 0D5E 8024  not run
         SAZ                      ; 0D5F 0800  not run
         JMP     J0D62            ; 0D60 1562  not run
         JMP     J0D54            ; 0D61 1554  not run
J0D62    LDW     D080E            ; 0D62 800E  not run
         STW     D0823            ; 0D63 7023  not run
         JMP     J0D50            ; 0D64 1550  not run
         STW     D0DAD            ; 0D65 75AD
         LDW     D089E            ; 0D66 809E
         CLB     X'FF'            ; 0D67 07FF
         SNE                      ; 0D68 0870
         JMP     J0D6E            ; 0D69 156E
         JSX     S0A3A            ; 0D6A 223A
         LDW     D081F            ; 0D6B 801F
         ORI     D0816            ; 0D6C C016
         STW     D0DAD            ; 0D6D 75AD
J0D6E    LDW     D0828            ; 0D6E 8028
         SAZ                      ; 0D6F 0800
         JMP     J0831            ; 0D70 1031
         LDW     D0826            ; 0D71 8026
         STW     D0BE0            ; 0D72 73E0
         LDW     D081C            ; 0D73 801C
         STW     D0B96            ; 0D74 7396
J0D75    JSX     S0BC7            ; 0D75 23C7
         JMP     J0D83            ; 0D76 1583
         LDX     D0BE0            ; 0D77 93E0
         LDW     *0               ; 0D78 8800
         STW     D0EC2            ; 0D79 76C2
         LDW     *1               ; 0D7A 8801
         STW     D0EC3            ; 0D7B 76C3
         LDW     *3               ; 0D7C 8803
         STW     D081F            ; 0D7D 701F
         JSX     S0ECA            ; 0D7E 26CA
         JSX     S0829            ; 0D7F 2029
         DATA    X'0EC0'          ; 0D80 0EC0  argument 1 of 0D7F
         DATA    X'8010'          ; 0D81 8010  argument 2 of 0D7F; LDW X'10'
J0D82    JMP     J0D75            ; 0D82 1575
J0D83    SML     1                ; 0D83 0081
         JSX     S05B0            ; 0D84 25B0
J0D85    JSX     S0DAF            ; 0D85 25AF
         LDW     D0824            ; 0D86 8024
         SAZ                      ; 0D87 0800
         JMP     J0DA4            ; 0D88 15A4
         LDX     D0DAC            ; 0D89 95AC
         LDW     D0811            ; 0D8A 8011
         STW     *0               ; 0D8B 7800
         LDW     D0C6D            ; 0D8C 846D
         STW     *1               ; 0D8D 7801
         JSX     S0E4C            ; 0D8E 264C
         JSX     S0B98            ; 0D8F 2398
         JSX     S0BA1            ; 0D90 23A1
         LDW     D0DAD            ; 0D91 85AD
         STW     D081F            ; 0D92 701F
         LDW     D080C            ; 0D93 800C
         JSX     S0E25            ; 0D94 2625
         SLL     0                ; 0D95 0A10
         SLL     0                ; 0D96 0A10
J0D97    JSX     S0E4C            ; 0D97 264C
         LDW     D0824            ; 0D98 8024
         SAZ                      ; 0D99 0800
         JMP     J0DBC            ; 0D9A 15BC
J0D9B    JSX     S082F            ; 0D9B 202F
         DATA    X'06AA'          ; 0D9C 06AA  argument 1 of 0D9B
         DATA    X'8002'          ; 0D9D 8002  argument 2 of 0D9B; LDW 2
J0D9E    JSX     S0DAF            ; 0D9E 25AF
         DOT     12,3             ; 0D9F 03C3
         SML     1                ; 0DA0 0081
         JSX     S07B7            ; 0DA1 27B7
         DATA    X'8003'          ; 0DA2 8003  argument 1 of 0DA1; LDW 3
         JMP     J0835            ; 0DA3 1035
J0DA4    JSX     S082F            ; 0DA4 202F
         DATA    X'0DA8'          ; 0DA5 0DA8  argument 1 of 0DA4
         DATA    X'800A'          ; 0DA6 800A  argument 2 of 0DA4; LDW X'A'
J0DA7    JMP     J0D97            ; 0DA7 1597
         DATA    X'008A'          ; 0DA8 008A
         DATA    X'0002'          ; 0DA9 0002
D0DAA    DATA    X'0000'          ; 0DAA 0000  written by 086B 0C32 0DC2; read by 05C4 0C5A
         DATA    X'8D00'          ; 0DAB 8D00  LDW *X'500'
D0DAC    DATA    X'0000'          ; 0DAC 0000  written by 0B04; read by 05BC 0D89
D0DAD    DATA    X'0000'          ; 0DAD 0000  written by 086F 092C 0C15 0C20 ...; read by 0C1C 0CA8 0D1F 0D91 ...
D0DAE    DATA    X'0000'          ; 0DAE 0000  written by 0DAF; read by 0DB9
S0DAF    STX     D0DAE            ; 0DAF 65AE
         LLB     X'78'            ; 0DB0 0678
J0DB1    STB     D0DBB+1          ; 0DB1 3377
         JSX     S082F            ; 0DB2 202F
         DATA    X'0815'          ; 0DB3 0815  argument 1 of 0DB2
         DATA    X'8001'          ; 0DB4 8001  argument 2 of 0DB2; LDW 1
J0DB5    LDW     D0DBB            ; 0DB5 85BB
         SUB     D0807            ; 0DB6 B007
         SAZ                      ; 0DB7 0800
         JMP     J0DB1            ; 0DB8 15B1
         LDX     D0DAE            ; 0DB9 95AE
         JSX     *0               ; 0DBA 2800
D0DBB    DATA    X'0000'          ; 0DBB 0000  written by 0DB1; read by 0DB5
J0DBC    JSX     S082F            ; 0DBC 202F
         DATA    X'06AA'          ; 0DBD 06AA  argument 1 of 0DBC
         DATA    X'8002'          ; 0DBE 8002  argument 2 of 0DBC; LDW 2
J0DBF    LDW     D0DAD            ; 0DBF 85AD
         SAM                      ; 0DC0 0820
         JMP     J0D9B            ; 0DC1 159B  not run
         STW     D0DAA            ; 0DC2 75AA
         JSX     S082F            ; 0DC3 202F
         DATA    X'0DA8'          ; 0DC4 0DA8  argument 1 of 0DC3
         DATA    X'8008'          ; 0DC5 8008  argument 2 of 0DC3; LDW 8
J0DC6    JMP     J0D9E            ; 0DC6 159E
D0DC7    DATA    X'0DEC'          ; 0DC7 0DEC  read by 0DCD 0DE6
D0DC8    DATA    X'001D'          ; 0DC8 001D  read by 0DD0
D0DC9    DATA    X'0000'          ; 0DC9 0000  written by 0DD1; read by 0DD7
D0DCA    DATA    X'0000'          ; 0DCA 0000  written by 08DD 0957 0DE8 0E54 ...; read by 08D5 0926 0E9E
D0DCB    DATA    X'0000'          ; 0DCB 0000  written by 0DCC; read by 0DE9
S0DCC    STX     D0DCB            ; 0DCC 65CB
         LDW     D0DC7            ; 0DCD 85C7
         STW     D0DD5            ; 0DCE 75D5
         LDW     D0EC4            ; 0DCF 86C4
         LDX     D0DC8            ; 0DD0 95C8
J0DD1    STX     D0DC9            ; 0DD1 65C9
         LDX     D0DD5            ; 0DD2 95D5
         STW     *0               ; 0DD3 7800
         IXS     1                ; 0DD4 0401
D0DD5    DATA    X'0000'          ; 0DD5 0000  written by 0DCE 0DD6; read by 0DD2
         STX     D0DD5            ; 0DD6 65D5
         LDX     D0DC9            ; 0DD7 95C9
         DXS     1                ; 0DD8 0501
         JMP     J0DD1            ; 0DD9 15D1
         JSX     S0AE3            ; 0DDA 22E3
J0DDB    JMP     J0DE1            ; 0DDB 15E1
J0DDC    LLB     X'8D'            ; 0DDC 068D
         STB     D0DEC            ; 0DDD 33D8
         JSX     S0829            ; 0DDE 2029
         DATA    X'0DEC'          ; 0DDF 0DEC  argument 1 of 0DDE
         DATA    X'8010'          ; 0DE0 8010  argument 2 of 0DDE; LDW X'10'
J0DE1    JSX     S082D            ; 0DE1 202D
         DATA    X'0DEC'          ; 0DE2 0DEC  argument 1 of 0DE1
         DATA    X'803C'          ; 0DE3 803C  argument 2 of 0DE1; LDW X'3C'
J0DE4    SML     1                ; 0DE4 0081
         STW     D0620            ; 0DE5 7620
         LDW     D0DC7            ; 0DE6 85C7
         SLA     1                ; 0DE7 0911
         STW     D0DCA            ; 0DE8 75CA
         LDX     D0DCB            ; 0DE9 95CB
         JSX     *0               ; 0DEA 2800
D0DEB    DATA    X'0000'          ; 0DEB 0000  written by 0613 07DC
D0DEC    DATA    X'0000'          ; 0DEC 0000  written by 0DDD
         DATA    X'0000'          ; 0DED 0000
         DATA    X'0000'          ; 0DEE 0000
         DATA    X'0000'          ; 0DEF 0000
         DATA    X'0000'          ; 0DF0 0000
         DATA    X'0000'          ; 0DF1 0000
         DATA    X'0000'          ; 0DF2 0000
         DATA    X'0000'          ; 0DF3 0000
         DATA    X'0000'          ; 0DF4 0000
         DATA    X'0000'          ; 0DF5 0000
         DATA    X'0000'          ; 0DF6 0000
         DATA    X'0000'          ; 0DF7 0000
         DATA    X'0000'          ; 0DF8 0000
         DATA    X'0000'          ; 0DF9 0000
D0DFA    DATA    X'0000'          ; 0DFA 0000  written by 07E1
         DATA    X'0000'          ; 0DFB 0000
         DATA    X'0000'          ; 0DFC 0000
         DATA    X'0000'          ; 0DFD 0000
         DATA    X'0000'          ; 0DFE 0000
         DATA    X'0000'          ; 0DFF 0000
         DATA    X'0000'          ; 0E00 0000
         DATA    X'0000'          ; 0E01 0000
         DATA    X'0000'          ; 0E02 0000
         DATA    X'0000'          ; 0E03 0000
         DATA    X'0000'          ; 0E04 0000
         DATA    X'0000'          ; 0E05 0000
         DATA    X'0000'          ; 0E06 0000
         DATA    X'0000'          ; 0E07 0000
         DATA    X'0000'          ; 0E08 0000
         DATA    X'0000'          ; 0E09 0000
D0E0A    DATA    X'FFFF'          ; 0E0A FFFF  CMW *X'7FF'; read by 09AD
D0E0B    DATA    X'0000'          ; 0E0B 0000  written by 0E0C; read by 0E13
S0E0C    STX     D0E0B            ; 0E0C 660B
         LDW     D089E            ; 0E0D 809E
         SLC     8                ; 0E0E 0A58
         STW     D089E            ; 0E0F 709E
         JSX     S082F            ; 0E10 202F
         DATA    X'089E'          ; 0E11 089E  argument 1 of 0E10
         DATA    X'8001'          ; 0E12 8001  argument 2 of 0E10; LDW 1
J0E13    LDX     D0E0B            ; 0E13 960B
         JSX     *0               ; 0E14 2800
D0E15    DATA    X'0000'          ; 0E15 0000  written by 0E16; read by 0E1A
S0E16    STX     D0E15            ; 0E16 6615
         LDW     D0E1C            ; 0E17 861C
         STW     D089E            ; 0E18 709E
         JSX     S0E0C            ; 0E19 260C
         LDX     D0E15            ; 0E1A 9615
         JSX     *0               ; 0E1B 2800
D0E1C    DATA    X'008A'          ; 0E1C 008A  read by 0E17
D0E1D    DATA    X'0000'          ; 0E1D 0000  written by 0E1E; read by 0E22
S0E1E    STX     D0E1D            ; 0E1E 661D
         LDW     D0EC0            ; 0E1F 86C0
         STW     D089E            ; 0E20 709E
         JSX     S0E0C            ; 0E21 260C
         LDX     D0E1D            ; 0E22 961D
         JSX     *0               ; 0E23 2800
D0E24    DATA    X'0000'          ; 0E24 0000  written by 0E25; read by 0E2E
S0E25    STX     D0E24            ; 0E25 6624
         CAX                      ; 0E26 0130
         LDW     D0824            ; 0E27 8024
         SAZ                      ; 0E28 0800
         JMP     J0E2C            ; 0E29 162C
         CXA                      ; 0E2A 0140
         JSX     S0E31            ; 0E2B 2631
J0E2C    LDW     D081F            ; 0E2C 801F
         JSX     S0E31            ; 0E2D 2631
         LDX     D0E24            ; 0E2E 9624
         JSX     *0               ; 0E2F 2800
D0E30    DATA    X'0000'          ; 0E30 0000  written by 0E31; read by 0E45
S0E31    STX     D0E30            ; 0E31 6630
         CAX                      ; 0E32 0130
         LDW     D0828            ; 0E33 8028
         SAZ                      ; 0E34 0800
         JMP     J0E45            ; 0E35 1645
         LDW     D0E47            ; 0E36 8647
         SRL     1                ; 0E37 0A01
         CMW     D0803            ; 0E38 F003
         SLS                      ; 0E39 0840
         JSX     S0ACA            ; 0E3A 22CA  not run
         SLCD    8                ; 0E3B 0A78
         SLCD    8                ; 0E3C 0A78
         STW     *0               ; 0E3D 7800
         CXA                      ; 0E3E 0140
         ADD     D0807            ; 0E3F A007
         SLL     1                ; 0E40 0A11
         STW     D0E47            ; 0E41 7647
         LDW     D0E49            ; 0E42 8649
         ADD     D0805            ; 0E43 A005
         STW     D0E49            ; 0E44 7649
J0E45    LDX     D0E30            ; 0E45 9630
         JSX     *0               ; 0E46 2800
D0E47    DATA    X'002C'          ; 0E47 002C  written by 0879 0E41 0E75; read by 09B2 0B02 0B06 0E36 ...
D0E48    DATA    X'002C'          ; 0E48 002C  read by 0878 0E53 0E74
D0E49    DATA    X'0000'          ; 0E49 0000  written by 086E 0E44 0E6B 0E73; read by 0E42 0E59 0E5B 0E69 ...
D0E4A    DATA    X'005C'          ; 0E4A 005C  read by 0E58 0E79
D0E4B    DATA    X'0000'          ; 0E4B 0000  written by 0E4C; read by 0E76
S0E4C    STX     D0E4B            ; 0E4C 664B
         LDW     D0828            ; 0E4D 8028
         SAZ                      ; 0E4E 0800
         JMP     J0E76            ; 0E4F 1676  not run
         LDW     D0824            ; 0E50 8024
         SAZ                      ; 0E51 0800
         JMP     J0E86            ; 0E52 1686
         LDW     D0E48            ; 0E53 8648
         STW     D0DCA            ; 0E54 75CA
J0E55    CLR                      ; 0E55 0100
         STW     D0E85            ; 0E56 7685
         JSX     S0E16            ; 0E57 2616
         LDW     D0E4A            ; 0E58 864A
         CMW     D0E49            ; 0E59 F649
         SLS                      ; 0E5A 0840
         LDW     D0E49            ; 0E5B 8649
         ADD     D0805            ; 0E5C A005
         STW     D089E            ; 0E5D 709E
         JSX     S082F            ; 0E5E 202F
         DATA    X'089E'          ; 0E5F 089E  argument 1 of 0E5E
         DATA    X'8002'          ; 0E60 8002  argument 2 of 0E5E; LDW 2
J0E61    LLB     0                ; 0E61 0600
         STW     D089E            ; 0E62 709E
         JSX     S0E0C            ; 0E63 260C
J0E64    JSX     S08D3            ; 0E64 20D3
         JSX     S0E0C            ; 0E65 260C
         LDW     D0E85            ; 0E66 8685
         ADD     D0807            ; 0E67 A007
         STW     D0E85            ; 0E68 7685
         LDW     D0E49            ; 0E69 8649
         SUB     D0807            ; 0E6A B007
         STW     D0E49            ; 0E6B 7649
         SAZ                      ; 0E6C 0800
         JMP     J0E78            ; 0E6D 1678
         LDW     D0E85            ; 0E6E 8685
J0E6F    JSX     S0E9A            ; 0E6F 269A
         JSX     S0E0C            ; 0E70 260C
         JSX     S0E1E            ; 0E71 261E
         CLR                      ; 0E72 0100
         STW     D0E49            ; 0E73 7649
         LDW     D0E48            ; 0E74 8648
         STW     D0E47            ; 0E75 7647
J0E76    LDX     D0E4B            ; 0E76 964B
         JSX     *0               ; 0E77 2800
J0E78    LDW     D0E85            ; 0E78 8685
         CMW     D0E4A            ; 0E79 F64A
         SEQ                      ; 0E7A 0860
         JMP     J0E64            ; 0E7B 1664
         JSX     S0E9A            ; 0E7C 269A
         JSX     S0E0C            ; 0E7D 260C
         JSX     S0E1E            ; 0E7E 261E
         CLR                      ; 0E7F 0100
         STW     D089E            ; 0E80 709E
         JSX     S0E0C            ; 0E81 260C
         JSX     S0E0C            ; 0E82 260C
         JSX     S0E0C            ; 0E83 260C
         JMP     J0E55            ; 0E84 1655
D0E85    DATA    X'0000'          ; 0E85 0000  written by 0E56 0E68; read by 0E66 0E6E 0E78
J0E86    JSX     S0E16            ; 0E86 2616
         LDW     D0E49            ; 0E87 8649
         ADD     D0805            ; 0E88 A005
         STW     D089E            ; 0E89 709E
         LDW     D0E47            ; 0E8A 8647
         STW     D0DCA            ; 0E8B 75CA
         JSX     S082F            ; 0E8C 202F
         DATA    X'089E'          ; 0E8D 089E  argument 1 of 0E8C
         DATA    X'8002'          ; 0E8E 8002  argument 2 of 0E8C; LDW 2
J0E8F    LDW     D0E49            ; 0E8F 8649
         STW     D0E93            ; 0E90 7693
         JSX     S082F            ; 0E91 202F
         DATA    X'0016'          ; 0E92 0016  argument 1 of 0E91
D0E93    DATA    X'8000'          ; 0E93 8000  argument 2 of 0E91; LDW 0; written by 0E90
J0E94    JSX     S082F            ; 0E94 202F
         DATA    X'0815'          ; 0E95 0815  argument 1 of 0E94
         DATA    X'8001'          ; 0E96 8001  argument 2 of 0E94; LDW 1
J0E97    LDW     D0E49            ; 0E97 8649
         JMP     J0E6F            ; 0E98 166F
D0E99    DATA    X'0000'          ; 0E99 0000  written by 0E9A; read by 0EB0
S0E9A    STX     D0E99            ; 0E9A 6699
         STW     D0EB2            ; 0E9B 76B2
         CLR                      ; 0E9C 0100
         STW     D0EA6            ; 0E9D 76A6
         LDW     D0DCA            ; 0E9E 85CA
         SUB     D0EB2            ; 0E9F B6B2
         CAX                      ; 0EA0 0130
J0EA1    CLR                      ; 0EA1 0100
         LDB     *0               ; 0EA2 5800
         ADD     D0EA6            ; 0EA3 A6A6
         STW     D0EA6            ; 0EA4 76A6
         IXS     1                ; 0EA5 0401
D0EA6    DATA    X'0000'          ; 0EA6 0000  the checksum's running sum, which the IXS in front skips: an empty object runs the loop until the index turns and then runs the sum (seen in the range and expression sessions)  --  written by 0E9D 0EA4; read by 0EA3 0EAC 0EAE
         LDW     D0EB2            ; 0EA7 86B2
         SUB     D0807            ; 0EA8 B007
         STW     D0EB2            ; 0EA9 76B2
         SAZ                      ; 0EAA 0800
         JMP     J0EA1            ; 0EAB 16A1
         LDW     D0EA6            ; 0EAC 86A6
         SRL     8                ; 0EAD 0A08
         ADD     D0EA6            ; 0EAE A6A6
         STW     D089E            ; 0EAF 709E
         LDX     D0E99            ; 0EB0 9699
         JSX     *0               ; 0EB1 2800
D0EB2    DATA    X'0000'          ; 0EB2 0000  written by 0E9B 0EA9; read by 0E9F 0EA7
D0EB3    DATA    X'0000'          ; 0EB3 0000  written by 0EB4; read by 0EBE
S0EB4    STX     D0EB3            ; 0EB4 66B3
         LDW     D0C6D            ; 0EB5 846D
         SLC     8                ; 0EB6 0A58
         JSX     S0ED7            ; 0EB7 26D7
         LDX     D0EC8            ; 0EB8 96C8
         STW     *0               ; 0EB9 7800
         LDW     D0C6D            ; 0EBA 846D
         JSX     S0ED7            ; 0EBB 26D7
         LDX     D0EC8            ; 0EBC 96C8
         STW     *1               ; 0EBD 7801
         LDX     D0EB3            ; 0EBE 96B3
         JSX     *0               ; 0EBF 2800
D0EC0    DATA    X'008D'          ; 0EC0 008D  read by 0E1F
D0EC1    DATA    X'A0A0'          ; 0EC1 A0A0  '  '; written by 0AD5 0AFA
D0EC2    DATA    X'0000'          ; 0EC2 0000  written by 0C7C 0D79
D0EC3    DATA    X'0000'          ; 0EC3 0000  written by 0C7D 0D7B
D0EC4    DATA    X'A0A0'          ; 0EC4 A0A0  '  '; read by 08AB 0AF9 0B7B 0B7D ...
         DATA    X'0000'          ; 0EC5 0000
         DATA    X'0000'          ; 0EC6 0000
         DATA    X'8A00'          ; 0EC7 8A00  LDW *X'200'
D0EC8    DATA    X'0EC2'          ; 0EC8 0EC2  read by 0EB8 0EBC 0ECE 0ED2
D0EC9    DATA    X'0000'          ; 0EC9 0000  written by 0ECA; read by 0ED4
S0ECA    STX     D0EC9            ; 0ECA 66C9
         LDW     D081F            ; 0ECB 801F
         SLC     8                ; 0ECC 0A58
         JSX     S0ED7            ; 0ECD 26D7
         LDX     D0EC8            ; 0ECE 96C8
         STW     *3               ; 0ECF 7803
         LDW     D081F            ; 0ED0 801F
         JSX     S0ED7            ; 0ED1 26D7
         LDX     D0EC8            ; 0ED2 96C8
         STW     *4               ; 0ED3 7804
         LDX     D0EC9            ; 0ED4 96C9
         JSX     *0               ; 0ED5 2800
D0ED6    DATA    X'0000'          ; 0ED6 0000  written by 0ED7; read by 0EE0
S0ED7    STX     D0ED6            ; 0ED7 66D6
         STW     D0822            ; 0ED8 7022
         SRL     4                ; 0ED9 0A04
         JSX     S0EE2            ; 0EDA 26E2
J0EDB    SLL     8                ; 0EDB 0A18
         STW     D0820            ; 0EDC 7020
         LDW     D0822            ; 0EDD 8022
         JSX     S0EE2            ; 0EDE 26E2
J0EDF    ORI     D0820            ; 0EDF C020
         LDX     D0ED6            ; 0EE0 96D6
         JSX     *0               ; 0EE1 2800
S0EE2    AND     D09ED            ; 0EE2 E1ED
         CLB     9                ; 0EE3 0709
         SLE                      ; 0EE4 0890
         ADD     D080F            ; 0EE5 A00F
         ADD     D0809            ; 0EE6 A009
         JMP     *0               ; 0EE7 1800
D0EE8    DATA    X'0000'          ; 0EE8 0000  written by 0EE9; read by 0F02
J0EE9    STX     D0EE8            ; 0EE9 66E8
         LDW     *1               ; 0EEA 8801
         ORI     D0816            ; 0EEB C016
         STW     D0F06            ; 0EEC 7706
         LDW     *2               ; 0EED 8802
         ORI     D0816            ; 0EEE C016
         STW     D0F05            ; 0EEF 7705
         LDW     *0               ; 0EF0 8800
         SAP                      ; 0EF1 0810
         JMP     J0F02            ; 0EF2 1702  not run
         CAX                      ; 0EF3 0130
J0EF4    DXS     1                ; 0EF4 0501
         JMP     J0EF7            ; 0EF5 16F7
         JMP     J0F02            ; 0EF6 1702
J0EF7    STX     D0F04            ; 0EF7 6704
         LDX     D0F06            ; 0EF8 9706
         LDW     *0               ; 0EF9 8800
         IXS     1                ; 0EFA 0401
         STX     D0F06            ; 0EFB 6706
         LDX     D0F05            ; 0EFC 9705
         STW     *0               ; 0EFD 7800
         IXS     1                ; 0EFE 0401
         STX     D0F05            ; 0EFF 6705
         LDX     D0F04            ; 0F00 9704
         JMP     J0EF4            ; 0F01 16F4
J0F02    LDX     D0EE8            ; 0F02 96E8
         JSX     *3               ; 0F03 2803
D0F04    DATA    X'0000'          ; 0F04 0000  written by 0EF7; read by 0F00
D0F05    DATA    X'0000'          ; 0F05 0000  written by 0EEF 0EFF; read by 0EFC
D0F06    DATA    X'0000'          ; 0F06 0000  written by 0EEC 0EFB; read by 0EF8
         DATA    X'CACD'          ; 0F07 CACD  'JM'
         DATA    X'D0A0'          ; 0F08 D0A0  'P '
         DATA    X'1001'          ; 0F09 1001  JMP 1
         DATA    X'CAD3'          ; 0F0A CAD3  'JS'
         DATA    X'D8A0'          ; 0F0B D8A0  'X '
         DATA    X'2001'          ; 0F0C 2001  JSX 1
         DATA    X'D3D4'          ; 0F0D D3D4  'ST'
         DATA    X'D8A0'          ; 0F0E D8A0  'X '
         DATA    X'6001'          ; 0F0F 6001  STX 1
         DATA    X'D3D4'          ; 0F10 D3D4  'ST'
         DATA    X'D7A0'          ; 0F11 D7A0  'W '
         DATA    X'7001'          ; 0F12 7001  STW 1
         DATA    X'CCC4'          ; 0F13 CCC4  'LD'
         DATA    X'D7A0'          ; 0F14 D7A0  'W '
         DATA    X'8001'          ; 0F15 8001  LDW 1
         DATA    X'CCC4'          ; 0F16 CCC4  'LD'
         DATA    X'D8A0'          ; 0F17 D8A0  'X '
         DATA    X'9001'          ; 0F18 9001  LDX 1
         DATA    X'C1C4'          ; 0F19 C1C4  'AD'
         DATA    X'C4A0'          ; 0F1A C4A0  'D '
         DATA    X'A001'          ; 0F1B A001  ADD 1
         DATA    X'D3D5'          ; 0F1C D3D5  'SU'
         DATA    X'C2A0'          ; 0F1D C2A0  'B '
         DATA    X'B001'          ; 0F1E B001  SUB 1
         DATA    X'CFD2'          ; 0F1F CFD2  'OR'
         DATA    X'C9A0'          ; 0F20 C9A0  'I '
         DATA    X'C001'          ; 0F21 C001  ORI 1
         DATA    X'CFD2'          ; 0F22 CFD2  'OR'
         DATA    X'C5A0'          ; 0F23 C5A0  'E '
         DATA    X'D001'          ; 0F24 D001  ORE 1
         DATA    X'C1CE'          ; 0F25 C1CE  'AN'
         DATA    X'C4A0'          ; 0F26 C4A0  'D '
         DATA    X'E001'          ; 0F27 E001  AND 1
         DATA    X'C3CD'          ; 0F28 C3CD  'CM'
         DATA    X'D7A0'          ; 0F29 D7A0  'W '
         DATA    X'F001'          ; 0F2A F001  CMW 1
         DATA    X'D3D4'          ; 0F2B D3D4  'ST'
         DATA    X'C2A0'          ; 0F2C C2A0  'B '
         DATA    X'3002'          ; 0F2D 3002  STB 2
         DATA    X'C3CD'          ; 0F2E C3CD  'CM'
         DATA    X'C2A0'          ; 0F2F C2A0  'B '
         DATA    X'4002'          ; 0F30 4002  CMB 2
         DATA    X'CCC4'          ; 0F31 CCC4  'LD'
         DATA    X'C2A0'          ; 0F32 C2A0  'B '
         DATA    X'5002'          ; 0F33 5002  LDB 2
         DATA    X'D3CD'          ; 0F34 D3CD  'SM'
         DATA    X'CCA0'          ; 0F35 CCA0  'L '
         DATA    X'0083'          ; 0F36 0083
         DATA    X'D3CD'          ; 0F37 D3CD  'SM'
         DATA    X'D5A0'          ; 0F38 D5A0  'U '
         DATA    X'0093'          ; 0F39 0093
         DATA    X'C9CE'          ; 0F3A C9CE  'IN'
         DATA    X'D2A0'          ; 0F3B D2A0  'R '
         DATA    X'0013'          ; 0F3C 0013
         DATA    X'C5CE'          ; 0F3D C5CE  'EN'
         DATA    X'C2A0'          ; 0F3E C2A0  'B '
         DATA    X'0023'          ; 0F3F 0023
         DATA    X'C4D3'          ; 0F40 C4D3  'DS'
         DATA    X'C2A0'          ; 0F41 C2A0  'B '
         DATA    X'0033'          ; 0F42 0033
         DATA    X'D3D2'          ; 0F43 D3D2  'SR'
         DATA    X'C1A0'          ; 0F44 C1A0  'A '
         DATA    X'0904'          ; 0F45 0904
         DATA    X'D3CC'          ; 0F46 D3CC  'SL'
         DATA    X'C1A0'          ; 0F47 C1A0  'A '
         DATA    X'0914'          ; 0F48 0914
         DATA    X'D3D2'          ; 0F49 D3D2  'SR'
         DATA    X'CCA0'          ; 0F4A CCA0  'L '
         DATA    X'0A04'          ; 0F4B 0A04
         DATA    X'D3CC'          ; 0F4C D3CC  'SL'
         DATA    X'CCA0'          ; 0F4D CCA0  'L '
         DATA    X'0A14'          ; 0F4E 0A14
         DATA    X'D3D2'          ; 0F4F D3D2  'SR'
         DATA    X'C3A0'          ; 0F50 C3A0  'C '
         DATA    X'0A44'          ; 0F51 0A44
         DATA    X'D3CC'          ; 0F52 D3CC  'SL'
         DATA    X'C3A0'          ; 0F53 C3A0  'C '
         DATA    X'0A54'          ; 0F54 0A54
         DATA    X'C8CC'          ; 0F55 C8CC  'HL'
         DATA    X'D4A0'          ; 0F56 D4A0  'T '
         DATA    X'0005'          ; 0F57 0005
         DATA    X'D3CC'          ; 0F58 D3CC  'SL'
         DATA    X'CDA0'          ; 0F59 CDA0  'M '
         DATA    X'0046'          ; 0F5A 0046
         DATA    X'D3C7'          ; 0F5B D3C7  'SG'
         DATA    X'CDA0'          ; 0F5C CDA0  'M '
         DATA    X'0056'          ; 0F5D 0056
         DATA    X'D3C1'          ; 0F5E D3C1  'SA'
         DATA    X'DAA0'          ; 0F5F DAA0  'Z '
         DATA    X'0806'          ; 0F60 0806
         DATA    X'D3C1'          ; 0F61 D3C1  'SA'
         DATA    X'D0A0'          ; 0F62 D0A0  'P '
         DATA    X'0816'          ; 0F63 0816
         DATA    X'D3C1'          ; 0F64 D3C1  'SA'
         DATA    X'CDA0'          ; 0F65 CDA0  'M '
         DATA    X'0826'          ; 0F66 0826
         DATA    X'D3D8'          ; 0F67 D3D8  'SX'
         DATA    X'D0A0'          ; 0F68 D0A0  'P '
         DATA    X'0406'          ; 0F69 0406
         DATA    X'D3D8'          ; 0F6A D3D8  'SX'
         DATA    X'CDA0'          ; 0F6B CDA0  'M '
         DATA    X'0506'          ; 0F6C 0506
         DATA    X'D3C1'          ; 0F6D D3C1  'SA'
         DATA    X'CFA0'          ; 0F6E CFA0  'O '
         DATA    X'0836'          ; 0F6F 0836
         DATA    X'D3D3'          ; 0F70 D3D3  'SS'
         DATA    X'C5A0'          ; 0F71 C5A0  'E '
         DATA    X'08B6'          ; 0F72 08B6
         DATA    X'D3D3'          ; 0F73 D3D3  'SS'
         DATA    X'B0A0'          ; 0F74 B0A0  '0 '
         DATA    X'08C6'          ; 0F75 08C6
         DATA    X'D3D3'          ; 0F76 D3D3  'SS'
         DATA    X'B1A0'          ; 0F77 B1A0  '1 '
         DATA    X'08D6'          ; 0F78 08D6
         DATA    X'D3D3'          ; 0F79 D3D3  'SS'
         DATA    X'B2A0'          ; 0F7A B2A0  '2 '
         DATA    X'08E6'          ; 0F7B 08E6
         DATA    X'D3D3'          ; 0F7C D3D3  'SS'
         DATA    X'B3A0'          ; 0F7D B3A0  '3 '
         DATA    X'08F6'          ; 0F7E 08F6
         DATA    X'D3D8'          ; 0F7F D3D8  'SX'
         DATA    X'C5A0'          ; 0F80 C5A0  'E '
         DATA    X'0856'          ; 0F81 0856
         DATA    X'D3CE'          ; 0F82 D3CE  'SN'
         DATA    X'CFA0'          ; 0F83 CFA0  'O '
         DATA    X'08A6'          ; 0F84 08A6
         DATA    X'D3CC'          ; 0F85 D3CC  'SL'
         DATA    X'D3A0'          ; 0F86 D3A0  'S '
         DATA    X'0846'          ; 0F87 0846
         DATA    X'D3C5'          ; 0F88 D3C5  'SE'
         DATA    X'D1A0'          ; 0F89 D1A0  'Q '
         DATA    X'0866'          ; 0F8A 0866
         DATA    X'D3CE'          ; 0F8B D3CE  'SN'
         DATA    X'C5A0'          ; 0F8C C5A0  'E '
         DATA    X'0876'          ; 0F8D 0876
         DATA    X'D3C7'          ; 0F8E D3C7  'SG'
         DATA    X'D2A0'          ; 0F8F D2A0  'R '
         DATA    X'0886'          ; 0F90 0886
         DATA    X'D3CC'          ; 0F91 D3CC  'SL'
         DATA    X'C5A0'          ; 0F92 C5A0  'E '
         DATA    X'0896'          ; 0F93 0896
         DATA    X'C3C5'          ; 0F94 C3C5  'CE'
         DATA    X'D8A0'          ; 0F95 D8A0  'X '
         DATA    X'0066'          ; 0F96 0066
         DATA    X'C3D8'          ; 0F97 C3D8  'CX'
         DATA    X'C5A0'          ; 0F98 C5A0  'E '
         DATA    X'0076'          ; 0F99 0076
         DATA    X'CDD3'          ; 0F9A CDD3  'MS'
         DATA    X'CBA0'          ; 0F9B CBA0  'K '
         DATA    X'00A6'          ; 0F9C 00A6
         DATA    X'D5CE'          ; 0F9D D5CE  'UN'
         DATA    X'CDA0'          ; 0F9E CDA0  'M '
         DATA    X'00B6'          ; 0F9F 00B6
         DATA    X'C3CC'          ; 0FA0 C3CC  'CL'
         DATA    X'D2A0'          ; 0FA1 D2A0  'R '
         DATA    X'0106'          ; 0FA2 0106
         DATA    X'C3CD'          ; 0FA3 C3CD  'CM'
         DATA    X'D0A0'          ; 0FA4 D0A0  'P '
         DATA    X'0116'          ; 0FA5 0116
         DATA    X'C9CE'          ; 0FA6 C9CE  'IN'
         DATA    X'D6A0'          ; 0FA7 D6A0  'V '
         DATA    X'0126'          ; 0FA8 0126
         DATA    X'C3C1'          ; 0FA9 C3C1  'CA'
         DATA    X'D8A0'          ; 0FAA D8A0  'X '
         DATA    X'0136'          ; 0FAB 0136
         DATA    X'C3D8'          ; 0FAC C3D8  'CX'
         DATA    X'C1A0'          ; 0FAD C1A0  'A '
         DATA    X'0146'          ; 0FAE 0146
         DATA    X'CECF'          ; 0FAF CECF  'NO'
         DATA    X'D0A0'          ; 0FB0 D0A0  'P '
         DATA    X'0A06'          ; 0FB1 0A06
         DATA    X'C9D8'          ; 0FB2 C9D8  'IX'
         DATA    X'D3A0'          ; 0FB3 D3A0  'S '
         DATA    X'0407'          ; 0FB4 0407
         DATA    X'C4D8'          ; 0FB5 C4D8  'DX'
         DATA    X'D3A0'          ; 0FB6 D3A0  'S '
         DATA    X'0507'          ; 0FB7 0507
         DATA    X'CCCC'          ; 0FB8 CCCC  'LL'
         DATA    X'C2A0'          ; 0FB9 C2A0  'B '
         DATA    X'0607'          ; 0FBA 0607
         DATA    X'C3CC'          ; 0FBB C3CC  'CL'
         DATA    X'C2A0'          ; 0FBC C2A0  'B '
         DATA    X'0707'          ; 0FBD 0707
         DATA    X'C4C9'          ; 0FBE C4C9  'DI'
         DATA    X'CEA0'          ; 0FBF CEA0  'N '
         DATA    X'0208'          ; 0FC0 0208
         DATA    X'C4CF'          ; 0FC1 C4CF  'DO'
         DATA    X'D4A0'          ; 0FC2 D4A0  'T '
         DATA    X'0308'          ; 0FC3 0308
         DATA    X'CDD0'          ; 0FC4 CDD0  'MP'
         DATA    X'D9A0'          ; 0FC5 D9A0  'Y '
         DATA    X'0B09'          ; 0FC6 0B09
         DATA    X'C4C9'          ; 0FC7 C4C9  'DI'
         DATA    X'D6A0'          ; 0FC8 D6A0  'V '
         DATA    X'0C09'          ; 0FC9 0C09
         DATA    X'D3CD'          ; 0FCA D3CD  'SM'
         DATA    X'C2A0'          ; 0FCB C2A0  'B '
         DATA    X'008A'          ; 0FCC 008A
         DATA    X'C5D1'          ; 0FCD C5D1  'EQ'
         DATA    X'D5A0'          ; 0FCE D5A0  'U '
         DATA    X'00B0'          ; 0FCF 00B0
         DATA    X'C4C1'          ; 0FD0 C4C1  'DA'
         DATA    X'D4C1'          ; 0FD1 D4C1  'TA'
         DATA    X'00C0'          ; 0FD2 00C0
         DATA    X'C4A0'          ; 0FD3 C4A0  'D '
         DATA    X'A0A0'          ; 0FD4 A0A0  '  '
         DATA    X'00C0'          ; 0FD5 00C0
         DATA    X'D2C5'          ; 0FD6 D2C5  'RE'
         DATA    X'D3A0'          ; 0FD7 D3A0  'S '
         DATA    X'00D0'          ; 0FD8 00D0
         DATA    X'CFD2'          ; 0FD9 CFD2  'OR'
         DATA    X'C9C7'          ; 0FDA C9C7  'IG'
         DATA    X'00E0'          ; 0FDB 00E0
         DATA    X'CED4'          ; 0FDC CED4  'NT'
         DATA    X'D2D9'          ; 0FDD D2D9  'RY'
         DATA    X'00F0'          ; 0FDE 00F0
         DATA    X'CCCF'          ; 0FDF CCCF  'LO'
         DATA    X'C1C4'          ; 0FE0 C1C4  'AD'
         DATA    X'0100'          ; 0FE1 0100
         DATA    X'CCC9'          ; 0FE2 CCC9  'LI'
         DATA    X'C2D2'          ; 0FE3 C2D2  'BR'
         DATA    X'0110'          ; 0FE4 0110
         DATA    X'C5CE'          ; 0FE5 C5CE  'EN'
         DATA    X'C4A0'          ; 0FE6 C4A0  'D '
         DATA    X'0120'          ; 0FE7 0120
         DATA    X'C2D9'          ; 0FE8 C2D9  'BY'
         DATA    X'D4C5'          ; 0FE9 D4C5  'TE'
         DATA    X'0130'          ; 0FEA 0130
         DATA    X'C5CE'          ; 0FEB C5CE  'EN'
         DATA    X'C4C3'          ; 0FEC C4C3  'DC'
         DATA    X'0140'          ; 0FED 0140
         DATA    X'D4D2'          ; 0FEE D4D2  'TR'
         DATA    X'D5C5'          ; 0FEF D5C5  'UE'
         DATA    X'0150'          ; 0FF0 0150
         DATA    X'C6C1'          ; 0FF1 C6C1  'FA'
         DATA    X'CCD3'          ; 0FF2 CCD3  'LS'
         DATA    X'0160'          ; 0FF3 0160
         DATA    X'0000'          ; 0FF4 0000
         DATA    X'0029'          ; 0FF5 0029
         END
