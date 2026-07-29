#!/usr/bin/env python3
"""Apply Galactic MVP-023-D from the exact pushed memory-hotfix baseline.

The migration is presentation-only: it adds a stronger spatial projection,
bounded observed star signals, dashed routes, and shared animated planet
visuals. Dry-runs are deliberately cheap unless --checks is requested.
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


MIGRATION = "MVP-023-D"
BASELINE_SHA = "0b68df7c62590ecf48c422416ea64fb390683372"
PATCH_SHA256 = "abed1bd7afebafc541a9b553ac0301ed68c553631745d05240fd37a23e2657f2"

MODIFIED_BLOBS = {
    "README.md": "a5808f683b91186554a2dd4b6b82c5c585f31f06",
    "crates/galactic_client/src/lib.rs": "cba25c5e5247ead17d435fa858b7ae100004be0d",
    "docs/mvp_architecture.md": "9a38fbe7e4a59538ed76f8114c0dc27e3a4b9040",
}

DEPENDENCY_BLOBS = {
    "tools/apply_mvp_016_b.py": "1557ff3f419abbf6a1b58b897100aa72da80bd38",
}

EXPECTED_PATHS = frozenset(MODIFIED_BLOBS)

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
PATCH_B85 = """c-qB%+jiTylIXj?0^Pk%nrhLEsGDUu$(eNG?s#qIGI2V4_R7g&XbG~pLy>A;9J`aOHLvG=)|%IOo6h`(|75=8R24uH1VKr5y7&0Ol1QLXs2d8X0x%C23%0v^7G%u7`||MN<EMwta?Vbx?>j*>=a<YMO?s2X*l}j=>14)T=JtBy;c#bncUP^s)9rR^wg2g#*zSJM9<VO_@Anw|e(Xmn3w?HxbM{LVT<|2d*}E6-m>&g8&O**sewzAcobf12@IlH}aT;Vn3>62D8PDKZp6swy691jg@Hz8Wp`QiZVUKv0aTY`wPgXHLgO?F!%Xl6v0?z*KFa3a;pI?7j1Q9`HbH2mE@ACo~Y%z)RjLYxyAkF+}2H)abJjj#lFZc<7gER#Y4!iz;Ssa8^#Xsd?YVWXfKaAnG0EK9;g4z2ZI%9bjgh2}3W7q$|X6OE|K#E1;FGXjU*Z;Mw_1Rx6g4sE?=<IZNy5D`rp1yvu+v^W@AG81d*MDOt@eHVT5u`Z}3zA}UCs%29{ri$*rDI<u7f{h(u0kRSbQ{`VA~rylb03h+ld4VZBs@R!L;o`1HcNqhJ6%c7gb<#^aWapih-k9B{sN3h9QH6r8n65$Lz1u=eBn?On!Wx#6Fp5Of&dT+8ioRm8NkYk*Z?2{l&`-4Is61r<?J+vV(c_dB8glYoJD?q$>P(LCl^#SiD%~s6yCmwlWg}S2tyDkl#aA4zwm=oL?Ze=hBl*sMe)+c&-D6BDCk5#iG6Ik(+$EDMG3G=vmnb6Q;<|b3NZ$`N>NG>r!(LsNaU5nj!Ri$&_zHmBr2uQ=1w<D{2Be^5wIJC7Vv})*v!A+GXdD2bAkyX7(_EbWEJOv>R|vaqIeLwK=5|DNbCT~NbDyi*iE4;z)&GNs_qriA!IxaE?IW95<(5AN5rH`Q0MR_<Cj^UAYqZh*Iz&le9}&6);}vWVm!S5_dy?I{Svf*r8%;V0gunwFRzjR%RCZfDOv$|0rWBhfg%_g^e`fEgnvRJ5DN8cS1Jd`XCUbC4#^ZJr$JVzKnMa4Rtsd0M{^%Q2;@QcQXwzPTo?wB(QFx~tMlvM6I5J3ia1IJ04LQ&j+GQZX-cUdqWPr{VwnV3zUY6-4x~OLE9g1Tr93NA4DDo+oIBk#2+!jj4HiL!evpX#j{RnSKEs|#y#hfXZFF|#s>z-uK2*Mo$_WBH^JZZHb?>Ih>~0vGI$#D)H<ytnes=2jN3#)__<i3W&Iff9zZr@!{hQ0-kT9}cmv!O)07ry8h5jY}BJ=WKI(>xSehFTL`B@Mh?9f{v8~2mhITd&*e`saWY(AZSoW-G3-gG*B;-}|N{gn;OU-IenF~}>v`3yC7x>)CwUtCS6Fx>DA;Q}3!hxp?c9Dh;?SQO$j0Eh;QFQ?NJ`h4Mo&8`&+qE!x%mN7g6WFF3DISCt|KgCB+V_{YF=O?oS(6*WK1I&2#xdwZc@GzeP*dJ<-6T%uA+hO=8Iw1%H76BiL5+2YH6l;cf9bTdk4(2v{B~<z`&?ia>H~HkJH~|U&YzwGB5->k!K!J}>`7%zfo`7)yjS;OqO1LjN_Sdsjej>|a-FgquegK4@!1g`NLO+F)Hp5?!F2&~wAqbUFM*2g0%)0$Cii#AHQmR0Q%O1Z1+)q}V&t-c%U9!;ZmuE+>4_}@fdhq|%(W8gQ-r>JI{Q2j@mk(bZJ%2W33ougm*}xed6g7^X9lbh&a?gJ}Ieht=ig_o0J9%~Z)H^<Udh}`vNCdxesO46bUK~GscKFJBb@(r@etCK5{q^Yat0z#9fR9^?Jvlu3`N^xI+@KHTz@N!a*&<>|9$6hGdl#amn}M12R;UCSlm@=c&YAR9xTn7L7D>GH{NyaPNF8`|0pg86J5KsOSXpkTgp7aB-&>+$huwd`#8;cB@9q)#-HB^Yko*CR+f0ZRTf_-_fn}qN_%0tSaErf$pXNb0_tFf_!WnSxBH$mqU;)wyP6sH_+-}y&S92)sRsM<x)g$2zwHFt<>ERgQqx;ouDMw=k2<YaLI6whS(^U|q8`~|tzP4h}l{nd0jNAawRvdu5(ghSe>F*D0cfz`ZUf&*!kSjdOm+a@nU!4cD^o0-*V8eHBKr2B!td~;fJ8yTuJ|_7rW3Q>#WaZL=WwHx6xJ}s;KbnX9-c$JTK+;AMqV;`(<TSk{=<lQWLj)q=Cmx6{AO=Lof@GWToxt-)U<|;T$hz<Z#_eSOR2F_vG(#YWFt>(72+cVT$9!IbnYLDuEeqdr%Nk52SrgG0^yV)x=x*+@FZ>gp@UMj=ShOwn=P}rp?TUW}iz}?dPhflg8YkhL41`I}z_^it`hcY@`aflNmU+ggUWe^IU?)6WK>6K5FYexD$MFYgo2CqWSDsw4bm@m7OV9m;&xLatps$IOmVmsctTd3qd^#0GGO3lsgGK#VnE<UhPOY8sc*!&P0^Mf?Rj5UQz$v@|bjUE{{O%NHM>JVeo3xZRZB?c0N`E?pOwBG}Riu#gJnu6in^TPD-Vz;-dxFkF?H*X2gQ~aqC+a%>d~ThPG)<>-zVP!fgK+_0!bdQRVRUfv=mVG&tJA4lMXPBfZv)i69hm&W8uV@EPBuY!2s5ED2R9|+A1v1yjcnF;`lCiQrFY;m)Yv2$Q_OYh(xEaWSfY=(fgwj&NN}VEq*C4K^x^Swvr`TX1F;8_n#AfD-Y$WhpWZ|DPt1Bx20)-gRKVF8r^%UD?+q30bUG%8(s2gV3-jXH%tO>Gc_jC$DVc?3+CYn@Z9=l(z&3FE=xhuI6Wi^T-o|EoVkDM<P%|7CfKoFtK9mR(wQ2*CC_*GMlBdQ<G9=A03CK_mMh5P-b9P%~Yt1PbciS+ov8*xr371l7L}!#v?o%J!P5>V#Q}&2HrfG6^>RUZ$Ftl0E8BXy3xR3w$Mjg%D;k0IU!KdsvfC+c<5~{*H=Ij*e_o?QvFY0$}DIWVM7u~^d-yV!_As>FYI`_S046`Tp5WhdguhSo4qOJR%j#FA6MecR3H~fKZXxQD7hJB*ppjT01(9;drIvToE$DNpK=1~~Hzy{7B*WRqBQn!stU6o4AU`kcR(CuEk8t-nfcXe`WO|=+070kz`4Md!!r36&9fuLEOAfEBex<jqC6yTRD)WUBCxDr0U4ALz9wyoAIj3eF{HE1N>w)=L`R)Y+mw>54i_Y)}dM&;u38TVo4uRc1O@%n?G{q*O+kFvKnvxIB-ZNRLJ=WU&<dBlID{*K3}d82V<2}i@5>u+zL{_p)M`a6&F(-7vQ!JN;Hux5D}N>I;05^tsc^-07ZIm(DpVxQ<Vk=E`eqYT<c*Xau&#UL|)$@LHhnaRFA+SUoFnv~O)8VahzUnijI{ny`=Qvt0zrb)C`)!>R~KpI282B9&mY4vr3t_z^STo}7&hF+H!<>lv{;sYm}-Ewu~F+a`EZ1xCkC49uID=<440`bGcUu|~yYX=MtEs@Be7#HiduP|^W_4zMISBd0fNgnz0xugdC{EWd9l81IkVx6y;;$8%4a2kNBURl-GDHGLRmrv9wZ%HVzz*kk%ZI2FQI;CFV5*~t*KkZGGitbkRGy({scjbrHOQnM;RHlwRU*T%iJ-w+1&CaD-mkSkAy8?4=tEH>DQLLlCrgl-p44~jR0?^D}MF%3q&16t6z13ysFv1lDNrPZs$i`7YeT<?`4e}<%k=+Odu~K+aEW^GI_y-&v3X4^=B23{BZ0a8Cj`oJ|0S_elW$j1)k|+Lwg2JPCxr#Anrz|v<#Lp|+UhgKkc<Z!dBPFVVf);tE>@}Ya<f0VyWPcAQmLss~gIZT;G4>eOVhhxYwbrRvi+T~4>&<neQ+Cy6O~Nb8elC}pO9F^xIDa1GDFq*jwbgS#Uwz<5XL$&gJjIxX`T}~N1<*||UMy&FO1!FUletOIRa!n5F;CCWNMCKnLw}Wmtxq}V^jzRXT8uPQb(1{7=qyC17^^yjGVAIP*bm}$tpkeFg>c2nj5cf_Y;e<3t5bjwWYWhse4`P;Hi1fmXbq@NizaW&*oT*(SSKqAc$7Q^`g$uEy#y(0tXZR{3e=aFBOn-el3j(o8dWL>NOofGr{^9xb!XYRDjsM4N_)D_Oq2#f(O5FA2GyXr9jZUbmaq`}V=xUcBX@g~J$rwMngwE(u6*!O%D@zcdaS}nLLnkm4~vKi&LreF4}2@FUKORcS>#^?XCh)h$4+<)^iIz(?14U~up{W66-%rE%oi?{F*b-AZGWt<2h(sN-r*W-fr}#^Rd72X^l8Z)Y?G52H@48dK??-NQ5vYZMj%w6*f~GY*5Yb?sC(MF2eG@Q2SLH`w2pfNvcNV|yr1H)X{DR6o6}vBce~Rv*%kzIu&^W6zINJY-B_wZznpZpPPpw&bW(i`(H*DZRad|^lI9Nvr#dhaT*IfYz^SrSlcY*;#JKu(66HF*8<Dl=?Dg%s0Jb7@OF<-*Zp0i7scjelv;Se8$hGNl(Gf8$rkKEJS=bDWtg4KM`7(&m8@Y1ei`93AIJUtgc8s4oWg2U6(vQIfEfG%#Q7BDq@m3lpr}$m7xk?;<8C-U1b_eshU>ct~Wcw{kvm1Ga971uq-z-{FK#W`s=9Uy%W#(Lnt3JQS3dPa^R4k-hFZK#uAF$KQ>6GSWiYsIlz3n(3g6v$-&k|JBpqPAF^$-18Ny1>R347hiHN>xkds|7eAvr*Gap^-V^I2Cgj!RxX3`cceuzash1P+Nqs)y7Fa+Khdc`dNC!r4)#gB=G(a~#F2PmN{PN~{baD1C4@6v@sxj1wE?cJ(!>@<*_!24dAaxZl!04Rs;NP7r~HXO@A&4X%Oqo}r)qBAC5@CBDndF{>1oj)B{lChQ5n)mvRonL9yq)hw0-aYFMZsV-Encv|zID%n*1B28!-LSzz>T~$cRAHwrfb62c7tU~#<CWN|c4>ypjR<<-)^~WOQe+l-{b;ctV>jQl$Tzg>ZNka^)-ZWknJ!-tz(5puIs%MQ?dhhC??E`huLUR!*`{?UNbBT21FZr#UrUfMyeq()mf_d1WH=n;%<kO*CRI>KFrU*UgurlP|zrSXf|7c=txkma2aaA$;ZSN{a_4ZtzY|G@vAY0=M=63eyVh&ECptCLyDBcD0u_&|LzSGkSAOxC=TAM^@gBWQm`g;^Dnb5jC1wQv1O>Y)04W^7jkE<xbUcX`U8XeriUjPTKTFdAmzQ`er)^s{eoK@#S_4P1}(J|{zG5g;A8b9MJZTlZJY&OkKyU)JSZ#NpBtw7w$1Z@Rbb@@xk;-T1nv>kY~a#{?o)wf=!&&_03gH9VEsd~MhM;rOHm3D6p_U1aiHE5N2dMb-ZMHyDX+>sIVXCpno){L0b0ydbEuGwA|=EMk7GbzS0Yo{-tP3^9oUo=ci72uP3qH)61d}&->lS@Qu7PaEyVpcn)t-HOR-5;^8+aK=PZf_u$CfC=VvO6aVD6s~9PhSqjFcvLywk}%AY-%&yGWobZ$(gd#I1a)1mhD<)grpfRq)=K}(hyL3P=;C-ji%H&O2V@|i4<J5l#rrhjf6Vcdp&cfHTO=G|704?tmY%lr!v2<a5ladXl5XQR%cy_a=$<s$1MHvHPu8es|S{HZFI2#pwp6XM1HK=r|#PRbZ_1la}{Kc$s%ge>$d6@8P=IqQv%bPDCli$lUOlnS|x3PWKGIb-HTSX2TGbd+RA>xg<^$E31=9%*yLOry!MnRp-GlDegCu#DI0Wa3#K+WZ`D=`lqD4fuPSL6&5@@hI#*eeBJ3E<kwCQJ8iRMrn}P}zScL_Z8B$Pq`uc_U<oVOXVsqBfW98+hO<b~QfhP_%!5jw|9ovsJ7uX;cO{D{EbAr^1$T4k9Kt(^DRi2((TPlH3>f1S}WURC<a#V>7DpF~jWZvnO_nu!_pUC<9#LAG=L9MI|Yr;UdnVSXsU?42_?#<(+A4Jn>(SgUj*oR+iwn>7s0C~_zi3o!w=DyJ`JX;wDi(+M>NjYH=uuK8BvXy#BPZ=8lvhd9c0(@3wo2mm`P}d7^#U#9P1zptxG!irL<OAH{l`6QXR-z!TKBRWd!>Oe#=@lqwi})R<IJm$|f6Z~DN<og!b~;?)T*wC)p^z^!3b4|bqJ@slEP#D3UWsqzyHE94s8UT!E6m~y#A&u%SsDbr9IgVRFN`b95+SFm20Sf-{LQXxw6&T4o?9eo6mQYwCUrs{S#=6l*4cx;)`r|_oT=2tyb@K+_T+}BNA<-hu>b|{)C@#h)k~AU(5ln4+Ko`8oRq09B3-6iE;kjK107Y7l<2P>9Ci#jhss-u*UA`e*BqP0x`qI4w<QxA@u`9#TJ)?4fOXf;p0^1#bvK8Zy0;E7RUJ^lXfdX)*%9(?Frb^#0IRmuF<nKYb?SywHZe!5gt!<2aq=VPU!xxF8y@V9?Ea*d7*Z>&P9N7#8|%|W^PZbqil!lz;Exg0KS%hQJz6J*&1Hh2GpvDM&*}{wd4VC4W3;!ru*#Gr!j`^UO*(5*iP09^;C(e^uIu%By+(W<I(G)<X}|d<J@@DF2O!BR^>~D<k(hr4+}<v0eGLUKuYN1mWkzCMW_vwpizOMN4UH&+P^7(qJpn};_IvhLdZcg769Gg#g}~{tE|cdtrF8DW4{Yd6w4{$B#ZN2fI)go}q>@umEnwBN_+-`{Bl6X=^2Dpgjw;MutajbdPm55oRbYBG1r@5|XsM2(N{uckk`YF)$&Yu$PY;TVrsZB5;rFtZEGSwNzu=*hz<&@mPaI%_!SBZ2GhvqmPbVY6Qy65o;;AtiY*U_4dtOg5Xs$#5&K=1E2lvxph|RQ<M_<lyo&HG68Q7w&kH%dQ;#>x4zZ6_Ln+Su8zqQU2(fjVu-bbR3UEAH;hU&FluInReH!|l+g&R;sVY#wxq-1t|rq`IuXb=n->T|_^P@YZGanngf3aveBB;VG@h-M<La)yUUNUrqTRt3Egj?puKZkg{Pl*d%M=DJIx-ygV`dG8J<V|yD>HDcQ8L~WI}QBz`ad+x6v$XW(7bwF!Pgea*adOvi@cO34I<@qLCbiazAH>2lehJ-P2UTU_~JnUDe$nPQH>(pX$H_%x%W?Jov>XdG(W7;Yud{R{=O+vmoD%iA32D{M~B-7Nr!gEu_uQve1t=?X13nqK8eZZ*^bW&Eq6t-<Q_8FJ*X>VKM$C&qGGy-e-mX2L;So$q}bQ}V1SnEFcW5lG~dw5eJQLAgt$Zf=>BBaTklR}il;_#`Kp5fm==YA%^7p1*RaWEG6w$mx?WlN{ipI$zE<h?lZ`ki)I`1gYmBF+)|twHCu=T#s7G>QJa){WViNFYt}giP`XJXUcu#}+2JN+!_r0F)iOa`*1Ia~n^>6gnS_irD#0;^xh>VK!@SLu;vVL$<0(VQ)TMOJ)jwjP`98!`q|bo;@7b7-ATon;wa+?{<?r=JX}N-A>`+w>!YwIKR8=ZAN|1uc_W2<5-s9)Nt*SU=Aa=^74hC;1l$EK1$-%Qm5L}rbrPfAR!)C`?m!3)P*UNMZVQU;s*5W{>jd|iLUh%k6LJZqr7x=@qVtZsW+DmiZD@a%cXG-rP#V`)cST?(AZWe_i-9lPn1I1-cB%A?vUMOR;^YS-Jp(>{t9lWN@nxc$IlHS)aK9V$V?SChB}L_$i9elFLHO3-l=ZS%Xs$wO6+GDt9hu6LqJ5CNx!AcV~&os+)L92=xD}eiD65L(hH+)JcG8@UdQ3gkAj(m)@!9!V`fN!6aflTa5fRJh)9&_jq7NsB9ETlXe~WInMu<d_Uv`6`eY*er!~~+4GLlxp5&?6;9F6|N&F8m&G}z!)Tx9UYW4<7NWioUL7c5I2B)^~vhHBN@8TCRY~k_F38jyUOj6uREP}HY(Am|jNG~nR(S{{8$qEBzRiagK=%w9~YgLYH)j@#U?nQN~ub`UDup71e=K$UqwOj-tKB~v=DY@vI`|C3wKYd!;oH0CHW<|)DXpnk*xzd+EMI8M=OYWmJ*gQ$_n8qT9xZ=E#Bvs0VEL|*OD6l+XL+=;A9T;^z3=~_R$`bQh8d?!@-!)~j)gPAo<UXlej4|5#L-`x0i&aY!!e><^G)$G1h_xt}L?+A>T(mq54S|A9AssD#*&vYky`!b;(Zf&0hy+6sDt$fly<^G(>!~cE=0v4R{@+wcymfjCSvHpr=H-&CzgmS?UL_iybg*jMoqqP0{2|$i=~OJ-$sIRJQ!<)K=4pRCz$M~gf1=ECjK>OVGo3=Qj$^dG;JCz?PfkV^<~M7~9>+6L5Qh56b~|yQds(mwMZRFU`vQ=1{z1EPC&Tt#x$VgPL-yv@P}^>A?q1(scd(_96cNLaLb5l;sqnCmtCRa%2uX2XQB_PodfF<^H(7$`2*cJrJj=W|vA5bPDc+U7@w4(pQ546&Uvr*ZP1!%=$35##-E+0Y7+L!Lod;@?2N?2ih-W<g#!@!^;`v{8^U#=b|C-n7G}ERIv;%{XnDU?y%$9-<I?A~rsih|3iB(sd`jeHoVshS{vWxn;3dpFo=$~vz+s#r!R*T~>NuV@QH5V3T4_c20s#va`A^s<_yNrwc4|96X!^|)EOg|)-DVM6}xHWdCgz&c9u2?~9Z``{g08TUP1HLJ>-8rbSrKzxTCqOfApE%I)D2wB<x8oHZK-Amc!-&psIO@rZ3;>Bac+-_DD6WUG45#Pg3kPTDcH7G=HoR)xyKhKK*Z0%Vs+>51RGi=1pO}^~+m5SgE<>Bn@7uPO#%VVgW^O5`9)IIHZqK|^g=xDeP5I)5bQAwvJbQM43<UGvA8yA$b9h?cU({2M%BB59gGoJhUq3!vA%v~;wj*Nfvm{Pap2{sQIy<aBQbcULQjC^B6KbEv*}3=Lv~AXMopDd@t7_D2mUTz8$E#OAV7p=_(v8TaiThdb+cMT8g?V`@*>-Btq7vJx!%R72=_Yb+s;5b=;cdmiHIZj!cfw~WkApGUlTmM2Mu;0!rA57ZKB%yuvFT}@D8>4(uD#_Q3|F{DtkVTR)vhd|$S0y<O1+loHIF)~Yu<1uG;ieg?a`pEuVrglY-)?T+}={n=@vrhgh9mRP))5NAHiEu9vdZg-SH~eW94ua?L?I=fn@PI5W8*SNE~y_!YJMuMNzzxz)E2(9NHGb2%^XpnV%(v%DX$VTEa2Mbv5Pp*2kI|B(s*UT=*c+0NWJ}E3-`j3tm$(NBpei7b}QhvFaDhkS{UXG*U}f_(9?7M^UyYN*MuE>fU`u(Z|CVCr8K6pKa+^1&O@6Pa@9lN_e%rSiY)b3={YYCAB0!!<)%q@|@EujO1kPu~m#UZ3B<u$;pc7yY(EW4qfxt#o{%s{D?Z`aw4Ly4SLix^6@^*4MfGRijQzB>-d;8xe{v-2Lw_Q^ao}<O3+lWK*u&DysB}EgT`!N4S{83q9Oje<kP~lxPa=TL8d^~Xem!&zI@u;+0r|xZ32>rU?8GRc1m&XbetIvLW`Dx37g8dCZixdYnqRxmdPNrZjd$YWN3~m)fsxmePZoL_L+y7J#ftNuv2$5sT}lTM@`G2pz1N1<x2HB)7^HssA=p~fmh4aD?bydr#fo2HRrOF>8#D7hI45f9$tZbufH<QRO`2DJ6*}81@I)}5zY52Je1Z&R&WJF$(Kg-!6qjwtAwHx2{Xu-t0L-8@qXzW?Tv>NWf-~OtGnYe!?t<Bhq5O|oWR_27P(x)*{#AwwbSMsaUs*G@dXBkTRXNuqpPMZCn9UPNUImA^de46hriR&<^?qkz;<i9K;#%&i(1tWVlqVmm03)A{(`)vVyy3bHiii60K_#F!?F2z7IXZt4nZW9jH+p5D+C!qt5=t|luXQr&a?&ZOfW_?Pa3a9K@Gf_x*_JoZY`ZiWtKkFRHxjNToga0Xi$u<uZBp?w+FP`9Pn_ij&y793~0YEpt0+s8G>!qsqzYo@&+<-pVC@oS$A`T;ZA|SJ~?`IXr#7RHZJBD@-_|C4Dw{gR~bgzVM=m#9&yxZa~OC{$b2-m+sl<<-{E9Usd`|!aa83W5G0<|!W0VpCC~gY3}%$huqET$uJ72&pw{*Q7G`mC?A>I5=rW03u>vCxr*X%f_1OvfeIG-S$yNdAceY4g(4b6)!UU2X5>}ws`6J`}vb|@|pB-w^{_QlzXm!-PHE@z^Rpo{C8K_5~{==+~anF|?lv#gGT0<1c^wjpn=E`+JwPVhwQyP<J{>q<W5`<;CBo@eHyc8BUXI0*{I?K9}^DA>Tg=tv$(p*?Htp_<oFC32d0?)-IvMH!Ex9&8FL>GI@OL@STyXu%v7Z6{FXygETiy#aYZ?w3BSjXi<Fvkgx32n`&opaoMI5ksIzwidk^f^pQJU>bND=+k~z(Jofw`CKoel0Snmugb)NP;zMh-;HULqQ`ZzeMos1oT@e*SKUF${s#@eE8D)>+_e##X%NoX4@JD)W^z8x=t84LZ2kFZ}Q0j8@^SgHi`pER+3p_=P=>3kd5V)ln59ms(dalR61z-pf^YwYi^7X2s~i@-pI6?`o`^}CB;jS_J?nc+nW?1`+Xw}OfJSW<GuddZ8u20f%zXBf#DrB>NLc=b!ai1eYc7+;SWp!(Cv!dV||>ZU%}+M&mP=2%kF{c1U*^=sc1LBjd~VlXyC+cK<AFgAUyZth1J&+!*OpziHfQJeH;k;gMIin9Jdml5HXSuNJp%jOg3`Obth&Nkn=qRG#+oyIE*=rU26TtoU0}kO+mGgOPA~}+iy^JuVcoaE|$W0Nt@uQL>QWh;G!qsBBYUf10vv^SbI=&Fu5@iq+4SU-Xxfxp|P;=7}9|hwh?^hxPnOoK6R(lppd(75I_OWh(1vcXJKF#Hj}PDX{Wyw;o%KvhaOzNw+H|F!y8ji_2~>GOfwh+Jj}bgbo|rQn+J=9H5_*i+KS@V<g2P^zh@w10rZFRTeqzJL;39<bJvMj543G>dp#3IAC%%QNrEN;$Q77AW473^Eg0NG5<%n0YBCqldP_9d6-6t9Fr2g%?Nd>qdepB`udS#jpbAqZ&5ZHvup5ei>Sh~{KfUk^CEAlp<homF(Vl_%1kNb_Ln4=p;=fskMm@7^^uV0rUYh=Bdkq@m(1E(Q+4vNbjyL2?ya8Sbx8jXke(S%**0K4*3930OH8;m*DW%dE9#1~eUGxh-%+ZEjDpOO;T3pd~wLG+Xm-~l<J#TOSN4H}tgO?Rv=~X79!ymmz5BK&ut#uGFz1pMxkA3gq;ox!UR|)j~?|u2@0dQtqdnMoKkoJB81O}u2<28Ur@|9IS>dUFcMAEjr&duojHReG1tu_w0n|2Ls-ZC&)KO^5}v%qMJodRRKkuapMx9V%QG_5Yyiv>}roEmUYu7m!OSlU=d?JH{#a_`-6e>kDY#HhDdhWcsZ9Y-Ed61hNS(bp*}&sR%><#hT~lsm+Cav=qG9Ev+Dzk4%VoLL!9>AE(UH#7WR-iefhlG7$hKf0PuUtO*EQE`n|`Rb$cj?`A><wFng`tTE;RVkHJDPLg<<_`zeqf>qXAZB!z900`YxAKh7#pQCZQD9SBPM5^iwWDYes|O$g>%_JI+9(w8RG#=S>bXNaW@9v>BUIqg>*dJ@e>MF-nH({C;Cj9PQM;|w4;LT&s}1+0Q~~&&Gtv(TQ3D^_0dd9gB-MH`ZsSTakN^6OC8Hs>5<|h$n{774f4h4a`ZzGwlwE1o?5j1qH)+!yIOFwITN@qYp~gi`W~iw|di1(I6=74|ZFvPsRfL!7J1R$*NHb>LaSwCAw~`j6`C3jlEt?ca)>#8OY1@$Xt)4hag#7Les^8o4wm#Fbir^QV#yRN1+`~-*^O)l?lbI)uBJj>YX~2SCQ4Uwg!OO0nNkHCZ87;*N>Z=eek-I?iQ5-FTv*~nsv9cQ1z=%9I@02fMx*@ym&|33hwdJv=GF{FPy@l=&)8gdHYTV;Yn-4UMaO*?B*Ke}9qwWl#bD`R9bC);T1o~y)j`51>_8m9(*YCJ#xu9A*fKgYnZ!C$MAOtw~>%!l~UX6oIU>@Axcw(vv$TtPel&wFea#b<0dBqRIl4AP#sI4cgmO|~(`5Vezc|hsE6fckQ9Ud!6d5r3TwJjE_t$D*n?$*<nzf!0x!PbvEQ&r8FSz@*w<pw!VY@AEq2dmU8?|@6`!UK4aXSkB?iH%>la^AETbhNSAW)%6*?JS&bX<_txv0=DJ7Z+F=;c=JM5v+DCp|o3H>-OYYcfT?9Y}-C~r`|h*VW?82Lb=goyS-{Cw)$>01JjGs1+#O`aGBT-1+KD@l=;%6eXAo?sFvZzn2m>sD_DNbIpexQuIZ?-pJ~AY9rC)^fiBxJhf1mjxRBLUIpr%KQ}D|StF$i9iq;!5v5I68>3_3ci^*uIJgQZ8Z&KM+4;UOl?a@XZZ;e}}o}U0MK$b%9!UrQSodpiIcL`oa3e$r+T~K*w3J3b5&WRo#ADb337D19`!ns`&IWub!?aR1LRch3m`;^m3KOFo~YS0<Dj>P*i826s@RmuWTB59_u{I~6B6~M(W)5Ub~evJ-)4F)u`OpBg*EiKK?^XNScw3jf&vxX*nLYp@!GW3bPp-);`5#QYMTfh7^c>C4_r*cN*JkH>dtQ|*5tjfL^2RB-hxP3^p-Gb;7ixLf6<#d}t6|AWIqJho6{Yc9o|NC>v*&qdTjf+aCUf<rte91K|N{mE*J`Lj!q@SyV)9{=YcmCl++L`2u5NC62HlCK*$F)yqEY8AdT;WLRtZPrn?AF&TGuq2(Ebqz+O@9wh3h9d>n_9$dKk?<aPMK@Xb_vlAE^?D6iw6m~CihVRM+hM}@#Pla#0-~^xoQ$gDI;2FEmHs~r+QQ#8+M(V^=7IVFewIjKnu=G>X*@YaTH$Bm?mzjS@Pv6cuV#D^tYbHVIYC+?w*0n_;=^=EWHbFV6d2-2e^wCL;uS;J8gZnBX)!D`-92ilsnFF;qyUn%-mjYJRI)q?(VLEyVLD<*TI&j-MaRGb;T#`vO4CB6eDAFBth_N&Kdm9ID2;-e|YD>6pNOv*qpP|e2%BdTwH%if<?e_>mY~;OJ~==M;xz<*pXGQ|AUQ(<XBz*zQpyV3EeZqb1=bp1a1x*m8UFAV$gHpXfW_B${OtK`bz|@Tz_$P5X0Fy9!pGV?DG;|Xc9bXKm->aS75cm@BQ!$kHa_z2rC#6SPo?}KLAt9=5YiNF=yr7&xg-ssx2nqp1>#Yc=%4@9z@g{ly&G@`R}k}yS@Hk_c8nLfBiRh5~KTwR)&Yg{1ZEMB5}t`j}T=(drwD2Nw|21;q~u{>;>pKdsnU{&cXnm0<n+6s$1Z&20Pt%&HEeP+3*Is;V=0Kx{=~{2sFfkxH^bbk|dOV_Z;v_{rrNT`3by?b1ahNAySh1dF-<bU}w>x^!f`h1ju&%B}KP6D7c01{rpn!0bT;$LQSR!#dX*_+F|~VO_@Bm@%jsrhZbvbRg;II9{^;i41yE_ol^$N+=rUQW^=48rcj87oJ-J=(||Z0&qKljlD9}=NmaxLaGi<w93c(>VzCy05U;;vaS#%N0b0(<M>^+raict=UPjOrtx=+n0xJ3*^#LU=k0d>?`FHgxGH?@UdYXsSBrP?eW@X6foo=B5uA(LqA>!V<=H)Bzkc%*Zy8cf<5&Bwsn~2~cHWPjC{~eVH6l{i~j}v&HFRJ4^eM<2TnetAXR(IZkqXZNM>g;quV`gmrJ@5hM#<=$+tx$DPD{($6&NP7uB_?UbIW!N#<1ZEgbR>Z$K<X-D!V|!f><q^fyH>Qebh!$H8CWV*j6xL41L)*pB9}0+H+VD!nJJ+D!Y8Xzp4mjC8`z-C&}Xm*mz@305fIDdb^w6J95IN9rHuSNp_qTW-$TNL-_t2tn8`s^=p=fuGe8!h&3GIJk{DG%3RY^13mD-ObI{>KS~kp131;BooFUmtgO|oA2tsIAqy`*JEvN>#vj;#Z=oY-8Pufp~zkN_CbaQcz5uN3TG3tF7fapSRD9{wqiauxv^f3%Y`_!A=!Un4fUzjbHXF-S|%N$oX@{r-7XzZ8Q!jK{ubb_vbmmNxj%feTKR#9-TztD68#aES0lIHR7`rikA;Sr~T2unZ1VFc(`l1pZ*qB^G}IzEUD5|~I%Mn1`Wuxlx)R&g>U^Djw4h;8IYaiT5mQ3NqrlD-LhEtrJoAyFqlU}m|nK>h`vp>Tp2z>MNYesKjBLy#{j#vcHQb`~ZMrFbr(t9UV1VSs@I1P=NvLr64$&f)~1D)y}uJv2K&Xi$>?)j@E&A`SvdgYZ1gd4_roLjXPkl+gT06N<_SxMWu=umt}f2~O{c"""


def configure_shared_guards() -> None:
    base.BASELINE_SHA = BASELINE_SHA
    base.PATCH_SHA256 = PATCH_SHA256
    base.BASELINE_BLOBS = {**MODIFIED_BLOBS, **DEPENDENCY_BLOBS}
    base.CREATED_PATHS = ()
    base.DELETED_PATHS = ()
    base.EXPECTED_PATHS = EXPECTED_PATHS
    base.PATCH_B85 = PATCH_B85


def validated_patch(root: Path, patch: bytes, *, run_checks: bool) -> bytes:
    with tempfile.TemporaryDirectory(
        prefix="galactic-mvp023d-", dir=root.parent
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
                    "Le patch MVP-023-D ne s'applique pas proprement dans le worktree."
                )
            base.run(
                ("git", "apply", "--binary", "-"),
                cwd=worktree,
                input_bytes=patch,
            )

            if run_checks:
                validation_env = os.environ.copy()
                validation_env.setdefault("CARGO_TARGET_DIR", str(root / "target"))
                print("Contrôles Cargo complets :")
                for command in CHECK_COMMANDS:
                    base.run(command, cwd=worktree, env=validation_env)
            else:
                print("Contrôles Cargo non demandés pour cette validation.")

            base.run(("git", "diff", "--check"), cwd=worktree)
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
    parent = root / "backups" / ".mvp023d-backup"
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
        "created_paths": [],
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
            "Prépare MVP-023-D : verticalité, observation bornée, routes "
            "pointillées et planètes texturées animées."
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
        if run_checks:
            base.ensure_command("cargo")

        root = base.resolve_root(args.root)
        patch = base.decode_patch()

        if base.patch_check(root, patch, reverse=True):
            print("MVP-023-D est déjà appliqué ; aucune modification nécessaire.")
            return 0

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
            prefix="galactic-mvp023d-verify-", dir=root.parent
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

        print("MVP-023-D appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Contrats métier inchangés : GENERATION_VERSION=4, "
            "GAME_STATE_VERSION=17, SAVE_VERSION=18, RULESET_SCHEMA_VERSION=5"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
