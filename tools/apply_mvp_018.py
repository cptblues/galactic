#!/usr/bin/env python3
"""Apply Galactic MVP-018 safely from the exact pushed baseline.

This migration generalizes ownership, loads configurable dormant factions,
centralizes faction authorization and persists owners. It validates everything
in a detached worktree before writing to the main worktree.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile


def load_shared_helpers():
    candidates = (
        Path(__file__).resolve().with_name("apply_mvp_016_b.py"),
        Path.cwd() / "tools" / "apply_mvp_016_b.py",
        Path(__file__).resolve().parent / "galactic" / "tools" / "apply_mvp_016_b.py",
    )
    helper = next((candidate for candidate in candidates if candidate.is_file()), None)
    if helper is None:
        return None
    spec = importlib.util.spec_from_file_location("apply_mvp_016_b", helper)
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


base = load_shared_helpers()
if base is None:
    print(
        "ERREUR : tools/apply_mvp_016_b.py est requis à côté de ce script.",
        file=sys.stderr,
    )
    raise SystemExit(1)


MIGRATION = "MVP-018"
BASELINE_SHA = "34bd3cc29b41f19b2c3aee5a54df607d471297c6"
PATCH_SHA256 = "926ebe6d35bc9d0bc2109e71499332fb201fcd70395a39f845162aad3432f178"

MODIFIED_BLOBS = {
    'README.md': 'b97826787da7e0583105d4a01a529149d2b43c99',
    'assets/rulesets/default/manifest.ron': '272d49d6dd55539de43b685170a7875d19cf044a',
    'assets/rulesets/default/starting_scenario.ron': 'c5b1ad650903a6decd233699f9b39ba9ec2a08a2',
    'crates/galactic_client/src/craft_ui.rs': 'c5cadb540f25fffdc02c2b1ee3c9fe4da1243143',
    'crates/galactic_client/src/lib.rs': 'e422336cd450caed260ea8abba13e8790c1cbc0a',
    'crates/galactic_client/src/research_ui.rs': '798f265e1a14bd98c43d9795be38f1ca4547112c',
    'crates/galactic_domain/src/lib.rs': '0a624fc51d36d1946a530dbc9fd528471c52a803',
    'crates/galactic_persistence/src/lib.rs': '025218b599fd5a0bd23b96f1be2046ead309c60b',
    'crates/galactic_sim/src/construction.rs': '8fc582802006660db086cde9b188e8e49853567a',
    'crates/galactic_sim/src/craft.rs': 'b23484a6be52ef6d7fc4bfd455294e6f9821ef96',
    'crates/galactic_sim/src/research.rs': 'e52b44c134400a67649e664a58ce1c6b691dde96',
    'crates/galactic_sim/src/ruleset.rs': '6bcfec643994f80cb3dba4bfbcaff48042c58ed7',
    'crates/galactic_sim/src/simulation.rs': '000a9d730537df2c910f63768e6a3196803ca5cd',
    'crates/galactic_sim/src/starting.rs': 'd961d4e839696810d1f8bd164ec268807e83a526',
    'crates/galactic_sim/src/state.rs': '28f4d04647f5310531021c29473d3788cc881d56',
    'docs/mvp_architecture.md': '6ea9e968ec4aef08121be552f5238852f491404f',
    'docs/ruleset.md': '5157597b6d8f5e67bdc1ff99afc5b8e3a254bae3',
}

DEPENDENCY_BLOBS = {
    "tools/apply_mvp_016_b.py": "1557ff3f419abbf6a1b58b897100aa72da80bd38",
}

CREATED_PATHS = (
    "assets/rulesets/default/factions.ron",
    "crates/galactic_domain/src/ownership.rs",
)

EXPECTED_PATHS = frozenset((*MODIFIED_BLOBS, *CREATED_PATHS))

CHECK_COMMANDS = (
    ("cargo", "fmt", "--all"),
    ("cargo", "check", "--workspace", "--all-targets", "--all-features"),
    (
        "cargo",
        "clippy",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ),
    ("cargo", "test", "--workspace"),
    ("cargo", "build", "--release"),
)

# zlib-compressed, Base85-encoded binary Git patch.
PATCH_B85 = """c-rlK+j1L6lIT0XqQ!}jfdL5NN|2DHSz4lP#^|;p?KlpHqt!$g$hH79+})7Hu^Dkb;(WlyzMRoM@7q4|PtKQ|%(``58x4~3nT<1wut|1TXH{ioW#z3h<774q_VzB4B8VQnJwAH=^0>2zgY)wDon#r4kHPuj_@KW(?sP`us5ct)f?l_~KOFAt?d_>w?zCDh?dyN}mtb$$Z65?J`hWQIc{*J$$g+rvBwYs6be$Ds61<DkY5r*OVHHN%^fD>Pv{+{ZAiV4B1UtbC0xg#XS+0YdPw-p1NPbz9Aet7*EFgIiOfRGC;^tEjlhr!OgLhG$lOlhVt>=XP5|df9o)?ea1<`sZh;KeEqAZIpNC!aY0Q=^@io6}f>+I$efnN&pvB00_H~(HFI9mE_dbv#J=|zHXX3=?;Oz9-@_RiisS-_xk=s%bx&}Tr3AWoM{1cwiy$5oUS!HTRu0Jz}hKZ=Y5i!@GV$<3!c$fIQ*WMrByR`7Wi%*oE)&t%=%Y0Yqw*hz37eeATfP6FBoAo!bgf)FU0XUWAnW6L8JhT~DJN34>K57lDeoeTh-*J`%2B)`(R;i!qAF-C!1_OH{Kbr0fbMkwh4ln;-V>-n4$Uad}$CcEu!(CW4O10czrR%53Xz`qY9%c0dI=(X7c@!2H!kv+hFjq+Fgm&B8x+t!~gqXp3P?onO<mXANKNSq*?U8C81nAs$FGmoxFX1v0VKLCs(TNC9~vt2oko_icGQ@~&zKyN@HWEn?5&+f6jBI_cH=AOaKqItf>X!`EaJWqfGixmD3)SE3}Fz&H@y+#&rlx#VepWtV+9)&{R$qcy11`1W1aboO``|<EF?sNvj+5R9N>g=!<plX8Enj*pW+9M>`5z;FlTOxUve*rk$!)N*Qk}RT7l5(FvXY8}kUbI{v4ES^F19%bCL~LFFA4--NVLm0xC`-}}7<RVjsGg3_dr`dK=^ReayQ6-u9;)3Ct8woBh9Q4nMgE?`8$!PSwPzdl|BCMT(0~vytk2WsmFtlP^Gyb)Qt*tv+2!pn(*+6hD|o*MDe5Q)ZM~+z)=B>8BAR2@({MUZV2K~)*%V*Rig2BDvK&xUuc2I_X%wH2fHRZ9^l&`ZSEwc^wlp;x<C^S`dhNrYg+F>-6f9=TV7-dr`;aw_Nf9M;u~lZl1Nb%t_80uNBbiA?fX6H!vj^8~b>?ua0x1+EcoO8vVm+rvVW<4H(d^LojVAvPzxZXH(l6FY3BAL3RVWvgQ3-vuh*pi?<%{0~Hf9Q?v7y&?NhNLQualEGFjNvZn<Y1_hL3RCY?2+F7%O0x(^@Zo&7xJ9W{u-4Ydk~9@JpIaCa==d%Vc#GW%09U6-|@ks;MF=`T#2uozD?bbSRx957YM=BH6M*19cTc6v!INc-riH_z-Df+&gTKgVwmuWlSIi5D(X|9e8s_5(;7`!NbM62u|gLw<HHqbV_j6{3V^(cgblHm2-z7%m|6kqv?AX;_lIUx?U27vxLd0+0B2>K_ctyc8WC2iwqcwoS#i{j~PiUqGEb^FEjwuy*(ATJY%>8ov6F(_4`KH44ym<qSb1C6_$O&j#$767dLj)lU*5STYvEl@V;m4lgZ>?ab@Y+Kwr4}2m51O{eyiZ0=D{M7Dxk>bl7NNVI^yU&L$JaK~Hgl&0z0oa7xz;=OBR)q-+L+{yGUtf;YY?u}E90s8Bb_cgy$7^w%Zpn`3DA87H9xC@5Wjz)hl2P`l(E`i3i*{P_CUC5cU7OR8Z>nlu2mTmX-zQ<CS~0jiuOKEl~Frs6jW`Zs-rXDA644;s5)mwFZ80{UEnp4a&8y1Cm9B!td;a@9~-(lsT7!|nk}^sU2Qx7`z@kY6pQ{6t_x&~R*UO6Dqa(8)xy33-@Qvw7v!G7gJNvebAn+faNo@;S_>JRX2A=6IgtCgGp{&bM5LZ^7Wn6X(`cPXQ+w-hH$TVMBoq+^E{%9auw>30(V+&OOc@O<V+gkjmURx6IY)f>6^?=RZqsGjZqfDnADlb5>U|P61O464>Mt_kPn45@sJThH*8WlY65zSb~h8^bKO)d7928Yem*BhNp3Nh3+_O-x}+4d{l*Z<tSlYR8XIVN@ai^Y$`3_A)YCd$(P@g>6hWtr}7wu{U=}|g_Fgq0CoWAsVnTxlk<&oE^EU@Tr%wU2Ln(-ro+Qg_prW*%b{$PZh1T8V$jh54QBkOF4;12W&0NGQo582;zUr=2&&OQA7x{CN{8p`WF7<e=HiDUz?RgYlco3~T(2&&C??y9DIF9Oa6DoMZZDpwLUz5OZI)itVckwhm3>=FqRJhtlTv1taKwnAKQgClh0+6?CKGP@!|q|b2Z|2+WvHHNN}rtFlurcPq0e8+$1eVJs?42nMVRo1Xr36<W3o)w=|ayp<&<E&UpwmUgRi8ydp!F-%lUV$upkpx{<AR<LcSN3xC_MIRJkU;Q}$T11EMII=cMsFVtG%;@}7-ljo&+Y{(YRbl?;7P;H`B}I8Y841sk_Jyw%gYPUpB=%8{V-|9XYLeknOpoGzkdDKv|d_^I5-{lj*@kNdbkZ1)6Tzt>I;@!(1_6owqpmrALp^ces<i?Ry>hb?FmO91d28mZtHs^4`w!8OY4s@xia7RQ=2WJ^^QumGP@%Tz-csN$xCc;AF8yP%{i|9JSYE@c}QXNMfqy6q?`QS>TZk`m9nS8CSdEK_`a{TF_4H)_tPgE|Ohg$-qEwo2Yn?y2qgqVl1$3%R>Y`pzo}93{TWEfrJ=5khX*bMVIdl7nxjf9T`D<!`9V;NV{#K+5M5#hjs9Qo)yBL{A%>35^D5D(Mf0!_sP6{av7PoQGdTf!Bia)KXA5Td!dHe|hX+S`H<{k{POYz(yU#qE)d7r7G33R4Mls+0<A4{n9iQNn5;Qi#X~Y&_x_|`#y80@+lWF8-5GqEffZJtlX!D5K2+}t`^MuQVz5c*)~nXfK}7PX_`6df*ua3`5Jx>OABVY%!_QzJVdK3y~s$Ohtw@!*O)mQ^o1o;1!fyGO)zESEv=jz0NJL`k{|xQ`QvI^CoEKwui#hPa@yLW6Sw`d0(kw&Q^WPw>-ij{t3~t?s9?e5Dz7aRBOz9Ek}S~#%0Ibo?plV=;cz@g7bX1cGYVRyaZr%F$W7oK@^OXd#xBYgg5{Er2{heR-4t7^+dfmi+~~T{w3%`00852kf3A=1D?YZrVg$kD<?-3ki}2;it7GLOm}Ffu5o|ev;YfAdqIz3yHX0uu%=-JIPG>kH@pLq-Z8kEXZ8RSl?U5!%qeHYJu^*ZdNjA>cz@oCNhF@{rnh#5J(mXmx<D~2h{toBl1DS_KT10ckfYlGub+K9(>L=_;xJr{{k%ucFjhp}{i{-bitvYjadlk&KL-0aWM4AQcU(~sT$S6Q}<SRHS*2{vLmv+Sd4;ooI2l!~hK~CKS4#W&P_*c$#%KZTq1lSMv5^Ltc`(0td9b{kH$*TGU9Tq(INABZT2X#9;oq-T`{MkcwnCQ;eMFFR#-$FO;_Nen2e)u*TmChPmxHCjG(P&HO^jrQJcPVJj0U4=yMFSG#>@?xcT!BquS_$;d652M4uXlTUF-#G<9PVU+O*okq<Xx)=B0Fiv{V|I)D7ResL5NdW=Zsd^v@9VF`{qX2^214R#(!7(v(1No+&LEK{!zXA{N(h_>sKdVy*T#CKo}1~n9K+t$j(iXQ`#|@|Gr1r*g>!K-#fP}-#_}UQg*T**A59bnVAe3xCet+`jhrlY3MS_L+=T(NapiIJttrg@A*L$JPo>!OX=q>uskOY>p_#<F5zo{jd4#(XU1;X<x*;r2kT|<B05h&#GsLxE8sT)s2%Z|#%Yo~VB^6M;SA`6z<5b#S0UE`EwLgZV?bHr!Ju!i=$%%L3vl|zs64u>7N*Y8zx`q;X9qIsIY{p)oP1BFlgX0&D$gf+%u_kBjhOx!STsH64YFisN3%ng>jlXuGgYy{PPtjKOqlsYWk==n2JfJw>~R)4bugyEjR)iVTq=E^+6t#&xAoLZxpTD7OjYfzph_C760M6gOY(|X6$99-R6G!oYZR)wCo=L2?39}Dj^-JO;;Uyu%4?|HwL<6n?Hc3wf4=#*Aev%#WVh{EQUzcKgG1X*76daG;odtOjN84QR~2Mp!pFiV8g16*v<7E#EGKZJp1wIcI|0q>`SIEDv$K=euiCoVzC!d6(#8gWrZ#Cn)L+)pM{DY&);Akxu6@ze715yl3xkp~AOOUE2Scr*WSKWy^w->^!7_V!b)bXQM%=Q9_WQ%x6g1Y}`Rs5wt=3p=C~i&FZpWgk4>78mQt3{xTAx!=P2&7<`4?=8OJLLDPijQXFO!whB;&%d`q{5(HjgQO@yQwvCICB`{P}VD03p*SE%8JG*BeUs`eWJ!eN=ZbB>as_qPDDcCuTT4==Mj4&O0%6`%$jN49E0Z3{I1NJ(+yXHoyrWwYAfF^ym?AY^r-n5P+u{$;fg_g8VYdK;=EZ3NU`@3PcAW?mVaMS?@u5PLHFV)}MZiNtS#djpyWiebEk{&C?~Etk2TbRXcct0r}DV_?LEo|9u<fm-6B3474@)U-Kunf_Od#Xc?48#&>9H1f7ok#b?lvGeHh;jk8IBMeAQ8JZdw=bByo(253<;)fXZGJyp*r?r?>+>QnCSo1q1i4hoL|boaGgZNyN^3j*}WI7MH9@=W=bhpyA<SQ7xjR-HpzySh-K1E+>@X4Y&-{(yg|505TA9UMrR;*U-ifrx`{n6el4tSQG>V*TqZU8smenMhj+#6PrY_=kLIpxTQ<y?Z3#6Fi&I&%uaQBh5Vc^)jVwLO4SJk1PfjJ#X}bXvwDCVH8+o5wHMioK}WUTQnY))PsLN=A-!2kJH&j15alpWA&tFaf&d2d<{}3V^r+npMJzPKb835Q)DYZKEiGzd6<B>n$s<c!30eeLwNR=cmWAOB9sZJ<EEjlP{`1Z4t+8)7NjXhR-R)a`Q-tl58;@@L)|1`wI6Z*v5ypz-O&C49~9qL8@{BgU&*gmc-<*4kdIZLpzel(G51IP-obgN(?8rl2RX3%81;a1vu{shcA)mg^hBlXdk-l5<@ay)y1nrv;9qH;ptcm7cxW{cbj-cM0lysu=O&OoNW&f6G92DP2~W6P<EV(*{4k(kic8`LMG4#T?{j@yxcPzS1GI~)Z!&bmNg8bjEJOu=6Lzr|B)%Y-`8SosIg{K7l#<cbu!ZDL-=GGNec(1G@$j@rr|*IEX#fbq{cf4ipx$TnqHXy%zPlhxs^W!f{t$XVJCKGhs(Eo-34g~6G<N6hKyd-$ALUw8D_5IEA)hSqdT^Ru@xS>yjtq32aCx0S;oua@6~oeBVav@C8{&bO^P4P1fdw9;0b$B4n7GEVyyyBE2L2pLP&@1CRPJrubgdn47%52ok9TDR_p}2wuDkr5;z1=QP=w+8i)0Fd5G5S#Z{C-Dc$SpJrA+HAfhn&D5zaDx!!hJLvCS+yj5~tZ25jJx+BI@cZ##RX0C9Ts{c-sH@!M0-W+uUU(1(Md*XLq$MUw#)zdhoe6zt&GqrwVX@8C50Kybad9ce^u6Qdq8O(^XIWkPeI)#6)CI%t>A+1*KIV^ALRd&Vl=OX*2mGe(K1%_B4F%c?Lfc1Mx=XZEe}>#HiJp)}z8!7)F{$$Zn>6OTD>)=(nnM_KiRv%M0O3{;zxEUSDgBmE{Nqg1{{RyQ#?A3K=XLiR@e5)&BwxQtf$Wm<$NQ7A_H%Fr-{L;}}892}py?Jd@?qKjtuS7(ZgX&E;3E9v5c_a&{E|H|v>tSS530rk3RKP#iUj1P1(#KdjDXQ-yP;99+L@AINs4@|Sk)wL`>AHKdSyp_Hvd#?5p9eNCyXJt6WtKnFlaVVR>4CeHFp==EPMHxQp>4R++-dfriI`lctm_`PTSzyqCzL?M&PC>%glCNwdkC5&jibbO<2zwFu{+HU>1MlfEV%Z>Hzi%kVuxlYVE#fu{Sby>AE!=jCw%vls<zdS*=+ZmqtwDFdZLFVnVLGl`6e2f?@z@zAP4_DQ;CD@|GmRAPrYKtZ=%JF=ZpCM7@z`4Y)#R;CzH0GQz<<ZjScgOE)56HvzT2*TE$q2ML*L^XIt7Y%P(tiOWYow*YF!C2=@IB?rH-!X^r%+R51DeGud@#rKAWX$yjPkfD_gTK89GQ|C4Di~URORQ6X?6qG}V0^Bzp9HN5(_tGKTRf-vBXlD>(MTLj>NGL1;pKq+L?`%3=k8G0M(sRuFwi8-1A}$zi}3FKyca1!lPsbkHIY`^WB2g7IvMVypVZGYIdDI&(q|Cj-TP;Wh%*A+ZD7b9gIa32sj;LACJ(+Ok|9iUR1izIa-4^UJ|(Iy&edbUNozJd6e#%r8DzZu^U`H9fD}J-p$=uIVwgJul+PgtH%-N^0`kS7#X^r>KT~Po~ewFKfc?ZEkNAQ7jIs$>Ld>7jA=vDV*1{jcnZwgKP8^l(Xbc1;dE(44B<Kz2GAiPmgCaKwbGaQ|c@22c^c(I`C-uQVDy`)19@YqAX)vr)*4ZONC}rJX#l*X_ovgl3~8>`<RZpE<Wqaou<P&u;#><h)Xkz=wqlwj`L6ilwa0)@;9NSAJRJ|ha*eNRqZ%$$<Olg*;aaVAe}o_NVVW*dQ`a}XCZVNVdKhcwuCH-z+OoqD=1;xCF|qC*b{~b5zZvE-NSYdvvuu*n7A)+Voia<mGh8TuMDpiVHgU9x42Y<aTTWRL&+v(m<^b$r4b>&>BA*c@#;G^ihspHwCyEu-1(6iAF6KrmhaN{m`G^jT>&LchPe78(g2i~e;&9mW~q@}EdG@Ss%^evpmfUnymnuji#YYDmv$O-QT+FRK+?w0n1=TfLQ}mQB`yp+@oq~aIl@)PZp7q?;OP?y&bqYh!;JIt0*@i;_s3Kp>JPfQKIElkN$q6Gf{e_T2EAh-)#s;qYj?&Tinn5*t8W>Z_lf@E2#BtRpplXWsP5(q0fqOt0jWmR?^BJazb`|_2tD0UMIls_2unkA<08W~QALauJGv(B6Ai{_saR5?!mO;5i8$#Vi%>wP2&K!hk4x98Eo_ydknI}M!IY^sRDtI7?CsIn@i!;W!n2cSe+^$9y?TABn)~Tj6nBGtc44Qx>AsHZ8t@iZdQ9^`Y-nthvVt0yCJnj&oXkLF@uZ*8?oDn={=k&$0R{sNhX?K6pc)_27Gpe@*8$i~iaq@z6K-ZTxe5OB@*Wcxbnoal6@3?tnzo@!E)NHfdTulW?8TALb6sPtV2zb3C?BkqG&MDK*K#)cbN?NxQjb-Rv5m6}bi&!{mal~Fzo@3@(HqXacF^Mo)^LBHlG}L1GphWN+wZ?EWp2SL{=cH=SY*q75{Ltd1<8cVXqo0lEG@$ZBg4Z!lHsVk|4%afCmG(A3{gftr}27monx8$*Zb7JG{R%If2V81b|AA19n=@RMwS3a>M$L3r{A8u`Qhm8^R~GfK7Hel3kO%cW7a#*B>WNL;zpH2Hs{7AM|J_1`3Ys<9A=Ww5)!}32#X|7(EUS``{+=boH&L<8hx>U@ZU&8-Epj*YXh*&%r8oh(4*5D9dz5Hku}qu&FrxHvP2c%2t0hFf^MF@eg#^_cl35a`0V)A+3{QTQo)Xm^C?ESVzq}4WFGW`X#|Qn;p}j?shk9V0Kb=mS`Z*l|Cl}}(myA&-Y~7!=4rM5NLqUP!zoVe_<w-j`t0oWMoP-A^6F)?&w)F#|FDYviyPmyP61aRwq3>7mCep&=Z>3+zEEvYnUjm9NPm=3n|LmAzbU3e%d3lr``taetfO1VPT0LZcwNM3IYo@t=2N0o|CG?1@-PBrocj1WG_qaTa~O^e4=dUmLGIRd5eA~K;ncl7r3<*TTIZJyy&sobTNRc*qdc>66`D~UfD%g^g%>A3G)ux%h%emfVs~S^VW?4_-w6@>m95pa*>{tt<wR-Wi6Hie55@fj>!L$nAu8pHKyY2tON}we8{E^piKwF09}EwMQKvH`=ZA;WaZSIp0J6a$Et+vpbdP$X2OB-nFQXN(J*Q7vel}Ya)Tp+}YI;Y9H1yf>c)<-%^<BKvmJ?h~m1Hqml@VYP&6B^8dLHXd-d&$_x~>cPw(wa%xn`0+4c{EU4Ns3jihS;L8JG82W#{S@S5H7W^&GQ^ZJHlrByu^Z-e9E9`)~uxA_ISn|JN#HAb&fFxAGBF{2Uxm-|*<LYx#x|EL}O<DXdoIxMxqWEULf@Td;|lo-uaag86_Y{_oBC>n)D)IlR99>KT8GyVokVQRU|?>zKG4yye^4o?WH)Dwpp5h`Dt4_YIe>PKlNum(|(%49?qa$dr3-H9ECiwvIwKiW*QBD{(n%9pBoTga+c?+_W{f3XMbQ_73PG_PYH(=QfrRM<k>|DKn!dDak|WV+*4qT_n?JKEJYq2Q8h1X1q{?|IA=PE!TH-=;Y^gR608>L-2i`M4iTK*hCaUolIVweDm#D_~z~Fue>&qRhEJ_y;AIRc@SX<Pf|FI2Yj=AUaPfl5*rL58|L!II|WT%S$YF|-N6`BCHA_*vCk1BIiF0$3flkla=m;{YwclGO-#0n0k}~d!&`Ax(Y<Ycy-eq1+Z{tc@@^InnJORM8-@DKTIxpChwC*jR1vd1`2R<A?6*%!M!1?Mg`BmtPN)X4_(Qau%G}p{zKj6bJ)d6pa8%`pxdjUioO?oFU0>Nbhat|3L~~^{G2xGPzl2WrOAP|>I$)Gj_hpjja9EZ~k;4bGF)5JYZ)w*vILMq}l{N>Zm8f1c>YopX(@y7n+>4{(VLh{h3a8rgptj`3hb}cf^zPI6;IRytfm*j#=7nypL|SjBJ8#zK@7{%`eb<Om4`jsIU^GB?-e5GeG;?uvT~*F+t)6Rn(1aO#qmpf>U*j*VJZkFRYU+)c<Zrg1lRu;<{NV7A^H*!Q9>v>CEOr~++m+{&+fr2JMT~^2*%}^YilJ>ZQNxn$OjhX9P@QS^q!ME%&cytZQ)iw#w+^~Rro7UlLgQt6T^exO+c&Sn+G|wZr%(0$M@cM6S5Q&-1(YX2w;odM{bZnWz#`y=eO?E|hf7Ptm-&ist>kaLADBLGZHStgs1<VhOYKb5d<*H{mVPvrO%uKrgL=cCD2r2m+g4ZX7tE*{Ys)|3`Kz5T`R&sY*7|4-M=AsJugWzqCbGwy3E8tuHtk%tfF=Q%*awd+$|I>t>{e|dZo650qf~faZ4#r8ee;0V)l<38eJg{q*6{^FPz*vRj|w^*tQxqUH|SV9^#A4cO4#$_GxwfX)5%f-IiQr$8g;umh1}cH3kw~`mQG+>U`ahV*<G^zA!7b+klGe0D6mWIi3v)u7CsqDYT!=$T%v}Z-(TIRmKTCmtI%a-_SRK30=4)Q9mzb(w6eK~it4e;tfT%wr>DEGw35RhoyU!C2#?$NZZc7}Mij?Q4~;p?JJ_Ak@;9xmKlV`)tETf$O4^!|23=Z+WQh4*9dxwKYAP`%>qVnk&4WBuJ1o#MN|k$B9^pD^-J_g-`e*z<&x+{R=9*pxS7VqSl4JvKZ8>mpO7ZcRBvW@HauFT{y%5vScG)_x`UtET#$e#fKCNPa3p{xURLLvD&3VFOm*w<s*$TY&uYabaR&8CI6^XVh;*BjnEjctEj<`}`ExrhdlQwm_SZkDdi;R#E+0!{;X8JfxmSLL3$Qd`fo|K9oza|A`Dvjnnc#tq5tx^#r(aL>DM&H4DmRi9G)w1oV@S+>!)lBZf?o2060DXWK4E?7yhBaIJR$bWoj3c0NW8X?K6z8_uM80}agOK@QYT7=TMQelUQ#Rt3&;9f1j7;}CoqlhBcn}S1T0ToK8w{UiL+UiAg}Ukg;m^{3nXA^!70o;T+Z*_SX0+n+tMyDXcVcPp$>cT2>@U*k`&waF&g7^ff1f|7s_M3@6OP}os=P2>($EUq^cqygQSk>NM{0FS4UsfsPg?o)cGB=KyD^`LeojzwiRNaSPzIU5w32PgU+Fl!={dxM@1yye<h(})8P50&WnQ)(Vld(<UmNrB>69#^EJ<A<TMVrAT!64%Xqj^Ff<QZ$rU#{W#HOBLMnmZxvB@A;M#0s@rj<$c?RPJZPmj;S(`VlvzdSM%rs^qGtJ@BE9Rj`W^OLv77{vcWc>L;n5FhgbPrKc3j$RzW;r}dD+U;rpo*#dG^xccI&;nEfK`vk9g5l+lxYDyLybJq%bS~`kg0wWZ`&*v&`8U(qk3w#`xC+_fKM9W6-wdDJDIR9&vJr!9IA!^Mclk#hc)lxbaRx-Z+o!>%MfMU5<oL^Dcm8uR20<3;()xxXvvY~DxFBQ6AH5T8FnWlJ)ExO}oNUlH9PxGT#*KU(JlUzbD8{d>vLQ5#A+=4!o<}(eF>4uUk{wVoup|ymT*-94F*O>r@P75cMG%@tTksM@naNo6QOTxqSP$j&;CCtAfO2i#DwEC)Hi#E#6pO2LvVRMAIp+>?VN7;Wy)yFioHv&%wCC(aG-J_WYR{5({PQ*G@Lr(GS`iF44k4U+5Z5%oO4KkiYK(E7G5PKKaX!5yiwJ+)H8hV!v`l888qnLG@(<QtWV6%tj+RpLk1S&$yN7Z_jWRFbLv5(4AX{!84yOFv|B;HJZN4ihy~bBGFl4~;CJ#ilcGo`AYJFI%u2h~diC5i-*r;QDY+Pq@l|{*w4jz7if_^d$esrF;KdG0R+#Qx#P0PC&r3=dNZa4H!OSDkb9ngx%y+g&_%^bc41CZG}IeDEdsk#G0F37U@UL!sJjvrVJ*)^<NQ_4HE$=4D)dK-=N<_(u%D_Wf?RMv0{8KP;)!Wx$NCQ=FaC@J+tU_Pa!*`fehJ;_o1rK+wEId-YCf@Gavv!uXNAZv(CP79=9AROPaz$0=D!3-{dLE<IU;xbwWe<N9HUi&rfAhRki2EG?qMCgLhVmP$#2MswGV7CctEsm;5@EiZ&dbjDUvtzDRP56RFjSd{+FBiHjidg};JgT{60eWB+^Zw%NK_il~6UVYu^sDkSXV;8^h6j{_M&tIFUX$k0Bf9t|n37#!fEhOdYiRCg8{4Y>r09)U5J%7UuS-LmmvBO(k}+T8a(&QkrLmx=TFs{_xYRhsPDjb#xMO+~n`>L!NUIQq0Tjsm#1?t*B<PuGm{rHVC9s%eYg=+O5j8OPVsF>FIQQ5pC8ds3LQ8U`IO@F)I4VG-h+ur_lE|VmNiO~6n}%#(S50D+Cq-zP^p#re_$tSuL?<;LPPwhKq0gThtc*LP<!F~WwoDXZDOMO1NXuw2Ym|AjEo9yULK#xavBikYc)gk@u)l3|S|~LrfsbEH<XvRRqS36NiTj#{m<0BkGb&!Dl#*%3x0#Drbaq46xs6@3(dvsZl2Vq@F%Fjh{HyfiWCG;<zM&zIs!iwVM?&SmM$-kr&WTPxbNUN>snK;rc=o!)NS<azczzYWCs#@$cx9T(_pjeK%DG7h>b!v7K;boOzir3%wdKGipa)2+#ILA)(kd~g7@~x!t)=3Km348CMhkQ_hDJq53%D5E8})|uE3z)KXf8?;tx>K&O4`2icO^7d22Xix7Q^#Q(OC|U)gyebKoS3}?<bQtJU_rwE?vBF-57U#$ER9;sL#FPOM#a1Qw33WbwflQ6J4EBN5!Gd+fydwKA;2h*wbtrEU^a$<1k_9utx(*_f>J)8j~GFtCLe%4#;U%;Ql1F_8}#G?Wto6=-#aIJM)U2_0Ul#iKc51l{C<&!X+(M2jdWf2nPrI?Otj3^G-%nHWg`BN}LZ(e}G%%hK3H4cgrX&g3UGwY7D+ski)XByGUivWlBWkfe?YbFkHgl>{L`m0@|cBAhIv)^UN|sDXZh@KxqN+)%I@ij;tVIRY^$NiPcU$3QFl9H!^kb*cSa^G@m<97nFwWe#(!x5j)Mbbjh9e+~X943`7{cR)$iYX78<*Lv`z+s6T_z3fzgt?U{5ZF|1k}%)DJB3)27?;g#cH)Xy4axtpK_EIxJtX0eIVI&ApUCX?frZ_a*r%;V<=Xvu92$0Gi;=2pK}jfNmDIBZaRRfVp6qS;;z(M;QVN$e^@q}8Wk+edF?<`|JSMg6fwMuxJkB)yq99aO&RI(ljrb+0FJb3+9tJ|AB_CF6R4z3ZxqELD|Ma=c5Jw~sjqctmq=MAOT4yY2p{CPi_3U<<=qH(7mw{d@Iu79_=EVA*k3R^QQ8I+r%b<9JbbVAd}|u3CJC+3I1*<yb-F4)vN~=ekm<eAxJ6@<kO)Cf%v(L4}m3%6FXc?Y1jgD^lI=lKo!UUX|r`XY`LSO4LuTZ#hjx_2e~Wjalxp$B%cFz5-xkcEe=pXsVS5Zfz^INvTDnRa!!kOHwRAo0`u?hkZ-{;=AIhRt4h0C0M>04t3#i^@OyE*1F3CzjV8j9blG30VDQPID;9GYy}gcfka(j4I}=cbPF`C;ziY^m)Q4?sH762D+ilHxX{91Eg6F?zw47}x?b9MjC*?hrYhXa!xSd3#lcF@!NCR)s+{YGvcCaJ>1gu97>J7lhcpxYe)rIm=F^PXE6B$JZ@+_#6s}+ggy}3SE=lMi8CU4uEXndh@fb=UAt!R_D0I{A_2b=I4nWIaq|g#~3eu6N*tEY<SJmyd+YH2TM(e9wF91#nr>l&C)7pu0<lSy`7>_%h{%AOu9h}z-gVP~ZN5biCc`@=XEk@q8iji})?8V4qmx`~T?Ws>rD~huc<*Q^x=E;&6^~m|c?#cU*@om9+t}A~V<(H?VPy_Ys8Wc54c-5$7?#xu?uOiNR5D%Zr9pA3Vo3^3i`GyfNz`kPU>IhtQzo-d?xswhI|E#Mvw%{Spg#p{j&S)g-VuFshmG=51muaB1mMuri6D~d2Amz~PEM&S{86Qi(pvoMbxsT`tibKYrw?I^NE+x~bWM0s`maB`ZLhPB5oUswZ*AO&fH8g?%8-nyi!{d+GFe&&VR={PIj%oRs-u_StIxCT4YE;)Kma?b=TAH1!#;V#ZH5gO%!HXEr(#0a8Npt7GJY;!y1!@uI;%UPu|Bi9=OHv7zUUN07ZUw)YC4bm79QrTQ1iWd5S$GMLxp37aa=AOSpn3VNKb6aE$Kt7O<SQlp`hA045TLH^4`lk_N5teYX54}0v5Gi})<1blF{AaUbe&hh6A2O;Za_T72U(n_^c|(!@sL&M9@nB<M{vaTCNdINHh_MV%M0~LM_whcmPx<a|Cy-8-@Sc>;dTU~-z&cxahRDi8hrU(88v+W)O+GAa*0-@Y4z|%@3;nq*y~S&fLwk0(t>NzX{HGbC$29N%Ux8(Q_AFbE{fZ*gUc*_%IfTGxoy>KqheXi6r|3o@(PajJUiTpCwV7_g@0S`uB};^#znqCKhw_gELv6KLf2q!yMC=(u--ar*z$7Xid;~Rwe+?j%H;^>nzn3q-Kxv3se$Q;UD0PU1Bs=?BV~&iTZQ5#f6m*5@1<y@N*34ZmArl*rK47Vz%Sp{IBc}5U{q+uh!L(QWo5Eu9*QxAsU*5tN@5O|F$Y)$D9tQ*TZ9KNyA8c#Y%aLW$Ivuk8hGUe$N9Zip1!?ee^iXk-mO;J1Z(4fKu2%YI!sBMKi0>s0&}m2f;3MH1C>^G3sd;y=AQvm!v=$WeqndE48)>`?B1Snt<Qm5H8NW%f*_)v^u`VouHwdXX*I%dZL48M?n~#;oGIRAmJK8#l?#=VmrKO3z%PBfv+uY^&2>I4#T4&I7~gs$ny7sUtI!+QK|gbtYbml+Gq$o<_SAR=iW!cQhMr7-hHx!uzC!az+&Dum1?|^%-erd_s#Hj_nyE^uK1lQARPBH4nA6HQ7{>y4b#Z;xbIR73tf~&nd2EU`GyH6dgUwzU<)yMJw;r!^aaxs!<)pHNmA*)>qi;K8gluxx&YQ)6oi(>7b(oeekS=w0op$el9%ftM8rnXKmERw9+kJmE((9(0Fw9x+B}V)9vJku|595@Y&8L^q@`6ySIp<dMG+nu?zLZzwZ(H>Rq3~|f<s5Ixl`tK3RFtN!x-PtOR?F$5F&@JjM9I?cgwblDD1+kXXTvC`n3HMV7<8MS(boCQ>4{Q$HuI>qwGPa;9?zBoo;Xb0@Wk8t@L<`{9P1n$zEV7zZZFOZlWnk&{P5Rg8K=Ku+HW*ihqDaCh!CZ1HMg<HlM_)#PF=dJ=jq^n)eU}*vK9Ao#E1L6ct|>({`nYwBQ?Do3Y-l-4y7gcaP+8$W2}2PdJ{>@s9MN7S@N1Hx*eqKzWH{)S~9!Rbzs=&@#4SgBhdy`UXG(DW`3Eh+8ajNVR-~>--rmZm$?dro!_{jZ(Z)zvj0iv!CSIQ^Mu`@G(2f_av6Bg<#l&Bu*N!(-inrbX-mvSz3P7ViSa5l>aA>8xA5E7FOS31A5PDXUxp{oCvuR!h%)rfyFk2oarEl=jDipd*bq%-_s)F(26O=Y!|~hj>myc=5Ic}Kx5Eg#P3@990(f=&-Pzlt7j+@@YzSX}clO=e<M8ODE{whhNzY!tc>U@J`n8}YtAv;X1usutvG2b<`s(Bi{(Et9_QNDtAHc^4U0Lv*r{^2-=ntxhJChqAn^ltY`Xb=u-A|VX2cNT0F!lanyS&C}g^(-n^cZ=jEp49+*ciSm*iDSya8GR<yc)o!d?Tg8iQ=Y|Z>4jm+ycs5EkJEzR*0fk+G$~<gMDVSS7zwWaB<rl%ASaDp;YmiY>tss(6voKbj#gI=p(X?fsHO4vvrc^iaIWLT9BU{Q22UQK{1x(C$^9*a=+K@wTJ$obtU4k>{`*yX((n}>L<{-&E(a%att^z`>?~w#mxi5VB)=Q<yF>QJxwOZ_#t1#R{+z9-@e6NKTk(IN4~h&<+*ZH%8-J_>fPAm*rAb3@Im(u_wkgjx4)O?nY<OGpg{c0SL|x8`|U<V-OptaMJ=NAH(#|$zDlhpLC@cje&tmAaKq(T&xE<*O|u$4qk(VL<E(iv776hb7@rX{-Q?!ydb+XmTEq;H>b@88<VoPUgrSsDaRj_>MwT;V#*vNC*?T3%9=q06Bc}(o$Z7K&w80}N;6T(Ycs~=-=CRqFU6)ANm!ARgHd|NDX>KBYV3oS{(AV}e`!-$hFetsio+pE#Ym8p@cN^Bn$HGoLs<|K1(4jtNzaL5&;5k#t=po{g2X0*w!#Or_e4EUfFRXEd@~4#na4i=&yM>=Q(mIt*8T_>&lFp8g(>9)?Ul+Yw&3wcoV#M?X)uZdIG;x8Ff{xh*FfwtP6z$rkD9p>h!45^6a3fUzU^a}q!%pY$yc<QsVQnK61G3Qq#hNkm(+I6kV=KINg|YuRreUcY1B=$5t0?16-p)Omq*yE|{GG08wHuFVN`&t+{|udGfQc9a*fDtTga-7M6;6UkEhN*aTQs=BGPF4!)|!Jf+4%EX3R{sClg2YBo`@=O{BQnFYAnyFpMZx2(Jb0Mo?%b#b0>HN*yOp2f^-%Xmw5A&MmB&Pytg0==!D}HQ}kpo`wsmINvHg@*9)+G9g@%<udc`Z{xpswr^F&k+hOu6HND|CM`$fNJ32cy(-ro5bh(vo`mCD^0w@QCjhP_ZP77rK>Q+QMHW$5C*Zs$LVcc}7zfbbyJeeoO755MI$5>6X)jw?a^%B1No&8G7iP`C%qWp6()L$%8*z2#JBTj1DfV2zU+gX2VWJoY)(vs+~%RLEMf*&qN7<=SDDLk*aAC^Vd>ZcXm!}b?u1$h-&!kSbX46oZYm(H-+$aL-M%INut_H3JslB0-z{M2c=QCi{JKh+IenmNct2f|^+04eptRY2lMnCL~ZMB(aozZbN|T~6~<VzOqeU8ScM60gytly61uH)&fDRuz_WaL<2N9ThI|wIP*T^<(RP8Pc+e>X#^&s_hjy)squ3qLqbsDh^FzmE>TvEF;9BTzeW8XYd5){bw`s#dF=+hcYH}mv>Sf04&B)<_OW_Gk)^in=X5=xaqgeJwIBdLMqHUdvt~`BuYLVZ;XR>FWi9`=SOy~9#6wsiF$Y$C)YJa>UwLK0bSmZEE(TH833gwg#-9*o+FiaoPoj&IrM!El97CAW9<UYh$-MyvF$rAp!0UME~N9eRTtD{_2-Ep_AxUgLx$=j5SLK8JXpKmZ;!fu#>k^)SrC~~W{#VdMGVkGW?I&cbjnv>ukepAA8MzCAoE)Sp@)f;2(&Z;voOs{vy?1TZcfPHt1LdD%obW_I@^J$oQ%u7(KO<BM9C~yQN@y88E%zG4R;MPGt~lrY<dN3A?c|Z21IR*w3iTtVPR*KGvoDKHkkkHj->6630e$Hg$3_HIfl(vFIRWEVCU#hpSNM3qR6BcsV5Avx0|sdJXXwAL$K^w3pcw3qM3Xu!TF7LEZqMApC~4O*{;7*;&tYUa|X0?Vii5sOdJPXt)b+1(>9V)ZHhA_ulP+zOZNsU9#*E{sye5vbQLa5`o7^AiL~;wV^HBClwY3!pTnX<U!GtT;FI8?Dk58hkp;YJS@exETogcw7YA3eYI5n3TmrW&3{_$D@`9$y8jXnr&WY%J-l#cdad~>~^$#%hSMQ*w<Ug}cJ=1&2=cx$L4rXqQc+ZXcJs0Zt9L|)A3Tmzcq2)5WOR{NpmgJ+n&u>o8bZoP-W@3pP;R$r!dh8H4F%0%cTB%zbzl!4NkmfdqRtTGbsgc^}*k_8k@Jd`fx7?WKmKzSbJYZ(?d<DdC#mv7GU-1xj;8T=R+aI;~>p(>WFP*qbq$X3PWI|h+DP*Rziw4GAqLn}mAf_1KWMH++@0fS~#d&ofrF|T*Ic3v;zx4`_;XlhzrY^mZKHT4LkM_Oc(Hn!KL9PxjBFYJt7$%&DN;NPua@vlK7DfW#)^_N{WI|1gC@t0skrpbu++j@Ye<&>xzR+PUd_UhV<&9Cx`%Y@`=Sp{D%;IxSAzHc;&N`zGL}sdo4`m34J<)&I(aStpwT3Sx8FT-Xf>5Yg#xXY!li@IdY5Whcb8ItQ;I3b4meNTxl3Xm^5uB~MJIpORvX0yCEO=HbW+vor*c#ODN%Ff7g3sF;#b#kYP(@;=`AF&(Peh_kBtw3C@`*sX<I>!15zQgqd6K&B==yr&od=K$&S|v~CbncCZ$<N*M-pNY(l|u%E}Ug>A|<Z6lXu-Zn$tN>r}-mz6QZdd7z@oP)mg;Bx%ZVQYe=F)a=72=%;J3#54*aP+50hbE_+|I;K+V^5VY_|p9-GbWtRuhD!ci69xRdzcDW{)>;!wkyVIlZr3IV%+uj9_=?i!2E-Z8=T0DDu^!3@%S1*pkXGdp8FJ6D6!g<%h9lq20)1QQwIQT#R^M3^(!>zL9=2LO=DNu@SVav00T?Fa*&&XubJiqxAQ#T^{2!D|vzWJ1+Op)ZUh`~EbjPF2iTEYvw;R3=&5QD4(6DzJ(a4LW>G6f~O0}D-r`7~G;$vgpQ?+jutZEP?(`Ig%n-tDw9=$&LA2oMn{#X0np2Q))s&PJREc?3A7uGyRaghp`8WJpv=G^dDoL6o=neQ*J!jmxmYO^L4lD1(_1G+x1HIa?#3*I(8o_+_00A8tNn$t;1H&A*_jZOL*<5Qp<G=*D6n0N{C9HsI#df)t=AOi3Fg>;y*%qsTzeUZse#DL{)pkSR`UB~~U%fSfd!H%}=L(EHlJB7yD*{C4xdQ_TN`nRZ#;AIdObeB}V5izElk0B;Fid_k6=AEm1m!BI}x4SJX__Rj`N))^`R1x51;=exN1dy&B)<29$ELy1ip779yV1k=kXySVv8QA=m~PR;uJ4!S2IZ5hQhP|Yl2xATBg;HcM1;fkNrHI9Yb^#k=4O{9rVd2T?XOocFFC|jKzfquf904~+xa><zRf%9MwI&@0iOIY2TPt$e5;v}XB{XAVy=ZJ<cxU_*tsM^`^8aAlyqmnzk<=FZI@xr_H2e7Mc7AUe7mYlDA!WrJEHyV!)$H4H;<I!jqI~bnwjl$=Yr&M*J#`+;wDZk-r6I*?@`NbkfYh2?+lwtOFP82)AJ5L?NM@S)2@F9W?i%cg*VoiW@mW-wmmv$A#cer{$xRmkSeBu|X@FZSSBQy|144e#3q7ItY6%r&Y@d9u*;Dh*e@-dhtz|LV!gSUX|2@<)#Iaw8E`+)LfS$YkrK9~dvUi|~Mf$R|geGoLmIJi=j5gSv-2t!wZvS&M(CyS(D{0dd?9AQkOn6jRjDB($*a<oksOq5${$du{l3bW!48Nx@S6+*<X&u{)6bM!3>8d`GmU-&a#XSi3|0h!H^@8W)jC$!Ftn3h)Gjl+tv^NfyJaxeiCK>(?o&(kT)UBFOky>SSf(a)lDV5szsV^qpmD5~g1e>gk<Dv<g+XGK8;QEhQj(Eu-QU_nSFDmd2GA3GG|n;>#twjE%gC1kODR*YSj$3eg-o+E-jp{xv!0p!^D1;N;Tyqk(&!^yZ4&{GdK2YpRBG`t3KEE3inlV{{;Ac_E87kKLBxQE%!#_`b&I)eehD?GSpAkToNARO}47dU2x;5moQ3%*#;%OY8>^C_z#K@T1rAZ()<mbhZ`f}<k@HW!@C`S)a*5je&QlmX~u;Kapak|F^Us3!CjSmI`9=OG;F!Oef*_Rk3HFF*+(fOF~9`g|_17Jspp3=5IovL>)xFYp+o*oe#Hg^ml*1bc^u)A1=XS%){;X}x2v{dfEvgTbt*9J6ExEF+9gpdF^~xN80mr}R!}x*!3q$ePZLp3j*)|8k%-GM~FnGsI?sf<{7XgRX$)kO}})mcp_ocyxmxfAjYRn_NcbJTDD?`Nhdmn+a9oILs(pq9<XX9)vLIV41?`fLtUs2kDxg%k;&Z3S^Qez$oG5XA2Yo7El(YJuT80C9G^Gc#D~F@-LFPlo(;9D92%d6^4Po{(SRqu=5j2$c$ncS~-u3$&NBI6)6;Hlb!zyDVFf9"""


def configure_shared_guards() -> None:
    """Configure the proven MVP-016-B safety helpers for this migration."""

    base.BASELINE_SHA = BASELINE_SHA
    base.PATCH_SHA256 = PATCH_SHA256
    base.BASELINE_BLOBS = {**MODIFIED_BLOBS, **DEPENDENCY_BLOBS}
    base.CREATED_PATHS = CREATED_PATHS
    base.DELETED_PATHS = ()
    base.EXPECTED_PATHS = EXPECTED_PATHS
    base.CHECK_COMMANDS = CHECK_COMMANDS
    base.PATCH_B85 = PATCH_B85


def validated_patch(root: Path, embedded_patch: bytes, *, skip_checks: bool) -> bytes:
    with tempfile.TemporaryDirectory(
        prefix="galactic-mvp018-", dir=root.parent
    ) as temporary:
        worktree = Path(temporary) / "worktree"
        added = False
        try:
            base.run(
                ("git", "worktree", "add", "--detach", str(worktree), base.head_sha(root)),
                cwd=root,
            )
            added = True
            if not base.patch_check(worktree, embedded_patch):
                raise base.MigrationError(
                    "Le patch MVP-018 ne s'applique pas proprement dans le worktree."
                )
            base.run(
                ("git", "apply", "--binary", "-"),
                cwd=worktree,
                input_bytes=embedded_patch,
            )

            if skip_checks:
                print(
                    "AVERTISSEMENT : contrôles Cargo ignorés à la demande. "
                    "Cette option est fortement déconseillée.",
                    file=sys.stderr,
                )
            else:
                validation_env = os.environ.copy()
                validation_env.setdefault(
                    "CARGO_TARGET_DIR", str(root / "target" / "mvp018-validation")
                )
                for command in CHECK_COMMANDS:
                    base.run(command, cwd=worktree, env=validation_env)

            base.run(("git", "diff", "--check"), cwd=worktree)
            base.run(("git", "add", "-N", "--", *CREATED_PATHS), cwd=worktree)
            base.validate_expected_diff(worktree)
            candidate = base.run(
                ("git", "diff", "--binary", "HEAD", "--"),
                cwd=worktree,
                capture=True,
            ).stdout
            if not candidate:
                raise base.MigrationError("Le patch validé est vide.")
            return candidate
        finally:
            if added:
                base.run(
                    ("git", "worktree", "remove", "--force", str(worktree)),
                    cwd=root,
                    check=False,
                )


def make_backup(root: Path, patch: bytes) -> Path:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
    parent = root / "backups" / ".mvp018-backup"
    destination = parent / stamp
    counter = 1
    while destination.exists():
        destination = parent / f"{stamp}-{counter}"
        counter += 1
    destination.mkdir(parents=True)

    backed_up: list[str] = []
    for relative in sorted(MODIFIED_BLOBS):
        source_path = root / relative
        if not source_path.is_file():
            continue
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_path, target)
        backed_up.append(relative)

    manifest = {
        "migration": MIGRATION,
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "baseline_sha": BASELINE_SHA,
        "actual_head_sha": base.head_sha(root),
        "validated_patch_sha256": hashlib.sha256(patch).hexdigest(),
        "backed_up_paths": backed_up,
        "created_paths": list(CREATED_PATHS),
        "deleted_paths": [],
    }
    (destination / "manifest.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    return destination


def apply_to_main(root: Path, patch: bytes, *, force: bool) -> Path:
    base.verify_baseline(root, force=force)
    if not base.patch_check(root, patch):
        raise base.MigrationError(
            "Le patch validé ne s'applique plus au dépôt principal. "
            "Aucun fichier source n'a été modifié."
        )
    backup = make_backup(root, patch)
    base.verify_baseline(root, force=force)
    if not base.patch_check(root, patch):
        raise base.MigrationError(
            "Le dépôt a changé pendant la sauvegarde. "
            "Aucun fichier source n'a été modifié."
        )
    base.run(("git", "apply", "--binary", "-"), cwd=root, input_bytes=patch)
    return backup


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Prépare MVP-018 : propriétaires génériques, factions configurables, "
            "autorisations centralisées et persistance."
        )
    )
    parser.add_argument(
        "--root",
        default=".",
        help="racine du dépôt Galactic (défaut : répertoire courant)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="valide le patch et les contrôles dans un worktree sans modifier le dépôt",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="ignore les gardes SHA/blobs (dangereux ; le patch doit toujours s'appliquer)",
    )
    parser.add_argument(
        "--skip-checks",
        action="store_true",
        help="ignore les cinq contrôles Cargo (fortement déconseillé)",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        configure_shared_guards()
        base.ensure_command("git")
        if not args.skip_checks:
            base.ensure_command("cargo")
        root = base.resolve_root(args.root)
        patch = base.decode_patch()

        if base.patch_check(root, patch, reverse=True):
            print("MVP-018 est déjà appliqué ; aucune modification nécessaire.")
            return 0

        base.verify_baseline(root, force=args.force)
        candidate = validated_patch(root, patch, skip_checks=args.skip_checks)

        if args.dry_run:
            print(
                "Dry-run réussi : patch, périmètre et validations acceptés. "
                "Le dépôt principal n'a pas été modifié."
            )
            return 0

        with tempfile.TemporaryDirectory(
            prefix="galactic-mvp018-verify-", dir=root.parent
        ) as temporary:
            reference = Path(temporary) / "reference"
            added = False
            try:
                base.run(
                    (
                        "git",
                        "worktree",
                        "add",
                        "--detach",
                        str(reference),
                        base.head_sha(root),
                    ),
                    cwd=root,
                )
                added = True
                base.run(
                    ("git", "apply", "--binary", "-"),
                    cwd=reference,
                    input_bytes=candidate,
                )
                backup = apply_to_main(root, candidate, force=args.force)
                base.verify_applied_files(root, reference)
            finally:
                if added:
                    base.run(
                        ("git", "worktree", "remove", "--force", str(reference)),
                        cwd=root,
                        check=False,
                    )

        print("MVP-018 appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=12, SAVE_VERSION=13, "
            "RULESET_SCHEMA_VERSION=3"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
