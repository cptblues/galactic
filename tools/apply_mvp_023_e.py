#!/usr/bin/env python3
"""Apply Galactic MVP-023-E from the exact pushed UX-review baseline.

The migration adds typed planet mission targets, local and interstellar travel
durations, provisional orbital designations, and bounded probe trajectory
overlays. Dry-runs are deliberately cheap unless --checks is requested.
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


MIGRATION = "MVP-023-E"
BASELINE_SHA = "09696da1d444a44692bf752346294ed87291d579"
EMBEDDED_PATCH_SHA256 = (
    "af2993bade247829a5c6275cfc1c2eca49acd484588baba723e3468ddbbf1639"
)
PATCH_SHA256 = "6514b57e2f34560318ed9462d2dafc30e1e797f9d8282c40f6f3e838b8bf88de"

MODIFIED_BLOBS = {
    "README.md": "c1b8ce17c97236380aa2b207f5cf3dc29538746e",
    "crates/galactic_client/src/lib.rs": "c9aa4d3f40f52f25e160ed019cad457fdfe3a118",
    "crates/galactic_persistence/src/lib.rs": "55021f081abf7c09a48f6f443689d6282d533fc3",
    "crates/galactic_sim/src/command.rs": "ae36c9ad3f83e39377de89457894a61bceb3c61f",
    "crates/galactic_sim/src/mission.rs": "a4244373ef1a5d255a689b825974dc8e249b0ac0",
    "crates/galactic_sim/src/simulation.rs": "47b43371f86fbfa7a99f07f1f9b0da5073538fbb",
    "crates/galactic_sim/src/state.rs": "6766779cbd95f29f6ceda1ea5f385d5db56c3803",
    "docs/mvp_architecture.md": "4fae306b831d1d75efeeb2433751b59f5053c01d",
}

DEPENDENCY_BLOBS = {
    "tools/apply_mvp_016_b.py": "1557ff3f419abbf6a1b58b897100aa72da80bd38",
}

EXPECTED_PATHS = frozenset(MODIFIED_BLOBS)

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
PATCH_B85 = """c-rNC+j84TlIT0XqFNnclf#R7(~WAo*KKRM6>YbDB)4~FybcN?K?!3JVB;cNV<}=@_kBNL-%fjeLw~Ygax$|nr~(QgD7*XY#3*cu1gf&Cvhr40Sy^*$v0&ZqxtB2K!SUhar_T?2%Q-vCJ~zBz?tWmi!P(x-9rSwpd)vdEexD8c{hjf+(d~BinvHh5ZPfm^e`DRzerGUXZT<%yJagGRTsmIhc9`$5*juK)lXzjk+yo!2+$i?q#0h3D3!tpKa-k1F!d6ZU-`IEV<y8-$#OyM4;|7#ZFY&dTL@tZmS>(Dd7F?xX%>4Uq;v{V8CY~Fy*a>1b3xn8=F5&C^{^?<mc*)f%!eh%Yanq=QZ7jVw#vaWaf0p_#^8sQKIe&GNxWk}o6sDdpo-h0`N#HN^eieo1ksHS@O9Sth)LlZq8Z1sD4#1nei&+eClG{(`_&)BjpJ9k6Aq<c`OJ`mPFtLA0lzNHFoRrNjoPhcc!%W!iKVXoJt{Ys2SGS*B><Wz0W${&<-2S=*Y~6lJ+!?@U@$DxcKEWsg_%>lN49AHg?-J`px1TO=f5ibfZ04Q$Zl}?Wp^Gps9Ln4SJc=>79~}6IBf*AXgwP`oKm~yd-GhF|-g)4p9{`08Hox!!4_j??z2!1>05%r#?*m4tw~>1et*mfxfMY_8>^-y>uozoTp&$(<S^~HjyK>OzLT6#h*|{H{IRMsKEWFu;j%yB00SMs<^l=$_FzsmoRIoe)8gb*K(FH6AX|K^XQ3-_<D#H}M!iaq*xcxQJP%CDQ@Wx+%N*ur#^TQeT3*omtBw26aA!s5yLODzcP$80T=*7SRFf3|>!}0<cFbvlB30#<7VsC?(Xu)?H?L`Db@@{{P2&d4Y8I*A&cpHS#657sB0;`^bMtjpqU~A`~(XKlQOw0jmu&3cH#mZa`z^GtM6LvZeXYqsO<!b6gvkMQgpGGdol~b&QvSnwlv&-7_8@?XHz;8cAE=rEuPjfE`BM&<%A&KF;(=$Z3fOE~hhrVF5`cz)C(<i6w8ebo~Fu({02@hY1hfgG1hS8aqI6l7nGrsFy!xTUVBAQoVBN(Ft(uBkr6#5YhwXZqPM+WZ#!J+t*^ky({tm?vt@0=K=QiR<2S6+mTp0WpQ*x7!H1z%=e7s4dsvI-?82~aF`Or;2qgZ^%3#M=00K&aHn7X%l~N5I)D5TXEO8qld5cbB*SSaL}LbV`C42To`Krt;zxi#33@fs{~^1KyAtr8p!3A~isdJx-Qkyt+UV2T99?%m)O1;rJmjQJQ$Z7n5K`DA4u=Z+?kl6($_Yube1x&Jk+|H0)Uzt&k=GY1I3Sm0^Z}u<;S9lGyVv!qk=GJjetk5%CO1HmCUjmUU(q@E1f#qy6>Q?D^}L-TrXYJ!Jp$_y6Lv8|xHB07-?+k`TXRy=SU#T94-Ubtx3NxBoZH@CxNf-qM^4+5-$G$m5N6^YmpHCEXLyP0qQ?j!rx5^ay{u$*k96&Ltq90iOa=VE!%<T~t#{H>r)#6O4&NJIQDei4}VW6E;)bCLQanR_PVWHGqjU>JzP}1;sXLv~T|#fJiOW0YHFijpA{FP|e4r`jA`U&*{r3JagwT3L!#q5U9<9a0vrzwDEItwGwg``Eus1sJl+!_*XFrzZeu<=!YQ3oHHOG7%I&i3UaCJh4?0lU~_~egx!gp3?y=puW5t?;H2kBOz`2-ogpJfPH^66OMx)Ylq=U4vKgspoD>ZfHPI*#oxocnXjIH$aL)VPXp>epdi1jp<Of=R1b@L-#LQVFhMgucpU;>ZImoU6(p`}n;S7qbn;^b?k}704AtI3R#h_PV%ry!LZZeCgQDJ5wJzMOXj>h*(&U#tz?t4q1(ipiF^##&OuQAt6(iwnq;|J%C4>pK5o6dX>U_6MU*#qA@>j47J)|Zhc?7riS=c8V4`)o8G@6AmUc0Cks+O98$;&eFd?6EfehOa45psK)4-^t`-gWVi7%y%FN=>`6L?=0QN#CaY1e25}I`}iY_s>6N^!uP&AKX;$Gm#(jUf8|8yE|dqcH2vYl^TX-MpHE&LKA#>v1xpX%Kj+lQ!H6#}FPzv_-;Uil^#R0*D@1^Vd7|WXhgid1z6N84xavwkf8{ORla=evJB{{ct%<Lyt%}Au=t2+l#tFv_Zc?+Ke>!@7czkj=h2O7^o;-dwJ^a(-@4r7he*Eg_#ZMEq7!BD&HtLNJxK1382c3OTiujk5;sWII%6T75V`_7H>ACNle7fY1IU$qD&%rx*60nDl*hjM@{>;#v14(;ezPjN^EQ9&fMOER>InZt%=mJf_<g4b^6H~LWk2hPykH8FVHfWhOZ#XvVv<X%Rhz*zl$Dg9?c9W^^oVkAU9>+^B03sDFwK~=K;j+C1J!&K$Z%C_toP3Kz;BAUl#Fux_S}Tu5V#VD)EIfCB8+XCDk<+*e$=04GVdy8`YMQtol4fRyOxQi0$P=Qy33~|wj?-a_bsw>lB=SH=YaPGIK4xWMkkm)^QFM0d0Tc9k?8Y)|AJ@Q$8lXhb>WSMEb>2zDP3d)0fodI?6NwN&m4p0#s99D;GU+L67<2&&_b|F@wk+j%OIm}LGD~OGyyhjYZ8V8L-hz*;7r}4GkEg=ufL6<@;Ey)jy%}gl*VaiwJ{Y7RO)EfEbA}pVk0`g<Dr=|s_@-rYk3I(i?A(jt)0N<qSfehDC<s5TA|t?Dy^oyLG>oP=#^&`k+j=RjCNH>je8A;aFA1kH(IoUeCsO3li?D&Y{v*KuPke8NegebfyN0D%3MNc<#^1t%1|}_Xqpgl$ukj8p8GyY;+nw>A<S(H9DQBC)i0{f=bva5S`Z@%^5wWi&bRhDYY`Xy)D7sa1{xK&9Eu>n_4xi5oIfc=4g84MLaD(Rc6fx7b^Kp(3ipB6J&g=LRSl+zkea$Jb)XwXW9AoqOr@mj?riI?zY_(dAb^Kd7r#j}P1_j*${~GN@P=(4eHsfy<bYM67cqB1MsF~#L$wr%a@x=;Ze8XAtk(^u<a!p|gA^&mUEmQ5Z*Kl7u$9Yp+gkeo>n!Y{Y#i|vEf&|N!GzwttxpbQr<<Sz*Zn9DEK^PrZ-EFpd{B<@|)gi3Q?Wu<XMqxa$_QV5=!yK|$Q#G$=z&+j^i1wPPk3A3UqKB-A2R^b*j4){Q23NaCJ7x96@UR+?4(O2%_b*Hd!~|9<-rrnPG`EEf3%2#~i|3EQgnok+57=P%7Etmg8|u~((Dv4m_|5;k#^3zc5&td!@b`bh-~87R{|$dQ$hX=A#OY$;;S5k0H0*++Xlf(S5l>tUS~CTFH~&B%T88#C=r+%~I#jZxbBecZ+O8U6Z`eoPT@Vp`N>EuwPVjCjEmAf8aWne}6S7Pb_U0uo^hCaR+mZ@0E8C01D4CvJftYd^uwaV3^9#~dnsu^BdNJ;k1Smsjr-kp-?CpZBJZA0D-u`}PD2-)@tbr;1xpbSjp2CF?EzI2cbcGiD6jw56LpNBZ6zb~oq*+)LKS2-86$mA<symvuNZiz|Wh@?B%D5&nL6u;#I6Z1g`xy)6R-2Ov??y8bc8Rpf<=_h{R>+>8K+<R=%Bw?dporN;9)>2m%G%FFkExE7wIK@RRb5sOB}v7aE(4p>+y+J~`*Tw<3~8>Juru9ZKDTN2n++_hHDg^fM0;PW`dej@+|)x55)9bdW%EC6*<LCmI9p2^Z5N4ATch3xvR_9+lJvi-z-bu+bhY+KX*JahUV*5)fJaku%7O;}wud%^XsyL<9h6Dq@o>}`j97a--rng92SO&zBj^2;T#fEIh=LGwH2{r;{6S%P?)_~Ug5><3{&tFj>DZ0G$z&6J3)5)kJ`yDZ=h8dZOUkx>QZF8fxlTM(nOF!tPY6JXWzO&!tll!5ySgk^U|)Og`{(cyT}IsUuy_Y()R`ego3+h**=Re4cvByntq!}Fb)t1((uoOGxT)_3*X9Ya46z_a%El^6kixCoqYSEToZ_t-Ws#NjRxCoGq}k2H1(Hn9^_^7=8Z>riaRGt_nY02Rp4i^&cXog!c7}t_V0%~RiJ2G8e0TccZ%xSnG#<ePU~A01G$vmsV9KQ(j93_V9oiv@CJYs7VbM`SCA`<Rw(2ZMbS}GRVKG{PXzPtx8b#3KDQ-1bl{s4Zvv)ncTvId2)FzA<*jFh6jA_TCuu=oo-z_{JZTf5R@|t}mKJ`}U-0petl<%H9KCDsh-DB!Ci0&ToZ_kG%bBjGfhip5ifXO5g#y-WBpamj8aPFVVH%;-lW1y%d4V1Rs>zpZt16!+Nd17f*E?ja7y=>inuQ=|!&KKjzJ=P>=w$}}F@j)whr$G`mbJKO{MDJh{rCgC?>##v@u&u2Y<qE1ojNFwCl&F{}!&kh~WJDOu1>=56rckrZ`_QVt3NE0X>wbx_bxfx^S75LrTWO6{aafM%GIk52xf{W6?;N`D@L`7UVhvQ-Zv2%+FPOl%rD757wav-g#0zgJe^TPt>h+4x$uc%7SBy}g=%$qyGbOz)`d6)J_$)EQr$rcotr7y}R$|t+R>ew-cjy535?EY0c6W}|rIT~(NJp`7Lve?W(fOX{X;x4^XE1^iey_oN>F)pTnoi6Hw~L_BonD`{nNAYSiD_&24{Y4q%heXx&ehiEpsKbMNY>>q*}MjAcFfLpSid(Kwt8SS1u^HBsgpEw8lz?Hab=r&_dDhgH*b@x#Gda3lF=l{4%wN}WD-Z`XHF9VBlx{N{Jp<}zlY;itCrH{kinLUR!s?AkrG1M8Co=X34!r);w7kSGd@E|$`h1KLQLy~ILU=`D5Lfjyv#Y&fi4VhdV~lCD!K--7bVq@m>bM1OI+tg{5*|#*w{m?+as?h>UAff7s2^7PS2Xuw0x-}AGAyc;UVjncpC=2{(;r!p(CkY<C1kAWDWBtriH=R*oIl4_f3yB!FU6A<;YPy;xeb->+7p8v%-rsISbQZu1e=dji8#QPMl*C7^w=Z(J@W_P(Yhf44mUb3AE0f<qCU&Ca2Mrif#ob7+)znHK-cGG0hCR?CoQK%B(9!Qf~He)15#~jS<$Xk`1_HHA$4hxeGR)gv%9>xI1Up`8Vj?_I=BnKRTYz{ekGL>Z}tE=8C7<oTn=A;*B)s-^%?rA!4K%FP>@Zn#{n@#KdcNHZtUdT(<)!Yir0J7_PH-D20KO2Kug9RzAnKMGv9AD~8dKbBtgRz0OY!=rjeHGv+~|k5IkI0reJs7=nF)MfqM*56t$ag3yH(uvVXfF|*q6T0-Pa71&0ryNJTIyLp7THv;BcV@odyH>!+VKyS^>xeUW%c41bO>qL<fcC(i)H%&rlab;FmC6_`=!g5$SmLuF+7^{_A<XTj1&YdJ%oG>I(!@XoKQBl(aR&yGEJe0bAfYXQ{<vi+kgN9BSh1#VCCzC$`5en19QmS25iuxo+zFDh(uGQ+Fzi_L6snzOV>a;4i9@7=>8BSBL`P=x%$y>Rzz26yvT|C~~&H^c=c*PZ78c%8LB~sNA4<wBJyz#LmM10I9GF)H!9{BF15hHbA?t~aSb+*VkO%P}gg-Eb1i}hl(<;=?#0*B-tj3uzHWLIWQYpQxNQo<}`i4M}j2kh~7<$*<N&-(<ADEK_0#n#o5up%w8?xtnny)=48h32nst7YAh6=YZvt7Zo&DF-2Od-Gw570PN^p<Aoy_LF?D)hVl`o1$B^0>dDA4A~dKc;nr{fc6FU_WPaj=!<Y#E878BgR7FbK66jL7(cRZb6&IY#jF*L`_k>{Ts&ag8V^|TdBoMb?s;Brmj(f@-IiYy%CwFz+oeH&?aH7(oj+A)GNx>=iS;W)#nooeVq+Gg{!2y;{3i3Cg*sLcfz9z&?SQLv#C?uKu1LIu{p&J1`4WMwrGPbB7Fc*gy?MKawCj<$W+~SXkE>|R{wmJ^n=nL;cu##Ps#$4Xun4CsDDQ7<t)b2SOX@~f6ZU`b&o_K8{gwM6=`i(?tn*ff{kxZ3d~*b?y~M`w%_C{c?eC0G!)y<B<#GNj9@Put6`pR4cr;ibqrq+p=Gs<vu8_x+hv|xr)QOXH+381k>Tfj+&n@?yW)XP5OYEgJAaL$3oYYUqlFTfECPn^t2<K<voVGN${^AEaG(1$C7HSH7@AA_t+rxgpGuWs62`Qc%pRjMg{nm__(9hcFpQ~b_nx+M|X;^r17q<@A=~0=T+UFFMCYeBQ8r`2@jn9$LK2oPwCoBMxQis-odK&E){3tTjVz2O=sKDJ14xZE$r$ObzmmyNF2lE6=2yuKga(_v^*lj4(V<U7DBF6L44Z=>DCCxF=5do8EKLuf7`@2xQ_&OJKY6FmK;ZDSn9mwJoxIG^4QvB8S&h9`^Ah5iPXAs5Y(&tHbG_mCT2F1N_tCb?5gf&TFU)f`HiWm=)u%ze4QAp*NN6F`3y7Oj)A5sy&@JMiA_7Jod9yrV|V~o>?0vk<Ul)xzZEX?*+?ySF}Ks-~E89{h*8R)Zv9|O$Zfaldk3JrQFQgEQ*bEA$%tMm~_Z#G)vUCb?c%F6}rd;bdL7Z(I#-#9KW&s98><h`p=P>A4M$-KZ=hi)}%+2CxNzs@p1&74)E|E{xF9SV@mp0(VX8LX;Pn8jL!)l3!-%hV5lupQLeY)16#9Qy3=L<E5CEM>*kx?@8XD9%5-(In1I4e7HAqlBanelQ6yH#}#BMU;aI54WO{qBE{S^zM!^y~*}&e_ZgJVGg2*pIfDm`Mp5Vp+kIEA&|FxM<g&Z^NmpICE7mMQePP991K^QHSn{%LxR&ek^B@UStPL*%iPWPGe;2a2k+c9GWI}glx@V!@nLqfy)o2$9-um{Y>JMb!pF0j3s25`_n5Lhd@MU5yd@@sCxzgdI&iX9z!SL3d*6Y{1e%OokzQkt>1}w%5VMn<#`d@Iko)%Ta2wV9Z%t%#OfN!NteCTOB1&9=NkL?G4onNdH}gM}z=SEyNvt{5CjPokJubJh+H9lRFK7f9wr`m~F+8LzZE9dC%IC6l*zH<;rS+SuL3q!#Xz4kDEj{ZHv3&$OU<J%hDc77d;{`)TlrUD!2|57*njLSj;K`SSAo9ZL$eN3kIGmC&X9(qe0;N`pK1vtWAMYWx?+o{b!j^r#JbCY|CjXJ}aziM=-hG!lzUh9zg>7?7s#~E+P*}uCyp^s0R3rzctc&>WY1!iVp6+JW&QZ!n(6NG0n)whO%W=go<=HA5sAw`9Z!V6hYBhfMh_i~7xk6IS0l+#X49+o?74wo<^f5Pan0F`wMJ*?<hco5C#5K2`-u~yS!zZt9|8uLQ=Ub?#p_ZpxMifnXjeB=kJl6!hRkc9gGI9k}0I@8W?RCGh4Ng0&Ubjwmgwn|SL8(3v2yygadm9~6JEQ&m&d#_FN6RL8u#r3f*#t&%L5Y3C2EA>yHHbSO_~Gj}AX<{QBBVJ`<R^{eDS88d=cYUp*Hk2d0{dKL>xxacDDTD8{pG9jqr6*sHZMRxztv$|W`?h=A_Wl4ntqvl3%hyq*KCf?=4x>RU$pJPu)6WV$kH^$E7)d8nlL<iK?V1@@f7T)_fEt;_*1@doSPezY3RL^rujow9s-zGrk);^Cr_w~3P-R|fOT85pC2fg3~pqp_kx~J{>V?@7Eiv47a80FVBEDVvD#kXJKJh{;kzP9K(Rr5RXPmbBI@ky979D>>kgq>y_*0AEgJ+B1tp{7%Q9MKSSsoPY?V=zqV>LIS=cTPJBvuS;7%S>F2M-$M#)&vsIyI!h%>u%XJ5T3;G5+U_|}4li*OZBtj<J0!lerWOhjImBI&6tFhS29MR&#)d<n;baK7CYtPSb{7Gr?-II*ByZi76**Bmh*MLTS0rQBj{W)0F9mJGEJCs_m5s0!9by&A72Sc*`XltdWV%1%jNg37xi$Iv_h3AoXvn0hVl()2wQUg6eH4OLST&wWpw7+Bx!?Sh;UA;Q}&yWTS+J|32Z-X8oe-5-77?yI=k->3WIq5?xguxov$YkelPKAr=|tbBM90oPh&X@JTP{HtjSG)$}WDW)KX<x#U`X2nSKT;vb6OoP04@?7?lFj#o!lgaXO)l_Eas6V1i#5?`%&Y)j+VL6MO1sRNaj(Q?rp!j}v^!*R7rZ10Qe0QijBXu)c3!HK0mt@c8Bq$v+-tHUA7roYM1t=0n@r1dNrJ#g9IB77uAa%+<GNtBZm=E=){kd@Shj(Nn=JRT^K}2Zj!s8Bz8nlXQdLBQe#f<3zRoY@+k;G`oNn#}O4%VTFHJe&i^Wa$NRa+9pY@e;3VOp{uFtP8|m>r^LM!B;5V{#HrBV}B+>9L8aIBwZS!IvWLr59p+a@m%6NkwsXWm2+RT2f2xpoq<x2^)2)^V^E64={mwb&^|qL6PZpd%HgzEc(6PV7I@szkgPp>9z!vJ>6|dVM>=u)IPA~yw&sCX31HZ2%nfnsSjoWKe07W`3`pEE`aVXxZx-BxOP|RnaDSF^7u97aXq2Tttn-09pL%82M->w*Vs$ohQYqPOd@9Phn4TmJmty?Bk3BFt2|C)aj(%{)671JhOghi@(B|nJ$290^A3B0flD3s5{x9z@ehA#fmT95n0|c*kieWq<_3n~sg?$qB6zeX@U;x*3{_byS=gF1*i=CWmXbQE@#nkL^XFiH@XWiKTIn!a%b|u>rv|=%jERC@J$|Le+|lz8XPlf*J8VnuOUui3sf91R6|AE1V4Q;e%AH*VAo|ZKqm!hOGck_o%t;)0smX~i)fqGMu$HVxb+UHfq|wT<8H&>tF6IBA>;=*dBM}f^nEpGdQ89vMZh>F2xSvHXFgKpdaYfvuWg(y2zqg)xuc?qND|g#ky>y*!*gjIpZtoHvxA!XWs16rbMH^t07G{1A(FZAyohy@;c5~x5{kJk~Bp=a6>ct7X*;mc2nL3S@+gp6+x~rJ{&-@|}m>mvo%gFXpa4?gxZ3IE6Oi6abJXf}IVv62a9fZ()of6GgQrW}{o%Z6^RkJdBp|wUm0z;720R%6dU~X59#X>?YI__v^w(s<MPJh>RMr+if1&m!KS_+anMDac#;od080H5WpQ+_-(SNKt-=u}n`7byshdV!L@yRnovMPlw$u_58?x!gaav3zp(+}Y@&_%Eh%NB#a-<|L%ZJiM-9&d>ZM7cRNx#f3HAOqZct?~=Tb7CGu?H%OP7mXk~^V?E3TN^x<*vn2%?x@^|1CNQnSvSVx0Ce5W7<;Pl|%g~W}Y$MnDm28t0l58^`j>n_jUT?fV-gOp(dTgVCsb(9kAmx+B=^4`Oa6*q0Pz``>vF#iK&42eQa^2_7s>A5h36j!lclH!%A>$CsjSDnCR<09%ZG-*D`QE|6YU#4Vr@Bi~OzWl)Grq2bO%p92JwwqO(bjoKBP-4nByV<c8w`Fc^IM4s;+d!3rH5P#JmSUdeHguCXIJcW7GY%TG)5jfWmlf-&&3rU$d~+%o*o5+BXe<3!tP<pZg;Y~%c2nXSQw@=ya-pA@dwC7T=ml&^u;|NF>@03VbJPv6PerE&yP;f<ea`b2BZ7g^xt0`|2Tblcs%{##Y-@~cgARX?@arB%r8)^_3Q;+=%CmA?(xZ?tT>#GYuCdzUmSmT^h&il9b#omSC0=*UOfBx74`b)$&V+hM%4=8Nw~Vw)(IU(YXzR`kZ#Hw?{r4nbO{sK5r)mGgp)XwRlkJ}`@xAXpq;jod4>d%&X?SZTCT^7M{&wiR#hjOMn5!d+UbKc`;g&z5oGN&W7l8sAS2P9-W`!y6^i8DNGI^j77}_b-lzygEd)cQlDC{Sy2))k8ogEH&Uz~j@k=y3x<jvde2m3io*RKB#!}0Ioy&NBV;(cvh*<T7t!=S^ud{`Q6ouKl{3fALo%pImb?UEdsuSNA))d-1b0;Qec!=nrJ#yg=`umx2V2+XNWlDIe0PF6xOw`vygQde?0uR+=m8C4(N@Yd0EjCj{k>U}KpV3~<rZPfeU!(~b$Rxl%gVxIZi=T<IGS8q^q{IhkU^MUVO9wjwNjpPQ&wG2F!O(^!((ID1h*B^tHpF}$O+h%)_A_G)HA$_%6_Ja1Ic|$?l#<Dm_4W|LIuf4F=TjJ#YgY~n++r-ek;X3{m)0b!0?JB(a-)dcEW`fLPMZeSiMW0aoJ+^^aY56RcAZ2m6*$JTKv8j4$^x1{dr?r~f6gr%o=Uzy3SK&KB0P=5!5B$zi1)NuNsq!<3`|(@X*4D1h(eW1Oa_AxIf)cVO@I0Sfz${A!MO;y*2WHNNh^^hVV=T<!+pByZATd1>yaxO6~siYM~PE86w@(j5Fb9_Sko}ot-?jO8W(1@G9Rd@TefJG$-0Lq-}<n=9`0o55N44~@F|=_-h#;}RT-74XX$KVX3v0*0$cKY079Epf!7A4ESRrb*|6}384Y*qHY{YR0jZ@jpv#8yc}R^MnjiD&DO5U7Mi)Hc5nwX~LsJf<V?feVKjm9AA?pcGjkKmEF2iZ7-fU>G-6d1O%97X8nR9!-TJqn3D5o1Lz}(=;;LAzO*!)EjRcT)M*@=hDQHL(3@dX~-iIeHdNiKMv`VPzES0*qt2_}3u4I}MD;M$`drcYfO!$uP}xBG;dsb{-dY!7`lvRKJ6t|bCZ(o!UuURuOc7SG(@6>~<@3U#(KffeX04LUSYSQ>Cxm1<XylWKjdG7U0=pU`keL>Bii^vTu8JG(i=o)ILMU8ZdZ!R5V8Xl#@ZrCw^1POV!qtQW9teEZgowv7NtmLsmdbHWlNXAwoEc3EkRfyrbfbVfrwH8R?jN2Kl|bI9fPu*%D&Un-?8f{&&(Dr(siMGnAa>NnG!vDkINFi-tXGQoEyre0CRr;LVq0qnc*!}o?vp~P{rL;Q>2C49C{9cfr73{cCjAcx@0=gq7^Jp;pLz0XqL@{m^UjC@F4v0x6HojEIh(7|)#p6(*eCoCP@8LOPI#<X@G?d^-8u}XW%>_1QCxSjz@I$#<_LSbNDo5)*DkzN2pIRetsZ^%$t@(lRyd1>haYO_*ClCI2gMg~}nZmU*n-K75!nnzEBl~{}8DR#SrbB60?)z+}0GlUMU=|m*<p7^aDL-@1CbZKd6x5|t^K2phZ6LwEVha9U5_T&XM_T?o{GRoH!->7AQp?{UP@^cb!$u8a{%hvRol@_o~jA@Q%Re$?5Qy+_0!16nfXze784K}4n1GNGvzxA**&C$B0h~eDH;N@?_xj87@C8qZjm9N6VM%}3AKU#-V{U=+W8n`S=az;zls@r#N3b-)8zIa!4!<m*NxTpfZEK`E8EaAtBI~y!}G`5`?=?h_UJs>X}v0dtcC{ZbP^+Zf+iCSdxDhA(-cfb9OGC$Q$pjOr0GIlHNbOx4mVG><Ug+9y<cB1gZ)Au9*H!{;|>}y&yGb+83yE3z6cSzvZWHuQZy?O_>bw|TZY)c6h>C?_jPD#=_Lnb6fj{{H1&uVZW?Y8px>}@?5Bx_3KJ~kR$15_-G6U>v4MFKQR&WGi~MoPTHWu9SON;Jh5Bagn}QGJg(`Aw@<QIg~@!Qj1<YgOGg;@7Vfu3Ztk)&uk`x3|&5yi*U8oU+p70zm!^Q%ncWIVIF+68<{5OH$~3q;8W*_Ap6_dkPF&OgcSs0F`h#D0K>c33s5~5x9Y0!1M^*%{6FQh2>oecTB9hu_NI(b0TOS8a1|urRnwROr$L%i|NEN&4^6^*^Z8u#MG9#s%~<w)Xlvn8+%ODwJoxt8{w1PhUF_B!QbEO42K&37f6$b)i#)Uy^49EN-aTaNWV<5Wt%O^PNF-*zo4mPO9x$kZ(mbduAgi77bhzlYfk8htv0*ie`8+gBD+^<>{qv4Zws%ErLS;hIpbSx(;JMataNiK7G4x5+O1Oh6^!(bZW&v7?!4J5OD|u7g2Ux1xgyrqQp$^TGw3(m9p>CSm5f39&3CXvh6d(cYZOzRfOhJ-*ahM8zrdV53?VR2Bsq~fjvjtha)(>R{*yJYdKQ`0%J!RN3;8XEny4f!mhW$xi@_Qf@@0i{g49M{*!Parjf<1HJ0$PT9#}cjTx?Q^WsB}e=Z6Wk3n5wF8z3Wr3iy;S4vKY@`C_}t&{961*WSR*LD55zpJ?0EmUJpJ1C8d)SC-n^5pb<y{&cvlSu_H(DtuD5`<)Ta$}Sl3I~z}nlK!T>TgaKsX9(6AO*vPsiqeuXC7aLH5i_c-vMAY4MsofadI5$@R{b;aeKI);%;Ose<;kn-Ly`M&2aU2^;PrOA@;`Jzf(WguZR%D+QwL`xG`_kJHhaP7T>~v2u?0|N1LYOP!r2MV-FQtY;d`hqByT(AAg{9>dHz6c$t$C-xGBG`i5UpEWNk<AIwt+BPCD3`OrAgf(>m~Yq0?ZeE~<rx=AR4I!l?S^K(+mFReglp>#wOqSCIX$ATW`F_hB_miHm$vTyd9Lc~{r>CX;M}bT!mVYOWC5ltX^q?FL+?cB+fz#oL<V8>JL3v{9K;xKO%0sjv#ZsPInMe(HEE)_~YlWo5t*>H)5+y*371rO4I-Fr;j~(p5K<td+VS>(x`+j+^Pidcbe$c3#T!dZjBQTrB`nwrn=(7M1!hG3l9B{@0oG^~$|or%RN4y~<qK|8HgqWdzh}<IcuFeWI>CjL)<l<WI9+*J@f4ceE%z%d)teg>eT<Ls=ZvMQ%2-KdSaN?rMgt-3_@Td2#5{SCVg~CM7BBr6%F6iTTL?lx1F8KdX1vrZvZn!hpm=%}HX9)XdiDTT`(p>&t)nzTTa3Zw?x3utKNo9@?{iS<mBDjwZ<;ZK@VZl}k8vDyoiOX$_?)Seq<4zTmP$c_G)YHP!>W*|^)EDXp^JuciLa?)}Xg;wh@AJE^A4R1*54_eXnl#&gi$7W<&<vO1|tyd$-_3D!p7dgm8sVMH;;ygMTDqV=Z#HkX*(mW$jAro4RRHLtJT**Dy($ud&39_6<&)docIYSx~(v4hegX#o|MG>{u!PasBHlG^_Q&k^S?B)I!+19(`^jc7o*H*y8joXXVmE&D~EMa)!kL64+D`6m6Et0gHcwn<@HTEEN(*hIdb$e|*KS(-xC@%`!Ya;5er(eocu^3>$Qou0da8(|_qXeU5)nrB8MzEJ=a^M>=pHa{Fb7;RT1=*oy~PQH<<{AJXRJ!6?3Owlb<JR4>qw`VUSLn=!!p#PYM?`>RTIop<lBeswC_xc=pJEJw(&FUcTpC!#(E)9XmvaJIa=o$2fyFBIOpudK=@aJBX57jf4XB2i;e6A&-txr;$U?~5+BpydUm!8Gn8J9Xrtc(X7E5q>`tdwKKhK({zSg}wS1Gabuho?oxyIW{~apuTSLVjNhALTYzwnG^0{&wjwwVZSoR+PKKe3{>2gSGd5@PKdbwUs&Sr{_`lKAyh!k_!@%VoR06X^Zzt7?;bd-DE?-a!|YxDp9xrh~~$xltqrn1&=>H_;T7D>#JLJ9#&Z$W-jEy{WK<}NH}~B*rSJRn7;)Cv${Qh{gUa5KXBc7%#!z^0Dz1klwVwOLkVF&aGhu>bR~Tj8dMt*ARu3BkyTpQ>*R*fWrMetp({AB2F!ZWGP2v#${$YzEoh3bmFZkCKFo7NHSCg3Wg)9IYN;ZGLz&boy;j6jLe|%%CwD9KkNx<LF~%%aRb>Np+!8k>&&w5^wRok%`^(|+3)`uc+_N(K*vZ(G#Ffl*j7PRt3C4zzXek)Y1ACYtcRCBI7x$SVZ8mlG$(3JNiZ6`P*0iMxB9F`K$n)rQ61^nVX;udh`QaT&7QV|0sZ>Vv3E0qDUiP(@y9GtEt?k{~8;Nw?A@3@6#oYuR2vB}~vy!M5)qw{czJ&VtmsbNSOiq*ey+Qt(J@3rNgo#?IL9vt>cAwecHBT3oh$JoLO~Jl0FGKlKNut{|#B^n(x`~Kfp_<}x6R~*HD$FV|q@Ap#sfQD7<K7%7y3#y0LCO0fc+H*@tIBi@NSzC6>bnZ;UQ4?Ze8UUb>UTnLz`9-(9Tzr!=O`6Un4S|{HLvNB7KN3I-Nm;(zRWf+2H}ZK*IbohyBx!IB~|&|7FO?erD+n0#!GM3Hr@92o3}_A_iU$DErs(1Z(10wDf@wLCX+9<IclxFO^53cnIVw(KGxSbcs3AooJaF`>P$H@i?`~0j^5&ny`3GNVsNxB&AXLvzj*Hae>xWm7oE%yojiAAUnFnb9oKN72!dla%Cr^iR#Rr4BJHIraiw_@^yE@vaQuMhVB8<CA)5<t4hGruw{fFQB=|d!gi-VNpbSA5LqQkgT6D3_U%SR<Tj{s0bhLer|F#5!bSxJ)MeH>;L8i8F<46_$%JKgG8l?WyIKXR=UoHRcpm6h)LfST${oBKJvR_;Y@R0qh3wO!GdjfNnm!p)MFy(vXwQs>H>VTR`;O<(W&V?ik2iE_#1#cv>8^`cA<Qp@!ZdTB&jmpavjB?_Bh4Jp$c!c*W%x7n_QGZeIegzX!^%V<dS;}-h9^t`6{s$iDXA<*g5#<k^q7<fg?lh6eQ(Sfktd%<~bz>EFxhqIj@^X8V>Hw4NU>xp?bSs(&PGA+nuFLM~<UtSJ4G>V$8+rUc#{|}`k`4M+rz1r%zwQiY-n*C=RV6or7P`u>Ch;wA{^)r4Sf#SJ1|eyulGW*GnGp=v8?b5=udlht&Q7^~^-xu9)x8siN3B#dttcY+b!T^HXLonM*E`!A?vKZNHThKnS;MNb7)kIk-61r#UFpGZ;*b~Jg$ug+2G&{R0P}+G(p`qp6%We6>j^+*%#t3}YIKb|@^+aspU3i|JeZ%k7tin>)vK;LuCR2I$eYD13G-K4@ikxE)9>ahUL&Wj&fCr3(6`51FTb5n+{%YdXI)eqclwPU70vD)+%4@qWpd^m_1cR)%I0>JC#`uniyvUbgcHp!JamPCqV6r{FwJGJr08Eb?x?>5q_miC&&DH@lu9de5-NSI4EPc5kK$k2AH{eLC;dQ|oMH9`v?!eO+fQ+iJ#$zTCY*_!4}b-iVY0{jw1w%i7^nclJAX+jMN5NSdU1*@9(Ta-3NX$ZbBZ|rEcI_c!8hm9oxvM0HkPZHedk_Yu?eFJEC6bxxKjWbc+7VI(IOucTi$*`YU|JoC_7Ih0ylS+@M1p)FxwG8J(XUkQ<{D^x^%J0r5j%~aP_|ugK*Kkbo(jtoD>1Yo_`UhZUVz{79IvqVV71;%%H8y+fP1FdyJPsUaj07`t-j38lBGF{&3ViWdHN`{{ra?a~BhNLVq1E;=>09`zzi#Oy4g(O7{^XfK$VCIYsuu^pUB{=79V#S^_Ttb^$L~g`MWpsY8wX>NS9~-B+UA#q8!3rjxtCy@dyY(M=<++W|fIofXcy8xZT_69*Uw2l6Z6<MzL4<Oufm*Ck=kNfXehAb79?Ji7qxVz`K{d5XjVXjL`l_qqN>%srFSkG<#EG4h%Mo;WEg!a<5;F*J!28l>z%kx-+}H?o|+QWrQp42R%}wcrh)Qr-GRw-AwhN(3d|=j@gnFd=H@L}zJ4MS(t}ZsaCl0W!S84{Y5z4sRKH0iPk9?-d{yOK4M~k%B-EQO@n(5##7Q11c7in3JAb%?^v*)W;qQLXa0|<3z!<XXFm^bjmGmnClZsioYToUc$Hm{jp++06749i(9)elv&8fiUzln&ag*w=nwDTg@d>x83&UQ+=t1LOo0Ue;Rx-%6FB}=Z1f#536mtHGQ(|ToFzpVdUqV0Ea(UnN{9%#mbv{kLdBKf6|hKGZqR58#v^RvT=^b}jc~an^Z=J3yDj-BV<8xb6lO4l7%$cVf+hrTQPyZTPk#!R-FezoeLFhsu+t;_?IyEci=-jYzzp>1>5p8vBoTB8d%nE=2aSeq`U7ZA5-w3J;xO`4i$vnP84kh}At-K(QP$8RH9#5MX#b42*{ui-vnwayyt2YewPGiek+2d>L0;f+&%!9+(ve@@F4QB5%t@miQ~R+PIh1o?W}pX`(A^W+DU*~1^&%!1Rd<+A3^m8;l_QBcBMEt+gB7`YGN>}|fywtM9qtpm8AQ(dNZJD-wFM)>;{iVIgNZQn(-<0{`Ib`~^Si~Mcep6N{dA7g5p&sr44eWX3$Z~2Pm4lQJxIHW)Hx)3yrpx_Sr<rP0g_qADdIrc9YTj8nj7sdJ1v?XWObaf1A49BB1{(u%NNH#KRY}*d^J6J^26ct$6Ar?Q;g`OR&GDFMCtDje>yyVOt+0?CC5g|UgQ4(JdZ>E"""


def configure_shared_guards() -> None:
    base.BASELINE_SHA = BASELINE_SHA
    base.PATCH_SHA256 = EMBEDDED_PATCH_SHA256
    base.BASELINE_BLOBS = {**MODIFIED_BLOBS, **DEPENDENCY_BLOBS}
    base.CREATED_PATHS = ()
    base.DELETED_PATHS = ()
    base.EXPECTED_PATHS = EXPECTED_PATHS
    base.PATCH_B85 = PATCH_B85


def decode_patch() -> bytes:
    patch = base.decode_patch()
    rewrites = (
        (
            b"index c9aa4d3..5b3448d 100644",
            b"index c9aa4d3..0bcb12c 100644",
        ),
        (
            b"@@ -6155,6 +6399,64 @@ VmSwap:\\t      2048 kB",
            b"@@ -6155,6 +6399,67 @@ VmSwap:\\t      2048 kB",
        ),
        (
            b'+        assert_eq!(provisional_planet_label("Port-Sillage", 0), '
            b'"Port-Sillage I");',
            b"+        assert_eq!(\n"
            b'+            provisional_planet_label("Port-Sillage", 0),\n'
            b'+            "Port-Sillage I"\n'
            b"+        );",
        ),
        (
            b"@@ -6303,7 +6605,10 @@ VmSwap:\\t      2048 kB",
            b"@@ -6303,7 +6608,10 @@ VmSwap:\\t      2048 kB",
        ),
        (
            b"@@ -6332,7 +6637,7 @@ VmSwap:\\t      2048 kB",
            b"@@ -6332,7 +6640,7 @@ VmSwap:\\t      2048 kB",
        ),
    )
    for old, new in rewrites:
        occurrences = patch.count(old)
        if occurrences != 1:
            raise base.MigrationError(
                "Correction rustfmt interne ambiguë "
                f"({occurrences} occurrence(s), attendu 1)."
            )
        patch = patch.replace(old, new, 1)

    digest = hashlib.sha256(patch).hexdigest()
    if digest != PATCH_SHA256:
        raise base.MigrationError(
            "Empreinte du patch MVP-023-E corrigé invalide "
            f"({digest}, attendu {PATCH_SHA256})."
        )
    return patch


def validated_patch(root: Path, patch: bytes, *, run_checks: bool) -> bytes:
    with tempfile.TemporaryDirectory(
        prefix="galactic-mvp023e-", dir=root.parent
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
                    "Le patch MVP-023-E ne s'applique pas proprement dans le worktree."
                )
            base.run(
                ("git", "apply", "--binary", "-"),
                cwd=worktree,
                input_bytes=patch,
            )

            if run_checks:
                validation_env = os.environ.copy()
                validation_env.setdefault(
                    "CARGO_TARGET_DIR", str(root / "target" / "mvp023e-validation")
                )
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
    parent = root / "backups" / ".mvp023e-backup"
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
            "Prépare MVP-023-E : sondes planétaires, durées liées à la "
            "distance et trajectoires visibles dans les vues adaptées."
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
        patch = decode_patch()

        if base.patch_check(root, patch, reverse=True):
            print("MVP-023-E est déjà appliqué ; aucune modification nécessaire.")
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
            prefix="galactic-mvp023e-verify-", dir=root.parent
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

        print("MVP-023-E appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=18, SAVE_VERSION=19, "
            "RULESET_SCHEMA_VERSION=5"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
