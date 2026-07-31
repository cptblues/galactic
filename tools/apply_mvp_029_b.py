#!/usr/bin/env python3
"""Apply Galactic MVP-029-B from the exact post-transport baseline.

The migration adds deterministic ruleset-driven extraction sites, atomic site
reservations, capacity-limited harvest missions, persistent cargo and reserves,
harvest reports, and a minimal analyzed-planet launch action.
Dry-runs remain cheap unless --checks is used.
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

sys.dont_write_bytecode = True


def load_shared_helpers():
    candidates = (
        Path(__file__).resolve().with_name("apply_mvp_016_b.py"),
        Path.cwd() / "tools" / "apply_mvp_016_b.py",
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


MIGRATION = "MVP-029-B"
BASELINE_SHA = 'a53c440c4c11f039f370b7ddfa230f2a1bbfb65b'
PATCH_SHA256 = '1bb81e721bad2de3c92bfc0067d2e5f5508c80e8cac5fdcbfc9481948fc4d990'

MODIFIED_BLOBS = {'README.md': '354eb2e07e939138bb1dcbe26ba3a29a183e652c', 'assets/rulesets/default/manifest.ron': '0cafaeb739dfd2ce3fd37f0f6a9d8b29544c1dfe', 'crates/galactic_client/src/lib.rs': '894372e0707a7bc670aa74be720a382955789a31', 'crates/galactic_domain/src/ids.rs': 'ad6db0015c5def652756bd37f14bec915540b81b', 'crates/galactic_persistence/src/lib.rs': '316ab21dbd1c5082598235afde6f19ea5e8d40ad', 'crates/galactic_sim/src/command.rs': '2236901b46a349c0c3b3a38408a682c71cd12165', 'crates/galactic_sim/src/lib.rs': '5c3d3903a00921cf52f9643553f856bc525e0316', 'crates/galactic_sim/src/mission.rs': 'b6d347b5f164a5032c9c4638234184ea144392d2', 'crates/galactic_sim/src/ruleset.rs': '518020a9a3af378955889bbfb4b24fae9b1b9418', 'crates/galactic_sim/src/simulation.rs': 'ee1fc89c9f69c850820dd78d726178af5b1481fa', 'crates/galactic_sim/src/state.rs': 'c4222b325fcb92b071a46c62a340c9b4f75ad608', 'docs/mvp_architecture.md': 'd9f2b0176104d0e02d29d5347cb39e9b5df9ebfd', 'docs/roadmap_galactic_issues.md': 'c34f16e4682fc86b2e0c8a03bbde902c8cb6ff8c', 'docs/ruleset.md': 'daee46229d864b75603532a1c524d775c0fadd88'}

DEPENDENCY_BLOBS = {'tools/apply_mvp_016_b.py': '1557ff3f419abbf6a1b58b897100aa72da80bd38'}

CREATED_PATHS = ('assets/rulesets/default/extraction.ron', 'crates/galactic_sim/src/extraction.rs')

EXPECTED_PATHS = frozenset(MODIFIED_BLOBS) | frozenset(CREATED_PATHS)

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
PATCH_B85 = """c-rl~+j1jEk|6rdugIdzs(=QF0zmMFQdKX-q9j_gR5c{4X0&P<DhVXWYyk-<GeL<GX*Mr=%=Yn^?aMLShq3c=RAcAy`~m-@zvQ@kTr#eS1WEPu&UV>Mi^z<O@bK{PaQE=bC?1c4jg5;q55ngshx@PJ9JZ#T;Jo;IElx(!&%x$aFFNl=t=4X5b91<L9(3C6?OtzfV`D@8a;?#5XkY)A{}OC;yPf7{(7^w6wgPxEimsw$5e0Xj@-$2??tV+7V73Ut^DIj8D43_StJ#nA{vw=2Q+OCmq9Bd(*&+=_@Kuyf<0Q_Y?P#$U3@^j<f||90<1m<ChFR1MCSfpy=V6@9k|3I%hv|=tyHEH6y1M(6Ehc%GQ*U8BncRH}A~s4mpU={K4S)=$ahApKHS~}N^C-<=$S}#X;6<=V5TJP)XA!o{!o^i|5vK4byqHG|=y{e5gCxf9MrqVs!@oz1`6Py!r7%(k-)9-jXc%R!wZ>ZGv(JJzAKq=WyL%g71b_Y4{}G(VIkXx*i+%<)4ReG9U<BBQvk6X}K@;J(2bUotY(5E-yWim5#oedm?h`bHXR|E3`waktsb;fu3J3w{gDl1_)V?*uHrE>P_0`=c#7sC(?|#dIFbOBuS=0(np>=*er<u!1Gyw^O*eJciAJ8U^fOcs40B(4&*2tsjJj2h5HKXN@qa?w<;myY|%c4Aco-P1K_{S(3hp<@B3s76>Ecr+{dmYUeFrUe@ILYSGFprQxp5fw1R4}O7%7+1v(C`ul`T8ULN`?_Fq*&)sM6>4T<y`|A%_g%XhMr*XE13Q<d@-Y;pxa4AASFb>W56$VNt8EZI3#=^Wg*6!Fk8gwZ}2Ew0FaAJMH|r|((ODP#yMaf@BzQalUYvR0zhkxNqm)3hYAj%FSb2s2k`Df1Wexu@ax&#C+tVA%LFIQh(5>`=@m?oI{aB7Y6LRLkdy&2f{usOe`{@|@{nqa2P~pm!?h&(DHz8S#5mxBGabf~+U#Gebsp{RwZ{?hBIxJpd9s*HI47z&46><qvmG=#&E77ssI^9Itr5V#E8O+7q#tye>;cd_uG9cmb1+(@adI)>^wSUWbP@4(KLC(fKlqOQ!hf~mPtl+soXmzlTsQSMFa+(We()yB!-?@0_Gcc4lK}^$AN0EIw$bJ~jwYi4EY~29hd%)DyG8?`oU16y=`}#v?J4i;%_T6u9Y$f|h55@xv=rpc?d2eEJtoMnapc?!^Fex@RlxM--g2;a9uw@NVf0X#dt1w4-g``#zY4RjVmOx`L?BzsX}a5eOsGH1Cc`j^hY#F}yUW4u+ym_I;lK52jCKmM7(;(c*sR)2@Y7*94x{r{tGzoOx1&%O@hWvx&U3Y<$Z<Q(E#$Z;6RyFsFr30-tn)9{n`;~REQ2F^8V(9x+~zNaAeg~vrMwZ?wLTFq>`#D>0t30jGz8J>IY^&)MhpkT2?!|p^DG@cpTy^_G=o_#ZNt&JyVu*?LCJZqyEz(0J33mI_rh?xyd5EQr@0Fl#s7t$3y@9EqpRzFp9HbA-~V+@d6>f)2#>x5gzV>&FguO%W`MsAerEqYCDOlrp?>reOltNMeD)5BKFQy}vD^&arSUXOum71|iVq~LqZtUD364&S_6`KEC_frC`Cos6ubROL-)W~goQTcfG!N4Q04jq&fzL%#Z2vxqaoM7SSu&0<ir=S0P!kBCoD;@B&(g{0-3$(C6oJ^vAIc*i1?ysbA0(CGV%=8(&V(zlaFxYVS|<FrpXcH5hc`^Pc)Q4lvnk;EfTYo1Ncj3<0zgHn{?Psal%E{H%G22d6ouph4i%)9y{%qzJ80m4c932`Cz}TE<9((Ma3BtUj&nBCCVOxkE|THpyL5IQX-^om92xA<*NnAk%~VMFi+z;Pe~H){@MmYUiwi{C>@Q5dZgTqG+D4G!zdH=(F=#UL@TX+3h-*{Ox?sw{W>E4v;D6P{OvFFdo>9W!e39p~q&5#>i@^Gb+4uYEG=ueH)D==V4uE|mAW3KVdmUc<X}$j9KHckc4%YXXZk0;C1v@{!XXo02NZ>>G4N4+*{02L2+_z(~tOZ)Q&V*fmT?uBr1^(lM*)Pn|bpj%)*lq6uH8eWAa4@Vw6>Ko^vPGZN6xhy)LxKuYV_Zf2TR#BpM^paN;3jxITbxg#jUm39Ew0iyeGc>jlS=9JpHKrH(VxdcmBt@$#KPc@Z@99_UcE!zm~R01IZ9`6I?d8^RNZetYlSmlfoeO@%#c2q&L$CPioq|l*>v5IGk!myzm^2>_)`cJQAqy1hzEFh4aUIJYMpMYt-i!XLmEkI^uPUnmR_9K+O3`KX3%c!_V9oA+Vwi4onB{e6FCa}cZ;2xNIUU#F(CpQWS5{94;Ojuhv@oz2D)%Rc=`p=@KG{f<e&d3x;}s-@Krt7con>z<2RoRihZSWo>Sn(HP8O9vu3wJG=Z}q9tUEu)?XGN8P3th5{+tntbQN<cvCOwT;2b5Mg^<%{WbR8DD9iGfO`P`&2qri9j=e?D_gVf4xIwM?ryu;>G4Gi=kv)mY<SLT94y9UOt)*wpA8?s;ZqG>zEZJ5qUtyRT4i&n5z$`{Vh|a~G!T#K_26ZYMY!Ff(Ljj#B!B*#*9sbI6@S+1b#oB>$EK!GC1+VRyD2eT*|6zu@KrdC+>`t@u;&rnI=V5>4+Vh=TFX|JAvCCh&>ZbtIpWh^REglB()Us<GdkMb+#@>L-0H}4gP&93RTxh~V2w4`Yzm<WXE6<P&^EFswIsZXFW6{Pl8aAs6x%Q2;fFZ-=?!fG_E~LES9z9G?7*(&?B@7n>9ksLHpmcrWt}s=p!FWoqm{a^vR_sv{z~vKA)@;cL@~0;6o^WC-7TV$-sWzzD~|{d>e$ynV7Lv|8fr@u30zT?P{7G@s_tC%NU2%M6k*{Sbr(~wsY^zMn<>kI0+A6Z6URq2v4d&HMO-L2qn+08vvUd9!JPh}UD#>?8-B?;Y>8P3zT7A*Cix;wf^TL?#0R+L|ApPnzU6)Xkj#FXM5BvnFo~|BNlgw_4?Yk6BwillJ9%D?ih*u!f+y6U&WKv5X@%+V(s+CsW`jk7`cmyI8eS%VzKiSk^q}8AiO`^5AhNCv!=UE;2xy4`s=%ud4}DjT4rOg(wlvg#VUcFGW!FG|R8R?RaNJQehpym|trMvDl$)bQgY)YGvPwWP&dcCow;c$>b(FHrbX}|eu_aE17M^fx4ft|R+Yruew-%JrdQg?txo*qIln5QS=+^G`CMp11yL-Egk)QAjjnOkMv<7`()5(vEFv*Jz&Jc?;T9X0-ll1W-C(NJJinT&D;(Ru`hV_y!3)vdE`rU6+9Ol=IIQ#%kk7xo+;0!Oy;Bo(t@Z&JKic^%m>GlPl-tq}BDRgVEyW8w-;|z`$Nt8j0a52Oysu6sIw|S#Dn*(~`U|Lg*lsMOxuvOw&fmbH22t@pg>!vy1FojbUmSr-DFCx(21_onn7I;zFuvph~klFKV_R&)`^<~+V)I^)Zh+{&q8pK~~zQ`^IS)SI`|D60eS$DsCYVL*-7}XvaJtEs((t%EZ1)J+jqY#lK9N=+sq4HDOBG_Zy$y6(CV;|F8KRr4-3|>Dw{Fk$n{e!ckx8M9Z*~j${F2sc$DQBR>t+(<SVhW&>ocvbd=z<skND@yZXJAj2u~l<dzfWdvJXP1Hcr!jxwMe}szki^%!xZnLOB$oipS29r^F<1~(BJlu$d+2)eZs%vNHkVwhzMU~Fh4glRA#u&ZafX7nR=02Tz;EJ3-!|CsA|W85v=nI*Sk+>#=r|#aijd}?RqmPI&7ukPbhsFLp#W*(rKej61Mc+rwf22xcfKd*1B}gPl}O?o&*fa6Z=fP*LkXf3nDJv*dvz01?bZ-BcR$J@oL%+v)V(!3dvjI>tK%VAroejnuLW_%F&xbM+~vrW>%)T;8xrLGFhO7S$eHu##$ZaUGy_fu(lqm+=w0eX*7;fWWHGt1{cVvLlmZ1H|wfshPNmH9KR76@Jkhq(ZCOUJOFa!fVA*Z3wvSx=%*Pwuxw6qB0M?pToRn(7M=^|1U#;=nQ-dB-_P;dDwxAF*wgSkplhr@C#SeOsR`Ssx9Wc9dkPPCSd=Esv87*>@rT;wm^h`GY>nsU_VzZ(=G!|x#S(ztML9cvYxH-YG~Pq}Q|IcoiNG*G-WtET`~j$f#pw6XfRkl-_|{T-Qb(?pi-!aDkVC>-7`n94Ay=N%?(D^rizt0nnBNY{2N`Yw=4*nhflJiHKhSC9?i2k9G8t!95xO=l(6dB3puoW7R79N19+JV;;+6r%&K%7O6p$l~h5=-4OjKO?%TYAJ-FS~L;eZ8VbAPDLsUS+dUA>$RxJnwS>&A{<-uaAO#<)n%e{}=L=4=5Q3*Wp0{u9E?HHvWAc7pZsjDLf^M*p5mni$T~dB%uhKSd{~cW4EU$@>l%jp9v2_So4q(8M~kl5X$pk)>pNcemTz+~)gCoNG~<&Qc~|(Vu<eI*ZTh5Z`(BpZ^18lO5o?9{xOxqEU9hT%@kiV#WUy8meUxkjJ#<5{Q#T@zw8e71Vweec#sC>rQK78Q^>~zfDI`dV(h#Uq8dbx4y_hq|TyWg2}TP`U;X;o!HvpRWu|gHQ|Lef=2NL5Hl{CaO4wCZjQ5V1%TftsFNqUL7_INyMPu<Msp!lZWjQT3A`dFO>}d67-SrkOz(qNY;*mpthgIG!dnWP5F~{WbGKvCk#rB#K#m>-N~S@4Gs}h1q6|<4lo8&Iz>Wn9z*`q`-b5Ef>A?uZfTwJZJiAQ1Spj7Fm0Sq?N=eSM>Y#pHM#wyV37i5P1CTZmiDBd>`zV0wMEN#hHUSTqAj3$FDVzi2%wx{i{sPMn|EF#!*hWC98AyVm;u*`FYabKl*Frh*!fbCS1gk`Qxi>6dEVAnosuZx`@EmEnii7xrMNg&g?4xX)FDFryzfTI=yBkwv5(y5%bLND58c!zSg=!iJUXYNJr>IQ)vJoMH;`x>U8Kms?5tP+OgZS$(qoc{kDdl;h@^vY$L;}M^lj3;tLRaa?RAmarjTq<DPa$1wD7Hk#qz7V0L&PUrZ|Rw!fo53CcX~T*loWQhb~o95>WAs+PvN}(=bSa`wtKt54__4D;TfF5>CAkTqEQQNh9HDxwRK^|LrcW@B*s-lMPR+I-sF{h|L*PC;WuYThsVL;zkrrg`MZO+$8WzmI^F-`==kXD?!S|I`Ni?ue|mrS-w)U8>eW|CCzgg)`O8w(*g1zMu3l#7=LSj@8WFh>)apbiQad}{4lUd6<~FmSE?YJlp6)*SWJopyWYr4nY#`}hymdK6I=^Hhwe<>Jsn}e0cgU{^WL8iLcdFCiAo}r1&2{(xR9tJ<o26InDpqMapM96ZEWYPjcp`m0MbCpp7G;A=-I61$&0<i{{uPgdeqZ_=^!wZ@P*Vv@UcuAtyK%8EO#;TO4QHTo9O#Y9{R0~2g)!Y1sJIC@ZnS#?9YIM=a~ilGf=V4c*-ZnvQ^upr?ciy(;SQeRXnS;yaR}RC)ZN_elskCZy|^7b?RH4pn`G{Q|0S}{kS(Gj{-pLLJKK-Y>Z3jvS0~0}I=8H+{1Bn%4duCY!-!igh^O<3^m05Ji4~eJ&e`dU(M)~3S{%ay;eVJbRU5v>sq7NN0e!O5#ko*#Z)P~U=;@YUqS7l92C-v`;eduf)3fZQrbQp&QQcc4VDbyyO3SafKKpJszNjI|)`4cHvr&M5X98Aq;>;Fvbh7;en;%dCxhl4Inw>VzQ2WN9<{4gP^Xd2d<8(G12yl?b1Z-ia8T9M}fVMF9(VEPL;e>xvXKj~ncN4V$fQ84fQ+HZ><ZJd;+wYPWYWux(AMzkOw2YQ}Te};IV8zaMc;4-_TH*O_Gy+~<x+6VZxwoaKF&Rmk9W;^P|8nPRhX-`I$L|-5crpUHiY~n~?$&!<Sb@Y7w603e-V?h1!(Sxw(n9{zB3fXBG#uwg2^v0U={5I_riZ-2>v%o^*oLA<&PHB`c_@D{KWO3@`@kPA5_a834*-I>^k$~s>GAPw$l;}*ymK_d@;{~hjDgO7@|SGCGLP6e<84&map@c&+KLOLckJd<JPy<ABjut~j3!?)A9jqfVV$S55kubznE^-WS?zd8LN<AZ)1?T9+VG)(2g(IE^<O-ob>`hjAEAQ@_b^v~n~l!MHqM-g3N*0(&tkyuJc?kojOzjnI~3Cxq|q4WeJO0}jb${Uvxk)lg$-a)%Oem9>)A;yk3uMF|0=aSj8M3zMG3V$fKYhn<<xRdLSYj2#`EXTgAXitCg^U9u;{E1=y~vKz^(Py*yRPvqM(M1xU)jmTH7d2?$iAbhl3A?C#Rq#^@GJ`7tX@&HXL2rSUbg7#&sHSN_Im_IAh~*QCkk)T5BvBaEGLt&%T3&1GsCiqw~cDu+Ryr8NoZWA%&B}AMy0Zju@*APQ$AR$yIuykQ)d0LQ&eiS&%qIlntNg!V+@v0Y^+9COrIH<?OGTYFoM%9?KI!w$~2|^&4?S^BtlO<k~&2nuTa(xHIst94x^e5&cZ&e9vg3DSOHNmFAt|0N1%))vm;Sb>G~3l#ZZBl4rNqEmjE~n38Y~YDYdG=}U1A^f-+$X|h;;L;cm)RJf3-Pg_GIty<lsIU7r?d7vq8JY9HKT=Ty(Ul*Ne5}r)~PJgv>SPWWF;m}?!?ZizQW`C|$o~{d0VV#KIMRU@-cIp0WbFZV(4BnQ`(uk7`C!ksuVl3!WK^sIU?56N@jmyJ(;@X-m&Y~P>)v8E7PSR1ouRuS3+9^7!PCyRgDY~gZL2l|XAz4O3OUcL8VMQ8J=%?t#O+>vm>g|nQd-wMuBDY$agGQIfc%S@wk<xe4&(W==y!|O5iAIJ;>amT&d&d2-9cH<_yTIyB)rwTjh{>2FuHt!mG;$~+!B_il4hN@a`)BfyZz?el%Hru>#!|dwgNV!cl*Bv!>m5rjquu`-ZmImre3MR@?af|uuWPCQG8SERX`GPhEE=^6hp{FTrs{@WQIApjDb6nk;$z`vUL%UBsk9=^I}V?5mjSXE0u4CDDM;25eGU8l>D9cZ8Mv_*7-E*X{Uo_x@t8qjd?#Z++~4w>yX?F6_pNwje9Lafmw(9Jbgl1Mu*o3f;T>u>PG_)I62%vn=QGL#!oJFC42U|6<LuqzZKipgsm2uMmd7GTH2BOm!sRT##Jg_^SHNdO_wm`f`4wY&h!x|7g`8sIRWQT3-wW0AOJ9=#&>6a~aefJ%Y62TKg~7qcuTH<y0W9vT(Q0-2ZvX6P|9J5F@a&LoT$*LA-<`aD_x|{FS+j%vlds+m4))*e9~_;1TiW#G@XcFbImL}n`S{2C-yELppL{#m|7QRA+tZ^{rTO<2kQR=v@RFTVLJ>}3SofY5OJcyIQIhe_jK83Wq*2qh=T`vGjQ3a(z=uF)+{G5W1t{&Q&f_%D6-HqMy0On~FS6D-KcO<mJJ76okc)bb0dc+k=Jf33JrT>`;P9KX!;@yvtviP-2Jm+G2I%Q{@aE{7LvJsS1^CIgr#Kd;^|ucK`=9qu4!?f;{uH3@+`E%^Z~yu5WPqFM>>;q6etq=r+x?T*eo!Lro-R!%qSL`J%tP4S=$lJz1{Y~OLa$slQdTpesJukN8`rf&!!#Pjc}>ex!H($8pxti6M3n#3>y94ep@H_2F(?OkVvJ+7>0F{u03PrQ^cp}pHN1$l<N>!%HoziQ`5Di>l+~zHE&N#xGwX{J^47y(mi0_VOp*`H&0hqR8qb9=*;nPte*gIBtFO-n#Q&8qyl^pQR}&&OKf1DLnALXM^-32p<r0fy<zivlcf6xmr#Qc^`6gv5`W+7s6_IL894oyn;(@xx$K->Z1uwmmUACrGVwBB6g**%(v++&QCXZ4}#2Nz+7)--??S{U&;ZD5tgM8{;(b>ArB0bGHOJ*|nF^8rmz^c0=voQ`uXaO|d%a^KBBK?knm#^#&es%Xr7xX3L-rA1_=pf{9%eXC*H-*KOG!`+9Y<)$_cO61zJc=^BW<jWl$on!TcPV_^oU*VCcj4haE@uaao>uql9j9Z7M+a&$bxHkvxq7J2xWu`lxEmb&h%9ozb3JB*6zxHSJdNjWH;a3zfs|jUyT@+8JG;#tzYeV6D2*nwa0FB27-|aRh<sAr#Rd_>A^}2}9wmjPfQ)HG3iE7{50o^PwhIm=jsd+U0Y;H7<~iP$xG=9NLOxyGQll{555v{M6#l{hA8jO4^K)$gK?Z8PT7}&yBGXD#K`WqI2X+wVrc#R+M1ut!H<$~LLK$CLb2NIr3MUJPjvMB5exfS9IIn3)Db7T^?`jPfDP|0!+bMX+`%^JphEj1{>&#nToEfJ$O|WK96Lg#mlYojW0tQ|LuK(j!*UgG}tJYjIW|3(NWA|`ouma|67+=57T*hdjlcUbipvImpnN=!4;bYS`KFXyg4hPF8^a6t>e~HqWy+|_T=U!8*b^^K93gv14IaGwlX)b#RaGVUrllbD2tVq5~tEbvjEW%vmlCzp}ncE!wRE}C#;~L{+gTo>yFwXD_V-8S&#OWhIldCra=T5IAI*7jhW`ct_IK{{w5yBlv=6bGsW$KtHQ!6@MU9XT>UFD<4SQX$XA_dMPtj$u+x0L${d!bjlL!-Szu=50C>SqLhs<m~A-ZIS55ppO48J*#XEL73)s?{>Z@j7Y@awOCi7^i&tv=~EsDta&Yp+bi?Ys_YoE7GC-j7F@n{V|i}D)JXGmcm}-Iu_m>Z2MGWF%?9~a-n&_!r=xy6N=C}@xJpfEY}Io|IK^0#pipA&YjLqk5U?Lf(*Um6rC-{$_;TdM0QLRdhrfXB)ZK8v_}Up%AnodE9Bj(UkuJ4X?}VS=nPXvUd7+!bzJH_3T??Llcz^>dyd+TxYua6yScsB?zCE+?(X=!9hG~HI=XRtj5->!+{Zo4eT=@MYa8yH)FSPT^zPO!@rnyoGf6GaOp6z_9zWEE3_W({kh=Gfz_rLtjBaxY6-7GR1!!F<_weUFZ7W^tIxl3^JJ&6`a^2eG4%g(rggI>sTarq^BiJ=@BJvb*i&DQpisDy++_P^?c}*k;uoK>@$APnFEfWI0vlf8!CU5Rs-Dji>dS;aCn+U74lhiT^Wt&NBSFLD6ySEvQM$V!Q)kaZkHgq~vwBh;lqJ&Bx7FgCZiH9tqL>A>6BXsXWw+qZsV!0Bw*p?xu34XAWBTfH{vowlMqnw1bvR!5GCFcgJo^o2-S`4}<#gHDVY7mE$_?JjRCH9=+Ln|2ZNpa6+`hDKg_a5KJgW3FA@qh$wfnJae#k*0T@(HFaQNlA=hJ9te#j7H}zJ`LLft6x^vE5T%o7wgCw?=Ax_L34rIYLTIs4>Ao_%ob7Q)$M@qRE)tDxfEIkK=PAe~GD_UPi+oKwyBgjC#=%oA|WFD_GhQC5BO`l*Y;Meg=?(kgV&GDvf0xtflLRSjfcOWRCpBX17ZDlk?eZqL)9hokekgSDt0i1_$tJM9g)H>|uTYC`r`VfBNQ!OwQ08r2x}SvzhdkYA8l`)@6%NIC%0h=$JA+H*CS@HCK<AJt13AylOP!KFc+|+q&o8OlaiAP1}IbhjDG3=`e}PrN)bvfN9LmpI;6{&I&O)wP;Nu!$38jrRv>YoCIon*UdS*kWfCNrY%qrXEpOG6O&7s^MRHqp4aflU<{NKrE?%4*aYDPJ#=TkU5fbE+>9F)EQWXIPf}~<Nz)mSK1<QYKhvT`HegCd?S4}^N@&juSZbo<WmAzPOaL296BWSry}&t~&hzUN)=MRn>Tu{R@qu7`i=EvA2#wP|5C&mTsRTv=gF}-qJ1wje!#5k3bz}C!VCI_xq3|tmqw!?+6Q9E$U)bevGD^f)^Pi9A2kxv&tSp@U$~x2ir0e+FJo3Ht=J?hS2z+?8Zav+6Tcfjq!e;UOD0or7zkhtpU#xnj@boN2XC6@4;2CDWI=>$L5M9@9P_nxToV=D&t63!c_J^7o9IFWd`e9}lO7hDE{L7LyO$QNbP8Wr##*X^s5Y?U*^4VtBlFxXnvW_@_8FNQh`a{9S>ab)wjzO?{k))40Zd!6N4OPDB{(xT^(jp<Ns&|8+3rPaV5s4I5kwHmo0K5TC(K!NRyqG5s_O$L;EJczF=a?;Uz$8!DIRYEAMUK(?%q@>)2-Efpd#|LAE#T>EMnJRSQ+4^(+$ElmYR~%5%KF2EolDl=kvEBX6uekhh}ueNNlmj$%=4x-G{rW`2?+ulo0(Hyv#^Cw_N}rn7Os}?3m^KHpwKmxOjnHDibn7XJ1h45lzKBxl1GI6sUu(Iqj#?zNR@kk@yu~REKv+sRqx5FplZq`_ABZ{INJBkl37Z0qMD~+75jj^PcwCY#fPUaGK)()%&QVRfHggNJg5nVmmLlAP8sCQvLh!$8&A=jX8O^(#RqhF2;L)#?rP)JAG+dXFLlWx0vLyu3#YB{e{!i}$^pk)SyByuZfZa3nu~8WmBj8^Q58Hdr3`Ps*X3jV{o~+M5}1Zli8ac%XvOBZaa)sVO?0eMALcur*^w6?5!=vs>*<&MX5SR47Yow#?_d@INOhU*&Cd?AMPk1ZHgz_A*N{c09=@Y=?zgNr)ZF8)VU}6vbx<4}N&&<1RsjL2S!6aXY%(i?;^c++kJ~ZT$G3u{xooO9M38XD!pELI!>hq~=(Ew4XQkFh5LvDVX4q#<1P_7iXPc@C4qwc<nwX!-q%R#ad7)Qvph20l4~?Z{?1uS}*ubDSBCiw~+mf$a<4Kq^&tca1#uUidV-ae_J80n-QxKzr8}{PHrC}>+z-dO_Y~SXGB?p_OlRHSukq4+1J5wkZ6C@htbB#xyrV7VUN^m%v76O1KB6?fP9pI7y{bLlOg2U_CVcSt1q1C@da_fHobEMh9t5=fz*zJ<0qT{0*SxiN3r*9phgV|e$e?qZx!PDZ-hX?4EZ7fk?h}+81L&A4X-EW`8B^GSahHnRNJJfc_>58-+XVkc1$$t_ThGv1;N@f>*%8FI_R5*yK#YSvX&=a|}mBE#o9}j|oAI(OeeqhAPi=a{ZQ0`DFA(2pQ=Donur3s_&l1mO&UPzpnBJhwC0hP;>aPvt{czz%Yn=aLP71%2v^o52|18Ow!=@Y*fG|OZw2glSwcCe~KA~7*-x8=f#K9h>(PYUf3BP@>%GEl8!mv1k`s@mm!Q0kW&OAzElq>7xCu(Hx&EoLuk^E`Q}pLO@bLYY`=$&~IH>Zn?F{Hn<EG%K;A>lHzQSI*=6zKB>VT(5u-MZz$unAmNl*j+&uxpmuZ<Qd4A4xWONMXJK85K)clM8Ygfr>7=>I%=8915D-+ep(29mG8_<{Ht22U9-zU4f^EY{!pvcRZT*-oY2Pd7>R;#3N)FM*5fy>$TA&O3u8F~ZP`H0pbh_5uTn6}(MMarTiS=Hqvh^E_EE1;T+8U<zaE~5FjrwclBo7x*ju+3i(MVBX3V5|D|Q0o5+rKisoLP}Fbb){XV=q!{GX`O=%%N|(U8ih_B^k@3bU_btXKeTwmfY<%qGJy0Ua3HZ2K^z8?&ACs4LW2p5B`R*Qe16mD#Q<R%RQ+AC-!-vEcUgX1S?R@lE5lFmVUu_Bmpl`S<obVxzojRs-7W-KGO;?gaL>0NX(fT9XnVH@|<I-?y#QNNLQF%#$YMVqUaC`Eaxsg&~r*ov_);V_s|~{!{E5&D8#7I-7oOjz<-Z6etwWbaeZwx$*1F=IrJ=`j!^{=#u}i6wvBy{U1r5#6D8d;{T}NQ|v1Zf&R}p^J3o|S>lxafy&O<SH_z7KUKLL`&3!K``jnF2+iIBVwH%u$y~I=vP9xoU`sX$**!5L(ww*qGAi4NCB)<t!|hHK7R^wo&Z_J)g<-P~brh6+D%dysjw8#vUP=n{h8j@fPv@owamYY5#nZS5`_SGN$<Q0_KyA%-JRo0!0r?Whd_j15WQApynxEW}K%|BVCy~05*+tgjV9J{A0b)5!5@!Kv+VJjic}u#LK$of3fR!{v<{jd0`9!}iSDg+^KQPs)S6tCF-JI)gUBtbNY{_H^OR;!Z9u&C2fL9TqAWLg4AD&>2k2XZ1_W?=OD!I>S)_udA&%A5RzCrivf4<0o1-|l==kglDy!)m$Taz*iE9vt7OmfwrVVdtswj3YTSz3L&g!IiqyG=61uR@C4OwLkQV}xEd2$_h8=>(h<_&8|}F_eUdN%{y-NiJ3%L2i=bd^WF@$H?<aveI@6qM@B3d%a@m7yng*O;^pO834_@qUxpsj#8D#n$BV}x77lfyoDt620Th>Yq&Yu+-tX5!}HC}-f&ng6tJ{#gwco!6;YL<A{Q*4$ZDdN7i-01jEqx$k3^1s<r!rudcmwD^jjiYX+$X$l;1KbpYn`rRV)&f<#5?-7huxauYgI1eDMY$)gMV<^~bC<FG&dUQQ^|3H6^I^M-p)TvCs(dVI{&4St&}1qiDnl<a~Ry+1oj9wRX49x3=0_)d)oCrkqHWhO`FF-JsE>a%yXVJ38%Gx<7jp&YSWkt%*AD>*&WtR77MtV%9eFNKhs2lNSln;@7NPz9&WEeKjofV3vuR9pce3u9i|0!4u=QLSX7W0Mwm62+9=!3>@*T6wtfb+{1uv9(-!Yzp|YEr##Y`4`YYCmeyFYp762{K~ZBPjY7&*M8EhbMy(AcHc)2MJh%*#3-tj@iM*!g+!WO+^gR6U(r6fGEFbk{JU_)^Xi9ww;c;b_F=6E4icZAiDa!^@-eF;{=kbKf{A!iL!W2|c5NgjcDG;u_$dZn7Hy^?(a)A|r2gFpG!ukS2nur3XShKNGbf-uPUZgx%x}US!%mY>q53|)y<D4y^D52p_E8Y|t7AW?fWx->ea5^nA#=cKj(T|m}#X4F6Wj3}<RZGL4X2QA`5=-zWOlbE+VsS?xw=Gr_*+0){LcGZ95I5=!a5xIiuY-_h8A6$QhSh7S*zV<Q-r|WFHYulBr)QV=P-@et^*uzDMDB#jY6bGf2Xd}fl@D1wk$>l?6Yl<6sjSF9l@+vA5MSAWvho8x$B9+nLsVeY-D;U53APheB|E!Y%}xifwY$^YG|^=OB{QkWVNlFmWc{M$YhF?2lwR<G?(3Av=XfAbQP!)_5Z*<U2emkmv}7)7J>TX--K!ngR{f8cx&CpAnHX+}JWGMzjn`dT;M#+gN1mgeHRlm0DU;s#n}`fJzb~eOl`vJLs&H-wN^xDI|JB}$Iuwkh)HKebw~$+?=H7Nfxe_UlT+1c;;*2P&IXF_$Qt*K8PM0YV?PhmN%gUjaEY`AF__J-W*DSo5uZxz2gM&>OLPV`&-N)IeZSV87_SBnS%^%m3c%$Kf*ikaRs)IGpgV?tuH*Fkmop)2jkiPQndh0%fc}ksEYtE{XuZG+rJ6K;4|DDLS%8JXPsE&5Bn71gAs#CqUq_(3O%EF#ut+q6p=t~)!iVy8<;}+_5H=Es_YYS-+1*S)ikrC$B<Opob(wqgMJ84FBEPbB{#~{Z`jigN^6>%7Xe^RkHE&fC4bD`NPDPLt@FU6vU$Sh;b4%+bV@JEa>&=Gl}=AYA^;m5x-DB5_&q6Z4BMJYK&f96e!)h}zsJ5q`KK#}YzU(?neS~j-3y=J$qR766CC+_xZYi4Y_Ug{pNyiko7VLKSmE)BEj(*XJe=F8S}e$`_J+fZt;hL-kjLHZ6)8i4LmENy9NQtn@!mlNa~NnaagTy{P<Yq7vbmetoad)I-dh4dG2I3pgEA&dIQRR-4FXxS!GI?zC{Z|dAWk$T7@CtJ(W_s!8M)@&J^9G<>Ceouun&W;ZLbjsT+iw}z1Ii<E1`fi9g5P21FkU=;a)!4eK*^Qg1LctNrlPthoHhhVjeSO+PH@%QcWIc2MlWw)Gz&kF-g=2IZ89ePEdgifRyp1Ue8{55Yt*n`66O%%ZI<--U4>at|c*F-5`IY;@lJv|foZK{{hLLd8RPD~YTz2MzRlBwp%9yxhTgG$Os;@a*GW7*y<;Z&7W_L4a>~!{;n<}qmxyp@&Dra|6bWo)Y#TiuR6tcC>q+}lV#_|Cj{M9?0lOxLOB0b+y2r52acHzYxtB@!PR&6-SNa&binck(XrS??ou_<{i&+6Y~^W&GA>W*JB!aTgjM5ZhCUyQ<&M&xXg^79vR3m0`ENMqZuD8iiQTEo*s0J;{aAxQ|e5|$KH2uorYlsKurV(e#+X_9?C6Xcp;7OPmjDq6hM2q~kKM^jT^Wera%9l*-J4U{@1$PF>TMdONDUb2*O`LHI42%eF%#J+OZK9m=@@+~h%AV!m8m>_ZjDu(!@wMQi!9DNAIrM)>N4B|vz4Fb{mAMI@ml4m|BR6ntC6&cZJv1Vsf!h^0;XE(6DHfmP8Ab@sAvszC6Mpk>e(l;~O+c)(hF9@?O{}g5tN0=@}DN7(Pe_(3Hb&xD3qh`i_QDtq4m3jKqDUKOlPpti~x@r}b`z;V6_;{`AS@4<PiiwbhuN(_{Xy6=e4BRSwr)86g@#KZtnm^`QuUuw)5h1J8|6ug9wMF#P<G1)LkdL_M(UZ7xMM=%xD!x_34vss$3N2aWS@f!mC~-|vi$LX1Z^X=494bz2w@Xvo!`dxkZhqcUh=g^1k~Gxz1|KMoTD4h!gRy>{U0C^aoo9p2>>XU*5e94Wb%%uV#4Vu|OLTJ+$j!j)81RY5GSTnz%LeW9C*pf)_BM02KU^GYsP(-hGi`XwB>4AtnUZMOT+L22X-G!o$wH}`^;ASmYRpLqJom*hE{mVMePOIJA6|_|<d0Usy5G1|A=*q>k5zbFb?B5;<fdRL&YFccdBsC_BqnCF<KKU771?x|rI*dV)TX^kP%8(kWbzojX40;xDXTm}r_=>mxh=Cd1`1+6=z>;P*koSgy08ALZ_K8n7(+2dt%^HrNw+g5xdY=FS+4}U$IfrYyDL!88zmLeea4X&1paZPJfocT!OOsO6TQda=5<4b&8QyyWmKr~w!-uE$xAcB^$~eDlPPWH&8jYWt1Xpr>@QuPN<NLdjh?oQ(n_n$O22Gj-Pv?t&WRDfrC(K?)?51Qz|)7EpE+pA3m1Tl8z$jJgu-}@D=`f489JS*Kup8=AhM>uKO9C`CdW}@TgfqntCm?12COvT#yA`zwJ>2ZKw&Znd37&*9*-sw_udJ(JJT#>eW8m^Mjk9-KAXnFa5A~3I2fL=k$;t4#`9o=en2B=NCjURVp?m4g@}9SNJA0xhS}X@WlDNGn(>c+Q^<+7A+R=No@$c7nryrC$8}Nj9u1DGn`2hCuM~h8mX_j$)YBCyo~VG~qT42Ava`7j+GKlgHEd{Af%@*?Ts>C3+&M<dl*(tfs_9b6b0Z1TWhj!AIuwqD(qtsUvJKKzYqjOGpTcy+$WTZDhHH9@I$*t=i5gpRCbROW^h+B6X;i2yK_N>AvmpoyNb-|w)>}BvqtvWnJWDocgpqVg#ENB@l$6}R&S!iC$*NgjB4!2<!>1<v5SsK1JE20%=04(3Fm~9IDSbFe2*pSeM5DxbRw<{?Lo}$edv{Gq;zl#o;q;Z%n}mVPr%BP>j0%e5?E1Uo^I9+W<o7EmS`SjNoQl<dj6PMYHQj-#IF#KgO^s_WX_U9heHs`Ys6`xgNy|t$Rbum3bWnD@4uoVSKQg7O@|1A#1Rvc{ODnOP>=T5jgcDr3P@&lHxVV%JPyncvwGj!F@J>bZ0`K=<)2Uy!$E@BAdC%X{ooKCvJd^yA<HF2nHG=Me)<euyt+u}j)L#q~S?hvg%RDLBC~l80f|N6T>jVh6ap|cBht}q-$O_iWLT0dDaoNFot{Sj>>G}k${ooH$d~&yB%j<b*SryDA?0u)1%kzPL>u9IYGoG1dURw8>cNODAiJ_!hmO^k8G9&ZUkeZ`ZT_|2y+tdnPm44{j%;$N4-AZRJH&>{5c3H-7&La77SZ)oE6O5aJ!`^uQ<04FQEc>E_l=S<@M_+w?HaOTn`Rc8abV`|uqb5cr^-XndvyWMSeym{;zhYL0G~-)D=~C1^vf6htyo7o1_@;_6-rqZGi}G{22#%38SCm1^zBIg#upi6ehLa=END(MuUEq3e!G>NyDg&`xx00vy>WWgsrzkZ_n4Zb(%9xyoqd7U5lY>>}Tpo^f>(-NGo!AnGeZ!t)S~5Kz>{|LItqWg%u_exqiu%`3)xSXKdrKVyocbQqb6H4er?;m&{0uRaX5#YOvujDPkRK+Qh`Z70BFlrz*#wlSJR=_%u60qf^}6cUC;biwN)wof&<ouSlpdJ);QrWpr^tpvVdLwod?ieOByR8@H-UNCq57v8J74L`e5FpEZw{@sYNu1$@`7D@JABBC%yzeWw31uhF2Da?wv^nmK8eDNic<*;)z_+UOF{ElnQMW-RB?SRko`Bi#$IxLU68SMiB0rNV#bmiY-A~pTWhClL77uQ0f+8hpaDV)-P@+i+|Ev~)9iJXg_bw8g4MIoe#J^R;77**^RvEk$OU?GRdHJ;%~JGQb#TKTNzTIVe!bP`fZdbTY=Ui?+f1`H*PN_g_l97@*=brl>TsdqHiK3Rq(O5VaedmAht@4Wsdy@Jk+(7<zOZ2~^13o>LWRWLM#@#!b%l|BmHNK#%*t;*%QDr*xi3vOcW^ggb}1Sa>i*5y@i-6z<NCU5*elmRt+^3$U`R6*kpMJQ9B%HJ+!!Cc<$j$NThl66YkNS9d}5ZMbC80C5TJ#c;ll88xeG&mb&^>Y^`WWaECOWNdZ-eTK8Jgdiwik$0G0V?+<V_R;xd*)_rwL>qe5o|>@3gUTEdql_b(sR5q_jzVBGg)D(g=@m(=sQ!nwpqQT}8OS#{Nc%n+kA+)!*Mzen2YvBP?IYp2<3uX0#dTg<Y^FCm{6iy2g3SA+F{8cTQl7E}5%j3?1Z#r87WgP$yTz^U8TU~uvZ3SmV?UX6iQVcce$6Pml`3s%b44fy$nQ2j_&*&EK7;F1*=TTuH>zOV63;FZ0E_$*~Bhuy_;VRc`KEMpvhv`;(+ohtj7;?TpmxG$Q|EdebTYZMts2mPKRP&lMSawPk(4jo9AchV&#YFXqEK_r6f>#M9+$=T;$HhHe83nw4i!{6K}6R!M;ifGN9$L;7-+J|%l)g==Bo4<|Va9izE5|RGRwGwHUaWy|9vw4>|7{qzE+iiBxiDa+4s{}Y#Iq=kZXpryo1TY>3#S$uj;Gn40H>-6~WK^JT8u#o*u*wI)A~Q0S_rin>Une|?#6!BMij(qQs-dL3kBS(P1pXkTEbXNVxh?IaDsq>0R1Lp!xQF3pc{d7j8u#zzu_mPx&W5ieO9C2LLe9;KLX(Qldv{U=nald9N@&aa(NM{dv%<ON6G~q=x&MGGZFHbuj4P&{>b7^_WZC2giduqtbcOP>I94*_3?$)*d0ZA&KlY&T7O}oi<9F1{1m&^gt0IQ4_(VQ?Q9)!D0<xoTyq+t3|A&;|50e>uh=#Jtn)9O~K3Q6;o$Ywywvm$Ot^9+C(m}`BC43`&ze?H{9+QhXciK^m=VohhU0}2g^u8^jf(BuAbnvdZ$zsJwpRe1xwY0uR$puKdLGg9Q4DVwQC}%&ZUW&}TA56pHC48~Y1FSlG+gs>6+uiK8o4wx3RBDR*?iXv0M;qnl%R+Kf3?WsUd#X#0gQ|hu=0CBxY*9zWt&m)ZFt9IVFa>g0T+bl>*IgAd^=#xW*F2|-lIgC;f%C*YdP&FyIYB{}H`!8%mn8PE#&rjbJOEe@Kt*tQM*OPzK$i3-UB9X=8FRvL#6gr!+IPh!X1t`ED*kSj$GjA7L7F361sGIExg|(t<)bVoO7gz))C!<?qWj-(kI`-Df51L7;&v>|WT3;Ni4arF1N-Lnx0ZM4xezWx+;T5h?_QC{l9y^Fd97AoPyzI+`^>ny{u}Kz3_EyP%(Y}9f{s+Ytn!_6H3n5yL-xRL)I>y`+MB~2&-~}ThEs#<{9W97@2x8ha^Sa_`L3<CR6nMiM%n|%jF0|qT{W45a?zx?g0Qs@MKmcce!Zv=vSP|}w^3%xc4rqaYP!8nyGU<)KLO72(JK2#fi5U^hk0L-KkaNlORd5Vny8-T!+$WdjaY$OtbQ^-BF8AW#*5{Xl0ghrXI0G$c*W^)C@LIV4k?LeqsLM~A?kzt4F|DxzKADyi=0*IR$dFNj1itN<#)tNg}%h0msJ6q!ny`5M65>i(hRD|J5ucBcOv9xiSZML$RKgKth1Fi)G9~t@3N`rR*B7g$-FdkgJDS$FMeG<tTRTtWK2q!;xQwZrK?`{Pe##IyV5iNDL%s19aY6HBt=zhFV?BK!UUE4*zUWqs-0DJ({UY@jfWhSjb%rr|K(cghSvk~SKq5lxae&&;bMEQxwo=xu{>v(kiHyZk!6Pz+p_4%auaFzMz>{%Vq~wMU18<a`mioEy>l*DN}b(q7ktD!UB(i`n8i23m&ckjt<z?Y@|$;ews*M|Pc5vd>={`YOu#PdmKE<<q=oACT%)+#T2YqFE{XFy6*Z%X4Xvb^tS85k6AZ?px>Un0DcK(~V|bK(*Z$s${}kURQG9WEKBM}+yzKc)?IH8Ltto}>OweVl>wW71)@&fk!nc&xqEfs2v=sFMVg6=R*T||VCxW_lR66Ud89}T%!Kt<9WL6xO5Sjj@TNfT@H53|C?K@xY8gwr{eYbyhw0}H!eRy_wfT>ZNWv$<xynXln_;gt_irg9;?7!PTI6C{bwCTy=o403&gTsG0JK3)={_*}dhiCgI-wyV_*+2gF^ypM+{(S|c31-VlR^F)?{$)>7GZ0^oQYuKkW}aPUz9qWH8#yZ8Re;jW`m`dH4}py5cqlRxxd3fyAc#0PHh7hcfqZwfdzOqRYX~QSC@U1q!~Al+0>Tt-R58X9J)Qh9hh4j_tgphSlM#wwK(!m?<0UmYIRfiNk<5UfFC*B{!=g58HUQO&jimJ651tms1RXk9LA^%RON(-9b<K`)Pj>G*6xmt2-DVfJn2>YFI!oDyqUxuZ9hIJqhl6C6474&=LNppIfLj8U46;kQtVAp%7^@%rT3m&x#eKiiZ^&GZy8Z@*i$O^v>3k}ie6Jqj8Za)e7x#$Yo6-$MNvi~;21J`u#~i!nv8t|%;fE}=buceI&#CUk)>elKU$i@&W_NenD_j-Hqyu+lSRqlREXXD+N|5F9q+sR4lEEU8#(7d`iu5yf`PiB+YBd8Tc&%Kjdn92?4IW&wF8`1)R*^8O%M6Ih^9Z(V<XHoWxC-TWOVa}BI=J&_b!4aT9`^)@CkI6lj_w-TWzKMw`n^|{M13tPWJ;vb>pqF3>=*_~gsq&uI{y%z@3{>yghob-7ik2_skss*`>EJP!9MD>DYvfWVB)vMNs+58C9-3e#o=%m&GRC|8K2I&hunO#ba=*da&){|o)^j~Dr=Y9j<#x<bw>b+Si_c*dehinF8((-$HbaTn4pfSCpAyh<$q>IxI7!FW=1CGx~A7yW<zjWM_U+uhL_1An&s)!KuwAMGH@)jBmZo5E=udKB=9Vk#aNDkiAd5wDFs;QKFLe7MJ{X5Cl~(^y)3w;p?%<xK1{2Yi@{W6E>#(e(;4MI&#9nVAoy(94Xjd$QO}y;vwm=chl|O2FduSHTeL3<7+^49yL`+E=83YnvT(Ng+fW5t{%qaLH&lP6rbWKxpB2XyiyAdZOy0PxHdUtV%7yUUmTFT~lbiZy{G?`N=<_!*QtDM^pQGqJ&NF7$z!jRI=?_&{e)-2<ML)w*4rq*juuuzb?uJmRy(utAuYnf%&{^FP`ebK+_#1O$U(vq;5AL#m3e;YwV0%=*=kLQjeg8K&Ftn>W?qabr>k>#<;S^B;665;T&nP3Ogqz^HwH#&U+&%C@b)N5wGIlP0K2SZp|E7%HBjA*Ib9?)Wxzo0zH;-Somf=Tp$mZMBCwEyS;`P~fwA4{v_rX&*_o-gQf`1gw#Kr^+^OxzoQ0bz`Bs#aiGoXCigJF>u{O?5qaxf7S)|NNjpN3@NDZF8#xr!0YJ#2}9_1HFzYMawSaVq1CmieQxtz}?Pr{GdVJ<>d@v*^lf+HJ~lXKnzIG9jVIXSjLLF66A+-h*7GVp;DWBvN_lP94ITBh<+!8D;R473g<aP@pKc>cTII)UYH;?S>${VVqCPeW8<9ec9sXEI^H|FwaRrWF6xYFQH72x$J)d5_K6GALxBq<`-G!16ktlxYUPnY2wv`5};Zw(^5yKIwcI{rNOA2Db^M5>q_m#e3xWyx8qPM9y=bZAsxxYNW_6dHzeknTxKM;yc{1>=9fj9&tj2KTIX&p`2PjE;5C9yd(dvT?|aMS#==rvUzFHl8vZa$k{O6-l$UofMT4tkpZ`$qJy+8%fsc{N!}GvaHi*I`cDDZ0D!G$TInTVBuB8zE$11!ITZpxKejW9(M5n*AYeT_r40XnNq`ObZ_Wvs4i@dFu6!P~bz$_Is)KJfR${K|hUv{zTsng<Jmla8Hi8Af^K$;rt_d8quN+b3Ax$gXL?YL7?x|0fhJlOR{b67R~JH?)!r?U%K^lX5aDhaTv2>vI@2jqM+PQxik1=O|ap-Pg_IK)gvEBHT-;_*1x*tm#60(wqyH`()xaDsRk0!2@sXX)@cS+%3Qm1e<tg+^-#_s_vrXSdyLhpkp;x3e`K?SOFE-tP6*Ha0dYjkVTjG^&k9d1iQp#^x^iPcAapUr{=W`hEQW*J${O!%6&0^!6u;)T0(WRFos~3mzGI5a!`zcF{B+urLzqF;P$Pl>EoDW^h1-Cr=Pa9<HoB5nU;dcsJyBt@kWFu?1kvoz{FT`g|SFC$nidyk_O}#BV^|NhE}1UNY}bl-ig7kZ&2GKBW6%C-M1HkH%Bcq4Ahcx*S5n=TQ>pu%^ssTl()e#q(2g&{dx6F8;#VTNG<}m(IrVMC5>E@51zYpL@`<iFlriskq;=85#Y)Lt7-sR%XB2bE{wNE$^3H$~%-|smr{{ZPG5h-6U~akHlcs3jseXQHnV$had~)!O8pM!_&jF!Rf)*hi~==9}Z8@fv+DdHoL%gI_(#%3S_0mofoVQtKRta(aGV#+1rzE2Z!H$=%ap#8ru3-`^V_hcQ8=et!o{;KKye3{qfnr>Zs@h*>;`@E|NVTMdJ{}7wm^n%Cb}Ul-e)w;bI8Du`dMsmGR_#i-E8P()Aj&z`kQ8SgWDfZqUZw@C|0#6&s~*?=b3QF6cx$e)oNpY<@83l-2i<C<l`mR$u3Ch^}2K3cK57L7Xv2ohG8B!YpPtVC72t9#%tly5VS$f_yQG(`d-Fi}eCitrXo7|M<ewKc*cqyOvI@I6}<?1y5_Sobb20a=ywhe3HkraKyz3Ib>!P%u<9!F?T0Z^i93^UQXT9zg8`Sde&!dLE<aHWzSD39dDxEraw8)GR7IaG&3eT=5F`xUVghBbkB-7?@+#Z!6kh(Dk-3asSuLUfbIK58nN1D@~0X-qLRug-zmhr{HsQB$s_4tg|QqMyw}%)Rjf2m;ujUZV4|F;FkQY<+G-g&VbldNUKKdE?#>Fgd+iGwbKV95Ip&x&f1U|?n-vz@*=}~)W%GNQMU!!>0N05ADUQ2wqQG9G&nUF*slfwO-JEq^d}gR8TIcrs)j`}_+n$Ks%bFTt-eXyYw)Tn>H%b=Mz~n%IA+BdNmq0EO44o--P;-kmqG`dnYnBvVw`ACwOKz?E#K&5RXju2jhP4v8udXQ-HB*tObGdMbA$6~<oXg{B4jNIMp;?<^@O&6n|NWGL8vLY&g&xBkFC>HYm=(0qwpuXc=WOt6M90M+Z`bQ<WmzRofi%=Zvqs-hBHfZPl|$Y&qG`jTF_g6{#&B4xN`_SqSP~rdX*QNl)20g*l2wo{H&YQ0?10FT$zg{T!`s>3bA+BcY#Kt%mW;MmZ-J~FCoCB(U2!ZDvI6g_9N*D}N+|wbwwLr$EWN*vp2t8V`cSxKy@gjT^cR*>Eg###2(|Awo0U5d2Zi22_defykg&y%)14hsWp?&DB`Z!spT0oGOZVA5j?+xb8N&6b0f!g?QWvbemxjL=ZOQHni!mkFa;`M%O4c*QDp+bj)7o(xS`>B0!`;1BYj-m|A4k1vHZ;Abax<FVmI=zu&MwF&@Si=mIf3PH7qx8{IY>8AGuU6`m$Q_Ir5jc;!=k0cuN%FHQw`0#U&?gbMaFdg@ip*Mc1_@ZdJ_GGT{s9$H4;S{7m7J<8YUywlFU{IDDU7p!>@l@M2ko@z^Nce8<_c<*~FNAPPbs8y>%aEdXY?{B<By*KEK+3b2vCX+dn%L#u0cHPNRJe9RAHg<^$l9!z=1d{>etepWHlW8oh+co3eWIuaz)+-nE)vvZ*HzlRs79*|{bTHRTQKP9A$lNgho=m>I?yP0A8*)K0bNQ~pdY@EK3yvB5kX!W4arPKQwvrtz$4JrSP^SBTe(l-nxLV!+irLQ)a=f{t+Osql*zCu%+qemBZ*B8Cd>*e{WQn$~qH?_E3=fwxT~zetlQIU?bSRISs9m3YwOad@YK^@(G9FcgNrDESd-yLhvhU!>tED&BB##ZL(r>$#})EpGM)tZ0CQKs<J(sc0HSw|S@hA-kL~n-<Bu<Qs>jA!WuW>W<HYVIMjjiG@j{F-+%jP<$UFNlxMmQrZGDYb5VaCe!C#7KH$^ruwp=fdPS_zC@(5%08AZf%i=+l|P(yun~Ox<6f+Tb-9dxtjaoAuU`IfKi0uIt-?Plvkunf%KXE{I#^ea!9Q%)!TWv)|M0U8Zic=$|ERz^cm?tb{KL3{w)l#hrD*-H!dJX3h4r{HUnylNtk2c?O7YZiOAs=Te9vx(&9AijW6E=rayS>$yR=mks~<rLvV&W$p=^?Z(labSMWQAWBQ@K^t-J)J-nG46vv+=7Z|>kXH6ITW#svPUgKO7(oZLC8$+BuCoXqIJDHhlK!m{&oap8H4tR3;miV5wqVytbfQCD>lI6J3ud?yrJAw+sn4bl<CFO$(8jhb>T3B#h2CE(EATVqLY4YfCYQBHX1#~0V{6HFN=^bWOGnTkLvNuwEh=a{Y`N2+hheL+!`AJ-Ir4`8T`B2Pr4vzdhsxn#nNq|ZK)%T+QEU7YsFz|`B?X>Jzg?ShPa8S3lbH`kV>pfwV(7V=HMe@r}GR@zZKgk)xjTaUqy;l8qK6IrzB-U6-KszhhCq87rN*V72bSwyMlWtpFo-`9!I>C5B%_J>-HIaiMq*W7wZStB*!YJ?)6tgb~KMKZ&Bw=A=_nzoh?U#D#C^yjjq(`?w*e34z&G>Ns}C*nL{-N>FxD**Ukl?=nF{Fo!GR$%VMvi>SpdJukTg;Dq_8b0~13=(1JsQH25wj1YX%kW<C01TH~l1w^MBxUD}J@0VmH<qBsU<CHgS8YoA_UGC7ZaqeJZTGuGPES1xiv^Gy1oMN~>+CQ;G_|^`sOsvJn+)Nq#ziN=I~22|yqb5N%>BM!!jbL*UMbNYwXxc$4nc@6=+`tJBG(4_$k$<Zc^c&=FCj=|^h9eCC6qM6)sKwkU%fkK?%ZP6&7Ix7W_R1SYTg~fN>wXaE<DvqoT=7LDlaWgVx|pu>|JLNGox@BnU0#dZ{Jre3@`pwD-iE(?I;xQYbz_k<1Q8=>}gI(^Zx7M$=ha?`Diu#s#YEF)!H#3{4$I=csjn*rBxNG)Rt%E?X}|LC7gD!8kSs)*;UyH(`^-Wjy1jwTJua&DS_%{AAIucC|8Snm!VCSK?Sl)%TE-C8BbeN*0?BH?+vN0!U`HbR?t`~>bm~a_V;8qlx0|*Tb$Tix5wN~Hru<+t<9?J?bfaX;9^?$<5XseJS&RAC@E5J&3+g-qc_~-R9%t07d@HVMf1Ze0>R|K1F>R7bs|^F7^b0S{3v$Eu6hECj43bzXs*cOz1+Mq<2K0}sKbc{U>r5OJ|M41QJ}u=)M1t~liz#w?fM)I<4+msirzIu=yAfMV@*=e%z#!E2Qq}iK$WYi-EfBC^$WGY;_VB6*LAt~gp{j}=^8-6d)0lox(UXa8ItDjW!9zjkgTohK@XP4<BL8^M^Q=%n~+Xj`ONh$Z*Ih=j^MOQ*B5Hu3S?_Hyw#0aN(lkrldoHYJ}Kb^%lN?W!~+x{e;?vmW;FYol274|8Hl~kb0NviPQP)BS927hZ_0f{coXg(GVFD`-Sf>>YrHe+_I5@U-9tntOFTnFGZG`3-Axc8nq92@vlcvm9=!SRZlm4Z1bK(8(#8xUt9TJ`)V`=7LpqI<ID>A3QJf8DpjKW7;}m%nD3~o4hGkhedBFl^uRa15{xFHsi|en_xCjP15?7-~#ib+uN%39mKWS0Wp&96AZ+@?PGFza&%R+^urPMvHWp=$MB=T}n1;>Vjh1-;bsTF}{y7^ysd2x8PaEF#7QT&7dX73nYmN>XLgT7vi_5G4BuX<C>`;}M92bW=@#bh0z*IH)#tH|Ht!dB`@Y<B96vgU0f{cL$dD@@EC{$*S^^9wxQiQ&y6?rY#9c^`N}@w}tR^k1oR_3U<LF<n;<FjIU}&*~uvN12}Y;`x!*)?w=511PKh_zSLiA@D&SwBV`(f_v5Px!~Mm;JywAOBS5>USZW)<(QI7VfCgO?{1lg1%}HP#g&?l5C?HHzU1y(aI)ucW_)0tuEA_f^^B$q@(TqBHm1=O)1a`^17%r+v_)U7ZK$7iw}LtTiT<la<-Xx;I*oJmkI9-mFQKA&WI>pWxcLMQ9VLd*cAXI+iCxTOq7A%V46OKpGfdi}6$y+n@vRyPSu*A}Vd}H*fSh0*Yp<j81x5*Dd_FSrIjrV8B$jY;_#<q=AM4+*DL1u53?E?iD0|~IHvnu?fyM0|E09sT`UtBy0};;-?i-TzsXCH+p1f+RZB5Sw(Oy51)NkBQDEvd$Mt;>SqFqb^W?wltkv*bHxY}sSwswD|Z4U>yPNt!DCA=Q`CW9cksz<edI=xLL*2!8KJ&Z_$f+cjSTk%HDrExbdTNC+(CsIl*gqO+G<YmIrm??i$m3Ix?6Y^l(s$i0Fj@yb^$oN1D;{)hZ@~5B2(6-J)YBo3NdV6zo2PNXAE7lsr;ZVC_Fy931zeNIKw0bDni0uF`Ewqan$LLnM8K7!o*T~#>6}+9}gU_G3_J!tQpa+1l<r1@IVB@13p+F(5QW2FV4mXgL?n4PS6L)0_9l7A=ueE95r}v)<FKYA9>0n|GmPeXnUoKj|C>r-*<eTOBXerE`zvfJQv}DKgX_0&~ex%}rY;LL5eboc}<c>Br1D}zm4UcBS?D_O+KEQn%V+p@S8nvb)&`f==xb{2R8+Xs!omOjmySup;b#?96`!Umcy{{Qp?sc1}@6vxexXrm30EhcLz56W-h9IV3a?JS*m{ZVS3pRp}_Q_6y#K+(Ty*}OlpuN}@FWkuyJ|Z6jj?!tjkUBs6jH}#x8(#!}{n!5yoZNi^HV+Lzv(Cc=|G4`kHSP$%IgSDz`+^i5rAZWoe?d(%_^8K{e+;0>0_cOJ;$R-8!ACAY1Fk;;03g8sxQIl9ar`r8L_-KgDrEe1_lZn=0q3Ux(SnE+-eRgFSTz*U)*6(mIh+KeXA1yHCN0acSpsKVib=WxP(gmcGM8Bf{RZIz5RO7LcF_us!{FlXQ*!r7+)oZ+^a$th(c~&0QLIEc6Yv9a)!ioq>!YCZkLV$g01nZ`bFJ~wv2#BL$hEQa7~0_Y+H8dwi|{|Y`-E!s0`Dqeo_uCME@JfyTF;MO@1>8xm!OY$921WDtu-KvrG1S!qG<OAu*6uqX_2lqqMzY}3=pHZ6YqY*Vor3X17ey$WkzkWk4<Wu5<Sp`P{gT#r`Wv0CxDna43G1hEy#lx=St(Uq<|pN@_7MbsfwV$5v<q*uWf0;)*5&Jin9U!h3GGql4t|=)ThbaZ&&O??DKq<!s4>@MX)}1pXdz|rb<e1V5lpB8-{j5L@UIv|8c<yljF%OhnFw3)x*JnFwU_ZVQ;OGMVQ!sJOi45jz-Z{jAJ9R5Hn$%Vf-hwo*?7^N|@vfsvPzLE$;q;|DxGZ{;}{P;$VmcO{2U2X9heFSjmrzXSnU6Mb7JQMJZe5wMH6E0HYzoF^w=uJq(vAvk9kRiompj{UniTFz-Ii7jecBjB`aS0%l?JrQw`(CYS|XCQjlZ9hbPJ_`M*~>@bD-1H4fta3!>xt7wRiC3$C<rVE5TP=grWvv6@0U4-d~R5^xb#s=8UWRxq243Q#W{)Bmuk5U|!9IV;!!SwFmrx8pD;fJ=sHdt$b9XKGXcP?%}2>HucH8k-uYvRcb2eaN0FWk(UlGp&Bw2!uQ7LL$)Mp`+^;V)B8IDZRa=H2X#JKIsK)$R?qcJ?}sgRZ0x<-99tg~#4jb2n&gk>?U<p`QgGfZ>n+`mg_21Zy~jU60D?M(~F}^tPDSV)lnW;3m&!c{l-J;0=hx>;wXWgtz=*H@N$^U>wG21dkf@sHeAM4?AkRfIv9Kkd-(UTZd0T<mkZ!5h6#=T5uEm^?&|1_}}l?Il1$F?XxJk*x1~v(|O7BLIG8!w3m|^P$_O<u^-`NBt8y39Def^eD{6rGSBB(|M~M_IJ%z9Fd9_a$)6Jnp0k#9_~=~+{%1G1u>jc>fNV2B_UQ;taXuHoeUN^U$;BJOKS@HcT^WMyQV8}01Um)<FqYiJc;ZTf3h<3oVx|CSr!qi05}+*rXafNHfB&Zf(8d=Wq^(UGBz7@6dZw6JB5~vzA`b^_YqKg~I35QKhrL;An7I1;0WHw6w%HcTxXS=LK)z20;@9tA!}?|XOrT>+uB%wP-O6jXOK@}*V$z@}5Y)B^Itj1h3wD6;o>6KHF-!@MSAmmuWt_Av!d7AF2#oCUJr2q^6BPiQ;uvZw@&?1r1;6k7(2@ArtBjvL4JNKIv?>1}`!75iMa2qmK^0Na!YRDL!)!E55{8lwASo|WnjLU-`hRS7gah7_ig3eGv~vy}fZ#<;l@Ysqd?pw$!|S^=2Kf?&B{nyFb_m>QDlUe<L0ZQL@)M*#{6RdkyYsxg^Ss;cZcChO?=<%iC-h%<#5WJyAo6%d;XDSm%*J7S{(QC=;?a+WOah%;K2;vsVh6-$00aR2TS~i<$xcim2=MTEuc#L&zLAdb7Z901w3|;B#Pp;NLmGgPtb^oPl%Pvj3dcrE^d;mI_{=5bOoW^>2^9&4F$fZUsH5YEisitXBg}W7Ks%Vk=NKQxWU}cDq==YhH(|n_RO4EBQhxXI_m4L*rN4(yj(7H;3Bs~|_t*d9{|rt@en$Bs<1zs3b@AuMDB75W8(bWy!$%Imi}$DqTJnjoAcRrW+wQhn!%k;&vomxFR>e1}AXPjiL4@QGw(-B>TavS!AQQ=ABGQOJ)j)xrX#*&uuLU1Hxd@+s<T800GxZe~?kWatJz;7z%CL4O#E*DQF(thv0L_nIKP4pe004Nbk7htPQHr?UP)!6GU8Kxs8Pk|X@y{S|4=?emJ~#mxx360ijOMfCm&j~S0^`SNn8aiBYavTRU#hnu<HlD)UT4;j2q<ngy%vpC<>{P)iYM`uF%leZ!cIHjiD+=bXGL&kz&*TqY#VdQUv0vaN-Raa5;+WLz_S!I#Slkj(DKY?9M}{M?i3RaYsN#)QAwE#J4L{YMW9avY)YI28Og3-H)T<{__^SD8CN(=OHE(3x(GG4S?I*1vo44sAQO;s39~O6JQx+~{Cdt1C#(i+W*NVi97r=cAD%mWpu8NRhRa!UQ)M`qa{BK7hQorwg_u2P@iXmi<PHozT0%9tm8sh>z<C;T-&UcergZK=$M6U#WW;%gIBM7}%5Jku*19*uVYr~L;2T#E&0Jv`PGPDk6*b^S6R8hydgQ?6QL+HIg^V*J%^sGA_LEA<qe!lza&Qgl;yai;i>C85!i$WxBH`VHS&eY%3Qe5WFiwy#0d8WFBza1foRPT%<S87SorGu|EG#=x1|>U_Nk71WC|{6j??X;IWEMWn?g!_BXfwQ=#XvmF;35{9E9$w&{-@jh0;kz;t$|x2R7h5p)e>$6%<%U6?$ZTr8Ul;XGH%$zjm(8C*vUAXwFaL~LDT_E0?xjS89in(`}D`fGcJcPTNy0~+Jn%$nz#<2&@!Eoo#A+vW`r@_b@yYyDjoq{Fbf+qCyCRVj64EUL_{=`yMH6hqS1-C{*Mb-KoZ9Y9f%q#Jb@i0Sm%zbXc~e?3%**o9BYl`b}>AbBh5P23Jxa|hgFQtVy!_|F?Nn&fx5!ND|JWw0Po`+*0N@RCmu;FBi1u*VVpQ)>eQXvS|-h(wEEI2#%$!eO>C_pjAAGY<#+#IP67~!BL9vkgY$8;z!fALM`7_II+UA?u_~1q(`bzDSjj98r_2ZpTT$Tv$mf_Hl}?J$VvaDv8Z^mV_WuK;WN!@"""


def configure_shared_guards() -> None:
    base.BASELINE_SHA = BASELINE_SHA
    base.PATCH_SHA256 = PATCH_SHA256
    base.BASELINE_BLOBS = {**MODIFIED_BLOBS, **DEPENDENCY_BLOBS}
    base.CREATED_PATHS = CREATED_PATHS
    base.DELETED_PATHS = ()
    base.EXPECTED_PATHS = EXPECTED_PATHS
    base.PATCH_B85 = PATCH_B85


def validated_patch(root: Path, patch: bytes, *, run_checks: bool) -> bytes:
    with tempfile.TemporaryDirectory(
        prefix="galactic-mvp029b-", dir=root.parent
    ) as temporary:
        worktree = Path(temporary) / "worktree"
        added = False
        try:
            base.run(
                ("git", "worktree", "add", "--detach", str(worktree), base.head_sha(root)),
                cwd=root,
            )
            added = True
            if not base.patch_check(worktree, patch):
                raise base.MigrationError(
                    "Le patch MVP-029-B ne s'applique pas proprement dans le worktree."
                )
            base.run(
                ("git", "apply", "--binary", "-"),
                cwd=worktree,
                input_bytes=patch,
            )

            if run_checks:
                validation_env = os.environ.copy()
                validation_env.setdefault(
                    "CARGO_TARGET_DIR", str(root / "target" / "mvp-validation")
                )
                print("Contrôles Cargo complets :")
                for command in CHECK_COMMANDS:
                    base.run(command, cwd=worktree, env=validation_env)
            else:
                print("Contrôles Cargo non demandés pour cette validation.")

            base.run(("git", "diff", "--check"), cwd=worktree)
            if CREATED_PATHS:
                base.run(("git", "add", "-N", "--", *CREATED_PATHS), cwd=worktree)
            base.validate_expected_diff(worktree)
            candidate = base.run(
                ("git", "diff", "--binary", "HEAD", "--"),
                cwd=worktree,
                capture=True,
            ).stdout
            if not candidate:
                raise base.MigrationError("Le patch validé est vide.")
            candidate_digest = hashlib.sha256(candidate).hexdigest()
            if candidate_digest != PATCH_SHA256:
                raise base.MigrationError(
                    "Les contrôles ont modifié le patch validé "
                    f"({candidate_digest}, attendu {PATCH_SHA256})."
                )
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
    parent = root / ".mvp029b-backup"
    destination = parent / stamp
    counter = 1
    while destination.exists():
        destination = parent / f"{stamp}-{counter}"
        counter += 1
    destination.mkdir(parents=True)

    backed_up: list[str] = []
    for relative in sorted(MODIFIED_BLOBS):
        source = root / relative
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
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
            "Prépare MVP-029-B : sites configurables et récolte "
            "distante déterministe."
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
        help="valide baseline, patch et périmètre sans compiler ni modifier",
    )
    parser.add_argument(
        "--checks",
        action="store_true",
        help="lance les cinq contrôles Cargo même pendant un dry-run",
    )
    parser.add_argument(
        "--skip-checks",
        action="store_true",
        help="ignore les cinq contrôles Cargo pendant l'application (déconseillé)",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="ignore les gardes SHA/blobs (dangereux ; le patch doit s'appliquer)",
    )
    args = parser.parse_args()
    if args.checks and args.skip_checks:
        parser.error("--checks est incompatible avec --skip-checks")
    return args


def main() -> int:
    args = parse_args()
    try:
        configure_shared_guards()
        base.ensure_command("git")
        run_checks = args.checks or (not args.dry_run and not args.skip_checks)

        root = base.resolve_root(args.root)
        patch = base.decode_patch()

        if base.patch_check(root, patch, reverse=True):
            print("MVP-029-B est déjà appliqué ; aucune modification nécessaire.")
            return 0

        if run_checks:
            base.ensure_command("cargo")
        base.verify_baseline(root, force=args.force)
        if args.skip_checks and not args.dry_run:
            print(
                "AVERTISSEMENT : contrôles Cargo ignorés pendant l'application. "
                "Cette option est déconseillée.",
                file=sys.stderr,
            )
        candidate = validated_patch(root, patch, run_checks=run_checks)

        if args.dry_run:
            checks_label = " avec contrôles Cargo" if run_checks else ""
            print(
                f"Dry-run réussi{checks_label} : baseline, patch et périmètre "
                "valides. Le dépôt principal n'a pas été modifié."
            )
            return 0

        with tempfile.TemporaryDirectory(
            prefix="galactic-mvp029b-verify-", dir=root.parent
        ) as temporary:
            reference = Path(temporary) / "reference"
            added = False
            try:
                base.run(
                    ("git", "worktree", "add", "--detach", str(reference), base.head_sha(root)),
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

        print("MVP-029-B appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=26, SAVE_VERSION=27, "
            "RULESET_SCHEMA_VERSION=11"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
