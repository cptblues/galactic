#!/usr/bin/env python3
"""Apply Galactic MVP-023-C safely from the exact pushed baseline.

This migration adds deterministic Test/MVP/Stress universe scales and an
interpolated spatial/flattened projection to the client without changing the
simulation's coordinates, routes, distances, or travel durations. Dry-runs are
deliberately cheap: Cargo checks only run during a real application or when
explicitly requested.
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


MIGRATION = "MVP-023-C"
BASELINE_SHA = "222a5e43c6c3a599b0a0a00d2a36b7f01c1648f5"
PATCH_SHA256 = "c1c7e24547d33ff88264a5e5b1c84e1d2c6bcb9085597357b114bf85d2a7eca5"

MODIFIED_BLOBS = {
    "README.md": "324c9a0f365f3d02addc454af41d24ceea937aef",
    "crates/galactic_client/src/lib.rs": "451ed1ad7af0924c90e4b38ad989fd48e2a6f4d4",
    "crates/galactic_domain/src/world.rs": "1b1b7b48476c8ac19a9353e2ff3256df68964f6f",
    "docs/mvp_architecture.md": "118b9e5625a13f593aca5ee3b08703bab88a1f95",
    "docs/universe_bible.md": "9f655de435db802d07c028c2fa9531449d2533f7",
}

DEPENDENCY_BLOBS = {
    "tools/apply_mvp_016_b.py": "1557ff3f419abbf6a1b58b897100aa72da80bd38",
}

CREATED_PATHS = ()
EXPECTED_PATHS = frozenset(MODIFIED_BLOBS)

TARGETED_CHECK_COMMANDS = (
    ("cargo", "fmt", "--all", "--", "--check"),
    (
        "cargo",
        "check",
        "-p",
        "galactic_domain",
        "-p",
        "galactic_sim",
        "-p",
        "galactic_persistence",
        "-p",
        "galactic_client",
        "--all-targets",
        "--all-features",
    ),
    (
        "cargo",
        "clippy",
        "-p",
        "galactic_domain",
        "-p",
        "galactic_sim",
        "-p",
        "galactic_persistence",
        "-p",
        "galactic_client",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ),
    (
        "cargo",
        "test",
        "-p",
        "galactic_domain",
        "-p",
        "galactic_sim",
        "-p",
        "galactic_persistence",
        "-p",
        "galactic_client",
    ),
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
PATCH_B85 = """c-rlK+j1L6vfw+uqD)7~fY4~+P7o9|N2X|x!n*K~v}ezH9Y6zJAlhnlLwzAdON^M;ectl{`#9Wv*bg{whkr6(vYA;`-PJb$qBIe+5v!On0d!rm@}5~$*|xJ-uvY8RiJ0}|?Bw~2*C*!6X0!Bj&GBshf%Urm`GM6oO=~b}j}}ALX}5>{ey!DN={;-BW;4_KU;c%)Ivr!gn(*IFm%;bYS;ejuIlgDGH4j23j4W@?8IRc9b+{L?@A=h@Sz|Reola*~xU8|c6<qo(h&=!{;4Zg9UTc>82%q48;oNdLi+C6@`1dXA3|V*+M)!YP@o;OmFba4WQn&6%>xaMqOwDu43z@$b@I!kEa~wIY10%sNYZw3)^Sd8gGx+-W@#|ks*|49E_4u(_Yu4De-?C$WwXy)gA^X?Za-E3lt47=n2gZ;!hX=+!qVDYeZgGDXa75Sro$W+^;2_LnHnshE_$2n6E5In9%p8~*@IM7Keqv{QzT^S?A3jenrtA~`c)YZ{$l(F=gP9XqE<XD=`Ro-EBw(?}S-|JM=UGk|5|N<MPuQsWiG6Q{^Vo%E>%jk=&k-T4_kulPU1RV98=lJ1&Tt4oYwQGwpO9U}@E;^HwmPF$gbf&xyN<_c<@+!UYwqtGK)r`9y~R%79<wXJtj$%!wLhO&fDC9(pWEFjHRQp<4_35D>_i}25eJBU<ck#$dCa(X<pi+KPMiI3dJbQi&1dnY!G`LG8gQKCDifrG*2ItjEWpqeKoQP4pmpVV&I;kTxnfV4B4len4kW+<qsMbUUc1&MXVw)Etkz=KCE&9FKwVO!kX`%1yM^mthxqK$T5;$B64WeA59rbvksVTZ!LT8_K#2ipfO*e_6rc#ehX@v5Yr@P<xb{6naLBBfxri-946<_*2nZ9k+TC93nEm~)|ISWRz95!>Vejt%W*56(S}ub0K1PwOHD4uyWZ&PdtN=J=Z3XycVZ{*(W5JEs>w|;q2d1-Q7+@xFsILm3nd(YlO2p&=EZ?<Il|(w)1tJLV2$vH9myvb~SzxUlluaIKBMSWhV=h6D@T`(Zhk-9~fPXD7rUes<UZCy(7(f8n&w^nMk`oKKLFf((1Pz7-Oh{o_`=KMi=@AND9tX9iCUe#rwJA5*F^^znju-J@?F$Xdkkx$KSpd#{w^ks=I6Yz&_>UUcph(^CVg(G&82B-uO?<Ycb_SIfiyz|P{*Du)tw1Tj5GK3-?~(7g)Ww={H#BO^rRDnYT?~a_ubufj$GcQ|5(s_&KM{w(0BjLhE3w$s{r_5J)@&^n&U}e8nl)QDR&yW{51&Xjnok7xJPCvO6W5uUK?r7OYa40Y`hyO)JC<p7hBoi*+ganbeH3BzwzreUaG#9f2(?-a(m4Y~I39xxx-n>}@#Djq_H)EPMC0)fz89SXYt4D);WdEoUysKhPf>uq=(TTi1H=wAOK|fSdUz{4Bv9EtaDfMXP?Ior3Np;2m$o54{RF)X(AXi(b{_fjcLpc}D>!z6GvOmCDr_(LmmmAy!T}BV7e8?AQ%N`jwZ+#QU41`C8KmV#a*o`*tgvG6H*mSwmi-8f_IxgEH-0|30<>u#1%N+!{<X{POa6*qaaa3(fg%s>LC;P8{Py+9<ow^xFHT-hUcNAhxZSqV>9J<FWAuB9yahNAz#3b}Gq&*14XHPnB*ydgdOY^{b-iI)wmn(9@ulO1^<!eBQ~K60smGyMy9<IvT4^GEjw4Vw(R|4gYp6e+ECPQ8w(l~mhmk!VgE1SA>1)Fb-#P1gr-7i*$oTK=ySnJuU`NlSkucK7CqYmLVMTRC4YZ#44>UBWBG>z-zV}f!yWLZOtA7EH;=t2>ycHjB<xeR_(k7L-5iRu<H}q}_akr?65&h_r%p;}`p%$SJGOl1g8pm0!-O>?o0`eh{%6Yg-hitIcGj`71g^o$_JxgSTqZ<CN)~Z;_p#<&)2Mg)~7=BO+=mKp3HK|uZATV13^LU)}Z78Uk2o<Bb<xC}0GdcAvc3ivu2>iZ<0hVk%W{*~2?}6?aRwPDG;s#*z^W;kwHlZ)ZW0+E+tDfb!f+9p&{`t+zUrx@>L7q=eUKrU92%4$mUwhur`59sKtSjeI!k8Tf94asmm(Ki6(nLV1M73U=NL#X*G^06d$+&>Cj6g_o3q*$|B0yMzEb7(2wJ;VFX-e3+zv8k(zA65ToX7<qcJD`tMbw0QX5>$R0l~W08$u8E4lveh?zd&gw+k~6^8gnmUrYy`wrCPaa{;o>!BY6HAJpl;<8c^V&gyLwthK@L&xrnX`i+g_q`adnky5}3%~S5$%4w+p<QXT8b8|T7o(;&FaI`q#nEk@%Qpp)QDl#i@QuxQ?r_$cj;II74XZ1~ue{n*HEuV+z;$zEJWy=yCeU{O@c}%A4z8L_1ea1=B&p0~#(Pn^8eez*J@dyw19OJOfU63EVo7a{HRt5u-qziK6V2CmFXy54V%#2dEHbFyeGo=DO_;D<Te(6R2OMu^k5cMg1SfRQi@puLtLOK@s4p`=CnO#v$mG(oef*KJgfPwH2tR5uJfhjVM0JcipfX%uZH3;>)UV`6Fy-Dz4^cd0CP&0Ype%1?`7~)g#+K-sCV5$c@I%0b=MB2-{0@_g72SiK%-2~Jigblp@=@Z*~OR1GK9fKhgObAgAt79~0u+`NXqY)8^HO!3uY5Ym^B^VBhP$ml`$mQw?ZWBAMk*W*lLtU-lXs?l91NGErD$Mh-%(Wpei4qIZH}&G;)oL9Nvn+tr|NP(Vqk`?0{pWv}>}L@$1)03oC&0lcfdF$aJ8Z@#-6oV4%6=1f5KD(32K7e*O*zWs)~6P=C;F=;&<d!(0RI;v9RzaoIbTN<I*wUFM}w(F==f~>=`e97#D~-s6@C}foSsxce;+vDq$~HQ#0e>H3M)f3F#@ACfY7WzG&=ibibB`A<<|7)wGDbFr|#=TGa3kOh`b~e?&Jz|<iuHkP9p`$ZKKF6F^xV+L;neM_c8%jNpq1}f(g(Hgj?Gs5qwoMm>QU3TeG#$;S<aQOp<WkD563Qn_yxNu~=YLw^Sp~EwG~aOWCMPc<82WLuE=zn$E2?2r8e<$%IT+i4B=7IJajOgs3H*d>2O))<Cj^wC6_FmDoTDwgc|A52q;6+Kpg^N$tS8*3j>RVsNdSe9K0$qd8xK11)WAw-3Hv2SVS21EV)gq(>>5SH7J$t>6A?^8lWL$4VL0V?aNLKmPj0P(h2qPOTtvEcfKS!A{;ceycUP7q60GwG5-wPB<!c{Y`v7Ut8E;GU^W?{1Nv^y3+8idbA{fizx7)tiVQ~n@&c20q%sR6Gh-1=Ax%Gf|8J6qzpc)QPv~@4;ytdjc#)aF@cQengaFcJIq>xV-6I3Aar}Z7kk^m+ryOZUMYm6pcC>c9~+35XzZe&skme1W2Qc7fUYBDwRR561?wxHZQ;*j;bHgkk5p!JjD(cqCl6QFhkS!<?;Gp7^!K#`c-EgtDa`;v0yF57x#{&rM!!S*Vq1~p`u12CDLDmgK};60>rO5MYYo%SeBYH16|#QBC(b;~<jzu4GA@yVLL=mVwYD_K*AomIfUn3iv~<U^KpgN@I_;6!KK%TAgduu1GHfXXz8p%G4ao%M>Abt7gc8*&pN`q1LcT%u1hmN<yzE$-eL<{%<4hiQGHg~jO~GPe?pF~X<bGUP>-s0rzTnWQ>6EpG3BldH0*<YV9;SMd&*I@f;Owp-i)v+lFp{y%KN_aXy~}8s2kABs;=*@rjOi!HXUJ^hF-T)R(SpH_o@~+BBAY@HOrZ5ndzq!;4rA3N#vg+c9<e4H0XPl7-G_Q-(5@OSr=WMu`WY1(N#_QZ%?4&oafQYNl}<#f7jxBxL`5dKmGvP?6zO>UUr)~7Ry9<M$jwoqS?fiKC#37xu9}HdFqzWw=Ta57+UBsEYg1IpYEh(`_q*iF^hWeo`7%4|xr9q|%320kYMsTE=NDR3>kDuj^U+1Qejkrt`PVrTY*zM(3sBihX#Xt2Z<SBiqEz$-?W$}57Y@1LP0IC(+^-~w14vW5Kn)$GlpRH07ul2wW=}=|8I_O`MMr&10rd|0xId!^x&_b=PE>&Nb3jr!P9RA&L*!pxx_Ts+ouQeTLEGv*()>96_nao*KwWg*${y>U@wTBde&BALbbFZDvnYCpiOUaeB4v=<c;uzo?+%Rv-8mw9yh5KBDUluP*k7$7hVXEVmg+#!4|M<#ia#dXHPTRx3;USaIkLe@5}jzyEp8Ti*%?F<Qk>)gPNd|5h+~}RbDix@DDO#YltOj_T$uUcl@)vQC187|RO|&YlkT>$^k;&Xbc6g^CSs9I$%~_-HtilFd(`a-*nmUBwRP<!ZV;s>a(N5^)h|xy%pBK=ZpPymJOT&=TeQu6UFqH))-pbZ$YdQ!)f?gohI$?QfVGv*1}MeAbmNudAuVo9_)zbf{RS9_w%Hlt->YPDk(>pgZ#~?T$@P%p#qAUe>S=xyLw72&3bR3Zs5EUr$Gt+6&|A-8^feEcJ-cpEre1n=lNJ1E&XL_{hk?j7=+ZXx2<_B%g54?z&~WM50Ook?9dc_nM^H~UE()376hEa_q8wS;;Jc!1H-ue)pVqad*Qd{drzQ%1gEPUO!RUKYy)!C07C2gAcgX^-JfPjB{XYJ;N1sO{q(vs<a)GHh%!x`nPpLV9#d^YgWteCH%L_5UWuSRweX~m&S&f`;ylIYvGMh##=5Vf^XeorJuFE1Ww6H`?sTJ}A1%;a7jzqbZ^Fl5n$k8QZT0ujCxN9vMSxet(gHR0dfDKShY(A#K;ndN}DxMmv)0bf*@4JcimSD+Q97Tv=E1<I?N@tJ9y0&I}CwO8jNX~-oiJy3|r}Oh?44x`}ws*!OpH9G$Y|+r+{DA}O288X&lb+dAo;blzg|ixE!<W!nOjvx7LQphV#eI>}k~5`yr494HJ`5`bYsvQL3nSN?F!JLA5w2TYBxQOS7Dqj+7`2#=7EYs_06K}&WSuUZC+qgRdE32_+iP3Y>jj-VLmiUU0ei<TCKvDTWJ28o=J~{4`0>o;tvNpP<Ey|4o&ZO2ArYV0g*9W=YUW(V_x~rlzYF0fJMf)I96iQE#VfGlJjR2Gpnm4mY2~|gp79UgU*&21ASh1tgUSzpAmuRabJnVKvsR_lqM(~2j0hQ&gSIm3;PR5s+v|1@gwu<(Ol<KaT>3#YkE8lKelzo}03xMF-xJfl^wx3o^e29E41vcp${dJ&#ixo3pC#Thj7fV<Upg9bg|lF)yZL(@iv5s~+jZG{JcbWvi5Ia`^V5veLA$oqszYV>lAYci!C@8SYqBb><>w6j<EEkW8b<7!^g4qB3QfBQorLJtdhOnT087<UvVkaCXv`rcxvpQXNDn(^v_AXDq;hd=L+)&_+q@RK)r0)zb|qeN0#NhtiUDtCm%3JIB~pBe30TDh`D6(=@ZW5BoLj8ik2TvEDFiV;l(dm)gI*^UNU80uE6Z{5dWS}wPa5>+JUU-<;Eze8yg`0><*hV+#PuU=#sjHAnFAhvqaE6z<b|;*chT5Fq~n{pD^JDZlp;Hkn;=iM4bzp-82^1EGu#tBH7`OO?Sx$V5tIKLh%Y6t+u0Y?9SjoFsi;erCF<2|d1}Q1+S2(=x;avBVB9u;2gVf{p<z1VWR3mo%t5abo}mtn&m104;Lt~7S83XSqMeznG>St|wj`NKKukT}gq3z>SDF2rG<3vHa2P#hdMoMPr))<L)4L-{cgh6IV*(Jw%Cw(;4DeC$)S%8}^4u!HRx2EDPI4f^Y?f_75Cq#36&t$h=X}0a>O#2;l(j{5SHO|LX}{eyI(<Ou0Q}HyTVl21Y)6Sbt6o;~C^5VsJ)Y^|u_M>{1NfgkhG+x)B4m2_Y;!lVFqPqhM_aUJS#3yrNY)3SDIU+;wkMPuO<3OqEc6dc*7V3hN6J*q%8p*>5SKiEl-=brOEy^weY%vOFB}G;t(21ZN#dXwLp4j>jd|dMf)LAXJ^<K4A2}l4`71k1)ZGFz%fZ~*4D)sfvjB<tncDNKVp!L=wIg*UJ3X7G%;pF2-{VMSd0<ouPbSGPQrZT6;g<{#+D5l8g>A!;q<ZmC>xQFb^~hQAF`iL<dI6uF8M)0?dh{AXgtFtpjqK5xi?io%&R<@<eEVi{escWw&5Lu|+=P|2&EY^hygHAAKx}eX>LabSbXqGKo7!oe%uShyuTjIoqRd3}LeL{mO5iN&+blGAhSa`75>B!Y>T)T{wKizp<l+fB85{eF0~WHg{31Dif+MB7!`cBPO`T2TGWD;to2u<xLo}lh{!}4Ta~BmTteF|5K{<HQJR5kG^=;+*0tnf@;tzN+)@m(Ml^r^-LFb1w^N~3=CwW;n&B&=m>-<dWhaBUUa;*%&2jZl7Zy)b*<&9Vw6vS0og9w<fb|W%AYU0MJ_NgwwOnqHUC8RIW01ZsqlOlT`^Wa9uynfDE%!~(}t0qtHlgOP0IiKm+g|pAe?537+&Tj#HRmY<&>c}7)RuVaD7w@mrIdpNlJ6*w1feiV;qGJvQT8yh0Sun!B$i(dT_mj||iVq5p<SxoThEK`FCFCEof5ksf>yNaJQA6-Yx--{!CXMJ${gO?F(s5Xl;sB(7^RSpwr-bs51=>d=f%buxyWfCZq155&20oWUc32eHGD*e8S$S+F^5_aAAx_eb#|nAOcZgD??$rNl$g|jF`6m$P;=uT?uus)7#r3TX;_sv}8lJg8%%rQ-ME<bQO=P?fe@Nn5?jSQhbhx;n-5Yj#;`q=0Xox}0MmEgIe`Xi;%cVLeeg^xC6D^-=du`7)3AVOb_j!U%GP3kzyq~c2uhpTG&rcntp~0hqznpWJ-@-@C=NzW;mX(u#oS5w-4Vxrk_l@nMAXIPGYApO%LjxH*ojN(Irk}^4&n~Yra>K6hc194pcZWGPFJa}9@S*gQhl~w(vV6_mWh7yAx)zZ#y1Xs;XZSVG=nZk<F5P{2xWQ_c-dIo1bRi=y34{uFjiAmSr#N-y??m>r<OCbg8av5oru2>(1X+>eVZOR>(&}qi5Dk`pCXX<4&j62=m)~HKY>UXhbpBW&&-_S#jn8n+grvy0U^GZKS!vQ_O`^_8NfJo4D9O}N9}`mrl@BAoB>ho|rIKNCCt3QWT)MRJI24~pjba`}OPh)H?+d?dv_D7<;uZuXx@HqRy*9$oC%ZqSP0`+{kvTQi%3M>TS?YqC>A`73M`emR+T{v#*3rhu6$Oxd(JUujvx;oZ9Z0r;Tsc_=X!Y6xhZ(gi2r&{{7NdhsmJ#Km9(4IYM4Xth(huLt-|?bx=GP5ZLXuUC1)cLw1?eH2sG;3uP`X9}(8!vog8?z#LAxzaVwWvA89VaJMy4e+3hbEA&9_oUFfsp^oZ3Rj(mcG8WauMb|24yG`Y^kt<uj8QIRxtm_BRcpL9}TIozwIrztE#X=TqV_tSiMyCQNEiR<SKod4`)vxj*QL9fE^lyRbtLI~pn9scJjQTUSRRlLz3<$C%dCqdHlPYJxpzBPII-rF8X^japYa4<SxO9#w8(wszdUOs+*s^01HNKi%#bIR{uL$BqN$PQd8HvrqcbmXEi4)hfvK?OpVfDXtajVCr^amjn1Ed$|d2)FC;7HDw!<%5l~8gNnD>-kDHH{6j?h^a)-P%4DjN5Kib~y2Q{&{0}^QCq7*{5gy<WU*=Rs?&Ix$^Ujk}ZXa)-&@&9A=mtAoZ%=<e6Q#}6*BAG9c>4MNE_`zCdp7=k?pf|lh@YrDg@YeO`zNXw?YjyXaP}<e^6OE>^RRvW<81?eqRm<?qWAyqDmDJoFCvw2U{y=WKco~gTk9Ft&fRWGg<ktns2j1ZF7wzEM5d~1)9;QtXn^~D{dB1^2o-k-C|8eRo)U*N<SV@@caym`mj+#%G=J6gg)&R6!;|ifsOuqx)#y7Yi}dD4#VRO>)xejX<?yD<gLOR_B@bSHD?r-|z}o;5kavz%KkIWW7dK8T$BD~wk>%M|V852F6uPT^$A2>`6gU6DXQ3(Rvlw5|yuVN0e}8zeZw%BiAZ1;65I~%aN~umC$GLiMuSL5Pd?um9@*xNV7#1po5h{T2NJ$v4c_4}astfJd=C|bPkl_5e>rCWL*Kh(9TV1Uuu*!+3vJp-!*Ok%{X3MwW<$@rZ@b`IxvaQSw6)Acui_^$*Mfr<_Z{L;Q$$fpbZe%^%FNf#Xq}q(Z_7FvTztQbhhy%ytq)f}sAY$p$e;kZQqRPpafhbC{ZU*w3`tC3-<4axv5gt_ncCIJ2WRgTjo*ffe)^{_ZcUpxM9wBFf!w&z&VmsjkFCBngnb?7|C@C+bUBby|Gr@Iu$QJWt@SoOxHd~xkWV`6gLc$q`SX%Dl+Pe9SAqM#&bVa$_ARPfmDc8w08Jf<`W1)gy<x+}0yW4tdN;==Sy{+%=m^T!?wlZ4jQH4psEzIXl_y^Fr6YklRk_?MHr4s}UjM-^7lAGoR`-I^*n|!L;ny1GdvG&e>qnhV3Lu)A9X@YiUf^AgkWEr=NT{PaV#H;IHiz9!|JR~zB^adEi8<gVEvH2uga=<kB=D}nZL;O`Dv!7L*5vHjZkQgU1PkGtHDwO0eK5GRaO728|mR?tt6|i(cy-{&9HgD+)?&vm~lFmj+cQqRuZBp;1NVti%?CDVj%<M7i=`6r>7GQ}>5pa&@=qzB&z_J|(mubrkE1-i1?0A#4^8q&kGF!I_q)hjv6Ny7g{ewXd-KIgeyDyW51&1exi0r+DN9y1lpyG}1aV;OVRYRyjCg4(qlXa0@F`c2t2ZqxwK7f;>u0CW7=Be(<H2}H+kh{82#iZ3s+jhZWEA_Hhw{lB=mnGT5mcFs>w#Y0}mFsIX*5#GAOIpd=+nrfwwm<8eW_Quq@7bNrwYSU0DX+d=){-vIipr$YlSh|3K=)+61Vdtt$Hz+x{x>@FE@c^M8W&4!C#9^#&MJL^h=KkV%2wm&jPH^beHI7f3RP#vEFSiO9{uj2DCwKFlZ88u-~RmO0-A-+A5<4-cqqyk%ae8|=NG%RtR8aM&on&0I6FB%uNt&FI@A~R<H?(ov*&bz4i^P~p)H;C>lYSucJjl?*~y#ZlgSS+-~4!Tc6#=bh9(4b2K&8U_n<!<^*X~|e>6H64T>v$zz~3c3BvRLQn9G+V0c(on=C0umMKzJs7;12X6ln^wn1qi3^a`LT4i87<P+iQmli2288Su9^H;CN>{qN3bI5wX!GeAjWpQXySt18#QANLKn%@$om8n^sF=I)6F@dSiC@e<rlsI};<j>L)V*ngB&a-G~TltC#XkNmo8%n2TSBdp5zqopuTBHa>H?3?Be4o87%h{9B;;w7>N+sZuMO<uM1Xg|Cwutkc7J(sTQg@WfhK|`I09?Ozh(kXsu0&R34V+cIfrVMEaPBx0@Mn22w`d#N3NfOs7j~}&nb;?`*BR~+1ECR4v5<H5h-lb+*v~i5r8cOwBp;369p!rz!}#Q~3c5KvRMaagn3&f^dV$oe`>p%|YQVWoN>dg->f?)=_B(QaCZl(<o%Pc6c;zUgEz+t$tb|*&%IxX%>>+jWRpAGbNGwSOH#SKP^1!Bw_ZpnV67j4$iIj8KGILVkQ&#*gT~5hdd2QJE3ZNq2Zq$p!rC%98lJ#?CNu(M8_Z2*)Kk)jS(qwXCa@+R%bSQ7s>lxiaWi*isBrhRU4rqf>g^|_O+Yd`)5-m7GHY==jTQ>;-1||4|xR+aSmGrwSDy+g}i+R*-H%vU`V_|LoN993YV{2fU31f@=^!agVAqe~?ADChWakY+aN-^v4auh8b?~(`f;t+$iw?=$=iZh%ntt-k~;4L-0UgLQ4XoBuabqai21<6X$Iovp$VlJU#GD^uPIzl@RM!F+6){d0S52b`VH(H4swNtueOEqyaf_Zx<!rq6bxvPn%O&fbu+-NV&A8Zb!mjIGF{;;%=pYXkS$W3dC{feRVgEb9lw1Xa6+tF|+ZlM<RS)w2npe+gL-12<SfmKt!&a+k!=vzU;`xwpw6z^l6zFFu&5NFbKnRyYOtrIsNJh70QM!sg4APr5c7ei&G#ZXyUF;pt^KEOL2&4XTB9HGkKy&}beDotcbWn#<|>22aR<eRUF>92?~bqHFVm;e4E;QXA+x-MmHVLjrVkVou_&%gOqB{rm7iQ^gV#&3u6K-y@3ggksO=#=pA23!iZ5xg=<*$NrR=0GZZJ0d9Xc@eD(C%`D>GjQ6<-XFrdwhyPt?$aj(X+CiNYKjWHl@hen86lue^;SOtg8V9V5uPVRIu83g(@{DFM92)yuoTG}Ko$A`4Ds){CG-C&Sw_hfU(KN9(piU!oUg4l?z1NuMU`!QwiA&hRT3)8?~=?1J*s?w8P=V3pS%fh2`jv|ZYG-e(w>`pL%Tz?-7A~o3BB;2NFmnr^JhZ;(5d{K$6Bc0%OjbZpk?V6hj2OU8r=g#W^W)1xbJ{XDlBc|{l#RbO&sqJ!BhQ!CpjXn$}xT^4lAO`VgJhzt4PDXH0CJ85cMGhJe`VwM~^{v2-eCYmP(f@;0sRg7M@_7r^g{^@P&VqIyFgrEJ+AWl6pYN;f5JV;0j;!h4PVBzW)-xNcruR+e}s1-_5hfsI1(xFZi(8$YFD*@@S*z-dAo>lef8rE@csBwjM;~#sfr}xfN9JFi_iU0YZF{c;x$T_(W8kK*?^md*=Fv`B=|9^6_;H5#CQ(GvI}k*O<1Py;^A@i+>7FQV;xErTgSnA$&E?b5#WqFA6l9j=Lr>&zBaAXdmJWrUI&jSdu^iRJ%8L-w6$ohk#h_Ud5(<6|ftqo=+cVpiSxdHDG2X_diN($+y2cm!2OWN%yd%6r0^tabbZ0L(f`=OF!C6?UE3wYAyJc%y~<D?WBA+EV77(>LWT4zJ*p_hrgReV2A}C08M&PC$m>h@0+KinfShWJUAT(_-cAKt9+z(#XF<f0Uwy=fz=x=W<0wqUfEe}g;zeNilIHa%Y}c))&IqZ_z-K1hU^LOEeDgOUQQ1`ByVNln4pcL5Sj7p{%@D~vhFcnk8j}%#gSb|YUx5EvU}vmzx#NeTz|{D!EE^g^ab9pTT*pRaVovj{rg}4N4L!m@ul@@M10%%cb0qU2VhfIz_>6v)hvzi&EWBe{EO>J;oz&_IlZl3dxyRD#`_v;0Y#Y=%s_hPSSQr-qGZXJ7JhZ;o-i&Vq}P(C-3bIdWgF7nTFNund>uO>n<}RrBIK!<$4=g+&Ric|g%A(;uHE}P)Crb@Z-~FY!$bNWa<MEn%YyqmJ9g*|=`oc6g?LH=EX%&O+xr->b?}v^bdcc{zPjE<HL`GE0U;R8`I6#9fB;`8=g}MbP%quzg(kK4VDTK$=sGj*x_EBij#&@`n7qb3z&tFPYPr~8$(ZL5F6RFIe?&k^_0?Kia|dq(&`*FL##cZWRBCbq3b@a8xmjaVWDh)3FH_-Ddfh$8bq-BrrdOa>@9(_(I}~9ocr4z12;6|X&Yr)b_s*x(SU4Z(ZSl|)FeLAL01wdmYAwv$-Ty73rFv{;ts}ghKtyn`FcLBvfOI9aFW9*#@L>{)uBYmb1e;Ak|9=(8h)t2H0k|d*84%_A4v_YW&xz9U0{h`eb1bY#Jg#ni5bMDF4ouG_1v3k*HNRwq+SmCM%MW=V`y>T0gqnw~kRbrRD-!bQ8w<?al$K;MSw(@-#X<}3olj5wAZndEt_%8QDnO2$fRv2|{ANWyiNF%H49GY!tCVps1TjaB1m1AJ6PyjtYs>=8OJL1Odq@VlvmG1(Un<~n48%YT=ppgwI`BbL-``Q~;q~$+6jRw;)=(Dvp72+uV!?!30Wd0RzL2df?|lq1h5AdDoq=EdnBI7~1d3pByb$>Jm6h@cVtc+s-3k=<t{gx+n}+X$s1E3#5e`#s0d*nD6vGU_7C1TJ9{$u|Wxyeqp70?6&$f7Mp%e?gk;>8pznjxaE%_=Fg0&@Ia2ygN)|7^^77NT^0%*V}5jiY~)8t+j#r9&-7=~TILQHln8;Pqz44AprBF#Vv(GPeJmjeJch&<_ZEbFBZBw<bTAu@v!3z#bAE|R0ATMI^G0si`GO3>b9JYrNS>2)Yn`hZT*`)OYe;_x+KBasEjnma)DT?QgiL$nOAr28l}h{6hDW90Zw8`U8@@z@n0{L=Mjz}W_Bk~zJs80qK0Tm&>J9})37+zn-VkZ5Pc41E_XmW<x-8a-t6e&6WP7XK=y5UR{Zpm}ex>5tFx)lnDEFHRD_^@w%$rGcBCKmR53V5A<L{ru|W{N!SCe*EW?*Uz;kgQ<yuUak4<w}_`!yW49Wv%mlK-<eif)WWwAI7B98<Lv~!*m+vA`vZ#T!`cUR07}lGn{43l#X=ZEl$<0>$w|6Einv8OveryIm@$qH@FU@;Y|OHB7y3MuX&x*s6De^A99nC~CKm$-czw0T{yOoEEKCeMBug@*e8Vw7g@8ho9m!#Y6GM>beheuJ2+OYS@5oBPPh=Jl`d2BN&~+eC^%d_9p5~YZre0YADrk}cp{hWTbjwDZ0&E=cOTZ*RA$T|;$e9Wvz>1Irq5=Wm@rr|Qg_0B`74S7T5&Z^rrPclpIQi1?(3GIQ<DPA;2t;sCq+AN!3SJc+0JXo1@e~xyDKwTFNjoXfVc|3=Ez{E}*d5>k4;KI!K*<9$ms~jMy<l8uJncShC~Y`QP;1KbrGPV&O|ysir`YL|%7Rm$wlN&YJS>qIPwqRY#S`5MY}}rrC7E^rS@foBpje!oq=$YsuH*_PJ0^1{*+lrjJh>_3ap6;F>>=OxD(a3H$_oWBKo6o3&!`ORisS(J#&YK|sN%TRTs!l3cvB6e4qRekyyw0j08Kq|F{YUVu2WJ^!n_zHBPw<X6PL%11L_IulK1D(T9d{9-C9{7v%+$thbkFak~_WUD%fxO%D<kX$s>-&0ma1xc7^yt+b=zraDM+cj}$U89(F}%Gb59uREcwvg0%~7ztABPZ77zBZIm|t{>~JlMzOwN&jjAY2|>J)8=}WCYw<D<5J$^Fb0JX*ZsxQkO>o5eQ@QL!JA*e}j5L$WHZwFPTAW#UAfwEK#c(jNxoPs5ZS(H1B+4vy)nd%zLweH{?XSXrWncoBKnbw%bqf&e_*j97Vn+rRAiar<s$RLXA{Gdxcb)2<RQ^#_`yyxpEhRiWPF^=^@U$=dx^hsdXas*BJK~^|1Ad3#d(GM##c^v*99X*C(i|ZGJW?@c>dd2nE2oM?)<t`hckxN53Ljr2F$F?8Ci^pZe8@FRaMrpe`-O*V9$2;i3!^cyod"""


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
        prefix="galactic-mvp023c-", dir=root.parent
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
                    "Le patch MVP-023-C ne s'applique pas proprement dans le worktree."
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
    parent = root / "backups" / ".mvp023c-backup"
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
            "Prépare MVP-023-C : presets galactiques 16/64/128 et projection "
            "spatiale/aplatie interpolée sans modifier la simulation."
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

        root = base.resolve_root(args.root)
        patch = base.decode_patch()

        if base.patch_check(root, patch, reverse=True):
            print("MVP-023-C est déjà appliqué ; aucune modification nécessaire.")
            return 0

        if run_checks:
            base.ensure_command("cargo")

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
            prefix="galactic-mvp023c-verify-", dir=root.parent
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

        print("MVP-023-C appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GENERATION_VERSION=4, GAME_STATE_VERSION=17, "
            "SAVE_VERSION=18, RULESET_SCHEMA_VERSION=5"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
