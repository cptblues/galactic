#!/usr/bin/env python3
"""Apply Galactic MVP-028 from the exact post-playable-colony baseline.

The migration adds one deterministic and persistent active-colony selection,
stable navigation across player colonies, explicitly addressed local actions
and mission origins, shared management/craft UI state, and global research
fed by every player laboratory. Dry-runs remain cheap unless --checks is used.
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


MIGRATION = "MVP-028"
BASELINE_SHA = 'b6a36535dc436728b87c81ed6570ed0daedb49f6'
PATCH_SHA256 = 'b1a003703fb1c5650ecf81ef916cbcb6a99f5e139db8fe23992ed5123dc93db4'

MODIFIED_BLOBS = {'README.md': '688b96cd4e5cc33ce08596fda670b757ee3b81bf', 'crates/galactic_client/src/craft_ui.rs': 'c6698ab5a533a6f575448501d4ca81bf27874276', 'crates/galactic_client/src/lib.rs': '0926f9c83ae0e79f529e9b636603cf39e80b5d9f', 'crates/galactic_persistence/src/lib.rs': 'f75a418a72d9761e5d5354bb6e788d01b41a1af4', 'crates/galactic_sim/src/command.rs': 'd2755b26d8d03354df0d09e673f0a93a85a171e5', 'crates/galactic_sim/src/event.rs': '63ce69d8fcae2ba3c5a93ccaf6f6d2e1478b097c', 'crates/galactic_sim/src/research.rs': 'b71da493c968dee25a36e9e6554f9859294a46a8', 'crates/galactic_sim/src/simulation.rs': '7cafc059ba65e05c781cbb1c5973b9221c612751', 'crates/galactic_sim/src/state.rs': 'da56ccdf7e662e69a1e38c48bc3525951e19dce4', 'docs/mvp_architecture.md': 'dd692b61aa48f871b65c3b6924beb7da4fd71676', 'docs/roadmap_galactic_issues.md': '2886be65ca8722c014e12f03f107cbef3b2953a0'}

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
PATCH_B85 = """c-rlKU60#HlHhy&3U)QlEm<sCAEi&X-CJE<(_@U??Q^+(550!WlCo4BQ>2=t+^(?;n3p|ZaSw|H4mcdJ4}*OgFAn##FZW#kWPZs-M1C-tOj1-;P0tK=4Y(wc`4Jfz85v(0x$w`=or8m=pE}-y7bi#GJUeNx7S37rea{aU?7cG_kIyE<dAr>mA0AFR9jDvr3=a?Y4h|0VntQEQD_8sP{?0k*PWo=&Y2inA=)e<i<NP+-uubf+)CpMP#4JphzYN)mg=yjh(cIg-cNXlO&%zKMIqbbRPm{Lu)N|%h5QRQ-y!G0Pz3cy*?l}uDgi2rdVX|iPRJ1D|UVoa$UI=YDOO~X56hd2ml6qn4I??t2b65~CXU#SMh!@UT>?ADNbJj6i`PYAp0k(M*CTYBxV_goxc@!=@Y6{+`j7qTh-SsE@x>`YVarBOb*c|p7cze#uPZFr$ItfArrPe8QwO}yH^T<yGu$~k6D?g=LVwg#LueI0u{B!5otLF!u-q`um@BgRstr*E_6QuruEQtkm`~bx7m~-yI+%B#^B~iTcpeI0D>T|dNH>pQ20hpB^`ZyTC+?vG+M-g=X6i~o%nCM7c1GccbA5#?s_yQ{UFsdj%^HVRNp1dW_9t>=C{ckI_*GjHG1&mPX;`@2DiLt;Y1aPSPC|*Qih^U0u&YNQ#%;l5C8|=he0KO9}4}JZ#f#wk19?&O37=_Wwho{c<AFxA-hWQ2H*Jp%A982uYvDdjz!-*Nx3jr?)&X(&ezVOl%+m?f(RuVqjo74v&sehKH)V=6~W<84ir4IvP@7IAphiPm($AFV1O>_ni)Q^!UoD3I$mo)a$>rYD>Kmr7ucpSxl1e_4ftfGZ~?sNKdk~EsXO<d>f`d?F@){F}%2wA*@9-_@&>)eNBMZ^jtykIf>7sR>@qBAdG9G?pxyGP_RB)_u=pon~=dVn~MHYq+v%91qZEacK{?=5uR0R#etA1u89>25xo2e5S02T44~SLf+$<G14kSVO&ng6qtO!^zk?1FqBScG-BG=Q=f;;w-6VVd7Wr5V(~)09Li<Jb1vDUUxJFq%8b7G_!DUwXA@~lM8=+>BS2tna7NU?LB9cFz1ZDyPQsehXk7in@;aP+&jR>s_#VvOEH{IC3QdK)ztgMUrnbU_8k0ojKH15aTL4!`+wa;DHGqlvw%HWxX!oUiXDk%h2Kwr{iMGE9J<aaQWjhK^KUjWk+I9$<%0%JwZug=qj#=&`V;SBF1wh)puHek>W>!e9B{z9TzT(jvL@Rwm#{eKxg)1FKxD#Cf3*&r=fMV62yncZga3msp1@c?|8c=$U|5Y8EW!23ny-NkZ_XMtaZQKwU|H`J7#DK{inLPEnuSy6EQ$hm?*QL%E;u9ly`1@rsq_6Bf&NnB{;|ME!f-J%D4=7OzdL7aapuk6P7#}axP-@7G{#|<#yHW&_zgSvfGz8T%mI6jdl40BG@XOT&M6Dd<vaXm{>FcNROeohFflCouQ<H#qL8`y>e<i=Gumk8i{ihwzeudlL-rnkrhy+04sqm%a(ZD@$=drl)XAU`M?X)Udn?#lk1zaSaf$_4e9U=v(!^2jVNGC?3j~R(CKpkh&Npe&;1IlJmuC^Iu&D!t@#WW>G>yV1;d+yP`5Sh594*-6#{ws?YBm9pI4^sueoK;)bB!Qvd@A(}8q%jpF9Kk~{)Ox{D@urdrd|~4z+z&suU)7bFT{haeZt?c&+#`=_^ZSM{sRVugy1}KJ`-Ob=3YuFZO;SfvC-7t`sb?lhdco#m*HIW$%&|Ok7LKx@Y&A3Hkx|7G-`zm2_6xuQA*;atPEdIhm)S$MZAnhLR2&nb4CQ9qoo#l>Y-^ny6z5byP>Zx8Z&fO{Un0tONs9quoVRn>_CqmYiJbwW?RkYF^K8WCgDp1o3P+)emPfDSQBBc100_L+FH9bMqjdKR3kg0<2zt=cRwJ)=&eU2TiJ0)r?d++)E3a>BeN|LxnTeisNxU#rrtetyN9slb;fRYP`yB}1bd%*>lDO%@m?;!Mjdut$*FMzAxs7+%NiRYfL<~U?F4JXr&&c{6gO^$@{<FC$vv$RSQQJ{{Acts(NVYZ()k6i)P4(6mq{==kqp)v7vo@yTr_B>Tr`p8oJK1qezz4+FiVecN{ZuqEQfzrz1X`b&__U;kre)P647Kr*#oF^EN*8sSIv@<>7bgLe%9FkaOHTQX@F8B6gpIC+WW57cRPd;Z&?U7q<{pnhEV`s=JH#!xWXl_RGC_nQe4~H+Oai~Yv9Inp!K->1n}kF?~s7o8^8x&?034Fi@@AqI$yBg61Qr^;!DM3$|AR%%aUepGL<Fd|10k-MqQIzna(!}nnb3~hpT<5q%qV9XDQ0$3LJXYEaH&na@66hdpK~reKDJx^#Yion9O&6;-C3}pI$Zu>(0GSHXyjimqPTaC2b#Um$q9R3A~XqNgPL+Oda56@#U9|dsOTtJbMuZOze>Ovx#+@63Z}IpJig4CPD%IO4;yAs17#~=MLmboC`rAKmUz;)zw>Ds#nO7*+vCA7Ggl+s7ZU4!dP*S!s76hbk`oKAj9rRt!;g}iJ9c3?Hv&MmqrT*+y!)`EcS$D1o49FY#B!zG>zbI*kJ-cWEtYbko5r~G@3}nOJqD%Fc4XS3rfSmn6OJ$5X)Lyu+;N|MkyAvj>&kiRhrSrk*qfk!jvIXKBLlErv<L3y;^ze#z*zyqiz~%P%|!Xt-TJsOBT=0C7M+oS4T@zvnaGT;m@(Ro<(uvB#s-Ii3<ep=v0{0j=i-v2VR6zix?es$e=Ww4DA@@Hiy3rN|iMO?~DbweNzoYIpAc*K)!E${OIf-ZRThh&&<qW8eji=09!$Oznw<2gsf4hS!gJrEn!D4LHO+ven;Ic_?2Qk?@UfFFO(AlsE6XV;1ajzW+9E;H>J>wco70qwzpb$XvCAQ0~BlRsshL+OLphl6Y{W(<~S)&I(@efxE@bNZg(sND|1(;uT#wc%<YBLg@o~n&&L7O&yD5FFTBSMj!(k2j%kaz?A{liVSUc#4T@98!fskD%JpQvXUj>nOjjn9wLxE;r57yB@fUrr#Po=Tluao}cCc(|xWz5oN>*-uoMnT|QJ;<F=C~}`hxGC6I#lADw$Wua*kqOfInSWJtg7mH;f!028dVIq9ig>VHmYf)k|GZ@E@%~i^ZZ@9fM8y@Az0rbKFUsE7{D1#a%|L`1dYrpX{?neU~@g|5MQDxTdom{o#i%E;{q<<u{dGe<&YyH<uTLgNSj~*vi78aXYE}Hs<jgE5IOO5`sFJ&|8n;DaYIbR4aZ!@ybB|4RgT!scXJaoBY(p~wR0=aH>?4+T7x#y)XCRQs_6E3SYxN(#N=?!OGnvLZK_(?RB1!dAB!2RB~4{~fEW6x4HHCE#7UHJwO!5geJNN-$)ZGt>IeyGa@cdnPHO@Zy;AV*l;Q<C3%g`PBCzFjo6dj9h8D0;BIxR26{pFzZm9Mn@|V|=BCIBRngwsY4lZXI+%pjl(8wl9vzulQGjFN1iV9P&VNlb&YI)GKwywl?gjN|m4gHBm8aFaG9Qh}8-2^cHB>P2t5J8FoqPkx8z(3pSVCQ9oZ@n|=4bLa@cANDU^U+ziwr^duvem6F%8@oSBzGZxkXJ`IbB|VM{&E8{{&O#60c!9MOB}({`)hN*NlL8m`z>|Eo6caEKtDjE@6?caz825P$G*);JmQM>s8{ADt{T-}&|h3SdI6AnU+GQG43b;D%A@`mLoY^sp@Oh*vvM-LNQ;jg?#pPo4A@iDp~MJY`e`6X!?7xsC;#!(Pg3#rS7NrrPdT68kXkMt{;*!g-a>Xjks;&Zgn~oH!*S-I6rOL6)6AbY<1rx(!Icupb#?!@GJEQr{bxVDn9b2;9^-})M^XCGoMPUm*4YIEP~!#`Ha$%EtW6afjiOBxu{xcG?B~Y*e$y?!-}t1C%+5jCX>_~oj;+v3;V%Pa9{*0KNxVF3blRhi>vY=VL;5>z+NypREttu9mC?!%VPO99BAq&~ykI(g{=U&2+d&}IwbfcF7PzuhDiW7u2Ao!M6$*yc!X(6kWzFK~XE_zsHIV<5md;Ha(qC^p?zt1E)f-Q`ZntNe<O>w2%=0_gY;P3)@VD!W)1o^KXW4cSTWQ^L&p}<!TfT)77ZCct0J-;W&YcYoop;+I`pW8H*AE3vVq9+@h;q^yVD!e0L}~uRmR%<YNb1E)md*m0)xfReGpak|Hc#Mg^KSJe-iZ(JFGBzsgxhDoDwd_hr5nO9y^RCkc*ybfCv@rZDbKow%}7l4&8N2L72IuNEndaDa0hOi)oK*lr(^$Ub^R&uquA;$wAaz2+ACa%T<qyhx;^q8_9i`$dpf%;@thyAP%!6m+}*9X!xq)bV^{baw}$h<4y+uHB)+{;5WCKQ13|6O#tW$OxWai^6gg}jOlfB{!^5=Qj4YrUE_Kgk_IRrY6Ah|rv&FjEoG3p5p}};Cq2La*X$3MzYZ=O>Q~Dk-K~{Z=pY2T!`vsrFc2dzCWn|wgbH~_FR0}<=`YqP*^=Rg?qWP;%eQsl!aqV6CjEaUO4g2b72uJsDfXYj6G8)|w-8oiVwmB%nRGA|y=PGF$49XPkt0i>g>AKqo50T8st5%9qvqM`4zf!DMG;d+c?K$+w8nxQXtC#umAFVZX-BZlogceJ`(>dHedr}e6UH28Mw9H(pp|*MJ1y$v*Yu;pW73`3vuT8C;bsR1DUI+c;g(ZQzti&t{y>)UCr4lqQ=Jq?>Kico~2ix&4MX&$FKj&Rs04*TDTvC-tAC5WF$KCBn&&5>y2gNu?TemwPv~_#q?PxOtC;sDNtv_~q14P@P>vp>WZz)+Bi+F2jp=QM(eiMri_sYck%&Bp6nQsSiFEjAYN)c&qP7@e&PWY<&%I0a2vkjNiO`Y+Y#mP@*@4SEvP8tqPHO%H)C%3$gXNr@`>6%4<z4898UG`TRNtgq)xbs_f>dB<3d-uwsWTd^{luJuj6mL{hrb}IM`oV>Azgy;NqftxtA{4#ysjLaS?kCs8(wl`h=$^hhog1W`g*cRZp+rPhzdIaqHa^Tmjyvr#=SJdZkzbOn4JPO^N!hBA&<7`!$7c9oN!UXE{kgBN2Z2Y`EZRx3J~VGt0U&f#DHvq07r5YLoQI}4Y^iOJ6A^;^)mMT7&0xvhR<W4E(V^Sx;2e(nZh!FSnM6sBReEKyIgAsjq`~?;xNIerEFl@JBE2-0NFGasNFmg%u<>Y7Z@DeVzWvz4;BFcCmYEA?+xDTaY$4=zqTpo3nsG%s*_I-@god+L(8fUfo0P=#D6cA6tdx%LG{mcQg$W+eC|O|$D1xG*DO+`D-SBX!2)Uy7N|nTH4<aUtxJk>(HzkE<Z~ZWhW;`-TdwT8%@Mva+W*b)RlJLU(YP5J<5#E&-zhyBu>lGvgDSMZ>Yxn>3zy4|8;&{zT^p;Bh!&b?1OfvH%a4@f&;}2KjOpKE5V(({)h~{)mN5t|iU1`7d?<FC0LaY)KnGZ@+b@z|IfBOA*PfkvqspFBcD`({51&er41JbI~aAa@4jr_2&j~z&%>q?6=qS&tD+7mku1{Zf>=jnBIUvXKTOqsc@v2uF|U$sP69m?m}g88xr|7wH!aGl=g@=mQ_KA5%+*G7B2L8gtC&)@As^Qxz%B1CVyR(o@eQ>mI;TDZ-&UlQwz`~QdCjU2n!I7))DHOWAelncVMRt%E`u{2jqJrc829xmzF!X*#N!zG_n%f(a@0xTouOu=!tA((nxkCTO9^0m_+k>b?rXZA1Syoxq&8*XcFn8aLZyePUWOUcODa<-*v-TeY*T<k7q^Q;SRW~XB={mF1b(=ZuzEc$C4ogr8u`JK&dA=sL^6I|ul6``V0t`2_H#v^Dvc0}%;dL6qYY1AsJ`hczAGJ%bam#HJ^^7WD__J<vQmJL4pMZ{nRviVyJW{Ox57~!KbtXJhY(ZZv*AtLMi8P8xR^=ZWdoDvq`l&}Ck{$7`lzt<~{pHtlLB#+XuU^nFEF!Io@hgGAb;Oo^Zm7_;8)+E6cT4InA{DSi|(fEd;+-xu|CQjHaW+X90F_|NHN}LYmBZJRg7J4C!`5V8H$!uBOJXf$b{;>LlsnmFE=U)?PTao$|VM(3yd{`0)Xi<U6bUFL6mrPjB3;k`o(66^+aY<XG^!&+8&>wl|;Ug!AmYS@Vy)sVA(ld?}ktJ)=?@(wQd>LQ>jeYX2)L~66tU`tSs8zlHt<aY-A)WIPa~@t$_DVoU<HJ=G9>G4gzMvd}AEsYj!GjO@2dcc~-`;^(nk0<>m}4LYeEs_nZGG2&T!Fa%;pzdsJLP#aDf?x($)3O9IRWL*Z!mS_JiY#uJUER)%rg1lh}a{3JLb6);VUoyK-OBYv(1vjfW1?YQtQt)%6vM^zrK-cfsgO}rC5J+52XAxW_wE8e3dn8loafD9&N(3tO75py?f_lw4D}2ubB{n2qk$Dc!CJdwW=3N)*w3MOt87duh$ZcA_oEm1<iE$wbzn?iSY^DMPwQ5j7BmeQ9oN^#NkCg*eK?^NLamBKar)6d0|~0rVaL7p3Zq>J@`HoOtWm0%AldpI!y7wBN9X^Tt6h&4%=^9wlYKaDBsOW6pLc-$6{TqFv~5TMT(=%J3O?suXAu~zjn4=Fm7z?S*%E->q6`Rfxu_JY2VmT*yY{|Q$eba4#@~A@KLKe<@+_6POTww@u68`?-rU}!_@QYkM_AtEqk-ks_Hw9mVdJ>GoGD>rz3&SiDvD{$x<^-66B&927n3ii|a3#*h31ANtzj?v@#&Mt7Ze%XqsV-LIA|fuF$xQP>DAI@=b+&FTPZofU?n3rH1ZlC(#KW!u)xQ%gU8&_aAHPsZ^pWS&$F&Re5@1GfPEY-L^VxP?~Cc35rRLreRCf3|B&;GN@tS1S5xy94KW7aEb<jHZV(=OIQu<pEj6wI#q^lY`^I|b~?rAMCI~54;V|8jm`sPeWXVpZ4udIr^UAjpT9to5ft0+<5`@+=&-Myw=JdEve?NhU5!>4LO?eOx5H~FGfXb~N0iz5IVN$Ybmh*%Pu2hc$~TxP+*_|Vm+W_xQu;e*9l?i4bB;N=xk}eItnW(lEZE=ozhUc*4?+#6r7LIZ{5b&7kyBu4_F=EQkO+ZyDuUbng#GS-g_K>~+Y}O%-|e{X0s?*D1=~u=ApqezI6eCM$<rq<um6#{zy8OkPriNf_0toPrMTBY6>`|^A7&zwrmb+zWtts_uw)vs9G6f|cAn;Z(NLP9Ew`jn-&85C#1+&@)rgLcyy;K_;)qxXx6w&Bbo*H0%!h)**tVwVOe%VkbZ1d5AQV>YCs6EEwx1<~H#SvNcJ&h-pz-V*A=1U;Cqf@CDMq`v_EMMWje-3pjD8N-V#yS0>C-m9$hx>mj7*w%3Xc~S$G2>}RW)x!ov4}PhsTG&U?+Hdd@K)`y;`0A?5(GNpYnEkox`#7_Up_?L|;Dt5j3{+HRl~b@wre@+SZd9s{httjLg56uekw10-ZiHaZ3Uk>~g723IatY&Y>42(?ICV?GHl018L~oUrwj1ck6tt9yw)mCpw@Nt4Ugg(~<<T`#F(&Ime5W4FZLyIGwSdbZf*Ghmk>h<q@G;r)A+H=JE8*omV`3PzZl8{p~rKtH|n0Q^*twZU<HueK-z3vz$}NVE`3X59I9zy3(EFj`KW<(}Pn#2mo*U&E~@r66fm#mq!mX4Ri)>zqf6jDC2DIEbSEIf;esJW4SVfl$(&`9UokhpwQ7pFQ|5;7*c3f=LRQ}_l&&Fb=FpC66b6z+A^zJSQ2)(NM2NT*8!;pWqTA!qZ5~f3yRu&{M??&xngqR#lRBPCY}$Ikd#xx5~a(m5YVETZR#;lgZ)&Ic*_=Xq5#GFVd@-USKi6aYk4q8YmPV5CZ|Ga$Q_5MYpExW`R>190bb$rbrfHJ@&dyZXUTcfHck377?TEUd%a068oZOHW%N5P@K)Bs?H5MNO5A01QIpj3)yeF#bvkqssmIaF_?F4tyN)rT<@~)e3y_pVK&`4y1!ONM@&TQX2Hs(J+-`SAhlAdHRGkl~q$zt+ppwFrQRNU*R?%PhS$Yf3OMk^q(ZuYU78zGN+yzSVdCZavGF2y-fQK0Dg9i^_#mA^>I=w@Ay;UOlp8q?x$O4``Snj~c{1|NosKxpVMg|6l%+f4w?;Wf+XI#T`PLE!l%wC<mIDPW{cT;Bro9-j%0;nX{=OF77nidgNUnElV5Y`;UGuwNuEj<iq7gCv$zCpKw>l`EBah>OQ!<QGF{3LGBk;#)zy>|@7d+CIpaJM3Nz-r~5ZD`<F6su^r>_|WETPd~^Cl&5(CCkgR&wLCVt-PR6kf2e$3_85@j6|E^p;&VGw)O=+SeP4jcxS)D@7BJkRMwSxDa;X~zHoT!x3=`S9^LD8Go+#CS{xXhoyXBi3$Uq<Jk4gK*3sH5X4(*kP4saSSN^G88ZT5T^2?$XEScI)0r2QldT+Z>h{v%~OvrGEDC`f%ITYgEWKqln2IT?F1aO3y6R0A*fs6pF|4TgyBfh5usX3iA@0|gtp;X%%w_iLjt-H=l^k&VJ;@E=UtX2h*6@sDko<*<Ov*H`kI_~xnt>f;{JtPCqDgrjflKiQCeS5J=czM_|%S^I~-@%JyMwzo`AaksXf?dPh%4d0J#*Fe>1>Po{Ul?H?W%7%+c!$QqhP=CMI^_hy=1HU5>o;qNz=nJ#e1}diFJkC@Qka||5l$hE8-%NwVuUVe7Hr}k4<;|;BYDj{S5+(=6fEs)=pHuSY_s?1b!qGyTMl;Y^9w1VdFlVLBejBrxhPeToJyug!d#(}?9`rV=HjhebbDzWsIB$4>TCT)$#OSA(ux_^5|p!5P1}UHKZi2|tFU;50dTh1pwciCy%ksTjVxA0-RWjjT|s9u3OSG2$z;;BQ7x*`DX^NPghH$f#h6G;Pp`I@>GTQr(UVMI@e^dIqJ$5k(hqz-bQ`f|p)TZ@Q}Oec;&@zfv>F}h)R5B{Wb#OHqJGA0vB?b7b37=|Ez@^nt0XzTb4V#ZyPdH(t6129O6{^-HN{|hoPyaqE;c7Kn!XwIGVAn4IeToK8sj?}opW<e?NsY+P?q06%Hy!@I$N=j(AO5d(O_`aYq$IJ$#@RCex<%<X~wRrSqkzyQBh4JEzP*BRO-ziB%QjBh-|^HLW;((G71*BJ6ZXkEO=A^wnk8(ws)Xs<K#D(=59JAsUDNAiGvlYhwm{{oSP3*)owq1>0_?X1D>mLeh~t!OM*4iEID-@ZPJS<7IKNWclFNT5kbf~pvvJ9Q4^?29&y=>vU~ls!Haws4$>a!lenD4{muSd-!a7P+mx|%;!dQzO=(p#kw#x7DspD3Cdu6^hVf3b$=u83)+`f_EP|1!A*1(?@w|xgor#B(RZ(JJq{J!YH!_>$ws3NIPS9_Z*KTp#A_c3i*-9-zyGfVFez!`r?Bi&Csp;CTL;7}0;n#2m5+zBDd>7#n3Xn9!LTRW4Bew&wXNcWGj!nKjn$KC1G>W8(1xPN~9q3$!<$@1WLauar>s->{#jO>V;S1n+0_2i;;6@K%@yL>4?Q9q{rtuScgfa3{a~Tz9uqt12Eym2Vh*q*?Swt}9VSmnslLav4QMcb44C*mu)krl<R>e3&#=zn(k31>4U;2ZFI4k#U?ZC<Fvu|GggzRU2a)CxW{^(aKJWD=#u55QN*gA?+SEfhigqIyevgX`$oni{s{4+_vf@|wFv+EIM7y4y#lC#ImGk)b-HT>hECBgKMQFjdjql$R>l=R?|$Jzsdmt4c>CkZyW?dE0dg>nbtGj$}Q)3Pqi=UT(^mSPP-@{vTV{GGK|yjtA8-x@pYEy=^mpH$NZ@!XnjM7tHla}&DZt(wGhCv?M`&J)k==!UmwC7!#Y8@pUokc29CHIY%jxV@3Fn-a^z-*G_*QqeAQm&g$jC`l$!NdqjwCgWlhu<ETD9A%^=$@cPM{8DkhuL>_Jc`gAdgzy*-WI1hue+Q*(^KB`r5lX}U7%J7;Y}M7)mHVw$N^H4jqwc~xoWPcQ-tElilX_dO-cI$dtC!U7q2nJslnzAKx3drv>Ec=UFJC@xyz_z$Q#V0Kq3ES<!M^teYOyG=r=TaiB-=HT`22j?K>ZlKH@XhZZ%?u9*LlrV7zB3M*}unJAFh*cCX3kCH~73Jq#NPOQu(R$YNG+k<oMCQg^3!kA5!Moz&pdN#}PcCyV(4*4Fw`Z6pOG7qBAd;$qAfAn{<u-@2wFDMVs|mA)MO77<PNYmb`nx48sl?w#V(YI$X1r9|Znv9r<CJpv#mWxGA>p^1f}pXxaE<r<1pcRmL;y9KujPV&SQnAU|eX!xnf=l{vGW$dvC5o$9937bmADM=y?lJ$riewJ_uxZ7`tc<)s$AU(7El%FLFrznGmzG4Cm9m{61jR@8{5C}JP_Vg53_3Hn7<8~!H~TR}?T17&G(Pp1V<lh^GvZFP5@gTAeiTTDU$j-6*gv`{mS<hCo(u;GeT*iO8Z1RNML)NJ(YIRH2$P@iK1m>FQJs`Cu}rAA<HhAH(^WoY7T&6{yY!6w8beCQzn>D!w{VPK15<&jyywm#o))Z5WDm@D`6Tsg6;kLKR_yfc7ZeX$ty2CUw$o^PpoU(c5%C#u`)IW73xFT|p#4OEJvMXmRUj_ph9-zJ?$c&UM0HpiUig+cLH^L-jMbQL;Nh{)rhDzDDGluKl%bWFtEqWF*~E=(|)s;js8jlT|Hi1SPHx5(5f*aqMBNvAmisFxuR;^LteS#!LTZ;zgx%uZh(y;SildJ0Haw*a}BB<|*ga{g1#+@i$Sr*uPvRAf%Vr60;B#53N2`0@P0OPH<-p|X%ut969rW!d5zFTTVZ+k)pYoBK&-r%-R7<5C7aPOmCeA^n-57NB;7`b)SxowLx3{YW%yK9<#sI=B`qB}KX?xTZu6`Rs){Q7ErV<IZ*!+o=(dTX=~Cls%P^Guac#$Yk4Gtq?g9T!%Qj%$T4uYQ#dToRb19TP>F@MdK{wxn0z2k<i6eZv?pVrvi>nWAJz&*Wr-~GW~|^T%5t>?{cXr6nNsL`I{#=QBRBK26+gZ=s^H#W+TT7H+f83Rv#}8gbiwm<b{`TgM_t&s^Y$E(H=P}wS>>v(MXsj?WJ}N2!Wu#q+25sZSAR7JcLSI1FoKmu@;`2^K5N08{b+iokBZmcN7h@%K2`U8>c=mZf)nq7BhFLk;=xp>lm&y{=GBn=I!sszjtEYyn{OYduP_2vt?QN_fD)^H)Iw0_w8A?v|<_j`_5RmbPW~w_nokAai^-uzi-F7d84KL`xdNQ7zDQR@10n;NPMsx|1M?SytSSAcO~oQZSKy$?X25qZ8!d1gLND2oBX?uS=VaKB}>xbNTjt?=3HC+jjqR7BaPKt_8uMRNS!tZXU@${Tqj&|yDWF*=B^vHh5VRCT@g#9m*IZhdVX_)n41+XdtKhQeu8F6UjI8!i=UGATmcGyEO*JImZQBlA<KHdBl8=Pb2pm^TX=I>mM|kzn2|#klXtV=TqK$jG?(QkceARdK3*^Hyyd8HcAuo)bN;v|_fc!dhMQ%HZohxa&sisQwv~S^xyKa4&iDmvSo9s6fA-@}Cywo{X2Oiczwc<#m(;cOzEm<@ytcf!GIH$ZXHTDje4vLmt*Lfu%$mR5jC|KywMUBRgQCkouPg_bwOd0=b|ZdliG#^mUt!jH`%QUdc|t)qqwb-|`ChRqvS!=1Bx_m#nvD)B&fve6mh@H@H)o?@?S=?B%zP|+lncSA!<=+K2cD(K0sYu6Hs@9!zw!cz@Uurcg7TJibD@bm3AlJ{9Uu`IaW~w8rrYAK@>bg{(Fy=zq&_jS*_GT8l7An)okeBP<zs%bCNST7t4J+I%bwT-^iy|yd}dDfLdF+N>r(;~-;r_<WZ3{k9HqHbRcgqVmFZ6lX>85Inpu>Ync>Jdty#3Ni~E2d@e`i>B5fg8HFBN;OskU7RUy#$vqyzYtLAtrY8NUSxy(6~+)}+$I*SrY743m^!$RT!COg$~YG^HF8&V;)xDwg8v6-0Kq(f&+CLnk9WD*gxob6)9mbQ)B@ui`b?&-kEslJ;<qzY=-5`1WEz#foYD!rAUSKb}+L3>un!&?9&&pwJPwhq70c*ad)%X8~CyV1mQ{K3XN>fCW+Js?5+<Rb|YA4_85YYD&BlQktmiJ8+RJ8Sb*aZ;6^WUB_#5c|E%!`9p@j<j-wcB29U#l7O)L8nLYxdYHex`W=`pWVU~{O{L+4?K_}=H+SqM4BINcvyee(;!99vu!^$58c#A+o8@jVHvsPNSRCa&7ybZ*RFRGKbI0`_i^Wr+8VFTt3kA@mQ<CGj{2Hyp<JS(!DI?k2|_WrTvEOx53f637{QBJ?9=-tfag+fhz|8!_8c$SBip3tN$q+0n|tH8RLh$N@QigqzK|`?&KSCvtG)OZN)yNRxCNxD4Ha=@Z-Wx9Rs(@0j9N-E`sI&0);*(nr^f&-?Pr;1^N02739yC`Tclsh?xTsascs|It}fUX5l}&6u1S*ho5;+&21V`w1rnK?qsCejV>Nt`YFosg5{~)Y+qeGhms-`XXHR%+tzAQIuQ~u)cHQ;g)~L4yxmK;Eo35$4Yb#?xHPHUrvJguT*p^9J{CsUP^JXIaNEY$ZXywnnAh`T<NmzX9ZxD_sa-htz8_l-!<&q|Jqd*1seONCp?yo{P{6(VfZXVF~i|*?%;Je${^JG_l(@|S^k?L&muq6k;g}nrG8iQWM3T!4bvf3CI8hWSB9oXZyoVB}&8mxJRstkG+4g&ibShAfGQ9B<TVO8X`epLZU8T-Dfxe>P8hPd<^O+)u9Psdwq68G*o_eHq2yw1*ENUYvRPGP;4oVd4c4{~3aM{7!RqIqZ+5y)zwKFvVI_s`LjOsZ5N<>-H^-_`2(QOW|$qkObMD@LtMcsr)QqXAoVtXfE$=RLlBJe}s9Anx|uzvNK$4Ya$A8|I_KpPjrsdOCad<hzq?JH{``qKjw#8szR<)Mq`se7k}Hb#1JorlPu2#+rIGww2C|q2JT!e!Fh^=Nm}3av*}4RW2FPwizB{DI{)_$-U^7=mXa1dU110-sTFz8fNk-2GZ}$)A8A1&vm*(B&d3k_hh=S4!RdbWHu3RFr3d9?RIax@Oq10&3Jp+$d*uhS&S^+LrNtw%rZWzIC_!nLgr13&MCdcY9kUch6gLQ!t2fWjb(J=3p#33EhEii|4!397ULmRD#pH<?D6mx$sQ~EIov%|N(+o4jrtW{vqbk&s7qJS^)MI}Qb4MhO(XeZE}@<l5>TD*QDjj3WfDGGJG*hl$8Mi4-|P0pVNaF1B8QzEJE7G3w)~A3Z2lHgS1cB|OE{(64XGv(A)}I96m`aIMMn6TaYt3kui4p};E^n;$lNUn)iop*6Mw6?xeSLL<8=;;ss)M;<B@v3!?D{PY{#Ft-#kmOMy_oA{(Agf>5*(2UNVtivUrL0J#35OR5D8CH*^+cXDDuu04cbqYuu(&^tO~=J5GTLi=>@Iu;HFv&fc<1ev!1goFZS)*r&wURi=Fm>~VCFPQRn{p1xQM@k!>r*4)!n7#hk$I6yH6L;^#)>a#cMiA!?qB%qRm<tI$nJF*!o0r?;Ya#`$v5ld4nw(>)T?6S#I%AWw&nm=$Wz~!MvQYxbF=4$?`7!kD78Zqb(Z!qW46yl;c`;5@~*Qtp3twBT@4;FjOxKEqRcyK6kF<HIanoz0TI^{b1`TGyH`M@=DK^Y7Wf#)qvrJA;Kb3U>J*iWZRJgJHjY%PUb-l(dmoHNCHDvF@gc2+;VW~Z(ntm1~Sn%OKX?kwBhK$&f?5Z}Yua?|C$Aak&<w@Tsq++Tp+$c^q8b518$6AFG12aZ)tSV20!Egj^<^F=gI9$+>?bg=oDb8!>1_G$s5a@i{>jxUCj-r2C-9#0M@Lw24Q$4e`7F}(COmuSWl7c)oUXOCoXA-aNyx{j~^m^gEgQc+l;1JKTN&pB}36eUv<e&09`>9wAvoTOFx;>8b7Pfkx>&Q6biee&!`E7I+}X;Y@M&+($ygARa#H&=ZNbBba#s6?rMAh|LYe50)<bA`NdpqdkW0kenK!sahiGVXcecaVW1$nV{3J12O%EY>HZoR9Y`t-aWJQ)V%LBThbGLDE4OHMfqVbquvr50f3_FL9#Gk(+mH>BUsd0~&`)l!=s={Q@hdZRaUgqJve?<O=glY(TiewzNS}=+5=0B_AwZsYdQFx|0z=nzJ~BZd_-DK`J40(rAO%m0kZ~uSK`?In*28ts8}~N}w5<i>II%!81$=gg`CDTu{?uTmTWG)VRpZ34D}A=}x!ne_H|Q8MH7ayxugsf)=q0N>-dY`jlE&0U$b!<$Ub5xD0&mL)SuSa}k`BU(6e@L}3noMR+eRUE&KYhs;DU>Nk?ByxD8Xm?(zuCHzvl6&cfDd_tmiHmNU02H1uMS7MNPlG<xw$4uO?NH@I&KHFeJG1{>}$wZ-tdZQMu|DZQQcW@H^PtyqREr8CDP5Cgsv+I9NeFDpMKsaUzCuse6mzoQ5#o<tKqV`%uThJMv%fg$8k*1+6lFS=T4tYZ~7c-y_y3&r5B(KEi;<|)wkiO3&Kgkw`m^B$a0LvXH+?jg|jB#^7$Dy23A)|Q-vMeAtfW203&V)uzOX>RG1L!uPv5)~JUk0cUh;xYuO9*4zSQiv2WnRRiS{CLtkT2}a>y5xsx6`(;`#ACz7#XOHAaqn9(YStj3Bmk(<MHr}0rT(ohld_pSeSoB8`>UFQHs=mG{g)<PFLW=uYfcbfBOCZX0T>fu)I(JJaF#cKOFG$4$1xdNZXW<0)T-xy{^MkkK&}=zb_shB7vVH6~dzyJ?iDl@rQlAoI@a-;tJ0Arx<{UmE;+Q=`TY3(fQNA`~&>`k?$XagRfsVK4;<bV9;yQUUKwXB(s=E2u}rK;#3#e?wpR{I<LA&XZSYu)=2RQws!K}x6s1t#zmU0lj(y8K+~5&gjRKB*m^+dc)&~IjpWa}2n}@ku>jUk0M$R_Ks^CYaQz9FG(iSi2{gvM&4IR8WLINm`&YtwrvR{jSQkL-R{%J+fH#uh4Fq_?SO#2^7JnGQwiN`~E($m$dzl1eP#cgz4v+%?<p2Cj2FO?fGBhU7Q~OX}CQuLzYeO*1LC`W#@ykQt7&w?n2u4K$mPQt&2xNi@60E~W{3rl4stwRc0W`1yl&z_L5hM_bk)MA7YIQ75<Z$ARQLqS7x(qg@J5lAj>epUZeapJiK<jrTuErd&W9XExs9*o^je|14c@U*3-%AwoB!b4Z5i}+^dSb@71U;aus>LTfG8SnYfqjM&(*m|Y+(th-uRwYLDG0{_Y@GgMsVCUc4e1D%5G4^vp@Hk)b56?XT0MR#xh}`x^VkO=1$K6UTYPo`q`i{NW$f^*NBkAy6zkG*@AV&aMh|+x(G(uWUANmIRiZ2I?0ydYu@q?%HVSdEJxY?-pJIv_-XtIx#oCUZkgT}@=?J*9AS-c~dcyaF&jAbo`Hv(!eBsUKY@LeRA926=A!IT&HcG<!4b<?Ji}WZ!BDn@2oIw8xm%{j^yxRI{V|yqJW@ose7StfI6L$|<d}XqWf40elJ1zzvIGSX|)6t1uJY66*R&mJ0&8Y-7(T(OX0tG^)`$n<x!1?k0>$^Y$>emVrqS?E_2DVHHv0+2@bY~a=s`>Fx|NWnx7c50p4+t?4QU$OK`SoDI4g&8$2pCPM^#1|9JC2S"""


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
        prefix="galactic-mvp028-", dir=root.parent
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
                    "Le patch MVP-028 ne s'applique pas proprement dans le worktree."
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
    parent = root / ".mvp028-backup"
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
            "Prépare MVP-028 : sélection persistante et gestion "
            "multi-colonies adressée."
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
            print("MVP-028 est déjà appliqué ; aucune modification nécessaire.")
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
            prefix="galactic-mvp028-verify-", dir=root.parent
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

        print("MVP-028 appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=24, SAVE_VERSION=25, "
            "RULESET_SCHEMA_VERSION=10"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
