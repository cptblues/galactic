#!/usr/bin/env python3
"""Apply Galactic MVP-025-B from the exact post-analysis baseline.

The migration adds a deterministic attack mission, versioned combat rules,
atomic losses and territorial control, persistent combat reports, a nearby
hostile playtest target, and non-zero lower bounds for confirmed unit estimates.
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


MIGRATION = "MVP-025-B"
BASELINE_SHA = "df8c921c35eb1fe70c3c5497919e19c5a4531330"
PATCH_SHA256 = "9117fe3cae8748464a55f1b679af66ba59bf3c44ddc7cc21077f06e7f7e10dd5"

MODIFIED_BLOBS = {
    'README.md': 'f8e0dfdfe8837ada60fb36108379f25df680f412',
    'assets/rulesets/default/craftables.ron': 'c1897a972c6fa439e35db14bae7b4277403246d4',
    'assets/rulesets/default/manifest.ron': '39b798c97598e362980fa3fa44bf43bd9e55d1da',
    'assets/rulesets/default/planetary_presence.ron': 'e93b8ad7838733b8cbe0a6e6b5448a5aebf29740',
    'crates/galactic_client/src/lib.rs': 'd43bc9acf6df4cc7ccb481dc12043eaeb41f03e8',
    'crates/galactic_persistence/src/lib.rs': '85d2194f5bbc06925da473469614cd5624238acf',
    'crates/galactic_sim/src/command.rs': 'd02f0661286cbe61ed038ee319cbf051b5c979e2',
    'crates/galactic_sim/src/craft.rs': 'd15f7f4e61e84f97fd9cabddba4d15cde4a6624e',
    'crates/galactic_sim/src/lib.rs': '90077299061c493f318cfb0d4077b5f258adc9d7',
    'crates/galactic_sim/src/mission.rs': '2bda15a954bf5e6ab236a6418af2ca6868d4bb9a',
    'crates/galactic_sim/src/presence.rs': '5f97d5df184206720399b817423ff49d45e5ab0a',
    'crates/galactic_sim/src/ruleset.rs': '73988e02d9b48101fdf0a769dae6bb818c9461fb',
    'crates/galactic_sim/src/simulation.rs': '7d6d08e76bc92e5d7416343fffa76d8453f20017',
    'crates/galactic_sim/src/state.rs': '6d49703e16e3e5d04ca848ec3ebafa71e4c1ff88',
    'docs/mvp_architecture.md': 'd5980074749a3759cfa5740b3d1bb8b9b0be60d1',
    'docs/roadmap_galactic_issues.md': '45777729f1c6f23355047d626b115c17a0971d71',
    'docs/ruleset.md': '9fbaa832213c2b1195f3417b4a854c00e39c6c7b',
    'docs/universe_bible.md': 'ebdde269d3f3b5701a35155b240c5c4324c3d63b',
}

DEPENDENCY_BLOBS = {
    "tools/apply_mvp_016_b.py": "1557ff3f419abbf6a1b58b897100aa72da80bd38",
}

CREATED_PATHS = (
    "assets/rulesets/default/combat.ron",
    "crates/galactic_sim/src/combat.rs",
)

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
PATCH_B85 = """c-rl~+in|KmLU4BuL!GfoMeikNs7ABX;&#_WtC9AjpTH9ooshFL<XfulR;)gP_`;1pkK}a^Eki&2M3sk!MselaUSOn^e6o#XRWpGmk0(WRd?5%DRigBh#mW~_S);V*JYH9$HC_2c~S(?lar%^7q5=m(@}7y{$5McQT!zs@5P<bc+_r>$GzQdGzz+%&Q8C-wz;{ff4R1?v0;4uyWa(y-Tr>7A8g=1-TeTbq;Zhle!aZ?%OnndS$wldgW)VKqG1u7PiAM)B#4V(8s)z%;$ZSkGK$k8DQ>?)qv;f$#CcF$&EdZz_!3{_Yr)HJqBNRZ<<N0Foll}+9N_=)UguF3z#v&PpU<))7)97ybQuqWF}zM@X)Bn|=8H*$f5YHwK|CJE@MfF``D~E~<5>nHk7D>^kqs~6qKNZ$@Dk>58BO9vmIv_{=m~yjabCcDaeaB5UEY3$XVK!z8q8%fOB47tpXK@O*Ac#H2R{NR=F~QY`QCmlvM4>j{Yw@HDa-`g4HsDg|HQ%BWQOpFzsx7eFe&0GjIy>liFn63oBLA$SX2}dH7x*E2(+`Pkn<WvX&$_K|8}#}+ur=Xy|!WX08iF7e)AjEB>0zq`Y*u&>oUhad57=2*plWyiPGD@6mh_|04EF0X2aoPj(;=Y#{$&GBIog<2yXwmfIr!+;e)je+L@29J|cu+(}qI?dV(Nb6v-rkO{V@PGuVb;3QvkCfgVOlK8JOj!J=miSnzo^Tg;(5M3^7o<MRkN=mhX10wmy2$43Mc2?yF|*dA;JGl*b>JjaRWiv%IMwlTf^XTSzn#m}(LXflbj%`7fpk^y0C1Y-l{r4f7=;kM2fNlpkcgVhqUBLvqrV2~otrU}3v7IORFaPP7#L8PH=pw%;!oyS=Yts>k(_$o?ZR5}TOnM2%{^gKF8lxktq0{_hy*=2GGU*s(q3XV>6j*m0MDcG1yAUmCs?Dl^`hcKHgDP~y`O&Dqp37XT{C>bZTmvK76IUp4H`jZrn0Gv?(=xm-5T533$BWPg_lLUTc_#z!9aSC72VgPZzwu6IllCEuF^C6zB;B2-S!m#-a;GImTa9EOvAw0|wzJocng5d=l;3_X}{{q`_IZN^+4Mq#GcWWECumrz~7O(~wlJI6812sXkEC4?Uju*2W@G8c`L#GwcF_&1KQ$Ki?X=QNO5z&e)Ia?3_%}tDPs}`&&U|dopQ#Nsi<d*=byuCKkscw|#agjgC7BD#eF^b30Vp2R|Sklg5jI$bz*3$TMFizNZ#ezID;@x5YYqy8{!>wITwV{v8C+T7`;WWF(5J=2Btqzd#R(A&o`PxQfZ6kpHT_SqU(m~K|u?N%WOEO(dLjZ9)%HesBzZgY`hhaVhx;emEcEpQ#90Qoc&v9~oQG|fS;S_gg5NxY9?BmM_Fcc1!-e^y>$)m|7Ho|uRY}n+hcob6aqc|G`jcHs&6Pi&l%m91v$95|iFJkzk*A(4cBy*VePwW@|uc7|L|0SbAus+Tbq$=UrV)8l4KCQRRHnTCUX%OsotT%kMgP^}{zgc9_8Pc??L9nBLCT8{l{%zJop)4BHamkmVk<pN2&9J+-zZ>nh+ui+9Z?v~(vizD|Ex|3bG2++OerqS#fdBTk5yJxef4EPyxfioJ>>_~Ac-?Gj51OreZC@HqVTaZQ?+~nG-Py~0m?iUq4!}B+O?UuA8>si}Y=Kk(2*~aK0Ae*q9+3ji%OYfB8PS{bC<EF7BnF9x;P^4)nX~K+XjR)DaR{?HpJjj{uZa6)S9SxSXbRQ)+Z_$pw|X6c?cKdj)B5r(f-{IXh|39cWMp`LcWg8T5(yg^{<4TrIJ;6OUv>nvb(Ey%Auxq3{smYdtOQR~gFuhx>nh4d>n#AnB~7OJ!Rn%jhZiaE<nt@1lXc`aiwP2)^$!ko=IjI|+IWnLg}AQt90moZwV;FPjEXZG1gEoU+^}$lI-dY8m%^P|WH3#jaPv5ZfxF#~rws}~F=8-uwdLz0%FbsYE@a4nyXWi0ItqJMlV;PT9)UUvfpZi{_PGji@$1%+;>X|s1>fK;Ks-%Qc!OrOh|n}jlQ9tDB_v31#tG2Y{@L#S-Vg}TPSlP2H3^W>Q8oE7nv$l{+D7KaC=Uk{frozqYu+Rr0Z>e%P_n&U{v0I^;P&c;*c1B$@!a`fAAnzzx}$^=BNF22AO&Ixi2WaMV8{DgXL}JUc=4#S)$P>-c1f4jU@!TIZNM%`7ahI>n_lTcKIm~&RoYt!+nG#}r~~1h3kicAY>>dx^Wws8(%Tk?>jDman4$o3Hp?z%GuU~J1e`A-IJX5HYnb;8MXiu<<1)_i5RyPS5u{q2H|%ntpExyD7`d-zM+cgOd6YU37JGZh3HJdSyIxMn2ph48XcD3>lU)I2Ls1k&FNBs5!$81kT8H|>3T)E5K=D5&uhCYRvJdKi0IrbheFQ6_sy&<M@EL~&Xq40^n{k;^Z?@w$Q$XDCLu`_yad?iJ7I0l1VA~qNx(;Vt739t`KdeBGImlrxsEp(`NftowS~M8sDd3M!&Z7weeHacWsQ*04v*D9Ta@NjrIF(D=aDp)EZ=DVIqjr0%)8F1bvn)Q#dto$Sc{@b)9-Vvm?;e_m7NFtF4?uH`l5{Zml|kU|X4z!)b_Q|)nr+sCo2P4L`#hOqV<H@fq|pDFaOL~SZ1^e8TEUBCK7l@=?Mckgs1+P>ig1E_xfT2#INkv_4B_8LsL*MD{t+O=Hxn%*!%wf6iQvtm067CX@D6-PBFG_X+q22x1sjSz6d$ra*|U=vHUHBhDi-+z&YGZjjOzD<^#c<E$_Oxqwtq}#pC|F?JbsBXnfCh~2qZByc=i5m_=h*Ij>6MFoxVGI6&}BUXKe9W_x#D*3lJvd`xBBF<xkP$TVe(WWI9Unqb!?c;^9$xnE+!z{h4?CR&b6V@Q}YC)M%-v$3UGYZ?kxq<OBuz;teB4L)M}doW_%wAjN0F=<V(<65PGry%rMOAWj$4;Kzi4G~oDm1RZ3jAWFwmKBuEENnt#A87<P`#aj^m#fGvc3{`j|TJWD^HbegG-R%5)62pf-v&k8cU&LpN^WSID{DRH4yThgn|L(K-j?*BYN1xMhku;_YGSq>9gVOsU|EnohA!S`^&k6L|q9|r*15_Ns8?o}iKwDw3e!?tWi!?zl48(8!C?BpjpWdZo9a6#iOVsHz%Xo$Z{PB+6n=l3mmYIk%Y1_fhUt;GQcbz-mQ#pBV_ri`})ta@-*7;EG3pKWjD1;Hay)6=-c6&Q3VZ<8O8QG&YebUfo_AdTX3<hcZnagA1f9r>nWQZy&KTIhA;5v9QTYy5pImCCf#buUcPhd-tRnqG}2EcVk^yf=%F{U4H(SXR;13yO@TK|~-3qR4y8#B`$8u#&o>1;yQ!avVuQ(Gu`(3!mwyZR8*415y6A_)tCF|hlzXmoq+j{Xv%kNhNS(0>PmJUc&YblST+t)SE1>*Ig-JE-iD`DAxzhYTpYJNt~AA`*Z^!o}nK0+`BhQ8YfqS7$R&zz4zO?+F!;)A^$K?vL@+;cOH?Zw8ysgEwT<_)Z|+bDeyUDCa%OP1fiph$q09lW`!1YX7{*3zGTsc+}t{4F>SX%VtH_#xZWPjb5emSJ?SR<-FN3HYcu4==Cg(`8x*WJ#MAi>-SsTK8G@zxv$`;@e}Sj+N_1P8~n^Q)FYjZTT2gq4VdNxqI7U$6PKGc?avLDX6OqIo;}w=LQ>~A06gVp4{i)dMy;SA2H9)|&w`woCTKqqbrO>9`lZk&-&TJ%noWC%N(i`3iDD|a_#5cJGFJF&;4C7z@pNJne+_mTM9WDp1EpmzxkR%f!lR(pp&}U#ecJ>wfr>{a&rV;GZO$a3%P5(keYCL_R8C8=Xq*rTg~N+Df7H;Xg0})7_!KCQpC`ljN&NX0oe=g}BQz|h-h+)@OC*@%lchM&PVz8EoYIw{a->`p#q{nyb2pEV(La|rLxvjl2LR~vZQF;lPusS=*V@_<ly><5KyXjx51q`iX&#RxLN!20FXA>o^q+(*9+JVdA!z}e&1aV6J{Zu3jhgM^B2L)~RC`=Pu(n}L*Mv7WTKVUU_Yc~%8;+_*+RxST$ddJ&lIS3%p|v4Z(IAhH8sbC}IWpyRSQe**3Yl_3Up(gOnOtg1aEoVbytY^{d(P?nhB9gwi!3E_&nI!i{{`|`cQd|o_JX6~Xvt7&*%_&Ra?;9PmZ17FNzTz`7P3~_(7)9nSEBk12J|}t7AXugNZMIf?U`u18e~LIh=+2<S7^S7gzmI_-L}Eq=!gt+jG#A~zik_vZQz&>L&~tkhbc56VZsAh117hI=XyLFE`Sn<<HEWtJJg`*qQ?S|TSb4N0ZQPK{=SxPr0660ubiyGb)7NW?$MF>-E?Fa2Kc8$yZU?kBqa9t`yI)0xSK<mlf@xKD_JGgf~6$sxjsZYhcDR;OPDXtIQbf){i8ixWT@7JKqo%IJn8xc=<8Fp^iN4Tg7p&b5|e4qG7!ky^hz5@vp8@{hr}IJQ&V$vaD;uJ*v<NZP}fTOewRgQ&dwsVmcL5d{~^jQ<D9iVx%~<lTa<UsjYMqjhnf%0Iuqr$xAxEyv9aCX+LF6eLJKS|Y0_(j<%2*l8Qen7(Vhwo5O-mAMy?LH6X^656^|P0Z|E8G7l{@DZej524d}RnZ18V@i)b6W$!xZ`o4nmzSY~uSpf5vizvlx9!xC!`!jpM>xe^(UBd!|kF%-@F)fL!FspyaCjMGsz9PL+sha`gS?aqFy+u8oB?U?>Qd6&Vi6scFSGGV6X_a62YK-19}1Wl)dt4>dQf;QYD6nCi9J=&&2Hcef$P#;aBdE=UWa;<RbR~-m9|B(JeN|<DJO{-v!noYgWMfy35<{{|9dN-U-qd?>7hy4xkHBH32k=3MU8wznBb5Jsxs52GUMhnz}SkN(Zh=OsW97!eMhgiL3q&PWv`}WPryWqu9aQNoc_XqFD*Y5E6`<F+-@$2^oFOOf`{->jVNIBJjGY1X+yEuOcx%NM#2a6$k3XFjAp@-J!_A7dpB$siJFF@J4{fg#O@{dcuiO~7(_A7I~{5nBDvvucQpB5~bK{9G*(dS0f-wKY@Y+@dKt09*wxszDPU=o5#12WUM96}x+mof;S@Bte%@`CXqSDBp%ueRAlJMQ9(ws_^yPdzqzJnTdIi_ZGG*If?C7`3_my56eU>uC%;5rx8_-9`!ZIlovUC$fBzjnRo!^8n4slM;sgdA5ibnLW<UQ|B7YgL4&lhZyC@H2NF<A!n{e-UOs0zKOnt!t*c>2GoQ0MmZqzcP|)j#?o8SelEX|Z@Akcyg5tqCrLVnBQFLEo=}?;>ClM2iu7WNE|QZL{yEIhZIb^TWoHY#!qq;4(@^u1L%Ht*;(i{l4Mu!s>@tUAxrTTA1t?{+5YA?t4WryWGWAd4N+*#!?TEQB$Gq}Ipd$#1g2PWaBcp;8vJ(wL8F|PxSL;CzFM0{tmd`S8Pl|QYrLK2A!n3c<?tSmjjhvL|s(MnsRWQfI>c+D7I*o-FBVLAF#)R5!^5$CoGw%4c`iJOVw9;4c=niv)kM1%<_^f1poVwf2P`X`D=$)3-;9xBK8$^ene6qtSG(W`_A3X8{GZ?_vXhDjn^8yrCj~=`pA^ToOijdRy()@8FMKS0^6M=w1WDQ5oqGs1NN2}lgK-+Hr?-DvjowbE)pQnKd4e7**Ubaa}H}ed1ZTYI%MlZ68J<(SLY7=U)gTZ(2<KcJV^XJBTf1V|2125P#v9Iccmu;eXm;5jr8CUEi2_+HW`Rp8o67CBXa_1;eiCwQK*~7h%0LMk(!=QY4cSGCG&Z!5cSZ|J~T3rX~Z`y0$H%fe8XOTN7Hi6&@8hEord=ZF>U8163;^qTk5IXo5fg%JZ2j{;ArmqH}%HwZPGr+6HA~Q7y{*kLP@Xv)ABF^3?Ly#e_NC3RVzY`e^gObpCtF(s~oTGSqP>MiB4}kTPcuIy?_%<4kGc*8Gz(SHj51?9CeEn)sETYK^bh2N_%aHYv_InB~KuwcVxUm6Vr5~^#1B==)uyG~pgzH$9TfzF#cVcOeLYI5<IgPVJCHz}QYkd-H(K928{ZY|S0oMq$s2LtKwC*sYm1gyp(}&85$<QDz&<UpcSD2JQ<R2z+TvW{|5;0o}yD=1SILlx`^I19q;DMG+fl#C(5GJlWH*z=Z0)uQW`}=egyVLD&wR$~na*5I1iDMD{Jy2dyCALeiQMgT>aE+$N8N+^N-z^$WPXa~_<~84xcXZs1zU`qF;(tB|Gffc|Iz_peLBgL0?y+bkzyOePS%6?R&r#PwwZ<<HD2M+xyUtR8@wTlZqh;R!pXEd3LNWF?q_wovInz|+b^jMP&UJ><U)h(w8{<~pO)~94VVyk^adQzcMYPX&Hi^aLjS#P)p$Nab@3z^`ZZ!PX|2Dh{fdvROM_r8UGR&T)uhd+jh-eg1mA?*tOQ;9`aJ%<Tr{AM9-RtyPy}orl5u=A7lkDhEn?LXwkt1QrSug*9-zKn)dp#>&#06caXhs@~W~{s6p7EFD979x9Y~pOeA`c~N7&5j<jFtijhae0YvZL3VqiW&|1&Id#m@2=4&}Sdx!fIegHt8X>PXl7o6wLDH&6q6z$UBN8&H`>)HfOwXrx{CFt6NeH)(!{Z*pFUUETj^BwkR|egKSj<a)f3ig1T_Ydw{tzm4VIp1jCUsKc~}%WmBbDX9f~H#C$9&;#E@9Xr5op3jUqCw_vLP{Lsy>B*5uH3|?j>sf^`DRf0!t&!gOq+%}ffT4^CQrv5{gfXFhr;uANQ9_p*`F9WdTXAmtYo?i)V(PWZXN3ZqQ-2ytIfbZNWjKOMjPJR2p<s}W>fwRnKgyA4=ULwtjqs12$1)p0>sYUc&&klwlD&~l#C%~uU3VO#$M@G$<^2Tr?n*w%>F!DPF){u!{Zd3=7l-+BV5jdjyyS)xk{k{D?7XK5xpPqh>=7WDISi4@QzZZP^ek~}uz&D=z{b6+fOHJX_F4CEU8h1P3j(ob48S_Ya04D?85zO41EFQx?UWEROLyMo<J^&%8T#mr1O@M)Vi!Lg6_PaaGM|2saF_J3-Ud|J0&<#qIq<LdqW#>?;yuLK?<s;|Q$1*~Q%mu!Wm@x-m=iKDchV$CE#wXW-+a>Er-`Vov-CK}#&zE8R%cDk_m(*hg{AP<>TAK1My?k%bRv$+h2>;KljX&Wp@U{!YM^MJ6c;`Bw#3)+*v{qq*dnZF>k3lLseS7fk_~2#u;^^Jc;k)BEuf@J^Z?S#f+HLi^b&uF1+6HNJAv+jmJ83k7$B*gGCcZ)|uH0DPowV6UQ$PNGd~y3-fAQw^>)ZcybQ-*Tb9nIb=yct`)t)cw@3nQJ>+HPwNm<~q`ob@$3m2Y}w?0RxlBV&OIZV2-UR;p00iRJ6B6o;I@iZ^;^|G%W>Q&pvY8e@<B8O!Pmwx*%(Bwv(vETfJZV4Y4ZgZEW-Om0tbC#*aJupIPV8km82B5#4;q6$<(-tku8AAwL@sO@v?~>ss7B124*p!$;wvhmEf0_Z24ezD7c;!*&oLA(-gJ9V0^vRicw+H_+&-7(*(80Q)gOz?L8=Z(82p4&rhf~uEjz(tmZxi15K-;vag26z>6$}R4^4-u6pmgFV|9#7Q|0!AwePr1a)$myK1O&Ot(_gdAo46E+MZ;H@G52zo_po_*nUItOEIEYp6<#bx4I_3z076X;K%qQ)7+(Cyf~VrcSvpS62ZQP5oLr_VV>x*WAd~^B13LW;c~s}#_Ndq0@3-6W-tP8Zyj305S<;n1w6ml!IUHgP^G2UVGFQfT3g5{Sbw*n9;b}3$U@PXaj{e0aAk}Yt2hviEGYomCTD&Y;^n0GMf+WVA2XS5zGsYjOG;9BdMZCaAc)4|)qA_HaUGd-i9S_$W^3T}-KQMXnMN~vB@q?l!1JVf&0Lji-v?F_PNU}bQbY)Lo&W4fpQ|}ywR6bY3C?kLJmu9@BXv3aLp_vn;B@j{mIjh2lb#DRbA>qRY(17&N@X_1@1$Mq=NdfG5R4%uhJ;6(1#fL}PY{VdU660YQ#hDnueZwPU4O!6*Xbp5{LiIOoDC>a@Aeq+TgdHv8Hy_L3Y~uJSSOo7904=6F;hkD{n=K5P<I4zza)Yi#IX{tWn@^rR3Es1$4gm<JoRZ|~URSzrqK#)15PyzGrt>JXJ?8DT&H3VtS+9cAgZD?_`=gUn5P}E6Vyg$#xYq^ZZ;aSeBQj=3!H`LTxW)z?#h7yqMwIFF_*ffD#_Ewq@SC4t=YR?|D2f-3!UXwu@D^l=B$^!k(genX9kC-6oJN;1;;V2hC37hJm#C-K?$v@W*9o=Z6TyeD1!r0LAix_?9mSuwRI3!D4waV?%zCHaaysGelxANHpP%!S#-O12Uh>8|k<bU`GN6o-F+TDgLHA?*rrh{YJ6ry*JW&*O2kyyz&UC0=kM5g#TOG9<7)ZlyJRvD<S_o042ff<nz|4ms_Ktbbi{I!<pT>@M?`$Ib%6i=Lj-i%s3?&|`5jQ;dJJz@6(~`-SZoCm*W52HK%t8b^W=6hR+KC@8YQ*4ZU9%Y69t*+k8#sa~IA>YRQG_E*BL^fv)Zn;4h{0|RzcXk!yeD#OSmZQJf%omD=;ES^`m7vZTwHMmt3VhBS=7f%F{T__R44UO#*Z#`Ps?y-DZcnRtbsIpn8smaYrpr`LYnDpMENswfk7w<=f!9+z$}%6!3&=CjX1J+@5t$<$j#L#zeG>o6D=9QV2kTFPt`PRfm}jch@eakJQV(9$0MH%`1=FQO!DsFo#ewtR+P08>#&)^MErPzp+~J%a+&bBP)gg>?{r&xWLq`Bt^s}lGRI)>?Ngz4n{oP|lj0(rkjg5}PYv4VhQ7lpIK-oj-$@5GdWq1n$Qz_-h4@+D$=GZEw;~tvPn{3#WMq9i%aZdXUH+joGg;q@{9P1<Ms0GVGn19Ig?31%GDmlQ$FkP;blG6jv|ARl@G>4YY&h}&^onVCt$#j>WVNt80qNlw@VId;KDqW8>O4!*EVwc+n{tJb;BGWaOoxvjODVlALqZ=t3y2I123(<Q5Ki5#iyz_GrXqOP^X((~G4p-WTsAH51{-3!&6JF#dKNg9`3;l-nJsv>C;(xKb^o5Zc&YHq#BDDE3f|zqq|$E_qR}Pg!sYuT1CuQi^kdb#on@1`hG_e1H7L7E+gNgy=9{G*KE2{B?=v%Rj1XDGpRw57lBsJn=cz3Q<L4efSCRWP_q+pzn8%FktZgtjeDnJB-N}!1T@xN2y?%Fe(h7P_?~rPM((Yazy*qdrzB+z=RNBi!P5k6fr#RNp-@iLKIDGf!UeiAP!|~fc9h|&q1>KT9(5iQSrFl@xM6=wx8MchI3D2`+L}7|FQr@s<Q5!57xA6GDGR)#pQZx(`CAa>EnB6hZ|M8JdgZ7}H5Xj+zaRR*K0;M#x6r|??3WL$P=`MY<^z>;rj?3;Cjp;4DRjg?J_T<gmA77p>ZT9luHK6LrpTdLJ2QUA0dVH!i|8U1k`qH#H1DOh@-u0hX?D6G);AwJ}&?n*I<R?&osX-Wk=+kMpw;bPs`l}}sD@n>e#;?-i_)-^h@p_<3+Q7O`j(?9r_4hx%{JVpbKiaN>%aM_j|AtU8IO65;FmLqmSln?pG}`PrSc$Mbd)|qg=tk8-?fu+Z%JY`(jhUm!M}FL$VYoG@e+esD`CvJ7H!frcel~k!`K?r6#>vf*c8E!w!nh_yH&R~l)3fACZm3-`guft?^FX~gXLSa`B1`5Ecpb-*Ffdn{?tW{l>!@?9-l5|4?G&cVyk>^c1dt_yiGPaYIXo?LGsfP%W2H2HATeSV%4{(Qnk}JcNR=h}qqYG?CzS_u%TWuHG{ivM3({!HET4~!5$rB-9o)77N}`in-edE4EJtlhmKk9skxf1qZn{r$a*~5dz@c6*y*(C#3v_K+5aEyS2C{C?q-k=nIv+jMbSu8LEpNUPTK{v@#+6ftz(+^@VG#?JC?f`PKc38@QEf~SV`+$@#y4(oJlNu0KO0^F--bvn24;6-fwARXTUl$D6_pcCCK!h-i$NYn8HQ>y&vutw-kDVH?x5$*#@;S>z;P(0<G8aa>|P|RA|XV>rbvdLhZG|d!Wdzvx2=d_-7mtk2U?xk1KzY%A+hEghQ_^0rw(XKz7nFyj?bUNX;Z7z&v>^x?)GDEsh{N|=#_ptoqekH^W=%D3N{Gl3k))hW$mM6C{7mMmqx{G%3|@@%`vM1H54Vrq~)0hYSJCX;QM!39KVX@PqhZ)X+b2>Z($}Au$E{J|8%U*hH7>rI*+pvg;2sDNi<3R9Lr${I%tGu$+(4lc*suCQX_PU(}5Xo{Bo9y^X`d5u9MPQb04rDo#4GY?_CC|JJ<NmEq%4$G4yvsAW|3JuMYk`WbTw9>g}Ky{&4*I7&XJwL9oavwN<dwe=6cVMca4qpO?q4j^Bw}a?*|Q|7Fv+M<?N{<Ciawph>s4C)&^iN-on6XY(uVO1%}(wfcs3U9Q6RU&vTxK~rc|UiC-2;g<EBVA|SaS<K0LtW|PiZ*D54zkK4ftzy2i1cn;ZuDP}7?s$uCsV~ZFZfR}GYi_AV%ZhH94qQfsEph0-Gme#StDXx8Ce!)E2NQr;gAQ+#QuZ;^22%2dej7=oUH_ark7;dWRjNVo*g5}pnn~R{5A7>Yqj6CePsS~M9`dGcLyL)I^KJ9(Mq7B-R{3}a0{FaP_l72OOan#1dEDKlVRqRp(cabACSGBZ$IePq-~o`YST`rs%hEx0l)dwiFQ}7K>|^<WC1)Xc4h$A)&z&Z~I{iE9v+kPRqXu)>?%V?c?|F|)XHb2P+rSSnfM-T29Fsi!bDYf@&AZI&lnu0eLP#X^%p2F&<}1=%06rDG=uY#4&JO`oz4nzcqVT{o507m{mV>F7;1YY9=VBL!L>FsLJDK!q+Y;IQD3m%$joJF{LdaWM<!ixgA*wiEG78VG!cXy4<FV?sov1XUp3FU!L<#MusU)pV67Hw&{8V}ob81@Vl3`hYDzhrAzRB(mM6MEibhlL~hLyjb5wFr_uxv?~RtCOkQ$0B(>s;z2#G@K>15sn{o)Z(@R87vAo3_hb=piEPPDGwZYRidey|z+3s?}Uf<GCwF)}7T|K2?)k`t_>Yq>&ob5%QHbh|aOs=8eXOlJo*{fRf0ZDgpT}{o((l%<S%E#aB|KcDJ$WD^23|ZenYNG!H)Die&Kj+xQm?rVGdNpw`=ZgBJh}c<SgW!j|K$dwON&X!_NuLMzCie#-F-Xs`4`ZNDlz!#8)53QSd)3*4pL@Xe<N%Fy;eQZq8;+>&B)LRs?4x^|&gLyhtXs{SIs9`$O8%;_K3r-Ch$9PQesOZ2qW#XwK-)V$?p<&qnxtOuZ?@vUh*&S4)(H$r=Wp4=aviw&&m$lNx)zwVCHpV`>cE!r26n^QNf60?9_0^zY&U3CU#k|HKAc&yHQ`S|)eNaZ~)FraCceRoWCK7y5orL3TNqh!29-aL)bd?l7nKR~HfHZ7XX+04wIOU*1dsG+in*}hh3ktJ#>@;QZe^Bi_&nm6FR9f~zp%Mre6OenRp4SH6oSq7pxCcg@qaRoSpSS@;VEee&XKS|FGGREGw=ZpLTKW^}hN(1BCz;5(d$6-AILe)p>PnmUO-InP!DL6@e+iO!pyJwAfOK`Ojh7RM=zHEH+^c#<=Cc$e2KoI_@whXowt{5ogv5hdqW=(j<(k7&5)3B{2j|d_*GSh#TGo)I1I;bzyd`eVTmgP|upvFA<zSE&KaSp;Xnq!)y#&vidc&EnM<qv9gG^5{Iw`M|3e3;`|kz&%Tqv2MRg}Nhkwa@I5X!a<eHAjWI!lQKs7ZG}8@<(b0PDV*nz&-VOV0ipstXY-_kg5#&(%h+rOza9^C0|H+*p<u5zR+Z0uf*nyzF*=c%WrE8O9Mz@p3~}NqQ#fwVIt7gE*DH5zWgoySRz@_K4F=3@e%#-HPGs)>&LEbzN`{jbowTwAi*@MOut0Z*i&u^^ucFo1ItIpb<Qp+y*zN8sqM@(S}A4Kt$bKgS=sio_6Lb&@g*zFq1uZ_t<q%7TpSTs-KZio%=DffyT6-Dmwn%8R$sYP+!#^qS6fdJ<9TfAAh(M3`^??uE6przz0l3YzHfvf*L=r~sn$bf!4w8dx{}i>RI@8Oq5RrLxt4YRsjOVO6zpv!O-`Pcj@84{#-kvY_vBR?TMx1G2G>=aPheuam)}2IGxEOkixXBO-Z@)RLk9QAw^s5tyMdJ=O@Dp!KsTob->h|Ms(2#>7ymoCO!aarqCS#S3FiWB%8W_9(byAp%x*GOyt4$95gyH$mG3H4zSdY31J+#^S;<qh0BpU$kiJmKLF~2a9K+mC!5xe^YUR_QgF>-}`>oPQyLu=?_Vsn<aXhkTD#|tte7InvBH2Z};B1*Z3%DGXZdR$H#B>^3Q6-QPlurm3F3J$Dbcggc=~`0>II1-#l-rpg{miNRq^VC<ai`g*a8{x>83a;(YZNCG8-O=C2B&XZY9!O#^iQ=iUiNG~3G}^%sg}?EtzFj2n1U70rq@w)gV#z5DVA!T4vp6);Z|Oh#aM_kp$)clax2WiVk4dzuzx`GUvdHkwrT>On6marw?F!^K?>ahz=Gr5Jcx+*#!SC$W#$(`iJ5I*_*aPx`uw+(6aPUZ5u<dJl0<k}DlrvVP->0@cDBd``3`vXMgE&azK0SB(39<6^1wZvqGW{_eG>kX4LS#Me57>75KdrGvBT1Fsn31QWxb%KLVzp+q>tdb26{iSMp?pRt=dF3545Tg*{?nIfXYP|r!-H6l*15t;KS>Co)k}Q0)Y~4F^87^!m6jxGO2)G*5EGas;iP(F*`9gnaSVA=cX}7kwxBD{^9`+5&6Pv2T0EmqI_%XijP_~vVrAxsx?9)AA0;4|N3H^9EFt?^z6=q;G$U%pk%|}m}L>}z1z!aw=p>;I_g_5!p(eYfgr2C^%=WIoM=E7MV=kZ)vcmzbAKjzpj^xijs0Mf#nI@BUIW-AI$KJ&S8K0U$WOQ)^9HbZJehrV8VF+tQSKx?_l|LhnxE`Kd#P&|{7CVO4X#qC6Y;qGNT}oRxavIECytJ%>{xeVRdTjyQ|j|d+DRFs)-{2!n1LXLmlf8NhLjiCjP&a8l84EUttSofn}6K0=ic-vB_Wy0qe*zNm`vOWwehiU64qzdRP4|2ic$7#kEC~HeIvWIhpG5RIPL}=8C~}I0>el6i|-h8Y)9Y6kF6Q<TUc`n&rR1{{_V!Iplj!n5q-^PpGoy2mx&Sk+3;n+RlxIKju^8md#A&tBr}AujWuRxZ>@iJ2AIm;)tsFr%)#`~)dPJp=yw_PX!3eh_UU5Mz<3H4cGzIsu6wXF9fuAe3&B4kT@c?e|802I`O@!h_rvXezZ>?q`e$K(d%I^}0_Y=Dx~G(GNUbHC+xOR8a=*VF_IL0<alij?OD?4a_*-93zBp@Gu+jS|r4PP-EVR-C2_*p$rKv<6<%UBOS}sK}%nc0?JAjFg;i#}WRn>*?GypZ|n^u4w_`dY3<ocJTgZlJzYr<ut+A9BzkBLZSO%GYkg&Yt{NvQGIEU4SEsG6UUdAZ<{Aw@eF(yEzjZD7k!p1i!0sj)~dv34r^<u2*5r`)e^1+q8ArbU;|lplMl^%{NJZfk~oz`7MhCC0PKsL=^g_zA9wsJ8-aVK_R_bNvKGqftW<32d*|H`p*-`B~toqf_fU1%#?dFna{^tYc)P+7qwkTV}<OW&m|ER)_o9vg(wWUCZ6YlA5UO>U|#a;TxWnE5zm(_ttND6Qb8f^7`ppnRourqmwt<CmJwRMfjCg?o{wBtK)86txFr6MBS20cxjw7t+}2{a%cV3<?0oA;)+IzAkMwLG$l?+avsYqKRmuS5-~j1UH3Eh1pE3F2CCsCn$AOH<qa=QEM;y)yqdmoJzaZAUn&1}y7pmu>084tZkDaS@oRq=@Xk`qM0)ipakYfPzJ0w^Wyx7;r%{(!(0keSl6eW$F!REDj1A3JiNgNethsDl?vy1za7!VR(>rT-&9gkT6%g>it6%)Ps=E<;l0YC#dxF-RSAgQjJLH#a8a9|q)Nb4q*ky_Nw&xbkxy^FtYS=b>$)L&yaw@UtcVJ>st6j%w!%3pJ^op)@QQRowq^2$(!YC5=+o$?2@j9A}IoPW+a#8w5knc52qJXoSwS}7of%nZoT$z{6K)hGuV<3~=3oi*=(g%C%9EoKMvCQYrS{^ZsBK0B8Rhky#V1Pj9LB?jidH78A%`%>erE{E%R2d<d@ru&VBu4z|rt!@1vfO7nw=-s!mx3GA##D<?`?Sqai+M3FQ2ntl%+#liZ(Y%vrbQo(@O%6j_qfj5{NI$D;O+XknKXa<Vk%kOo0UE*XQ?+h6W|jt^D+^&fAgu~r0OB|Wj?Ghocr>%t+K(I2sWryP1uEo+FfpZXDYDUz-xu~HNG+ErH{8|N|t=)H&IgvfCq(I{?}5oG4jWk;Hio1rhsQTgnF2f_@HT7ONSt2k)OHXX;v;?@<I{Y7(A=qX6+lwuP@2?Gd0c~Tv&{hp)8}N<iww&T)E2Y*@-<oq74)oc4I`kY_~oKb}l|Vt}+(g)FcJZ%1c&H`AdBeSQQG|nYWqYGYyN|u1ey`#OyNVXXfrcHI!^zeYNH&7<G$RUDanKv{qNWv_z9<qL+$59adyrbX{AYFWWHU+;&Ft3`^ytjca;&E&cs1p~IDHpO!Ifrn}2<6<Da9bV+2<!!GlNhjlLebr(Gl@J0Q4Qmi`>G1A+E<zleIMy$9HLub^Fog4b&9zOr2VkK;y&>-RQSGluv`c~0J^aCk<{Mb<C=aENm7#pnpa$BN#s5Gn!-r3JI5~m@*ql>SmSeci$ZVc9q(6Fm(`6?!8upTGLkSQ^gWCt4zj<7Ck%g8%v$&GxK##ZszDNaIJJe3Nza%*sDNi}Op#9nFGw=Tb!yQ+x0OuSuHv|UxKtrBUa^M_91xm;>pCwL`V3r$KyTP~WReEMve)GDM^-+nBSNPUZ$neHrxK*}}1M-7*~wU%*6WQ<MECK^I)UEXFv`dbq7bZYoiI=D8tCcxp@6#k|dGhM23IHowhPbN+@bw>!6z42LnyVqHgA~|8%++}ti$KKTnuBqTJ@QLd{{MB|WV7wH{=M4DIEDMnZH?C<;*KBG6wY-I!c+wGSDAtM}2uPI}06d|<GS1HL(&9v)OX=d7%E&3%lSW-1>yd><V|wCg$=Qt7(qb;6<%0&+E*Bi^PJd~L3HoX_lx2yjBhoCNOAY61^wSbm4U<v)j(O_v-BTez-QFHQlxR|y4+H|0IR%wCKKY!NSj9bO3g%U%aUrN#o}AB|NZyX6JF{izPjzjYT{7kzqSti>Fnvwzdq{UHHJ=BzZT!n-g>yuOkV>+Hca55V)ACuxFbP&!8QYhnDPq&84sYgdmSy(g&KULbtdynwc@TizC;?1hwe798lE2==`RBRFXIX^*txm<SOR~UO3-#=rmK;tZ6@TEWFQw@QL;@s1Xi=&Mje)As(oF=9eREO{ltbuVN838yZW4ieoN?bdzvEf2ecr89R5kK^)y!vbXDU!a3GPP?e4xFlBoG$#jL=voBDOl4FVBY5+Y^|mBarM#mj<nM%8bk<ZO9wOK+0v9ST^6LE@T^SnBq>JesLi|UUF%IL!e*YvuDowwM#bUD#x>%nQpvR;#q!Q&@q*KV_M}9)Q8i7I7%+VVVq1FhPRahf$8^n7D<6g^Vtqc@-zFEjXlWadXmkSEE+#E*Np^Od=KPU`9is!>YC~2WK>*me@<*=!>QM_@v3Xs=x3v3oRE($%Nq{k`hkwF8e)(Ntj*_u`qSjgPQQTwZ3h1bo1z#L7hZwJVm{4g@I>V;*D%=Q^k?7=FzBDGkg*TaK?4)r<F|%btRtQAg0|Zif+n}0eO2W@F-|50Zd-Y}AKV3*nhS$oXQ=Q2HHPU(b=#tbEo&|28MPT_L(lxxY!?@C$}%rHi3kEUK)d}djcMeq(f?NYYka$-{3D&+@r2sldxw1Cdw<1U@>G5DZ<{*B7CU>O9rUc*Cmb1q+92>qZyxTdg_dsh27>}5Lt2Sk!;&0R%7b<or6~w{@s}YwgQJm@FP1VCivHFecD@4UmB8N&kM-kMDSzAPYjj?1KBx6&U-#j7<XfuTjdfwd18bXjn)6uB3Pq7=#7w!T%C0v}(uQvW&9WO)@o6xm{5VM-oT0BHZ1mYx5D^sB4r`=Bb8)UB-GLUU>%R7oXo~oK@Ut%|8(J0ba=pc>2#ZgzWhcraMCP%jY23I`$?d1&SSnj&qvOlwt+5}IOxgTjLg5Z9Mh|Vjb9%UvRB$U%3fmig6$WJ>V@bObru4Ulmj-)KxVcp4|J%Y(caI*Nf8GOEb+`YD<N?a*-F~hoKoWk%s)2rGA`+!2&j<Y?9Y8cGm-+bzaB{BzG?V7?mg7Y{v0a5EBcr<`HgS7{#Vn0)adPD5ApyiDOuS6?Dn3!eY(8V(>Yli41y*nTX^sAPORYy-sns1m^7rMbdrUKIiH=*;pN1h;oD2QeK?B|>AvM1W5j^ATH?i=LL)MZ()Bxty?TpAd1KtXs5P+7p*UyOxIiS?Nekslrd8lDiBR%x`)xqEELW)vm5kp1a9E4ie^oF!>__vKc&IQVMzU;?ayW#Hs*?zb??nmM7u)8fbTzto76a1t2<N5PotJ7?%o)83`FK6TJy|BN#GYY%IxF5#bXZtmJ>g~Ec?e|BWu)lwH7IwF`x?#5)pP4-wQ|UJKVo~F1F&Ml^axBbCd6d0*fAFSqs&Ze?vMFfnWQa#lQXfBvhu?h@J|~dUWA|EJehte2PDZ(xSpa43$ytwi?A^$tXU~Im=|*$O9<H~1Us{2X^rg^etG|@NjPy+|e)$_aL6h{+l%BFsH1>GipG?(zSEyCs<2fBfrf#?R8LQOww^;&B1z#b#GHQ-_^9QmP&s+7t=A>mZL4_g!OK9G*Z(eOk7>`y;y2!G0R87oWy_g))XFc@<r@#6mq_0N8D7^-pw^vykK&w_D)9HIzNeMe)H3Jw=qrW9%qhFl6nJ7LSpEt0?DMCOmgUX1yx0K}}RsW`vk}P){{%IB@RgrvpnM$`D<;&SHa%)kVaw9fY<xkC?^Tix<$bI|NlmED3x#N*^srE@r<*b&WUior?mY`z^O8U`K=8s>Jih89?)jSgm!vg8@=+9eir`^M!-r5)gAX~#nKh;P??oXuV)Tpm7#%q9Xc@>7TH)U6BHy`}@xIGcMOY(A)A-`2wV1UoB;qf4JauqGXLb|OTxCg?ErzU<qa5C@G#NBo9Nl(SPZ2?eE*G&j_H?u2sv4W!bp)?10)$WRN&|<5T5hz^A5Y3`7I)h@Z4i3pYoM*Gq0+_IB#69ntPNWv)2)9#=T8_}lyY*Skuo2sn8bPExBUlbvs&wA^BuV4&Jj${p$NB^%&eMDrykm0NGd&Be462eccQn?)h*qcB3btCoF1+b)w}PEcS&|iz8X>pys)&OnJnVt<#nX@+{TiGBy?XyP{KK19N8#z8PTw893Xfl4A$bcCtcmrSK3uh)?OCZ(EVw6GUbs<5k5#^?$0|)izvpIvvSE>fzMGEj+oQJ(Jk7PmM{okEZ1ku4;RW<LTjU|k>;iC{i=as!GHE;1r0xHW`F_mrws_@M_UGRe?sqG3UwYivLe<?BFqQici);`}v*s8njHnygN2LdL#dn?%tg7$yX;(Xw%e)v%PavgOAZrL`E@AOG8Pf5~;-3Lmfh&DZiVFY}p2x_43aL`76tQ8ql)TL_!IbU?gX!hmTp{+tqcWapU0KJ6T(TXnV)p>sfm%KTJ2$S2PrQ|WJfUY!-f0KtORioqt&CkxMD6yHax}_U?_!o0n9z;`OOOsPNK{m&3NAy{i$!w^HB69^rxkH%)-kUPO8dzQ5UUjFgRSO=D(6HVu%k`JAd?DH7;aD;SCdEUv4UPoF|>Es(R@1fR33)_+n-IiRyr`1)WP7z4CAvJau@6mwPoH_-vCmPoTn_02`Ak&i39a~a6B^Z?`~>9l!88mW*E3B4^_Mn%MD*rAHeRt(QG+ez)`yu>~~9#0d*|7qQg1$Bq%Bsj0e$|WSgl5N~>lj1S8!Hn?%<^H7zu}Bw$i3o{58ziqmrihcRAG3bE0JxfbZ_<uTrj49j3t8%~w5stvDlnE9Mf@0jGKEQ6r}l@FmGYw*_?@XgvN8IOa_&GQ7d=m{Op{E0M=K~GQeZ1@D%1fXtbd2m*v(HaY<8g+W(&dyG|J?icDhdXB=4|aC?{k6@_&01rvZES4R8}E0&3pTg*TRXu9{I}Z);J3_9jA390L!NL;qiUSo4J6NLtvNX)_%UU<ANYIWDEu;7fJDPC4UA}QU(Xv$dfwnYvqTqZG`ae7%miS4(q5B`m{h%uM83CV8+jA4iQVn-?zrD>_xro?XgjXCiK3GwTPT|02KKtRf$-m5v46sPt(bK~79xXpfXA#dtJXk(OVy`+dHnl7ybIr+y!rlUKo=<}3c^3A7SWwUHanx3TJLv<2PeOO<Nc1fpCohYJ4>3dz8z=DITE3>#pH98ee#TZ_~zxC*MAC6|8V@)`j%rB8ZzlS`%An)<1!CGr~#_qATZ!X;U?_<3jyBk_O<}n8}M(R0bMti2fAn;C^H~wszX-i3q^8Z!f}R9bV!TOV~|Q<?52*k8flVNRndbAAx0|IgJo^j{45#eRBCRKA~^?{%})$#eA*YAV6z;*K{(XHS`0Z-!!+m}*#%`qHF}XB9P;;r!COSY7T=MrZ9D|<?+&M9&X%|l_gWD}<Lw^WB!eZ)!V>A0F4978eW4pm7Aoq}nH-GUBm)!SYEiREa#l^tSOZSU_B);3-QIq?y|X*oiO#z9C>igfnvU^Cw1us0*gx3B-~Dcl)pCFhjF6(|N&ZwlI$I=@5h}SHEuXS%gZPT+7g}4+>5M0-Gy>?45$I1DHSO5c6dNqK&P&t-AL50Rm>u0Y$*1-N*^QKV%WE$iU+`(#57kVqH#k{(RBN-j3{<`0suADx-q|SXZb$9*-tP8pyxpm{=~_3{8?QA)pzq?L-GKl05a|A!#x)Fl2k7gUh>YkREQ*U+mi$=>f~~;4UQ|j`%TapWC#)ORKYIs)Wf6<c_$b!Br4~8=$8`325|7S#dO7XE%lI;$@Sf#EF3*&u$Y`lVEiENdzaY0AdTS=Z++;N%7|LSQKCB5T4Y_xS#C6p<K8$KIp4x2QQX0Ob4K3gH=!sozA>Lc}ZRHKss#iL@`XC$12C#PZp*B<$>C$)AyP>+N+}z$$y}eZ@#Y?MHQ*4QI_~glx;NT2s6r9swlz_N|@l?YMeSNu0I?JJhj!U4TVrJeV4lZW%HY2pev3t9U&@yT4)$!@+@tfD-$<gVXmp{J4Ki?f6{_&J9-<h={SgGKOQfWA1{t-x=kP5gEp&>hG{qM3U%~>`*`u&F}y95r)f02fQWoa4j>hwtZ=xjSOI3H5jtfb^Qzo39P{`xKcYFTel9>rq4zBIS8CHirMXo<`&xtZ$C9RL09L_IqmxfQl#yOI<BTpNSs5RqRP#~a(B5JF(sCQYQf?Sg^8Y$tQM=QgT-<VH5!ahk^FHoS>_*%DTp4wDsjn6S`ob+-)VnGYZe`qWa8jilN(yD(H}wg^UASQjVpFN-9zih!|KuV)2UjE$tA)ouu-^Vo3bK{WHnG`ftE2`hTc)prZ@9<tu#3wwdIs#s+4ar!pO4c%SN#CHN1A^T+k2QwZyO=ydDnRdLl)#`P9TSO<**(YY7i$~=<#fwu{Svu%ldd`-AXcOQcQ!z+M1N$)>13G06nc7~e*?P8;Pt<yIQ!)r`X7w0my;p4r^Mw}*Tqe-RW{)0vad$k?TRVMb4TBTCBhZD%vV>e{we(vp^|pTMxqd1Rrj~0>Q<`tqbk7pZ@1J4*iciR(o;}m~m2k7QZ|f-guSzuu{E>CR2#vZsYBar72udekPGWR@_{P3DV24$vAmzh<5=11NAQjQ(Yo?qOpMPhO9hi=Xg4LM6hXQI|OmaNq(&Wa-m|1rB!GuDwsV%+NPE+PX^vy}jwI|^QLJ^Bc+|m5zNnUDC;<VAUqcghA=33y*zG#jJLJWXUh?OGmAI?B+8J2WM14BZ-7c4gv9)<qi9v+2lkkfiw+EJL0{nm3B?5wPIPC|2P@+QYStEx*MhFWxqrP=qLGB3Pf#;ONO&yh<XTh~_XV3l3bHp(x1u}umX$Y&Q#K1WwM7YHzQA?w4q|DnU}Bv7p^*KH@M_PN0^n3N>UuX6NLnWxCbts$%;dcv9}%Uh`53aS2_Ohax?v5Ir!-lZ)`ldgH5NMTqetJ8Czk$o8Nw?I!lPts6b>aMw4$yXMzwoc+=JUiQ6QV6zpc9bMsb#y%10P+vqYOWBPO%2sRW_!jMAfBwF00F4c)WsD?PvCju<r6nep&?{36#&62q#VJ>*!X-=c{@kP6YfEE&7WQibZFB_B@lt;halSPwf44`!RV&R82Sb(o$LZB=4a+A&Nj!<5W~Xw$wC*GAWCK0z#th-WVTSQ9nUh>7i!EoX{S*!o1$Ol<ccl>0^mE@mG}y;4uTO~_y?GB^djcF-<Gvb_`Lb?kkemU=tP_$c<oW}dA2u<gmImMZDe&W&dM&$jKU{etJ7^F&s*XX4r@5L24{J01T^Z?@aBTV7oMZFQOO{9xr{gT*<nc^l9}3rM%?HWhtnvH&SN5x1jm*+VU|hJm(18ALc?L4=SDRVP5_=+9SV;-J&U?4rqvM8?jfArxJ6G^7Rpu@$R1c2Gg9`~O>r*{eOQqy+DQOj785SBfjdRQc%SVdss)5#0=cw=ogQB(nWr<!LnKR$sy4#)2qhQ4^HIf_(t5K}55v&>2`k$}|1FD^rXX`GC%W-INgHNfWf^mVv9v@^noJav=RgLfz;kQA^!kgcALKgC`gzvP_1bQS7|!DHA5=L2lrZD<Bede=OHz@qK>bQ{k#qXO32SMA1k*b*y@W$Fv%Q+-dogo8{hrE#@znjQG@#$|4D|Ka3d$0|I!=*1CVw#a4&w>K=g%9qZm71nqI};H66BbwT!!M;1XoSwueB|can-eh%#DAz{3~-+)Q6KLpC3Ea=H!KdlY-mF3)!K)|0EiG21>_Z2Vc;Grd{HXIks~fAjnYqx$;W38pWS*5e2@nWyj@d>V@>B%f18{^;_I=q0&?qb!WSp5>Ty&h3>K>{)dX`z^9fvHHs8)eA!ulTPG*@P~3P4kco_ifgx5S=>f8yOrwb<U}J8DG>u@kbk{SP&E}fi+@^k5XYVfLwKjaU$a%M*=Er%l3CcCe<m5uMIZILICh~8H*rX=xbvvzYH`v(k_gg!<8A6akX#-#`d#<ksHxV)ord`-}Ttk&%r(Sx}G>narH$uU+xnull$bKyEh=P#gf7qnLygHLJw|O}tm@49%^fa+K&$ZR&gh(Q6Uv)F2J#d`jQFXH)fO0xjx0qI_lyh99ONo`Gom+oQqp+@j-V-dE=Ikus&7z>RSuOw&rzh^>_|C1iK4skZUPigd#aAU!#rRCTZisCDHX14Emkk=oYLOmye2-PD)Us7=-Ewue(mhr;3<jrA)qT|A#F9I5Aszv&8uIln6^vxRe5{g$x_<iMG`+EfryH*zvC^)yJI&BY33qLE!7i|u;7xTG4@lTn*r>82;$Dy7|3d2Dyf5LE-GymMnR;8m@cBb#k1-3od$)iIC<;rL9Wm}W@wax;D$Bm<Cr_;Jjt8*LwWaS64(th3pUPE>6;_>67eY?uC<Ks+Zt~5&;Z%C_>Iaj58nvOdtZweTNgC<wD+&S{>gtTrqB3>K%CbJ?41c{bF62;DZZ?%$%idO>dH3`{6cy%tk?jY=E=QM?Uq@KLJu+yC2%;tPMztn*uCNP{7~@%~VTd8fm*^^9c*v~14~A^mTH0gN?d`_Jh2?Umc7$Zbr1ISM(g9R=2=M){f~(amZ0g=#S@oBSY*CfApJ;Ep{QXT?lqe#X`=(@u4X>Q7cF(%@@eFyh6z^!)mSP<{U!kdw*Tp)iZM2ql>P0*{aN9fFoo=gjxAP{=XsQ~9Xe};YFQUsBT@=}!3X2Y1*M$$k@Y&sQ;7S>uwn7(`Rm@B@_hj6Zch#oAqAc6)7>goMwtE&EjyL!3;~}%em3?GXy+z{ploPo99UeTfv(@VD+zH@Tj3FKA%0?K34LHbdFbfnwS^Ac!>cbnKfl{*y17_E}>Gv6Rktc8kOUqGEBT}-?oqO$oNyB7h!ledft8T+?#xNVexY4Z9)K^u8BP=Vos051jX2tv}#&W8+#mL65xojVEzr4z+){F%WeG<LzfAc6!hL3R7{PtH|_+w(k=^TMj%viF8fD$aMFM;Jv^=ndX#Gne$d~hu`7>E@zg+cHu!DaB<n@Xsco)#GhEQo={W5tVK9|(<w^FXCdV@V%X^r8|1B|8$V%snM%<02zb&fRSm2)Mu1+PgF1v~T<BUiLk$>Q3bj5FOfC1D^VTuvra+D$ANQpp_U6OO9V@)1^{qy~&h|E{hZh1TfIENsPyRQOu?k@;T3D=YW;W$%l;7v&m%knVGQD*)&PfG9DEd>&>Njbi)G#x;+eT+UWMSx~*RS&Zwe}t%prI=x%K%ut)NmBUw^olC}ld$*rMD^s0ka>F56KZ<bxA@~l1VlDKMutRMezzrJ2U>j?jqwT>kPX_<wD-vOGY<_FY8O6DjRf(Af2r7}?8c)f_L`zpCv(0%l5l#rRCj2_F*vCfFfDPs9+G8wv*&)B&oODowjk&tTbVPs&zNNqF@JaA=XXNv_U^-!|!)uuWz`Vi#Y<xbWyHGMuqt%{f0fPr$>ZTUh>>~!8*-f#C(mWD6^`;XdK?JNRir=86xJIN#-7me<I({3*(;c33L>oo6t>2LMNVSm)`h5hb!KiuBg>biN*|98G}GhaC&6M4ub7zCeRz$uAypya?M8=9NwAb6}fKn67&AQ>L-_30KEr^JJVF&RDgH%BW5K@|V6qkt$6Ec1BPGV-J6qa9G>WDKCB!UgD$MS_Q2xY489yeZzhamrRn05=zM`KOeds->b?f@OULYNBRHw2?_bM$8%!&nmx*6P#=Wr^=)_8i~{bP1ONn%#E|LlQ_z;XMdcqk)N;SDLHS(`>HWkTZUV6q;zS@{eyIU6%|X_-FO8XBf@q~axe{BWD07Qm7=>v<acYQ+v;zXW~I>b4oT9N$s<4syOQ&Zvsp&rALLz{b8VpcQ|E)q1YxA&qz)<HpKwZM*G>&O0}XBQ8g>4w<N_p*8h`RC=3f$WE1dP6^sn`0y`nqFIEVE+4!7ca57?>_5_(~8wW-(%J5FR4UdF@5zyG|5o+VL6N+3yj<FU+gsQ9P^ccUUbqML2;k&|ta@ZxS={D`jMWxBwFWM@QO?*i2l0iTN*oZa&UD4b~#Q}lsI-w0A3AWTs^aihX76Sw_`-}G^=Nb4trIle#AMr`Rrk25>G!1Pel_t$D*JAsq$!Kfs$+LBq?Ug#^%a>>kui}`AE(};?FxcqLlZIn8e<;2t-BXf=FvW2K33|g|NLcame!z2wEk<q>faY+$lSqZJN$jLE7Qgp-kaYv%+VN@dPQisf6ASjpQ!E*8wZSplnlSv57LVE9^4wi?&Lqj-tA{|!9V&*dC=L=t^vsC!6*Z)|%3D4La0l?s=wV>3?jOT?YZ=mIeV7U)iJX3cMEj?T1DX2u+U=R2;cYHMOanm^S*k{24dfF#{3SgnBfo2)!3><=FjY6H&bD|Z_C`Y!<_!}nYm&j`1jaRs+NK(55;~YB4)KK$7DW0L&!1;(00SAM!?1DyyK`%wI!lVENN*!S2*%&ORnO3d^FZH^y6S%rrrWq_alW#}qRpXi-m72ZiE%SzQQk9{ifIvIKHVmDuc&~=_+XscZzRNzFmCkmX&B~Oo9;^}O3I}U+L}7fUA+$E(pC1qNMz_~#y6MS<$nX%tS;HXiUg3gEf^t+=qYmVX(Fl#t$WYd6<(_loBn$qQ)U{L!Z&oErebgwItbDCYu(J%n<v`|9A}q3pmCn$jzn5p7<^?!du;(08C&GzMg{jeZg-7!MRn*GQ=jf_bM-A_aM<TzzD)3Z(lO`>TR<3%hnwsY<@pSPrCA2qd{3&q&46$ps4f;~=MBZg5y~opQX}`a9eThX~Zb&t7eYBdf&T_9H7jD&n^BmZRK>ig=&$4Jn_USdp`L<4Gp9R&YNhJNW>OHumqO>{zuVTOj1#eLTS-@a6;Wg;6Z1BH7;pG2!^2mcQXA0Ju<$3sUHjq28*^(j)jG4ilpVl)4-X*EL7uI%l>zb93H9*#`k32Byv(V;#SdyQf(kQSz#Q_kX0uw1j;w;mBxHzV$P>m_Wvy;MJocw=f0gDeSFaT{eld|piCDFgLF!|7gzwYWC2qerFFCZW!%!Q~Zv+foyu(qmN6#Tar?xNIQB->^d>WD_Vhmh~g3wqBzhiLMagYA%4?D=0vq%vXPH@wz|jQf41EH;iN-WrcyckZ&r^&~fGh_h3;3xpKjK`6msuH-pJoumcj?1KLQ?+-KDG~4n<V#_$fl4^Pe1Y2aGpjaNX-K?48S?Xk!NuIS<Jj?U;cz<`aJ!-el`n}#+T(i=I-cNO!XT2r6$lWeWlq$KPz!WN-EMTZuTs{cCf0xDat7!h6TN~DV@<M(k)0t*75Qbx3w%8f-Eqg6m7qcRog!~&`Ygw-WYnaxe?CeUmCGbL1vc~dcgj8~&beX)c0COkge(%*^=n5fTzLm5THJg<Ql!>VTgU3IeVn6{%+}t?8+J4}TjmvT>F76*erUcqF!l<w-QKSH`Zn`?v@wVtyL=aXCjn_f>VvZC$9=+#P5nQJjb3Dto+vI7fx-4I4cemHtUttSw%%hGMM@(Gg^$h2T6yxLzU<!NO;IqM&T=-<w@028GKA_Y?fhkyN_$Lu>L<7pcL;d=y?fpPwOfsmRl_?!3pfx`BepHqk;=0ki4l4JGvxj3jf=v8p;)OV>dbW@2Yv(QHM+}|^uMYk`eEag?_0hY7lRt%TPmWHHULPJ|Rf!*tUml&_6|?wkIbk?NE!gLVCIjp|ljDhK`{at%>+iI>{S`0^0P^S}%ELuU=Dh^{E3erj`QVOg<FR*QZM@!u8Gu`cZ$5fQ)xy}y>)cHX9}c{qrmwEW=WS+@sD*eeD0T^TpiE-Fm)-FdJBd@&fUuOuVl6){S@s7rQH^Ttt2D?ju5k~whK?z<BCkyybaj_%q_qju4!`--;FWC*6yij3;1Ergxt*c%PJbmfW$|%~01Ieak9{oIlI@mkkJjN$Q(d3YLiSm<-_CAptGnV#bjv%hPAGo5ZrIBY6e3&>8zE`OPxNaA)We%iY4$;FTxRF=Ijs<HV2Y*T(BTVtk60iLR|<JiK)!JOLaJW*O|K&fo)GzS1&<JeO323#;TGbC&f8%4)o+?l(nVYyWG)k-u@$2dAoq$rYDic#OC7Gu#(G*Zm}VrU&iO{jEu1E)Oxwv`dG}HCd@WD^D*bNT!ta*3*2ev9^s4QJ{oeL|xZUsXI&go8^@7e1ib%|ZU-+?N8y|AR&4hbcu+zkWLz$cvJ~cgED@Nz_!m75z>l{xU757{qQp@>N89!KxQgU~oh%uhAfkN}pE|Oq+b2-4ey&SoOpBOEnphpQflVP|N&q(uHwH%!qDyV6}F<V_mg@LQABpH=O+*m<5o>*YM>j7gYc`#k%SV$dlf?W@?_hxn48d#)dv*eGO)};uhlCSPVOIn6Qw(3@F;NOE%SEkhpFLu=GoHn+xVm_NIug}XU12Gd=&Ts-6?WLV%OGs^$G;A>q)wivZhS3+NA*CE=+4r}*ot2a?4-%kv%c3B*9OgyVSpV>Jy+p8enleT^Fm~T=bO%=5Hq+f}JvP31`i<t<BczjUI2TwJYkOx)KNv@OkxT)NPsjvsY6{%f|5?x}(Fsu28w}zvpzc%gTwkadyX2v*6oCuDlK@!8JouLWv83j+#qbm0CFLe4`#{+#nXtKO#_!NzRtOG9l7V=w=fFp|JM7q#7vCx5o3dW?St0^9oPzgqKmeq<CI(MWe)V{6F{4uMWei_$CC;6kjqCzZc6O*{-ELO}p|TPu6{>>353Vr0;i}|Y`{<hbFIm<28^87_i<|gWF8;bqO%bLtt?Gt(@gb7BA`O!zI;(AJon4keu-olzyZKuxt&hwcCO{>1qQzEEm=h(q!k8b+5O<Rs2|1Qzae7`{cv}!O96QY*_yat8tvxY}co$@*$0pLMuD=GctgsE7mN}HQU(G10z9H{JyR^y+%p?!;M)K#l`C)B?!_~hIaX`EZJ_Yb#0udfrR6S6j4P-(?R@;($#9_`Xt=k<I8Hcr-c9$0n<uaWookHrDe>}Q>NzM6Qv)L@EH!5t2nA<cS7qz1wP!|*kg{mNZTtBu1hjcVSacSiX;?<_+2~?=BB-T?D#gxm#V9RvHr5jP%Wo_+|iJ_h=C@!)id^hF(u+?E|uUSr4mgxVTA>+!Cx``+QrV^~%6*OEq-SN35gzt5EDCl0lwbd&{_@y4kOozFQB{|E1bit~^;Ur3?VQ~=^;h*Dd77`f>u~rFuLSccXS*!Gx+z@aE1I9VyQKQ=mww+q=mN4WgPS5#&*H^h-aQy8G2bfweU1f9b{|(N0`(AV2uA1}y>T@oeWrt>IFG5JROK)>^U!IT^)WJN)l<sE|M}XhxgQl0n45izId$oJ#IJsK4yCn0(OUloyM*MDfcWZxdFYdJ4!=3)_UL4iC+ts?MzS`9qGKPw5bVG7eiiV7{5ys6bhW&I+e__2u;yfxT@%ue5s(i?5D!1$hTmZ5k^Qx}mDFp}RT+g7e9kZQYdz%eoKM~`+NamB-G#Xy<NM`DhKLjY9#KJX^vzR|pR#N^$y**|&8YXhd7$)8we|bvmLA@-AauuP%LXuV@?Ex=SO(HGt+y7Fm&i|GJ$uBC~{raw;tMm~S>--+|M|o>ay^33ERs}}%=DoXCk57kj8fD3>6}*dw7b$#tekJymb?ZDAT{tiFs>KhAio)H9fl#Z8Y`{%>q+{w5<mKJf!{F7CS7>rx48AAwvc5|&_#Vh>^=o4AJ&>36>&4)EA}{ON#o*PEmxZctszS>rKfXLVJ$e_O9{%Cz)j|0F=mhI7lX?prY?l|UuhVvqSEo1IzBoQPI(+x$<WJ$z>-U4;F=%SgcK!DUFAomi9Uq2TyLF?37e_xF{P^-+=yaqyK>?x21ry4jjN);$m=x<GrD30f@%p=to6!mTLNIVyssxl@_W)^TS5#;sj!=VA+G>3u^o2nw(cEA+8+g12CM^&-Gcslp;`lJ|V2HE>?UPEo-_?w#)^aTupm4}m+7>QBx*F;!Fv)v+Ig3W3#;Lt7w-^cY>J&ao4t1kjhiDXLppT7`EM`$J0T0`0!{hamKH*EPH|~<l#%UO%wt~lMpW%nQL1o@7?t<BfhRe!?yiWN>iFMlBRJ~YdpXIv!qljj#5SBi2n>!m>Ci=%`obTkq{=sq@clWd_d=IiHxM8G248k!xM~f_G6=md4<C@1gGI#r0>f6h|R+Xz6-qaXxsS^a+qNY)rj6rk2t3j?vv*ff}-D=30(6cW>8~~Lz5HPT#IBA*IR}Ox7eIJ;c8#90H?%)+e#WJ{4f-@b<N}z~0M*7g)8uTf%)N1EcR~}jCthuGH867(7()zX!nvqxqyLUd;;n3UKXI@$Ric^s3+&=lDTgfEyh9iRsE1tt08Wl2$^O2yWRgOMQLso3q@}cO{hD}47%;6w3HO|r7?TISs3aQ7_0`#*a$KWQiD3-xQzuWP!ga&0RZI&cr{G264{HTG6buhmw2(s(KqhDg379>lgUQd8(UjuHqGvZKE)2JAevle%pmT#>NfTtL_dmi6mu>ws4#p-|My?IuNo3`8U(EZ@<c8}-8s<h}?b`_z&OUBS>YJMC!8JStgU$D2Ni$X}8NnFv(4)n5F(TgIM<$Y|G7>QQtqrYS|E^UUd`P~nG-)${fh$mdf>|n_vmVMu=y#Vd|5@2jE11LK>E6rbi!%Hoy(Ym^I*)dV??ht3)-Rrc<FJ!o&AM(NMD;e+NttNiaV4P%mp|8vtNiD3p2jAW}UXWJTva1lb={(NnS;E3?O|d<_>_$8N291P?yw9!*M0IFa%aEuA7FF63jdsGEINBL?_TqNCf40B1ceYi}oM`q`ZBaDaQXzm=59k~HmzxyboOxWYU2ZcJW)fDF?u25G$;xqvVg`aAe*K3<yohy!iYhH)8u^A-yp?T7F>NMh-evy4>GSsoua3ggcbGSbYu097JN%o^oBg1~9onC4G%d5@rkfN=Rd7zvlT>t}XV{g$v<Y&#M93}9)Frn+MpHSk{9%%7Ips7lK=1Sf9!1&kLVtEjo={qu>P?>r<C;js=OUY<h|z2X?-GE=oT||a`};BCvOa%L%1N0OmB(ifHT-a2B9gS6M5?@Z@i^n_g@!CKl_+i$5c?I$umr>5F_0~cwH?M2(JSv%P!GYmg%;le$@Le+U-f1&KhL63tlseHil1^`JTH^VZfMogl)qVYntbXqOGeaqhkQ^5DwctRK{1QRF!qHOT&-WVv_Kk~Wf7LQi008SDXw^bGCoD$laNMfX)ke+tRAHHl*QcT+WGL8kkmf0x5fY7@K5pfFTfn?)gk<uF-wV-n;_FBWI`pVN6l6uWUT9@)a<T=jCEg1%~l~~>|2AXvEGgN*VQOpDUmBNC$CEB%B;Q*Md^G*j*ajTl+Gn`yzl!^x^g1NI=?%m^AS0oB}9^0R$KUf1hAC$vEElEfR(gQ=4Y-;0Ly5f)zO^^U^(sM;+pN%b%+TVkf_aJsc#YQqsh@Pt>EaFCU;=jX1R3XpKjMM)u_96BA$k6LU>KSSg<N8v608UA(NFib*D!0`tMj;8ve{^ykT^zlV`V?{&rcH+^MpDhy10e8(7`=IQ?Nk`iiq|xfJWduEBnL8d^7g4ViJ0H$3`+4U>X<f>!iPF#vtq-L3v!YfIT9)y|fs{%?M0)hNMX*o4Zn^JocqL^g?=lA<N7Jdu}tl;lQW_GfO8EG-emETW#O<c3{3&lho4^0F+cP-POD$_f!BrNft0Qxa}5GKnMAC!-7pf~GZkRTYYov5H+1h2t@{nHRTTQgVamOC<MtE7c*(A<nu{CFv_>Qc=IDVi-H4qJrGhW&M>DIrBfpm{4Wfyo`sBeyW<u2~WCJQ6+_|(caIN!Chpw7}!O@y{pwO&%G-ydDr~lt5#~q+y!N)o+k$u`LtAEjX3Us|G1kHO~XFX_Td%hRQ2C1BV%yHc={E7tjPMY-@#Hr8~fe;*48$!c_8_!uiU-0X3N_(Z0a?XJwYFI)Mx{n>|(Kqc}`k?u~|1(4||cBSVEDV1*;qSGu>2H0j)@B!!fjGW}8)tepV-mK`&sVB4=5VBJ0AMAXX2iW-vLc<pq*gO8KeC3)utU@&o`r;Ydcr^O+Mt^rF31*F{X%TSZ5m`m(abfgN4DsReLuIxD7foJO@}dSC?(EnTO+KspUtedW!%tKBSLE#Dry(M?HY73Oj<&<A&x`Fckaw!2R8wnv`oZ9;rhvuvtC?gJw=$iE(-Hhc_UQlT-&O*<HS37S;Z;$9t9yiBMJ3LDjIQ9<G^#mTGC$L;Vw_+)f;Ux0e?>fa8kWodh4>3OXdBb%*B^ndl{h4)k?l)M~yINsInZ||%6MU}j~*=$vfQeJJZP8mifn*}8#*Z~9LS&l(!5mplsudnUGm4I5}UDL|-<aB8#h^ZQYvp>x9a!pSvN7hhxRko<3tSG;!A#CuVgmB{lm}nUy6Ad-$g(``T{no?}tE#9F>IiLFf|PD&Yp1o-Uxv0fmQ#>W0nKU+s0;of$}dhar>&_EiV5m@j#&;RafZ<ig%<}*QuGFOKd><Gku`cdN%2;;;p9NB%DLl57r*_-M=GtM>=-UHh-<^yN&cnULt$$0EW#InymTcMCaU``$FAuh2mw!4VHZ~Ci7%we<m7Cw&<c1Z;igk}{r*6)&1?@0m3%Rsq7{{^Ns{d}9`kN_s#0D7vMrTGYfX*H?DVR5Y_qj2ak?0x%*><uYRmeq&qNN<()>Go`W}T0zt;K`Ey8?}T_%^atUB1z?4G9$Ew?K?dse4dthG#@c7KUUqkc~Q+U3%%mYx|e&sj_P>3k8Xb&|bG)@q|P;s*7M5Pd9~k&;@m)7opkQ&zuJ5Z%t=dShw|Ce@&dK98RHijkOIUtc>E=n=1f&QF>$);M4CNpsWiqi2EriSxxH>tO4m$-f_BWB`_bnsGu9Mn&*kNDd%xeQHz9+O(jmn~mnZ79#H>9%cnMN=D${X+naOK7L(>N6%{Dk!A<tSivP&n>rz~+Y*SXuIE*_R)whV&PeOn52!WGuE_u-G7;&5u60vA7@*={TM?Z`r%nzt;vw~1)Mzf%-f6rEcTC$E_4ju>TbRRmzdPy=YdWThPL}wjiDs0Lr`6pLHt4_ia7R;-f0m;L_t7{OrRMl={%(o8MtKD4izFY;Fzvt(8D>yJgS7J7(0nTxm(m}g+h$(kOD2B{9|q&*)by{gT`PDS-GNMt5L_0{daT`(i_shx4AHr~c0Zxq_oLW$$*ALvQR0Xp9Mrfs#^Ho&^jg)2odC#EUyPE-P`<+ZVGF;nt`YLNBDS!OZIrnmw&-3p7Ro#bwv>47v>pgs>iO>616vsD(lZmxg+~%5msTY;Tcrt<mO9`GGWO)jlYnQ`4rXI2_yHO{df3AZHmC6vi<lCTWM!#<8n)@HwN3rgUPqMN$}s?L;tB1*oQ0#{45i6<WGpWbmH<6?)%7$J!Kl3sFg#=Mt{R-lqiB(sCdHI)rg7L2%!O=YBK$*)xKV4w?&>Zrn)?dv`Fq&Evt*JKSKRcnLwU$|x=sYE^ox@Tj^Le;Em#UC{W!@aPtYojzo?>_XlPPds<cjoA;sv6ot@_`J={yTy0FGN&C&<<dwm%Rr_7V?er#kED#=vk|H?Qjo{_y&;<<?S+vQnvw7@852P}Ia>zyV3u&ubx7Ia$PK8%UVt~?=t{)V1lLO=AYzA!*u+M*P^vTKSk+v|!5v+n+KNXn9B930o|qJ0>pph~6DdE5|jiCW>pmU#`-Y<hweWq6{fGsCiM8m3(39fbjgZ|2zdcaII+)`fH}h|yo1t*SKlY9-lZ9#OuO2}-R+Xbes~K)5Kd#d$J^{*{;Jtv6?6xGmGOF*l>kD=Ru^xLRq;1zEG{2_ltg1a17U3mh+ba-vL2&e)nai=3eejb_9A36?@eDI>wk&!7>vr@#wKUvbT6w7tLA>Fl=KTW9;7owFTN^C|t9X*{K`8QI(4ZEYdl!+**5gmY8im-FoQFL^M;0$cI$(|iU5AQ-F#o54qyMG`;$7(Audrw8wi7hUnfpIrizP<(77#{cFw!K?RgH#@!U&F_PM`KSLvR;ynYD93Py{k|Ju-sfn}add6tqeqAT7-Y9!7X`3c(9S@F!A(oXWWrx$*uf+Ml^DPQO77^J+pj>Gg3%(F&hY)(#wZRz!udHa+QGpB+NLa1GCIZS*`LNh3xh=(d^Buphf?JDxVC|^63B%qjX0jn3ZQ@ytqId1QilNLuPo+ubo&)ctbt^K)5ohlXaEz+@#iJUeXwGf{Q{<ao<%hK+pi#sKx+_fg2^}BC#glhB555VZyg#87IXU-&~$^B(b@)%EGn0#B-o*a1G<`}!ysA=ah%HtryzyFAwUgAquu(*Pbnu$9|Is!G`BH)Po2&I)dIXEodS$;P52}Ds8T+BYzIfMP6pK&ArFg1(J*I^3YhNK9Hhkz|AjNWNOD%;6^_&;KmgsM(eo<K3d9m-MUF_F-56m2!Z``Sw_k^gIe?07oGevG7v&=^(%$~h3Bu(YtOl6y`$j+m7=>nth_N6j%@Yo7LL8(|WIm;I((^3Ar%4vK0$vw}+=6iD#q<|vlLXdEiO697G>HlAIf5Sm1KNN<aopgaZ~vRXl7a;0Yy_(>W{UzdY{A?|*qfR)ZO%)aW0rzq2lGqwI5|(*NIab%SpY#GC7g~IxI)sb*ve=W;I@FRmy~CJ@=cU6M1wQI2m=B`$DnvTMvdtb*16E;AB&^QWpnBdrZfsLNdQAH08)A$okQ=rz(NY{rK1{K3&6|Z3G{oJ#Ml*Of<utOK$v_mh_3Lw(FWgs4KRQ+VgP1mTB1+1;HnI47R(qXC8qohkJV;4kVh#yfwx~L08M~II)tHNCulK<q4MaC3HEORlEFEpJWVnLMl`}xNdqBm!E-{rVlksEnPRW7*eUKLfCNw}Z_yKg^Enk+A><Nt0wk&#;R5c!JSyP85Q>Tfl7Oo06+i5>2Ny_3DFx;2U-06FWhG${y_^AYNs$7=I_8+TICvRz^k8%YAeqYJ11lRL=H)!0EX;2XQ-}93Lju4u9s*5Dxd<_f4G$OTi5I0@WDmHA%W|_>G{QVD(uPOwnz_b#%Uf^;+~3}X|Mh?Y@9l1PyB!Y$uINMK!xgQV;c*Y`j+opT)Xd)m?_tYE|ME}&590q6a2vJH&EVT_``gUtH2?NnL}RL*0+WF^-Q9pW3gKD$_FML_w-*2j86*C|qYZl0G25|++j_fzrf`Zxlp`)60$+g0zJyxqMxW`fYr%E!FaPg<h5!D<nE2M_;fKa=;AC(1_nVBJ|BOT{V{C86<Jka7kfFxxu;<KB0Qx#IX22T6Yz{8OVCenm_3xp_4~>hWnCF8hPlnOxYBIxMZE4qiLg0DAS~lUMw_W(pUT{s5{ICC)nq;3(ajR!d@f#-M@nghjx|kwAmyEb!hTc3PogOFwpWjxm?)-4ziurAe`E6PA!wTm-<87|<8cvE3kZR(&oYz+Ed2PvgZNt1at<41qjn8bm=gusWa@!;}MK=N;7vIFBwrc}xTLBA)yk6U|!S(NBTJ}-c5xdxDlR5^a`1UIjUz!<;GI<_Hi%p*JS;(kde$()bPK?;he(lZdJ4mKaZ&yrjn@^7=^#kddagublxTJ$9rUx)3aZpTTyY@u3D<`riCbHuYjB^xRVC^UsCIU_huXk$CV8@sNnc((&lsTUN3y)shem$Ga09CR;lmP|7hRIJ~=98I-$HZJHIU`J>%(&R2=rSG>@6XP%7#I2q&mxG{NOIxldnQ;T41kT$|6{9ToT$XWZfy+gHrXL1w}1jVxcw(a$=M;1kKYNZ&!GP{OJ*4-TnH0<##8MwM0<^r9X^ns90HriAGH0w-6x&hCq2M$HvB8pxPXcLj3SWRV#C{FUu!{J=QqKdGvW?o&S<(=ij0!`^FPo-jgh?3H@d9Tl5iK&9PL-av=`JqP~Js#AQKxkrmN|;82J#v@t2Hxj)uc{UNAq#0C_1&nbX^Up2BvgOrhcG7<<T-GA3c>3|k^0WlcmYKC?D<_)-(KLH+`uAX1=Y9pC;Bm^Mr~CxMiu5+b2W>4`a<%ruyt!8t|IJV!SE)0YnqJWbx|15Xv>H7>bi&J{ue2r@3x=5WsERL2_U8ZYqvi^jP9=K|206rNues2+~u#TP(_ByWp7lA>?=4GP<mNuzqeL{iQQ0+NaogR29Ez`Tc(C?f@ful|1X(l+J+=Y~cpj?1K!0}B82FaPmB2PbiXrwc^GoQrQjSH+*3qj+-?ZE{J@XQUDdel!VLKH(Da{`f44_O?JGj)%KD@z5s`t8a7(SUn{Pk_1R*bN@XjYpASb=ctgT0dI~=fpt$o2?dQF0E?RRTJW(XJ?N8<XyBl7Mg-T<H*tEIWI%ff$JbEzaFdgM#0{P@TS<<}9*L)E%v87qZte&(+{YP0dh=y0XUFyt)iTrtVQ1sTI2b3x3uv1MC!m}RNFy(6KT5vX%}LV#D6<lxtt1!(qFgJU8|dd!=vmCXhPHs&^hz|&i`noK16OcHX<sJEG%1+eN`*j(X+<Muec}Xx$>h99E=z&9!^+L9xL{du1hsn&`VkxoB*ewloQ*2Z16zbAD<of!=xy{36MPv9)(TjmrG=ORjt!&=HCQC%vqdn=&VUEtl9>*vnV_I&W^!sVgyD%x9Fq}c)MLyZAxH*tA-8=2Q4QpOCTg>FcvF4iVEht;6VR5BsmSM(*<fg2&TjuZa9z5&WEI{PU+91%AsQ_*vXwB?mGl?jcD`|RmI(tEoFQU?88L9&31KpVucMUtyX@el*T!y#Z8J~6uwd*GDR@M_-RLF2EWotB36@l2SrR@nru&hlOb~T&gwdBZ9Kw;F@rHYy4i&FqG&10B;ebih2tM1S9U@0{%q=GRXOQ}H#8cWbXY!A2zM(ana|9S?$vOJApDiYzqwLeimfTCVmF!H!(*)%KI6UWREVlqiFqRa+Ci24!A)6w45ZFJyCE&DzkB78){DWqRf4!nXva644xN9FzSy^kOTiAI7&^QOd4F3N3A|`##_{jK%jm8XxycE*DsD#w&(VimD$jC4n<dJn`;K6y60SlU?Y;=vtFj9eMO5)a#NkgMqa>*nII1}hUL8|*K8RhO-;Y}HT1oTLRtifuGjPcHq>tJZ<)jlB`R0gM)%gs5$f8%4#g4Z8$-58LBauE6~lcA5(5n(Rlrc>nDD3&fUoCTG3GtdXJ9Gb6fpt&(2UX`LWGy!IR6_EXX2;*?ux-#kAe*I;Upe-~-+x%gS_XOO42%U|AD!_4}xiK53W>w8*!;6@i6Q{Eg71|~#AwfW&G1Kg5A+336%H_8NXsXkRI!i2!CVOt$X4ep{;O7Vghg@7%Fb@>VPRtCjIoYBEW*EM_{mX>OFcW5B2h<6k(jtUurGORYC@#i^F__<eAhlXUzvB#6SRmg5_75Y#sc7r+7!%;-@=>JJ=g(jRgD;pyyN0}++@Un%yWdt$$C*$L6$75W7VuB!9-IfK%si&q!$+Elp&cAg0xS!UV*!r_m5>!nAxrk!hG0MwW%D8mEpISTd7;B4EzNX!BpDVym&Jteu);n^HWL%{9M?!6WQ&CGjp4Y50J1wYl%eRhXbfEuzT&Ny%#eEw2aU)8mr8I5WLZLvZK*-=?IIJ#GO`uF5=e`Pq0tTG+KZIm9{mN6E{s7JZkj!U17F<X#JF1<!e}r<tcO#`1UOxE8hIDX?u1|u;weylL~T*t4A2Wlql&ZXh*U~UdBTt+Nl_AldDD#r{(m09^vFt{vlDas6$g*mi9@@tx?LawESod#!Uf<dqkv;{F->PhH08)@QD}Zy!Q`CLLTDdEkv+n_tPXaD6V_yW=LHw_<FnBy?(MYOqh37Pje8zZ-~CjR_}!OGqu(<eZeAjTJp(XviI@y9Ly}&Jcj2Mtf)nlmN+-P9R2z*FrZ{pPoFXL(UM_~ojBE^yHRGN~<Y^Yo@xty}`;-eP*TG?QM)$zMX)>AM1g?WI;1M#1A<&`%N6rdq4lyN<>)-&XRqz%`0+SoA@jjIA_DGq~$Qa^?A<+5<z!-c6THKVF%C(>)t~kOBCO-fLUC=$*kmX#%$qj_*WNWvzk9o}ZSh;|*#+%wn7^o}ULKxn2U{HAR3ETrI7%hclASUL4<cjhk3e$Lz8q6fwNr40>QxHE#K&m*wO~lTj8l{9OLCJ+y%_N2dgKO~0js&X(F(P6bYnvZ!V3ClO0bj-qe4m^rfDLdgKAuFExV8z|+>UbK1+dPKbkoYL9c)Q!n|w`pwJvNLY!=d^0526$yC&_P7K`Wl9?S5}7?@1WDF?M@jMs?Tl-d0bg)RYZQEq44Xn6kr0b*m(!T"""


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
        prefix="galactic-mvp025b-", dir=root.parent
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
                    "Le patch MVP-025-B ne s'applique pas proprement dans le worktree."
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
    parent = root / ".mvp025b-backup"
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
            "Prépare MVP-025-B : attaque déterministe, combat V1, rapports "
            "persistants et cible hostile proche."
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
            print("MVP-025-B est déjà appliqué ; aucune modification nécessaire.")
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
            prefix="galactic-mvp025b-verify-", dir=root.parent
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

        print("MVP-025-B appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=21, SAVE_VERSION=22, "
            "RULESET_SCHEMA_VERSION=8"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
