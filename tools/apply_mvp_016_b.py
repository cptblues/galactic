#!/usr/bin/env python3
"""Apply the Galactic MVP-016-B ruleset migration safely.

The migration is intentionally self-contained. It validates the exact Git
baseline and sensitive blobs, applies the embedded patch in a temporary
worktree, runs the Rust quality gates there, then updates the main worktree
only after every validation succeeds.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
import zlib


BASELINE_SHA = "9c93a541b6722bf4b931bf73ef6aa093e0bed3cb"
PATCH_SHA256 = "9c7fa06fb312aafc30dc4487ce0efe97178816f5727d810f82289400d71ef11c"

BASELINE_BLOBS = {
    "Cargo.lock": "ff47f3f3e5bfeba990af82967600e79016fbecb8",
    "Cargo.toml": "caf1fa67aac4fa9ecb8ec49a78630d893727b28a",
    "README.md": "51e5f98e291aaab5d51cb96c195e05c880593a9f",
    "assets/data/buildings.catalog": "986066b8fa181028638d66264d468fecb721dc5e",
    "crates/galactic_client/src/lib.rs": "c18d72c063e785ac667494980a8a24546780af72",
    "crates/galactic_client/src/research_ui.rs": "d6f3aa50196cf373c8e70c60d134bac60c6f2cd3",
    "crates/galactic_persistence/src/lib.rs": "e4d4eb133f62423d2dc4b7918e3f18cf7e6722a1",
    "crates/galactic_sim/Cargo.toml": "fd88d51eb84867abc3b086a4524c3c9c7af4332a",
    "crates/galactic_sim/src/building_catalog.rs": "9045ff66b6d84fde4623991ecff1e6a5f3bdd73c",
    "crates/galactic_sim/src/construction.rs": "eebaa7a8e099c8c937dd9235bd0d968e9ad17fa8",
    "crates/galactic_sim/src/lib.rs": "8bb0fa11ad5060e20860e90b6afd4e176a2069cb",
    "crates/galactic_sim/src/production.rs": "6b14240bcf7b2a0f607a78f97b6f5724f721cd72",
    "crates/galactic_sim/src/research.rs": "a487deea82ebb94f2aed34efb835cb3df6e2ea7b",
    "crates/galactic_sim/src/simulation.rs": "36be184dee9e445ae8659229e3baac05cf40b42e",
    "crates/galactic_sim/src/starting.rs": "ce0995815f0d2002df246e73a6e5b1405a1b2b0f",
    "crates/galactic_sim/src/state.rs": "5d046759283a45fcff181e84e5ed12805a0086b1",
}

CREATED_PATHS = (
    "assets/rulesets/default/buildings.ron",
    "assets/rulesets/default/economy.ron",
    "assets/rulesets/default/manifest.ron",
    "assets/rulesets/default/starting_scenario.ron",
    "assets/rulesets/default/technologies.ron",
    "crates/galactic_sim/src/ruleset.rs",
    "docs/ruleset.md",
)

DELETED_PATHS = ("assets/data/buildings.catalog",)
EXPECTED_PATHS = frozenset((*BASELINE_BLOBS, *CREATED_PATHS))

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
PATCH_B85 = """c-rl~$#xq_vLL$WS41*TOF&`}0D>S0rj*U3s7#&GR3w#kvvqZmK_Eaz2~$M?8Y;<idfWT=0r%Whsn>7UOa4iJ$ulz_!pDdJMb)kQPVr<4K!lIx=H})$j^oKBXf!V3RS-Tn3X_XPbGjJ43(l1vTk(7xeGDd(?%rf)(rkw9R(G<09<*DnUbnl|Xf(9CTeVtEum9iwd(hbFcj|jVjeWp_akPw}+4(4r(%@n6W-AD`$BPg1QJAh8Ng8yTt!8Jtj*p{wbr~h>`!ZTxB;oQBel>#aWHE;?HTn{?n(dCLl19lm;$LUsboxFTv0tmp7~Um|ggR*N!&4kxcdw75V;`Q;P1dV*5~Yo4yy8uZC+k%_Wl#Q`HquY&27_CLqj#*C-0`*dx_dZ4{^=RvdG!9%aFvAfbh1chtU-E)15DZPag@aGqv32pJ(e7%V^r{Ty^hCxe4X9hy?QUG(PtmWHxFkKjBEQMoQ9)SJQ}9)Y<nwsA0=rF9l;xDsM&%?u9?VM<HanD^#Q_v?6(}?+pS<Lc=KinunaGvw{O{!cG@i*5`1>*?R^}R(s`SAo-WqOhz6QO7je2uKGiN)t7STP@BsG2<@&riTFf3KYe33rIKOx>O2So?HseKuHU+dWx{OBe()Em5-9O)Hh27or)@a;1AC1C(YX|nsxHTT_z&N6CvcJ32*$dlyqj21A@AtZs^L~48|GYaHb^ALVXl|>fPNXKlh^w?!)1ile*s6W;3~K5O&ZALu-ka>5Px|fA-cGc)6GqXbHy*clc6;OfuzlV+@9wwuqxPgV8SO;7`{%74pa#2po$=oJ-UOPn))SWMJWQirmo3|QyqZkI3*6%E9dj+k*CE4G_GCSeM~iXPh@sULJ_5k#3m#~2GO!FWdTVSGy47Mfm4pg^a6&f<C+$htYc_Y!JFrZ<E<#7eRYFG(Xe;c~M%ZWD0Jk3u<9B07u!8{aC*gX!YD}V#$Oycegwr&t2g(!J4R3?%tpGOlnAP3iX1|7`%MkvP4{J$=7>E`KMYCrBw$?bO2HVsHKvZ2i0IJ-DqiGC~r`DF?>Jpn`X!O7+vJDMskUlv+eEj^lIU56@<aZ9|-FCD)+3y3KyQ4|CV;m@1X$I!(HytNEgfjfw?jo3%(fU1(Fj^&1kVK;-ieQE?opqcB)BDjXTm`de6-P;s0(lHZi#Y%YAQO!5|8_iI#j8)hH4y}l7o+tIS_)SPvJvd9RWt~G8!txbgW3D#Fib|5K>tSwLKq8Gs(t%yaI&68X|w{otUZgc`BgMu2REPL+hP{~WgP_wrb!T`E1(#FF*l#Z@O}$ZsF}hRr6$q`95EjJ7KCfqPB))tVUhrzH-l#p4*ll;tWrRqb#n7Lf-kG+;|f2|Z~kKy<Jjrh=yJZ8E-qqxGmU33W`svDiJ_CNTC@tri}@TIz^Bm1GE7zyj5q%q;W%5sQpPu*QwB2t>teQq`bjX2f<H&=CeHRJI0PbW17<pjFV+czoPZgh6ay7(S^(7%04G_%<KPg$7)?`z`xXAcidXV|aHXRX2h%uQg%8fx@pK%|FVf}+e!@HfL2}=3jiYI_ipB&0oLMBWjp5>czt`&Z&YR5^`{PKM-zFj^tk(Mn^YxT%{x;%ctyOOUQf@WA4SwR=BY6JHt3Wgmh%vu!W0@y#R)?zreibDizJpO+Ax*oQg&&90=zTQ3LK-?8EdYU@*N^H?uJG&dB3XP`T@IH~GK;6vh!q<1RTx6g@6s#z(<~g#qvYb#5I4hmwxlBPj&~PLCV=Kw%OpyozW|;h25mJsF?k3hr>@e~0#WlJ0JBx^!~Z>m9lR7Xfn66)hqHJdT|I|?5L0G1pW)e6yLHv>V1;@IzIAt7S350y1YdertlBbJjMs!01AMHuJ4!yKSOkzi0T3^et7G;pS|@~#Nt}vaXlV7$F7Ktod)d7bjVS%V8?B!SVCXQ3CNTEo)742hp>Hum`7k*L*v?nfOSe_uZ`Hf-Z5QUT)5Bi+t*gB&YC`E^Pw!#5_y9m(PO+UM*nLR|7<%&=*9HKEw1b)lI3c)1`(1<;0<0yLQR$(t!|FqrM3;+o8eIXYOQPk?|I0=UKtb5-2=MQ6z;v$ovJSeqtN<{89(JfL0AS53VI2-fFzzT}!xz**z6C%-t99(IPZNO!ZV|w6ZLYM2(q$Bl2W_J-IIbe3yu)dDe)TLohrz8DaS}nxJ2<vong}!vgWqfM>9jjnvfgqL16>3px^fJA&{n!iFXQE>Fd1JRUBZIUfdNNe44AZgwTr-jzB-6O*w228uSUCnB`V6kTF$;Y^=wDL4-1V49VgYZ4u^~y;kkcauH4o<`rxC0obWZ9(Reo;PdY>aGwQ|;;c~qoB}5un19I5L+san00(>0&aq=+;+I99oRvZwc$p0$YSNw~|gJ3%Y+jd=ljdT(U=;XyLG>%dr2}I<eEYS*p?^F~I;4B<PX&M4n!Lj=>i06|eWUEKq5}cU$nC@z6@IH(I67ZmD_9N*v%&}#>lN5XqRA@R-2DWs-kzf$O9uJUO41!LpYE(AJ{g6mFGytgX>P8a%6pror7~s2OmUEm7vek~&YTjW1BR5Oa=`jeN<1}AM%2dJjW2%T}liC9GXiix>djLV+D(}^@E_AUrmIrH`r$#>V&~ylSQeFWNBbqIzi%-Zb0VhSTQW(I^-)8VFd>_t7@al_^rKKWEKSP$?yGNGJS45Uw53(Gwsg**Pwms=DK^N_q_n?aB@XtpUUI{6Utbs-FgnYk<iLl_$#q#DegD`wg;EoJjz?UOTS4Eh8i7?&L)CG^catPDg1Y!D{A<Ui!VV=NjN@2_1og$0Ev3XF%&Bx_q%Hfo5ZxIoX>@tVAHVt8cri*zTeKAJ#RE(h0=c}PacasBWr?nYMv^*$rOtloG7=_Dl1owNja{o?`8p&S!ju^po7p(vKg0DZUHXAev2?AioNzO%d0<Oj}-o;ZU_xo}Z(bo{7B@trhD{vsYU-3}sco5>0U1f}gBS^73X4VAVcI<NUmY9{KE0JB2-Sm_(b**A|%H!Dgi#&Q{UCavbaRfZe^yY5~qevkz%F}ou&ZzNyokPyU!`)&u#MM^NFyn3drO~igmM--+p-bHj=~CNAn2r>{pW-dIBIGc7^*bd^>c!cE8p?ILJlo6exIsEXp>I6F!%ZAiNi@2Qpvx$@m@dxY4E#dO*wL6?!V`uW4p4bfqb!Bm+Xyu_q)=@ii8|q{^opRGdq*llj*6c!1Y#J~O)yA^HGG7{kdhRk5DA(IPcm^8oooDJee)S6vy2$$!}#F*OLhqi+SM><C&Qpl`CIm0S>m$42?p(MN?iKBLyoUfF$NWq7yDEzfm5Ao-B(YQ+mtBoxVL+2x$qdDRm+LcB2vzLDiEM4rAmp<_S<{CZl|}A_)P6X5uWu_(U~f4*F*xkOh6)cFC&lI(gCSnM?+ExWxGe76WUvGrOUP|UXh{fWrpv&<>5UG=kX*;R|No9i<N@-&Q8D8--|YaxYmgR@?90=gpj<3AVDddY_o7^2{M_=Z>7H|2^d(rWEIaZhAHf;Fo_oh5OS6;1**~Rc{?0$hMex+6qt2dD$ph&!xRLbpl;$BKQs<gARfW-$7M8*RX&vNxSzC|{E=4iRRT9-;HU77sCv1W!R~-v5Y2`(=d1~Og_recWxk5%F&<h&x?yFTs;3_qE!TnJ5nOvoURyh8i%6ZLv>~C&DXEDka>^<9*1S>X^AZZ#RUxV>pcS4KXa`+eQCWq!>XU6M_fY!Ejzx{<1MtSP^=wGXdl@39a2`+LqEC|1(tA%GXu-BKi<a7P?N+$k8Fx3@aaKnvf!fd2rx<WA;O!XCHXKK2(cric@@@C{<})uK*t{Z97QZVJqc_fKk2SG)w1SgGFof`X7N;q@TapdOVm?PLOE@3dR|w)*l4v~SoDNgD5kuA5;QEAr>-m&6|6>Be>-oF+;=}xuC`sxpigzhQOcgBp#Ns&)ew}V_B2!Ej%k?z%FTyKt0Uh=*s`KdrElvog3sh~Ss(LFWMHZTTOv4f9v}4@XFy-l#N|c3nr&SDZ2bTGk;Au9B<SYq+!sIW&FN;qo7Sjj63D8Gqx-}FMLf;if%HpW_ZG?EZA5#(mt9Ko#M8usyW}C}JvMLY!63DKil@IE04u?Rq#$?Bq5Af5)8tsS(4pJb5Mnu~i$s`ad#i{2<_+wch7^9MHzAB<RYdAyWG(v-~kIU&I=2l!pPK3Nv*ng2UJwOo*Fb2?cS*Y)nL3snOe2_mONM!TO2mW(O9`8r_pedv^#0-VWr(rk`r=QX|^(?_5y#_Er1yqw4%7{*GKEJ>D+mz8BfC3Ei=5xYv-=IP_{|*ZU<-_SZltgp}w~FX)w?KLNL(~0h^oqb!K>P)0qK~LOrm+*0{<hj|H!w{6YnZbUh|`VJaJ32T4}%GSQf`3xVh!Y!R)wKHY=m{fkBB+bN6rvX=hO1w@9u8C7wS&*^R38}#d<zI;v>#qkSFSN#U!`ONeJA=tPj#;^k5pFH-T247nc#%!|ncfuQO^k_xD<z(Ws|ewHCL+Si|CSWChE-0`Q~VMmyMewwwYdLu@1<K*DcH&jHXEbkv^$O^kxyw<Hu#qv=Hbjem<}1lYukk@Yt?nD;1OlNYrYi+O~81j=6;O<|(}wun2@AW*#k27@2@^FIK@1_Lq?LE{}`eG8l6?6Q16<aU*nsx4e<j`8M1x6Rb~ehxI%`2vm-7)E6q=u`{{7L5bWuhW%3jDaABC=?IVRRFM%7sG8=V`$fDfOHZ?<MR-a{dAS!EC+C{s8p3w*IWF9EKGOyI`ux>1a|g%%*1>`T=|Fja3)td1IZL<ZzWlL7zFob>s4@c3C~Zlbd($|W>90CR?X#K$3wI!9s&VbRob0qt7;7S4BjGQB^CO2Fi4Y&^Gd4;BMVy1ejoqc?^M-l)#^cpB#4*@3E<pvG2`&rGZ>qWTi%(TuUD(Zyn++1Q|r)UP3<wm0J~p5D(Q#=!>Qe3I9VG_J~C@@@flm;&R(ZYE(Cj>UFPcpD=gbp`ho*3jHk3#6||wozbW7)F7y=k6PozHk4E3T*;0-RIS@QtnybZdy#yv`6sA!{J*t#~$`OO*Q^HaWK__Iv{7ps2lI<5jD#A54d>;hAUkAbe{h$8<L<DYqe@p3z!ppPrW)^<LJ3T$xwn@(#PH_@E)KMh)XGR_P17*NcJHUOD`YpINcLAXNR<GXOHzCSo=<FQfQ-DM4eH?vgGO+`Iu{5@D`Ta!_k756Cv3pu!A7b67bn}(!53_krT@|ToIuDoW<ziL2$47=Ov5f(MA6>HDq->Q^^fq|-NZ>PTWpnbX?2Zayw(^R>l|4|ljeBUvOp!4#>;;40gJv_hW+T<frW5A~?`ppfnw@KI*!dte5lNS5^I_tthDUBQGYViL?utqLs_Nb?1#oWGb}Ynv3fzkUV%fxf2}rnk<EAiC4Iu6eDrmcP!o`^zK}0wD`XX=$gS<@vBJDix3>vRcW{Ve()kd%|t(|TK4IsbFObsr?a-yXHVtF5YF)aQh<kA~~g4_Ar3Ids==J)Fd^k2dB#dU^}i@Ng_-!<-+h(TXYSQOFx5lQAZBQ2&y3cB}MHYrY6;t9syJ`C2q?qD!U7BhazHR*#L2P)Sm>YXp<Yh+C}JaZa3EImJa>>~UO(q8;-m%B?iS&b9wSjC$ux?zT-Sevb<40hGv0bCdDCa&#AL+q_(+`O(2boNvQ_eMiFaBrRM_js#MEkm-P_B(ZR{ol4X$6{gq@+>^x2z_;{nVfx3?)6+~p>un59Sdx<t)rBZn=##DM#36nGy0{OaNPBDM%+$EpN(>_?DSa(1pdi*WQXAZzjUw;8K(+&OMPlg=s==|YS4JZFSf?*;0f~n@8fZVR~I;f(1C*MBPwtw9mnZ%F-LI}{Az9+h_3hd{P16fM=xKTo}IitI(z!^#qf`>k6#}f;sm`4-VyBCX$QIB*3mVZId-=hgxDuMB3IlXx%rH~#B_Mzu|czW;FuCdVTjY4&+uJ6UKkLY_Gn-U=QyIqEoZ&ajH~i;f;Y$Jp8>s;lX9DhoA!2g>+McZqb~%4Bk9wCcebtI=6`WUg8keIp1yc;a>&N``s8@4@tiKE19{MHHKeUPRcT0TY5rTKzWkA5vM34(Gc<m4tl1|xw?UwLuknfrB%7)d*(kxUWZX`NUAft_YMq0o+b7>&__D)6V~oksVAIrxOQxY*X$KeKE6Uh3V;k|pnfG31xlmNnGF~XEpxG}J6;@0bii%1rhQblsh73hjRP))Qvcgc(GAJa>k>w2BQSHLz_e&t{Qtlsdp(~(~qQ!!PUGnVd0wW)g9wKdSJ7hH1JOWRvLqQYv_FK$mC-^B!d7J~FRH(XuPF9OG8t)znjIC}F1+39=;Gav$LqME|UnFYKB)H94xU5`dFRx4)o#gv#uA7e_80I@eQs#0CFfo9gxTdVnwExe3UaPZ!)~3V<eE&-{`hNK6kv2yn$J_rpN1^gH#!-OPVDlOZCXU?wXK-4|-=@6$RK7Wbg8HHanG`5%KxIAhn!-Ezbc(#n_Hr7<a{~t{8c(aeQ}65sHTVK&$j*PQvm=-Mmm<IHp7_r*;Gbte-Wg!BoZW7(hnl@wx4YZpD!$ob8~~xmE5ep`(fiAKv5G8VvLvbN<wX*XqahVDgdk9IJ*Dm3Op#z$JZK${bNTRcdcE8_$7hGnhR>hAI5rq@eX0n*;GsU0A`J<fLcQdW88oZLefm!N$fhVeQKpABfeD;e*j`g;7%SUYK6rc_Jiqy0P<^|qs*pTYRP_s0U>LQ(OOfunea^va__XChiEz1`qDum)3oAdtX~<QFgTWv14k70EnqioEVU@~a19$Lryq#vaL`@eSU8_JtG@=s>c8o?9fLLB2qcx<rg&U>GrdN@(tV)eBdjy<W4A9+%EmhX$K$4twTxffsm*qAH<KAQ^40oH&?qt8)J@1ybK`>e?v_&w=^AJcZ@&{Rk2M-=#?4w4j-5UT;HI7G2h0f~*DTeBr2k)ZgDp;&nDF%~T0sDzj{{kL%3@~&Jr`J}1B4G@uHW)0EXu2K)<MG`Ad#ob^m%=HiTBje!7~QHK;LZ8u`1JVj<mhL9YnDD(#~9`?`}s%Qp&W666FEFP{^{w_@a*Z)A5Mp_j!%ZC$8ZIITt`L<CUy~z9&^5^9-IkN+)q!Td-WJ$;<w>yL8p8zZ>YJ$${P`Dws2S|70{hO@`f5{z9O(*%k@gL36efXb*q?CfN4!QRI?a8+QzG^J}ER=Y1;g0nDN-A|H=l9@H>XLVU(r@47<`b3>(o35xmzz0*c|7T27N2u|}_B&Ddr#7ZLv|jP*7QZwd`@oHjNCZ9Rc8XU1-ig{_;%re@#(gwy&Z9)D$XnQR=})i1M*>FkquZl~3j@=Io(p-lDfYYx*RbC^y&XhWr3YnW_^Xq%!hDYJs9`HQ=jk}aG$6$>7kl}h&9W1xai-tAEw-`!qjo3dWwLT6+oT>~VOPYy*JaVRYebm`T_(Y^fS_~_?jAkN3Z|NWo;j})gWz72*5RcAL~Lt?1Ex?2@v0au7wYp&9M*uufT285_OhEX-tC)fH<K_C!sP8L%*#ZDQ-n6#guyw@fu?{#khWnyo_^V9<%Mvpx&iK~%!o+ft(H003gpW`fbL()EJ?gn;GIF-nN*KhA5@cNyOG>|j=8;Mz`saj$yJbN8ND^<m&n8mKv$=qlDBo!S0I{(-C@7L_#&D0zy2&@-_n2OmR(mBvJFPeS0ULwfoY`aU4F>S(fonBUKFl}>~Y@jSDg`c?q%Ylqb2f~DI{D|3a)8%QO5X^TM7velJKsFvv;v@x*X^m&6``ko#w!pYKD2IhDa)?%pN2_?Xs&M|O7uoVn2oOFy{E@X`$^)c*+~yW><|x$Gvs;#WzVCO?;I+2D(_-7&b8*e&0~reImXbx1kB0xau-U7IZYQsDn-xdn1?`!~kWu>8;n`ET*FHWzJ0_K%Wv0Jr_hy4tuh_*SWKhMa%<f-`8uoYkbQRq1b~vkSZ5JRj8Fd+=D+{0kv~d}wLvCu0k*+_4@rq>0rpOC<xNMfN;@OYkhj>Ldn2H;-3Z)ui1DPfr8Aq@o@10Z8URSr!;;Rlnb1*DD@K+}<U%h^I>KJkUepHVX3rV4RuApRzKCkI2nvbG<4-I!gE+UD#<8E}`ZZ><J{hghDm@gvnH073$cnUM&2<kxSUknhf3P*S?-L3|szOyho5_rw6T2`&`qq|~u@ueoX22tF^D2_f_p&7ChmTUp<xwLXjgNITAW@h*kknno&gz3@oF>{o{Zzo*U@GKf%M2Y#E2`{z;14nNVKa=-dCLtc4q91-eVBWXb?(2DsF3y147+gI2jqfh-WI>jA%^&hByGtBIU5nH08J$4&K+#avweU=TWO5Kb(?5_^4gbMQOO}Tp0I(4rb1RIK<0o)%{>&sY0!DwBFFs5W7UF@J@hhg9fydHF@Fc=j7)#BB<BgC8i39rR@a*u}%b$i%p1%0$_~g~e(-&usx4#^poMwl{mki85rY((2=g>MOsRJqt!{j0Yj`mb;di6W451*P8Tpgj4mk_{d56zNiafUm}v`|4wR(o|XmtqRbzOZ?|&HTYrC@i}WFg2FzbGCPb)5Bkm#VQSg^-c$9CfefF3W~Ie;C}mtWpk)Jj?UMxS6~*X$asa(JHqMlU#f4n0&Iw(d2kxOj}VQ;nToGuIDs|Be{G1z98AN4*AoC~lq?gnURd|KN)bu<3!1XZ*=^QjSq5hELSvT>$4fI@%UTAuRZCgHTinHDusg@FBn$;L8c1Sw5>286PXnbCzezn`uR^%{5jlbNsEdn6e-~5M)cRy@h50SAb>mbEO5@6T7X4_k{G=>BP;j~Y(R@9l#U}(8tN(hQu9xV39*uutK&Su=1T%cZR)fEJ9;eJwfJ>!#?u(~m@uIFhKcywG>t;c$=<0()NdmiKxUrsV53lv%JjoV8X|~~LR^p=Ctb8b%6;#VR+lFVv4cCbCns$#|1DZg!0)R=>GiN3M#(V}lWSiFR?23FTPDH?`C^hBYMng2}9%c-h!mAB}dl}tk*M5VXAloOlg2tFP#nX!F4n_MxFUQ$Ie)+{hB5y|CeUo2wNxjF}orfl6u4EjlIczPcib`yiRkmmtf6(L)v^%7ekckS)L#>jZT>1}ne`T%fT%IB0`&Hs9a=64}xXx>qJyvm(+={CDb7xhH28+7J3wOV6$}GEgz19R`IaJ$}?I1`*ak7Ze1qxB5zQ>>34YAd(@1SOMzss%f_;!Q!S}kyBwA+NIUkZqcyNu%}?J=MtfW_Zy+>_Y9nw>ZoOu~zbEHD^2Xgu~J&{nfP*>KNM@puwYdj`@5-#oN;h)PY}qb;VwIw{QCgbHzVjZi6hU7CQ@)G24%1!t>n7*%!So~U(vX6xVttAfHRXf@0(4c>w?)D2=E+46x|DPMQPO{Whk3{A>W8lFD3qvoDl2})&7&pmjT?B*I%B5CvX5O|!qkHi<RX?X@lpL>Zz#hSZk0>)NqIPf^{d~(iTIiWU&!AAI<e>RqfMG1U&9Y`A5yHBk0*ze#Ui!k4JNEUAZS55am;I1?*8Ei)=k-y$aXc?gmR9i-?SfmIsveExwcrn6iW=@a1Ipvy<ITZ1Hq-%aPc=XUK9x@kY<A#001zYv+$C#&cvekIS8qXv<DDX&K&1?Wq1!D@P!`*1euc)@u?$`I78UwP%S&avS71^B&vnI*UJLQ?W6p=5cLrkcuzv{tHhtH3Pr)P&}*&RSP(^6&O!t>H4e3VQ41??9KudX!^aDyeZj^lxXNF~0r(=Ae5#^Zy(zY*vrW}llHda836V<M)gBCv*SG&Dy`D;;8gxi1$py58xv^K7pFspq||p7;Nk`nVk2YOBb#zgusk7ZxU8?D@2!f|EeE-3wHPCJ~Qc7maN-sHOtV&O~M}rxU-yVD^4lshXD1q)0`P;zPgm=U(<*#Z>Rz<li0!Z3P`m@IS1EL+MN$Oq7}?Hg1059`<BjOE$RukmI#s+E|*&;+5Sl9rWefH@Mhu+3-->hGH&KbK|%LADs@V!6D`xZnb+E=U2AI@ce7w$!0hb<STNate%NGzW`U<PR9NIc(>hb_IqJ#uhq#{+!{@|6}CnpGQg|v1hriX2Osb#<;@R^<XyT9DcL%#e3Y9qoADl6%Is$IftlAH&;s*z+fB3l-S+cBfTF_Lq|R>jcur}N6uH&iolJTF>YcFH4bO`~?Q72ucV7*rE#K*oD?z`ltIKx=L1tep(<&<tqQP_lOvpGu{0u@w{N*$XFpV?Ol$E7Km)Wb&r&$O;t|*_xurElDo^>k3V%Q{CqZ3=o-8gX!$WWdoQS>}q)&u%>8m)j$YskXsr}=0wcnQbGv&HBg&`SAcGFwUGb6Y*Hs^f*e_Gpo=3`P32Xc18fcxBYM!+9A0H6pzuzM>VeIA2{+E}9W#^V}c|IT=%MGR@`j^B<2NKR$jeF4u6SgHr(TI-ZZvmjlm)Zz{M+*nhdHw~<#;w4!A2_xc&|F=xXcUq5~J81C(-1ArdP6?*7(X&gv5<5BSV_{rhxXJ^?s!~%RT=FLxsM^II~<F@C}tJJt@sy6NbVCdZB&zA{+mHzoTOfRc%+2FZGPX&JE-hDnycv+SD;mmo!!Fu6^SXMLUAh;!C(#;Dvgn>!J{ZhHQQh6$)fx%!IqO!(ZJ|o(W<ZID+Cz_o{W88kutmm^etk`+PHq$n52M=8Gq)>q&&OBwICA&Xh4igXQqoSr<5Ik^J>*(ayQ>=IVFMzs7XD?5T%61|KyNxHWkDuY?9lAdG)u^whX0YqNdif{VBd?wvzBn^#s;L_6dVe}RIsW<O>r->GS>guAT-AsmN5?PDj;+}`5<1xJsFwnx0WGPRUGwzkr>}lJJb7%?l8L0ShFJ&Q*@=}PB(A)RFrI+A6BLh`n(8rwMLDl_?Q9m_S5hdb?-HauN=sOv*)p0DV`4`G8xylL&_qyBwQg=2DTk6hh?8TO9VwN2ICczQFpPk`ysA{Mob5;n%~M$wB~r5Km+vYBi60jq2ZL!8zQdt5fu&sx&lew~F%nkb<f<8%Xo|Gk;GDsO;e35TekHEWhhUf?S4|UTM%<XloO#et15d=m@9z&E5pd`+gOK67!V2o#b#sg(7_QcENLRMiL6|{w;bI+4$80domi=-br<WD~MB@~;+NZzoRGS}?ctxjMMH{skUakd)`&hyA!#F$Ws22U(f|9yWYEjmfb-&JndTeGs;ySY!iy%72Xbddzi1z-#eL%mJqnfiZDeL8KKik>I&tIMCWLO2hXB?)u)g@3;$<X$;DgAaNpS>=aot$ASlhi4M3UH3eWEj%fLn0(w_bqb8=lG`CBsPZ&jp-G1hs&kQ^#Bzsjtw)eTwzO}t_WzKC}v6U@L^y?a}|+RfN7|->VaN|1&(u2VZ5wLp%ZN)-z@7HZfZ)7ctZEi)5R>RP-j)Ve%^BaTp)K_7?>KAZ0;~7`aCf0@loEcJiIn0MpYjMEwgD?MHQP?0lT0*44iXUDhdgTp=Y4l-@6glK>2OSz_Ia)5SEOOV`2|v`YeY}E%yF_2YYEGZ($<FMh%o$c3N0NG0C8oiI+^ABV3n<KGBn>E3_3}B;UiA8FW|Ckq@x$k#C2Yh=d2Lb2qKRcTvT;AdZbBQLd8VTwU8wo()B=*h0E0CyADz=J?qlS!b0Y2PQBXP*x4tS*<qH^~`x{s&370itO=f)_AnRfT;Bb$DVPCQf{uA6{8+PQCwo|5Yqm1nCo6Qvnq5%S}s@9ib!0pnj?6xD_OZ(B@1$^6wogH*>p-pE!CU1LTZ+qh!wiBuv*QzP>PdYUIXGB<)4mqyIn)(GTyEgdsP*cjC*;F7UUu;129vSXSJwZJ43l%z)Vc;fTE2qS#=NI7)_Yib36E%$h3JYHWw)ggnd8c8CA2^GMf65avprIZ7=PV;PKK7l9!P^$3l7*v`p6d!hG{a0SX>VjFUD4Cdv9w8Bx__rzwar%OGR&NbcLb&>O)hN?q)WR>+@jfJ>Aza|JU<AZs8mJinAH6*c0GXwM`|Mb&eHz;9d8+1*Xp!PB?-#_&wrakymDRs~EczI8X^4d9$fm{`yl&zOAylclK1QC(u!E$+W8h$h-mwEKI^2ib8Wex)S9%JI-}`eqnSJ<c-LC{8-A+r4z4VJX8bcNq3q-6hIPEr*z1Ms})XTqo&)c&?;d*EA7X2~S4#b-Tw*e)`>duM^ZdXqvJo1M0Xz@CC5U<l(xFf99*<@#)d?ZtI?kTSlnRbPx!Y&-ZSXN;aQILI$JF*AUxto}Y=}S<g&wQeFhvF!lVx)bt@y-UGXUI%9t+RonJZPFw0p8hU`63SIQ-K@xr#VswQn2CiWfYErZXvLIQ{$fHxYUzYg>px6+KoJUM+gjd)a-T|E@(cPH$)r8$Jt5j_xK6)9a!)Uf#eS%J}uGk~Wc%705dFB4M_p7d`{^*qu&)1RB>a{v?Q-Fb~4kHv`@Lpik0c8d_eynWsRe8AW><wDs1#rA6HdgiJ8c?kK+~VZN>*bW<u>Bl`u%Cc(;&=ucfUiw(dD0MM$V4-AkBH{&enr<|hThej5>iM$ERd1EV8eG$=?6y)8`nJVdTrGW<zHa6)IZoN8x(;iOpC@cr?bh|)uyXIYy?<w7F9^eTs3tss!<zRr<h2<{ll|zxm24<-9m{2i^TeGD(3oax9~1!WgLDudb8E4S7yiyW(skgd|9t4G*0lyE$Q2M<b0FKmly%db-R%!zpl|ZZVf!|+n|&{@>+*ZVAKEN4arA@n=zWEk!zEc1;#TxY)WmeYsXQgBqe;vj(H^pp>to^$Cz>wH{&_Vd(9x^YI?kCK5Z#aSHx@Y<ZC9&5RaX#f)?5?v%Q%I_e=ybaSa=0!g`k$hZ7G`yBUw0BV-sWRZV}&p$LN$&~wz_DJ3=7)xI2dGE0&#=9z}yINj{P6%4`R9V%tq9a7Hdx$p$ikSAj)1Qlz1DHU^(uafMd7Y8;#0a(E|_ms7J>&S+rr6-0uqPQcHrB>7x8q=o;$VB{V6IU*b(J0nEwfK-QgUt%9b@aC|J4v}}KCY>72|0EcH&$<ZLS<UdFQSfffRR4msYVaxEPlvo?b?jL?aOnem_m52;kCR84FU`DIiup&U~M8kahRFrA@7-z0)n$8EE9x>F?m$6sF-!2IB(Ill55Ij2DE<j#hh~)a(huy3=c1Uj748S0Z!h2aOT%ggbqe0ZaB)Wm8u?)=M3v{R=MKJ&q1u=4{jbsNI>LBluWZVC(q0zBeJ}F#)t=gbMiwTnTc%2`QA5%ah4ur^e!FbkUYXdy$%c2oO;TRE~C*qObfrBRx-ymzkz7A>S-wN4C8#(V<X$28>_^)bAab9<7khIPf?<|o-Mi8T4LcrUNKjzEajV0pB^|$YG;ti2OGQJ!3J8qpB~p&x9!h7{kami;kf9VZm_D%Y%+h}4trfLr)q}j@ch#XB|D}whJHEI?!6_LnIEA%mseXjkcH&XE>uP-wMWBgQ2}Dz9rRVk8Q}H~M25*)eV0Ol*LK>w^`0fjG~|y`NzX@UL|)J>BNcbtdzr?+*Hzbu8dFM2*K1W0%NtbZue7M^k<Edsm9&KPJ<8`(+v)V`op#Z@Gh=44neXKqKHc-n!sW=6H8Y&;h;MS_26aR!7uYYOl*8UlKFKX=86w&(IJ^d!jSRW>QK0V2$Oy%^?J@GeF~ja5C6@pY*~ALbFV{_~@Ujh9RKVz?hsLb9?r^FsV{|Wb8c5-Yu4r6X8SX!5ABDEJIx*qnm*%eAPKD2b{TdjyB&s9^Z(4<tY`h~7&kr!w?|m#*x^>GQt&;>ZNj!8U$1J*(8;}&0v<nL-w7eUVZS#)U{lIl;A(meu&30F*g1=s7zr;Q5-F)$3ZrwP|v&0lf3Ya}oQ=v(Qr@+13Jrr8*<By{#8mG^h$j9ej&UjK0H;3h%Y;kYgBDyNBoSD)SVA~q5$qmFTq@3AGsV~5_l4+4O{811N)gmSukQgX+qftS3W;VL2z0jH&V&E6q<_U?*?i#ac1@y`<H8F8G9y>zZXDz$YCzm5*Xw_-eJBU{Oe!W}jxX!$;nAAeac&-k<Y8>SXHPp}AuS4!&LBFvONE~YHJYfXoHnKm#fjMc3F!<#LHPY5eaRx7S3}`%vB}bCxrw?tm<Uey))$(>#&A+Q^4n>Gdza+7@%G~fp|Lp3}w9S~D5=sSnrHq2qI(~@=OBtB+-q_Ds26C$C&O`3Y$WdZA-0tQ!1L9Thd`M`>bDQyCz4C*}6(p;YUu!ckJ<&H0Z34qzZ7v=b5r#}{QNyDxa*xT(aIf-f*S2M)H1Bb#6vHL@YG=U*1U+IIQMj9raPj(GjjOo{G4ZQsNNC2b5md$9ZAft@V=E#?Y<84`gS?Q8@{FC&sZ_Z?xW7T0k|1f8difo^u{uDItHKXb2PwVia(oA+Ge@Y2YyI=KhE~6CZ>n5SY`;C&cIa<ax5C@;1GhZe0zW!t+&rFBHYas;rIvOG6HC=|3M+%}gZ8l1YB64Hm!*~OcK7OgWmvI%m4mg^p1@Ss=E{1kALJQKttQdfe^;}*_Uo6wKtt2WS7w;XUw`>RxHWumYm`yRPz+5DGsm6429{$z0Ap=U3*)N*%^a^D$u9G6tDETfRro*4YcOXK2+{Z0UADW+<C~R1be}eO<C`fN|KS`^NcB*Lq{vM6ycKBq^2;+}CKl$xkb7!(>QC4|KNy_4n1;2fn4C&Vrc6Tb?$j8l`zek=v+Popi16A7Sh(M*?_wV2y<UB%ln?^%VPf{GINl=3g*d1p-_fVhG_s8bwDCKSPUn5xIz<ByaBa$<tgN!A?g2?KB++c~KH?Y91ET=!R~1YL-UT7%?n{GW)VLHukLsJ58K4vtkL=q9*mo_Z*pr&!kYe3IMl|F<Xhe@UY#Yl|F{FMQRnpz*N)7E6ReWcfzoKQ)xKX9eK%pDX@T-;;hoN33d4jFBGF}L2il+RKOX9mmsUtGqnO7@5!<uDp$6o}h1D0*1WDKn4KrpimlbFdnimL@n9q($wR{pw5p~6zdCubo!F9Y*f$VWJscUwVicW1Z0+tN}bc($^D89S6+v1=L|1tV!VeU=&?pQYzy;x65kDDX|)Rhl~}9gEO5nqyKH@Wxpe{gO_2SewipNd~%c)_H{yZ7`M4ifX6<Zk*-+Bx2c6@hq~9doqe=(Gf+iu^o<hL>p&)C0M5JXM~Y#oHc}#k>*#V7QN0CtwP+(%-QJ*W^3%Q*+=$rQ)zR()y0+0d<GJj0usmC8dXh<uv*Yp*{H4JgjG{rZKGa>7gnuo(TzG1H|$9|R@|JXg)3Go+VUIK6;!e6Njxy>2xKuUC~l56-dLXSy0?2x5n;JqnwagoI1wBvj3iSt2+*R;)~K@<ghNt8ps;0$%&@tx9-FH0!fv=gxq){K3h0BDnAAfv1sc7o+`qbCRm|a-9o!=|(FCiOi7neLPiXkEw%b~``fPa2wA7A{#x7cHr?auVX}zuHny*+andtWiCB=;*lHrTM45~#Zowp49S|w>#CM%I;8LzVAs^S*PxclsS`!uCN&g#3a>Rux(Pa{27BY8?bPdj_gb{O|6YOtT8M&DJ#G$UpewWp%~zFMEPpqQy@W-Sd}``yqjF?oCKw?cxfrQ&H=c8ba~i6ClXBDW1;Bgb}7d7EQI*C=B=rxxSJZw-&pn~V=wKq$S=vpRva(#1JkW;Nqf?Tz6FTQe*@g;MPolD}$RNqK&-H5=gE@KTL!kYn7KwUTeKm^CJB5cLoVGH0Q(0{W?;Nh3Vx2HSGqpFJvdY@B&ciLJ(mOX!QO|Ji_tOkDvbeUdC@r|6#C;Cse!FytFVd0=@M-qNG+7I<g_%!`R;c(x3pM9<3rQx6AQc>0q2o}^!vGoPe+K(W-8nGB7JGZk)~wItN1;wN&F)oNjky8?d4wW*X?9-=f2#7Gh;-tlpH-r_KY%eM}91tKK@&r{eILY`@8OunaH#P_gj8Q-&W8@9(p=>r}vr6i!`b(W0S8Hl__q*CVKus0wHqre~ho}mv}vm*9rsjw6@RLvH$lS4i68K{6Q6S!Q@FW4?yuwxbk6mRHw!&!(;xMog(i!p~T99-TV&QRi0t?GC*Two>wRNGqGr?My_1bQZKEO{7qZGERmI+Po$5^e;qJ_^6X+I0k8(Qi!tP(Rv&u<1LN?ONrCTya=D!*a!v`icWH!@GD!Wd)}CibJvk#&`zhh;!9&8O40*fJ7V`&oHtWOIq(%r;JRMMHoARjSpcQVYAGESfR6S0|qlBj%Pep46d(;DrT4+-wK%ly}sU%NYGMf3T@Ih$)Kyp8gX3KsfNst>rhjeq7BY8gW1rpj|ykYPY`|tXgUk^B=NE7j|vVI!B(JFeY>&kI4)pSp>d%(HRU8G_nq-FI=@}Hg2BwLT&ijHOylNl==uu93s1B!(dAp`o6l9>?^xP!tN&ecB8MYNDF-;e;Hd60R<Sn^yK&olhhMB4uJ3{ktq^b1*do*3QHX>rSzG`$A~hG@SsCXXxG$x^!$-@Mh4qzAOS*vpLp><n>2q6KY*i?LQJXzP6?UuHq_()mX1BxKj&}R|^_?A5*xdC18ACOT=M`mWWDxxP@LvEw!xT$rn^?HNk<%M&**EL{ThmiZO!_iRF(pjv<LG?S*&jvSez()<>~waS|3O1~!hd4=dumg~HfrGCR0HMju#Aewybt_nen`UQQrM5SKDJxzuYJCQ%)jceqqu6D8Vtoot&)Yym4?}ufO3)#rC(Y$Zrb9M7Y(;Iy>HH=j~F-B^leeLa%Q1-ou%CUhMGYO$}k<p@o<W|g;AJB-UOLSGqM#ngm5~oT;bPnw2BhVNnbv7_k`hA>=NVnB3>D9k+9w$-sc0*#>j0O*c%HFeCT@QT#6X3JJs`*va=JogOE8b3;h8!SR@srcj>LIh<Ug$gJsbi1ZV0<w77D?ne0{^K2Eb(4G`3-7gR{0Wda1&qMZUkyG-K7kYNBvlZy)G<>F!fc6av4-lx~u!B1}{N+r!AJv5VhiA*RmH7s;Gj6MQZP@b-Iv{*)3ZZBbyBFY0eSSllyYHM{FW!5UD&r``%-DY~VVf2@8Dt7j4N(9)*DXQ3Mk%`utvc>ug$koO+vRupUncFcj#=&&wGgY(Z;vkEcQXVxSSxtxR1i{E}i}gy$Y$h`C>DJno+fz<{#If6iK~?4A;?y>WcwaS^QmW~2G%a$aTTZ22mENpOb<&m@DGg@9W_R12Rz2v!|DBe?JlL4?O~v1T$Lzl&`)8bV@F;+OT#b6CC}riPy0O`)M+KJ8x*eheF@0%Dt1NPc5o4dsImS^iyZH<)Z7T-Lh4}+x#@1@L2P7Lw^k=dg8ip$(AvWgnnt--ua;IhzEvMngjOXW4yAL|`%t`@%Smp<V&bCi~K4`12^q`S;>c>V-gb%@D-{W&~rGIoY*Ouv!jSKoxitDUiuBu|c;58I_P9Y_cATU2NO6W-=)z3NUdxo48Pq=o<tDXYD!z5nt)QnBcAUibEs~+SL5WopMi@7fwP!7g@lu1#k4W1>M6piZFRt?MYh>?#@ny%xzlOSxRUtVF><dZXVjX5i&;*(f~a0s+2>kP^JFdWjYii>J-$6%^CxM)s4*=xec9>x^?cDv#UIng$wOk`#kZyc_5sQuZ7tI9IGfFlSxo=oDA6>d3GP;URYPNHy)R6^Er7T1)Q+a#ciSHT(x3^1q>+@tYwI0^A`jsD$>bEFgT`>45{AN6J@Kgdmm>tEV%7)W&so(@dA(%K--4G{ZQVKf`5Al=<84oti4Y&>6Y+R2IHqKn)ret}lo2?Imd`sS?}G+ljA-Tu~yt;TqC#7s=&I*w!pqlCv{$xc1o)LE2@lb0`oI2Bp=%vL2UX%SlWX23q`F5M2I0sFt|3!OpxkApD{!qF&Nt^%m5rrlJs#Tn7w_z=y&7E0g@$L{hCMWp{!!+2y_xoey7qM7|guv0y-oxhy4__iT2YPTS$ocb$5$WlpV)w4f8p{!AoCzFk$^Dx{Co6Y`YwA=6W%O#UF+saQQYnH~V+#Utx*+ah@_~}llDpIYA6sKF>C}Q=%r>}FRPDK?rp3kS4vAG^#Qgil|ol<BH8Lh<T0ZKiIr%(*8(NN|h9*JDyb?@?W(r^b2na&WX3YY0evn;DxddWY&K7M_irCHrQkbW7s2{dN$FPOred1bKTUKb1`>x<bZy5N`%45#rdUZEr+xQ%B9w<l0_HZK$ANx&i*^$1tmJ?@>wCnI6CUS1?{7l*MdkWtKH@)8d=37H=3n};kzcN23B0`(WqmD{=^nNkPCS<)Aa)hnv{^20nDS6K7Ic&spL4ZUj{QX=s0L(i&tbi$6&{1KabvYt*Y&5atg*X}{n(I|32UJdZYha{Pxuf+6bGLB4b<N3(S*A@$0UY^3-!`g&7=qAQu)Ns@Z$L~YnxHy}x&9=g2PiDj<2SixyL2yr@CgusD&XlHT>39l(=|`2Xvv|td743{{MuWlGV(~njf1;ghIjzaa)46@uH^k(cqqG_xPDLMbC)wYvx4S`YXCKa;o`Mg+k?>o&L?wPId)L^?yB2IQ%=+^>#fG#w3ci>09wOi)X+LuK3_~eXQjF-7e-#Qs99H?NQSoBFPBj}|W@ciA7jPx4&QZxbwD2rdjQ!lyCb#OD<%_%xVyl|TGWoKj?#2pTl-bkjb2Ec_cp}XdLQe))RB0^r<>HZb5u)-O7zRp^iCWsM%vBEjYc3JVZPvgMyg@{~G0K;OnH3Pjyn$So$U>z7XE7aDXnE^Q;ifOL3i%CCgBD3pj%>efIitehbg8AP@_rn@AC97US}}U95)tZlTbQe;+wHQT3YG>#wiCV&<7s$4CGu@V1tqGfxj?YnGULZg)~J$6%$vjnv6QRzKr%6=f*_NAo7q+GtcOhYy5$HI#&3YUdAt&blgT0(BRsZkc(86Tnt?G6Vp#E=!mug#ORAM!DQSUvJG4N(y`lvY&7jCHFxoPT#^Zy3E@0jQiX#%1P+W_+P-7m>7ionp&E768&E9^_(kK(*d|pK~{?+itsgN^dz|NyXaTp9bKAA?ThO*UQEd{2?-p_nx%H5oL3BSgs_$^A_heF|H*JcrDyZwDyto@yT?xVloK4O`|P&qT!uv%K@z>@5C_P=CFv_1<ILY_;yz1swUnl^xhYNuZHi1NRv@t>c*`t|VSalLAUBl6r+xzx!K{L-6`1BQ}L;>a{{!@P)kD^<oJ?Cy5z9dvr%?ct|yoxB)m#0zobqZ#v4Ag5cO=i_thZhn&&SFdbG$1l!~PgD)+mP6jloX3S8cofcq^N4S8jm&=C{rz{?+j7cGlkT1If97f%4(a)8ee&xmPWAX-&Q1=G&R!O-G;2I(;x>u-Ldr7CqbzueE#8?yw(q15Nnm6jE-QkP;{IuTo-gHR1-Nj(e}3MYgzaXt*KKur;Z8Z>K5rslrq2rzy{h+sSg{j@l9B?of@Phr4z}cPR4PrQF$RN`&oXBQ<)JXB=R#|G#DvgFp=<%vhg==5KUzgR4L~19+1}5FiJAo(CbJY1kK{=8$Jt=@=VH9{v?iajA?ro6Q5(<sz^$h%HilA&k&Q1OKeTcjO?v0;Zl~L7HYfebxF3~76TPK;Y|+codeqx}SPyo<Z8aV|2%i7)s?loi41{>3F-A4QT#Xlw76L&SOyGHgDVhQP6Ig;vs5C|;dULBLo9Z=w9Eg?hHzst9$WPg=c{Ec@8u)Z<=vYZLqpPeMwj5?7<&;~8#G3VIbh(YT$~VXjhLtxWo<{x-bYH;(SmGz~$7tLj7Z+w<j$j{?4P&}KZ>;CB8fm_{r9}LBb@K8t=SohG4xb&9C{~DsNV$d`(1HT9o#0|dV~V!Nl20J|q0)*?B;Xbg^<chO@e(L8YN|RqrOT7!Cnv|JKXb60iq3Zr3d$2ed1<bv^fnrNm$MSwLO#<9W?&j}{AVJsiXw(~YXucyoYW60&m0X~iHrQg!~A?j#k$7FY_$1G$IMagB*ML|s2y9q`VOicTmAY@N5&RZDugVP_~y30)`bmGi6{6O4F1RQ$;-RRbzn*iTsboIh_z0W*1SmUW*4TA_v_^~kkLWBCLgTCfvHqN$+=rx?w6{2FMEE+S!WPZpB0iL6fPISy93ms9$VOVsx6vw5UuaU7>#$vL(1jt9ZA4HmW-yeZhRU4at7jmXDMT9UuDRisPOLS!i5X$azXy;ODwE*zxpEcj&hgTH<-M)Uwx^?!0&dkEzj<+zS87!{?(V6++6e4*f|XEtFOMgG~O>;TGAr^er+Bnbi(iN(Fu>KjO49Q8Y3aIt8xbzNE9(awGJ_ps#Fx?;u2X@8n@K#8wpp$0jTAV%gpvmvk&eJD^4b&s%I?L&h&=+Ja4#DUSOx*-~oZAlP)^xeW4CKSydX|d?AgTY$}br%%rmE5JU!*4F^$_J!R8jP}Y<U$B>&brD1GrudkS%Pb9~>HZR?g#OSKNo4E*2xwWM8aG72%R(>-elTMuS8vGK`+37P2wD!Jkfp%woWd=ebXq2U|V=*=ay=2k<#a%}h*Y0Zktx^Lsv>=2<L_xcjRzfko(5{pi)5?F_#EWUWr}RoMhHD@pQ&AY>4@}JA7=*`nd*&UeCW*=bxG2_&WKhsr(9lQYY27L&0vd2ZcM&4(p*2FSv#SU&?;edr><g5C9FE7SBIRI{SS=`9rYeBbBx>{V;sc4`^6M*h{f_D=-(f<Y6EZtYgAiaz=P$Ye1s^V>Ied!(IM`+jl+P^SW*)&6nzf({ZJKg_GX;tTGG<!P>F?J20N~!9W@PNGL-693e6~<)D^P<AyP1hcs&(ILf!YxD+%4E)Rg=`x(<a|aRV^sy0;oU=%$L}Ltg{Dx%>K^%inm)ZIOwCoI{9zjma^@B3sRv}@=s51?mPYWfru_sdLmkNg;;H62~muZ7%~oSM2jYEC58GzxsWf?S4we#e%S5rjiad9>`iv|`{yN{I@EUZ#rbMUCd}X2Lt*~zjxNma4T6j5;vB#yYOoCzgg`iEz{Abp)hZfY&S4oYJ_V};2EEl_>JMO0d9V`{Lk$MMi&P<XjC}bxf|fA&gf8wkyW?X2^G|A)-{|8iO6FLM2{M6V<#qWyT-F13=d#*lwjz%xdJrY(@TCM$yp>uYb4%G!(9as*P}+a{-f~&7G7c<26OIp0j(!$y&ck1hPfmeeP_S&2%OB*He)9CiPsb;(PM*Fv6T<Y?$IfJTci7n<w}+ix*d6YT`#T4w(~NA%aG2?F(YR9b^)o1YHat5%`uW96==ACFX;!}7>C=qLd%nn@FB4d3`se2`y{x`vL!|r$XL4ISmC?&imm$`A3}@X)S!y_0mg}jK2mUBbE*3|e_2th_qS;~<ac#a*mlQc+^63yZ#HTbCk$vgRivKDwN)3o!Ldx!+U`@GXt5d9~kw}>SpXBW>Fl37ZH`qb&2Ib}lLGLZB#hZ+8<H6tEAX73Un7~u=8aTkHW-V|6RB71*S~YQk24sBkR!Q5+=6L$*@a*Z~v*F`oK)*BE7g(F?3o6?wo8fRQrnp_T+JHm&)$3=ccIBm<y`$FA;mJ=gVV}J^JbHTet5I`=z>)VWN9~j2=P%EWg+++nbwcnH5eYowes=ic`0ViH*Wuxd!)L#qK0P(ZFJL?jIgnlRfGzvt>Hj!%3}U3Lfa26zIz5P<*}Y}^m8yQZlq=qj26Rf<E;B^;dXu`MLEXb_?^)R7O7+Uql8m&L<;l?!e!YB0=@OK1J<zfvSdS6*h&Cylet6yi)gx(>v1cVHW-F1}6H3j(6~?JxS-_})C*tAv_lJ)NK=hbF$;h<G3L0+1V;sS7wT8<;Wm_GD<zP%f+u2~6E&Js>PA@C|iN-1V5dM9q+We4U1QBu|S4S<bY<F?^)HZ2Ej}IL<&aKz;DU)-uJU>L9c+kAlPw6U}r81ikOb{MSr(trIgd{>I3*vUYkLId-^-fDq{^RT93Hq5as){k{Hw6vCz&Lee`+6|SQZzwJ8C0{g*tkGK8EqITAFkP(Vai^afgo5THt?v4Sb02L(Q|kG<Bv-WSuv)=j;cod$&g6AyOw$Q@w`1IMR(Sv<M6K$J097DL-uIM$hl{x+TyarNBD%@&f$?->ssHu9u&y449HMGO9l~$;{gMOsJp+Oy<(9;w9snVODztFr;JFIDfBHA!h(lFmmH80p+YwGMjh4LEUdaKeSli@kurcaVg10`2UZcx(St!WjET^`W1Hp0%i*h&<CEh*zJ7Z8^bGmtd-@3BEWY;!*q(;Ts+6gxKCzj-tX0O!Y}{bR9A-^fe9h*~YK)~ab&K1%QxGUzP+_|a&2q<G4-8nD?Cx7qL1nCzygmIY?rsAWS+i}X^YWgJ*ia+(<io(q*|Pok<}-g6aQ0bCZKlWSIh@^mp2jF>oQA<Ts|lq7B+WN}o542{McsT}qU;fFy1^`_P^T$I21-y=IuGFqx+68!t~^`4y^F?ZzU(UlYgZ{HeS6n^-dGjIj#go$&orLnY}V<vk(_b!g>v4#_!4FB{3`=PJfAN9vgQzn!)KkO!Tjbk%Hcu)YZ`>>kGN(OHw^<T;qc|jh$YIbMyHj(8a|p;YE@j^k8e$WS$smps6K@+UkBesmoEi>bMrY69D#4AtS=D8v{6Q~67&+Ms~DFF5oxks;VQky?GWMep!hop=P3%s%PmL2=5P@EQY)dcLjFZKU92(eFTo-RIdQo89Noc&$dM`VwNO~DF+ep0Xy>iRYLUb#tjB#|>ZXf$74rRz5>l=o41g%50cWDf9USO5G+SmRb_zB|!C6^qRdQfrEX;FBa_>ieFns~J$`R=B<za6Cj8GJVMk?FWUY?Vi&+l*kHf7XnL{W)vJ|}#y7<BCB-(eA<d^lZ)k|xg<fUbA4Wr|pjQY)o1_WpGeK%$7dj3^Mi{<gY{J>(FoskDsVrs1mW!4kq9d_I=@X1-Vhg{B1)H<5Im@bd>(gS3%Q{Crw&Wx7Sg%*(cwS`*_&>|2s2>Kv5|m0SyyOP0JvxkbE2l5a;s%ms#yao$U6dKg)>uHuRrtz_MaEm%L3;CKh5CZ5v8c1q}RdN8mX(NfCYO14Lld9V=?>zUGLnsNgr4v$aD=g4}kH(XUM0IyvmkQzC_0X6bB22XdCP~#N2G$pr~1Q%5*uhsP0&d5TZak+zMW^sjF12*bh8vxVe9->Syrc|i{CwW4S1G^kyIFzKU9Te_VS`K<AU>fWz>e6@~+;h}(+3Qn0VVzE@D4sRijK|Fp?rZ4z2g6hpFa&1WjOXZ&>8?``FiryCOp38$s;(6)vHUV8cvhgo0<Gl*j3MYYm+SNr3m2`KLQZnm&W8CWAP0E8xu-$u?SWm?%v+jOu)){V3OnAD$R6G$rZ;Pr@I`In%C$yXc8Gg=4C`@~&8vA4t@L+x%7|O6Juz`sJ{NC`WQHS~8U*OnoLg8O@E~VPXcPM(r_pPlMfJ7k`O9||6QXv(YuDmsIS7^9Lc>Si1Ljf1+QSOki;e6Gg6x>DhZA#7(Yp$a3)53osFZ}FuzNLF$5{LY6d`KhTNI6zGrICGqcRAkft+>E9pvMzi3-I%ciXW0rr_Zfl`%R?cv>?u_d3f64pQl2%|*UB<puwE8z`$S+!oA?%YBATYzpRUGoW+cfsq;LpE|TTl9$yPt-9lQs(W{%s=QE!A~S$+-EYNI+VTQbL-nj_utoZids1bEmNlmKnwN4kg!v4{sZyO->8EhUgsVmJJ$#uN?8T#?7Th0NU9cmX9Zo>amT@$$RNWn_E{Fvk-*dA4`JJ=GXX7jOO7ST84oOSVkc4tqJ$cmWGqqq5#WcD$4s&+CW<B_VWDCp~Yix>C@&mI2NI<cBka<ze;J8KBcGb7yB4Rh6ESWFJv{T%+RbTY-lY_=mlR+^sLRH9*D_7a<u2eFD1FWL0B^pB5Gg0nz;Sy~-hwNs$P9ia{#d@_|uef?%m}mT+=O$~;&1Q62o5YNg&R%gU1!kGXY_bhX6-udhP`X7%NlUc)#%dMkrXCM;r%;;KIa9}JZw^bX;2x1Rs`~vR#@^^-`|ZKD;}WZy-AHnIPfZpnqVkrwkKbm|;eMom&Ur!$qu|@ziuk1%v;64zRCF;CS03}4A#7UhOV6F427u!`t$~LR18v+s?y&?cto$(f0C50CB(2k?Az0$?^lQ!S$0={JW$!cUiK5LZhegt_G}_obwXRD5=-MeIvaiFmFOWU3;KaBJ9EZZ<*BPBPaL@fz<8`h*>G{-MPkm+O7=h1z`;Kd+2sMGhc=Q1|C6cmyG%i#87E}n+@UadsZsRJ9kr`500*`2o!N)7+`$kOC7UfiLA*@qT`{>hX8maAhM&TqbCM%{ilTU3FMANZfE@xf9(T7qJBxuC+KGL9kU@f-;a0g!WBn;(DRaPN$R@6+$yz*%~C4(w}s)&#%c0W#ly4Y2F2)4?_w&756({iCx!(iOhz|}haf2fMO)QU1U&Ls<kg$`sH(Gj<?IF+tnf{As(*Mm~}!eHQhnauk(X)*N5JS9f3c}fiOr|D8mwa#7_lj-W7=_0QYIcrFVbh;OruT*ZHi?$fF#uXKJ2K-hvUoCR4bOaCc{_DCA%Wf2L0;v)*$z+zt*f^mGdIRZXBaR0rvll4y%cgdlTdGb;WLwrvRf!(Z*=ge%?ezBQea$viTLmJKY6o0j!4rksItlfK5K(Z8og*<UMJlfCV!b#eXUHkb-A;L%Zn0}nd~%i$tTJofPH%^Np}VcN=3=RA94&tf%hn=Y&Ek~ptg&%gxlSsv1pI(HoF-8?{`5zyx}CYj=#=keHEE0PHRMUyVho^MvKGDNa*0|@l9lE8$e&4meir}m3v#otn;M1;5#wS{v^$WD{86DXiexQ&Jqj1y-qAdqE$mQy(nZz|<|@iFcxb2MJsk?F54@Hjqv*J-D#^c1wx#GwwCO#{xSjBMW&EFcHSt|Zj72Qi;PysmyLP9u$D-kP>pe#X9RXRkl$;Azd<lIX(nOz8d$DEdoYE!cKYYlgR81C*fI>znID`_scwRYQk1wK?848Bi3`e6iMclOR;6)`&-)b6?*$eACXgp*i)>1g*U!`amkCZt&L<PX$T}E@JNd1l#R6@s+6D2KBw}qi4YF*er`(J;Bn#;u!lU`$vV^2t9=HbuJ6|plZa5+sv3u|mpDdps43^uRb)VE?zs%Q63b~a=4D@-6IHjX?S&3&RChIY$FI1`~*vE+3I<6|pe3ML?%$w_#$+o1?Y-F~m$-T6Amq24++p!(34>_>?+>CRttoBZ8k6`zIYS~P?&%VMNw6}66YZ$z+$7746GIV;Oi#sO>)WRgI=r74ifvP*fyFKvO5bEtB~LL*$6dGsE-^XRE9kZwg(H$YS(Yf!0t7wYnwiG-XvNb~Y6-BrG&(pcjUl;*_@w;T_#gp4#cZ!+IZV&mnp(>wXJv7YCyd{4R75JmZ)RKmEmNi`=)n}+8MZbJ$P;-g}jUiBY%t2W$Q6$XD7ThQG9ce*$Aed*arG8u`rW9x-`lk#$uHOR3DMIi|7ZQ^J)g)8CUduZ#C3JDF1p-6?JDobs+gDDMbyTA>0+OGQ(a1a-0^gpB%Trd<$o^1AjIU^GmyAxr`23hI@_cl<Z+_pGG&3>78OLbqj9ec|q^t<C5b3sx}dy+CCCWospC8odA`UawcO)_En)HohaoYXp-^4s>!`L?%baS?Vzrj}P?W~5L~iQ!tQ9Qj(YCZkNUO}Coh)`9^66JyCP!&H&*ewQmJC;^qSqQN}rLATA4gY@?F@P~IsO|}dLcP0_?-YmJL@_}Ae<Po!#;>ud_o9x`VYjyo+Ih=J|>u!g;max}8Ra`e!-2zU>a23^ZG0i=RTcO(Fe9Wb^jz-lhJ5Mp!=}<niJv}Po9b;F?Nk~<H*%<_PBgWW>66Fg*SP(lZ;I>}*=3%jg%VtZ@jap84Q?HzDlo7fpxm9=f_x>LEaP3{r+a7dZd?IWrE-tY^44ZFt2IMSM57^Y5pw?@{N|E|Z%fhY6@MVUj;GQ-<mWJDDa3SM#h!$len9&5CUD6eXTJuGEX=IUJ4xc?6K6!aEJUc!;J1vMbR^VOli`HTl@p&Paj5_9Z!gK^tQDTm579__Jh)1;v5jbr{oRS(!%SR0^J3=r}?EIxGQENUWYE4z5x_pXU+SAO(!24D;?)Mf_P19Y4RBJxhd&|PjRjAOstc1l=p59i2V1Z}OU3O)g6q&PkhV{{7Qq>gmU!OZFbKJ~vO4SV|bl=p>Jcc#%sN}eEA`I!{LFZ;>kjzKJ+-I~$nzxJgCNGLm&EaW`lVc4Ci#2~3pqlLiBKsx~sK>4nt`7azBgKkbIczJ{N9y&Sk{n!y399R;R+D~dH+zzNPGTUa=#xh8oAc=7s$zShDW_a!8h4#t{=cDS(8?f8M{zuyE<QxbC`=8u-rbCBg$*H`PAgaV72VqtO-d!7x_d$oTf4+KzKB=GTNLiyAKvE!(8j2lpILz5L)Wd-V6}7?p6|~diM8VE8brho1ZV2HX<?s&Gr53hw@7OSb!S?RXmcpks&}w3l`I}1YtfEKs0G3cv@_n$oa@Zz_j(@V){ofklgYQ#h`pyWd80pLoj@nGo*vV;WGg{7RyZi5gk^^0S+)1-`@6t7?eEuj_w$oD+ahExNCix1iD5U&`rOF$rG{uz6_T@ny)|#yG+GUv(qtjhdUp6@J!tFtS!yGiN6E#fe03lzGrUORv6}M19EGNW&T?eyGO};qAXwjW%MY0xZ78qstIKGZ#vg|aY$o4YRpLR45j&eGs7@9QSvGj5g(I0?!+Njhl#jS)=W(aHx<oP0meXs~=@)(L7Wj@DnfEGlTYY57ICXDaV7{BM*WX7Y%Stc@2#o%l0R$HYKQ|ayVRh9k$tt#s^uso|gn1y*dtEKjaC`ZAuFQ{umJP3a_kwSfnt1dpbhNO%dN@1lT~{rsu)z3$qpjT6eAsXx&4<j20~=nDVI?E#x(ScmQs^<Mq`gIvnoev7gW3BfOIT0>WesaES0d(uP;e$@Y~ErX-iTApu)TV*kTxYe-h+W``Gg1XOQQ{d^Yh_(#O)!6@q9?E){q<=HM)}}+8APNeY;{-#qt)?7BTxVq_`EZ`V~{Ouj>=6y=?9Zk<mP}LGff)cbbU1t&T!qVBkJ9<&iexlrf>(`2ijL)Wj6V-e$~lgfu2}(}b1KhNcLSJexq9vW=r<^hY*rb;U<1ak}JFNw2KU8nn!ZUWorv)I8LNH=i$7h|Nhf4cGJ0rAt?}F(Q{<;e0w{V5-D{WU*Lj*r%XD220C2%QFVcE{lh-U|CWqpK(>Um;1VuwNXF^xgoX`_dAlXm5$`eG#1JO0YRB8%tWJUm|R562<Ds_2uLLcnz*cD_&Vew!}JbN=^vjas(IG*$!7MO4{dbRd-M!ZFI0f36h_coOefZ6dOSwHymZY)oO~2f2C!4by_+z@!~rsn3%I{r3XQrmR<iOP`>dj6M|m{X4LOEDL`TzbwscWHp|N~uT6vgw=abzP?m%DoS}md-v&nW_K@WmmwdnPMbJYv1SO5)L&0TTGBgZrZPE$QCXoH4B7EhhC#sE1uslp*971NJ;;$bN|Cb}|wyIt|b`)K=4PZAcsaYoh|1l!>f#{Hgnvi0h6+hOm<gBAXQ;UF=DDqP61+4arm$W`1IuXr(;#G@F)6=GP{V6|AM6sa%?monIDtz_((Hbp6{<;IqQIlpz35n@DBHBrkCDzkmUhZ&3j58_FD^Vy0FUse07Zhvd2LmB^zi-;5lvoaCYp)3ltz8QChn@lCwCof;P`A5z6NFR+QUe-fOv5%}v!bU!6w2mJ|%T)k~t3>Zem82vpB@?RMEJ%lmRZ0{#g~7TmnRtvR#>&q-CJSxhpfO`F3Zo*CZH(n2Yq7s(b7!=_(`t7p3E6{j^!~woJ)IUM8q2ys;a98P!Zc&uKDwfqslP;4ue#WuaCl*!u`E8y1?(!y+)<SGOK26<3}4awy`AtY`<bf0fM#9}2EV_84^*D5E`MB4v~*$Vr}=0wcnPQKGgy0>Im{AF{?1YdVra`JOy7rJ*#^RIOwIUoEE{*E9#dZ!FJFEDAj#fkaxyJJ*pW!&Wjx>^FU`lq>D7(w$5Q1L<5|Ln{|7++X*9lw>f$+@jPhK5ea6y4vx*{@?<<xhnzt>U{(?f{R1EHPg_=tk9D&h%%0X&AJ{?8#Fo_quWouK{z41Ri>QK~)90*@yo<x9$ojg7I`S|&vBjcH)?Bk~=$46%`PktR9zxajeP@wGgPlwMiw(`+XDYvaP@c8)2;p=B-L#v^z5u_`tRP49(0T)<q)3CXW-)LqK2>kv2gjIV4Es1AYOp)8@mbr%Pn0Z%I^Qu2HO)KFE8x?<RUGvK_Y1m%bb}2`qNtWGIs>>W|J!7mK242-&3Kx!9ICQ8OQ5X#L-t|hxDjH6pr6^fKgDX{q$NkR-8mRa?M>6swEeD{^uGn}k#=y44Fy;GV;Myr!gk1v=+y-a52**P@499U&f&YQnE)onVh4j%5naf*w2G#7a7|g179#7!R9t6+%w<CGj!zFVFCw|sVJ=hlIn+az6Qlmh`6WLf(UnR76a_`hUD?MZ6T1ehgYM&cDxXjmuGGO58xb6*s>zPAAFk8vfx^j!jCan;add4w(%EGLRy?8pVC{4-uvffoNc7!u%QZB@$aRs%*!JQl7k8h-*2=UDKQ$9}dN;XO<Eka$rZ8AlLgR5fI$n;r!5Y8jlHyA}7=NgWDZ8EJ!Cpyb&45@C;<vdI=DH$bOXR2AAZQSE6K{#Wt*N;*$XAWDH_~l36_Q6F9LE!|A7RmQv5f^Wb$hS4@X`U$^S)-72R?O3+viZ}r$h3Q|RxxHOt@bPt^q$0-dT?)Lj@=HaEKe(FlO?YE#<&7+a|UDZ`}F}MQ}}J$a$u0OqF#cmsrx$H1a`DPZHQ_m4MR%QvyvuSf#6hfY=!PJIe~J7+u{;@)Y9zD%|cA(@7rOoTamAuVG38t6>6SMr2yaa#?uTr{6Y|GEC%w`HSZ=*7)m9g%RM8*-aQ_CEHC(}4%LDfsCwbNW(}wpv}=^k9j|M_)aJ=i+8DO0PF^P>Zc)~(i`SOz^Wu3eN_0?{%Q#!?rIWStnl?gu$kx5Fva<Hos`)@SFjMKrq{9yA8<m}*zpUU<TF@<nYc}p$7z=i|NQS7~uUuW3ys$h66yq4)%}~D60zC5W1_PZhzb8v$c!3WIoMg}|T4hzP$WqI4xm*Gj8hzuCbZ$>VI77z4YC*hz@DrD92;mDwqzZn&9t6Kr%hyi9rWj6h%#yBJ3d^d4f>d(>uR;WufXinT(=};Yn5oavzA&1<9}M1y$#9WYoOg1Ur66xzI)%yx4-9nV{bEGtb&_Db5z!_8O$%fNrbW#R@?VsgfE8?tXAT866JE~8#OtGOhZk-Bc@fVmUO_?C&IBP_K{=x}Gy@8<@Ig_G$!ilD(Z^`CUWMn=$N~tw$_GeRSA|J4eHSk)?f;Un;pWjAQu#-gE4N_5$_yS=jPnFoOm)m8(G>kETaXFe2283MF9ri4@Y1Zg!$ddh^tCp_bXOOqh4Zy;F`wNjd8}sJsla>7&TiM4j74=QkPoj-D}D7MslPiGN2!{?zOD;V(~3(>S~Vq%c)qzsh|(*-IE$=ab8=Pt*7`2<RTrf^W_U#iP!}pT3S!~dAC&R>n~EF|TymJv2VH>J(ZQO4vxAz<PKj!6Kx8%+j+T5d9QQUEP0SF0sfy-oK)jjTjmW}qO1APSuWxah!q?w^kH^=)ZC9Pill{9xZi+hF1`GncOP91s48n2di_tJPS}0<I->Kni)TgbpaRe37Gq3>s+jZ5_@YQo5rSD76%DHl=%aNL*CW@AS9t9VGV`yf-x(w&RU!!EPy}<$slE9InXbq(hH^UzHtTS@L%;pQ)%|t$2T_||1!4JM3G*Xmxve|yn<SDMMOF+^hbSf-28;)SlgG_*}0J_AXo6%IJ3FHY#h=vvrRz{zc%LGrGaIZ4{G<P&t3phQmKDq^2WxdrYgo%s{#1-Afa=W7K6eLY|dodZ(_d8R6To-DN+~d&&w|qFNm~!CO70SVs&@7eEDwWeH5mGrv%*`HA3}<FA&{RG7iV}3)r)#z<rkzEwBT1&Nqr}yfqwpyClx|RbvX0EMw~*Tl`sp}fu(yEs#(a#@9GaaU;lKiuZ}o4jfXJDMx34>!jSEY^#!Q_3+}SMrJ1GYzb1FRwvQTF0c5g6ve)uod;6c#-YABmC7+{DN>s1<$BTCGZu|`Qazlh3$3U0|h>0Lfe%SC#>qlDRl>y=SEj~*7Rm|I4?!3pKjqX<IFj6jBXVr6=D@zmjP%Y4a<^8BHrIQr5JQ!;HCl0}qy_R-5FnYGw2V`dh^G-|o0mZIrrPP}Zw^3Ex}lhtNz&t~2EH{t!E%_SPzWFz2~H`)+p(ClnI8`6v}!}E9r|4-u;aJB2ccH9*#M|SS%Qr5ba6K!D0m+Nkw&cGomcIE0dts07r5L7e?9-3+J2(i^7b(D%KHHb*!g%$`URk8JT3SxOWP)gtpSt(zld4ZT{=PiuMxVn?@k#%9=uxSq|P#BvL)@j1lQmIyv-PMDvVHN*fS#uT5wP9BVT=Vl+XTLh?D2`8Bb#KNa<m0nSGOtZm+0OO!jjIz8xHx;KT09hFezH#&>l)+^c(@&Aw;VAPJY2>iKE&81T`!Y2imnh8sZa?hgui8SIBll>6tG`yn&Sl=d5GmQ$DM8oQEK7`9^IzmuKbQXxRu$`W6zQ7409=yI10Dq%pbY534v}Yp(DOYvE^no2sguiM+zOY0d`fWmnq5b38l*7C81P7=FRO<_2Uk-YQ*XAeT)MCN_8VHt^@*Zi_QW8-Y>m==&L7>dqyMQ3FB8JTQUZM>4-4;=8fBW*0lvQ>TACvj2svyw-}iXLkRpeGOJv_H855U%K#)N(iC+jRTop`lz-Ey8V1C!smcTCKYOa|Fv^^%DD}JR@BvzyWm9voXfACP5oa~*-a{kdPDZ6MJLXrW&2zD?svCt(oh^H;+Yo3}vSkq+%cMs8%Jcl<!xm;_8!LDMBv-syVOffXq73oA<nZDAlbIa;8wZ%Rka9*lUkmCM$Z<=ANkq6MJgdVAqrXwC$I(k~vmi!R2zSiIYd2vtEDAHCrY5*BsYvy}dnDiu5DoQI;y@2In@i(xGzzBV1Q*Yx?+}-w4M#jCff_I+<4o@U92HB+W)*!LMbS7_nQtN969X%Mj-owYj<pUy8I)M)1Gh7XGo2}1_@xsLqv6^G&z*mqNd&Y@pn-M9Z@Ln(Tin3uzv3I4a1~O9CPfwgk*T-G%{Yd<BUVGbocD~u8_0`=CwxzajgGc_c8D$+Mgi=7ESRU5$-a1s<rqZfNKI$~z3=Z#D5lGZyy_dyGi9Qja=kbuMxqnCTtJ)co)Dt3JB0cOuL|mKfiUceh+QTOqQ5^I1g3l2BTj-$mlM$e42$gqEtc6<O_dCt<%ohv`~mHu>7BGzBa0!?v>LHadKFL7!GMNM<QpMfxrfbEGtUZE*5zH{w$34Y11@Q>Omu`@i<AJ_9<ld?ORO9LH~F&%;I!y*8S+pI&1{a<x?!XD&p3hqrQ-yeK?3E%Yj#QAh=JZ+RX%`V2_8h44%|(cK?w<ALQhunFylAt{#(;RTVUvAm?DK}eH@)nI>Y@@)E)M_ozAec)7iyTGzVF(Kx~erSf7mO(A37iJyi5p-(eY*8RFgGM-#1gmm+;n>mxd04_oaPh1$Sx=Q{>oVl7GCl^}{WHNs%c4ot}TFA=B0jCm)_ZgB~@i}o;_k6FwrJMMxo%z0=VOdmJ{;t6MNY^)w^3(wVp(uP+62K#pU+c}z<M@lnJV||QAQP6*8E??C%-(*qz`UOqs;R1cF^ejtbbD>tSEVw!tPH72SXSkTK(1z~0S&o=!B#1;2RR}}|ICe^BAHF2Q<EMR~EsmX3S%r<;9f#5KyhaR}jhQQ3zxI~vLyetOGvfOhGw`8>UQZAZcBHudZ<N(58M{FVK9d{1K`Y0DHSF}xqjtaBY_@kty-vF=LOEc}hLSxNM0C*0QnKrMmx4NANQXui(qW+JjO&^Xo1!`hopZ2^m(etyM`kz(z8H-$a;ftT8*6R_?#K{71C6A>5ZU`xJj198Jg$*U%h|&`wl(&_;@e9;hs6Z4qvuD=b~-P~Kts9UTPfKT9XcX3(te7sL>Yg1bca_bFCV|AxP`-$<0mJ_r#}zRo*w<-)UX=5Rof07K8x>^mk%Th4@p-M-Kyf1Z1GQkpo<T#oCAtJo{u#i*X?53sAW;#v{;&g-F}<$Nzg}Ls9jn;K(m#jW6UmbjPC&WvbPfkpJ-f3A~gA^x^pSOO0f4$8dZ~n(~dwT{vH-Cv_I|N!x9Ur4^vCAmqnRD(DmAwlcmN!ia<wHSP35HDZy77i)4}wPuY^8j5iNcoF%<uQHpD~Z^|F>`0o?>SntIHNFhRD#JbnntG6+62m36wuv&zr$(nW<oURzxNeKANQEAy!%E!{H<yYa@og3~mu(d@iwzX3&<R3B(Q}h6z&EN#7;3hv}-+9UufhfW6>L{*V@E1XyWZFX3A94gj!4$kv$cSi~CmI@7-&P#}yo*T)gnhldNWyWHOMdJ2u0oZ07RA8cK=$Tw)=>9J&gtj`rH4*yz-5@dyH9zv`R6}IVD-(Z&_8axbyvjo4R+D(0~XdwRx|@UyAmkoZYCBdvtop}Orud+=}{n7#v1naC~Fz}xC5@j%2%Fgnfz=;rijWQlFL@)LphhLD1dV*o}%2<hf976E<RZ)e(f1Wt^NJoezQ5=@3qI>sN}UrX{X@Qqm*P<o;|$s;OcJ$^4iiKNF^9ruw<rAB9aVVRiv_ED=3N>cT9WprzG}8j}y}QQ_|9?!Q|ldRU^Wgf!IU{_Y>ev(-<Y*A^@G4_JYTMvqHIPxqlq()y+#a%Zict>;uS#Kfio_JUspN^z8Wg@af}$oPf`k5oI#2CSE-|d~tk6O$gu=*fH9@Gl>r^E}S1Nj8nK@?};RIS%*(wJSFVseg5Ia%RfCke*Dw%An;hX90YAjHzwb1l{8`ruHPQ8q-SNLugRY;6Ic}b2k*O>D@{5KtA~gn%Fj`e^E^ghBKm#A^f)|88D44W@so8lJwYK=1nbKly+WBbPI$#1{3%SL%LQPPXiB-NA4PL=J8bZHe;FG=Pfjo6<)<(~K7?ZDlCiOCxY>dL*C>;^^_@xG%&BE>Jd?}{IHs%ko|+TSUYQ_Jy7U1<bp$iPqPWy$0K$iW9Sl$fPFA!H?Kpi6CH9@Z+f_J|j&C>vATZ@&wg3!#C=zDyGv1@(4=<ktKI7dIfk)ccmGS0-fSuW!>f<mX&>1Dv6gI}Il*5fbwvo%8HbXBmP96~Skt8wiy+)=Z%Rr-sKz2YzeQf|neWk%geW8Fw>&XQbwSW#3^-Ts7@$_0o?A8h1>Dl4QSt%g2^MTOz1EK8$LfZ|5b`cQTcL)U1nIZqp{Oj!-4$~R+HL#Gs=R1XHnbF!96EDdVdUG<10SVB0UPY67$@&)HbDL3VoNPi{oZRXe6_k~g<;$%qsids=!9h126Srt+Hgy{la+^|Ut!zU@mfXsM7|WWePMsBT@3o3k=u<3jbSyNP2d(Ov(MI*Iaf@70^yH;qf@7tcE5@}_&E(@;sU~u8uN+`D!ogBa7U5#4M!Y!bsO`bc)dClmO970CXbpF-z=RpeQb_mi%C*3fjv?w^VVhh-K|{@aG*DHq*Gktj(b6?Lzk`iJ5(f&a<8-JH7xxEnmph429PmC;G){0Pp@(~nvB^JNMhV?7(P9{VKF{F>Hy=e~@=<5HC%Q%K?bdhT7SV5WIjb@Thlyh#)}YXQu%4CmnY$_08y@e1w~kg!YdA+M#?|Jn5`x+7m6)}lUKrKh3;YgQQX!|`q0O%W{!%hB3i@U+D(3G^Oi3Fqb4}!p;1c-j@yYPXA=A^qEz&N_Q$ZZ2ND7WWE~7E&__vLs*T(NMW~AP6(be|o<+GPBel0PmS4ljJxcTX}qp&n09XM|7j`Gs5bl|w6JIZ_fWJ#*QjA47NRz29ke>$C3HKz$-UYLnaSOa~k@2cqtvMqcQCHL#7W;@q<Jhgs2gqa^5KQ37AoUN=%9XQyf)WOq=V=L#$E{s-kwj{Rl{OJp}ravG4`1B0^fA;k3SM6j`!+)9^vX*CbVa9N*8i~btn?e!KIr24^KEW()L6m8IXi^brwnj?-ZXYFkot?dUyX6(_lH+h@76ON>7=h~IQEgrzHnRLuO7R0nroJqcuQ@!PUQtbVD#8OR6ryQ58aVdQ{{DVF==b~eU~g|vRa!V3ER`Ep50@#AYo#6IjS($0JK!Zp%;k@zRUNs|kO~mqs$UOcs(aj8k$gyzexR(FdlePRxTy#iaPN*=-QM1Avl;ET`_VWoE#PL26iT;QG2&ZV^?p#Jf8nQ=RwURW21x|#T1<%lnXv$kKzkaqXofLNsXYn{gtvnx)k4dltm=Lcj>n9Bx|lA2gUT#;*kG7+ik<2|9pZU&c6fGdrrFp(K%=3oS=h7adJ?4p=7kcLL(<V0#qX*@IZM*hRI_dP`)jujScd9Nc~6qOG8XKYIE@)K<us>Hc1C?}W<;D0>V{4Uhz^C4a$q4EHWl}Yp!k%cDz;aJu9SUScjp!yNU1H2PHw1*FDa{4fyZGNdM>%4T->9`&O2HxV@!@_=4rK$%FH*@C~BXF-lA}%@CcKq5jPt<!@PrN{X?z5nrNGRyco&6E6o{P1I#C-k}K_nQU83u<xH+*wx?uLq733&wQptk;hWE-Yl{D}j?i3G2JJ<A@{1@)VV4L4-EbX@Z$8h$gnYHe(Q+LFrTER0N#?<C&EQ!GM{h#z*}?dJG=Co_i}{>#lR-sS9-H5SWt7Y?!ZGxH^O*w_s4E<qFn|?YrN)aig+18B;XaGx=+M>#5oH0@u=XgP{{^5py2SlT5gP|sqY?b3CI<N}7|4B%(g0|PTL`DY0hLx#K44)wFxmX+0#^?x`VEqhh!!mkD5@;9Iif<!t&laS1uMWN&u{(%O$z6$6fG5R{tteR*U8OijP?~xCIHlca{JwUW)6<LA=9rG+X|EOgt|@OWd!>u5^Z7=uzcs!#R!Hk+HfWjz=rr8ryg+AhdS<2ZWx43WrOBS<oY9DkyH$xRzbDA1Y|<DVBl(I^gcq-7KZ#G0z7A_>c+6#^p>F)ync_#;8<}oO2b=mxVrg#fk>MoS~6tC(Gbo}pnG`57hoKJ<n@+evQ^_f;&gysuIKRW=5H5>pljG~t7y$I>)UUGrx{+*l-W{<Nyh41npsNc8m`$KS>km%VgcL2_t6M5_Q5cL4kQeI`1`lvZ_wA%DX$;RljtIb$=!Sw@VI~T8R#IP5w25&HRrem@Ss9Bq%^qs-?%A~2#^$@3d0KlQT~SDJ16I41pN5^damR^+p4{u2lK`HeFU(R3mGid1P*#J1%N2{1-la1PS#cU=fxUR6s9;rpip!oZq-gOC_;Kao(Ag`99JpAmVt~ltRNqL{qxPg14hTRs%-gS=@2vw-^W0WvXNm-RX{?7E(4X=$|SPPj4Xev2fuM4r1B7f_8STl)n^R-*|pDpe##tD#ZOg!ce;$Cv3Mb~Y*NldS({7I_|t4(DBCXsE+MgZhRopN^_)@%GLAs@^Ax3p@MJm-ld}ZSY%1sVaS6;zG(G~rF0#QQesk%pGFd%Kqj>rRBe))MyQkj-))0kFRxG_FZh)J=CG3a5r)*yVI*x$U!u+zmMF<SfftCQ#+^S(YWMSZ3fkqg%A|{DF_zSza;bU~B3!#q-fNaFKp|bO#;|X$~^TjHh(SxbEOL=_I2xG7@=l#E|VauT09NL3zH$e)ELBNCXJVjY7dl!s8ji!vAWki_2VYD?J#jGD%OgM@~qk<G>T}NzScmV{ONd)TVGwyCEGg@5I2v(`Y8D+{DfuPylSI;c~FyG&Gs3PO=;wd`1#6uhUWzddh%OnE4jIz8Fu-Vz>0~#WTlSnU`BE2EvGm4jz=KOmwidMLOS!)BB)*3@9B?&vUu_BCgj1+xD)v)_)J~ZDMniXNVcD|mM6zrMu2cUU2PHjDx)Ng(tPkC(0BppwuaAK{oE@x4IW0=DD{v53XP7t9eq9UVMj4(F0{y*cHS7`"""


class MigrationError(RuntimeError):
    """A controlled migration failure."""


def run(
    command: tuple[str, ...] | list[str],
    *,
    cwd: Path,
    input_bytes: bytes | None = None,
    env: dict[str, str] | None = None,
    check: bool = True,
    capture: bool = False,
) -> subprocess.CompletedProcess[bytes]:
    printable = " ".join(command)
    if not capture:
        print(f"+ {printable}")
    completed = subprocess.run(
        command,
        cwd=cwd,
        input=input_bytes,
        env=env,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.PIPE if capture else None,
        check=False,
    )
    if check and completed.returncode != 0:
        details = ""
        if capture:
            details = (
                completed.stdout.decode(errors="replace")
                + completed.stderr.decode(errors="replace")
            ).strip()
        suffix = f"\n{details}" if details else ""
        raise MigrationError(
            f"Commande échouée ({completed.returncode}) : {printable}{suffix}"
        )
    return completed


def output(command: tuple[str, ...] | list[str], *, cwd: Path) -> str:
    return run(command, cwd=cwd, capture=True).stdout.decode().strip()


def decode_patch() -> bytes:
    try:
        patch = zlib.decompress(base64.b85decode(PATCH_B85.encode("ascii")))
    except (ValueError, zlib.error) as exc:
        raise MigrationError(f"Patch embarqué illisible : {exc}") from exc
    digest = hashlib.sha256(patch).hexdigest()
    if digest != PATCH_SHA256:
        raise MigrationError(
            "L'empreinte du patch embarqué est invalide "
            f"({digest}, attendu {PATCH_SHA256})."
        )
    return patch


def ensure_command(name: str) -> None:
    if shutil.which(name) is None:
        raise MigrationError(f"Commande requise introuvable : {name}")


def resolve_root(value: str) -> Path:
    root = Path(value).expanduser().resolve()
    if not (root / ".git").exists():
        probe = run(
            ("git", "rev-parse", "--show-toplevel"),
            cwd=root,
            capture=True,
            check=False,
        )
        if probe.returncode != 0:
            raise MigrationError(f"{root} n'est pas dans un dépôt Git.")
        root = Path(probe.stdout.decode().strip()).resolve()
    if not (root / "Cargo.toml").is_file():
        raise MigrationError(f"Cargo.toml absent de la racine {root}.")
    return root


def head_sha(root: Path) -> str:
    return output(("git", "rev-parse", "HEAD"), cwd=root)


def worktree_blob(root: Path, relative: str) -> str | None:
    path = root / relative
    if not path.is_file():
        return None
    return output(("git", "hash-object", "--", relative), cwd=root)


def index_blob(root: Path, relative: str) -> str | None:
    result = run(
        ("git", "rev-parse", "--verify", f":{relative}"),
        cwd=root,
        capture=True,
        check=False,
    )
    if result.returncode != 0:
        return None
    return result.stdout.decode().strip()


def verify_baseline(root: Path, *, force: bool) -> None:
    current = head_sha(root)
    problems: list[str] = []
    if current != BASELINE_SHA:
        problems.append(f"HEAD={current}, attendu {BASELINE_SHA}")

    for relative, expected in BASELINE_BLOBS.items():
        actual_index = index_blob(root, relative)
        actual_worktree = worktree_blob(root, relative)
        if actual_index != expected:
            problems.append(
                f"blob index {relative}={actual_index or '<absent>'}, attendu {expected}"
            )
        if actual_worktree != expected:
            problems.append(
                "blob de travail "
                f"{relative}={actual_worktree or '<absent>'}, attendu {expected}"
            )

    for relative in CREATED_PATHS:
        if index_blob(root, relative) is not None or (root / relative).exists():
            problems.append(f"le nouveau chemin existe déjà : {relative}")

    if not problems:
        return
    details = "\n  - ".join(problems)
    if force:
        print(
            "AVERTISSEMENT --force : garde de baseline ignorée :\n"
            f"  - {details}",
            file=sys.stderr,
        )
        return
    raise MigrationError(
        "Baseline incompatible. Aucun fichier n'a été modifié.\n"
        f"  - {details}\n"
        "Utilisez --force uniquement après avoir vérifié manuellement les écarts."
    )


def patch_check(root: Path, patch: bytes, *, reverse: bool = False) -> bool:
    command = ["git", "apply", "--check"]
    if reverse:
        command.append("--reverse")
    command.append("-")
    result = run(
        command,
        cwd=root,
        input_bytes=patch,
        capture=True,
        check=False,
    )
    return result.returncode == 0


def changed_paths(root: Path) -> frozenset[str]:
    raw = output(("git", "diff", "--name-only", "HEAD", "--"), cwd=root)
    return frozenset(line for line in raw.splitlines() if line)


def validate_expected_diff(root: Path) -> None:
    actual = changed_paths(root)
    if actual == EXPECTED_PATHS:
        return
    missing = sorted(EXPECTED_PATHS - actual)
    extra = sorted(actual - EXPECTED_PATHS)
    messages = []
    if missing:
        messages.append("fichiers attendus absents : " + ", ".join(missing))
    if extra:
        messages.append("fichiers inattendus modifiés : " + ", ".join(extra))
    raise MigrationError("Périmètre du patch invalide : " + "; ".join(messages))


def validated_patch(
    root: Path,
    embedded_patch: bytes,
    *,
    skip_checks: bool,
) -> bytes:
    with tempfile.TemporaryDirectory(
        prefix="galactic-mvp016b-", dir=root.parent
    ) as temporary:
        worktree = Path(temporary) / "worktree"
        added = False
        try:
            run(
                ("git", "worktree", "add", "--detach", str(worktree), head_sha(root)),
                cwd=root,
            )
            added = True
            if not patch_check(worktree, embedded_patch):
                raise MigrationError(
                    "Le patch MVP-016-B ne s'applique pas proprement dans le worktree."
                )
            run(("git", "apply", "--binary", "-"), cwd=worktree, input_bytes=embedded_patch)

            if skip_checks:
                print(
                    "AVERTISSEMENT : contrôles Cargo ignorés à la demande. "
                    "Cette option est déconseillée.",
                    file=sys.stderr,
                )
            else:
                validation_env = os.environ.copy()
                validation_env.setdefault(
                    "CARGO_TARGET_DIR", str(root / "target" / "mvp016b-validation")
                )
                for command in CHECK_COMMANDS:
                    run(command, cwd=worktree, env=validation_env)

            run(("git", "diff", "--check"), cwd=worktree)
            run(("git", "add", "-N", "--", *CREATED_PATHS), cwd=worktree)
            validate_expected_diff(worktree)
            result = run(
                ("git", "diff", "--binary", "HEAD", "--"),
                cwd=worktree,
                capture=True,
            )
            candidate = result.stdout
            if not candidate:
                raise MigrationError("Le patch validé est vide.")
            return candidate
        finally:
            if added:
                run(
                    ("git", "worktree", "remove", "--force", str(worktree)),
                    cwd=root,
                    check=False,
                )


def make_backup(root: Path, patch: bytes) -> Path:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
    parent = root / "backups" / ".mvp016b-backup"
    destination = parent / stamp
    counter = 1
    while destination.exists():
        destination = parent / f"{stamp}-{counter}"
        counter += 1
    destination.mkdir(parents=True)

    backed_up: list[str] = []
    for relative in sorted(BASELINE_BLOBS):
        source = root / relative
        if not source.is_file():
            continue
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        backed_up.append(relative)

    manifest = {
        "migration": "MVP-016-B",
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "baseline_sha": BASELINE_SHA,
        "actual_head_sha": head_sha(root),
        "validated_patch_sha256": hashlib.sha256(patch).hexdigest(),
        "backed_up_paths": backed_up,
        "created_paths": list(CREATED_PATHS),
        "deleted_paths": list(DELETED_PATHS),
    }
    (destination / "manifest.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    return destination


def verify_applied_files(main: Path, validated: Path) -> None:
    failures: list[str] = []
    for relative in sorted(EXPECTED_PATHS):
        main_hash = worktree_blob(main, relative)
        validated_hash = worktree_blob(validated, relative)
        if main_hash != validated_hash:
            failures.append(
                f"{relative}: principal={main_hash or '<absent>'}, "
                f"validé={validated_hash or '<absent>'}"
            )
    if failures:
        raise MigrationError(
            "Vérification post-écriture échouée. La sauvegarde est conservée :\n  - "
            + "\n  - ".join(failures)
        )


def apply_to_main(
    root: Path,
    patch: bytes,
    *,
    force: bool,
) -> Path:
    verify_baseline(root, force=force)
    if not patch_check(root, patch):
        raise MigrationError(
            "Le patch validé ne s'applique plus au dépôt principal. "
            "Aucun fichier source n'a été modifié."
        )
    backup = make_backup(root, patch)
    # Recheck after the backup and immediately before the source write.
    verify_baseline(root, force=force)
    if not patch_check(root, patch):
        raise MigrationError(
            "Le dépôt a changé pendant la sauvegarde. Aucun fichier source "
            "n'a été modifié."
        )
    run(("git", "apply", "--binary", "-"), cwd=root, input_bytes=patch)
    return backup


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Prépare MVP-016-B : ruleset externe pour l'économie, les bâtiments, "
            "les technologies et le scénario initial."
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
        ensure_command("git")
        if not args.skip_checks:
            ensure_command("cargo")
        root = resolve_root(args.root)
        patch = decode_patch()

        if patch_check(root, patch, reverse=True):
            print("MVP-016-B est déjà appliqué ; aucune modification nécessaire.")
            return 0

        verify_baseline(root, force=args.force)
        candidate = validated_patch(root, patch, skip_checks=args.skip_checks)

        if args.dry_run:
            print(
                "Dry-run réussi : patch, périmètre et validations acceptés. "
                "Le dépôt principal n'a pas été modifié."
            )
            return 0

        # Preserve the validated state in a short-lived detached worktree so the
        # post-write comparison does not depend on the mutable main worktree.
        with tempfile.TemporaryDirectory(
            prefix="galactic-mvp016b-verify-", dir=root.parent
        ) as temporary:
            reference = Path(temporary) / "reference"
            added = False
            try:
                run(
                    ("git", "worktree", "add", "--detach", str(reference), head_sha(root)),
                    cwd=root,
                )
                added = True
                run(("git", "apply", "--binary", "-"), cwd=reference, input_bytes=candidate)
                backup = apply_to_main(root, candidate, force=args.force)
                verify_applied_files(root, reference)
            finally:
                if added:
                    run(
                        ("git", "worktree", "remove", "--force", str(reference)),
                        cwd=root,
                        check=False,
                    )

        print("MVP-016-B appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print("Versions cibles : GAME_STATE_VERSION=10, SAVE_VERSION=11, RULESET_SCHEMA_VERSION=1")
        return 0
    except (MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
