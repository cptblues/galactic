#!/usr/bin/env python3
"""Apply Galactic MVP-026 from the exact post-combat baseline.

The migration adds the Arche Pionnière, a deterministic and resumable
colonization mission, atomic payload reservations, arrival revalidation,
and persistent colony foundations without creating the playable colony yet.
Dry-runs remain cheap unless --checks is requested.
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


MIGRATION = "MVP-026"
BASELINE_SHA = '54004dcef607238f2ff91f799c9a4096551cd679'
PATCH_SHA256 = '80b475488e304683a8716616b4723f467a59e3f2503ce3b770a73cbd32f5bdc0'

MODIFIED_BLOBS = {'README.md': 'ff271adb1ea779efb6dd79aa9396fab7dc1e3027', 'assets/rulesets/default/craftables.ron': '19d2d8863356283a72fb0f5bbb5b9c2ea5bbe4a2', 'assets/rulesets/default/manifest.ron': '16a1e489283933415f2fdd6e4dca85f0022d3e4d', 'assets/rulesets/default/planetary_analysis.ron': '89d1af0b143781ab5627f68a5328ff14dff3a3e1', 'crates/galactic_client/src/lib.rs': '30457bd61b766ecbcdffbe405683fbb4d6807f6e', 'crates/galactic_persistence/src/lib.rs': 'e8758e3285a9f94fcb5b2ef26b796ac7cbb071b2', 'crates/galactic_sim/src/analysis.rs': '03d20d9932976d08fc3917de7b050530eaeeb06f', 'crates/galactic_sim/src/command.rs': 'd274c6b9a6fa2e144d0fc81e33fc35e3cb3ad73b', 'crates/galactic_sim/src/event.rs': 'ccacb63761ee36f158a0d1018ae42cde402f29fa', 'crates/galactic_sim/src/lib.rs': '67d6ab1476d945d5ff37517c7562675db62f28bb', 'crates/galactic_sim/src/mission.rs': '8757e503360e33335a1aad09534a7efbdc92c5f4', 'crates/galactic_sim/src/ruleset.rs': 'c6478ea4b31c6880de50188267d51beed5ac4a72', 'crates/galactic_sim/src/simulation.rs': '4b938b3a1dd8ae90d1e743bc9a6f386250e8f56f', 'crates/galactic_sim/src/state.rs': 'b491d4cd207befa60a99bd44764477f9dc5e5a7a', 'docs/mvp_architecture.md': '3b906b624de301e9bd7081120c6883cc55a50255', 'docs/roadmap_galactic_issues.md': '38751100e3ff4e59f58946227fd236cbcc844bf7', 'docs/ruleset.md': '3ec76ecd28c93ba909aefb4a27f080e3b24dd570'}

DEPENDENCY_BLOBS = {'tools/apply_mvp_016_b.py': '1557ff3f419abbf6a1b58b897100aa72da80bd38'}

CREATED_PATHS = ('crates/galactic_sim/src/colonization.rs',)

EXPECTED_PATHS = frozenset(MODIFIED_BLOBS) | frozenset(CREATED_PATHS)

CHECK_COMMANDS = (
    ("cargo", "fmt", "--all", "--", "--check"),
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
PATCH_B85 = """c-rl~+j8Vak|6rduYf4Cs?5%0R^FH-v$|@AVo_>Zv#PixtLA8QWGFBbNkYm5QUH<?Tcp{%oHg6WHQUF17~3b+*#5yj@=xYV*4^WRh(O?yBsD#4r)Awm5(s$QJUsmF(_l7pwze*V)bU=N9`3(+d)Qe_or~=EjUb%*zc{m5f2Ze7JDt7WY`eG9b9&wGXgJ*1+S*dT+-S90+SmX1kIq(awA~&!E&S2jao|ZBdq4Q86NJ~^Jec~<GLDw9zwpD<nLqc^)ca}WJBb%2j<;OS1NbY(7vUuEL-;C5y%7F)|GDGr&w_Bn!R8Y`O?~GgT21EgZ3Ghv7K{7O)4)p^<kW}nF6MsQnOw0BZj$u=w}tOqM?n&V&UEEW?>{fSINfk2QJ4n!mA7&h5p?OSLKvr?2GGgUOXFx2%%P*JC`kkO`O=yDFxpib!;j@02KO8EM{p89zNVS^tJs+aNg4+it2Bs0>@D@<MG(S6-+={L`YYD-{xj@IuwVnf|2Kf@T*E>oosHH;>&q{lw;xZoy8V&!@Bi{Yoc(xm<vS+;FbwX0iwV#oND}G+)+L%p(1C|N;@2N=9(;g~7kcxX<o+`(%9VE!(E8x_^WY-J>8?^I_9s3pP)rMu!QE)7fF1AJpE#f1ed;*Jz7u<sNwkV5ffL_<hN;ej#HY_^^9VO-`W*i-$Gy>tRu1g%EQ%Mb`P5&of&_LR5Cs;(3t&vJNw9smIWhF*Im;+cY5Xv7j#fB+9G@3otYT;i`;{a<{fx2MB1&m1<a83p|0N)#qV<ItKo{{PTYYFYi^3_JCZG_`3AgnAGvWp;<b>L_9X?hzEgD0z(E@Zxa47&QLPSddh$s;uYQnRhR?od<eE%CmuP}n`N35b_!Fs&^SH%1{4xmH7<D3N>t-0q+0^F#ne@!?VO(v@)ymnv#0V0ATOjHIA$19w|XmRT&Y_+aEn7!|<aF7fR4O*Q$P8U$&{&S3H?L+7!tH}gMJBhJ7+S05GKTg;x!r}v{6%76}j5=hnV8Xa*cv3u$l)m#rv|=a+v))?V{~K&PtQepOJcAj<!2-|}cO5`7oZsneOjVkNTbm{?;uS16{xbDv-fEt{n8e<U&^hVEFu04F4LP;z?M?gB-Q7;d+wS?j{;p2#YJy^<ux4YVlRJa<j?<!l|M4F;9P_`)9AJ%|16;eCw&i^c80fpn()XuhXQyjv0>|Ofcdw!)!0ER)wsIg~>mzsKExk#Q-i)0=7ofDNTJ?rqTW7PCKX(Buus;N4G-)=K2k?8XZC!X_Far#!*i^Nd*wT^L^M|mdqkeyTaM7=~r5d2}4b_^`wb$N;OAr3R&1|r_2Cy@m{EJP#0SOR{h38)5`H8}@v&*039Rw#Qd$Gs&!u&uy&wbFtHE-5iS-cbc)QfLib}0l^2gm*aU+vw!Y0sN=JDu5R)E{*x^;Ww8Wcj)me8hHO7k6Nf?*Of8wm5v%<byuPN4$_1tA)#NM7X{X$+rdmF1%mpHK$>4k_P<EELw452rrX#>@*fIu=&^-!N+hsf#1NthJE-84+nHP>Ne%(;jwm;t6=F8mM9N4c?Unj;zEDlW#^`$pDAoGc21+ok2l7F0&)Wk$%{)5=zkha+{qkC{);4@yZ{Q{i4$1i$~Js+2i@WJ&IRy-yR*^mv^UZ>w;BrD&+2x#h5cT;2fQQu0S{KdO2|*9(ZUPD@%UG^3jY+v^XW;H1h@sTWOpw&^!7=xz{Z3J`@m*SetgR~fcL9(5-s3fB;p$Ug*Nf4Ijo}}<HN;;mma|Bilh0#71B`gP<+T>vS+70G9_oJm#&hw^Cno%VVKbUsXr&UZRZe}MtJ0(GEm=m3x9vYUg7VD*ThiBU;hlt&U-jQ=6BBEz<_|6gav@Me-5Lc=l=B4KlXuzQ2sv0%N!cO?dksM{o6zL?Ax>R!?*6y8+gVhuYl)IPOiMfm+w#g1kRcKQ$ReSbGOgW++iF?v3PhGUI%ani7#S+PsAbpIs#_s4eeVydwK+fYkmT(W{}V-$QSRCwE-TBwP-tM{@kaP;-g^D&)`6cwLODV<}W@1Aa-)w&bdFi3SnO_Z$5;$rSJq;>odG2+YVBdBykxL;b;0U{LpjgoY1(O^6>Mz7XD-p^B?4Ki*=wQ*wf+`>p@4bC&ew+fr?;{#w`Xik6=%QTMWL5V2{8p2Cx*t^0-Ari^+|t^Wojm$HUXJLl^!%KRVbyb`Srb{cpZGJl#J(djD?h%m#hul{4rJUlOg{-Pvo8oEH8dG20JU3+F??w%_4I^6-}+W%J;d%CWZ!Cs!wNbm41H7=rUzutzE)^R~1{fAQI(@@MDK<>lN5h(EBU)E>X_FIJb|#NP6%&6NYYT^u$11RCGRanC@|Sb9H)?kZ?3RwywofVr9y@5_E`irI+&SbLAUb3wdLqp$(u9^r>rf$><Ks<U~FvRjM-E{LEg4B_9K@Z!&#&6f`Wudju($?zAt5b`zxKYk!^ZByjnxv~TSzeT{UD!_Seo_CdhVfL->ey+I~lByTjw6%L@KCQDLdGSJw+bJG*9uW!ScKRa}w0b)OCTi71IM$VT*`ZGw%E-?BU()e7^nY&XlGWzHJeWAi{bx>ELRh6+=S{Q%>2GU-??CX6gZKsNTHqp}*MCNB!IXYI=6V|X<E_|p{Nn^wXMA$-=QKvWR}^2Mmg*K5SzxJ{QVXSi{9qByNl*5dC|a1?@cRw(oj{5w8MG=i%HLOko5J$DGq_0`y?&>wzQm0|?2Q}r?|7WVmlut0XJ^!Qx}Du2{=3&jffVs$u(Q)ciUdD*L4e@+A+G`8$l(f}tkTAh{>???#lUet`--siC|s`6KmOUjIf$nI>!!2y+IhdkZ~iE7^0i91&fsEi6p)W5Yj)@ObKqcunIk&u{IE(=cZrH6f7;-EjmPlI+hz%Hm3%bWzOMuO9l~yv!dA(HK6+(T@1oG>@7P-JZue=ecSk_|M|`nS>w5!-k<$Rn;WtUf-G=gOs}2QoUcFYQM8f&Z0Yqc7YjE)^0L~>PR-oB*UgfD-Li~b1l{Y&V|3bvZMus(=?AJ!Ksl)j-50<Tff^?kfSm-y6B4bkQ4Kg;+x)#4Wx#>E07EtC5qpY>E1H}ZGExHCW^sa+TwnQnlb~EbGP`8;Q%wBh!5N5AGB*C8Wg<TciwHM4$rPi>GC1ZSW6u=@+uKeVShSL86upOAZ6a~}EVDd5We|}2`fPL0*HI<c>tw7KcC6@T45*Io_;wFf4Wx(0N)u4OyXf5y$Js1x|@~r0yEYS#`K1VJAxC|%juOaj!@MU)w@nzWUb=!lXpymaOxWP4(__edqQd?T5&h>F}C~y;;sYg-00vb8B1=HjnO%3s<Dc2vV5y)h4^}TY`spF%DIEzHGbg5-L=4+|!V?sEL2RwVmg%bJf4*Nmi9Wib8oKtc6UF@f;I3!}u2XUtiMv@W<4f?quJ@CS3>v+>>{zE62zN{9J6oN;vl+i0IvI+xK<N0n3@^KQRQGBD}yEcrOH!&!sxegp*5v$~A<`R!P>6ITcbW=u`5zy?Jwym~JDcTs@plF%J0AVOVdp8i5XVGK@*e~#ufol}tB*@!jj%q7(7aAbbE9u@l`9?An@>-OGZQ|w@_Swl}t3;`0jwxX1#AMj*(N4eJ>pHFNoqoUF7aW0(6fQrkbi82PL3pW-6qT<qJcd@NBj^~KS$?a)eXq1JH(T8&1+7sZ3&0J*9p;zUL&HPmqlS8U(#b|uZ|r;1o6>_KXq$`I|1EoZYa$VQ++l`N!2ZrpD>RbA`B6sDbY45%{H1<-%X^>AeLqb)z=y}stp}ao(xY1&he8T(e2e1?rgM$VoBus~#@o)L3BP1s8EV<D@%=G>Js!V7-PyDu<~6MY9C)N9*Zu-8k$Q8QHEaX+)>{*ReF5!&?$jI3y3Gk`>|mb(=ahXn)^#&y%by0x5>OItq-g60ws3Xh|HVmGF$h&lILgcdjX#7nyR)ebCP%vM-2iJz^JTq|@`;Rzf7$XwhuM>**`0}M-dyYFrFojkjPIpi7`*%D!TaO)?~cy)zdAZTI=}yK)ZhJoJ3jj6=&R$y4Uo6PDXo73a287t1ruj1<jS_=GreEPP40??=KgcriK7)<5Kby|ase8;gGIocE6{4`1t6_<Hnxu7oFpmQzsWWo`#+)ae(d~uM|Ns&-o{@iG1{~F@4}k^rRGO`zR~(sL5RCeLw|wPsY6zyHv64hjrd<^HBtrOm#Qqaz0)ODXM1NSV~sVo<$`IXU0{;^xw$hjAQuR>&h$B7ZpEVS1gM{0T_a`OgtO%)M06X?4VwgJQGx@LHz$<m@BAt9d7IJ^1;qUy?*CP&3GP4Zy(l-J753MXyz^d8fDiCu)`^Myy4M2h(S!2^tRVVG0D{150SCRgP-1BlZq=PIsh0zPDsFw|rr{yp##~#E3|CGdLYfhRxF#-zyztxM985z%Y|{RZdfTY@ZH@Z<-FAP#5x~bAf$uo|KF;+1misg3F+d9X4&%Q&WdG9y6`L+b<x~t-i0S(|>kDYmVDOt#BoiG*%<f^kd5X;`Dki8XAZd~H3fNTLWbXeBuo1~Cdcs8UGQrb3wmMm<PC}@yif-;dCz}NVc)>chEbF+|gc)yn#60O5{D3!WOj>u}3d7YLU3&5}|Ml*%Q<D6NAVcKN1h?}H#PwC|bL13budyh13bLt)rWv$^e3eIuM#c`7tkkBBNC1MMhoEh#;EDuh=10qdW!-foQ(v%NYEwR1kWo9ZS}R?GSI?U<ZF$s?oWpEm`bD3qo%9R$zF)`xKwA$#%nOPE>+qJOFXE3PVG3uoXpeNLqyEk)<Kp@3rBv^uVUIEM^y}DW__GkHyG%Z>k62?HnI_=Ro+F_}-FF%bg2RXr|5T+yR$Bid{4tDv4%z4%Zd1S9f)FiCXr-wD^DaucE@TS#MW+Mz!>zp~?QwW17K-h)?Y5J5IS%GC^Pv=ew<-fyF|u`U??>I(%E{x9vtw5YId|>SGSYC#a}&5_2NnW7;TVUFPPmh_4rspUnax-1w8C+kF1HzLT?zg}n5>rU%qsS6(TB$RIw4V|#S$POOI8K?Pl_wkc@!N-KdTF)vC-&Xy_(Hn_;tqoF4~_=K!V0moFe;80ErPt_f#LS9sqi>O(j4^vyYK$KlMGhbf@Y_IX5rY7=5mRZ42rQ^@z<2eaO=c{e(g!QMxhDP?0w<P+KXU<<HVxrlJwlGEeVx8JJV<BC^M55`C$%YsE0eV^T_g2s6z%!=_&uk?!}&^gbH&x7+<5C(=(e*s75+jka+U=^rqcm$T4YCRb6)zau>X5K8Vi<1P8LRI!NRiYNWe(ZI;O=$V9%*Y`B@fc}Myjzk-iBDte%Dk{1$vctHtQ<Mu!q!5ix&>+dLo4MLcpMfIjW9^}xf!t|B1>?e4O>ExzswU!PO%1`*4V^Y!EN8*MmHoCZEjh;qvE%;&U9!_`o9NRGno%B|DgV`E_lkIQoj(r{Yl(msa^!1p*oOCoRirJOWS;1&w9~Pn6T1-@8{|+VO}y|)*FfZZ8jAp)DJoV%i`;G40Z08omxSHHc2^qdpOAFX;CXO~b|vBl#2gdJlm5<0yj7ZTEl}I}Rh}^ZIy*VZrrmj=Y_a79CDC5@Ey==Mlu<fU44zT~EvC<-IJgYLBJ^frKo+c`&NwNC<}|YIqgAG2Yi{Ndv{G$UwhBZ5zFH+WC8(AW0E}DVoTE3JG42i{X_OEqeH|$JqPw?E@fZExh^fy$F3x`TmgE07WgYap!(Hdcuk>D*=*~FSoEw1$U8NVuVYDr6QS%ow)1!%&(gJf1awKyk>UB-U3Pn}GPWkaQx89%`u277$HPt;0=pMF0QAvvRTc!cXy&@MzTGIZIJQ8;XXw=!Mv!-Z}iBs4A>5GQV<>r}iRcW?MJzO%)sKe&;<#!nW_dWZ=lp6(jca`{w8|H04Xde?jJFgtY+h;tM{yyU|HwtN%9_d>{ZZ(Ee%}b0ovrOHv5SA~5-niN~N7W*{l)NfiFR@GxUna!PYoB{ENTEM*uLGj~ZV<ZZ6%e}Bw4r$)iABqX4$GFAAsKRi;8xp%D4YeC<MHBp*}$DE4P4+#X_Vv2qZjNAc?g5QyR*IP4>}#czdN0J+vOn)1)%J43<ZtxJPh#=w1)gBl!Y<~s(+fD-nM*rmXfD2^GZSY0evB}zwzCrANn{#SJ=p_0wca!1@kG8Me_DexcB<a85!R-FLeBi*g!(H|FZH|*Z{-Yj)WPK|IOYp@Z4v8A|QUvblGpb)RTWOhjF6$`+#zn+|ipJ9Aq{+{zMoJc?a~214qpm|5ek9$v^o^UF{^=u&2`9T?=IpdjSI0ESUSIzxh14L<G<w!OIgQ$06F^;ln9gxl<pve(J}fgZvvFU4hu(0l&&eUGA$cf2R%QKJ+YHLfkV4%w=8woA;$%MgrNgwNU-stx*!_<jJQ2chC&xedVHlG~wjOj%Jx){z$~*F%%j0Jt^Wb=sNa29pb5x4$6u@4dQX&7VLW(#N%NUHtc&6#AEQ42%a?%k8vhjFJ8QGKC-Akr{5Q5&V;i<<lr5oxbyHRoI(dFF!UhCPY4IOzB}n`Y%Nz8JW}Iq|06|doKalHisCZ*eYjn<z6M!eD*ut|QK<eEJ`O|wE4mv{<j9xb!L|XVZ&08GoQpX!XU+*Qoq;z${HY0-I|5?o$2s$^eMDBS651Y+1)2NBEA8H{C|H^B8a@$sJ+=tZ?>aLGz<5T(cJ^Al;mbCL<(T|YI~1>#)@+US#_HLB%@KgLCC?0Hkwkl)l&J7c6NJ{0?KI{;Mi-wYKTg_*$}zKl6(}dv*0c2xzh)Md{(uZB{Xsw58}v`)+A(()M+?PaxX1yFiM!dR>%O?=PBOnB<FO$G<o1_+WIXOn5D#;vV{^ONxBAoUQ;W-!zWd^H^G!kTg~%Yiv23qPff>l50@7Yp0<q(Y3D4Fl1I?ltws|mvhBugIA&PyDHXMl>2_Ocd3&#&a7G|>lqsCjnd!p8w5T3^#Xx6Dhp*;FzUsdB!4u7(bE0M^GK<f83u&5Y=GC=jw$c{oLq$)hB#338NYD60D(%mrJ9kqL1a-?2FQ_8B7s92y79vL}d$l`i9&Cfx4<<3b=HQcNlgntbc^BA2xnp!3F2ML=R6yO4**y?O;I1j(H`i3%>=lr+ASoK}^`^@WEe9Kg6)gMaNTjTrC4=%4RBGhDYN8CgMrc+J3tF+O^0<yt|X|_xw_u8N2Tw943b4(Y{yoW4f+H#-s2$UML$PJwj*LccYT8)RmKI7wC_Q|bnAv<U_08Th*JY!GL6Ez11M(8fQWd@12W%CsdwfXMRpO>|<df3w1bnsswLUDt&>|lxqff(eeY<IYM_Keekj+(~8dHsrmwI#VQSLn_hvj58x`2NPGr>+(6$Kl3noweOGLFCX^P_nY!rsLqk5zp;mo+45+L3ixK2Z=Mj5yQUnfajK@&|+xC!;bK?gE7AUIb^o_2!5H`8ZV<|Muobz7Bh@fN9qbPRm0jLW@Jhj1o6z6i)djy1{#VsC)b-!8@;0Wr<BG@bP<!RhxQBB74i9-x@wZnw4@h0t+Emnado`3hHEJM+3}{==)ufSocp5?^b8&7vkbby2Xg;2Ngi{IqMpBi+r+09EohBV^1J+@qx>C47IV=YHbs-t$75pc(BKWb=T95o>3wHjEJxGv`0Ql={AmB!eRFtzc!0?T+GVXzPT!w=I6kXtcCdf?&3pG?|78E*==|Gqh{yZy4$t>bzjgQD?H_-8c63$_nY_sVdEXf6_XX?4qAB?e&V6Uqh)6vE?bj^yW-NmtRmcXd&KLmLgJ(J;qHUFOo4>TlG8UWJ1=o@#mUjsZagK=75V|z(_W1tY+4<=Q+D7-_@ZI_0Y1`>HtzBjv6oPvT`*Q5QJ$iRo2<52;e){d%IVHI`Kixk#fB&dq|I_~I;h)}rID=959t`B<{XZR^x`>?Tj~UC^pN>wx-9LR(3<H()mp4jWkpSvWywn4<2lQcW+{-waB16D>N*d-UYHv|2EX$U6lh~gIX+yI(Fh08PcDr4ei1MRp3C__kn;FPZ$STYN;1AL(@&o`be1$qLltaBs{pP)U&@m=OFR6U(;ZgY*JDG%U%v8nrtE4aAy0~xcg3umyT<mgayjjIu;9yd<KS>(>VONtfY8d%!+`nakWvrLwoeW+onw5D+_Np(Ps@1oJmDk$^Mq|hI!X>qk$#M1VuZ2>M?f39q(I#81(Q<!8`q0kY2U*piGwW=$0EA*5ISJT<C%AV3bCost<n>35ogot&w*^znjUWw@f}3p>q*bafHyuwD8;$!D9!)n{#n-?rg4l;j6*rABD)Pz9#^e?>9FoJqaJxO+F4R%1=N*7v4htWH9O1kL6K_680e0&C==)1}nkI^lDDOHz31xss4nYWvSIgAT5=v-%bg3Rh*$E9)Dd`#1BRo35hoPi!c(LQ-oN_(8vpL9&B-Iz`te<IPF+nUS8!@RZr)DY+yE^lk?6sN6uIRVB#mAyqVS+?;sV?-*5JM14)wZwT!}hG#*RKoNS)GrbYPcm|+q9w;)UO5zLNXcdE{;bI+!qa*wZppmcI;(NN;WT<qL{S&1!DRo=i;##jG{Pc9vU;QhEo#ARu);s^M)eH1yY6AT)grNKIn8T0nCO=sKU_Ybfm~O&aeGx=|L7dlK*%fdDGhGSM;SEUz>5%a&)bT$okGsf{8b^=%oq9g3s*=^ep|N(kF{c1nz{gMJ6sRhCB6BO0JN<P9J=g?(UE~WUt%XW|pJeReC2<qKOf13GF@!y?95WtwH1?PFy;^E_C8{``hkCc3IZ^X7ohU(;=Way9}4J-|^FO?=)yjPG4CnVw-E4r4{!}>kg*<?sTuy+3oo|yTeJjUz!b$-6_r1lI;EMJ*Tye2{#HI(l}R$zGUwiyC@PI3S*4)lp^mLAu}FC%*hOn5&Ap6K~FXes{V^lehT=CMww;gU8Pr1%;_O_2C-#0ReWot0!?J)HFA}%;jyJ;2sJ!VaydR3RDoQlnaVHkiy|CQ?1naxO4c%QDYzZ5)81AE)T(u1V5=5q{dIvm<^Yq>3n!n7b*i;Q3{=%R@wBJ~&MgPg@<wrSX*>9T1>u_C6SD91+P$IE+UxU7bBcS5oU?TPAPd&7Wf{R&yM-CSR|)i5lodQXAW51}{;g>u^t|7hqsaNq(pN`{?j}TygT-=QF&UV3gAQJkvIj9GG1A9|YE~zLSpbQ<nJSiaS<7JTJj-D&PpYN*68Q63y8yWKE7(%f^0D!B_@&@{@yh9`yq2{^-}G+biAkV?MoHIN947bNvLqnX`^%L!U{`3bw?nCI``vb*A_{734^uEdXyAv7WvaM6A%5uz^fXqCSYSQ5jk1xQVCr7nfQ)<7c*f_dXQ*#lQ_Iv}V+xq&E}q!?OD&sBQRXFJbY_&DgYJ7}xY=`UPA@HUmXo!D?NEZJ$IOi<W60*Fji_j%JTXv*s(c{+N@^>DV*dliqpy}qxv=C1tr?<%Y01_pobi^j*_4<mId&O9MB*FMhKUR!=3i^vQii8nhkn!nh=DUXBYgCu8&xcya-eV#-ouff&7+^2|6toZtul)9$NUmLa1V_^oJY~w!kf=+0;jUM770*<u8ng;vow~GY-8K>OeLz0Nlsp?mnr>Sy3sI_<~;WOGoPnTHd%8DyWuV2@*9syM<)6g8%-^k7DCY75hX(E?G2gaQj_c&w7mc$+qFM1vNhQ<nP{7pE9H@>K~+sP*He#Wwe0r<?_P6?3EaH7^Q!AYGcRMI2<7?fp5e!IjVm0AmSPftD_<CLtnfS(4lM+)&1;rq5(M>NXLHn<`~kn#myBTo^Oo&W;8}mjgF|aBPqa#xtF*IR0V@ieAsb>5BB&9BD&{Ic8qeF$3tpALN4znBx2(X&&kJ4=aC194c$pWE^rbdZ*dW>|CkP!jN_G&9<C2>YwsPSvz4YTHj6QW!@3Mh_xZ{#OE!wIbVv+{EZswp$l?yCc9Hi!f9}H_66j7PLNLeC+Hi|Nj@XyTrA<AlTFQBoLiNtE!JK=3@k<FMV?to2nhr|$drX{3b41izaZ<!>b=JRN*c7^w+XvE}^11^s6mlp1jFG-FW<InSqgL3ejx%JepNBdjby;UsxwDOl0g%-3d_fz&l;KrNPavn@T&dEqiL!mM{29>tq>tG1VdPDM%9Z))r;ZC2$4|9T7p59jS5318KPy*JxA=Tnaj$;)mc}#!84sosAJ(=~ZW$Rm^MwP>8tuc(kgO@2hb6MC8!=%i2*UTr!v$z5(qgL9Hr>1Cfn|JbK(cr@oX+O7tDzTy@^*2-XIL&*TiUh8F&ArfFc%gUcGplP`IE0X8wy6Su7Z`Ol9{&;b0PgG8$S!S(Jhu3;R6sChNOATw6cpLsNvCI387&vLrFL#;td*ChcdeXj*8En@Y({g#RI1F-taY;rv+(JHd36#mB`a8XN&J3p{8ckw=kVC_5gG9>tc!7Bj~Wxl&D0}irtS^pohNiMr3xvgdHMa-QksUxtW3v}Wz<zoF;GH5MOgv1F@&kL+Z`0x6=tEXC4>ROt>Ti~uJph}Qk}B~2*2|(Pje^JEUCsJ+B~k#DK%%B4f`hBghNBNDZ3wl1|J`rc$0%E8DqvN{Yct$`dw4v)UrRQhV612gwb*X<S+-XH{2Fqm>;-tg8kR&Y|n;!yL&ydO1zl**Du1=d|qLBF#ADF=(f99pRu<~H8NW-UWmfRV@F2OOW#VBKuH_m#SMmMiXzIvM3(gJY_!Pukiz0~%1IGEV<B;py}Td<za^81QFeaC0>aI-Ib+C?X7~WksxVl8qje{{miCWULDn)u>z|J*t}d-+Rn^-R^W}xgicXZAPkq&V^LrM`$|;@23VtBtVZ}YAgl+~>T9iDZ5)4|zM$+|Np^3kH*gSH=if+pnQz*3P>nU7qNh%*cw&FF`Jr=O7aKp1Ku5@51DP?Hg3dJSiB8uiuGZ96KQOB=_9%koMnFKMDs_-UXF*I<cgW@<l{Ys<E>rgq(^{{j4-JsXN*dPl^gDl(Y*f_0EfSAgP5&aONWlIy3_@XuB;t3hg5TzP23Qrdx3%}<=RyK%&CWKRD@Xt5keGt`c$UQXPp7G;Ml@r7kiZ<*e0g&C8ZS+L0<~-ToZ1V*ORSGJ`0ji=xE-Zc?eIz@Udm|iEO%X0PGEO~C7@Fl*Pmr9W@aq*Y&zfzc?A-9Eq+-;RQv52$a(t9&!QSdlAhK?NwC1hjjJc?<L-8!mGt<B=B9Gve<wE1NWl%V<9D3_cspWbsbXcbED-{A+nHo8`w!;NGMHtNl9wC9K(o~p6u0^E6XhGgQ7cTfkfn%H^<W%GpFV5&{_aeJ}Op?{&1cc2DM-Vv(snIVeFMmD~H&k;pa`DAvQme*<7K!Y<SS=V@Te<1wO$BLi`8`o8!kGGE3{Okk1TU0E<_3zAi0UDSYTSajcEgnZg)%|FMwC*NjOj}1?`DZLt9U)f<J^UlM=@)EX4%jJ(<M$JTX)M0vXl7O3GjQObo{nDPG!TF$}c`~sp}FeQM4HqHtVtp85j6d?1$;R&A;M=3g%QmvP2b|_1$*Ya?7>GfoQ(EkM_>3?U{SmY$DQ%sP|V^>`@#<F}PbSQxpSNNV)?2P>~9=g)u@@vv0|-U-^tX^B;(i(ZUZ|r#$pVj*!(^qAju>wi0ldTEJD3C}>MyGUvr`gkI558=I=f=1V)h6S(=XGh}kGPfpZ$k*=aip<+IypzFsG0mfnHz*kW#wo4hY;^oUq_VFnwmd5axuDIEIUE+lcPoHx1`l1@W9yleNeDZWtvTRvHvK>scZ96VmRs`=P&aNohR&a+teixta8r3(>m>=$)x_m;Of8gM&q(O^RSYtlAq-gF?!4i$F^D1}H$&;3x@zJ{9b_3uV>y{H{pOk%q*{EN0ej0W@$74Ti^oFGU<W$~X0mykR_6Q}djw)?Gcpu7i0yf=1or&f21@`IKxFt=2P(0ZhNPU+{K%$~iCjJNmfg$w#b-B^M0@G;=1-sJ%zmT=kWWg*Ml2{gvt9u07r0E6~;KiH!{>MgxRl%)k52NzwRi-e}h#S17{hi@tbkXU+FWcJ}{d&<j3YaRBm~6=0N633*z&-k;1~&_`S51gkU3Ga#eS1TTb}tXuFV=;Zz}FmbjI1gfPN|wy46W97QX#|g%@j@eR!%0~<YF}Fbatk*>F#V)Z!0B`@{N?u$TbrkMp`}QHnLG2RFxSJh4S~VAV7-kgPNsRWJ4xAI`w~GR!%)XvJ~emX_H0rX7vRFL(S2OrhYE7DT_a{wk$<5T<oa?nmy8>=E9PM(GTOB=X0Yqf5~uFK4u+g@07+ys%6wZ&R1+<1ir>zC~}CKNnf=t?1{leq9tW*#3Qjh;_qxGQYS7#xXo<bJc=sItF4%!^Leo!js^m54i_Ge21;IRfeTN92AYk?z=fwl1J%Hj!-WT<0fSlx7fR5ePVCzo<}Z^s1hUE|<ZNX6*J@=W1%0-zal`slbD>lhGy5QWW;8zKf!&SV<}?fNUKZ~(U*sV3vX|Q5Mk5=NHA>z!$$hET)l^uILT|vYrP0oG<X!YSo$bkBI@s&hyOv7pPZ*8Zm9&jF(ugi3GL=VDN0=2}W{=FIQ!kkz24AUWI^_$NWLJNHL`Zv*5f$$7%`#aliL9}m&^i#Z<3qdv#URJyKfKJIn&YImE%8<>Tcdr)Mw@$v12!LKW2QH?wq6PgW!s-M;wZ4Yv%TYQcQNF9&>igT)k6UVOgS1T4JoR>-Rn86UG9NiRD#6dJ{UVk+3|B8I&J-h@CMPJa$h7{q_bc^aA)?H%&ww@LFY>V8a`1qiZv+QNcNb+vfNebo2<G+Nz^&(z>Kbze~TgsrE%3~xLUQD1e^bs$ahU%SyWlho#7PPLLhfpeFHbEHo$>b1Ye7cAC#PdWdUQ5+GT!gWnNwFYCobv@!4J}q(1g2g+c10P%Z3Hn4h&#s1kcr{$yPgs)0S#ehuwXPmL1vQynfW{(~jhSucA+F?Mps2}^M7UjRJ^GKa<ZQoP`SxJ58q@*u^!aX!Vn#!e)Q6<$Tl4!b?OJQ2!n7S<uEVkpI2)RGvMs&H-NtyTxl@R!NL<#NT|xOa+D7wTyG+V106=R4VxC<Dja6%hMe^$l4a&f+y=`&#xVO2_f<v?H!Mi}$ui6v{d3<w}xJwp>%Sma(_FH@xo6h2O}3DfmNPmDX~3HFd&NqTp+aWDIM0>@miOMRoRvrVdE0hG+)CvB46pQOmn3u5IenEHtrcp}Dp?ww7jQO<B{QuDFOR{bg?`3a8p#VOek&(hO8uAB87gqG^}7y4ZfGL^PS7Fh@MS3GKli?ZE&}dw*!#1I>ZbVzMz*T8fP(uk5mWN2*2Ngj=G3U)G8PQkPA+b4^I4&Fn83I@oMow&}MDr?2@o+Pgx;D(}c1D#-iFzbx+#J-|hG#jB~g)aPG3_3}KBiVS)b`#c!#WK={rTx!H5#1gKg0Z=g+nTloL7st5@Nqub5tmv4X_tP%!pq00ujq|LYu{PXI5cva9U8ccw>D?YvrVrYEHGEHRX{$n8$c_ldPU00hn5R%lS=eo2Tp<=7ZrIA4W3mV%=CX$4j+si7>`jW{{gqSbh*M#cU$1_7&D<>IPK#<L0mA`@t>`4<t~->}gW6)m9;N%ay|uKHd5*O_HfDOD+LPCBD6!Ec##*^6S&i?fa}F+W5H5BS=CxaX(aK~zWF5@P<yJGCL;<XFjb0<Y(RiG~)?-Sk2Jz7Oc+CVzQLkx)<I=IYb#EPgMCY*wqL7ec)VwPh_IXRTvH`gi(vGpbcEdiIVYc_WJP|M_04%|-K?3f?d6C1yipCoP;kT63m*UFRV`Sw?5Qel@4>;n4NVTG&EQU1d%T{!oPO{~NLM=+Nw|mq_=<5nV$D9A`-6Y%!NcB?)+Ga_5lW}e0P+ZHJMQyF)8Qs=>%eBpJX`9_rsT*ZKGGs4H$T=T2R(CLc7*!M(iz%EDc*Uomy+k^=PdC*MvdsM2GZ+b@|CAwwW@tM}1@+@)!XhKJlRib2sAYGbOqXRb6f<VW2pF$k9(!q;iY02*DPSO%DjUCv7{&DQ{f`Z0L7G@+p4LKrXdo!kQlI1~R+T=Vu0Fpu4_Ko|tyP_(5B&Yy_Y%KhVt>_Ou&4tmYv|l)lyc&Q&IM{!=2%4a0@$HVvqnYf%_K3Hud?~^UH)<^KjK;GHRdp!JJ)oNG#L4&-zyF@Qy-ZdK=;sSyS?kQM#GUJ(HC7qf}}GNXRGX&S1W1d0L-SEj;YM0N=C%z6;5T?U(h;-J67&qZJ`WYQ!UFogwq{e4j}~+?yL|yM4-Ix>D5W1I2FkZZ~4<(M{zR}Nf~;qcEGT`yOZB5E)eKjhHgj`VLcGZ`;ot)bXur+<YsBSBzYh^@ycErEm$o9O2O&C8S$9E3;^=yaH2tY6(!sV5(9UR=PVPegfC-O#EVGZ6yQ^x6+s)WFrptHQ0GD71R;aR65)58qm(Dn28s!jancBG_A59Ym~t)qG74e9KVSJFJ>n4yNg81IKZ08l_v^3<Yg|W6Ss6Mqri(A9rcGXGfhQuS3j?U0Fy_ysuQ|u2BBv%TR*WkwRn7nFoDV{?t?{y22$?pF!c;3|w*k*us8dPdy-%MV6u^)?fZ3_X4Rb!vMneKGmrXP%+{In_v_S5qAVQ7jkcBRFfM#7wyQEt~#(lJWdza;%`CSFO`ca>%Ft_eZi-fp45h3_!@m=P1^uj6KdRg#?R{f7fIAVSdkO0|T`6~NJ4RXbI^)d@#%Ul?&glWmE#H4u>dzC06F79KUm$~04Ya`+zP^!~ReB<#Ow&pchISH*;?*ZU@H9xc~LUaIk#;KL7C>0&*`yf<;NvV*H;Dp)^zww%eydYU6=asqN%0?-0@a`^@$|yCJFf7;+rY612F{+#raS~iEWE7Oyvah(V*HT9R%IQ8x6Jy#}@)W%|#z(;q=T**vEegmWQ<F5llLllKoUG<0b$ZS3%~cUZKeKi#d4^>lMPKRjB^X*dT^c>ZG1TfvTXN%>Ms6gLHCLsr&i$c-5PV%H!&fF1bL4MkmqvC-rot|TF&nQ`f@I;l74j%pNBG7R4dj4~$!q0>*rlT+tfGa<{uVwKDAJgSDCeg?W{X;<j48|DYSnKUE!E{wRWtl|x&D8kYeZeZmzmqYjo+rM0glFR<3XnkX~2Q9s&!j4Z-P|_V}BBbp$Fq7Pk?z8sr-0{;4on4SJFWYzFH+5>^%C}PtvVm>fl}BEYY02MXWKSc2l|qI1kQTFh^vQ*&7Vns%SAGUqeHt$ZHz;dsRX;L@bMNDoGkQX)s_yWL>Nh)ZETt3Tcv)**BYYbVWjfvNurW>|B6vVeGH+mB34_wJMS;ZJCnQ5*5uvyPX{}>h581#jtAqOH^=7H=?OU*4MwOSEjCODq3na$MVzY*)rrXv`B;(ba}6#ZnG>)9h#e8nma{)H}x?zzcOu1eopK(YEvR?V9B%hRG*R}WH4go%}{yfE9?tY)LJXr7ZiFXMEvnDqp?NNjEoUqlJY|)+sc!WN|)<ww3!>WYU5CY__E6Yn1WIb6FHGGKb5^b#f4X<43{}u1pI77Mjee?r(rK6*fz@xhG!rNG#~i&*T6ja)5@=@5A)koV$$H!Ffh92=Q1+jVZGNIxg6tzdoP3_20+zw`rG7*)$4ZKeN~s4+i+IqOlI@dkI)ydim^*g=3t%*d-~%%6Rn4rOSc(-rZ~(PDUeO`o!YM}Kv=Jr8c8>BPUXR+4=nkbZD&<6&cc?2b=B=8e&nmvQR$tkl~cFXO2+KDE6O*w%xy~1b%-P%Pc+JW<a=!tEFY+1OKarutZg8R5tyu<34_XN=t-WBjEL{;bg}qbueZI|9`4pX&t~kkQAW~clxbKoyHrlFp=mKz=H8gGZmJbXt+a^tx>`t<_w`b38jqf!N&<^cbAFP7_pn8!yUq3HnW3rG>6K<_pO@pd%LCy#hBj^1Q8o9OBIo4&5uVt+K~tbt?04pVp{WNrU1781<51H~*bKqtd$5i}p)FfTLA_V*UZ@S!()6yxtXiHUA+yGHA}xlaDA!<hVgn;v29oB+7rYN!h6D5tt6(z}W=I3R_BItAmHNwPELQ2-=*fZIrd^X<vNZqUvsEquon$aq$U=g_T=QXu$`o(c10iV+L2-(puuHHA2=|jG5L-ncNTR~P(u^##7k~2NYd^_7&XY?wM-bN=rasp~AeD4lo$kY-)Y-hMHM6YElOU>%J#h&>ucg&x&ceNZf3H0lt-}d5j1kuzB3pFjW@+m%$uPo)sGhGH{GrO{YJ6(Zj+(AsRrfSEfc|Y>*NRJ+>{3q-C9;(~;_|K&Fj0~2iw&dvMz_u70T+ASx)~))salc05T#c%i}6z}Xi->*eK{=yslBA-WfW_0OU2%Y*&5~iRiA2|)OeO^f4MU9kddM)^JV>mX}4KEl$aKD($*oXBvlo;c_OJS;IKR3x54gSyJuGC!3t2*Ym~giosd`rV@pj*b%{0QTDwqb+_XsCdF-gC(W&O(Yh^t9eX^hZKAF(U5}g$$J=4XGSFiJL3dY|k(m<4KZ=R~;cigH8LF%GdlCj*C>;*kX*~xKrWAy8qXsIZJtAJWDlpUfWt9(7^joRy<A+lr8)hRM}N(V6ME!HrVPY*LRYPX=>x@jn)U*@mR$5}|V32=|(oW(ceg|LO?xyTFL6fx^VbUf}2)L7#6cA&(jnrkZ<WJWcVuh;|27tEbjX~`ZKu4B1Svn;^I#_-xkhBbsmG6i=+=@w)?VYBG4`cW3>s2k+Wb~3G{;+89|aysZT(-<1w+73GWxkM=}XG>Z-eTv*vOnnJ6a?YapJo=e?J}?s~bEH^!lPmaQQ`V*59khFx6KJ@--QFEN=#tB;f5whowv4q9`$Ec}8A!*<)W}oBDJI@8?ev)YlL4iy9e-v1$9<?o?A5FCD@LntmW5AcMrjRwVTZ{fTtMF}nukK0*ug4rPOVW!rRcvT)!m6~|5;p&tb0f`(W(}zme`Wr!WA6DgINvL<$KLRt;MbAD!hxVrdJhodoI_RH?Q1>yApQC8XLSu$kHRjm)3JfA-9cWvkt6^$R=ahxZREFmc=0}^}f}EDMR$jgM*~g0v@xGkS7)o=-8}-9}in$<(kr>v}j3^QbLsHcYyK~m1HaXuQY%3Lx>9DY_5n~&M&kEkgUj?#(~!OREfrYAUTm$;)-<#N~xbZ0M%67#+_PDKwj3M50+9wtdef_p!xDi>5R4haN$Q<+!w56d~@qG=%=zP4tt;!(y29O9;<*-kr#wHS9&TnAbiy-;Mo~SqmD`ykpJ|z;My{K@eNpz&AjdfShA8D&u=I=#EXjctWDYqcxKaXUd3^haLuhUuDM{@>O3lP!%*xM#m%K=glQzU3hx*mC&VbkR^cJH`G#&ifZ<^M34Y_H9fleIQ#bsl6r3kB4+?-;F2eEnXgc>_K7m5d{kuZ1RFs%vb|se;<^H0f(D)3mGB33Xsk5Y;#|WhVKVZy-FZq8oXU>fcNW~1^oGM=Yby;)1DCNBQUAg{1C#U6S%SKXL4@4J}tQNv~IBBf_`C#(;kjs$Sbn4xNWnO>MxsTnFq_pdInFIFrFiS*QUu6e`CsQuwi~dPBLyM81GEl)Ow}fW>A_YzTcoBr&+-~fw8Sc<3*%c$cM&y-;Z)25<tFU|t1~B_Z_}b>?%XWvG4Yzl8Sm}?4X;fn~mtKX#ZL4VJis*obeT$4?S$!&G!iBhemO%BMF1n|b<bGvx`^-$~S7BT)<a;vQDvW-}9WEZ#V8a2j!7#3b=~~Oq41;6eM|4P(yctK3U4|fg-3J`sDn_k1acz;=FUO$S^0?<65G%C!6^t3|#b1%l%+IBY$#lw?%tC#&^@UcCBIA4+4iSb72E2CAXm`YE2!^Fy`FQh@rPuYQ(}u0Yx-;cz^2W|;G*lCom|1YK)I#dRZ7jx#nf{X$SkN@U0f~HjXuR4I@AEYRs7$3=yhsG|ouP6n%Q1csg|pyN$lR61kk$t4WRuX#72tmiqxpWH0uP1B(Cy_4SXyM`PqtU+b+StIvIChsm(LZS)$7Wqb(aa;zm8y1$5E2#x!Z%qYOdz}$NyMT^UO7ChM&4~OkZxxwEi@M5;bdhzWQv8O1{Euu+F(ZxeDRrU1ByJk;zDSv8^!!p`lT>HCZjO+Ug&;>2kpGGxv732kjBj(HusTQ$wPRj_J3W!j;nD^#MaCYvf{{BRwe*`>H{Qsi96*U}w`zSApi>3?y<8@(*Tf*3ox7UR*DAVUjH47`u<Gy8O7q{7#!47RT;#Tfgz0%&%klkWI`_zUzLUOSmJx5B=cs>H?r(o=$3$Xuyn^USSdh{!HXwvLqwTf#e*#*)omXYk$%(*J3Tl#LR7^&bV`f<pVVHf7p_&iZnFrlUv);Db*6$6UroO!hsRGl!`?_;%(V{6W)8i`*7O0Wo>S)E`#J<i2veQ<C&DmccUr&C4i9GufJ!{MBcG0?;=Qfh9h(`Vqht$!fN5FZ_O3DD+)aK)V1RM;1Nec%aSW-@;B#(@>y9-<XxhafY}^McAgFc$+af947~&#Y9{E0eJU);8J_|(=)*^p??ph-Bwq2bgF4%#NAf5yCH(<YcllFW<7KqW=t<Ysf~YCr+Z+gX!`fl&UsIhBCJ4~8Tns;JVP+QKya%!!y~2z-81Eq0U-p%yq+{=BWj3>%LYZfSARjvEveFledyq-(*alM)P`tnZsCJj}yf+ge9W=hvr={Lx<MG+a{`t}VvHRxm{P5uX=>5BPS?iP2_a`5Y&#Iao?4N$~-aXhq**`cs|F#_B@&3ER^ZnCr-Tim_$KReEos~m=fBgR4(Le7SBmKU{dI|Z1-wx7c%235Ui_L6%x)Dir&@&q7YzL1SQ5C&wjFOwD9y-d$j7(>48uo#<wIPT+MdkEF$*9O2xfnf+eQz%7pgBt~z1pm?P|Ag0GmXb2_NPJG&=_vUwP3`*Gr)iP-EOm#3`pS4EMOTZX<g&T4Y?=>7m_7~E9dzCt$X&TqZ3S9rDk)o@q;oSp2PXUDNqC@{vvUM&}ICJ^8MI(mhpAO9e(sDe{8H0@|Ty)q2_P<K>p0hFB#*4hrMkkD((&jSkcYK2Hy!T`pfU|`S(nsfLVs*OF-qCgmDO2gTw`P+l4)IFMYU`Ktf}=zvE0(2Oo=f3`jnQX9gv?B8`&@MA84m%nkwz@?CYjdT|38R94nV1eoboxPmxs`Tl!TlBX<qDjul`?@UpGl0xycWyP+_YheTJ{LW|>t5NlPyW7mO+A99!)ZluS7fo}*^L&+h252c!;aa3Y3I3>Uw{F%?3s&+)(f(#;yUZQkP>PJl|9p7*-lj%1o;6uQgx=nOYEAVAAX@f!b_*A`nWYcmqNF%s;*e+ugC}$oZeA|8BT>OJkYiZpF+!SXmAalKVu}z?7cM0cv!I?;ihUv>Q+2Jcz}y^<{#Z^=bu^c9o`IPs*7&`gS62y}qPTJ8n1#FF;}%Q3AN{)57%stu57g5ySbw=_3d2G+ZP)+F^b)Af#H_8MozbzxqZcw?G#Vo_q2cOdb(_&Q-DqG-$6r68rjkjsh~LYWFTdcx=dtb3m;A56;mj!$H!y8(DS{{mR5EKF?ont!!>~D-V=>E2Sp#2c;Yu-{*l?r>KMHX}&1S23;L6<UoPv0hiN6H;j)fnU1npKqmFHGpV`0TNE@k#@6yTK^S;@X>+fJ)*N$ZrA1F^iz{6#G<Rr!9MvMLJHvg$EvAd2qLU{wuNAHZ@ft3q#MLDj?Z@UAV>=Yz3r6tRP1Cp#tX71|oAg#%F0gc@_qFUnZEP3z8P%74pSnY_4&wwP<xGZsiG&!A^bEE0#g1T3en_}dn03e{<gA|gKKXvH!jjK)d!Z}UGD#;n(`WrIq5raH<=N~~B0C)Zyrb*1b&WBk2Q+T%zw7g14sj|EwfxASJf?92@9xzzS1Kc1|p9Kmcg*KaTGQ=BjlV`Rt$a~{C}e|IMQG0gPSUiHlJLx&@+b|?+-=+exLI3BZ1%LfHzp~~*;0;%$(F!#nf{CgD-&n3W?)!W3##m$zbo<#_dhb%et4fp<F09d9-%e1(~8W)uuY?#Pv)DmeZEEq3FC7)QPPMK|tM0ceYnIfBuSO?v0?<wTCoU2PM9jYWTS_JxkFx4;Np|G@JMh3G2ZN+p@PQ(#(h}GATzLNl?kSih~p2vB5sht9d1;$xY41dJE{M_uz9Gfdi!NayGgKU#Rzi4n5Gj@nNUU;LQn7l7Ly|t%LT*Ei5tg390Z&76^M^%uF%^B*>($;Kuhn{DD$ht?@GYjX0mfAmLX^dWpBnb+_G;R031yhP{al6ZVsYq;#3ZpuphP=A=an(Q)O0&D#s=rxVj;Y}13Xrqn&{<<Hp3VX8>0&F^3be|ht11LG|0YQ?vVL$8hK*YSeOp8l=&c5S9dxs$e5sQCgy*ij735t(MH{d(g)K0ln%^%iXH~8Kv+`Y4%s{&iTtfkE`NpmXtXDZXbIZvW%Zk<0vOkY~Z+asRO|$qKwldG`@@xoqN>OgdpCpZ5zx#WU(=QrdJnVvHSp*IC-O8Stx2IU3hTF7VJcTmZRfKf|r71u5XINiFmz#u91pR7Q{b;r*e>aX;jUz}7gG&mY5iYgNg5Pic_1&cu_XgR!B`&DMONvjb!Kc}*beHP8-EQ}<<PcTE59;wookG6kZ^pD!TLthCH9#FzfQ=_#&!i*@EtV&}mN(3M#OuJHim5sR(c8dSnGrqBCKZyGN8yz?<JAmTg|VuXHV2`M<a3R}2^c1+=*vWGqEMXuU1#=po!Q@YW>2p(`}_6&H+Q}N?g~$aSCo~rwgynNSM`678>SqAok6K5(PSp89MX%x^-?^})0SFhDJ=oO53Fv77KBf?9Yz0}z5(Vujq<?-m$Dw^-Yy7ox0iR#fw2|<F1F~}<;A~U#g-n;Y3KeP&w<(a!D5YBQF1<u%Iv|6-QR|BBWKBQf<)-9)>T#g=P{p4XK}$X3(s+(m0!luWsG_?qy!;QZXf;$Q<t<qvlzG*Hx?=kC&wnwJS>B;hFkG8n9ZE6t;-;Fycfh8B`;)vEqwbTi6<{e9|`m#PMnJxjW%%Hzc`c8aA()|I-T8}S+_syJH2jqG#qYhZEe-+Yopa_)$0$PZ~Hyw)y#aF^WnT%oXdfidhoNIf5f6HEiXuDYj3c?(ygpo<>`mx!?VM4_w3+Lhi~`YkB6riyEArHgFY<%F6?>TwtE0M-}cSX>EXfo`_pgT!*?IY&a)&%t<vT<`^WnS=SK&w(r!}&@aFLA{SU|IZVpfe1W!tuh-FM(O#PV$)D+>c8l8R2{|x_9jBK!2&Ye@%*7OsX)mi!bV-Vwnm@6(5D$%Opt3~fUVNtDFJn=q%Lx9=tIIqm2E*=Px!1Yao*q<<tVlxlB6JrqIKPY*$>|=nHvG*>+zHktY!oeH4!%CJW*G;a1xovbhxp~x3feo8v+hd_)YmU8OccloB+ualX=Ha`WG3<PIE5@MiuGYK0WUR>VHkR6M?Q9QA!X%ViRnVi0RWP3h;UyYy;7kQ!z;6n;a#kU+zq@KYhXq6`=uhC6Kp9m)$u_qb#!$^HXMvGN9{3vNP0ADx9AUeF$EXXS->#TNeiID<5Ba`eHMXl(lwzKY6{vfODm+N(J*u_iSLyI#Z?JnY=ybMc7t_gfuijOvLn^;b^|nO+u}E(V{_TqkHJ^Bc--!BkyOm|mFq33_#m|Ozt=e;CM2m$NPESdw%F<aJ;O!Q#=+S>!`72+5KwQtNyR@A*!Ez3xoZOgy^L!LsSMX;tT5ZXc7g=U6E?TKDzuA9#=$@VLpC5|bTThFDzw>dkKMtAa34z!2P3$!vikVF#@0-$ALz?t7MI5>&5Xlzm;2GT$^g90rPRLQ1`tuV^z?5WW)Acs8NZ8`QOw*q_j9OSwga91;W3Zqa0azyw#DZc3U|=7I1qK2zkWYgJI-?<OP4!bPJf1&0^XF{Kn2-$*ux<@t%-O^by*P+CVDqu~T*ToC^EWW%0W8?k$BH;23(64^NB)I)cB&YdG^G@mw<ih$wR0~Kf!aAeR`x_B1rV@}JT-Lp3xjm#&OtccHay+i4(GUC!7;cKE}1ake)tn2c=md=yo|l6-<EIqaK)c;aB4;zVaa3FQDo<gZ18t-MEq|iBg&^m9x82z4}&cpd^KIE#5(q8Fr+IbZ%=@@H4iQkgW9M%h&`;S;w`;NkOFHb>zk;%<WesjX4V4d&dmABcTDm%pK8??O+e;unIBs?S+j{)cvr22QDQ-DKX&0+4@IlL{=zeO%Icf?)$*;e?}`Dm&el}ziUINgTf-TaFTb1mF%XQ#8_e1Z*9zK8@OD`O7M>tU^5%y>wVlJCn%@&Qv`tB)MqOEq3QpZ>q2y<x8<A*{kPl?qFoilc^0ZPU#N*8^6uMDFwXL6;M7mKzyM-2w1F&Atjp9q&=$|zd`5%rcZ-Xi<DTNhS(<3#c+$Yh}(Ld)GFH2c4puAnfQI;OFdK35^`1Q)4pJsYHr2?eISy$Rf<@oN@8#TNXK;@DspjB;j0mxPkWC1K&p~?!Fj={?M)AQ&<h-pp)N2X;_;EKXA`$XH1f{t)220MM+iotHLJ?Kj9P__+4i3FP8w<}ZP8rt(J+qYuw7S_W%OR-6s5H00;jxFS~LjDK-eAX!|bo7i`RFpY_epZDkO{=Pm6ndW&rX|PHl#|%^sEnYrUpEWZ$4o>g-4m$N8S}5;7lv*y&dpNEBFnp{3RCRYszjV>3R%NQ&$1FSFcR_ub6S^lQ|Jp&)|Jg`p<)>56+~2zcI4WheDPh`NKUzdu_U)Sm%*){RjqqQ3oMQdx!EcLk#)ue?~nqtDwf9}2r32GXZ*IbyJfKpcLnt+*`YU5E}19VJiOwZX7CG>vceJLw#H_bX?wf3hxBoKyWj2&*PxF|s%WJwM?vBeJE~Bfk!wH_C2_R{K(krwO83x&%@4}djEiPHip(+Pv0ZkQ0!9Um>v|_E`LJy?*j(j1YuR{CB@jvW`Xe;3?2UHXy%A5<E7?}FPP}3UcFf2L<sNwUOs^JOlyW-f6348nu3CrtRgt@QX@TET6Fw(TfBPXSH5OWC4>ybwS{()MWujL|RmwhXDOalW4`Ja{ID{{zK8Rw`4WT!R_r!0v*8d0}Tan;b(;Oo!YTKA@%tNWNnsi^6WXh-8MoiwCUn;wl>}$kDgKn1?(q4DF*Y5AuJxrEt__jOAC<h>zb8cU+s5kIyzUHe52%%_IDW2RP!Qwk9{U;l@%J;WwmW>Vz2{fi`gnkrMtEohkH6^J!$Wx;Xz{`T&$m*6g3km~2a;9qtc)!n70g2VEMduqVF;`qC@Na3ZieIsjxe(I?GKw37==qQp-V;Spx_d(+9lg;u^K>Yq9Ct?Awi1n5gR_=`sZ_HG3B$U55zCY@<Rh@5aJFnp32EDXt@w(efgg{Ntv3tH<QnnBeA3gKbG7v>oHUkl<0fyy?V1<Ez20;<>2!R5x4&~SsA<;}Kq^d{q8YK|u(>V1!|U2Ig|Uk)Gl_1gvRI_JHS$Pg<l=wxcZNxd^<dhi_}?(TQ(r_qya|#?gc+m1j-xOQP&~;DREibxfGU)8z}Zu@aVL3Wqx@UgX|yU6x+RgH#r$&Ui2Aq%VtrH@?$%!*!^*~2>8Te;F_uY}9Nv&UmEgn!q3=A2qZzQ~rXh~&>^YDEV;p=Wwg0E&PCoVHd<ljpGRYmxsgj0KKZFquWqLM)98p{nd+6Vsquz$|;>8Q+BlE6sq8Y11&vT*&;nu=mU;qTGNsocJF&yF!eYLTre%kLjOZ*kxbr_ecTy+TrJx{SR8_OVs*y=DVUD8qg`|`G4;jh>40U_3U>}Ne{jDy*)jK@~p*HuTsl1}^U8!r?!IqlHLAPIOves%+nh;@d4rDcPi*33dJZWVk-<&4y;dNmrhv)5UzNLtJ>Wj5AM%xk4J&mpWeR%yp;erj1;^M)1eb?T$Sw;N*lhqhCi|5!CHX&)-b&;FHaa<lbpJ>jqU8R?S(sW;&ECek0}_Cx%a+mo&#N!fOV=~qmxg=I%%LtB)YxgB<>W&2R}g|_&GEeaP}f+qjqJ(inyZ<m{o%GVTlv|nWDX9o1vX|P{N8bc);DwR0h(CR-MTq5Fy$my#tnTzh;TqB7J$hI0nm(_^bdhNVlV*5Wn%OShUOp^|?s?K$BAYJ1&`{-6vp{WUCGL5L=&(0@l-|D*avz|$HYiN!I=(J&#dNs~<8cmWHi|eI}*J6OF+E=mPSxkY8EPBN`)xpJHcXTo8bh<--us1TC*$Y2r9BScfR*(n|`v4C+{v>l6zf<78SjP9i!F}ShB3{b~aNQYiI9txA{4GJZ(kJI7y*}IjsJ-Zm7j`#v^j81W!SJguzjWSyJlX2@N6x?h%l{DU4cwOeR!g`zjM0QbGYq5r40qc7XCEPavS#f5gu#55KFC7=iSno7Hr{`ROE~s<c>(~?cBc2ACvyVk+<)e|R^fW|V&_v?#*t5L91}vH&7+jq0}O6LF#a#VCHPdpbNU=wY_#@aAAIKo1{ntTzrkKO$G!u+!~|Kf!1*BpRy1Z=GGXx%8V5S{m#ZLg=FeHaT^f55Y_z=91iSVKZV;kqaqT<dbBy$Z3H@;Yua39yVw@GNGITfb;)_*`u{#+4iaU&K8I3Yo1=loMfAtI8-IpGFo;XurFM@CaT#BPY*l1xt$UOkBasS!zR@f=V{1Nv7OAYgKun>~BN`ZKZ3xR`)?*9+=m2K!o3mU;}VYn#11H`EZ*f{4IH3L|O=S*V-pndo#q!|D=_5;ji>;!WM2Sj2le2fXDvcV<(YR;xPkK*K~m5*=<7k<MospENY5d$jl0zt?XXD-q31%3@{PAM4)l`u3rTG0sS0dQM>hOaPNSp4~O3RuMD2gcN)Tp_ezm?(FnHGfVR0R)P6H@HBgFg69CBrK#47Lzvd1Ir6ZCzMSOzG0AXL=(c{Rp@*&$@V9~D&oQ+8w-p!&IpDt0-l&q&~~U6P_&E#ep1@B3jwW(K0by;SlbUK4+n;wNk+n*1BS%%#9pHT59GMHzWT&cvoo6b@W)i9-`!||Sdln*cxhT;Af|ORg`)?5!m*9QXc74I*Twz+!g7F!v|~<CcCag|Fi!_7_5f`|iBWLoaHknY(yRbwVZ-8J7T~h3!eA1l_n&D278zCIJAjV?%jCkCi<Voe=T{NPOkN6Si#D^0=0cM~s!PPRWudwQ@uHCG?g7=!Ur}-7O;JfCwG?EKOq83dxP`c|c6YXWy>6#7@kg`0!OU`DmB3K$t&&!FeGO4?XbqY8u;JV~|NcM!Klt}Mw&%m|8(#te+Zyznbe-(OHp9J)=aDGBehUx^CqQ<7yTqT$okKJphz9^ZeD@82`@V6Nrpsjf;>E<9-pnIRoFG*|FKCD_SW7G!deVb`cAeYY__oFP2FCa>l_HO=WEBRnB#lqReC7k$NYsHRa$JMj;~L0uZQI7R^%Wo4c0WHdlxTqrCJe8c`1-LYm(SzH(D<OXYcJMzwpiHZ?Zy`T^1uIQHms3Yt|1%L5%AFWpYcr5$bjMlFyC8kiBnCP9n|BhF%e?bhP791sO@IUShfFT!*h()+Y!Ut=EK7f6u2Yd5+aftMyqQ~YHZR(Igstz1KBPe$gUX3$iOYnxrF<d@=>hjX|N@)P%(m0?GcPhN3bVGu#?Ab!JOcJq@sEYut908-05OKJGBS2qa8RVWwW4B{u3SnSz<@aNs!^01?a&DawVpFfyKdl3z!#%D0u2RII*`#1*X7C;=Oq5eDvmcb>W<W*3$p6)e(;5fkzV0A%#Sw0O0;#oEcF|!hiYroT{SJ5U{(GH~<cfkxZO4K0Ac1T?o{EhfF*^ke?jD#g2av!u5Avba!6#Q4A1^wbNybwZoMXoG21YEm?@R<6Zj`<a#(60`7Y`j{;uS&?6)Ru6(kJ#W9<$NE~|LQm%PAe~+cyb%;qwY|A*vE#ux!yVu2a#9yfJ7U7!07$KKV%5Bc-5^=Z=CTWCYUEo#7u1m>1bx17;qT2xu^tYI{g%ze?47~&QJ}|!7K8fskSj$W92M(L1(aU&!zDZ(N7f~9_91zd~;DG_=F<v;phV!Crj0qKV7THDMnHA9_aei7ohZl>8iDPh^2EZ*cyg-u$yZfDt@npSZM4C%fDeX~!!U8h?9#GCpFP;($i5%B=zkL66^F>t$pE3viWYb2_(6?pUqNnwy4tVh0zyH_&?wpdCf|4MP0MP{~uK0Cp>Tk`xEza3B;UfzR%3~fCmQy|vET%u%8Tpe=XXj#XG}!6cSj_Ajm8Hy{Ql#Ms4?+un49F$`>letYo9)NxDnV)sD88R(9C6m4>@nY;92V2NV)7gapCFu40a|YkXGAGM%S8(&KW=O(0%V)Z`+z{OzibCpVG3<2Bw>Syfp^CzZt8(m`h+an#ulDgf7PbzZ5e^Mmj?cvF1QPd-kLw>st(TPf~+YP!4oUMxVw!lX7)g(C!!$y6(To|ViVvhl1oe>hNJ`NL$X3yn8=402MmseK^t2aQB0%`y5z%`19UvVQj9WTiiicugEZ*H{l9^zo8ByahU(~w2gnSAOVT(Hx|RU*R^UL;yitESiK+dhWTE-wOjkM94P?iPRd?23^o>@*syoGtE)PAB{TYa>z%8J@1a3SSBBrlGOB`&87sCjGn3NG7QWN+}h|r%%vIv0DK~|7msNB^WU`;@7jo>7M2p(agsQb@wx%qgs6_Hk5!760fXcb_C^I&AyXyviNDMAJ~t;a=c$_VGD^Q$1-PZv?Lyz*lo_Vd#>^XS5xzn}PC_^A!YeLoJS2XnlH>GeT;6J5drPn2hezW~d)@IOZLi3jsr&@z8|7oA1(V0yNSXWqmI@td46SxxwEmtV<;^SKyH5aO-up+YZlUgod{a1*DF=#(cWF><w}noty2e!D<V#iSA3Zonxfk7PuK#&ZAp+MBOnhmsAv#7O5VG#V5D7GMG~+&fZ7%KLiC?>S<|P>(`41!^HVDW)43uO(nTF!DW`1XA-Eg=95Gr1+%x27c-|XP$|sOcM#+^_*e1i_R}8-hOmWVKh3z_ME{$w&|!h9alW!5H?!MM}YHl45FkzghFcWXX-b+W|{tmyXJ7YPAJXjkSoeSv|hlSazY)Vl}QupjeAASHt|VZ;y9B;@WKFQ1zZLyEaqHafFQ}VpKSbJP>jJv"""


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
        prefix="galactic-mvp026-", dir=root.parent
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
                    "Le patch MVP-026 ne s'applique pas proprement dans le worktree."
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
    parent = root / ".mvp026-backup"
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
            "Prépare MVP-026 : Arche Pionnière, mission de colonisation et "
            "fondation persistante."
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
            print("MVP-026 est déjà appliqué ; aucune modification nécessaire.")
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
            prefix="galactic-mvp026-verify-", dir=root.parent
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

        print("MVP-026 appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=22, SAVE_VERSION=23, "
            "RULESET_SCHEMA_VERSION=9"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
