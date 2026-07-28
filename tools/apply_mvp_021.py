#!/usr/bin/env python3
"""Apply Galactic MVP-021 safely from the exact pushed baseline.

This migration introduces deterministic fleet travel and a generic mission
state machine, with exact save/resume semantics. Dry-runs are deliberately cheap:
Cargo checks only run during a real application or when explicitly requested.
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


MIGRATION = "MVP-021"
BASELINE_SHA = "792e9aadfb7a16f47d2065eebfd948bf4c8e26c8"
PATCH_SHA256 = "ed1bae6bc934926c6924b91264bba35a7f4773ab832aba49eea89e8bb1785b2a"

MODIFIED_BLOBS = {
    "README.md": "14e749999485f4f2eaf4121d16b8d1a8419a7659",
    "crates/galactic_client/src/lib.rs": "4f7a4009c83103bc39aaff706707a7a4eaf3bcd9",
    "crates/galactic_persistence/src/lib.rs": "b5863b614923ba5d3554374420a5ad8ab089c132",
    "crates/galactic_sim/src/command.rs": "a8426db02eb3ccf58e8cd23e28ca8473c07a083d",
    "crates/galactic_sim/src/event.rs": "6e59d09c95effff032b86fb420ffa95dc741051a",
    "crates/galactic_sim/src/lib.rs": "adcdee0782928ca2828bc5fa1e5fdda9ece06f31",
    "crates/galactic_sim/src/simulation.rs": "3172a00a061450585bd5035e9f54985c00a8223d",
    "crates/galactic_sim/src/state.rs": "be697d71973a0dc209be64a098d0f008a43687e7",
    "docs/mvp_architecture.md": "82607c277e53f1a4f7f3b21b8757fa93376e3bab",
}

DEPENDENCY_BLOBS = {
    "tools/apply_mvp_016_b.py": "1557ff3f419abbf6a1b58b897100aa72da80bd38",
}

CREATED_PATHS = ("crates/galactic_sim/src/mission.rs",)
EXPECTED_PATHS = frozenset((*MODIFIED_BLOBS, *CREATED_PATHS))

TARGETED_CHECK_COMMANDS = (
    ("cargo", "fmt", "--all", "--", "--check"),
    (
        "cargo",
        "check",
        "-p",
        "galactic_sim",
        "-p",
        "galactic_persistence",
        "--all-targets",
        "--all-features",
    ),
    (
        "cargo",
        "check",
        "-p",
        "galactic_client",
        "--lib",
        "--all-features",
    ),
    (
        "cargo",
        "clippy",
        "-p",
        "galactic_sim",
        "-p",
        "galactic_persistence",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ),
    (
        "cargo",
        "clippy",
        "-p",
        "galactic_client",
        "--lib",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ),
    ("cargo", "test", "-p", "galactic_sim", "-p", "galactic_persistence"),
)

FULL_CHECK_COMMANDS = (
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
    ("cargo", "build", "--workspace", "--release"),
)

# zlib-compressed, Base85-encoded binary Git patch.

PATCH_B85 = """c-rlKU31$ww&1&d1(K;sOKn-U{Fx}7OuHRVcdC<4uI=vG*;I<ADahu8A~huCv}f$9`?Npc)~(vww|%_Xzp#(_C-Y189DES~0g95HzB9KrH8pLKAaHPSaK3RcjwchczJ3vxB-}VYI(YH=XfPd<^Xl(ToQ~<QWVl23cb+^M4Cr<g?K~Ni;pXPv&Q522eO>>uv$nQoeEsdWWPN+9zfacS|DO=}8HM>pM$YGPGKP_Jn$WPI9kNc|(!7X^GE5^nB4<$^mbBQo2$L`><0zQ%w-nx<bx4Q2qGX&+!#JgVl7ys)r}HE%<18h##0RsgH<ChII-@X=v?Q~zfM4W0dUZX3UJ7zGr$uLdM(0=fnwB{wIgN5kDK?zV<ANk#(K0N_l$J5gNfD+6iL$hy`4#*ee|2`0mT`G~Ht4L~OE);VX;#vC-dO`sr*To>#G){X<_RSU^jPNMpJ`e234EGofK2gxl4K>|4UsU*vx}S-1ts$|{>z+B0j|zkG0%AiarB`e1@u#X{&a!wivi;GMHbDmCC6oy&GQnMW1L0B#`J0ygn4usmozHpIfVs2!;i)~71;|HB{6ijQRLA^5}yz90_bC58$oqDll^dKb8|4*dotb`?t~`QE$)R=<KlKmr8`?Y+x<PV#{RptgQPkEV#3u7lJJ}+UH12gd>>Bf5&qdD>(9wanaAk``CEs`f9m~@Fp1IV4+*8^VNMxBkLjNQb#&a##UDNL?799i{+nd^G%O!<AM!tN9e@h>$^uPI=Edhvl#IyVM!&y(*e9~90a8NVtNOa_ta)I1%_-_tI8UQXfF==tFlglP)z|p%p`=0_6*gEJArL@)g@D6sE_)n&fC2l}&k}Y&Ew#qIemf|Am|JsVP6E3aTWhVcb(+J5jd3{uic<Y(jWZ4c(u@mXRfhxdRS*N>w8xV?n;H+wY$-e*(^-~t4B{V6aOdF+n-B;5Hr%{317`6ko0m~GrAxthi2OQ98uyBO#ltj}0zhM#ORc~B&+Q_XrpcmazUFQynA`d8<Gt<ky}@7{oo^1uyUona)0LaUc^V^|8}@g|8vft6-*(nFHpuH=-mY(MZH}bC1U>`&iew*C_<b4A`XB?Qvy77*Y~(Bo`CnX)hv}GviF^tIP<1)9x&&E;z+I`ufd^$gr3r}eg7w4c2F1`EgqE5N7xSZ$;F5<~lBL%#fzFNq#Ea|ibELUHVB0Lk$Bg0PA8&9DeexEV6@PF{3y^0KeMQF?_{RKwgapy&gUC1VUy#mX#`57wnMEJ^<m4JKJjH&0PGe+*^e{^&@kR9;-(64u0#+xOi-&!-414|IHd$l;4?lhLHJ*ctqmb|&s!t$yUV%(X0FfLiCm^*c{zyTt&ly7gWlrb#C=Vy)%M|xpmS2nC;@yjQmcWpa_?!<Q@cAMvLxtQQ87pV#J%AOwNKsD^4-R!AVNYHuN?|`m=j(Fs3t0K${Fr7R6I=xOK%)9>o=xHeALrSa&*+#Udd9GRyfeoezLL0J6x9_m)Cs7XtlcS>LF7c~jQZaUwSpGd04Jy;(Tg}bO!&GRzr|SU;b~mahBmEJp!Aa&rDM_Qm`=iZQU*CFbOi!4Q_~25X@TB$888)1uuKhQ20K?jm%}IT)P>ah!6)zEiPU?+C-1*P>N5Bww@-n=ug~V^O#3d$$-ys2!7oS0CokXpG$QluE%J;Ecb|eX-`r#m`ys!32N;5_-F-pN=dk%<7HE^a4f8S%lcT@%-ggLg#J40l39l$FCI~X7b?!5rbD!HrTD#ZtGCWUMQ~X5gh#VvsBBsTN{6eE|#MXy4&-+yy&T@GRzW=NSDgCt!SSyg(Fr~d6fmD1cIW+%Xr?BT*E7nCN_&uEjf~^~UapKkC$pvwZEHdNb7*Vw~e2lo-+8S2K0u8-L!&z~e!DK)`3MxCWTm3mAk1A5+T6?t$#Qt~Nt3Mk6&XL-?y}tR<LSun24~&PFzEq)>x0SyK5z<Y!XEm?kSQ|mpxtwEEm^X%!kE;=hsnE%V;9tq#j^?+0sWUM%e|HzxXM4|FANU?*4i(EI7OPvp0^qWUbzqAE3lCfP8-p#wd&UuJMv}cs&K0s(2YMScFOjx9RpW*=s;`eVhI5hX3sf7dVU@HMsy=I2rh+lg;Dh9z#&`Bh$M-WPLOkEwFRSkvP3#Z*kAWs0Z^D1z!898a&{WF;my?g+)O}2i>Zzlp(1z9}kWwS!N($3&kbf$L&`?mmKX`o<oSYt<DyH6N680bxWCw~bupf{-)c~h7x=exBUNGHL%A0dBYyiqm2rsoozE_9@-(-DNLx`tuawfUOueX<BLG?Mb2AyxLr~1kib5P<(qpzQ`<=ln6y-(J5_c!}{`?d8H<`e(pckl4|dyYS3(?~4=sQCdBPk^h7TPgtSExHWQvjkR1w<qm8_<~ytLA4)>`IHu{3p7a*TH>!VC@(4WIi*a#S3NK<1XwcDd8vB`pJ`gjXq3{AU6a;EBY>sb>peAlVKVm`b}@8N^~U!=w>OX?`Q7IGK|Hp;&GPsnP8WYDwqosj?lUpJm(~Ph6F@|u)kAM7jV~_GGxSIWytksuNAJM^&M85+k4zMC*aCQH!l!-CKQ)EUK+;oTJqL9xNYGB{>N{dII(+lf$?5UWho?a7!Qs(Qr$@(qvel~%Q+4d^>h;m-!K>i)%b$*%J=_)0<3F8X|40Ar^!VWL^v%j(p8W9g?Vk>gU-Ze)@jYecEUbrbgTiv~Ho*nhyul>P`6xx#hF~EIa$yhHD2Q@8j?1oL>GEAN3^q460dVa<y`J?o{dI;C_#y6mGMT3c3z@`a@lXfILupcgk!NPFhB`PJF*QQ&|8e)7&f5UPZx2pi!cx6BIz3{9_1-gbIGZKc0SK>ang`sx?N<DDG-9?f<Jq>yNpS*8>53$SAL+b)G<x;&`yWn&x5sb3JJP<m^}#j1LKj%~#XJ`*it9}@D!RLy4z{FZh>bT}#>k#fBoy6uN<8r8^@0qf`rcY+W39o?I;RF4aY3%aBpyGsfdw>Yxtk4&g_!V_uqTPmb2_#sXsQC}2;>S~*G*u3K7L<YKppJGbL5V(C`<|dfZ=F;ieowhyKfHaZdiI4J{q}$XrAX7Sl}XSH4^21aUt}(x++j}k}3^YpmN|bPPnYFJ=3<!9{-_Rf23<AqtRQ$lGRMdhe7?TkiYR!SQIob1NxT-T?dw>#*Sij#`PgxI~tAN%**o(I23kiE+MKXxO6_V@=^7-z4|Ceg}H-zg4^<Wgah;xZJ9n%K6+d*%7igsLq7D(S`H-V+;>o$IlGp49_|^4R^znz=1zdN_;m5Q9!kmJ>Cp~excisJ7Y>s}Y_pC{3N{>r=%eCnbboWGTZa;eTZtN}6|XI!6%UzCOAH!aMV}?BQ~82;2()Ek1|Zrg%*7oGsW%dFknr)&*53GhFo=d*yZif-<~WF>8+Q!E(U8YK_AvhOgvCGHe)lTEphXp`D1!9+gQ2vxL<fRSy?qf54x0w_v6S^_c}q=uPkBitz;Xyqg-Z=&8k|kT+8)}@8aFIAHa5rsH||(`4igVhxFZ1)Fy%mGlr*2lX<PtKbymT?l?f&2Xxi-$H>IPArt_(0FzXve$@@_iJJR1)8d8129qH#{v++w@x#@ZQ^0f?`);&IPB{db2q#F%Ub4p?GJj}1T4#oGlK-N4h<AmXeD<o28Bw02?nGuf1z{+@E%o%;aIW~9H)3N402aeQgN3)cN=KU<2h~2zLcb|+mp9}{3ThZh2$!@#dtU77l%c>cp0_IubzDD;xT(o_Ou^Y7Lq8<a`QI<o_MR-h_Z3R8brlgJ|_$G->e<4HLV!!F1tH7N6QMIiyNmLlgnOxE*YNZb^!xW=H(umSVl8@wI^0(^6ihfD4U*GoH9$yBghLFE{Y36>20j%s5Uqr0R*Jv|;-gONL%gtXn&paWa!)$ilhn>l+do4oLCoD?D!oph^p5NQ9Jm`wCsbmSs+YIXH8o#9XR;QS*{-fSNPq=VC?YXhvwSGIBsh*urtxt?Ro7yw0u}W$&ep`*4StSkcdhHpcH}%78AXS}lG^TWOFnGMXxwX5w-Htgnrg~V7cy7TSrWWi=5<(~KSO)o@cGNS*O|{?5DAax{pS}o%;xi$0v{qH1ReLC%Ir<|WndOe;EtYDxwNn9!tXKib9M0-7&$}}oF;LA%Rd@m)ugS*(V6$xnv-w;_R?Mdya`vzlQHU{{38QM%0j-^2SRV02B~5UO+2sE~oNf<yCgUweDjQ>ZwUN$~WC2ZRLm<*O`<qPl-DAl=My8IbbOC?6q6v%pr0BxK<Q$gA0`HWeik1t3#k!1BI^c?BQI1C=5r<<zG*Dwtb4p)_vp!*eonUDG7aF}lz3-M;jGo+=H8a=8mEWUj)FRbKOV`o!eGapttQdSP<)_NzVM^+nOG`eOmzP-{|CRMEK3kXxqqnQlVX9OPktMNE!bpqmSo!)Dq)eSY$28ukg#9BDo})YilXpzcuSqEUR+B6zz)?w_&0*axvl+|P+TcCBh_7PitSv7=kNlYBAMn$&D4)j#4GI))XXHAj$ylTeBWyf{ctHLbmzSuO@~(vQny5sKki42lah6D^3I@g{s;nu}y2wQCf-YasC`)08uq3|@djmZI<MqoE42lJ(#|OV0y$b&L=J?0p?a^`Y!<)A(?||iFpi}y(>#O6VlQ*w^W`Ty&mxn)|s81LU8^yIPrPq{D)|_8b)7M>KOk^>Ig#Z2_%&(}TLq^&0U$OD+-C<zPj|vEMy6Jg1(%B_{rYEtYzGGU>bKK<$FsUKw7EMxVcVWzbFtB52>;$4(g9&<s1I}K;q+8Hr!lM1>S(X?U61qHS@qo2p*+mo0okMQq6A6!c!XfG@Ps@t<lzBG}KdMToVdk6UKh-<#HXZ4FM84wAQ|GIi3srlk`%Om{J7}I^i@BjKq)&~p1fHHF!PL`yV5=6eVTOD9w3vhl#?lq&U}9STz;ly1OT}vU!DJ47N5Hg4E*8!(E+{$(bHG8EIMGy%k0c&6<J4;0)bAb9&v?I%f0s{yO%jFLNXRyw+Q#_UaK+fP7&x6Csu4=|kxmZ!JLX6=lL@T17(_y)yAhFW5%WqQ!$fBI-){^d{k(rq8&=vMI%1ogE{pbb)9!58)b~bYi}%7kqD>k;Z5bZRombcq$+GtYP-W>YwUT)@_0nva`Cn<a=F)q;rEumZ=^K6;g8Xe2F?&~}au9xttsxG5jGU9#+f`AOdqcW8XP05W3<mC%#&vpNT}eoOPSI{n8IrpK)zW(*-dX<U>it0!(W2<qO_QGPiTiF|T<dz4Z)IC}sjE(Z%E}j6^g;THYB0ZHQm88#r1h33jMv`sk6>J;3hFTPU@0BfSS~*TG*9bqB)BJNZLwFi@m@$?{W5(U7Dl3nh|jy(GLIaJz=gYs9Q~(^FA@fFkD2AA4dPK7tC+iYQZLhDKAFT3o?|>lZ>rucf)?Y}vK2QL3cUixg7KZ&+p2FCd9ZESuG6^fRmKkJgHn}y1J>#!$v#So0TG5W%X+K`%z7@xT+wwH>sp{C%}Tz<4<=w;sKu=B6EUN@WMS0flN@Xr+>+z=ei#8$1PyNRMIgoh9!+QEH6uWYWc>S!Y&h04-{JS%v)~?V0MC|YC`d1ATaDw+wb3%~a6e?TLx5+&EX?gCCg9ywr{chywb#T3F(-jf_48*E33BZE`_*W&%RF5@Q%0pg7(C=4r|l-noJR31W~{Pu(P`*aAB`Mg8H;cXC8b5@JW0b+HS8@aP9dPhBU6b*5(x8Pj>se1A1705;g!?z#@HyPlGUMdn`ZjqhHmB3)XcA5-%t+JDOb$Y;O@I;9NcR1JcX;vMpWrsS`ynJ`w(P#SML_n>NRb{`n31^ry5?gjzXbd&|n&-;RUNB&=t9K?CSwDRQ0i^;Vc0C$r_!DLN!i2Ve_-RsOl(iw^BxWFnvB!sM?uyg60%Cs*UJWfJ`Nb_v<}EW4yT4@_FR)7|z|%NN7d26<*Ci9+f3DDQNpiR0@{_SmjLJKCPfJ#&r-4e^CcdYa$cVME(IN&0{vOI8sK!<YRbU;K^#5;{jt5!d!mEz(IN;axGNK8Z5CfRF&dR9z5f_WgxQS+hD>kuS}lo8MAL69cR2+hoWJvVJea_h#tH8m%Tw;1SvcD(rPNEo^15gc3`d}t~jtz!Hu5k0~;&!Os<$XkI-f`P5W+|Bg^vDQeq|LHf75y@Qj?>Z_Iw$z}R307^Z;Uajgh~vIgofd5Ef-IS<J^1EC^Kg`l{^sCH2Xv#`AE7W}}iIzevIEHTM}^ZnWxAu-uTgG4t<JljGyPV{m0Ns7M}qsKX-wE<HE#b`7t^J|uG*kuzRBsA^z21Qsh>j6)$fKD>(x#Y=BaAVEKZFm?%N8v0yj}uTiXd&6K&JwlP-Ni!8V;<>Jjm`DBG43EoD~;J)t!T2*z;hUR`2W@_*_NIMoQ2fMqJhtv0&qGqFDc53U^-8_^SvFOBHYDsdW&?E7Aq;D4m@D^j6voYI~<R@#$@#{W;2~F16TFg2JsG85rql+GqTx=4m^X`xo6IzA!Q!V0=d;p^m`U*Yti&=<K0*gbWX!i`t)@T6<yaenj=J>;VrQ53=H#>14O#3oM%OE_6htC+f<3_Ok=7?l5j=>MqDmNP!6_kEU5vBBjDuhvp#g+8?tr{&<wHz3J1)>_4c9LwH;G|>!gz-gs6yEb1%q$4!*QFbSMrMFc`ZAz%{YkCa`F9=S8or97<xcPsljd%m%8Cc4rG`4@!9UO#77=(DGQYc<$skwPJl4n04#5W@_}ADUSA#VUpKW9ARKGSPa^?PSrb%9IJ&#L<|=(o0@;p0K<SlHRZg<dHla;>|f=!@fvCD)LI)uO=e^XHT`2)5M{eP)IV?>`W$dbXcre_poo648l;@G22z+KU<F@=Jr(Zr@Xv&e<8fE;{yFmQYdp>E@T`@?LWgZor&#NW(OMOon9YmJt}ADNX%t{;IDHhEmMx$*bH5dD=UD-a66%F69BBb~<j+nuQcaB<0lzV)`%Y^K11H&(cC{z6^=;3Mu>y>y;QTuHK(D(u;^~b<(;(Ik`+>ak<PCV8Kb6bKq${ln4~+8HAIwS(3v4DcZJ^PHbmR`OC2Nu@S`vY|(G@~XX%n%y$`aDXBT0PmM8hNMNY}TLx&7jWC&|_sjwA`~QC!m8u<ul4%S9@vqHFuPonO_#2wXA)OD>p{`;N`y{R$3P3GvD<SlI#FwCI)d?(kLyOG8{v$BJpdNT#b;Vih*%`WcRW@>6C|XliX;)`}5qD@AM6=n9=_=#Z;9ofqieNX8^C3vLo=-97C2+_PL1s0FlMJ+&eLc59slX>F11)Y{=g4{a@}YgCx9v<`*@<_2z|a<G!m24i1A-|)-*CW&940GIt9_?rd)%W*<YW3tL>Y2(7QcmgZK(!dwYt@_MI|E|`WG=>i~Fs;V+W`M>&fhJ)3U}PBZnLlUCFGXuXY@ZfkN3^vWIs=SC)u<oWzBLqY7Gr$oPK;V8`z=|ot5r>TPj1ltkh%+jF?~?f?tNOB?WHC+#amsPt0e3uCM(XP`w|Qs$pwZ4Z<5e#Sma1Dy}Lp{S5!34bS<zw!&fC7OzZlFtRgd3@54wx_jaIC@a;*kT>MqPZaFs3_qV=PiT|s8-frmXbb+@{!>T<P0BAuWPH@_e#!Vu<ytlQm>ygjHuQN3wDV1r;`Z)ukFxNI0FDPL!S5TdeTCMH;G@gxC<qjFUjaf{SI4`igcO?RteZYb(>>aD?HH+!E+iMYqu7(<osyre)`V5$7eWMC#Y9DO^y!sdmQj^S3uWKlGoR>kjAef(o%zIo{5)p&C3WFeOzwtZ6wvgy`Xdr2pu2V##Jc<<-B;+H*Hk<0PbM~-?&o<%L8b19ASxxwKmi`g=MBp0GiRaC#Xg{p~qdGMj4vl)4`)l>RC&;~c?v9i(tyjH9%xw*NOgcos9lPiw2><M3(cS7<Q+S}}z#0Zjqb$1E)w|92eb(o@;rk_L;mc|>l`1(%{XNQWF|zgc){g>ac`#SEGj{91%4&gJeWF&DoNlLTpggcR+v2FIK1NeopL;K;K|G8_LDrl;VsvLkL6E-xO7l$T7&kW->D8khZ9Y@J9=maCjM=MF=bI8|feep0nJD8a+Hc*#r9Fnh=Cs@Mxa}LU$YVS`-M&(w*>M>Fu?Dto`C(l05-9oxJH272RQ-Us)<y!;1dZ_4>9E*vt_+h8Ihuf}uN&+wrb8G#If7ntl}y6(Biq@pBUIL|Ux~J+UcIp16W!Z8y4q(`T5{Mn+w96H2YIwadmeLFjhCH`452s7hPxM<Jb{lMc|8Vg)6pFA%}Quq1mS}_Kv*EvECHc6>)HZR+8WX}kh#@Z&X^Ztk`_YI!mF-3jq2x2m}Q{V)Yo#h4VwUaW+gOGh(miGqm+Q7ro+AHpz3T6?26}1rW+-dvDlO)GMDge>z{O47)+G$tGp4gV|&tJU@f^r_wkbe5R8!v_}o^c&HRk#uHGFUiOLrD_vR=jdb<d8*>4lvK5o1O^um~5)Z<DsV0|xy-<ciL7Jkhgu&C2jrl7iSJBwA;Kr2Gk+-18N1UT|}PD48@_13ZHSed<N=C*q3-OXmu!6;0=Y;eL^5Zz_HJKC!P=q~Hj+y63Q7X52yz^EogT*Hd&hnnVV<}Em$i@L|%rXs|($c0M9zr<ReArYN$FpR3PZGuB)>n3M0_k~rwy9sE@WRk|*3m$2iWJ!{JWVH;P1%+zhb7@cwx>yt_e(K+sRtU*;=}U{?So;-|gwMSK51ab%6lk}=n@;0WkE3Z7t1CGuQha!5@?mc^$nCHu;e!{og{tv}dUU={{!O8~Wq@SkN$h$6vyS0Bvrq86Ox1w}c2wc%>XLAE%KI`HY3QN_KEwfor*{&oh?0fPtExYK4E|T9ud16?@#m$NL)VgTd?B&g8y;|JSdw`PN=S5xXAbZ}&Uu;P?X{7*h?sG}G`SYszhLY?%~)M1f(Kn5E>?JMZArCc4BZOzs#tm@S(+M}VRXY}u|5O!Q4>}*V_FF(2P)ge;yw6TE=1oL^wz!09;dQqz&RS&p?L#e3v^X(uo^j#ntjU-Ml{SvJ2-6TL)M{da*4NC%8FSz0BM(V(0VC6xd`^f44zkuaulqRG_}w5k}XYZ=~~aF(i)Z_gl25EAcX29xoa9d|8_IOB^*xS`C+$Lc@Fd4{qVV`Ziu612y+pMvRg)Zx0-_}9RE3tXd2<6VOg_tH2S8x<5rgM)cUA9l6X`v7RvnKD&`1akBhFHo8eDV0}lXy`L#F`h&-nU21+2xNp^dKIK@jG#QQQc?sstZ0c{$3U?bE#j1Q`=qAV>#_=IH>KN7+k{j%ORLP2tP6*azXoUKF5{{`z$bI%;BiYg9mz2hUkH*&?GIpUOQMruVCd;q4vRh-QW-@>Z(G?(+^B^IPrIR(NOf{r-`@L-nBFc6rQ^&8jZEo3-kg)2O6{7825`=@%%j9h(!v{QZlw;CgUAf_+Z@@v_+=i&gkJaIZ-ATlL_YJK*l2go52R3j7!^@x1!Rv2-__liaj-tp$|pZc4_xCOf^$?4*#mPfQ>XX))E3+oIs@9%3hS-BkW+*pm;i7w81May>DCjSmb!rk6=eI)h;Pw>~%Nj$v@ngd9J^Dz3Lt-CjVjeD+Q$F<XOE}uH7PLa*`c8oho&34XgHx`X+4qnRst&n)Pev{0Kr&3i7m}qj*MVW&&Ui3>E*VM(1SgRTDUmuNRFUkw#F3D_Ne{Nngb8CET)=pN%>Km@V)ULWz7X`4(&5esJYd1^_{vmGu($XlkiK-hc*Q#rGR6oJTxV#M5eH!BSFt+Sn{SsPP;_S=ZEV=TnXe-`Gx%i?->sG&5UK?*cM}ls;HtrwvsycCvTbm2()Wvh;o^O|1@@6@0%zo_(x1Oa}OezVAIXGN`G+%}-Gyt2v?{aVH9j_`~aBWtYB<yl5X-lBdQv|Tv*=4YzRDHne*W;ldqY@ZwHs%M1nY}YtW#MghMgzOPx3iI1^xN4)s{}6Jzjp;#4I)sBZ>)}knKeaUR#?_rM8-jCo(uA-;m0gW(sW_r0z224=IOnvu)!)ekKMJrt<mARaI99~Iq2*#i)+L78yh+IQ@>5WmOyc0c3d-(eY{KF2qCr^vv_BqWT4u2jaN%3zZ_!!VXuZK?OMKihH{@PhmWj+LCxRtJY4yyRo2M&Da+h5<<;#1{v%G~%g9Q=Ty>b8h2&B&CM1;!i>G!?rD3(5d>6Ih-Y%nVzw_Jf`u~heNk^_*9JloCbX&V2G?7P!pSjC2G;3NGFb+4*XvDS_u8CWNv@88E&BI2TxsjVrzoX*8C-TsHiD&c7fOa^QDryEU^)Uy_rl*B4`VOtnb&g)*cc7gZbzG{sEQC6BtMIBUfB=?ZR)$G!c@!*F0$x~vW|~=&pbpf7uH(p)JZAr&AP!tq;$`{P@5aiCWH2DN1}+v}dm8gxL@lczhzpniuhJa=fhKVjm&vusK)tJdKL?1_k)(yQZ>-b1{87h-Baa@R{OwY=%+C2K2=0evaw^v9iv_$G*g49sDuK!&y8fBP)@3{%(=<@htLnzmZsc~db8$q^g>4*LRRxqw=Z3ex5r<#~wU{#~I_X$81AuSH1IOGjSCzLA&UEAaibpfUxGL&jELvF*Toi!-0Zmaebp?LDjF=vo=>Gyz`@bI@zp-TYLZ<N##)tJ4SehL#EbG|9Y-(Pm8pm<?!Z6Ooxyh=|unKq3!wQ;_)t%ZZ1-p)2c4KXuF@>!dyAts3f76TG@kS#Sj|-kZH#lVN(3~~_Ye6++8-6^QW-h)|w<0<y{84U47L5TsWpnYL>7o_;)o$o8sOsF@p2f0O$UnrQa!$@+x^d?txS>=%fDIbR+xxn*uItyrfc1PMjxnG-@7|yhdm}54ZuRWFyFgG}fU15;WHgh6VyfhvUT5i8_%zpx%XmiET0QiNb;o+P_hK&~2Ghalq5pgEV;{bG_2#EP1t&keeEXaEv8^#1{n(<q_Y#h5ElFGVWV=zgqAS~k%JgO1`=y>5&Y8bf)3$ct)IKol-%;+}<lRha28FmMU2>NgtDK93wuik1#x!mtywp<N!8yTdKrn^2HuWNPq`o=&%d&Kz>aoZb+am4At6Ma>Wj*Sj!zI3|Ca758(kj)$63m<oHIj8<+$zGnbtgM6(yp`$&ME9Js-1Wdb(J)4<p1|U{?qyGe)FhE3(KpO&vdw13yj>={V3xoA~}#1nl|bZE(pcC{D|H-Y?To$WLG2;q5TY;12zzOmHkDvpIkn|WxSRS(baHFq~_UlG;x#<Z~?iDyBE5Q#|iB&b^^*(k?G;+0>oVH8HN8#OD6T<|3Xl4*`9Lqu?dzTS($?|PKYOwp0SQ(-BTaGVr}k?Q(<oomdbNUs*IS?h}S5!?g9MA4WZc17{kIv42H8=avcZ}A&$QB|HuW<Kf(2B_{7|uXh8?!DD&2EvxXlwJU5ZMR)O$ooFs7pjBK;#F<s5hYZf_!)7^JLqXJ8znEg7LicVvF-l_QI349ntsZKorQHqn*J}-A0ADPj&chAeiF0KHN|KQ8AAsyE|WfnP3H#067wFGL5T=(UHa7UEt!6E!-WY2MtxwpI7C)-=QeX_N+?NJ18JL7mVA?xcGF))V>mRMJ8sN_80bsI$<ZD<@iFUWbjb{&51e|xyU6>b7a)7_n|-SY{79k92v(^+3%Z#!aVZEdafsNa4|)`$JAEwTpxzfHdVwzIymfo_QP&Eb=gJjS{XW;sd)I%Y==L7gr7vWx-z01m{jl*fywBCxt<Nj43m>p^EtcD6MfS*^5WJ?0aaCuZdUtYewhph_Tgs(eY|y~D?t+f!VgU~;ZMpOSPuxs;uCmK3S(f~yKAt6N#cz25|{THp^H*J1XFm6cf~|FaQHJ+w;Ne4)H!@;=Q*P^#L^D?uf0VpS>UZJ<IQzCfSKCS{xByLp_9L5v=Tpw(m-V!%Vbg#4mkX4D*F;Y+*_7=HZ=rVq=O=ys`Udk93mkOU-Jso(r?hQtcH?U>_5>w<iL@cJk?IXyT%3Vt~{#tSBllvX|jJ`iB?x9kLq_9ySWIuTOKgvGieVIE!T(66lqeHC}FPlxxpU{1?Vhj(kwseY!%lvs|6hi*hL;y9sVfAV^Fupcl42GnvA(KO8Cto}rFS-Uo*Pq6UM3Etu(I+tYn64|Es%yCFZReBNrvq&Sv`b(rnK_K!~E@sy!;_OOrE@(c8IMD+qOj$~N^=3Z1$ip$M-snBcXI0Ny(lin6IY6K-k0q=i$@1_5UWKzTipy)!n~JvUGYeQReeETVABcNB)KjiK@TXR95;*yGO~;Ak#(f8tOslW~YvQC<OVlmEn(0CieYZ#D3Rtv?rR%8l6d>*Uu`BrNQn$sT<bE>j?~%19Tm8Kuh;E|pG_JyZj6ut5jcb;PcI~)!PDePJN0l7v7(Rp-bIb?<z`d8Di<AuCeHl$Rn=_&?ZhNmWL+cvfpXjfp>O~*My4ZuABhg0@_hbfbE?PG)6%Z_ZnK=pdh9lj&nV}j4&qa>CXtjV~&i!RT@LV>3Yu*9QUNX(XRh98R2dU)VmmWzPK3`%Sb5(e)T)uOb8O6EnT=Vf4<6V6SBgE{zgIiRFlXx|2ueAllULCWSWv3;3)5wXAPcw5#j1vc*#awZq7hn(K&Aa6n7|Ld|+-k0K1>^M}!IT2q+r>hN(J5bJr2y)ex0?;QaZxpt#)fQoB=Q3V&KvqgiM6%6k0NV(xZU4+j4~?*+h3Dcx|A{wO>(n?2X*n#4Vy7fQ6zuUqoYiCAQEeR^xj(!?KqU*&Bt#(^OW1*6l+B>c<~FGOa?;uKVm<sJJq1woT^LJe38C4A5;b|8=jq6HMDw_sw*!~sy=w{iP>R%jiISsmlDrif1MVwJkHO2Q>*V2?_Ex=e^k>5T>ox8@0?h<Z9MR*(*+M8sO_#7XS;2r4=Y?!qYp_B`c3P8qHx|cp;pt6756%yYSlrso_qaeJK{`;j%5uAPX~7<%tI~sx-@yZJ~mcTV<LATA>tz;rrjTK%jra<IVgsS6K7;G##VvKAQSDh9I~Fq&Cq@@f+XhMH;T+W7vLag-rn9s&b+hvm^%X7a^~Bf#Z`M6ESA4sqL{f3(RW{~!&=)GqJ;r+ZDSbaNk#AensiIlt})7%_!267#cLP>a6R?1DT<!C+xf0T4y?jHo)7N4?R&j`JABF#Fxnhs(VZ`Y3mNyVUEp(X9IG~Wqwb7V9H%26vrV<PyN#P_f9G+3xcvlom3}{oxR=D{8TheLHKgK9n29A^-J=l;s7fD3*HtT6F9#zd{CN9=+19=`K^sn%;n1#aY`87K<#s?1yyY&?I`hQ#wDQvNc#wx5yFIM=>b}8qS$Gz?4ZQ9H=kN}CaXJ>N`Lww8LEr$_Ul)nKn{AY{%~{lE8Kn3;HZHrxJ0qWV=9{TmT$&edwZ>-d2(0|g93&n+YvCj<|B8nW9l~4;FZ?#$%9qo_W#V)(slU|9udfX80=Lsji(~qumaMepz(>%tOG#QO=KLlk)a!fdDn1^-c9$ux--NUZwru~{(s-&Y6E~QR^!?P>%DjRkr)~WbAs4YPoM$UX-5qAjfnA1EJuoUWBHi;)FQz(S`Ttr0%?FFGZZ>oK?}DRdKjUwQDC5!*tte@W>iYhgp>EE?>x5}elPdM&MTRAiy1pXZ_CdDWCP4JM&J9DVY!g?F@tS(&EZ=+6SjBcJ1bCp46$+M^epPOAmCCJH6szy$7&SsM&zQ*9H+GHN2i5g8y*uriCZG9lyJy&18gyET5&qjx&G#Wmd_Mi9v7mY2eE=^ZQ;bimtj9K&{w+fc&A0!3;rUyUi552u-Y&K&T3o8*sQJU|<5jT=k>TpC5bPqn_IPg-XIW_t>bl+2#dF1p8Y{I@`ZMdQa3?=I4*Y4y`LyqEh;e-oyBywH6{~vjKj}IE!Rj-`au>DR_EaH^)hF0026s<0^cVF(4+TH1R{1B`@4`+rU!*pQZCTSeU1Z>>NqYO#A$vsEmDHI!##~9zv<_Uh7Nq79Eg??aHV%@U9Lcx{&7tVuaq>;U)Gsl8PYvK)cm$PGx~0!e716G_-h}_N=6RD}*bcZF1gcsx_jNXeh?e%oiKw#sn6xpLeQ3`wXU#oQ2fEb-vW+au8SK8FIUqe~ZYe$&bsAL*f>`N|Soa~N!V%(cButO0ZV0zu+9j4VSekRnMH7)ueoptE?2q>cgYBK+{?26FGM!v@vLKUOHWNwYLzYC2nP?s2N+RdJI}z70lfiS!mzrmNhjUohJpL;ed~MUVk5ZanTz{X(4Jq7<bEe@pmh2GukX9y4`@yUv?H1$m5%}QRxC>LSS?X@#NU;{#)i32qd+}SmTacwJl0f)FK>IN@3h>%F(TA6KC|7@gxB3>ftZ9x_^-v}TTRA53fyB_;Jez>g43AY3=P^Yp7*jj{SgzaMQ;!c5Er-7X%}nL5T?bH`n!&Kl{OQ*7=3Td#{Oa=-rvesC-Z^hUp&PK}e}Q-GVwnlQO>E9eDuCcypVBGTk>U4>q1u(hrWvrW@We^=>2QaH<8fgz4WSCMvnI@aMOgidFj3Bt&N^nlvh-a?w)}|9x3@r!4|ks8nG|ifT|S(B(8Oj?{Qh1BfE4rWPSM39?k=C&t0nTX1^5N4O~nal93Ux>+3xOPR1yCFiA?+DTfa*1mx79e$WKO%*afAWhp|S_`&A>3Vcw+L;(7}f_^R};Dw!5vO1zEqOHgGOYepCDj9je~sTLW1@eQQIBa^5{HV05G?6Nz&LyM^OE@y1}*&kTD^f}LD8xD7H5r)Hkz6vZ|qK1E^T-dp18otF{4^{&U-J5;qRY+D3YmEU+Pag)p-N1NoEe=#h(hb@z1XCHXqeEc=)<JZc0>xbb{Rtx<EZJEMZF}}0&gF8$<6J1!lqDK&Kf&eM+S%=IZ7yD)0WeyG?uKFfhNqxJSOg?Y!wcGFzpaYBdQhj=vtsvbPpNa&U6%dVCq}-fej&^wW6S7jF&nuu1UL%0S42F$(ado4hSbZDo4~M>`6|(-2`Fr{1Yn}-&uYL4<W`wL*gu1Yt=!8Ji#cTZH!rak(lgQ$VQuP4$)#5iw_1E7gxBj?#)FuYP0~SvYLKJ`N6Iyuu&FfZq{=g+tWG;lSDIl_fY2;9ut+;@U1$*Daq+=)4BEZ#m9QWlZ|!aFM_Yrz)_5y?Ot(x6!uv6|9K5d?0|n!tzYQD||GUHX=oQMQGK_Odm<3GEzCZfu==k6i1gaFQXXNw$CfkISC`z;W6-HUWb|*k}phnN~I4wbTcS!#E)8z9fXcN%^c|}oq!16IE{8fbT*+rO-;qNge<IkU<muxnp%u!TiEH@3A8mJbazu>9Uv$}*o>yxvSgI}uO2-Df|&##V7j!uJ<!yk@bA5`yvgO<>H2miq64Cn%EG?)ws64XQB>)ioviq6{aekV`OlE44!zmwNlN#{9&3TnuoF^E{oyIINjtIwZc3lN-3Mf9DuSD=3J&+tI)JQWqe5mf~#uV?W8XRMe2PD7Q?#~-3hK2E2zb|%aJz?%XZfm#w}usrnF&z~YNRdVPL#sFj&Su$si(OI6&V3k2v#Dw0Qt9zKhMUt?yHc7G)HPz%R?oY-)LlPmL`?!n<8qQRvD`BY^%C2(}ONF%#7@A&%Obo=6SVX>H_#_592`2m9A5O5X(%@j)@iZ<!e*)X`SAHc<f#n_MNIkd?owe$YFdzz?Y4Z1f{a+Hd1V@cwQb$N_Ii1YeIFtiUB3O*ipJ<;w;@1m>=LK}jAAzO_xP=BplzIhBidTf=gTO#oPU`c20KT$0!v1HVjBqjmQh^VFf<wFz2m1h}7tjtX)LDzdB$_Aq5a1(c5sj1Xa4*>0+$5j>Cz3hn%p2qic3x3nb)q23Gteb*l-?Qc3Rs)Z|Cj9-=qo>;0}NQjY<>k`Bww+S)^l2dHNsGSFo(s04aJYDvWd%&03JpKG`~VDm)SJ_%bb%q$7U$?P#{^c^qn<U2uR`pyST!{A+T%{HtC-ML1CK83{)&e5;8ghB4d=mpC|D-LtKKfhcTW1Iv@vgNh^%L1Y{yG1LMHU^O!BkY#uY{)YZrH7^C!2GB3m)?X0mq10-0Wo*lwx7dW|sj1Vbjdfe#m|Ks0?3W~EIJbaCRRv~NrrDDfte7hFL#mr|afXvwnj~btG<|JLN@D1A@#;2V%QP3F~9kyoF3NtN;Eal8*H6UQL!z}b0D%l}Pm_F<v;EA6<rF;RwX2Mm9QzT6w3vETj2jLu$ho9x)jQKjzEaIfVS6^@`VG$14?#;Qc1K|f|0viaYT(I>L)iSJ=<875&md$;RZYnXVn6P5+@F}MuSfCs^14kTCV3tMzGnsz=KU2Y60246FuLNJK?uQN;v>aXvvr0o49s%uFi05OEahyWLz$ENI^GMlcMgN5;bSwk{xx~xW;M<4_J`e%}9PybE3Jd_JMaek~6kXDsO%OP)L=w;+C$TeuX#k4day83hEYASsd=;k5T<xq0s~1=w?4SQYR9*<WWt6<`5_8OkJwqB;H{~6^*pn~ZVh@_Yr(F=H=pXw0sW<5SKe`$*lK"""


def configure_shared_guards() -> None:
    base.BASELINE_SHA = BASELINE_SHA
    base.PATCH_SHA256 = PATCH_SHA256
    base.BASELINE_BLOBS = {**MODIFIED_BLOBS, **DEPENDENCY_BLOBS}
    base.CREATED_PATHS = CREATED_PATHS
    base.DELETED_PATHS = ()
    base.EXPECTED_PATHS = EXPECTED_PATHS
    base.PATCH_B85 = PATCH_B85


def selected_checks(*, full_checks: bool):
    return FULL_CHECK_COMMANDS if full_checks else TARGETED_CHECK_COMMANDS


def validated_patch(
    root: Path,
    embedded_patch: bytes,
    *,
    run_checks: bool,
    full_checks: bool,
) -> bytes:
    with tempfile.TemporaryDirectory(
        prefix="galactic-mvp021-", dir=root.parent
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
                    "Le patch MVP-021 ne s'applique pas proprement dans le worktree."
                )
            base.run(
                ("git", "apply", "--binary", "-"),
                cwd=worktree,
                input_bytes=embedded_patch,
            )

            if run_checks:
                validation_env = os.environ.copy()
                validation_env.setdefault("CARGO_TARGET_DIR", str(root / "target"))
                mode = "complets" if full_checks else "ciblés"
                print(f"Contrôles Cargo {mode}, avec réutilisation du cache :")
                for command in selected_checks(full_checks=full_checks):
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
    parent = root / "backups" / ".mvp021-backup"
    destination = parent / stamp
    counter = 1
    while destination.exists():
        destination = parent / f"{stamp}-{counter}"
        counter += 1
    destination.mkdir(parents=True)

    backed_up: list[str] = []
    for relative in sorted(MODIFIED_BLOBS):
        source = root / relative
        if not source.is_file():
            continue
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
            "Prépare MVP-021 : trajets déterministes, verrouillage des flottes et "
            "machine d’état générique des missions."
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
        help="lance aussi les contrôles Cargo ciblés pendant un dry-run",
    )
    parser.add_argument(
        "--full-checks",
        action="store_true",
        help="remplace les contrôles ciblés par ceux de tout le workspace",
    )
    parser.add_argument(
        "--skip-checks",
        action="store_true",
        help="ignore les contrôles Cargo pendant l'application (déconseillé)",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="ignore les gardes SHA/blobs (dangereux ; le patch doit s'appliquer)",
    )
    args = parser.parse_args()
    if args.skip_checks and (args.checks or args.full_checks):
        parser.error("--skip-checks est incompatible avec --checks/--full-checks")
    return args


def main() -> int:
    args = parse_args()
    try:
        configure_shared_guards()
        base.ensure_command("git")
        run_checks = (
            args.checks
            or args.full_checks
            or (not args.dry_run and not args.skip_checks)
        )
        if run_checks:
            base.ensure_command("cargo")

        root = base.resolve_root(args.root)
        patch = base.decode_patch()

        if base.patch_check(root, patch, reverse=True):
            print("MVP-021 est déjà appliqué ; aucune modification nécessaire.")
            return 0

        base.verify_baseline(root, force=args.force)
        if args.skip_checks and not args.dry_run:
            print(
                "AVERTISSEMENT : contrôles Cargo ignorés pendant l'application.",
                file=sys.stderr,
            )
        candidate = validated_patch(
            root,
            patch,
            run_checks=run_checks,
            full_checks=args.full_checks,
        )

        if args.dry_run:
            checks_label = " avec contrôles Cargo" if run_checks else ""
            print(
                f"Dry-run réussi{checks_label} : baseline, patch et périmètre valides. "
                "Le dépôt principal n'a pas été modifié."
            )
            return 0

        with tempfile.TemporaryDirectory(
            prefix="galactic-mvp021-verify-", dir=root.parent
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

        print("MVP-021 appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=15, SAVE_VERSION=16, "
            "RULESET_SCHEMA_VERSION=5"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
