#!/usr/bin/env python3
"""Apply Galactic MVP-023-B safely from the exact pushed baseline.

This migration adds deterministic graph-connected sectors to the immutable
universe definition and exposes knowledge-safe sector landmarks in the client.
Dry-runs are deliberately cheap: Cargo checks only run during a real
application or when explicitly requested.
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


MIGRATION = "MVP-023-B"
BASELINE_SHA = "bc1ca0143a7c2b9d8b95186ebfe6682453087eac"
PATCH_SHA256 = "221559bd3febd69fb7f7db7587aadbf1ee1564f64fc65520c1826815c7874fa2"

MODIFIED_BLOBS = {
    "README.md": "4922db9b98250c5734b701071a0326703d2ff94b",
    "crates/galactic_client/src/lib.rs": "6a9aae86fb8a4d78b7f9a6f721ba6a1971e045d3",
    "crates/galactic_domain/src/ids.rs": "55c75db7aab196ff3ec9e7b7da34d72fd37db2ce",
    "crates/galactic_domain/src/world.rs": "2274b9b05caaa48b3359df8508f4c9dac03b65c9",
    "crates/galactic_sim/src/universe.rs": "777c263ea4608138e786fe83d2b7fad461eae02e",
    "docs/mvp_architecture.md": "e6ab1c199046d1e1bed43908b82b2d6b3c9d2c41",
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
PATCH_B85 = """c-qZ<+jbkda_GCh0`^|fCM}BQO%z3qz2mX9XS`y2CYHxJCyvu@c7s%po86qg$dWCsmDjxIC2x5gorm)i`jdP~stT6|`a)8k**jaymdI|PP$(1%RfQ_R59V{WzrP4F=01FV^5ogelkU=Iv*Pnk5c&LqjgAHbe|FUE4hJLe$nCLyuQwizcJ}x8&6+!VdwW*xzyEu-KOA=s*&h7v_ZWO%@%)@e8RJ>PS;D=9bDlDvuks*e;U9VCW^Bo`fF~?<qm+4Zl=9>pzWRUsa1v!fcKM;pUU0UIGoB|qP?ar%G!5d2d2Z<CA!i{#%o6u!o~0cIRg*XmLiv0i##sh`dB#>ryhwPOa+XKIFF9WVUOOz!69FLbKBg=MIN9yZ0^g@y_EW^!X$&-BFLE!40Vd*?C3%o>=H|>hb0fkXx|y-t{|BAiVLUpIFK=%+q5^&7EWJ##+h3QQ`L{P2_W(XiZ*M~Q1igsh+mxly9XCmWbF7!#-kjh5iXCv77tBK5+1XD4Md%lH#SegvvQPZNg^$=1YzTS?7zF?-ia4MLc&EW4a`TJs&faf@z+Sl6fq4)Gi1*H3uw3RYV26dmKLSmuzX@MJD=Q=qj1rMn^&i@cSc)y@P>_g`ECF1Ks2uO?L3ht{&K6-ja{;V7p9kKVIa&gm1`xv&z;PJ|z&LpXe6*Yam+>^)*@JP4^6n1XVb5YO$I5O-JQL?h#;5FqAA9M;<@w5S6Yng*VaXE?O#A^W`Q}XVfF*ePaN&kPslao*FaU55)5Loi2D2_;GTU56&aAO}<hp#)?T!xm-0!>g%-Re^%(%_va3&rdcE)TEejnmQ43?{q{TSv87~FZBupeP6D5}WEUv^meCtvx{l!GJ4m(F>R=5CmF8q2L<C}2>XGdGOa6qDdWcg7LgUQ;m`lsM7FD$xAI{p8S$<qM!f4Ez_mm*tY}YQ-+`c&jZ0R*&d3>`glT1EdtrZ!cq~gyaisTu}!4AIyRfWYSc=eyg8AtJlCedw+Q6bC}1x^)z0t;s{9Geh-sR!lWJ%OyGB;@H0Nk7ajIAgc=?8qnl)b8=m}PRQ-o2{uCMY0Vkqc#xF9!CQE{7AxXK4Q<|bv_D^vV`ajCA@b(%SWwjWpUcZS~-@3^g0D26)LHZny`beL{gMOz!LHf*L60h7(kt3%(2fpB+S_PTg?PKwU2!bDw;mpxwz-=F~L-rWA(qNf~#FA~*Km76H<<gD(v`g)_S~kZZvIBoQjQ|M~To*~xF0of0IQZC^gYaneC*5AT0782jhjG%P-_vQDEM{)2*F73{Sg(6H!rw#sJejmBwG&vxptsBEbe_aZC&TZeN3AS&&bc?Vaa3c~W_#={ycON(b$cB)>JG4#Ub|hLA>B_wcIKq80`iuqmad?g7NS-`%HW#v8#Q^}!U&4h)+mAFxF^POPYkG!Z0fkMh9=oPW5gr(JXlaKyQ0WE4?|~>xT`aJ_FY{8qd}8`vkrR1S%-tpXftOaQxQYWH}TGt?VcKpwEq|>`%QTQJyskR!YZg5WRF+@r>mZ}6d|xE;cu6tM^s1@Q}qf`b_K4f@dVbKq-6|kcOHZ&JD2XNbtQ^lnUg@ZFZ!<dCPhrBa1E0PIh)qJDK}n10uT@)I`HLyb%RuiZ9<^+;DIjEZU!WKQvg$>k+`)CC|#rM6%&7p`BB$~7l)`tO{d@B06UK#>w!`~!Msc_><C}3vddPxOkMm*IOqWpdj)c_1nOFit~nTf0BZsWw?~y`LJkWIb3p?g4vwo8RYCTsibDxhwZPgT6x5GHzhz1P>GYpZUjNWxSFQ*85OQvubTk@Qb+k}FB{(RcnhWEP(M0zG1e<E5d(qjx;-y*f@(vY0n-xE=sn|fg>tZ5nIT<f`t7!!`f5d7mCS*5qm%QuYlHO|9pwX~QuOo0lNcK?iUS~tm%k7JPUR$)>hiij==4Ul6kvVDQK(Q_)5iGk&t#EG;-W_w<Db|!Jo5eA#Wu(3xjD-F+?)N)`zFbD?6wQ`aO4Kq^r)Nsa7c1J7z2@mRdZm1&Up$udSc2&X21)`-;XmS^Z+7vCp&7D&g2nn9bLqjVZFBue&*D7v9r*JJWXaOa!0<`$wTyLj8T+6aS_}3|cr=~<B*|%v!0xWOB8Rch9z9~J+>01=r61rUlK;5S&k*TC!9@*iSNF_h^#?|wM<`%T+)GoUAi{J?pF?9!yk<adzOKsmf$#Iks8Q%6im@7O06Es!fkp6n8K+bB9sTW8`HhBGe}`{iB6|F>Vz0@8aA=H2o&H#|7qzPSSXk99G&U76=@HUUES~40@2X^tis2-jreU1G!qhfRF=UTf4B8$o3%K0ZuEzwri%Mr3$A4X+ECva;Z^HLb)fJ;@G-Zqo6-r`I{8BkxEDgvW!un2?%E(EF0zs+*Mh{A5KL@LgG;0J1y)DB-vVcT)z@}(jwXnqwgJM?<KXwD3-FpPz;e)A8l;~8V29Ug-a`;ngNsQ_DKv)%nc4b~v+gK0vkgSE#grJg@G8)%HweVzXAPxxDnBX1}#Dj@pP3!r)4#u!oU?qAyoJhNSd~ig9`*0*(6oo|210}30ZP*h^t}8J7q;Kt5VxrKNC2x3f9%WirYcxFtENR0kAR+e`T+cpZR?!OqI<;y^H>HC$a@5o92TmA(Vqi3DR7KG&EoHeHu+mZhBoRa$ACY)EJeV{S@mPr_R8P7+fQcrmizb{F1NXm^-X!+&?T_q=t<XP|u`km<UBkC8_zPX6;?p@OpXf-XFXW<xkN*;#YDeFf>xcC2wQxA9ug{bNG<}F}Rs8!Ta>L6MKOyHt0e%$aAF5ik{EN1*l`q%#4Nc@PgZHU&GLj$huh5@5Q(APzcvrARr#vA*&GbHYy*x>HlsVXA)Hwx<90~>L5a!>cfkg-qN3D(fR)gj=6HIQrE*W#u=rIPTI{S>5MQvdltJ;;;Nsc!+E*8~FZ9O-;KF?&wkRGGM@et`TnjDNe!vPWmCKODKG)qlEBfK0A|K(oGQk}@J;|D<Y$m6Zy09{D7a`hti+_2&U`@=hI;=NS3A{I*(%$7q=9EF!o4vU+h9s0BZux+f8#;64cOy@#<5!Qo3=V=_xgT-{ZJYTii<;6@i*dPvfEx}~fJHw^(FQo*)BzC_4;pK^Q`hQQ~oV;|NKP%Va7b`R_?jst~u3&(294%5<w3#cdulp6WNttxeC`-4&y8+p>tV`6G;e{7Px^bjIC^QR$eyC!Us#@+J(~xvi)N@-EWv1Q<QWP@ySd6WlDXec&s8Z@(?>#wCI~Kl($;N8Rp6sw14@9;0y^^tO4%A&-S8f=#uCUgXWNlegyr4(&4Rs8M;sN!?09NsMs#(kAtc`Ht49RqLf(V5`7T{)#Gv}YU;^NfF&fLfmW-D2_6^;I@rtjKqDL-DiK}4HCfuGiIOI6CqZK;C;@9@B%b-S)V_Gi7``faHyl=^L{YB`j@pz(tA9gKy#vUEYob&@;;a~GE~l=m=Az6c||cv1-A=cJVzFN9%DkInL8gQog8bE{>44Bd!l^q7{216m-)o&G>?ohfi?Ae9?Iyp$*?A-&3HVy75ch87H>bBlgYSw0+)Tl$oTb5pM4cfkQ-SlpE5<5Al-dPRfO3x$i_Tp>)gZ*{8CH``^^c6E&|Shy%_wq?0bIMX*Wohl{SZZJ3;&5nTW`m_G*a5mbS?KGT?jHgTDxb|^}Z?7-*`F8eUl?5h%1zU<xbhb{X-@ZvWKjj%PS$%;oRN|$(>M&KdgF6h*_%As}U0YY14+mHm1_r9VdGZtK;rKQI%M$>-jusstKKGFk;ftz_uO08qg@4oO(=!+TP2e?FC#0#rP+zfaNAg2Q4|AcB;Acw=U(1fw0FhjLS8&j&f&*~@|LN89w<oVpVG%z$c{W8J1|c^Z9GA<<HU8-jKfQVb#nRw2T3G$@@y>ox@Vk>&C$FEpdH%yI2Z6$0nm|x+Xq7{NuTTDb^7`b}(-Y^<&tH9a^7_Zu&j~UiFz5{q#*@SGWN>iQ>ka$8BcQ}K?ToB)8{6q091e$rqtSRW?2m_|$>eBqKtz)DpZuSOjstpX(^c$@!EX|h*tp!ERpaoCt3V7>2rPKS6Iw_dc)j4sDhZ;D5bG4B<RUetZ802bG0cyRB0{GYCCKJc&Q7W?6d_rkz7*&@mK)(hPSv7|9TYk^pfxzqBDT42wYy4$<CQ?Q8)s***3({8?H-Z#4Jg$JJ2H=CUqIaj5!hiQDb;d<lJbWYqVdgcrI)7I#=6>wkvazy^y~D$M~|4+`RZ{owP6!2Q-I$Yt?iW78kCh~Hwz*+xdb&3!)02#xLp^3vGP&tqjGuLDx|^S=W-9s71SIJgZ)rvD@Q!w?Bp1FQjK}2=Xyy^ldZw(=AYb4r;hgpOK;IPT4pBX*c$YXis=i(D<>z{j^mam^gCwjdo;r9=Yp^jXc4_C;7X>U#~tClPojl6BkGL>t(3~}utx<*>9F3F#IyFn1!tM1<s;cI;61RpnW_t)1hlqxN0FB-w|3vx_ZvB)Sc#@nkj28N?~7okL4Wd>B6*W&?U}9^+0){Q=yAuKltiLV;hD318TGB8!>;&NbY@q5YM0;$Ew(oyHFntR3<hfFqCB#CR_wkjR)Y6BXhQJ6#d?N7Rr_ca2EdUQs}8Ggprcoq-FMVR(PX1c!>uMMjq3OC<=#C25z4G-%$3+yyMoQHD0y3!E6e0}Azwti1rk8JwZ!w5;C9XICSVhc>;LRR74NcWTt8~*+Ipt!neznf<zNwEFe9R&)KHkrg`P&b#ACbvsyY}VB3kv4R#%~Jt5msd)Gl@hRn(FcE~3+I+~Jb_R|}sPuLADF2Oi~1;tG3^j6QS&jP=c00)i$PHDS656yDuco}M`~*ZbJ2uTvTDhY#7yJj{Z9v8Ttr{qre?xca#VVve?w_tC*i-a$1ryX+~!W^n@2o`EW^VWn|`1&bjBSmy!8$dW0^0LlGX!rhNBS(%$Lps*XVvv|dnzpyJ{eL4#$J~<PS1%=sF?hwv=-TQdu(3h$+kf207Se(tE2Uttv2qGm@@*AUUYByayY$D3dke6$70Du7$bR2TsQW*|RQFD9|EEr;`^O0X_^lHOyY7meEX+?QddFO!w+sr;qEYwuhtb1K$s5fU^3p`(L^b#wvKx`KXUo)G*jQhy>in$CO&@$3F@HC(WhfxD9#$$6g3=t<_FqWMGm{p$WMsPQoqdkl6%?$VnN5jk0EM79-&D^dLVkx|#fsVImnt1RlQPKu|kX`CLx$-cuuys|52a>b=F8(+}0Mxt%ayYZg)>X^t01MBvc8B4|1coGQH-;mT-x^9gMf?<NjB?v5RLW3&J(%kO?u%S8{ZZ=d4;~PXYd|Qsv~-LMdYO*tOoX!2`h0&KB-OR!W)cvU{`xc!gY&mQ1kv9u>aQuHWU`Zjm`-5LvKVU}ax@PeZ@E%}RX0}!Y7bR=E!Z<2>G_Aqbg>CR^qP9twbKd>17!j@Fp-2ztDBy3tX!N?ZkjQ$Wtcs1`~rHD-nZhs>U1hpF62fP@Zvf5?!6-=tT3~bN@1x*?~hIWqG1W7KqOP~F(iX*^W6ip#(R?SW?*R(%F^H~;Ve|%jR@-qA89m|R%CMe>P<2O|987ZoXh*t7F8Cnt&+uN1z2mXMjjc}w^-hbkBSWM3~-8w)UdfA>cYCQ+M=4)a)`{h=Lo7+f}b(ON5M9q*DfM9m$z^%)y7MpHZnWr5KC*PU0qlz^}49!RpN+kI8+8!FVbt2AIgYdlKMM^9@WJD%Gp&IAN0LY{&y=#)kLH=_)RQr%<!MceQRSIf_4{%gSyC6;bD>3;TLJ#h#6T^L2BdG7ux!(j!(6)P2^6aD!4A#bg%HEm2EolK)PGW+!T&iNja+&1#auD7qR{OOOjbk2x!8JX|v@;tKaQekAkQcFi{_yH_>h~D>s<ZW`E7@S&l9<+H@WyX(o69xUiM3!XR4)7vmAN#{QM{E)IsH17|cm7&)W)!N55f4*LbpZT5G}vXe`OW??FmT$M%N8O^>2o#MS_YGDp3LsCS}e`8zfCGi^J25y7evtdhK<`>*k%_$>AujSQ*1{_#YR&i05I%=(os?}4a22j*j6fE<l9MB?iM8GO0y~L+|Tn|?05nI5ef?ESNaBWB>Fq9n|ho;;EOO08RR)D<5eHB}jxY0)`W~do5ON5HmRE|`n`t*|~{iuqdYD~1%0P?yi2u0~j#G{}vM=@s9dN_-uF-EVfU=+o8v}__HcWZPMZEq>*&@|klxM&Px=e1zym~SdV!)k?IN<s0SLeoKd%!dOtdKKfZn!H3SvcwJZe7>w(_P!=9X_IlH+g_*Zst`A}rGau>r!1^d_659djSX>KS`<*6(DtyNUY6E#7u~$H>{Yyp+qqtqH_`AVTI+|kfRHoNbYzxCw1GjC@GV>q9ae8Cv}@LQD5kM=z`C(Qoi^y?pe=9VD084=tz9#ezQo76s+he-Jbj&JefLHI)K*Ora<zV{vcE1xdKB%QyJ5~#YSL<{Xr-e67Oy}I=PT2#PJSf?yQ+X$dToUbrD<RZA7Vvo2d~U_sIm0Ay^_XpS^M;}^^}ras7)k^vR6xr#U`K1+O1PNbx{f?(a?qkMB=rjr=-q1a>4c<8Tnzxn@j7>rSayo_2#oZu6BB(Bv)NpV@>v&nV!bX1T<!(m{moWHZ`QetHPzUMk_Xb^|D0q6tAAVJUK-s+D)_MJ)pY#&WH|wiTl-@`TzZI|M^3*I=j8$R`B@SI1mR|yt=(f$}gYBd4m32s{I7UG_hXa{(HtFKd0(%dAcgSei5X>?XL;FetMbQ{u<fPF?KwOgACul!SfQVw@(reU!30Fgq50qMrSr5^*p?^8~qmA(fwVW1&Ld36ZfwX@{fso5!i2^x;|)EgbxsE8Q2(|24UzfsP0Pu6W`vHh!xGzM~y9?1U|;T+mz;XCJT`=`RLj&-z2Ws07hE5p<gB?9xFi}a2zSugOMtrzc(g*JfCCme}hdk;-k4U@(+fNwr*^jhFHn6Xz0nimV~<}EnvIo`!4!Ih%;>~v{r4kY`JP^9)=J}_O5oJ6rI=ol(?%EGz;Q9Z1paV`0&s<JenOjhx3u^9D4nOwuw*yk8=HRB_M3d4bvMm-tceHi0k_ejU3zcTH?dMst=$$414Vgy0iJg#2FoqeW&m7k;4yWM_WJ|99AJ6jr^W7I-1R#KG>;Fzt3kjBny>(B`;M79#h(MFmhrlVkn9LJX;2_^)g6H>-*osBo5pNB>H`sf-o7;!3TqJXD~Fv6>2y6)dv7yjZiR#PPYd`?7*WIA%EtkXN5aTMs+qH&q3bSa$<(%a;*Z_!A%;vReE+w5hRuJDyUt&uT-mVL@L-9IOS#mCS0i?*}GJ~{B6}ge`R&cWl~eK*uujQXdDT<Vp%h+YpE2!Td$Wkqge8DuESI-+q<=Ow?MW~Wvke3%gA**wIYiE%8LjU5pGBUm6OR)XL8V(@^if*Wk=2j2W&mZ+S7Hsc)5ZFh*C<?Hvbt{tm-~JdBDtwDLu=#%GiPS<H-N|`o=!ZS+Hu1AyGc#N+3un<AZvmghWSTr@l92iCA@1CQ5Y{6vD`=P-ZHXKyt4%!;1KzGEb=*WqATMNT%{_t6bi_%I9*Gs*j}DMJxBH6kSs?iBEY}d;W>`$f~#mr)WV{w1b7pHI1Ml;mo!0de?jZzErUk+hRnX*s(12=dXate6ND63idEyHTV@9;z(*@Nvx<4eu1id!=W3*foryHm3@r*h{{JUFkQDLU$+z1cp{H#=1SXQ+H<W#YwGr9+8bkP)ZDDlUEAnYX1(iLWzGsYrK<-3$-MmPd@3Po$S@rTRM|g9uWE-5^4S1w&8XKTC{_+En&URm6{A1YW<5MH84AqqG*H*IveYDll%ytUOEHYvwOqfdIrj2RYaE>o8D2V|xjJ;PpDc=5wKXZJMi}oM7N{nzqLklT=TWvd*{i2+ea&GLmUl*A15+E1N8FTTTDOb;6*y(_W{i9oz#tOogWIB|#y9>H3HiSfx34=m%+4PuDU@1diD-ew2$o9X#QNjEDr;aREYV;U>6p{Q)D8S~(#odW8r7oyJe#eAjon6r1S!^Q>`d#O-KCDzq4!@WU>aqFSRip_IA4il2c67K7CZxq4O%)h4exdM_ivL7sEt&z$T|!xkUCBDUaK|kb|3Y7ZDS|5{%C=AgHGCT5CNNP%mz;QoF|-ifH)AxwBK<pqo`mz_g6L>abShDxs_?Kqy)x74A$jV)|8UDm4}Ci-e5fJcDe6*<MF}z+{!vQ^|_UGB`K}9GhurJI$*-cfV2}QzXzL)@+R>^C2dkvD<yG4EpbMPNK@>vB3+R-uSBk*-0`f*R}@UusY&kW2)Gq~loO}9y;9^&Id5_!E&_fHLUp*v!&@3MkDkV>OC$fN!-^av%CCdpBEJqDOc+X4WX~XzoFoYai!p%^odZF=Jc)cd%C|_OmI*7EDNH3QZ=U6=Fz^6*@tS@sA*A04N);K~nie06v-JdEJCRL9_f^xuicML|lPcd{*Ej!Ct4(GnsN_pLp(k2wELVc~oe<yK<4mjHRC1J!?DOc{4Fmu82W<82cLFTZ;vPPH$X;{T-^a5{SaIA3b16g$)RH1zUW*MOG)5vrNt{bllji!m0QN<l1*qoBw02TkJQ4YUhdbR;@Ko|-RK_dK@oMp6?zf~@SZ2tm*a{tXT53iM1l5cYis9(=*(z4r^CA(`d#iPY?d|za;p{r<;o<vx{eHi5II3jVR!O>Mqt|qeReecz|6y^LNKU78JhA7lTrU7ssTi};f?VUDvWc`E%sO+YA#q8@Q=)}YgO&8MG^8%6Bs8nn+BZ=sVpjZeVbrcUZWaV_t16kN)3V?!)Zq<Q{nwc1=z6gyj-9}>+557gHygx!H9kxWq=L9>iWlpE;S5-F#OoFgBAs&RXbxHDZdgCocOP$K1u{=t-i*7g*{|7QD2+D^b6U&u_ex~A17mFHa?_H#ro(G{S9p%A^SjQ6P)Y=lnrT_utl1u^q?+A*Z$wAPxltU!_~WU6w9*P=s5Y?txPAhwuISuK1@d=d$r{DsH_ltBS!hIm<CLxXg-WbT`H;G`N+ea2CucpB`T;>0AQDKS0}$dIyOd{!rCN+neYbaat=`--{1%NBTBW=Yay{-AVhT^;H@a3W(8OH2FJH$>n^fDs9Y(sYb9Wb$+K?>kC-gc-Y)S;$jsfd)g3P+YW5|86?eg0=sBn~(S*Ku{dY;i(0MtS_mE3?|$xoXsn%{??L}Jr#V<wZY^z$D+w#o-_*#=JOA{%?HF1@r(m=d<O#@eY1YIf{{DYUyv86d{<YkiasIT{8-aoEYE(?8tO*f5jcNn;k+x7(%t_uui19umbe;pKsGGLBY#NT357|LM4Nf=-Q@fMw0R%D>7%KGram;z1SirN<bIfXNQZpDwQRC_3NLiIcu@j8Ab2)VxUbH#aw%I#bR!Vpa!AClXM42YRt^H#?6oqRRnHfZA>z8pkz}o1lM4haC5Z`i$d*%bVD|%vrhFSxb1yy!oPqE?T_N$PDoyS6{t*vFhM!aNMaA5w-<Wcm<mQ%7bvoUz)Et2(<_52(W>;cWF30lvn9(I;B=jBAQQhWTz4dzG=ss2X@A&*69dEUC&IMT~}{id*!edv4j<_p^fLjmN41k3|OW4r(&LS|9*qJzraFG19QXJ*r&Au<t+a{jnC1fGlKDfKL>}6<AXmNRcqlgm0M!CY6?{YmRDaDsl8pRVY5b8Qn`f1Mlo(nB>ZPulB?S^XbTnM?l;6KX%q8Njh6+Fh%LfLjH)$7`?{#eez20dt6M?(;tZQ#s;iMOtcnK9+hun*&}si)j2?X?<w#-MaHm%7=q-g-<CM1L|0)Ala+b-q+tx6C{i(=}2ZB)kgkvRj%NbakK)6nMEm_)4*CSz=MC0X2D#)tgwR!lULbrB1yD@7JPpVfgi6HXA+~<ncGIi;Gy$o%fQuYA|fxmuMS*6RW#2Wf_K_SPB6IJhpYK$g@;*|fLS-GttU>E>2N1Id8rS30E)foWS#Z0eQLpVxkw-=T1og~p+L_tN62fhf*`O8XzdO1w2S9_d5?{7mGD-m34hiaMJ(1Y4xfq&K>ebb$>Y}WWn?#A=6JL`M>Znxi`%#L_zH@>m5*n)3-P4*XV^9*UX2maD_&s!iAT@C22lG|TXSWy<@OhGo?Vf*aEcTezq{WnkEoT!s-@zmPG5A2v;pFVkOy_l#MuYY=Ra(eQ{Ieq&5$;&53k%JE?2=OxH#qCYIi_EaI_lG~=QMvoQ!Epau_TT^ZAH|&-QhCNJmP@w)W4#a6UA+$?t`lzn#yd9f2=)MTfl@%WWH^0b>RMvDz8SAQrdBb{ExrBK$D}${>$(4-gI^<btmfh*J3M#m_9k9pau#CP!AqD!M2^7&yq($S;x1+5R%3B7v2+Eki2r$rkABJV24v8C>4IM7<`?iE+9+-@?iw8~X2b`n!!q1f^XZaktb-S10|8b%^63zO+y7$U^7BhXAmMW~B0{90yIQ$P#v?2Pqwhw)<Zgr~YJhYIvtWgo#W}7#@!9PSUKqkt8nhQKb{-m&?Bbop0U(4oS_cVy|3GJ80J&x&txc1l{Xr5Bk1hlRF(3gc0gLqQO&AA@NZjAdX#nxHyTE%<+;i?BfiZeU+_t?+;*~4zKM~lfqx-}?-8*}9)EZq5jT8jbQo4*Am=8v)xN6(D);b5UbLdDHGkK_xWC^+`8mQ}u@aPN7`r+buz>MyOp&Oqhi6pLioas_>dD#p!mGh7|qdztngJ1#uM#ZJo$JpI@jx(6u-hh0>q&U<EfSluzbyfDxo~l3TFdxTW-3LyEarFB=8n=_kyf_;VMqr-gIC+OQ;tk`L$`vZWi$9EewcVT{54t&Uqq-XI0}hC$RrE*VD);vP)0J>64XmrTks0hck&2-69Ar7FFEN_zE6RtYS9Epz?Tx+(o~A?&Scyx#^~F0=?6gkG1+GON&>U#Ci0n*NaJZ2tpkFW-ahz)BCNSd9xBs=XCvTa7Ze#&Wbj2}VoQK$P%<%`%85c9eynr0<5N5>d;tq7{esndHP-JjQ@<bXK5n?#=t7ndX5I8boOBmsikS=CLgau&d#FrkiKD?uTf@E^@bG~pBq>WEE&Y$xzUaiD9!+LPz-QM^#<q@Hb$hKU$#PE_>C~QFj05s4hNR-0Zr9@CI<zZxy9_v?=y{6S<Z~bc0-T5CyLc?D"""


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
        prefix="galactic-mvp023b-", dir=root.parent
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
                    "Le patch MVP-023-B ne s'applique pas proprement dans le worktree."
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
    parent = root / "backups" / ".mvp023b-backup"
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
            "Prépare MVP-023-B : secteurs galactiques déterministes, routes "
            "passerelles et repères respectant les connaissances."
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
            print("MVP-023-B est déjà appliqué ; aucune modification nécessaire.")
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
            prefix="galactic-mvp023b-verify-", dir=root.parent
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

        print("MVP-023-B appliqué avec succès.")
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
