#!/usr/bin/env python3
"""Apply Galactic MVP-024 from the exact visual-missions baseline.

The migration adds deterministic planetary analysis reports, external
planet-type rules, a complete colonizability assessment, persistence, and the
player-facing analysis action and inspector. Dry-runs remain cheap unless
--checks is requested.
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


MIGRATION = "MVP-024"
BASELINE_SHA = "ef810ba0625cae025f345ab5d597c75b835d3df5"
PATCH_SHA256 = "3dba3d346394506ebc8b4d64ebb46793e2e74d10a03ca8039e38a8799650d4b1"

MODIFIED_BLOBS = {
    "README.md": "9852600945693024fbce7d2451799f470c2251d0",
    "assets/rulesets/default/manifest.ron": "264d2130a897f29fef3b41076d076ccac6284f34",
    "crates/galactic_client/src/lib.rs": "0bcb12c0dcc3accddd136f3f06ff0604eb22de60",
    "crates/galactic_persistence/src/lib.rs": "170699bef620b45bbc4e59c8dc823dc440f3840e",
    "crates/galactic_sim/src/command.rs": "a07ea3b86c600d42b96ddc593f8f8f512f42842b",
    "crates/galactic_sim/src/event.rs": "17fcd9cb8a828820ad7163f090c77fdc9f6a7bcf",
    "crates/galactic_sim/src/lib.rs": "85025037a7721bcc6a05ce18db49525fca437b3b",
    "crates/galactic_sim/src/ruleset.rs": "65207909541b704f8eae4901a463563ba58cbba1",
    "crates/galactic_sim/src/simulation.rs": "dcbc30f67b40b80a5d58b5ec426e9ab85142ccce",
    "crates/galactic_sim/src/state.rs": "b829448ccc90c2a1235d448f561eda21337ab433",
    "docs/mvp_architecture.md": "bfd5c43d9cf27bc6349fa2f54206388a4e0d7dff",
    "docs/ruleset.md": "381304d88e4d4211d541b083cdfb9413e1393e28",
}

DEPENDENCY_BLOBS = {
    "tools/apply_mvp_016_b.py": "1557ff3f419abbf6a1b58b897100aa72da80bd38",
}

CREATED_PATHS = (
    "assets/rulesets/default/planetary_analysis.ron",
    "crates/galactic_sim/src/analysis.rs",
)

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
PATCH_B85 = """c-rlKTX)+=lJL8JMO$;mCL@~WO*cCBj3O%@oyeD!WPdZ8%|Z|eN{C5<0YJryt#kHu-uDO0*{68_!anMs>@WGMsxRnnG(b@H%<Ro4u?e8BRn=A1)m7D_Xgp>QA6`Tm^B)}_>^*yN(4LOidH#DXnvKHuY<Fj~x7F#i+reOKyA$}V+v#i#25S!=KGbWjZES29wSWJ6_Hb*rwZ%5z?`;N;$G4xxx1W-57KH5f(<sW~B=RRA8?v)e9HfsHv*;>J($G7P&L<(Xan@dAAK7sjT!sn!55Lb&&)7$Nxqs=;vM5YgoSa9QKf!N*65mc@7<j^<<@7quZhxJIEMBmQ&*l?<cKd4v?{7a%!XS&{+3faHigmxkx*H$a-prp|r&wDr4h`OZvZ~Q=jzpK=_-U}1K(%=i|HzB5{xkN7^;(<Hu;6P^5GE7P(kxj-NeClM+0_F6`X;=(#!|;ria^)xbz6P5(e3qG-7SQkoaiXTu>}Mo_GT8cX_TheAHt3;X3Sq?i8l2x1q{!kDUKLGGKpsw*had>l6V1=8~L*oTZDl?bA$o*n58}&EfTB)GoOdE5j?V4JUxe|u{Ly)GMLJ2k+#_p4C40Rry(>M&ToH(B3Dtm2q!FE0C`Ow_!9!nB1&-tNtnfpgoPP)GLO@UpoPN>{0TsjIt@i>7-kj)?@|UZ^RwGe7tv1uuojDEVqE@o8qP9(bPuAbe*r+9`w2@C+@r9)wz0PHo8Pb(Z(l#`^akud|K(pLra6{BWj-PV#uUT=p(ypwqe%q)VMPS8qJ?M#-9_UFM>!7{8N2;=1h-GE;Y<H242UrNd2;(}%KYFbm<pfdC|uh(nFD<!G2kOhBBOf(Wa7`~agwpiIEhC%V+L5by8RVV_aK~IMF{{s#5uPJJR2}U08KQ@V9GFeAT9v6kKI9m2bVrljhGAz-}^yE)BI`S<FMy(hSLcrDUB@&(-d$MU{f%tFquXG3uw8`4ksK%K)qpd1#L;X0aCaMFZ^Ug>jQv(`-x)&2RGtNCB;dE6Qp+}-v?r)6IzaN%m|YZhyteSPk<2p1<*9nxrq+wjd}$Zl3-K(GqR9qf;55Dv9^KzoKNC_lL2%cAT33>)cpm^uIEI<Q>2U*yZws|rWTZX&sT?_aiF-h4J_!h47kw@dcBB5T^OUET=-E+_+?{g)#ntQiSLtW8fBq2Tv{>!oitoT6Ch?_8e{mO(*nE=;@kg#MMgB4qxdQc0soxh6JR+hd=K9Ps%AjtBb}-HDSS&Gi9+e4Q8@M&lkCwHxWO23*#=4ij$Nmi5Cy%h!Kl~mx7*vJ{&*)0O;J#*Auc6q6(ug%+5{d5fBpXVYpk)x@J}=vvUO2oy|wlb-=!cProM+n1)v|YP4ONf2f)fdYyl!&73BLuh3(cFYt{oSqA$$+<l5uhAWD~j*j-_57XHkD9*{yu3dS4*9sZ}?-q{^>{c(pt5A9t&nk^=iCD1MD4&mHsbqLNK1oTFO!$QHJD}aOCV7i!k+G@E5Sa;-S|2=({R6b;V@nsw@#HxUgX*OhyDGX!+V28>qNUl?Q8uZ|441!|_q}FMQ%I{$6(AM|-3I8<mpR&M^9mm1D>z4UN@Jr-JLv|WY=OGA@&?<E9gUSIbjx)Y|Y~B8*Hn!bOIkasUWH<|x3z&Piv)Qzo<Fo(82l+f?An3!wca;CQnsHL6m(gr5o5txJbe!-9t*qXzbHcBJ(4SRJm<mKe&FSvwnCN!ZbYY6xe7m1H-{+I~+@H|MR+{WHAmt>hn(bZ^jp|~yt7Ep?Q*+(Yr`qp)-c)fw`@s8w;#QjKVGvf$^V=l;C7jiq>8^p)evZ_hHq-6ToalaX9bbe%X?L0Jb3c6^fi7P)WhfKIi?rsPjb*!&PuW0juX@g7f09;D`el3)PomMuA{qO^YEyn2PXfonG^Siqrj(*L4PK?#=#I{-2A{ch58i{yI1R<n(y43G_|6iS0rK4R(S<(&<ueMrU;-*k_9#t)N0aEh4cm?L<z=M&I}gsgy`bF=x5J&lADJ4+@>aNjTwV@kB5c5R*#`XGMXAUZuvbtuM)4G&G8}&3D#`zglga3{*vf#yaPxT0ET2YGEKEAdbAKA{1zbVL-v?Ku%&VW@!Bku9yIK76Bph9YN8wdC(VkC1+YF&R>}9+^yn1oqo&4$K^x%be_)O?LFZh0tHzOWiUjpH%r{gdMeLy|R7EeO4yHzhhoQtglZ~6oXES$cD?F<+GEp{4BL)g)TlJ)Kjio#0YfC~ODOj<Q(e5h?*_7&lYcJBg?`jFs#Sm1a~>e8N2l@5|5PUOpje5o!0+eQi4<Syd+%?zQW1CpWsmZlz8*F3dGc~wqaYkX<4H!lz09vq(>c<}e>;r`x{cksu(=g$w0_f8LAy#zki@3AMW-yS?BlHS=Kph0G5dy`va!r5ZV-b5TXOc3({Xut^{a!b5<A7#diBY!aqE?+0{x#SoGre8u19#QHvzPOl#ua!l_c>OFqUtBzg-R`A$eGD(6$>?nq{@mia|JHU7M+^TB_;ANFmd^d3XWk-eOc%H>n!=6&mBW4Ut0^ZT|M2oY_3xc8vMio8V2?_OlM^2f^(nJ;cuHV<DJ>}L{8gB&Hy_`r@f><={g@j+#0Hp}PJ=r(tYd+#AMs6bf)<Y%Y)4ak#ty#2<~LSv-b5)|SEey(3&F;ZYK`42fqz7I#SeM^Me-r4*zWECHEeXZfitf{6>CDG@mFoC($GeB3cKjx5cXaT&g<l#b<k=8-Sh_#_{g5c3lN$Q1N;^*u97Hu1dtCo<$PqXbwl7sHcDa;g=E1)v*8q$a=0MN8psE#FpVc+1|NTk<LSC3h3_<i@5pLNKaYfU>WhqwX%b)kUPN97;Pb|?dv0`l?T-Et0gGZGY0%%pVVYc=H#+U@trqLFcLw<PZl~Gg`0a0RlF_)oz13^&Nc<{}>ZO-)k_C&b@h-eRkNpJnsr%m$l^xFJi|p&~!s~t5Ha~5$hfmq7IoA1F($rI(JDkMRu#r>SNthA)Z74=O9R6|8Z1M^>EChv+MPnwrZU4APGjEO-GpHnb9S-5gcg>1!v^Cr0SYD;+BW!x3q-kTln&Z~S^D>@=;v0wePH&swz0>ctx?KTqe?FgF!-5j)*s*|l2@Nq~Wi|3wy5Kiv&OUI>^V)+g8_P&+vKyO`+^lIoH;Vkhm@j+sR0j>&iN*{lmqSU&(rDBKahQ_84oAui;_)G9LvQCl8_lNO@}1^y4)+67L!y~)m^0sS0HK^S^2r;+oB3DK1)oYrrM-OFH*`o#D%j?$Zf9p#aaF;?{VPA3;1;2=#wv%E3pyMS>;!|$Fum8%hJpqy(2vlRA6`TO%K8^X+q_!CYwC={xoWU!g}^z!@yKTPz8rmRz;KFGpl^RF=xAm7&^wUexe@0${oP)lAiLWiw7LVy(A+av%hz=Sl;JFF1C?BdiFXOFxF&erX#TF9FB@b~*wu#KX!*E-S?1cQ&La8j4@YVh;nJZY;DXWSjjtM#U}=4s>hEw^r0^DNw}BNs;SILsI3;gVI7z~6k<5rii4JbWzd1Cp#dlKq_a?X-Fjn^yYpVd`K@g9c?d&p~aWv)S*EV3StszFnaj*a^N)V=;AWI;3EOy{6ZJj{@)IJn(zf@n8JW%Y2w{EkeLZ9s-;1%$i1<uZ)#})m}UZ2hlH#U2lLQOha%)*on*$nsvDGGR6mAd7*4MMR*Mz}gGS-@f0o&l$82SA^+2n+>k94EjF?pbOTDK;nakaEaA+^ju3#giHMIs6?w{K0i{h2?&`_V5IN0kGzi5l2KYo3)3p<#`r-^A`b3nO*r4ezF9MES};45xcnkG`syor<-%s#sfT=N#^O=LkT2NR~%olQIyW158#1o4?o<jw~A8{7zI9wyHf#_ceL>o54zF3C6JEnbrz#V+Gx-=)A$1Xa!Y7&yB53e>fY|8*{X+Zv4yXXZBB5zo5<!i`<=~Jce9Taclh$#<Gqv9<2U=KZ;lTP&WP;FIV*8G<6d_1>_(d`d{47PeBceitAd?I?n`-n11zcqVu!GNquGVd1Zlx0As)&$oxD(TCB=1lVZmkEQNC1dMyFQBM(L{1=L)P!Um+V6D`ewX93zh&2;Cn(fO_)N!k=Yp8y`#%+}JygrSz*A{wz*C;pdgoYzPqJ;K*yVwa?mH*nlLBruJg$&l~!2wwKrA@Ms!sPH;xU;n#1&;A`*cQ$t3X0|LcRdx63`oX#^4tU8L;ISW_^^~p<#C>o~1u9ekW`!SAY4cr+wi*VtF9XcI<QfcMUD56MjE$2t&JcHm)(YNNhtf)q}s9TKvKwDfI`qg;~f>Ivj*%VuwS`T#J1<vp*DR^Hy^oCa&jBj%C#?$iiNkqr`QSgo$UE6s5>h$2{>EXc<JNV<?{^{Dr3oJJgo$e=5sy+G1`6BVvkZ%8s=JH60BxsEPy<9L<E-HON*>63~@XtXf*iBpeprcZJEm@G_tKzPH)+V2dhNB62lHhR<gs@G=2{p=zqlOwK@f@N?F@8wV($|!}Ah>Y2pD!Re8KJ?UJSl~yaj;S_;1b)v{{R;ST3ND0u58-C=%+IJk}os^yE1vcjB;}w*1?=~44n)dwZtJU)ro`KPw)UL%92WWv`964__3@oKQN@?7vW^-faJ*&Rk-30%SR-R=y)&wFX7_7tZ<WKsFIsE@)yIBvw$V2${Wxz<!%blWt_ZoX!1w$U;~CUGiP1akanc4VUb<X>pBf+2gTGtK)+c5w`KM1^H;r*0KdUhY!sHYWuNisRFI-KA$&(yzdCK1N2DCW=P9{=qO+KIono2LK`ky0kTU>X%cIk^eWUnZ&MDk#9r~dtDU9XXmXnH*+n_9HWk6TZ&P+{>p<1cie|7Zg<>ATRH-|@ur?-D57sT5?A00kF{O0I@q}_;44^wDoI-f*b+Hl>i#Vp4^%Uz9J)mo-&ns7S9nM(Jx_S(kb>?%yN2qr`fEeX*#kwhJvl8kKO&wxuv{#(k&^}^Y3nhANqe@mIbpDezw)I_OCAP&y?0by$3K~w7F_hnk*M^W&j0{;4c{>#5H+^H!l;>QuZoM1R#iN|Jhi6*Ii<?7u(&D5ESLnCsb`kSLy|M2Ga&nBj&LXNlw{XtIm=8NH}K8~8jb?1PXcpAEnc^&JFvt?09Un-93!{M75+I41JxHP;bF^t^rcE&w14TZe5q{)|Yrc4(fSg=U&&<QP31!J)y9JyTzVpxULGY5Xi8mM7|q6s`_K?a5oqQRTydQ-R9R*#8JnP(UBT=|$3BFBi<lfo8mjt+T%KbQkBSXlK}$b}qRlse<sn<Sw>x;A$EIk-o+pMJdki)LF{3GQN!`6+k9KJW&$1ljGra20I20HE<RkTE31LUfr%4{sc3#{F#7D!L;eii2Iy5SvwGqK{1yh#Cme25$p)KC95in_0Rbq=zF-yV4a{8dQmhF@yy21Y9%IIdlP3!u`^5?_vNC=otjCd;edpN;gYfo}3fYHhR~maeNg24C_(2leV~fX!Wu*{n$Z~T#Uz2DuGQ6-29d`M3d};_;K^wmDc9VcrOS*4k5sfDLTSImSPLYqbE31lJN@oUb>`@7VO7(5iXKDk|n*oK0+~m9Qwf}u`uCcTMet+-~59pf1^Cl<-+!}EX2L_D$qIPP!G^q>RY8ONn3bIr+TSiuO%cZU?*WT6FcZMlwmDySNcW`x!3$m1;tcA8Q-p8Ttb|9Av*swra&3~g)kVBxI2_3_PN`*G{={{=ZX(nS?Q2Nm`=C4bV9Vbz1<P9KG1o1rLDRv;RY%ErA$$49-3N*sEQ~6RPZ0vBdsVCu1dkBKf4Iq^CY~A(8E%`5R$FwJc2d+wxn%U%GumhT22H)obmV)G@}gj_!^E<kg>3NCc)Dx#DMT15tK<5#)WY~9q+WsX|PeosmkX;a`{I(XMy?rC=Oi8mt669SWPks$BUHD@k7DRs_|N7JtkV?yB*neo8C3ex=rUHu{EbtjGfvWU7;_~$fgwvtg>8i1mZpXCdpG+_II{=t={(C$Wt@>SIn1w4qc9SrOG8$DH>*s%Ao5IR~I^^&Ch}D2oorscwZ4)qR_6npR#Z~4pHI5QzCSO5qMqB)SUxy>bRwmi9C33nda;4h`$8=m!EzDZ{|m^&gth`A36hpzIh%eBN5nU#Jl+ki8u<dWJil2iYIr4L>(2;IGK0rh=~ji{e_hW)`9NhQ$(7K9Gg7&JT!i`%9!i)D=Cd`;^vCHV!&=H=hUk(4Kg>q{dJ@i{l$zG_Zwu8uLhAOE=3%wV?mpS8sTQ^Sl6t$uGIv}+Yl%GZ)UlpiUa}0Aon{Rib5Xj?6d~tfbn*E^0Pl5{!_-A?sWz`?A<qcUucX-0A=@p6^_CL7%)0$_@ECo?&ks((PSL;Hk;D1hIk$R_|5mQxzBzOrh?*`XRczTm+{ZQ3=!hu01;T^$TK$MX!u;Drqv1UDCl>PpYO0Ing=rXXcTe1A*dtc=wdjWUd<a#-RlTj$sLmPQ%;kN<k^w+*7nP=KHEkcdN~G&H9n%h;`K*H|FlUpa=CCk3o|Z*tv15<2t8O@mmxzJ8F4hsB)XUni=I~%(_1eYdNi`2k^5*!95<`r;yiwHcZGqe<T#|ep*W{yjUCn_=)LKB3F7cj9){>RqPNQH2S<4Y{q1AtxgmFX6Q_p4F;Mq#<DNj${l{`SR?u|rG>!L$KimralS{c!c20zwjH0BHHi@1+SU`;?9!iBi$P$2w$!(6s2Xs3Lhr=-l8ISBkAu5R9n=>-2<|5({)UlJ-d#8tcN8Yo8(}R8V&~MeR`}%0_<-zIR@t?fCmwQKlIypSiD*tdtkS%daH0efiLq#(h`x0@pjr+P=N?{2ht+#nI4YPg8a*G^yolp1n&+7)qDz9)~TMu=sD6U?1BCW5Eo8PWCYc{-Ia!_4&F6-7&cI|pqHfVPv_guZ}?o?_+L)Nam5t9G9uwz2lrQsBlbOgZhM&2R~Qxsqzp1lznF@w-MM_2rY;V;dH<Ap!nQ&Q_F9*IRKD>k~l4mz1LqsS}xc@>2}H@Oeo*5)>CT()*P-Bx$IzTl_MT`uSMrH3~3T;bQfR0}8uVyiwloF;A=FS~~YsJXlkhsx=FI25to2G3W1<>XRsIg~qAA<f@n-__7C3s?K4JCB5%^uu>!!bMhUmjJ7)hzxYVeLN8S-sNIslF(<VM3te7!T^s)nAP!%?t&tSvAf;b+TA^Gx6k`q!S<+M9mH7Dlsl5Kq%bW9^giC$5<bXf(TuLBMFQ$K7@P%aI%#BSi(3e<Nh0C{<+LG$|HC3&;H$(RXLyV?LtRKbi*L`O`2@NQM0MW5w_H3u^E1CCA9&>6x04VyJ%IiT5phQ^_CWy4h)e$FNGMj~Q8cb%F2z_?<R9{7^(a1aB2`L*d?9U|uerC1XcV4Ce9p%q0&s+}j*gcibkNWuYDHVB7X*CKjB+sE=YfOfvt0-cKrIn~m`{thuZ0{6w&97K5tNcdOanC9G4PE`Pne4~R)?o~)>VWuZ_^Fx&x5jvzpMcDXFyprUJdFyLYYT-K791(5qrziaIo&KJOE5VF_}-I0C<$x@WhGSBgkw}+`^l-*B;In=OSA4WbZ9Sik?tx=z?NHySu=K^=^8dyql4q?><0WF)6Nur4<wSRJY#iP_XK6z6S&Wzip6;4$J`eC4fag$s&Jp@KY0*J~qUe2s`nwLZmRUgWv3u{ZaU_Yvo=nNS`2Lgep=7U<m<+bja`&Unk0^t-RC>gVvN}8NsNx2CYIPVn1%S#cBDe^gM>rZZ}|2%e4^nk;uKr&?aCNNYd+(Hq+~E<#0eHYv#{EBxfEeNLpA*RnUh9V8i;GLygB<$miVG+YB<Y!N+S^uUpPOa_|^k$=7+08&$?~tC5n=CBx5Q&TGqONAa7M5i_?%sOWEP8>oQ#AnQVb34xbJ3a~~D2#^!}dD!{a1{;F!#8!+@E)5~@y}cAGMU3Rt%F$9pNujd}yp-c4Z>2tBG^FI!D=;JWn9B5I)U!W2>f5F(*RUP+kqia{GAs@T{nlU?>0la<7^<*o$%aM{$Vho|yS%&avU?B)!*l`rpDWluBzQXOWl1#uKV#wF+F0oQm$Vc9?=<7BWVl<+RQF%cILH0S*jIO3j_&BC3ZhfGV335k35DgOcAs)M1yeQhR$LxsN{ePbw5v+yNfAXZL3<w?Zx+W#ed0KkFVuRe)uRrFE*j4q1{1rX<zRLhQSJ+8ilWivPMw#>&03w8#ec3~yaC?j!|L$3LrLoSfG^3XisqU-2_&*=Y~!|IW4kL<v4RM`k(oc_03S;0B(V%UV?zRw2_I9B&L2JK!|U|=x%jMEWAKIM#_qmqShWoc%93d)&jIN&U^z<F%1h~wHpiWQ;LJ-|rw=U|<<9nivQfg`ROIa)l7JRTX`{fkF$L`8n`1MAfTv4^Ei2l%(hRt^aP=kyd-&m-(<BUE`12N{UnjUhhppSQ@TWz1Lkz=hG!wR}Ly-Niex8LYNtFB-KM0ElnX{yYV?RzaX)ib&VGlI0FhSca<vWDEar8^5x+eIcL5^o!T4a}TB94ZI)j~7Ns3p8l3$U0_ti-bAvS}2vRY|fvR$5Hay0Nywp;of_-;d|lnhBuA@PBRE%^$dNfpSz8B97#;w3B+8IWM)GkVe`>E4yBvU+`wt0gK0131T1rDw}~jNJdS=2u6~_$^(nE`G_DNnQzUC{SmM2Fdp&RjL4DmMTg)e<Kq?{;DP56>}bQun6|Nt?v}0~a?cI?YO9T)#*JMOS6%k?*R0cW6}F<PsASh!(h8rVl09chSD+LX?>mdTVxg$`z*)Q)?nH$*TV+l&)r9tpFDHxQBuxnmm>X+p12DsNoC@5g5=*%W@8zqP2Sb_;iYoZ0(UF2aFSK`bG-Thq+kDKnet@dquNr>0TA1BxF?Dx{pu0n$+{*EAGGV4tMTHefY*KPu8c8?>KC}2RL$((uUmsGwosv#@8mtp*NlG&S&$a$U(k~!i(=NwxqDNFlGb4)LTHw@DyTd*{(Mq+ES?CI@=E+7CtdOvr#3%|$96}>MX|bBYq33Zt(Z`0BVOaMIqhj}-u#TgDbo)jFgyL}=;PrrQB<JG1>FCHdZvgJR5*x<^Ur%FD_CDt0{m3Vw;PF-dNye?+tSq=JHp0a#hU+T}H3)H?Kk&@8ni`XTst&K#3L434W#sp;QW4=Kp(UwB1h*`5k!z!b%`Fk@%i#hFrVbWG{3<{CqTj2`kiOU{&5*uGK~X5nl5S{<Ml$sLNWn(hQ$wm(PXK1xAf>XYGalH7G>qI9xG^bdiEmqE454n?rd8{%q|DqKTB<hQPg}qgmuJ^zG9thhsy8YcHlnD+p_aoz-|Wv4=*<$Ym5<YfX!aC0MRy(b;S6|UG<v}s)&r)>NADWN>Qk|Z5^buVJqzBZ3h!Gs28;B0*bWWE9LD|oWi>=_&q{SL1jwpz$7vs60+~D}3Ky(3=c4rGgIXlv$fH$_UOi&t$!!C+1&myPL`h%(!{N_ScIgHF+z*gS#3ZRidlJq}OO+7kqDLXlRe5{f*d2agPqa!@lmuAA&R7VTyl2k`N}_$Lfe+p|f=y^LPcYp>=3g|pzqBi{VqK6wTi`Aqm{%^8M62c56jk84OA^6couWu2g)Lcw1Q;_ddm)R;@*2f_D$DCw0c^`F6mqQC21SWUCwSWAc~wk%a7PYd0l7Bna1kshorPy+bt+#wZf*9y2B-?nR>Gv%r=)LPA#>Hf6H)IwbD7wGtb9TaQcy~>Hp-G3g*2oN4BQm&Gf3O>MS9t&Rv~K)+E61k+=K;Pvh+d(Nvh6Op(&9D=GA-mT3V`Js$(T%$L63c+sJ-XDj|?qiTeVX=G={T6OJUeMaG-^HUE`(uZVwIf%9rV`503?us*-`-i6nVk7zRd$ee@4Pi@tD-pZ?YjT48ZN$KR3`kh-%udh@U_4pE9s%3Mb3Jt$n&bU>iR_)@;B#cuzvSmffvHXn_->MzGKt<zzZa?hz3Y&FNs;VIug7j8*3XQsIRQ6z@QEweLY{d?&3RR6;fY#d9siDw?Rn2X?z}4ab`{0eX3fp$-W;v_=3Ts>D2|{*#Uk$#b)#^nxn^tkjX)@^q<ZYPy7}8c%@G=ZIPOhbc1Gcp8AU16`>(B9aNRMyifj3Hs#ET4l=2fy@U^fOU<ljlAVdeZBd1xGF+r{qd#H&Y4mQ-py7(Q6uo{Mh3T;mqaezAs~oT^sqtvj?X7~5a1edL3+8h`x2-ftlVld4G#J2zT|F)R^+Wg}UFC|6f{b{CYw)QO(p<uls{)r6YVR4n#^tzU=fy2_!-)@Sj0QS)4}r~A~8d)HD)d`k#q)xMUH;ur3%iZH)$SA6IA8MIg2ehJdhc2guvTw19;QY~{N(yeX}e_UDH1%{i7d6+Bu=8GEJt^8jf3V!5~65NNnwOx7O4VCMF_EpRJI<!vl^Fk%Y>6L8xcQNDt4NUbCs;?GV5KoO04DL!bbcdv92OY{sN_Z}DW4q-}@w9bkD$x8Hj6@D{tRh!Q(Y-@z-)0UGNg_=nibhNPQE=3D5SEttWoSE!2&&ibNUKcSAqQforO*fZWfu@nl2d;Pm8Y5cOsqPxX30@iBh8to(8h#~11AaFS^ZW<TS*GKV%lU;zACdDF&Dny!JldZg*0pW<$)zB&%{xgYNK8<GINJbmG8=6%JR$Br@K1mdvs1J@_=&A;~*;YjIYkX@L1bSU7PPZooD893|e;l&lP_T#z($t2KQ42P)^l_l~TQqxI_eE=dMV#c^}_}hDDjGj5dWYH}=JplK8(YzvW;!oPE2%O>F(vmns5wvM+Zja~RL!Fa=ii0)bnC0l{b-*!z5{aT+6Y23t7vUG#fR#Vtw?R~I{WA@l4~dejw9_*xdpYUWVQ;$i}(vP{X*NU`9OV}cTA4f{x-^1@ZcDh2y+MI9j$8fH@VYKH^MMO1mF>&-TrX2*h(MaCp~K@0_Sa3fCc^vb`<5F7aHa8u=Fd`wz0WV!3Eeu0d=rgXmsr<&=!D^@u;&{(w;(!S8_LT?IlyP#~X4H}SeARZ3IxXe4iM3TCumHis?Z8a*g-Zg*s*t*Qdk=h7jmL%LN;*ZKMvN<Rlg2}G)hMc#?i1f@)Sk246@(k^=>enTgf2}g)V&**U9y66fT8n=$$IwooV}Bx3d5wU$qe<$_6kj?7TXlB8O?7+EijchYQ;(;StC3J|g)tR1$g+4ScV~JIPG>TB6~IK-0DC3zGPeBFZ@6*9rg7i|IH0yJ?HmEDF4m^qf1|u2?EN8oVt42B*fq~4;D%gbEuEywVr^un)d93*s1Q*?kKO#Hq^efJIEyoaERDq`JnY{!D)-uXB32E=1*l0WV>A2Zmm*2l|F>cL%am)PToQ3lD(Fa~ACy?yy$9sSqdP48HMq1==80NrB;PMG9F0XR14&g(sPhn%$K}|^V)kL9oP>Vfs7aA8B1i76z{G0+wdDE%i-{Y|uWGediatjmwCel*eVG~=+L}=W`-(GhnjDJ4NU&Rew=W=aZj)1fQ|gKXu<aV3DG0602kr!n6-1%YU5O;r-R_8QV4y3l40Wih=Ni?BR@E?%T2@^DPP0$(yx+5ri7KW8gL5R81TBrv%XyB!Dl%O5C#SQ=u(DSdj~Xty`tZi7$M_;2qI9M$$Yaw-z<sNYhz0n_m(oYMu>6_WGsyh{|K5&q+t93cA0l38WDJq}in6DY+zQSQtD?kZ#&i153`=_~fwH7{YXTg6ZGU#1L+pcfmE3h($y4N&>cJe?$BzIUX`1xtiU*HpQ{a2C7Rz-wyIC#YrL>i;E-);^=ogy1<#eH3Laq0C0%hAx11T?vVd9MvVSU#bFE!&9?{ui6Fcg<&9j(QsvD`shu{4z%Q^l2@E`=Qq57z%YC0939VD^GCbE|R9H>#u}gG^N6hE5(0Tu&jNhN^vu72I(zZ(e%M+IKZHm%82;XQBDa3WH;z)so1AJ8Rj+VcW`f?XjCGclF308{o`2**$r^uEyQ&GL~|~xx?IS57p(&s|@O88DlglYtLYbpRA6<n!0`gxF$STQTuCywBlgkZZH*ttO*_TzA4-U%br$PMD8@p^7DziP0~2GC{CapEaDKOmceHkRY~ftZ;Wypq^c4oEuGp@qm&)I0SNy}uxH>3Y6^Eps^hfB<uP7~(x%&iCKo7br50sM{>c<3c{aAv6kz(!SC>7SW4vm_mFVa$Kr>IiT<yDIA#7@-LP<SA^JA?+fa=<N^OpNkN1>%>gG6YmJ^L_6su@*&Owue?YY$u1ki|jlo5{<Tn0sBBj!)H_JHr#IV$WN2gu3#R2zs_ZG|K(!TKT-1&a=97Sn!uC-j`d}FE5B4FLBZ;w}LvWNfgxznzIMTuO!Do$t&74lX&NSr!yFNgU!u>H|P!m9VitIsr%jRruyDyZ*$k%?61}yML?L14YoEnVO+JF5FvL~Bb$Q|C)e`^+nc+No6O4_9AeIumJ?Mh^(Pr+u(flukd7a{CwZ2FzMR3s)*!dawG$t-eD=@Of^LVNhtNB4%nMw{yB!bOW*efZ-?Cm;D=Je-KoK4gMZ;~jQy*9-sLrzsZ@QQ?@)n!y5k^akCV2jcVB$~bjc!Y81*QS4b?=X$mHR19Du<Fb6_w0Fn?H>8gVs2Vjd$`h-gAWG@kgUZ=lyQj-}bh5&v(7;@xb@CgYIVYv01OdfBduj@$@O{cbd(-C7fC3{rPxv2Qa!d^14Ad@WRdW-5M?Rwu>$84n`esuzP;)b^HCU*X@SqW=qCUx*Fw_881Hn=p;^}i)dDmB|Ocv@w(tt1Tve~Mp4ZxI8Gk3WDypvp@><3^rH_{6I{!!=?i~89Dc33RQpx+>giL*)|Kw-j6@o@w!B1uUZ?74&d`EY$89<dYf#xX(q*$9C!V@%LG5Yjdlnl>`kZY*nTi&u2RAH!!VLQS+=vpBiDmLV2k;N}NfXYJE3)~X-dW-2OXTQkTP*K?go@N-dmhg*?c6LYCX}TLvoN~2JdYE+rA?bNpt^BiHRjqXqe>Rd6zc8WE@Do$jmT5|I07NRQU#rKR!(mvzTnWLfS(HWl|4B9)CHu_P5s=5Ek|B@X<VfEXAGOqP^@MZFIF1ESqSXPBj5Noo&v85cO&vx(qq^mT~m^(mb-vTz11rzK$*_y=IVY)i9?<SYf$p}xt~6d@M1nFvsqF`W@^JSTb9nFAY@0fRC}?T4ojd<SvN`+lg#CV(wf2hnMCe6y2UY*PwTD9S|URo)rtA<B{eXY6%_}yV6>zxPjg45$Mkbac^$s%SK?tw9g!}Ns$B5j>t(1Zjn$}3<5Z5F!0GT-FUc536TE_5fqb{VTIMl6XL|MsdF!j?B2#Ge0{&wC4%x}5kwCs&r6pHJyY-O2^seOF^_6p(Q9HBPOYt41A;*_CP1wGl#Y~BE+gq7^iKiU;yX0UpvK=kW#KcRNMv7R|ugt_{&e`Vu#;|Hd@R*4~O16bYvt>!Rj6EvCq9iriYEUQwU2y^2EQ)(N_yG8(Pk)8`Hu=0V=$$~Yd+%-)dRMPR@9LRv0p*)Ic^{3zY5vbSuH|@0i0he8rGGGqX6aR|at%s>Tg-x*M-zYmnSO2eM^|}kqtKEflQ!w5NN4Urw`O9BeVGhd=_ovzo<7_1N{UbON_N6XccomH;k66^mRA!B4zH8Lxx9j|@`yaACQiz$>XMpQwX)bcV-$n&MFaC*BPr>*vH8Bwna^0RfRoHf^~=YRyIScPiS4Vcw0lkTJo#qjbTP*h;IGt6q^2lIGk)WnG<upB*-+HJd6|v0fY|$z|A|*rST{YSwQs19`&$DR?%OCFx5k#-v{35kY9(M(J`?pE1l>N|CC47B5&Li<j~p1&b=0YTd{IGDx)s^&E?OOZ^O?~CUj|`T(1aU*Ee$hFI275ZAqt-MEi^_b2lox}C`X5)23!vXV?ixnS@+cpbdDgC$iU#ccDjX{DU%9qs<R26VS(~fG5vtTb$I2x*_$8D{IrNbw-WbO;&lP}WK(fReTb%@ztYt_gh`WZw0j`5ND`}_;w}G6*=6xcT+&s-)V^M$z*p{@h*OJ6c5k0OuMm6EZmX<H>O9pFbG+yir|h&wR_peV+SB7*)t=lkx7j*TB=+gD`$Cds3zR!~0M8VT9^!-;mnwXp#<MspI7$Eg*Qc(}tcdIXRyQS?vyMe{$&(f6a_QAYa?yx%QlLe%^H3ccudjS5^55X1ojZ<X#ruW?wA**Pcjyins>?hoLgH570cJpv$4>%M&!0>@=`19jH~$B$eyii$<@HIOIID8nYLymns;5?QP6Eq!0!^n#E~;{TPDN3oQW!Wsxik{)0gZ0dqT)xU<iB{+Vl~2iNMcv6oO_LY+*+aq&5^q}0%t6g1;v`dD$Al_bZz7BJox2FJ`mcj0hh7>yrwG+Vcke25uSi%{fRfiOQmL@4)83;p147|$Ot~43Dk}l)OnBa=UJX7oR7cGEtCbVoGZ}v(mX=T-qfErKJrQ*T}Pt)lMqIVOn7t)Rpz%{@b4eXQXQ}4O=kaT6e6SU-*<*3<pD6x2%%<qp5-=dKxRCyP56_yb$@b4R^_JcjA{Wl^WTMyZqI&A;C8sIJ0YjX6}rb>>02n=SvB`M(^qgf{KtdiS1k@R7w{#$%ER+US!!(ZAay71YpqtNJjrcUt&&9QRaKm+)C&%;PSgg#$Fhc7x`~wwl-PH1UIkGVvDIJ7@4{KUxVX#@d*BzYIS)vdPPnAP^MAXYzxi&*QT+JT>o-Ry%Ua#vJAVGk+uwV=w|{v0r|P1wj$XYy{Kq|Oq(8Wfg)6A-j$2k2K>%X4^wwb=6Y}Ww`_2zFBh!K;97S2fIGW%@*!4P{PK)XPG)wJ7Avz_18UYFs5LE`FhI#oH_Jy$jbJXQ~e^*V@bBMl=Uq;hg(p*s`FHsV2>Rpx!%wg)!M%8zv=7q$qsD5WV^!w-S_Ndbvceb|VovC;^YMs88+?%Qu#A{RgyLcNt{7rY!tK3=njnewXv?4|y&o6kmE}mbguhOfNP#Vup=}~-_DZ#AUsp}k#+=wQQ{KYJ|6pc5;zps)}h&e7ff#JE*M10Z5J>{>cb@3USuZ2X=^D)M&3Wc(WKwx)!9E^5@b~_0C;C!oJ4}q$YYV@gMh`d2-hiwcv>WsTy^YwQ9I*CV%fUgV6uLAFh0(c5c;?&&bf|uHt63u-559p>Sv8{iQnKYqz;*T@EIzEef{2?#<?IaAdeYyi&Hk!XOS}j~jD-g($J+VrQkDON2YkqyR@l7i%;i4YXc(UMtk))jXGvKuJN@@Z5M0xA-y_~Ro&c~({;b-LBj*Zz@=0?--WFDFcI6r&D?#!pH@-Q0LB=0mo_r1XRB++S?t+Bh^Xi5uU8@bnD2iAejy$*1p2)7e%tHeGz5seyZ5R!MCgyK?XSwg+wd`y<J$Z^!LUNnnv&GXn=nNUhlz~R|&zn0@bSf+dYD1D^!I*eOPhc0BWS{WQ`F~v!AUd<AD0l^S=Haoq|PQTsm1|5Ig3+pjN(L^;n6orr$x~)E50M4)fMp8h}6{%?goyk|Uf5or~aoN1~NytXyP0s1~-SOQmBsqC=yevmrn^rFa`qCby&W7YpV?%DO?ubBd7AD`?!!y5%5!x^FLDH96I27p=tMQl@61d#j>~*$xJMH$?RwoFzcI)9%Yo;2XT0sK#pw;aGwBet=TgZJQz_rhz)v{j*e?<GWxcXearGu1IN;i^YMmc(gyKL2s{YLilESgW^sUKX6P#J1byo8A!hjMo${6qNd?&2Z;dN_-a*$Jlv$zp`45g&QnO8&9tW$nsO@+}4fnU`^XX3M3apE;W=xeK%^S!>ydIp5AuEC60f*^#Zuw*s#A>ysdy`AHP(j%E_GYu0|)!f)Y&NCD18*~9$Kp5r%12PX%o-pT$S4qoheZx4=fhfXfXus7U%%=ti_vRh)Qn`NIJ9v|$VzB>MsckuG<5Tl^cV6y&v?`UuT^l;zP%B>p>JUjSy@6FMvS7<121owYgDi>q=NVx4_J6!DcN$d`C9w=RKc?~I#BrgMbWUo_RAd}D}m+CTP1ahrGnZioc!%%gb_^^~3Wzt0=1uqq}Ff39BAXQ9wdlM(Tz1`||aK_~GYRy$8myiSW<R!RnR)J>e&>wjTY-UDL5(X4Dhc5bZvTgXZj)_>^?W0NGZaTkK4)5c&htgvA6-MB<4*cdCvu(zdB34ZDDE3F<OjUK6=ZH0}Z^pqyor{2NL$ebs#kLiAmW)B&&}sK>0;_cJ4s&`5XbRmFkf*3l!o{Y!YT76et(!y&#dxL@z2`O>E-B92z`I5tMEAR-Uw1owzS7I+Ue#(BFVG^ZpnPm<Bh|@om3rC&SzfCmU2B~_^Ir3t1I<f5M?(e*Su_q)o?w8qmJSD7cTf#B19UC(y$&7gQx1To**~nB1;Wt2-v}0PBNQ)U@!iet5|+nrsmg~W8)1$+>>O#k^9u#;S0@3_`zp4QA7-#ZpM@!FUD}IJpBHsisH}si*X<yPdc9sQ25I+cGX`d<s%gWej5ck}15?_$wT-5(s1np01)a57rgc@_(b=?=suKk>Ewrww$;DouC>?fp+z3TfGM;9`;j<{klNxfdEdz-D=jSBK!g~!U4tjEiUK8sPPi${21txbz>;u1$<##vhmh8GQA9>V+1}du8#DIv3J~b)p>d|(CqCSeC&bxA)v{v-4NZh*D(46(g(peYqI1SU3(kRf7RzZSX3M|)`K8V&F7$BXgLp$Ydxjytjl2W&7*!n4s*l4yv@r-6<#!Be~hz>zpY)6*|jwW=9DQ{(KNh>)Gl(*3@adDGvxNa&i+|gTd5I0iV$g^02Apj3yDDCr381l1!W6IV?x1^&3QXK}xixvzJRJXEG+KSVA+P=KuF5}f*3XKUDC!(6tW8xc4OFoWgS6zQ=(4d~$e(IfHW15F%v!+>0JEg0%su<;jVPzDY2mQ{t-9F#i9gIgoJ;RFGQnhWxEK5?pi*}U_{AaV4c}1E-Do+r|LaZB3@`P8WJtVjmr7WX~WgEeBfxR@1E_g61e^F@j`QD2I@8opv^g!;PTiPiW|1HMd$_>OV(^lNl?*(~S^tDy?^@G_(G?Pt;cdsw~G(Vrv255Q?h(SLztj^-F%weIh--sq1hY6Zt{UN{Hh8rCZk-kJVN2V4g1G2>Fo^DMnenfnV_T?vWx|ZgJrPD+Uouq?sB8#-t%CxPV_Pt1-1MRYGYwA{_0y6H72kV5xg^ap*5NXy{Mn)Yx7%AL5>7-TV$+ZgVMCvojWRibg%r6pu1SpbU#Bk+fP9&&rcw=1EEjFREF4ZfiH1U$hMyUabRyi!@>X@@7{WD;Ib3ecYZF%}!9iARFAiBs);3z$jw^_ZV(G}jxb7lLzoSOI`vg<e}QRd9K%Jfu{r_wpDGCixcvN<{v5m&&V&RL)9h;NfLpv{4`$;#czNJJI(72>)^xBQVV1^LxkIY(#|`Ga-SmV>{PW_WCxJ#j0wG-n?nr%UQPSco~9)y2HXmxJ%d`h!lZx2d*cIiy@FKl!%&&?<hov2}VwMaw0t6>O#7e8g@_wmv+Z-wHS7))%quMpEM2{XEzd2yqb_4rOJozpvgkL`+KA=F4_jB)5vR%;i=7p?g<;j<QR&_GCZp!d*4o-BM3whpFSb%`%T;LNJY@9lp;%{cTi_MRZ(Uh_va#&}!5H?Kt+z0v-yb(Zkr#qqIcE;+(wMDsQI-NGKP))rED$Zbcz2Z`A$$#{C*&jPmXLRT$j+zI-_PF@E_dgr;3~F|D5dk$P56RIc^Av1A_Ba@N<c=~xw$M^%Y|fG=EAM%Q1sqUjPY1dDbe99>FbY&pc_6@K6xaI@b>m#EFn%~o%xCI{5A(Y>eKbuzCvK6~MN*qU&qPK4#DjC9QH`%QV`^c$LrB=(r=9ENFD!LRs@?}^LOCfLx{$cAQAB#mfHUn+ATR|Hj5rNB4FvdV2!HQ?x8u8%9CKUTaP&Ygt3f^k))sV^e7PJ;23lX=(SqjYRoymzjG1S?to-AS=zd>V*WxUudo!(V=_U7<$JYwny3zD(TSRFZ$KiF`qJa8qhOzmW(2TLZG?Ztd)FJEVRiiMWBpuCI=OmRwz8-H_wDr_`nEqw{x}Hcp4aYleQO6l**V{)3-hQXHJA%4zqIsSA!I`^IAo-iDTPUR~Fu$pX3<LUksin8d9=TxG3iO62#FRay~65n(_)-|6iR20QKc&bhzS->GXrl#ML09m-;4l}2;n#<pfOEarn+t1u=CZ>yrwP}mCrv#`R*HkJDjx0YEA%U!LWMJc74x&9V&(MG6><TgZyhjIP_&HQPJB{09LL)eVm&bgu!w<lMbAC=~_jqn_Wwh^(8$x4_Qm)H;YCzP<O;PO>!q52YVu>6hnqe^WEUl2OdPPeLcbrt9c3&*2JkC?bjoW)~?X=Z=}QHWkNdpHfJP?-CRQp%YG7EYULtv%GMc6S(T7*o8HV-f*>l4)vy5s3s8BGi~=G3HzcXyUd(8Ufq7FIv$yE`7_oJHRT9Zg*v4DMPO9K8zGnS@&V9=wo+v%a3O0p=rOx#M?aL_5;`SDQ*S+wq4Ma0bg=u^1&5Q;n^m2ihSD2OOe8_$BIxkNV^fKGZ<}gLVnuHy&g(54L$d#xeD}=NaNYiqU*ebP(790=j~c_syAsX3i?3w3Un^iBiDv*ud6w*6aZ^Q*YK$cZQU{`6(HMbIomLxq?&Stu{q)D?z{w;g$PdNR26YASkLfK67g?AJddDk)9%EH5BGSSm{3K@t%6f`Z>x*$&r1=YE`AhH{K!>E?czu2IHN_*wj|8eJ&;Au9p$rA*ff!r>AX^cJZoOfv5T+oJLz24do_=qB}#c!LNH$8W8q#Q+~K-%(=ly=HsZuCUrf+(9}Z2>6oN&^A*<0fXC@It5gf%q`Uvw(Ac01h0&J0l?db@Vva(N7@f(jegF(OD-tBB|jr-@O;#XRkD}1G&IUnc`D5`?~C2jA>r{kJXN{QP~Asc}TESZ4V^swO?d&tgC_TDOSK$75$J*E$Ck7sm@K4Xn&MvdHlYKqd&4_+P|Q#7(xa$uBfQ|8j&uorJ%KkW1d>_7kIUwN=0XrV{~x1TaUN<xNaf!kj%(6MI}iqt+S56Zp$gl(LanzqmAqKrALKiI7Nv<NB8cQF&~{|fK@1w*|6{yF2h*v{5A=w`6!0<TG6quWmy8jDw>oS7-g&f4tg0biI5<Bg#t&<N&~07Ak+N7L!;r%{Bfe+Z3EptajiliOdfpkW_H@XQAc;Fgfx{zbJuI-`VZqeX-xg;6=yBaHDfhB7_?0B3-92^6OIe1Y?X))xt;pM_ye9{4}fl^C~w5ug&hv}6sR5nhf$hIf^qF$O0GBZ)BK^DINWNJ_#UWHuGV4NME5AiZt?YoBP=BiI#B9*FH?n;lO0We|vq$pewcdKPj|fyo0c8wd16iQ7UvTJy66+L^~$h;=6^027*tr#Rn~Q6^1LKhkiDY8#3OpAU*_Y|$)3`gss(X{%wPur@%}WWG2A<~v`Q2#z+u0kjBLc!Nj_$wANxZ&u;2h4T{_K}tcW6NZC084-d<3k6R9Dh%+mB0(%2GU51wap2K7B^;c&J*dyvJemL)uvQwv290Mz_a`tt19&v{30}|3aUeIpd`_T>!uVs%@Cp1u1Y+~Sq#~yWUvQT`QdD+5$5zy^VT5mBA>kE9yk^Q3W*`7i8HvX%B=sJS&KPn|_!Q29D5Nf?&?jHg)7yWa@($KE0G^0hpit&dZa?{1j4VKC<D4>)oz3{VJF5!VKLZ^G-LIl?gzG#2`k;o}>;S1U2{TwEGw3tJOAh#u5cW7Ex#s6U!zrCJ4TUa^TLKDUhmj&-Dzu)bJnuTq3&HRstZKon5Eq(ig8s)q&{W@`e`uR1HzR_BRc@?>EQ#lA>~mt|*W4f#0&;lgNTM9u=n9Aw&*DXn=(P=f<iNe4`h-@Yj}iH>=?QWeaH0(2+yBV;8icI{U}k~~h;AHQB*5u6kdP8Y6Vc9ij5|2sZ=(muWCgv^3d6*m3D6Z%#}w!%Az})Uyu#q!JciK{cEzS9Wj=uZ*VHV5d~|ICrAuMwqD0k3QHJ$8@<XW%+XAjvl<m?bFc<0h8zIU2JKcU~Flx6)=bPs{yB&ul*Xn2jTzkvo!nZLloOJi+!Zd?R5B7@;7pY<o3nZ<npP)YhfN;Ku(lx#g9N|BYP)PZFk<#Kc+3gxavKV}HhWvd>nPM4`uGp4?#)&M}Ul8tM&>h00jR90gIrBnMfdm{wHL4A=mmzII*_0gvsE1s%l$5_jsb`gEq)#Z1ePVz^23#7%z&H{@?n;sk*FHj$4A-)7ifKeRyy!{<xlU-Q^ITh$9TM6MIFnQM3MN8Gdwh4rqn1~kF^(vYw-94Gzx_9iI7MR>UuN*{Xn`UVe&9;VXi&(2DATZ<nz6$XZTZPLUrz~q3_&XlWt-SUuENMR%z!0qAwK^RI~@DxNfhuv1)v%bZy@d7=L6J6LjWT)EO1D%yvpR&P{^sG`X-vTWf1E9QUn<k3!`CkKPgBfF)?Sf4fZc~Pz*F7;9M=qnp()4Wa<%^QH~VGca)3wNaBF-R3b;neq{w^e4Pl$aY-1@=x$l$bXX&VS4fd0?tqJOi!67w-f8u^NKz8V%!fLYC{AFtO-T5X`4jb6QXWws1}=_F9(XV1s9IxhW}F6-j7t$*8d#JbX7Pe_7S4r{FGCM_p8!gAkkP>Kk(t9ZK(nJ;<ucg(U^(2v0<S@^)4YxsDcZeAX?G~<l`0F5VZy6}SI}D4HWbB>)&j!k3TA<1NmH|^2Ivz|DHWEpJU#ds>TV#Jp@VSI;tEc#N9Ii70fPP_lJXYhw*-SPVZikxy7)+Gl}X6Oa7r>&z!TJh34vImYhh?h#%^J4<2+7gs57C;z=y8}2+}#4F+x@d)bCQ8P|-<a66JG0rHf9YK+pnbl&B*My*_2mv2&_DGjCD?z@2HgBAtOsa;DvKgrDt8i}0O&{m>bL?aaJN2)7xihl!4(84qCq?EzPTjS8SsoM>TxK*U0-5dH#HF6WM>_N&N9D(7zXm&S7Y2`^p5r3;|E{RI4muL91GachR!79uJ@q=HZ+rHM1(@gg?>lfueiwY=pAYJz}LA6DdrpNuf}YK`<T67<vuwE!56Gk;1i5JZ}^L(ar^DJg1qqXm%}@L;0rwf_%KK%pu"""


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
        prefix="galactic-mvp024-", dir=root.parent
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
                    "Le patch MVP-024 ne s'applique pas proprement dans le worktree."
                )
            base.run(
                ("git", "apply", "--binary", "-"),
                cwd=worktree,
                input_bytes=patch,
            )

            if run_checks:
                validation_env = os.environ.copy()
                validation_env.setdefault(
                    "CARGO_TARGET_DIR", str(root / "target" / "mvp024-validation")
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
    parent = root / ".mvp024-backup"
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
            "Prépare MVP-024 : analyse planétaire déterministe, rapports "
            "persistants et évaluation complète de colonisabilité."
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
            print("MVP-024 est déjà appliqué ; aucune modification nécessaire.")
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
            prefix="galactic-mvp024-verify-", dir=root.parent
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

        print("MVP-024 appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=19, SAVE_VERSION=20, "
            "RULESET_SCHEMA_VERSION=6"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
