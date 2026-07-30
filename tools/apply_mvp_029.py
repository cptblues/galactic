#!/usr/bin/env python3
"""Apply Galactic MVP-029 from the exact post-multi-colony baseline.

The migration adds deterministic resource transport between player colonies,
explicit cargo reservations, capacity-limited delivery and return handling,
persistent cargo and mission results, and a colony-management logistics panel.
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


MIGRATION = "MVP-029"
BASELINE_SHA = 'ea6e91ccfb7dd72b151db728a5183ffa5c3b3f86'
PATCH_SHA256 = 'ffbac75c16302c554e0913a751125f3d151a752058de23f4e6b8c2eb033dd43e'

MODIFIED_BLOBS = {'README.md': '18449002f0ccdceab3c4e0dbe6606e95564b9b19', 'crates/galactic_client/src/lib.rs': 'e2dc7b1a09a4ef3de1b0c8699196be5ad7fef667', 'crates/galactic_persistence/src/lib.rs': '17452c7d4e2fb81dbc8b7c18ab03be9f53413972', 'crates/galactic_sim/src/command.rs': '3c98c4454f9e83a124b03a20cf361ae4ef27ed20', 'crates/galactic_sim/src/mission.rs': '0f9195d47b3dbfcd604058d030a9d926930b4be3', 'crates/galactic_sim/src/simulation.rs': '3dd525ec36fca64ab21055ad2c5899b18641edac', 'crates/galactic_sim/src/state.rs': '28da2d272fd94460fc854c4863041480c750aa7b', 'docs/mvp_architecture.md': '89496ef87f9565403821e4e73d3dbc39f417ba69', 'docs/roadmap_galactic_issues.md': '3364aedd023bbc55e79a09c1998b07a28e81fc52'}

DEPENDENCY_BLOBS = {'tools/apply_mvp_016_b.py': '1557ff3f419abbf6a1b58b897100aa72da80bd38'}

CREATED_PATHS = ()

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
PATCH_B85 = """c-rlK+j1L6vfw+vqD03^fC)kXc!w;_ktupcdt^%^O4^O%RlrT4LAKpM!`)~~9FY<8virUfCw5~a&clg)+0lu8-XGwf%$ICt)}^betGXMcWRK$v!WMz<y5_C2GPANK(R9i-H&3G_3+}%>di><uqwZ|NPKxhq(R>oVXZ`){?So#g+uhpP4o?Ol>-T!Q+uLiKo11#gwT+DpqxL`i1KS+*b~{^a1ON2*89W&$aWaoW_&b}0EJ>r&Xdbfg{UVOW(K76^!?R$18m4R#7G=ZvG7VWAu<ZI{9FCV!GGAlaG7Xm3A5Wv7^N?kE%Cdkh;$VLLmnHlT<{6tMlV}=2TUnSv@nrl{)?p{t|9ctDphft8{c#?qr_e)^ud!(qhgp|B3s@SC&%zY`hi@4)b{Z!qK@6h=NKGaYb`N8Q(lDfx1V)iWP!LDFO!6f@4zo2mdU&3*ACo-H)9%{F+Qt`Ouy5bI*z64s*kAwr&+JtSBV8ovlJQB(T3Gbiz3Y!-c#kcg!bqTBKEsd80D5GLFrC4e5K?DJltn4a=LOUz_#af72UIa!u5HlkAUzGD44X=?KTfVcP9rGYVT(MXDM0(<5L;qPHMB`cZvg?a@c@u;9>RnGTv;^BW8Mk0m@lI^%0g@ayGmFLI6{0z(AGRUi4oc`SvE~z^jM6yzP1tHi{|G51)N)lonL=U5qpT%aj*!0I<7w=3ZRcHOwTo<L2ME_01H`?r(^gUoQGp*lIDAo!%P5o=K+;tFzH2*E(sma0u3SnE@)#JK^HK^To&xGI69|i6C^7DLmbVZU&JhtRd5<=^IO}%Q3fz7taAOYEWRh{Jd0Qu!wBYxynM+PLB_8C4e=g~$Ke8I2psTwZeVV0V-f;bfXD&wFp<;izoZ1i;3R{ILrZCLp8QPjPk~GYG;ksS3!K9e=me0Q(TLYJ#%C~LDn``4I14g>cWeM+b`qpN=P;ZOH3cBg<7Kd<)`BRGIV$i~us{;QL9$ttWk?0k!V-xygDwH&8GAs4nTCrLDPw{-&CkQrAe|JTm?ZNtn@3o00{CCsfbWwWFbp$GE$nvJCc5wf9s#bq50gi-9FN9v1e3d;rQ`c?bka>T5KFaXgs=+-lkwh3zuOIh!Cr81U<$kXR=5zXFGs?$zu(yf;Rt{BP&P)hMa*8rIgko+2YUfCMt(pl5RX5sG5eD*CeYc4IP+pO3+4ew?&u_s;z=|=%{nVonuLhoRcZi!a>Q5T3QDe6y|l}nc7ax-dCT=Cnvb$@+0tS3ntD7Am&4(=s=IcFV7<G$gRtJ+-R;mi1gl3rUb4rtljt;$mKQIAc^JP6-!C7m$#;h|@}i|<=p~dGNEYFI$WD?(!YT`KWeO(>>_eu|4-#@X{8~K!M<6r-E(M8^#_ywOGGxye`1LE*!X%jiAcjLerl*rfr4~ro4vY2V5PuVNB%G&VI5`Q%KMmROGKGP_<Eu5sp|iKw>GuIV`@PQgmZHTc;WPlgCt|FGhJl$mK+%aZ5ZC~s*7|u6M~JBjOY;~92+InJeUPmI3Y7rFZ~}5cLUP^J*iQu*<;M@s@1Qwv;R?tBAw}=QiAAV3Acw<w_^#E~N?onVKM3KCFTS6IDXf>RC*etc+F^$=jt4-xxahDKu);(^eDrgN9sS&X3#-RGpE*YZ(55MT3p^T&=^UmPS@G+eJd|I>z&EaNl_P9cH3<j+pg(^0Y{<Sp4&&(qw*8hp<lg}Z)FI<PWRKz}bt%46pKl*vk42+Xp$N2qL#A!E`G~y?xg0;nwNs7&KeJ#tK0{8L>QC{bje|Y3THxT{?)5qh|7&+#RmG@1m7yN5@2-q<^w)&{wtAkL1QAcw0ruG6!GFx^SN!W$369wr(km#PVO&7-?!64f-x$T#jexoSElz&D4s7d({$dh9>)#+K)~i9W4&RpHx^u!fXSNc%Fk%-8-aq6Ml&?}Q2~I%~x7Xjp1z>My(CPP3!co0Ko_(D!m&yDwZ7yVW=onNZ{vG(`JWBG6%R%w*I}mm9!G>N+@!FF@5{V*Mr{eW7ts^F{wp^?$Ddt&_&&Ox4tVNYKBiEdke<_k;>-FL^4JP5SSjK2^Ah>ODxb5wB4jf>^CG%wxtCiqYw2URl3{deO&p;&<<F0^#IbHi9pYSu}0@3z2LLiHua(Yj6T}I4LRHK=JE&wyz@9lT`+nP?G(*$pb(*{lzr>V&EFqwhMk2=QNwQ^<AA+P(uSO)Zp8>NcYL}ddTt2e+2g*PCDz=AJq9!0@o!0H+P5jDRTX>y91`8TjDcp%h?!C<fr)Uq)c>}_}YTLfGbUyI<~yrep|(&XKc-JQW6;P4FKeT=2T6uO3zldNs)7_S7+r7ik*ILy-1lUA?0w-2JYyT48U?zh|4qFpB0;qZ@KbuBc|_Jcww(cro?=2puTXxIXm;p@+y|NiOmtEYc_eZ-!A_v+~7;qzzDzkB-l+0nYaAmq_#3H3%(&~;k<LASJGcqcaCQ^J1_25r0AcgZBQB|Mk^DeM@A>~-`KIC?l-CX3daARZ21yl?e;(0$vXw0FA`uJeHcXS}@93YWoTmX$#BjqF`CS)L6UA555{c?U++D?@~)0gC=v7@eLiWsUc(t!;PNX&k;EO`?>Wh=%MN`29rv9KKB6IVyoNdJ0<NGMr^Y_85OWrSHQ-++C-R8nC6wXY<i%uqcMK>mE@BJlocj?z?DtHX0)fq~W}E32brcf?kP>vRM$vu<bxDZ<WjACw0yoKkI+}pMPHWymK#(Fx~n^&i)|A9Itdj(#GoMItTY@9j~`NU7LK@cjKvnVDa%^9UmIRc}l{r?*v9I&A526PQ*3DQQL)yPfcn6<NvKnX*Igi4XNyEbuoV^O=C4;y}yE3-?i7PjmF5-^`gHGi9Gm(64@nRcYZPXipCDEB(Z4ju?oOO*~jW7rYQ^!yj96R$SI;D7xP`!(X1Q8<%T#%Ibc^MuvQ`nA5m;qlwY7H1&w4<Jqg~2nW>+2R6~0nZ3RCkNearU)=nH3&D7Q#fr=jGu<(CPKVR}+MkBiwjr^E{NSa=Z(0&CV8nXY$9~_F`ejVawnn&&O;945KhMf_tJNbN45<>yVp&l)41O6{bql10tgrDcjIGTrCBD%+>1HMyH<e!dSKKH!0Mz_@~7GJIi?>U=xJ@c(9r5$rcP3LCJQR8*=B8d=dU&qP#rxNn&gb|ksPI<&%HCBn9J^t?S=p}o0#9qC8{N3@3=PzG<KGkS{m(xp)YE-$F8Qb}K#RLUm@rBH*uC1j{>WWQpkP^Fkcb%O^o8`9JR8xD!K;77K5|2*`*Q$$D;3UYRtdO{?tk|Bi)q;7)kjvX1(RdS?+*ls(cpA95EM8p>H`*pQk-a2uxdiT$q&wIxP0o<JhJ@9GDd24$#-oInZB?j~rYv-t8yALwu|g=C1}3RC*+8g}jk;WHC==`CVId3oaJstH+(|j681(RkR|IMc)Ipgt4f#b%99h_u$l_=_3j;FK*2<-??u{OyuO8+CrLtwJ%wxQ&0*eD2R7<!+2m5<u{M_p8^gI0nVW2yME@HPU8DXBa)*A2HxVab2{SFu5_+kv*XzlYJTQu$Bafid9h%f1|X%J_jTmd|)@=eSZe4rcdfja$)<-r!?>=s{Jz@^JH4@=8fvoZTNz@K%z6390haFsmq47wOo*K*3TheD)0sH`RZv0a#akhX3(a!V`p3^t}xBjxQ^Bd#Uh@~~KQfGS50y1m;Tw$eaSibeoXV$}zgwaS1bS#dDvldS0X4mw*qLRO4}#WGJ_!a}yvT63DW$eW3t1a!`aX_}-YeOqbxBMrGr&)SX<lp4POeJ~4;Fn|o*TIg@2Xe5r-aMFs!7sswCn(9!%&<=L@E|V1~)POyF$V7|0d7V(q7C$s1RzOHW8=jq>&X&F+7>40}D<ZPg+~tE&H1Sn|C1A8X!}V8`y_9iX1$TP83l^<QRqC?R8k(fDV0owIFF`iL0?HVdc>R%`hiS$RKU}dP`*77=?^M@UW9jM<A%bM1NjD8BZlhTnTqX%cqG*|(RyE?Y;`*SN&)=oNVg!5<f*fvLUY5boUGFZFQAW`xt#*yTtKy57g6S0ZwO!0mE4K@xcF##-TboE4PV)>9ig;Y_NE)jU2sfG~#s$RZG(Y%~lN|Q&{}LD^yQ3BC!dFo7JFI96erlZG8nxR^xIYG+*xLs01lnPxo*Clb$3*!&4TJGna1w_ASKwH7ng{7b#i6l43*hDLtvl_E51NXrf)RLgf)m69s#muW(a@D6LQ1SK`xuo0OU(98r@ytz5<|MQH`eex%A%7fj+Pf<H7Z2v5Q9d(I#$1B2+5<)+NKEvUeXx=C*$zUwVx_{n=id}n5>f~$r3i5f6T-5;ww@Z-l!3G*nffzQK^lWQ00-{1~Q9kqjk4zY!Rvm{%m7KWwvmzx<+l;#e95AT~bnoz&4uJv1Wl4G_l`RfY6m1WEzYl1j2I;(=;lo28ToX9*2Uk_6|CGNLL33BJ5`+YSFBVwFa#szy)djIGKel(L)T+WYm@iPE9)aMzNx3-mSSMV*9^Y*Pa<Bvx4{Oi5{G;*Sc)rwOXl$W*cl$&zcdduc1c;H7QRKBZvSVQL0N(q&JdZs48#m?sxX~>*$TgznHP2RppD#28Tu?VYYE2VYZ4;(Y}&xq^&n3HcY~TAX_Z3*}D4_O+&$$Z}6!l%W5W`3OkstYLS4fG1kk$=z}tB!o@U;hVRxGyR<PF<JTU$CStu7GZK#pY`Sc|Qz4ww5SGTd60aLlWYT`X*jpHaM(!6L6gBiiLJu`U(F}9Gz-p~Xv29S=^(7xnR~UEH^S0i*+~@m^09?T~h8&y&d!y!q6Qw@*d8E-I4lcq}nv>eyC>v!UDMv86)+IlLW9&+_>BzFJHtzj+S=7VCi4)Z&v^oxO4*@%o7T5g+5<EcVk{?Doafwa7XW|3CEc9gD;gDX;-*nYpMLw8=x@#Cu9lM4nVg9~2hL(mu=TR1B&aHz&g&M7U+ruZURTAtiYXQ|-FrCwSqyv5Z$<gco%y^*O(J^~@^vBmvkB^Sm1sF?OuhXR^3aeOiR4rO5$t5Yi__r}hB&BDNTq}o3r#yu9Mr)vFdxSl8dnVpFoa7??z#00Gq*SX6NkN?-YW=3=m?ekWU%9z38`~u>#qKWuw_U%7xLP%or4EDzU#KH$uy51|^J={snw6n9w-~*qpw!Jl=M<d<>!_d*c8!o_xnzG`%*$<`Bhp_nZKN;%$f74|(d55{PpjZzQ>D;nQdA}A@m57aS{2fk0;jNZcbGN+9T}oqTE5a!W75&qs5yPQyuMMa;j~SqMV~e;{Bqy_P<qIOH|}B8;`(4E*r6k%qk6g}<Qg<~Rd~ym$?2)fH6^rXUBBhdGnazfDcb<3a9MBWUb3x0Z=2NlZQ4HWNxMYFwhme>TBGwh{EV2~v1)35#s0eCbeOl_57ioBNeSfoj;&59mCdc$WU{=bh8P+c)Xa3-bQf@(prbuCo=&3}AL*JS?Qg#<R7Q5^p%|lH5#GlXVDQzOaQxNi(IYh{!(_0UCTr-7<2*&T>ZLL5t~{>Tx+~sGO{_*ick-b*5K~{s05)m1M4dBPMB$_*9mq&ORnEThuCIzKN}EZImc-jMM9X_9kGvN9ltxtCarnVZvMUkiDm3Wuqm&&Ze?z2cU>N8voGq3YrfS!WRL~0Xpng**sPvO;?>Jg4hTw4PE|QG0W3(<2P+vymHRBcBdzDsCr-s7ZC#X#ah?qlnd=`#>3MZp1KWX*bT2sb=Vi+n&U)FE?b`>oNlnp4v{sSvlFSZ!X5kkbFH}ySE>TSE+h{I_u8Qbq;zAan(7Q0)pv(w&-*d$PJ#u#bbQfrzhF@6bkZui9c<9w{=KM^TP);+qm$t(7_o$kW6&rV^HLBsL#JRkY{==v|SV%V62WPbf|9A+6NLRoG6!D10#j0&iZ#yoCFvutIyWS7n}UmS$BQLr&;IX<0$v&x7A_w^<p{fMwMl4*QWVWJqC2d+&;Y{Tt<UjJ~#W|-e3?!cafjngPo-(?aR-QmoKI^$>K#kl3!c>Z%*9LI0FhVAg9*;ykL9t;PfV?M%;L9dCGa$}3&Y4u4M1Cyi|$5dfB6u<dFZMTnp{aJLL@;x|DEQ48Ys@?KpRLy<xZp^A?#2EEj`uB!&6(`B#i`6EL8G$0o;$uu&<Os@$UQ)`~!@q~~agv6Z@hHRWP`?InYHM*}vU(*@f^Urm&64(XPN_jE(4d>$5b_bP5pop#dSL%G0r(=E^9Zj;dZ0)#|1rqH%v?u0rTw)pB=d7_uXhJKJ)V?jd$%Ju*N8$X66yP}2<>p}r%UG#IcS%d;%eEYoK7ugESdGpj#;jc*LYMD5Mg_C%?1g8(Vs%SJwt>8(Zb^G%&;Ts5=2F_&w1(L@EbJX@QhJuh%vz?H@Ty%rirM7IpO|o$<yg!?zALbWQ+vdW6Fp30#n~blz<Gk6)KS!WlD>a3y^>70=HW`1H$dz_CaS$dO{shKJ?XI^~9`>DDcOPbbp26@bPk4jz2Uj9451qV0k<b7TH;{6!pG|!*Kasvg9uhg9SN!Mmj19QR{KQrg5^wglb8&6e--sm@FZqG;y#tz+#bwLH-`*97mMXE_1P9-Bc9eJ*V>!=)(~>we+%ek$CS<lx}W<`4W>;&|oM%B4)#*9Bd*HU59>{=>eIM9dO-o=s_EO$)ekM9%S!7!VRx$5=Is}OsJ^jSvh6iImrF>PdKQ@GvJ-&a}ce7+jk8{hnh8_8&&pYnk+DB7BmKG;w{JZdj8Wqc{g`Cn}8a9BXj3{oZk!IQ|iA(oM)d1T>6roJtva#jZ39IBwwL1vPrI|m`oy+@B+1%-tYP&ZqLKdFy>dZI6yfra6hnamtA?FYJid?A~5m-<R!tAvO5-_=V>@U6;YBjdOi30e^Ma~Mj}1tg|&pKioOEft~R&|rK?E4$-VW_Jj<ukC__R=#j7-sE9q<oO{_eMD?JzJ0~rraq6?tS4ug|q8G_0JdP5w8ezD#}JF0@=AA*dZL-6vB5avaCS=!Vn>ZFvRKLV~z5&0<v;`?zJPO?J*BA`iyGaq#-pQtI7Fl8yMJUpML#2e*xm9uPpP?%`18v}2&xa!)+i#V8vOU_T`+GHvig<U2SypPM~7%6}!t(A#;O_*wKMM~H-*d28C*~Z@PL0?4ky_p@q3l_uwvgG9kz3qMW)7NJ2l;9K^g?$^)Xi(Z>A7din@H85~iezfGe$Q6la4U~$UYMQ>)z*pt<0z5Jm#b*m22$H#K#Gj>)mTGAb%H}@b!lxV$FZ^M<f<aCD{L>q>I|#yf-<M{-uBL5yw~k+^>>4lLBBf9s;4P;u2oNAB%uB_k`Mh0KXc#`lM+-8vsrk&2+=7ij(LtmqDoBIkW9qcl{B0J6wgLb{U^+^O0e6!fB!yvgF^r+AMD69xEaF;ZSH}%mkHaXa3l&JDB45UHW&Gc$oF^r_|4Ji&C$!_r_a9|vV3d69zuf;);5eDcC{YVVoJn?gXbGX95m(VVxX{K8eS-qf$Xp8YZvF^OU9?t@L^^!Y)E7WI|fw~_iG>m5aK%r<UqwgJg?|1nJ_??8SC59kNEMeTU)tHg}H97?JGxgQdAyl(d^^2S7>R@E4l4@3b?c^f|mchWgj&hnYL=HPo4b?yR2s24kqWAOuO|YPdS?or)e@9W#Kq$^#@zVi3XI}u#pUZzSDB}!ekH^cT9BHK(AUj(lnd7)1@Yx`M&qIs~l>skhP}m^sRRE$&)Es_}sW#zona*DU$Xu4W=avv+8_UXKWs0b<{0}>`;DvIvEb1J^lS3UX2bPzx=(PW#Z}vXxi`juw;&;8rY7#6i;=Yq!*5NKj)xXVGz1+Qnr<;uN#mow8{X>+61IlJ+i%+-ql*1{z9$;A5M#SL<)o9Gq6-%+ruloyz|M30x_x1rLe@%4}SX35iMmd9XCIGD15><J#FYYbA%uAWL5NhRem%oV%s;W4y&nL&bX{`5}Tr`EvJzg@py)Ocbr8O{*rc)_m--C@jb2-Z^hY>xp{JwV&_L$o}R<vl#M8G308t=(Q4brNKDC&UfRM!NsRD%9+AKKFqu!I)8TM-z93&Fu3-DyWTV{Q+w1J?dsi?`FyNgL3FyI65Of^ELFF)!%bT34T<dJRG5X)R9*c_GETq*a$(JKakv|s8kaJDT=M-6$<_ik5Dnq?$nbVe54WCAn)yrJlW>9%az(cbYJDZj3SgShD>OIA3kY%9{$v(*vVv?qYIC;!fT!<j}?*;xE4xi`C6C|-lt3k!gst@gUiHX!mC<h9E=036OWkWerM-^Lyy2NT*#Z^nRf!v<Pm_Y`qMOaNo9M6Rh|E@9d66SL{h)B#~9~2eOMT?^S)?L+WTg^0>!&o-OSPB$J8QEW99I#yaFkEI$ladSW+_(|34@_-Ab+<~Tn+;covp78ZQ1$<bc&p-Mc=-${p9V0MJXh=TzS()galP_7Ug}n0x2N2<Hll$FFZY>v4sI(yYR6ZfJN*2w$+hQs&HeMkmA&%blyJ3+d%Ml!s>G~O!5Ej1<I%~ayT#<Ii%A$@2-}~+G;v8ZUW{PjD&`_yI<Q?SW+Y*kE5}JNX)Il2Uz%_sAp%W78!O8e-gm*QHv4K(&2IfB=Vt$r6#xawjW`Jpt36uAMxy1irgLL0r{CM(!Gp5>{@yls#g^0@)K!dOZ_0f+PLDK{3^xCxb8ec*M#TCf=w;L!>=(*O!x{&lXm(}`&|M{E8DXk5mM|zw1||_><+g&A3keI@*7#t5yuID+4hCDh2fZ2#n4=lD^~+I^@Y~r2<aRI<``V^q_>wove2`bmbl5k1YlmMo^9i^0;<w`q5FxXt6O1~$ZCl2+)My&04I8HkD%0HK9Ms-a*j>mV-C@SXI=UgC!^r$kN$h!Gsu~1&Q`IkSz~+wYZOV%U|MvUH258!%D-Vi&u1=PktwPPYc*{*$L%SOz;Gkm9Z10!#J~k?M&CZgpz&IHQA_m0=qg(iIJ=sPqr1Xd?7Agt}7V1q8`Ug9cZuew&vbDW;(u{?)W~y08D@cbyD3Jgcj!2l<687aH%p0W_(CF8%(lGosSO87pw_`LIzX``raMdj+y38*64H*{G5CvP@^Bgn6Sxh9|2s%$4CV1HfiD~)hSvWZjQ^7cr*#r?d%Xs)xn!8OF6*l%gvau^bnH<bnG7SUTjnXgC6@7n}jD?*58BQE?`A1k5V;mK~98!DmEIe1YfT)Lp^aOx{^a4+4$1l=w9A(t>@maJuM$bxzDd&&W&9z7950bZsh}Gp~NTMW3FFLgNXBnEqpciEjmbp801Mw(G)95^iM<*fyBI*+-fp9Ab6rn$FM7uXU3l*VtJ^GW<0%)Zw{7W6SkduRTG>9)UR1<h>8(b_{j*Cy$JQImgF}P^N9aI!%94&chxO9sYE#RIdh7B9KWI9sAqvf9)6nP_(q_{eAK%SbO%-W%O7z862_mAQ0vYXF^NGw_Q<%#Ro3_g0K*YG&@JoPqYJ%^XCt}E&C6Xkd)lbG|N0<&Jj^aP4}ju<hcwi=@5CKOfejg*nJ2CGQlp(t<cw;pM?%Jp~lD63C@=b*E-XQA;$nw*68R!!^TJPd%D1r@<oDev!gZgicRTERWrE4ku|FiHU$d&^Z07FGrA@Kt%~+Umm;pm8EVr5c8E&DMNHfbKP)HzX$GFCw?99_W4Y$(q~t)n==<x3`+H+}2i>pdIaPp|;{pXNr=YY(osyDzlZ0WDr4-hCopz%p0~^o>K?<RbCjo>JTzT$t!gk6uE#0J6oN>j-gWPaV2V(X^hqmfoe&lTa$7`bCxD$1W-CJ8l$L0=NmILP!y?(($mK2z3fbd&NV4)$E;~;eWlXx(aJaUEl8t?v+8b2Az&b|cek|y<uEY4gnJ|^Zc8Y^Z8z5J(5Xf{9{$=nl2VMdE6T}H47JKCTGh+ajAUEd{Ai=mqicb;c6K|1T^Ga9dSGx3ez*+_g_7DBteKHXr8igVu25)qeoElf6ca_~vT08NQC*=$KYmzLcD^o>D>-3IWr!`ESKo*Q*J#;1_C!kI(D3DG1W+%foy6(q#R3N|e$x<9#f~;OU+T7U>!~Ios>kEtSuqsr&6T?&0=w)&kdGK>apb@t4Q8Uhs+6QZjX?@!l{3Z8UJrK!yIcF6fl?)lv$E2Au_j$M;<Wl*$HtA9fh{(kJ1Dl=;$CNdbAe$d!sD_0+FAg_SC(tPV=QBjwDeuvdspMmFJ%m1&`zt#8Gv^AoHyk=f_)X;uB?(dnAwJ`APEzG-1}?gv{o&46kt0Xd>uZ=4n+*Ke<ynQQf8b`6~&9^Kk3m3)@f(Y2*jymY*CyhD(nLMT@A7~+{oVWBijm|tlJf>L>q-`z7z~YTW)m5<6lM4*|Gxg3gj3+B9AT?S(^1MUOJ)4`?s}o-W&<CH`RwdM1k(2i{F_EKIJL?QQ<{YIYJ6=l~qD}BM<inlS!-J76*y8=>FM3uhZYJV|;bYU%OsqF1o%dr8?90<vi_xf?;9L@B)#_SwUcCE)+c%1SPG>m*w;D6+V3#wIGsYybCggxrx)Wud>y--@#n<nejjX%6rjjyPBH?3p}W02_vaYW$oYT0Rz4?3j}|8M!g7EL<^>aGI;od9roY{4G(s(Awy)d!$ZY)`kDpFM>g&#aV0%5Ndr$7L9xEZdBU<Si3r1f+_rVGk}=6P)SK_Di15yuGY(wW`>p$+Ju%<vHYGk;0hX+rp-?!v82uDpINbfwM7RxicMtgG(7n!Xzm_@MMeZZN2r>n}E$qF5yv9?~NYXn+ytVqHW4FPAf9LZr_|T!1guZaFuhvVk`K{_g<P?CJ)t*GYykvLu)ceBZa?|lOhJq)A`C&OcdZqqdd-jqbcc^1#W>q5@YR#D2-Q*|CX;X}{`2oSYre#73xpdnZyaM53)lY-Bghrreq$C;MHZz$z){DnuOl6{aqtIfpB^Ytz1bb+1hk%Y~Gic@_N52zswmt^+Sd)ChA7d9)k2_|{6vek<FhJk-_-;xp{(YilP%#pH+Ce_jf}@Yp6#=ag9$yyA$=Te?Yw?~Fos-nQaaGmz&pY(=c7n*X`tSzbtR<t`rOE>fH!RbmWwH#)cXRyC(ENm%=pl!ap0W$id!zR*3eD7DZo(GE2Rp=|{X3&cT*+mh1+e(=9~GmyfxPycy)@I81&UJGXX#w1O-`_9BkhpP)p+tq>}@g@h~rF~Q)mN(KG#eb?%!A-Vr}txG@(oaXrN`n|Cpq_E%Zrd473rboXnyz`d1jbA~Ezny^yuCvuMF4=sBH0K{`n#@-wel3Wsn<D!l`Bl!HE{WbgO(Ko#6}=z<G0nOnLFPJ1k!)N0cz6&Mo^b1{0s(VNR;QylCnpigypMV|0B!i3DElGB`svDh-2h2$Fs6&*v+P&cRPa)u6YNx}3pd5UTYHdaLH<qIYXYtrOi2H*@aRv}3Pbev=SrgCtI3cMw;%K+lUlbJ~#PuM(JvXijrqstz{q|y+h7HGI<Frs<FV6La|a~drH8cW_>fh3ed7a70v0njTV^e}r6j+JATvpV++6^JQxLxr5rYYnJTBAYp)JVs|ji4-;kJr?4T(krF)phDC9JnCMtGbTCe6R%~VYCz7h?g*u)V)I+I^l**q(9_0^^cbU+8)`CU9>>2qebcmZ+qZ=ajJP#Kt84{ooC;{!l+fg4SNm5`NF9ktP0CRErAF+33rZ`;wpa>a)zfXgC2Pfl)eXdQ@g7M{QdP3G$xKh7>dT%=B`?b=ifEHARN%Okc;yR%jZrPATquXx2-S;LRhd{slp$XlA!ftj6Iy+m80A!JmgM!9^hE0WluYxGVm1P%;|Ykh@|R5iWhs~J(L-H$>v<j8_H`2iA4<XYp^Wm~T39oz(~jvE-U7!(yV8p7xPzJS?zo9vv)K5~a>%g`*_ZOG=b!|~wC81MYo&kCsx$Q1bi7AM3I=r)u``a@VwZ@Z<;7#fkDV7h+fs99LUV0fR&BL!!^5#AhwF)Pi+0GZ<Fo8?88oF>LNahzA%~?CM(VV7iSWAscmHAMiX(oaOD4MH;{lx<HBo*-j96m5d?=bT4JL~i(e~n@HW&N66uhq(gVv7@j|g{2SarPw2iK;sIKm<VDX?nB44$^z(VQiv33anzNfE7pVxB6w$+BJ*^`dWV@pHcnMQbia%ttYq(lXLeK`en?tk^>5taOg}4%;_4Xs^vkzluXl=qnptr3_7tEFKj(o>t3;;EBZ1NXR959g#WKM0H}C<#CzX+NI;-51a{+i1!Lx342&Np&^e(;yylWeXk<5`5_;(mj*c>42aNNbf?u@m$q1hq$?YbZr-WYFR8v#X4R~@NEUb5aH(5zuY3KjW9I-?8iX`wbPMs$U7cdBXxld`trRF=U2Nu30vwCKz35jh_MVm9xuQE)@)cHa&&sW@*aW-EK;7G<wJ=Vg4(;3>|8(@yrtz9cCgqwjEPb{opek^~X$|96L1DBR;}PfBvZHxSHJ%6@GS_WU^jm*0*zf3pKF;R3%4+#q9sXjqG>C%>d_y3;$g@0Kva=+fklujJF|E1vw&u8Y`))BWyl{=W3)F-j$jYU6y34$r;ipIwV7}7FE4aoet=sUmvOH4BD#tI4C&cV<JL&+>#%(vjj0il4!yuzPx5^p5wql@x%9c?IcF|i^%90M;4^8YKO+qsqv}6Tn5E+qPT~a_ikr{5_Y^(|#Jp#uWQ9<*oil->giEj&xDG?Fu?s6j7-KpnJBcX;CT7hPkG1(~a*8{V#&CTkT6SI_$549Unyj*2ANj4o6n^Gl_cAesx7Eh$5PthVeKxG+P2}52Kf<_3o{cR39_*>6EC~}KYbwt*-wiZI+w9V3Y*X0Mo@{EDbZmTlY)Rmji*l`%U4Js=yJ7_zviJ{?KGWz4Fi&$>g*?%i{BiiD_bJ*!oB@#;o1O-^jm=b7QCiY`G!PM{b)o6RL+u7dHkXGu7AM#(5TZKEu`9F>LM$)^CmZEBMHZFav*i;np;Aohw47hEZqSUz{6(iJ)$mLa`BB)iH8S%bWw9FlrSF{#ZrBd!&MytW(iFhvum)yC<pvo6^HpN}dxOhB4tBezs-l?laf$Fv$kNRszj=J*X@(YPYHW#yNp>O*v;*`PGd2wkqJSq<d)ZE(HG&-u;NE}BH>ifTA51_a=*by9c8~8Xv^lOHPjKAPp0*&37CtFWxG4~s4%lwTusXJ3?m0lduLKxc#i|nZYXiG!C^N_Vlp=uB7L)C5o_4-36rR2lITIlI}IC~y0+wm&Y-y9lFZ$@<_pIPOXBH^nBB8`mKD90MD1m@`^-JaDN3Gp~;H)zgx-pOuJjqbyhp}#i}?IJ&&m3zy`r|Z0WghPl9h)V5dmtyPtJN;f~yWdp1u?f*iJH=K(k|h9C@+Dg|d=o@*IMKmYXKAg%@yk-WGJUMhi(sk^Mqc4(7V}BMno{%Q$lkcxWnR+!ODyb_u&&H&BjX;TRY=`=nz$10lBepf*>P$jYRASCqDoG1-B=<^Io=sNj(UmkyYebjb%hkJNOgh9x0RMPhyV0XaS*U(P@c6(oE5B^@eP(K?(o8JGP=L2q_J5(ESJ{wpxk0G9^+1t6dFEY=Xn?&1Lr!fqwQAho4Qt^qsq0-PfkUSzB?s|l)JyYWcJN(HtC7SRdtKlep_vD)+!);=V5`sUrQBewCu^81@2q93`y%3u4Q>D7c9A;;yvL5Tv7M?=5e(~E2^=0GPe-r84tV?lxKkeRYfD!%hj4FgfKP3ZrF^<5>ed>*GGJfi2M%K^Qms4>2#`FX*e52W=$q^^Hwx0Z~7W3%rTcHEBqly&q2TxjgLyJ?8*bOfV@#VsM}$@zL_R`P~VD)b#Fs{h^`?2eCpH#r%)J)y#tAztwEpk_`TkNvdXt8?OGdUk4Qf(MNei)ott@=tfZ|jSD0nVcT~KcJFRrh!HRA*0|4Sgs?;URL3vcd@q8*SLTF1|9Pp%9d;NV}3vPl9xkji=&gNdvF?@0OqA1B96rr7^8oGU2hn4pDK_j)hP_iAppx^f=nU`-gru2On2=pc#^N5bNrxIm}%gf#xyPQZwH_?*URaXE$4Nl@HQ<u10m|i2hd~paNUx=t#(Yo1wmG1$)4jq0lo;L4o2N_9MTsWue@`~CS_rgt+7;h!S&?o7m<L>%dFg}AC>oSw?);7@H782kVNQ3^$1X#{T?{gn|=iqZE7Wwy?H;z+_*R{iSpU!#uN#Uxsl|ub8jpoS^TvoWy|M~_8`d9NJ`!n}9J?n1`6%P}&s!KT!o!3_}CKL!(L^Zhpv^e9$BQixSW~aL4)gfqId}S<0eE#O04>v)%ixF;$>AEW#%F&|by|HIHS8GikId?r}eZ_>&FD)c0nrI**xJzop&sqTTH$9TV>B%JJ(I+H)m0mTGE^<}jYe^|leJLbeA-!NT7u^FQS)lzUj8)RSedAY@jl|vSD@eeWm`1nBIU?672lSBga}HVymsJ2XXi`5olJKO+RR@BqK6memsKI8%@dE_VsyzeL<g>y#p(dY(XNj78u4<rnRYzh{dNmfYzlwOR<r8%shJET!u}uxiJ?S9oG4Uxl&2em^7zsws|A-2v7C!%=elprQdXe8-oH15W<oe;l?up3JpVhU!8dzx2`9(p~pt**snK%;@P9nVIYq1C?EpczBgkVKy<>NT6k0WllqRHR7hUD}acs&Kixim#V1*5C)4eCh`J4JomL_Ad7{9|XAuB8c*P}==Ha*!NMHy?=Cjj$R``eP7E8MbdmtBwsjZ3P7!d~{GiuTmwf6p9r)X<C@m@H|TLtW#Y~<v!;MKqh<(J0dXtV1KW(wSTK^eX-SQzP~G&RI{|`J1r5*?BCuy9Dtv0+kOLuV|AtD6H2W{%7<y4U0rEd(KxZ1f#TCB2_lh8WBq`en??SThc{6dJ}XduQT3o1sK{~cTOK7SKb2axvJ&uFSc)9~eBw3pGR(}D2S~nXu$$9Q1=JRnX&AjOwAF4hv_GCKH8+d<D34v;uuz>^%0D`Fs?8Cj(0CM;moGYsS6#+<i#W{kez-v)?d;1?v-7*N1T@}lUsbCnW=go#nfTK%uy9fA?Cf?9*v6o@wO1U4X|BptoGUWGY3U)LU=%Kx@{t4;mh4dSY7ZxQ!=-BuA9=863>6J!%P^e*&&K9akkarPoQhg)kTpTZH42;5z(4rA)*XsgOSK@<PXP*H-4;o!l<4-tO8Hi;7K*!UIjLeq&koY+{!XWVa3i(*s(vae@V-F%s==|&P<CQ{B%G0cE3lj@C4&K;&BIGZf8^x5N!l`<&@QE(RMUJp1_1KCZxws-LAvG0<)wD=V6$$Kb>1Cp_z|@USM0SaqE8%`<W%kbNUC!{44F~)xp+F{7Gkl7r%>B!iQ28$L$%vL0+rz^-ivt}&%)8dE#|VZ`dn_S4%MEBcV66e3!J!LwtWy+lc5a9610L#Nc8LudU%!Mpx^J^AgV`x=<L8+6|#MAE7X+DD!sN5pUT2Dyn5vjJ4UMV5kDB4br*Sd))J>=9hE}N03%AtQ>GMYafB~0##2}r2=sfsJ_r3lze6$~kLilAU`(n;vBCJozGRS1<Q4CDr;vW~*1n9ebicB?Qxvm+0(s{0cqHTCc3&N;T@~uO7F55x)4U;Bn$SEqk}9M%>44uDTGL^X!j2UaHtz<-4;P<;BVUF=hNquhG8L~$!}PG$PmV}&fsb(;Fh}kF)_|@f+Zt?l2HVb?t|{oYxCy!=JcI4x9&B}ywJIbMqP$DQMWC4E3b9{YQ*GzYYM~$4(F-c+R<p)gVqUCer?hfJ@D<I-E6BTYaNqadb|Zartd-@La`NQ9BN5zI+<rV9Ixh+KrM9lgSmeB(tjdq7`AT`+!cE$8ZN4;Ts52|fl`HqL4G=DATaqPZj-qg!wfciz+jAp)1^ByhILDg=2A&@I1?jj0Gf>trtrZF<Z*C;kclT-CO%KF90cDfYpo=>=M}S9;PHk#KcDEpLqWCxA_^Vc)QM9lW741j%UG9ZRvzkb?44lj{Xu4n!?}ot((*|3;&emYh%hSc-cfrB(CVR=w#l}VCjLJlE1<Y%pl3pQ8!qPcfaEAq=i6=g7+bnK!xP!4xd10qd!~8gI2npsCp{Gtsup<9E9Ml!H`;OYFO0#TMmU*8>MFDKe$T=$8I0>go8hSc+*1+S`NNXtXEO#e{m+r$t&U)})kt@;SQY-4SI`gz4iJa`}5J7iSI{~%+@g=_WrmEDi<f<m&VP}`Let+tmbj>o+Av&66pxf{DN;&29<B3g0Zin6FL}d~{m8{j>1Z^PScPVU>+lAfU4%#xcAWnA<QUoEz@c(|#Trmo|AH~T#E^t=i0BP}db*T}*39UOV&r$#1JOj9l(e=cy@+mpyY2sI90m$;{G#W>+0;|d*)rFndTBqC93AdmMEelB9`o(h!#nKkYJ+mXaRl&$*BG<Aw$+Cav>zB|}E0-YMt?CjDUBc5!EV{P?B4MzDvZ4JMR#eP2pfdIqX&meLyAf~u`P#+PPJZ_DvdzbOvMA|pz?(z#^t(XyGUA*LN~l?`yCk65-yK^OHdJas0p82~BuzL9NGiF}g?%(0XQFWQ3WS)JMt^B8a#mt74^Cy)QAs7;!kOtyffpDd_j9}?WGpiaNF7e)&|Gh*dfkTKoTMgLxl(KJ58}5?1Wwmq8NfXHMPXjK`_y<H0CVQS_ZL(t`aLV3dFZE8nWDeF!U`yS%GoGEBPnj+#Hsac1XNKhg#`P*FMXeD>C=M^9PQhw)?FOtxU9r3;bt#y+Rc?$={_p%`@K)ONdG%urr{ev3>WDl--SX~DPO;gCUBCb1dVF}^9!=d73gnB_@l>xQhVBHmD6?9q%ehXlDQI}1j44}o{Dj$0A{(XxStp$uMq*tZ;hJLal4LxJyM!MDcP`|ce})981gd$s)^3&YAO{35(o3H`PQsm@{{@WAe~Q&L9R$S&6ifsjeJIym-emaVM3-Xc!cN=rpqw>e3HN1{NIY;_o7q>OqF_B!9^HgSrCBaNf%2Md2?xmmrP{X)f(dLSjmqdO!>4KWtkMI%%N?^$=gOSD5225EYw4znM|>c;DF8+E|wWB6xEaX<eKiywbRd|EqhINH(e6DP6z#`ip5aNy3>N^kN2#I?W~C7Y!L0?@6rxX!Jrh%$0Ze$hYy)2{np*P=3yIuF*&cAyTjxhUB%irU5o5ki8#p6+N}sd1%hnUN2q8iuS$IA-Ih)BI3DTJLY@QQP|2+A7-&2w4yCv8-}mOMVq3D+Lw~|PITfmw-{KsGrEPc)W%#uKeJL*J$9a0AdadWmivKo-TdesStaO)*3n1;kZ-gB~{ri^o7ueF4HZ&{JLe&!Aj3{vDRHBnly^pHefBfR6;7_8f7bJj}m7|6oWpo8aJ_Cfm!wa^RwkoEdt*(0C&KBX95VtW%xFK8?Y@OerE&4a6UH=LiboqRCb5+`rI9}I@mFC=71NLf~5f2<4>5hPcKBc;e$|XPaf;(@a7K7Rs#d#Y3UElQSn+Kq2?mD&2&!{;4-3wl$ix{46M)%Jmn~J1RJpY8&!}_nOq{}p$6iSZA9e8-L3NjZ#5$)fsw)A&>=@-zKK9REI(>PYsm^>yQ=koP;wd3;~)B2^<j$hO8CayqnTiuwh3&{X}+4~|*PJt{kOh**WCn4UR6wa3;I>j(egBh%$G+sMMpN^<Z1I%o#-yLsjKMTx_Bd#=tK~19Rlx=RFMxgrKClgb4U!5?5y7#kmd|v}rnz55+<<@ZY@7dO5vNPBTyWKGCPsjTQ7*e~ty}h=%x!JVGwT+F9#=X)ZgI=fK$78_w2Og9UFG+ci9?>8jpLJL;58{hIh2s1~hmn~~{2I~A3^2lJqhm#DiL-Fxg8PxQuz}XP4C3Up!@~K`d6<XAn|yJa29vOOGsYtz#ZNUZK_BWGYN;BHc+O}w0{PfDePq8>!`;-PhdDv0qI||ddZA4^kHf6P#6Y=50ktOicv-ZThEo{y*{G-=&6i;uN2eixs>5u%Wv58OESfA1#Al*St;9TAro6uqjYF@FpyO9GXiqs0@TXR5#;p!o6&u;^_c*l$;%83Rc{<vc$dhk3{LuF4wiS5b&H0eAIv#j?Rmivj9;`-nWZV=FR+C<2`0*f)Gk)>?Bm{*mY&{82a$wFw+=bwY#03zg*f<~k++jyQx8L$(h}--!W%#3V!~>`1Gws0O*T~rvZtlK-R-KxAYTQLD1oe>nD%CrA&`s2#m&P3{`g-r_WTT=J^{%*T$UF|J)#jH8xn5t1(Y~Hz)_8UDoRrc-o~?ZaSZJqIcx818R9s`cH&_My5_GPMRW&F!8lHiU`Kfqq-}~7VCl+$;W>!V9!~!ArF}ynk#pqRHB1NC6kLW5oLNsjm=>ECw{!V9$F4v7_i`YCe>poQfwo{wu+B$_%KEPU;ues*fs%`hID(4FGxpLfUMzA6*t#Ag13=m|V3gc;4mujU_R6-g}JcXd2JWM?=8^B~0xcFcrvnDtz;I-J}G{h7`nnkDW0jF)l<4FMI^%MMPwe)!os*)?|$!0|Nblp7vT9=AK&pm*2NGS(+T1hU3c{8fz>NHOqH|R$2-7^8ORsp#Ob?y1`%sU_B!Kx5NCSl|}9N+oAY9ueiA8DzmxSl}h*v@Koa4)11CNrwZ?m@$@D0Ag`hq7p+20WH%syj?ytXncEfD{<4Qee0gbx&51lFXo0vA2If%VK}4*Wu@=^_$ClcYvCp7M>PY&Jjl*q-F`F%E_WUsg9)XF_K1X4j=O}au3xszra}CqKQax<XG-B=|H6$gEpF_(p&Max@wVlgR2%-k_4;euEN2Zs7h;-{4~2ilwV_NT;f>4<fGLv>XHer)Wks8nx_&kO|%I0gD**rx-iV8mnyMXt<0BXhNh57a91Wjaxu4m)lllcImfE@`F=nHkq_Rk<;pK>uh!hcLAyT0Egu@n1>!Jqkuu-T4F-Fi!G2T0+3@mYE7P4iyUUGn{v3XeTXXF;umMA@`{d`-mSO6)y+?TKhgSbmKT6Pz$T(}w)@;U?Y`huMG}?0Ah-=E1-ahakqGpY|-2|KM0zc0T`DL~WndljETxQ#i=F6nj{*6qrbmQ!m%wsi8QW4h@&0(8G*~#cPCZB0$45vEPc5qQdn7{}7lVC6zbi3p2!C-K*)zAbk8>z8=%VOMA(%A>ehX0n#-fQgs{rj+)N_i^mDZiaAr@TV|jLlg%L$^J@r*53gfdIzKF4bDw)T<7**#f_2BGjM9Mcm8Kg$~+>P>4}s4M}hAu5F^lhHQuI_m96l8XdoS{OV}*=IG_|)92p}S-v#@o*HZe7L9&(w0?>s%ydWC40k)7@)cqkouJ@4MAlu~Xw>n}>SjUB21PlK-b7h+62%y{Ymw0;8E5xr=Zg_qA|p)Fl&4{LHUX6SUJ0_<KiEFl4X54i<X}2D>Ge&r@mA*K;(g5xn*Hq#-a1Hs$weZ74)9x~*MG?vo=ZXR@gf1nXTvqN$$lt<kkIx6dqA&`AHOkP?8p}{Uq3rKK6*7eKK#Scw~u9!9}rDIYW-dp{pMeM!M=U-VzW0mV1NDdKNl`WHo5-D6Y54-8t|S95{qz^Tif`d!dmnL!+%56CgNxeU_iP7nVzu-oodlE3ed(d2{Wz9$vwWxLNyG}F^{-J^DIK+J2kRQlPH5<&`7kr{)i0`Nd{2{WY3<3Ya6p<8RjX#5=utY+%&lUKm0yOWV~29)8p%pr(w#1S&*jJAF<#f&NJWuXj&gF$iE~fhY(Z_K$ay-yjK#yGYeum$)CfR;(I*bSP%i$VIWWxwm~#|JO~&plUanb7y^r=$s&ya$k!h`EKTwycM@PuKw)oz;b3Ic30c{VIWi!YkCZPOMurEP=F7DWQl$_Y!8tr-OKg(Kyjz6pXVk<A>|t;`OPoj1WWEGG%5bEUFn`aW1C6^U5#j<A!>r35=SUlP@(*BwTfB@0K>Q9u0tS|q8lQycm;{F9a|SaGme_n4<D3*MyX^YUYa8)Bit2#39rlCmbElhte*juc!&4YgO0^XkFxZ1(v($OWo#r?hXf!Y``9i!9q=VCtVIn_52@Guv)hB><kgU^WK8GeB5E${=4k!~)oE6M_Z6i1j#~K+B?o%<x>wg6@5Tx8?#}Oms!G06SOm&OY5a$sKa+nzqLmWovmJ0Q*knhe>z#)ABB(x3%xx$wmrX2!C?p^;I0wl{9KvjSQj_`ZId@k+8oGb+Dpw`LtUsBFbWrTOx>$zfgf}zfhs*{ibPfXAT1^@)a0=`bE)ij*qD#&RN@xsX<#Jgyd^910%wvl5V5-0)8htR~IfD95n_pU#p6fEQi;0!=INkcSfGLiZjhi;6zL#WoNPQnX1{=pr=@dGDb7zYTa`51X0VJ+kvr??KAB=fPtK^Ela;c1XgxTM0TnYwZ?;{A@3v+Iv3GA<#aPRv6Qub?M+zTpt`yYfZ7pf4|tX%b9k!D6Ho8!~HW+6w6}AwvwdcDI9Y0{Yd~_O!nnIu^=`HnjD!q7*KhJE&akkZuL)!WZlf5ZmOhfBs+LJPT*Q9H>QXvM;~f-r=_S?8`5aFX*H-3<ll|28^#|D4f3hQas#3R-K}#hDRIXkx_0#RMyHd8Uk!hX81rzFfjpmNgkUW%!^C^;)pNVU;pL5!oT11b$0h{>x*!Hy1BL8rWN<;YyoQ!E)gIffYZ-`cC#?ZHzoIRSt?KWPb6Dr((d;C(RaUx*50<xmdiypynlZjOfKRCQ<N&T^*#aSJ}-&cq+j&mpM7>&8s459-i{a^50!+KAq5r<**pQ_kK;l({FvmlqRN5nG#<!K<v{l3Kz8{+9{-5KBt=-9qAY^B!=`5$NjPO{1iOt#uxpH96GrfV|D_ngfgHh}H3H~M1}q{zwB4woC8+`JH6GBO8qf|5Xu|@@uRb7VLum7BF{GWX@{mCB!o}rY_@4GO5nlk1T|m4iF^Hy#)L_KGcD9-h4EqxU!){AM+v-VJ?el>h;&Mq0_J`L`Nb}@N1L-OXcoIPSjRCYz;}~c}Y>c7+P_IPB^WZ!><y<UUMm+?V=`k*?5+=RIFzFfO%r81W*hgU`{=%adAm73zQU!=e7z=92Kr*MeTBW4>f;y2FdO0j@NK`r3k_2L5?{)o`87S>&sz!xwLh2HZ`3-2p&?JHa)&cq-OFhL=-jM!~F^{=6cL_=&7sZ@C)Z<rDY;z>MNF&g|P$}d9!DmMxG-gt0!X6iw-i*IO;=sBj#s*vWdwcf>AgLt~w+B0T6^r-_k0xP3G5|T~H$~`l`Recmv<H2I9)#4-Pv`T%TZSvo<9v*Qlt3zMte{1~mK!zsC<dJYFp$yGf&q!*yypBKs*H;yBYm)o4WWt#3TYbg!kSuwsyHQhD|8`Ly~uJ~jGY>Jh8&Qlgmc<r$gKxyc3ffQDw0yhaD%_ezJLGr6HP@lcKhjwhHf?`K;8F${cr!By%aiW&N8vc0|(2mo0D)e4mQPl--b&6ANU4A?f"""


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
        prefix="galactic-mvp029-", dir=root.parent
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
                    "Le patch MVP-029 ne s'applique pas proprement dans le worktree."
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
    parent = root / ".mvp029-backup"
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
            "Prépare MVP-029 : transport déterministe de ressources "
            "entre colonies."
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
            print("MVP-029 est déjà appliqué ; aucune modification nécessaire.")
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
            prefix="galactic-mvp029-verify-", dir=root.parent
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

        print("MVP-029 appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=25, SAVE_VERSION=26, "
            "RULESET_SCHEMA_VERSION=10"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
