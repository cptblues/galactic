#!/usr/bin/env python3
"""Apply Galactic MVP-025 from the exact planet-discovery baseline.

The migration adds deterministic planetary occupants, configurable ground and
orbital forces, bounded player intelligence, persistence, colonization
blockers, and the corresponding inspector presentation. Dry-runs remain cheap
unless --checks is requested.
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


MIGRATION = "MVP-025"
BASELINE_SHA = "411339ecbc91a7d724b16f9635dc5db08aea65c1"
PATCH_SHA256 = "fec54a83db7c034a6f6eac40e2b1aa42cd33271abdd7b9106562e23b17cd688b"

MODIFIED_BLOBS = {
    "README.md": "c4670cac3504ceb4e49597b603a983e7e8197d23",
    "assets/rulesets/default/manifest.ron": "7d3f8ee645f9efed28d505b6a30d0a5b2cee877e",
    "crates/galactic_client/src/lib.rs": "e7e8cad1190d1ef6453324e2a61b44538c52c3f1",
    "crates/galactic_persistence/src/lib.rs": "b36c7d36e9de70f4897b633ee2e0af62062dd495",
    "crates/galactic_sim/src/analysis.rs": "ad5f03ccbb8e38796bf7d82e1af4dff3138066b5",
    "crates/galactic_sim/src/lib.rs": "1c0af2ee13b4b38b46234e18b7b7487badb049e8",
    "crates/galactic_sim/src/mission.rs": "4947af1a57ba17092232e4f022cba89e9154c293",
    "crates/galactic_sim/src/ruleset.rs": "660ce69f47cbae0b9bd51f9302f64b122659c89c",
    "crates/galactic_sim/src/simulation.rs": "b694fdcc2d891eea8e71d31ddee04efb999c82f1",
    "crates/galactic_sim/src/state.rs": "8ba838c250a152732d6e392c0d5eef329cd04c71",
    "docs/mvp_architecture.md": "9056f3b67262dfbb7c67f928d1348f4f4f660ea5",
    "docs/roadmap_galactic_issues.md": "b07dd5510269d26df81fd8566757271a89c4bd00",
    "docs/ruleset.md": "db5b8902103da20d6bfba155b7d7c5d0f3f253d7",
}

DEPENDENCY_BLOBS = {
    "tools/apply_mvp_016_b.py": "1557ff3f419abbf6a1b58b897100aa72da80bd38",
}

CREATED_PATHS = (
    "assets/rulesets/default/planetary_presence.ron",
    "crates/galactic_sim/src/presence.rs",
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
PATCH_B85 = """c-rl~O>-Mbk|27=uSl_NC}0Re00@3TC3UkXRyA!TQA4UUGpp97lE?s=C48_l0ZMF%W_{T;+vBsh$32Yg<&~a2d#88XBmYVNlIQO6kr9y*i3CaY)Ql~xN(3_F!^6YF!`;K(BTk0HV14~8nFZ0K<Ac3tFAiGcI5^F}*OE!hJ_Uo`)^>XkwOYd+){cj9&}p}~dcE5E`nq1Twzjrr)c(UCg7wbEW^*H0!#|y!0G>R5m`t(@Hkh$_8r*!nj7D=7MDr|5f>9I<rlaX3$)eL_l+13v23a)8;Q3_0(jaEyH4FMdG712=<d=CZ7_lrk9Zd((8GN}&Z@vx^sFU1$&7j?2c6Gt9>UaucX$Cj{8jK#EL;Ew?M-xhY3exF(2Gs}C$z;y5W{nC)!Hf;gC(!9xBI*K&QF<07*>r-f4x!a(mcS1-6YodKIGHgG+N>2kXIU-C*gP3!O#pE^gfGDbjO*s>EJ{-75DMS?uUUr801UG<N+t-;Wx^&CHkk$SJV@s-Qh3ik%~(2Nt=d{`?RUQmUc7m=-tKM&fBnmU4~_<d`9(CDL3_h#I$+dZeDih4CK)q(#M=Ak(Jz2~di*QEd@;?kn_mHPsFF?7F$^=9PZ%LJV=xH;d|{&z6#yW{3Cz`ureiY2uHkL7wl-n&Sqk_%pJuZJ3Qy;JOgI1E2nqxjQ3`Vw{5+il3ABP&(~J3taM*;-a7HpfdkP>zBRGKCT0VkliZh2Z4iiAA%2;{{t!6<?r1B8QHw(sdm|y4wh7VJcMe|E`7Ns%6si>_DB4}fJ219?3V`sA{y$Ua&HymRtolf2p(qMEmqAmee6MoE7=o)%}aRc#j1dkA#a#rJqgb7X*a~OY;;fQF^010C;P24_048%sD3#?`V;F`wub40iwVFxkH7Oa*rQUSH1EudEL6R#38L_0QgdGjlb`<MB{`Gl|@4Q9dFXnGor5Y%y$;fzBYF|<cZY%m><84VMc3JyRF7vOyUFq%Z8s|;FZV?Y&90RAVMMQEUxG`hH$ra)&AHWyv8K`?~ZG@%!2a%m#ja0q}WL!7>OhBP{6d>PHt!8x1F#9|46EfA{NCuj*V1k`{CAVL%q4$Bs%QmipfyI*5`!?BVREs#VSP0nt9O<6DjNTA$cp5nYPWE_YP_UU4j3=)p!+M33R2=>Q-v1m4<)j|`FSUUwelW-C7iIuqieXADhyfw;T*=CQ#`pq82Y#7Z)vq$4-k_@2%q&Q%lRf-9Yyd7@<FJ`UQ#_s9%?#{sE$dwx6%(_xh;^WOt;NtMlAO27a>a_qrN!$-sM2(eZZ5^LwFpc9Vykuzxi0cPi;yKa`AS!>c4UEE7K|U{3*a4>8sD@bgqAnS(yFzWkJ_bYNVB;weslYLxfj0lNT5NaYbSH`l_0Znsqse?UT7X?ocZlb9vyFJ}w0nr@wK_+JLPAGiCtp_~%u+IrW-J6syyzp8ZHZc7pp&!Nxm~2YDNCGB$E+WSPRw?pSTAo1KYT~&2XFI7_^A^a3}FDXgr(sachw|XX#x+5ki}d9OgoLHt5P;kfm5*XbUyjOQlm;&uklWNm(P$vnSj*I<~<;pg(eXWe&q$c9dIs06_8EXMdK;(V?fD|ASO&=>*aJvn-kHxS&NAmJNAot%B7F1oZjWfnyXFiwT=?kfJe!Mg|N+~D4lpl`(u<&r0_CI&L+1UYNzKLYiFZmu$^xCXnRYH)+Qe3aKERQI9O5qcEk0Kw%aWkZFi@9wB02}D+HeqxWz2J@{INb_w)$~Xi`w}jrWMZ_s;K*cYe!8+}XPS5yLhM;-GKH-_zUV&I9iH2VAy#yS?QHEV&t%7};V*Hs`{1!eV=HA6asCJ}WYjsn7#LA&7<8&dce9S>;fGAJ503&i31OndsBPGHPphx5Na)4i6%B7IJND<{R5am9d?`Mpo}MDh#i))2=eYoo?GZNb_;&9w;EdEkJgIL}LS5vL&h(I$8^b2A0A5B?I*#4ME`rnc0i>Ze_H0tDwEtt%UZ@R(pB8Tl=1dvaW`+jjh{YU2kE>-+;}Y-P5*t57P$NK)pEg&0FVw=dIIgSDHAS-}{@m4IS;<-P=sumexXNYw^sfZAODaYht&`)ZsHrMH!^98Dx*nq7kBh5DrEOkjSGf9XuK(r!7!%PZyUFItSZkJA){0wc_5!>0mc9wZ6rzaHV8%In+bCsDlJ+_@|3zyLrX}RYmNn{m)zv{HJL;ieF8$gmnH|aQ(Pumd}zg7A8e-pY-ZK6WM$}nhs#vn!%5-nePdc0e(NY1dz0^{{RSR28WXjW{OkX{uF+Yl}sp@|1g<;9I^O}J!h9})Xaasj?yy*je&L^{_*I=L3r{nC$A4)gon@IS(2e%#hVvDUY$o7Q@@T`1{#~_Lp+l$z9Kblk1Qlfc95piR6abITqdv^jA<ArY{Y1Avf>HQlrVYmhEJ|QTo}JevV;p3&ET~(JYM}YK`g<OWXw)37z0RAugtQLtC8VAU{qA)r07KRv*cm~QwLLW%%sWfKnN_y(d3L(oC)5AXFBda6TJDd>A2-g@P=PW?fVDhPtS;!oB7kj$&8I&rEHL7ga!3N7*_|pL~&Yq>)PCYgJ~5Ogix3i#A2(FA#@h}^z!h{!STsK2>-r5+~0d19{ls(k3Sw9@4Y@edf5+#8{ObZu+i#0rZv9V?ZNWy1Z(hRquJRc^Zb0mvY=0PGQa_dz-Upc1+USJ-VZ+ae}7#I{;1o}xyXyQG<Xd!Uro_&lduu|&aH>P%#)0v;S-9i)x6z&zJ`6xX4FAOcnn9;Dd4^Mnj@_BfoP}jRx|crDN9IQ)X`tM_K4h4gZB6lXv}D>31{rnte$BzE#@qV8!qrAp@b8_ja5Le0N?q%Y_*$PI4xUS&E4D13RzBRPX4YIoPdBmpYa(gOhPmtpp{>kqk}lfE`ah*N37a>2#|L;$lKe^Z5`y2#_bPL{8jNeep=(`qW*<Wi@pT(r-d3|E6PCAO6!e=r`mk-F^w+5X<A>Ahew3Oo3F#0uPIC>0JZYlnfZ;)9-(n#tJkFCvi&GM1$}cO<}Rr5f0vj+69!{GjfKPehkkM2-`v>VYIZi-OU`7<@FbuG@+lhhStwN8)*!vgNcmn^ayVyIim?ex&#uJosc1)WOFm7joS`D?*JpUSnQhU`u2t-(8RRXr@SKPNX^QaxiPF>!Clx{H2dgJ8O6!X~=`d~t>rY{Ar^)0j_{@PEu^AmuG2rw?SRUtvNC)8fZD3g}(DDGri`8>l57-FU`sW-kDVxpHNw9K!@bcu~@W+=2FAiS54qiRqdwKKUUhf?qAN*-@EY6AW+*Xp#N^3U71q=gFz`>5TmV-Gx#RDG}1BiHB6wMygSB~fzKbpZK-emBBIF%$9StpEUE#eh038HamU~n$WUC3QtABjg0==b-<vDcHQdT|M-wB!SvMjultHQ1GmzD(Jo-~UOOd>q}$&x%-)`AOF_J&Pv%V2e&@2Qy@EI*yAiT`vLFkpSwa$p;YXCS|Z?$=L+<oRldIREMb<<Oh(+grB;9132rs(%UEjRuNVXG-!iqaymy$@?(9`5~s7g*;X2T#NB5RxYv&hx-oTGtmt1tbi}`Z^z!xI{_8(YK1(XU{?p`@q5#+>BqhT0AGi~McASp8$e$+990%(B4W67zsST<~$t}15{wwBen6Oko7Uw6TbXqHqcUvot9TgRX89O6=5YRm0@`GSOtur#fhw{v-v2+8<1mkHzo1^OrtNmmSxV?h)_r+i4;8Q6-;KewUz!s8Wk?%ZtvZ!?j$*w#*dI>afa1uN}0wMq4WW}Pj0(Vw54^Cbmz94O2rJO#$HA^ewT?Qj)ejE0r^{dzu3WoW*|F>gDh<g-n<*w{5MdGb!P;a=TG|AAS=oiVvY&qIrU6Z9HH&nd4@z_6>R2Um7j7Js?Puc7vW0SfPORmuNDcwq{4L66kX!t;SBqH$vuJ(k_h@WfXy=ow|3B;<=P=PHu690LNeSy|p?lkj6thr)4t-O@uCC;=uibCfXE?hd`$*~k7*QkH_BG;y|02&TB$-Wy3XDR^IGEUSRt)int-P=%Rut@%>m1H3sU(BxR4JSMM>tFt-R7?0~lxxQ&%h%5zeaPl(jX?6eOkj_#dx%l2JFNc2x8@D091w}VXGlR@?9I{K%HP%Lh4#QHn7ckgSog~yPDBd~%OwwsfMf>OR_|C$Hl!75IS5#cS^9}I7PR79+^&^ZO*$)J1+Y;6eBG#oiW~vNbQhG)7Z0ZZZgm$dMr(>cUxbRgA=5+I*FK5|H5siA{r-1v*x<YH=~M68_<5R4>MMVmtTeun!2H>g#I8-zS-Q+xl5_5rN-xwecy$Q+E65E>s1}m5cr3(F@$fTe8cxb^%}8}&xTN;-k}&)syWnu*ZWq7y&7%T`=>fSnz9O!T!mG)(`8#pCR^-k_uAFbd*MI%X|H&16y$$t~AJR1nWAP~rzTvDsB-h>nR+NAU$ye`4hq0{-vr2u`r@PZ?J=yN<Qn_J7Mi1@n_GTO2cf#FWUSd22B^mvJ^+F-940Ed<%TaQ}zhP2xaSP!D<2w3SFmhhiX&})W>G!z<6#<$}M>_QG^1>g7DiFc?hqhSI>N}3M7R{)V<#I?%#6*HqNxRMRIxo=yUW|=9!!N4wN8u`uHH6#zqAGi%k<z6`817&rFsF);<%eJbbJ~-B@ubX!yCBD%R$0EtCt7Q!-+v9#8%xO=32@Hf7g6_$G&k?Io7=(K=4Q8xW@mUdM4ci(R}2U0U{L^`Kj?c-APwMag3XcXb$vK-8gO&_Z9iBwPp$Wj;(NJ4*<^p7hWUKlqIu=P3#cnp!oyfR&_MyhghPpu1E3)g9rK<+dYy|<l@MP(7B1XFy$tYeHHr}<?(LNsY5scc9-qIBU4H6wIJv|P1ezq`;MODB3xhF6W{jHnb&w{R`YsPR<iP}jf67m+Gh+xjEG_xIRuI6~O`%&9{F3h_TxMHos`=vTCNC#w?C-_ThOWTfM}tj1qYeGI$|)<k<RdX<)(mvE_zY}z#0t8HDQFazi(^pYSKsAp;9nk&hB%JPn!C$#;4DkDu2Zz5AI0uDE31cOrdd$|QchEKb3v0Yr`$8)&%mue2ZK3?jW93Q+yKNUvAad5ew%puwk6o$y<pxHlvSjT3hTMRzyd()1dnBAu!Ubn_Nh!wGhLJqbZ?ZhD83T!#NMQE2Xf8NH~%W*YD&y}&bFlqjZX~VQJU?V`A&ZAF~yeK(v%HNw%G94Q<k@!EoCt)QE+l&C9GCEf(ceOS*TnpMg<d9nQl-%-Q<HaIm=yI>b7@FzyAa7I{P9<OV~Wkv1t*Oz`D+EzCua#=Z+sH7;!e4@wpvG6Vg-tw8>Y});6)jt<EOjkdzSlL3$yDrxE@;OzR``PwHF#N?y^Y=wxOJhi?yG?jOB;`4cIf%0mBQ@8lnU!f)^NgLk`~XAIB9#EN<Q4Aj?y{nt1D?KVxlN~fo?p_8MR&qOP^;h(Cn`>HEWMGfq|+<X2nC!&crd(RJkIz9;w{&{czHHTvD7R?;*y?S+Y{5se_dhzP{!Ryk&n5^XO{?YTJmxm?r<iZ|O!`3F}t6N*!z2**)!<+HR$LON}ry1|B+wScIAHL54Kxrd|Mfd=Cbj(r~w<y>VMAG_7iDT1BL+;LW!jE5m_jU%`@H_s)f8>9WPS4Ptmu2C{WOhECgPhBv4?x|pVnJi&_5)LqMxHjF&$Q!x;BL96K)*j>AL~C&5{w#S`{-qV*6)umFH9@0b;6T3W-<inIbc04T6QhVsM8|a8YYvt{zX=CT0>PxoaD#BgC|AT)1!|QzFoueW3k(|w7MERP^}dV%HBFYOf1HVZ33H4u0r6r99MM(O72EcQXK#VPG9K4wz##}CyWKIuguOFI#yDIaUh8n=rmBLZYdj2XG{>HZEkA~M%0^4AQDudkRtq6Vc{0W3(GPs^|a8qn&7-`3n*E_rXsOp`*6Wso?ys|zCi+3F*H6fLMbOKw-OeJ4+<zXTJ{MfuB<Qt*n6cp;r1OYp7}B7I;?On$pwcKg+m>W9oKc#<0-n}2*XoL;*qLorgRTJOYmA)Lk<nh!Z})2vt%|+uS~b9x!|&4UZCD^%$x0r6$1Z;f>C@K(JsJOi;__+A#gQs-4fT-GP9n$r2;)<K?BZIr&2bA2|N$o8<qu`dt4QPT;yqG6*s{=k+$8}2@0D3>EQTCAD*VSOTv+93%He>*nm07vOvzgc%p@A8j5K2`l{|As5qPM1=`}0L)~m%$`zsuO{%Em?XzO+-2TQKPF2pD4+>RR4KC=cr12fWNm|ipRR4lcz68Z4c|#^1JnH~CMwEh@{=}N)WATFY`+k3zPRB48pcsI_9M|8L5lF6#9L7K{uR!?SdmcVJK+zh+;AZu@I4K9O_m2N1<bv$si5{EoS>~W>kFG-DxFEKOJh(zh$V#8hciL+g^GRoSk<rSXIXxRzB*G~I!4Y7)(c3BPqnY%W`ey4VWWPM9+dfpy#XejvYL?JeUW%IQRHIsh71osH?VznH_m2F)JhIhK+E8*Wf>7buS6BJn+`@sYf(x3eNk<R51{4SlC}U&naS)ECaX3efC!-`iVN4b%jN*$Xab}vGwTYJ+&GTt2IDxEK?{wR(cB2^>MULR>Wx_rJV5K2>!Ul#kT^^U`E+~TYPB*p&+wn%LwX+#_JG;H|;5<)L?g%|kVN`Lqo1Kkd4gT#Fg7iet9bxhNc`}MYrzB}0YjS(e6*rm&{N!X#eCZ$OY>to8XgEVd@`Pj#{#(2gXHf%Dop<m9=Wx%WS=5w2cvRgFBgSTX86J&t2YGt14?J834Du(>r-MlQsW+Y*c=IqYTsG>bcxgs~$uj(@5{QKDgFihMaRsk<SOFAHr!mLGF+)W&Mw`9kp$IWEwkH|OR}$@~eWxiA%_v!BpT~rmzj-0ixGwd`0JD5-dNi3GDu($6#4tiB<Dut#W`xBRF=8-R!TU|z!pm1e=67RIbWjnEx4@tXy{mx6B{1lYxGO;;kHuSm^ypFWh9~R^x@~D<#{^}hl1h{X+Wv~z%v!bei}|UrSf1>?p)kG^3f!Ah;9j>4EY@hfleeC?s|+&GLmZr<1s3|%x9+^hwFNzPD7x@>Z(**0v(}%n)A<>&1JrL|s-tw4M5BXW8o=bSA<jmEljxG+nw7^JT^`cJ1DdpQFL;tRTHa7avVANufqg0*^VLleFY~QlGk-1Lh;j4-)Y}Z+@Kh@;gbRspvzCi#Je3(bRAs}^rB~Ew-(AZgdf5#WSC$UFl#Tl~w;@|2#cFrA5yRb$c8+4)Zzs`3c0Qejv{&YWmoLJIpB&|LXQ<JI3E_^p)?+KetuRfzlJ!`s%T{IYtm%<Th1YpwYi?gUZ5Hj2C-rJ!BkoD95JKe}Y(B}n5&7Fu{92j0aqc&G+~vmhhB5c(nKn(Cn0i47^$aE#r(Dc2O*oIJ{?1^r@SeD(aavQP04Ua4L<U6?$ZIX813wMqtyCujHxbw^Euw^lRN!i`A}MHOkhfAo3qpfZb|a&n{nC$LcYXV1ULOgsw?k(uz1@vwkB(Nx(>OqZKhue3iDMCobC}6E97Wm8L>lTKumy$OGjf(D7vJnqM6-F{O5@H^R{cnX^2U4#=ue9dCNVb1cTQM#2!J{L1Yv0B03gSRBSw$?|2~ZROPD{nj3=!hmQhjJ1s2;8uW#8uNiuuf{CXme3lz4$<uTb}y9-P;N^u-D=MhdG9tA2WDjwY!0^i@-b_7E5DSDs`0CLRV>XJfB#$k~l3Wrbp@{DFwDw@t{AB-uG#<eA`V&e@BZZXr;XMzc#$C%3(onlHXQoIw1N@o!GV`o(p19n5@<YTQ4iX2&$<r*Ga|84p%rvN7ydKDKaUL%nRa|s9MXs^n`2xd&OF4HnZlyMtoPj-_I`H5_A2{)0#=0&L%bO6_-6^qQ)xJMz0$n>;XNN#2!^bxkZy?kq`SYz{^#unUu-2~-UGAK*NCn;ySO=ONz!af$oo5S|Tpw((`#NBqhTW%k7wc|F9xk{4O3G(pTCQqN@PF^L<REKdhAa4R`&S<Li3d(pymMm!+*`M<CR<Mv_hC2y-0W~=JGb3dgUlEWh<<;JNc0Nso`;;(4Xf`di?d+fP987isCAVe9X%H&r7m}26oF5?;vk0yEz;tDdS_1SXEK6F(rA1@ntyURWP=1)H(kFQg9Q)IYE6vQ&4Df$#z8mkj{=MD7>`80g?QXMc8#!$I!p~d^`@Br2(!Rnb^RdepPqGfvc~vC|5v1buL$%%CP`z9$l-BZ}xU0#?1EE_8*Feh@%iq~vqN1wp;F)(RPf_|ozGC&nBmNK&H&i7Ki&8iGX_C#!RhGqCI77V{Cm~R)rCz#vJ)J(Eegq7wJc9Ebyp90`ApEKj?5dLwORua)M2S|ck-ePm4M1DXP)<3<n=G1Ivc0S4cq7Fziw5U3^D3v|JOlh=8|xyS+k7}o2AB=-2Nf(JS8awQGtFjwjSXfqH5gY%+F*<xbJ*7^^{Xe}u>sKVa9zbxDOM9%PPRA5a<U^&PlV|t;#4oawrYX$Jg*hBCEMtrmB`?CczP9nU{{uQmy1r1i)n7cXr_|!43H#SV#Se>Sqb@-oisAx7+<DCEj&Nf%4%@Rr0o2Uc5EzB7@JDWCGbq!2A=CQkEOtM^r3F@%%k~iFf}cbsbDB%h-i}qthdn?H_XU+EXk%<SJ5v5(*<9>{5%?FH$$1$5YS`*O6y{to!2XQ!{??D87!&dy@fO554I;+qv*9>Rior?UAd!W!K+d$ySf_azLpW5*z>pA+N&^PKMU8{c+=K9B>(pL=NEE95P;2~%wNA+>Iat{;aZW#1IzpdR2<G}n^^4Vz8t9(Hzw0Myr8>gHObw3u5A_&@_J{`j)q;<YVEe$+uPlW@_Okxwp9ohFD>02u7lCR4(MSFkDVXa)bAml2W!8TbxnIFOv#V=ffhcI1g{m!Pwupba(eYgTzY&gggZK0(O^UYpJ43w|6bI>M+Bz%D7UK`j|H&wa7{jx@R~()kj>}HP;5TsfYZZDBq1OvCyTt0AdTK`Z#x=xTCMJB9CbFMYNVkxQ%)XQL7V`LEL_{=ryIU3#D>ll_>>v)V!#u}!zKI@v?t=MnL9ktJF;%c#zZfcH^Va8l9|X@oqnGu7tn%nqWx^1%EKB{S#oJb7+Dk&c?yPC=)$jxEz!4MEz<y~s*&L__ueT+^nwm0{d{1Kr?9KX!RZyq6VxEK9Zd1+D-?T!^XWy4Q_V&P*X`~`7d>S7%B>TLG)OoQj<<r=vU)A3^3kNHO67fSxC<+mB`(Q#&LtdmeIHvstT_5g<(KsD^?mU2F1`SBp|ccsi;E~>HJ3>RB?L2C5*f&>mkfx^BdRX?aNF}Gi(vM}1ZBaKYD6>dY;pg#y89e1q~jqy8Rt_3?lPIqvnsIFW#MMfmjBtXHmdC(`Sn$08;Dk_(v@19!`<z8(|x5@l|J-)v^FRx8r^2(^=y4Q3KMr1CIfATr-j=SeGW9O+8S8^XK}wTf(MX`!0z+?>y)t<(M2<$ZzpVqiELEi;dn-;2+LU{4`bDc!{#WOQ~TeV<uhlS6X7vv!;>@78#f_W*+@Jo$k%wn*4&fm#oj-MxzF?uN5}gI;j@Du4qqNp{Gl{FJPEdXkNx#t9Uq(=yo9Q+j*osge14$S+vrk{O4&CoXU{l%1-qU2d6Xh{@t;3N*?Hp~2f#4a$y6nWaa|7t<>x;bm0Bn#b)9@Hu*Yz7J5{fLU{`%3s+2q1u+dOglkhhd%`@3Jnni;`1_izOO+@19VXJL5BVnS8cg{FWJW|~71c9m!&%R@vZsirCI+6@D%yWa*S8)iafQBqh>tE`PFJ;a3`-c;1<xsWI_`SCM9et=1UcR4x>i01=5&>_)YMh3r(@!iG(bo+t{SVC(EH`n~OLBs5+7K~gJPV|u^FZtw#@DZ+M*%hRgM9ei!|*Agiym{F6_XZ0LF;fcMkvDB{9?rFD>fi2MW+BZia9{7w!>o}0aemJ;3UI?@4x+!TOZSeuD(j^2ILaNWz6h_<!aj9*OWcnm0n0fXWZ(R-vD;)S;%H(wc<E)0I7ErR$6>K<3wgHZn?mXR7pJ1mmRv#ixn~?MMhes>)YbCl;=Im<LWe?mggMLDM|Bo<ae>@=+Iu4<(|@tshim58(qCNb3fMZU^CxX7qIb{-U)Y(R-bs8KTM=j%O`)n)hjSuF@_+@==B<ZDu!S`RU;5D?lPTnD*4eG4NU@P#8!G|F+Xll$jpuQISMP9*Vk;)l3ZT^s1nA-PO6btS>>?gM?hRIGJrPjp&~!wHQy~&`CEB66*=N+XnVTsP+cp@*yQA3)g7)Hy?SqP%>ngY;+mI0hcC}4>6I~5WBN(6f7-MO1<Fb>_PYYTcvsNV!nKA@FKK~nS$a0bOogIOsAir<;8?Xz%$lmbF5>7%N8~8sX-UYy+Ljgxi9T3;>y($(m9)2(oRXmcswF4`ZSNg>#}H|^pnAPjhSE+fMRb6}0ATiKXMWLb4%aR?edHQ^7j$n6w~+nvbel3O;|Vmp5F+>DxCeMnM8iu#L)Q}PDT}{+F<*_?q>kG90tM-gu;)v1AA}2M26=IgTm;TxUQzqN0;rpIkiChnqNlF0Ghr$N$#l4jlU^SsSq|(xB~?6F)nRufeIkcdn|rAUV&V>Yi>)M<-iL<nA?DYF6m$>tp>tQqi!IDE!!=L#<%QGvY>s!*1M%e(T8GAM)8KVwG%t(eN+>pQ^LEw1%DW;iwfB@h3-r%%y|EzCcU6=fU=umIUP1>8P*#LW3UN=K1nqnU(<>>-xnAYs9KF56APUPE?Ks)e<5<WN?*x<17(J$Dr=57w7&K~?>#W#_v*ksSeWgYy(1o;_+}vPOY{B&4X-6tPI+f^JkNr^^P>~pYJ+NX8@yu64)0>>s>#1i6pFT2k)V{XG4Bxhp@1rQSw)9I791Chm!u=yOnadcSog8_ZCNTqZL+@{tONR;0w#db$L&+aCgH@9a8V@qsQk(*n5S86BgL>;08w&Q(yNY1EyNWV{30!rtp#b$#VncrK2hNTZ(yM1VvJ*HK0~qpDK+=Bi%m;)Ut5jMv;pGGm-|danDvyOhGwSy--&;6{E}{W?BsuZwYKFU^<ag=7U`8496s%CprGY{5b~q>V{6g_S4a=e#r)AiX4m^oA@5O2?eR#A<DwW&rM^Y1OXsLZ#V*Gxj1_8l8QKJjs@XWmHv16tN=WOr+9RcIG4x}QEEDHm3=0%-;K-aA0$73>5M;^JndsnZ;tI$2EUe!Syt!5vc!Vt_fgyPsvdGgu#=Z0Ygu4A^%_Db%EDsX{WI%giI0h8W}Rx>m_wEc(Xrl4Cit1YhnK&xLd4`ay<HcPIE{N6Fo>h!D*&m+rMv)YYb7Ar2Fs|VUHr)Bcdd%6eg$XEek`xL_|cb&*VD7z|V3H(-~&L?-H&hN^b|JLNEoMN5Q=W_IS?c#m~VO4T&LtNF?UKMH8Ovoxk5^`W&WYafQERP-q`)(C2N}7WUn~I~X$GzBhPEbk{esR{@M&WhSB|#JV%LQJY5m-zfIjO-(ZYl5!cVj8=;}O?1z48j);sCxN-^ufgQmeB6h+~0K*=omk3{q+cw%RBhl(_XxS1sq%X^W=mUNX(a0+-=~Ugq~9@XL?v`4Qwsw{Qe0N;w+Bi*#~;wPxsq<#2}Av`o|Q4l!@QlVH_*%v0!6H`&l_%UQj#sDJ5r;5w2dppGuE*mt)C<BZMft4hJJ!AN2mm?NOz-Z|1MM|5q2Ro){skzwZ7Dc$qg5Lq3P_zU!gFF}Eaeaz$ALzmtpc&?ad9?`a94rTsZCY=SCy<S>5z0)xAE5q&gQ<7(lb5+8EJH?SJ<3^ruN+`B2%d$vFftGs-zMcoTAu>u7v`DNgqKL3AFV&JBm5x2f%GUsFRNdo_Goazbx5*bqT_z1^m$IvVN`#~QAUj8XNpYQ75y@FBtXX=6A<u84stL*4o!iLVM=rUeZqRO47gb7co+Kz#h8H@75p1yTAqUD~r%>v;NcFKlcf62QR5FCpBa>ofBX-XbLsf|2GpZN8?%V?x%txh=#v${gZ=I@C6fZ8yQqrW5e@bNnYvxIiU5t|1IQg{Is~cSe|5?l9LbsR0llG_0_GUNiZEkMBKRp)qHhP;D(*KUnqH7a#n3V?eG~;jxkP10fxY$SqhIaN5v41Xer2y4%rZ7hhgG2LOkXtH54QZzEQ7-Cg%Tl}OZT*SAJKwFtXrx=L2l&#iXd#vYy}Y{&rX-WpLE#eOeIbdjarf|Ll)`QnCX>Nvo}n*{Q_5K^K&yg_q+~Z?_J$!)=VsU&^g7{Yuea;TZ6ZleX+)ymTc^fH|3bRG&TUSvli#{<7T3*{xpR#tqrUEuRqbwEnXR|KXi}7rpeT=bASTJj_Y1|votmEFu8Hbx;Y4+N8^lK7kx2ZJX%0z-p@3c~XQ*3d=NG%0!Lmi}WY^^aj~!vpzJ1;o?~-rglCV9>r<cy1>V$jSy)JGiO2<(Vz<ya#g%xTNy-L<<6cfQY-dHea40zue17_}3#q^f;t|ibnz{?f5jkK<%_b!Gm+Y?m}o0jF2jByHrBiTNILWP71ZB0RaVaSqg{|g&FyPvmrX%P|@xKa~+y`)`Vw0`i#U*_PGJMPMF)wGXqOA#xF=4s%%Y_qwqiT_e%7Wu_~FVUs5Dg~%y2p$rkNd6*bja!EM{nPp21KnMa3zIpM5Mr{&B2t)&%AoUEuTHu&6bo>kX5Im{eZX_=pKqRn9+;o<r-pg|LN<dhV*b_i+i;Z+4hP}{m>O>^dGScu3R0`gjXK|S6C7fK-NvfwXtZtz51v^2NCC@r*{$u(evzl`F1ggXa13o<F#Zi85J3<P=Osv)VjIhojhyNyPc@@ORXWj$z<mLeSJL!Ko0^+~_cA6qdhPZjm;Pi09^K-$C9hT?B&C!MSt^o@2mB7iyxA3>c4JPlTq<Sem0QWmMVZ;RD$9?%Y?)Zyn9;0SFNd^M%F4^wdYR3|MM6Scu1?H~fqpYeUT)rRJ?Gx}=39YxI$apAjM{6%C846wYg-Nrh9z6$C}dqK2<~njWpGthC9yUYyE%*FFVbfmT8o4E-&t*`5L@O7*1GeZd7$*c<8T4Dc5>=P{P_0{-l_8uK4dgy^KXsT?3_)wyZNH^RC=FfVRp*4hAf&<7LdtVIG&G|TUv{5v>Z-Hah=@Hnw*ttokg+hP)ASfadE*o8B@rnva(L+vy1smgty9|XFbUvy(hCVoky>zv;gah7L9$)uCS^fzv*`rdn(l*K7ME)l4!(|O#Fvyh0IzOC>piM!zX)3HUQ~AEO~9W$f>Sn#l)6(V2<><4C^v@Y{kZnG-%Q(5I;?D`yJYCN*j%7{HcHhDOs8(o@!oOg@MiayOl=so?MCwJ&0=SUg@hqv3W<_isz$Rzc-2(n{OXFt~ck~1kIqtsu1HHW3-`l@Jcsn7hq{VT;K;;K;^r~D-MFa=SXYAJ}g3=D@5vI$z0I6MQm<az^V?8;|xj<*LFtaQaxuRDjjoI3q<9Y-bh=h77BWY^a1XS0j4A@2XAI4vT|Tb1KKSq!R+5OpUM6U)=A4?Xg$rHRU6H_Qm7#}>%@@9hZ{4R$B0`2yWWPBJBwzXq3Yg*C^mCYpm39l&9gtbUtYW2E=>E3IDJ(C)=rp`A#c1aEQKd<;6rSeM4O1Q&`}#U`H1=s{+|9ku7n340>tZ<DLmD&Vf|SK(o@F})2_>$kdiDUYe1d{;dDA36+C0)xV{rIuYhVn%N$?TT7FW!W#vN4B|(ei>|XdpJbhwzx4J4`_(pV}JA<tt-*$5_7U6wt6Mm((!iKTv(1@C-dJLs)L4&KlkE42aCE?&@phw?)ELFIONCknV75hv2w?0|pY4d)G==n6ueq++bPZzO8)#W53^VEg5G@zEaNR91#Kf-L0POKw#p5E6sZjO(fZ1cU`laZeb6Bq%(Yc-AXTEut3J5rQXF-JHQZz<}bw1jX5rl<a9K2di0OM|MaXP3cGbQvWhOupK;&iqsM3uv`7@_kY-WYp-fm|48bcQ~4Uyss(KIa$RCyH`@7)O<$AS`3*u)VHZcxovyitB4Nam+z)IQR>K{&iYrQ8T?Vah#}AAUkJJJ0PBs8@Bv&B)m;vEH4?qHmz9x3>AriRW%*5e;1@E;Sq9P}YeoUNYPkA>@J)eU#zS3tkvz+A;dYDikuJPrq|lWT50>qUA91m`irb3_fG#iSg6?qg!<=l9o<*=M?#1f6-=*v5=Wn>$jNyIDbs?Cj7v=ZL<@2DMe0wRRX@X79rUw<$Y(94<Nixg05*L{z3h9B=tuk~ui{pA2bE+!HCayFrzOtTdCzXpEiAWaIGP6LK?|#4(4^+)Ux{(%PNnEMBx*RS3lOF!#j>a@BJD&a3(Lr%jbpoId)*TWi*Ml&YIPPktB*1ku8)kX3HY<Iw`;x<k*>R;z=aej49Ps4XkA*Ve<seG>3@huNrJAgs53WRSZkC9m%H%IXS%h`6>?)Y5cXaYod0ya)xJ9R>x<I2w-GLC@!%Mqa#Z^+f(TKMWV1M>srdqu5WvLretuz7!Oy*I}_N_j~F{_`hTSZsf++j)cRMbd99#1}&StuzN>@qW_oj|uvlL?mMmwl+HwcJf0C`_h>tItLpCwXMksZx~-s^3!acXW3uHC_xI!dO$Q(Pb`@EgZJ<3?0I+-t1L&E9{dn8vfqm73$(&(JNVALT(AT6Q_{Igw?l2Q^Gsn$~R2|%~G3zTK9N06m{mwidGiqtF<T6q46i$#j9$8&UrE#Klwj4_>gDSR=$zSlk~%Yc;R|2Paasvzk{cah)X<l<T$lV`7~19Cj(1aE*Urt6QaRoBCcKI(qt!xyGWtu>KQs8xKk8526O8nm<Ff&1)u6lyW`UNzKS$YAUXzLauh>8&peWcB8TqmA%szB{y-EQqys=rGj+t5RYrng)VuRkJuaE6r-AA5bNhMonHW8K-oi0hMn@COrYq5svO^j}tdOrkW2zJODYr*E38^LDYLE0>N}XkSrfnnDvLr3dxkKQsZth*UA^l^NX;(U)GC(jx)*Klwz~u~Bd2-)k$q}gczU+*#65#wopo+s)&XD<nSbt*DlerNq*H0102J0nCCe{$N)z-E)W%GbUL!%<I$5uyLS%X*=(WnbJy+=2`;J5-jMWdc}R%7(8m6U>(kb!2?@RALb)Ol^mdn(PGgP8eM_NLHh3w(=`Dd=JyruQu|<r|9IQqq;4Mq!~mqgEif!dY^Bo{Yhia~|l;OYy)F(7p`+&PQ&fz$?Tg+HSXAH|G)wL1hV$7qk{Zznz$-55j$da*qR1!7654n+<QJEO40Fq8N=u$=E1Ukj+`jGb}HICjTi<NhXU$C@W1LR_=MYwEv{K_jg#V;-{ipQqxh1<LGT-0hUV0c3lCRCB)=QN_h)vah&x1?WyJ3lPq9i4p-ZI)mU0KhFof;%hkV})RrW(idnC3Lut1P$hlse1oeT!z1)yoSh@>t{HM9URh(D?-)*wO>IiTjH2>xXZ|YIE%@?&)uBd`clxNz^vku(_^|rf4-s)0_;8)XYvfe=4`fck`WWZ1vkEFKevd-})b)j!rV?)_EUDwW-{Vic^D2hrCp2+qndDd;A_V`EK5qQfDRqio{<$IBMFV0O|!oACaZp-<0Uy`*J&Ij%+=^vKgHf(ozds)8#`n^f|4v&_58wgImP<pM!Sh&i!FhFw=$X34d+$VioBvf?PaqJWeP@?Jk6|<cpvZOCgsGoTn<g8v$ZUKeGy6DSi#!&@>2y+(%(dqP(GGHO@&La$AS1qqtxVO(pVG<m0-opfxxG>#r0&>@;BEU`W^$zxUt7oefHMOJAKpry=M$^HEmb3uPlEH`DuUw~N)qQfG2coLw1>RDj4|GR|EM>cZ>&0$8!lGB$y$w)g*(pr3nm*}3U(B_$L@FZx1@9GdGU?e@7UWJUBq!2VtkN^J-C}iBzAhR?&{XhmlznOWL{<OcM!Fz#RQZ_2U2DGqW6uIN&AV~*d$sylKSK}p?w4s*U|deC&UN{wW4oMqlKV?i2_woFs!E};)SxH9z%_Z|x5&!xV!<+a!*9ZVwb=3aZVTr>fDx<IN&2^p9c9ldk&b?o%*Lk#JbAJxof7Q7j<NFy*T%nFM!O6O%!UCU^%BWiAf^c60#UoHo@;MdxJN9#R|p7x%bSH}P0Uqc+rE^m)0Ra^J#Ue*0z;$SW{ARs7j03wjWuu8<!y2|Q{(~+I=Zfo#G?;&5)70JLLR_g#Zti=QNJA_PR=VR%mt<dHLlD#C(fQCvCD$YZohfPW@H_3Bt-R?n`E-nMe1Lpq^Z40G`jk8mBysHQPvt+bCXKV5pU9biD@D!OMv65PhKj&=ZA+9=b<9}E_fL`4iw%GR^<)L-|;L{%|b+yEUu+?(yJ8$n7lXJKDFFaym?hB-AOOS$&9gySCTBGSohv!N!KD7mst94kfdL(!wgG=dV&b%SG$<kNUB>`mOjUD+}D*)ZppEP*1vI7XZDPH7h+QihGjlySXR&VB{?N``1jTPCPOXcRyXhc@+Y@1lNa-(0uJ06cOX5sH?3QwcLy%bD>jM?+`1mky~Onpso&rt*P#2~tNRw$qb{+&wS+Up<z2i1&UxM7zc04n#gT_GXvLS|2r7?L5nsNXMn!c0O*lv4#tkDImYD=AP`CKliARB~G+L76QVi=6g^KYyACs68ri-d}7AJYZhn?}MBZub82$kOxgs?Y)zvDe=2+)ckd$2MnX=4<LvH0H>1`1xtZrY!A*~WIby?eSFZV!7=xIO4>O3-9Io`x&<&+^ODr@=<M(a2lEWTWj*r^C&iu(!Pxhn)fIg>3V5w?a$Z?P5#2y|^9rc27^k&c;S3>~z?v*^&WD#~_O0_0|G0>LxkOZMfbju;Rgal!7eE(oA6tADN!PJS-(ivl+6;yh;nbryV>|TkAP*mfVdDGT0U>u2D8fl5jNr$kIWSF>i>O){JUJgCiP^>R;4vBM+{6>h2EtXR$<_oFy~sEiRac;X~;FZUr?eLv8U=CINqWmab>O3DX3o^Uq9#^UHj)zC=dEG`U!yC|}-&ndh+aUwxTpDe_%wnU}z;+xh`Tu8ZDz$GQ9Z%$eax5A)aM7nNbveZ!v$cb9dlpiJPY2&BsO<je`FC&hB<R7-SiS656o?u%@{qR&dnt=xT-<Ja5cB*F<a9#u$NJsl3&Br}pF!G2YE1dRMN8ENC8lYfhrZaq*ZQFsR`Z|ltgaa_2rvabOdNRo8j&zn<;>*ifI4Y`H(Wd>eoz&qGn1W=>R<p3SwgW*|y1~Y=)jHhvc-_?oB9N5)aG(zkQLO~UM8HU0kP+>V(miSV?{~3c=_6GdUm=oxf<K@~Yo6RrKEB^1*!{5EdzTc^Y>@pKW?gg3A1%V*y#0<DD4iPA6036;58o-Td#D?hCl=se62^08!e|&jSFI?V{BB3l(SxY09q0AhzUmn!$tFVW&77zGwrIaXKwuK;6EU$yl`!DO<ac_X5`DLzBh$il9D3wnnY>(D0NdG>eFXTPpWYK9f{$mZO>mJaxZ>0{nJr&O)K-5QbsPz5}Ck$3h$Ua?6Gse#%M8<;wCalSd>z0fFYTey-1&g4_Ct6e1S-TVci&0;G$EU>k=aQH#-BZ|H74DNBJgD%;N_p$}2EG^xMhWg-9Kzn{4{R%`u1?@9UJyDvXTgg%uSiQ(iE)Dn<o)q<#scMn#XDbdPanW!GjtU(X^=!qT|Ao?z)P}uLJzGCtqP%x$M&|O_%fo?6zhb_TJEc<>-KB>a5A+Xx=%Gq?i3x!tD>qUA}SwL6$qE~t21&C_51(n;P^-%jU$L#j)-%Sl_E!_4OW!OeFB@+9rIjFTDIqMSSu@ESd^wnK=&T23%UNy=g3MJfBQ+|WAe|@gF2o6vCQ87Br;mQOMn)8;f2Qjw0OsrUAlJLl9P5d)|~O=!G}_fS#e)4^{HrC#n01x_!^z5>;<mo9cITnjd=OXvdD-cjFW!1k)>n?J?~K%r7VnQ(=linC_{``bjiZ$C^jQX=99rW=B<vumDErdE6K78=V>|`e^ZHq-``X!Rk#vVNPa|?w7UDkrb6C^j($&*$6=>({9W}c(H3SsY1~zE;MmFVJg{(?xJm){ADAcGBmP-9Q&dSrcJ~plzAf>wF64nP_4-?Qsb0tB0@(a*_5ZGvhday$a>A^3mQ9Ccj%0zl>Hwomn^;Pd6)d(<ve#;tc%s26bMCL4$GvU8bG-Ag6UD`?!J8{&&NIpU&DKXXYFtE#ek#0vw}55PK@lah@yXd7C9PuioS<D^!_KMG3_43vo5KR;$|Wj@%4VOqmkC$$rDPKdy$gj+_##60f|LOqAgF}W)**WSP1-MVO|-i4LRI-v@Th$9#Osvj+l}#5NXE!x1Y<0n_(ZgOWzC(pL7A}xQd;VfPeoTN_ll)r7VEi18g^TTx1{ogXD>F@c+-0uYGEN?-x3b8_3v>T0*8sk3egqK%6DeWCCjOlN5cvU#Huh{OP$=>7p}R4AwgTT*R?nq4ukdevt$-TkLb`Ld!)`O040yIbnr-IaBihpa9W{Ijk3B1TU+e`+uCilwl{Wnc33;;wA)*~UTuASy;5JbwY9Zs{r%w&!Fp%2+35yr_-7-42l^pOjmEv7M#I^DG>b;lGuqml{1I+s;&JhbID!qHB^RUVI2v4ujAGQHcnDA(Gm(B<X5Ie>z=8UazdgjQCmQkDQWPqh6E9C_;p8uE&tWmw$1I!9Q*~sgS!~GTHcK)(yl&Rk4f-%%%F(&s3M9#n-q59_#+mdTT6G__+wD5e;Y+>Xe9#Id02BI8&Ui)$*_wPR=E{D2GGLP^O{UG@H5;5ypz7I`q!!+6;kj(0@PdO_Lobb>wx0WxAOH0H;N;+Sc(VV;gBN?@n}g$%!=smUya>#C>#^>CUS4(^S}vA-c6fZS|N7|oU&4czZ@AYAlwJ98@A=;T>%;v}E4N}a@a*7+y`P@H4hs$CjUYdrWpXuUkHlFcwnOW(x7pkZ)_PmbZCsVwrBTPcMj%$_Od>16LtY=Q2Xc8rnZn|&Y5NN@<VcWR-@7!{%nJEk)IRFuJB1*PFcY-VFH#FkvOvI22NAHdA@~wqnrMwZS5cxkOkr<}6JX$E<qlSg<7uUsz2NtcYliWnST1<t892YeIpj`u^<qWwrctvkaTg&2bxA<~d>X|<MJjY-*h)<3)FCwQq+jlI^Tbx-N->nDz7!Z*0E3PpqjwWSE&;|d6#J@lA@&Y9dCI8d%%eqAz^(+bF!zcgB~ER6l-P75ut3mm3M&-j*-rGHTf+CcZj#RG={y<5Pz(l7n{~T~i=xxn;B3LSD0HUKDqi?a8iM@U(4fuP#4@!UD{XP@lDh?}^qu#9^r0>;|1^WzK~5e=lVr%ynJ*Uj>kwD%bT`ZKXXCJGzRJ;Ly(nU$xo8boEfi=)dw(q$%e4^P_*k~Knw@spV8mtM1<+(;f#XI{huLIRT|2s3et(IpPcwBfs=Yy>&%)5PF70)xkB_=4RMt_|?QS5dx*I$BZc6tN**rrw`&yg(x?3D+G&Sj%XrW#Zlx3L$o3EFMcnzN{SLT}Dutc>Jg+h7f40_yZyK8roqhxEhxvPk0IG%w{m}IChQn-Q-KgzyAxA+!^#|S2#Fri?t{Fo*)_5cq6Kn0jFkUdut?pL3aU7+@25|H}AXKJ$l`|A~JceP@XEr=BEbcoh&1yODkEJ9>O2s?08n3yrkvgnN60!fn8gItXIELwA52Pb=*)Bkp_+uXs)*1hZkhk`FvM4biJ8`^OS{v!7obll$3_X0;NIk)t;v9X|yBKLyY!lM}LcbXovUa^7v3MHd~+tl|AW>XI4TJBTQyhoD(vw)MawZT`Ob218dK-GNKlN=vb5%(s_9pxVzm%~{-Exc{WE>hl;Yg%izY1z8)N;MS%g3nlbktT#%v)*uW*@?D#-1)y1W#Q@742cZNRWi?LhO$x%jZsb*X->Cxd&79pYHi0`aeIeVGt!tXmD_2|vb5uOo1Gp?FU`)D+f<{hEkdcctS^?4WyIKQFUyj%$r$s{@dtUdxWM+0doK>clh=E%52VuE40P{W{JuXOgRB+v9|xDzn)=Fn)3VAte2kj>ezoc3U~-mBWE0}qtMe#h>X!zf*dyh(qk~Syn`6eWh0?2VGbd(|6DRFOgN6M>Z&DqIKBEXBM8N9=Ms~p%3{qOw50OShhpwFY)Rgqy5)N)82O-&0qxMVWq50D>M@5!DNySiQ+dOz&93C|T(j%mom{M0$ansrWn;6Mfn`%Ky?L(?!qH#9)WzOa-e?7lAOQRU3NWKx^%AX226e`By5b3NLj3RWWP`&cD#Y2Anp#XBI+QclHf#{E832Ma>ff-;Lc0?7HX-ies#dT%37Bnjn5^ujMH7!p_@*2bfHQfs#@zx8}bT>jW?=vh=(;Wy&g*N!8X<0&&u`eEKDtFItQClD)<ah|2J|`jd($rq!Ao*1s+EW^rc*@s?dL@QasA5lx{a&I^wz_sNt@^GNvD>$M=jPmCsjFmh8{~TL=C-BF<)@tABRLuR%C$NvAZU!`TrV!}hFRRP%hY{4k97~Jo2!)NXtDa{Hg{>9e~ufbp@j)rMQaByinsCQ8BR$>3C!w(oe(OwyJ$QBWNtP$aw9^HN|$P)>)bO|3%st^ZZ46P)G$JkK?F#zz6A8go%z@HL(?;>KG9%wrdNIJ%L%oFBbH&S<H-YLpUnz64NJ-_>!_uFG&J}wh47?wl839&L^6J<0gEiaM%a^5iC1(n-!PPsx1?jnRUqp60_s}#(u&vho!g$IV|`OYH(jXI@Y9a~*EXJeK=YQl8+abNOosH@<qF}>as4e8$DJ0)a*L$uLb*jW!1C**WX+iCM&`_}xM<wDN!^eAYpLe@2^%a~9Cq4tl-3p&2R$}lTmWX*R#xG~mxl>)KoYd;XvVcQ=gBmjdm+D~&&=XU0uj&R`Nt?br{pYV>J^d377J!7`AVML6Q65o-FP{Di=*&=*^SCpi4VkG7uD<v)Oxu|K@<_zgq_o9XJcp3YHh{6-R<^9MQeg=WPu?;7Nb*DG9m1EEePKWQJh#yhGJMQxc(!TTgl7~lLy>_)DQYC-Yqad_}xFACD~vK1a$R73Trk&gJ*7z&|Eq8X%*-XD2mTxRLFDo3uRx}5abo(x=F#dCWtb+8;Isx;`1d@I>Rx!j>gQ^9vzC~`t20QedOEl+je_#UedSJAS&I;{z@HS@y2g4^6IviHN*BIM{e9KzXT$xBqD5j?<HKQ4NA@C(W6Jf8*X9_ro#Z^1MoV`(-G#DTpzPBMg!97q;LVu<k6y9wROE}XE(UOuV^CXcWb(iNw{p?(%oZ<mOJV0(%A*>s`t@t=Y6Cs8ad8AU?-cz!6_ytrR-Jtfb-7Rs&w9MFR%A$Y97t<8<;Vk)&ZC@^0h76;<m&AhvqJ$?2eMp<2$-oQ|h}(EYc;Y*Yb@R7tftwu#Tr%X^)ztrw=iCib^!1DjUH$p1dROTCNLktOkm!SchI}<$Y_h<|Twfs+N)yo3~b*10hxgi~Brn>RvvZeBMa=G-j;etAn1b!XpI5Sm}6Sz9BDOqb9inYvu8`F!U_+b}1+X{u@hM3HL?J>(+BgAN$lIm-uhGqsjDWP)^)49tYrZZ&2BE9Oij(SkUFJ4m&>7>Yu`bXj^A3t|cK-<2ijG+{;1LSb$Ey+k}_i-N1OLTe&F#Qb0J5>xwSq{23lJ+-Y5<*|R%wE|5~>c#6`zZS2sz^|qRw1rt|DS6mo{BXP0qZ33d9S5?hP*Od|2_R`ml09F!y36HUB_^y0%M86kC>TIx>q+h@Fjc@Xesy8OtgL7vjzlAEis+P7#*<74`Sqq2s-d40Ydap#WRSGTU&bSI-I4sp9kLC1QBV)ShxJ76%yqdaO+vJzBv#lE>y?jfAj&MfsrI7j?ttDEPh@!(&iK|mnkGSBQ*I}&Zo-VpU8O|jh52#yZ^C-r>S|c-~iM%7DurSR_OIl-^j$6y~L5sJ>*s<oljlblg`uoFyE6l`lSX&+qWkOa7GF-b@b-JiX?L}8Rbh5MdDsjJ_pm-H~)E9ELpm<ZSE;`>?jn<q&{f6x%o({4{<I9T>_v8dav*syljX}Ziz7n>Q-S+0zaO1SqiZ^$6+U;%AR^qM9Z6w~;Tx8nXYHpy+gnxU;eT04l-0mX1`85k*ml>S1!H0_}OkvQk1?$24q69`7*8AWwy*}A{qrd33<qLO20tPO;Z&7Bn-(fiUdb_(B{Pi#YJ&-qXW=)xW7#MMU^OaKGWi;_OUuRL0;u5Q^?Vm@#%o%_DD=<z_fwP-mV}?~S^eO|TaXty&7Z&mRAiepD#uKr}CfUtbVD+`NVRZ9<fzS>072Ai_*u@-L8b!fb3R*ut2u2Tu;D8WjQ`N~~{Js_Jvl-gX0i9oEj4j@Falrr@CVjtEUOX1ud__ak9F_|i8JUs)0uX?s75E@e$LR#nfXeC|x*i47IjD^RU5vq6!E;0unK=N9BQ^&RVJ2e&bc8KY6R@2D>M&#wjjo7A5yB5jI$<2g4yrhi#e268J)gkm4{<_f9G@tP!f*hY7g35BokRQSBw=`;Da>GUHeu+7gD-xb&H=Vo@Y94kjp-`Jc%IB^YcOS)pB0k^ycgE(_rWL`Co^dC=3jA2(V+)4`1m15B6$!6&mVGu91%LY`Sp?^DT|hBYf$mMJXm@k@S7&5m?4jWTEn3Nd<@Y{bdK|viE!Uxg1Bx5Q3`{O@ycD`2N(!ALQD9~S8)?cj!1z$0TYy0clD}a2XT^N=h!I_Qf*C*mo6rFkM)AVD8T}NpAoPVY&jneOewSy4TlK;n{t|w)Ye1{BFza3ilr&?m${F;)M~?QC^%d`w@67WMN|ywhDG>^4sHMx%y00%EOuM20<s42z)5I4p!6dI2gsFZnH?!S!{lO&BiYLl-^pYEBf^Pcxb)!rrNZ<m(pMKFB&E?Xjc)#5WO)cC3027emITf1h*=~!fJHQK1qZO^WLrQR0fxdS*k6oGWWrY}9?vp_0ANEJBx#?!t>Wv^G^M#EB-PeH<2sFIgh3>3#hr*$0l9=coQ7zQ%}RC<0cp&TLj>a~P#uydhaZ<YaGE&5+jNl3h@3K>^7G~^(fw>jq=!qKPz7vZ&L@8!aCrFIoS>BeIl|4?6lVHY0D!lt^Lzq*(XIeReER}t|7(tYp(9^-_l0d>E5%JToklSpv?#kjXzIYdw7t|{LOL_Hx8rzov(@TtZo{YR*iuW{(0106Qn<lx>^66THTw4tf2ajtg1`Ph{{#Mg%eT3WclF<~$=Uiwr$HOut8{u1p}PZ89PpCq{1O)&O!+4gRBnD9Bg2^|m$)lpI|na+gxc@w=d;;G)_?S95XD!cDf*|XQ;|mm(IZ~6ffqD(;GfOlOA(y)?*%};9S+cm^hza=u*FXa!|f%*z-in}jJ;8T)Z3{F680oO+Hr!kFG1SnAnnn@Ab4Pl_?e02a)BI-iCv9L&S*S_{Rrq3R<%s2it9oGxmy{?T?5E<3&@_F#;ygT+JQ7tMS=*+=xijmTyqJldF@u7*KR3%n-aba3qI(6T3yCcaBWnEYr~+oHGu1X{FmP@-fZ}OD|oXP#im-Po82O0g8P%WC%OHbjzS-o7!**Om65fXBMZAMg0&^l)#E@N!j>zxY=kVh1OYr_(R^J<Kw^Iqc2<2tX-P}0^(tenSISDZC3u?xJSpr8iD`n|b~KVuFci+{<VpgvSs9Q`0}vjDx6x^KIuyuAU*O>@;F};6rCftZ_~xGo>C<Gl-R9;qL{~IF6Zp)c0H_PW%}^@OqCtuyet|?6bBzFgzKKxeCuG3RM*m}}Lj?7f%LTa$8m8tmXyE2A05d{Pi$^_vO*;aCfZe@HlW8heC}IVl9q>$he4=*(k|Q$yY9AyPtV^ixZaivlKk5Q?@vdJaDA4~aPEIOHY-=UfoF#U(5_k>=dgjq7Z5=}q9M<V<waG6~eu0O4LG#@WFs>vpnja7g=ejs|!C>S!|2NL2pkf$8OZK*p!~Yd|LHsZp4A_N`f&&mZ0z)hWArnPWhH4xLpL3AuvuI3Ra;AuwnTY;Su;qQCv@k*~i=jaE_S3ujZdE#7BD)>IzyNMgN-#m}9M!oQAbY|Dhe<M&6oocKV@4egC45m&<KnPE|1t;ueTvS+T;G&8x=;WWiPOK#AL7ac^?jD$f}IF0n2+>UaHzWX1PkJaZl*v{TB*}FUw;NlOc}?1M)dIZumAP`2#yI-uCkBExP*W^$*=1%TOUR1LbPlM0^Qyg41|BVF9N+C&sc6N`xZQ7^WUClblY3aT{NE2UwAZ9suwY3pd6eQSbHfct*9f;M~v@uTq~@~9|j5TM0|OoS?g;IEo-W}T;Lh5@C$S$Kq4hV(6o55_+n`>A~OxrGTNc$30LKXooDnAO|~g)NlY`)$R=De59UEk&~(MEGKd7&QO}|@W{*;K0n385bnbLNW6~D#UB;y~bL+9)YH<fH1y_V%Y6`7-A{5y8bn|p)x7}*(4o{<KXTzny=5=(1HGfJ;SDM?wT8{@7{3w<$U9!VDU?7U)U;rEAC?(#=ZJ@Q_y(g#3BeYpXbV(M8Q1L@HxlB@+ADX`!nhJ^;W!|HGX-qaKv>FP}sR_wv!aNcKp(HFtT(3W83OY0<;nDV#TW*8%d4l1XgYyJ!vd2d+(L_;Fe!}KMyL><<Yn4y|ZJa?rkT*MFGI?m|Qrbnhfg2h9bbKWXXS3<x1IJfzN)dfzx8&Lj-ME8o00Sk%l{9*Cjg~vIc6j!S9`|Tia>NLcmfu1EH3cpPY;zc$rpbV(bWo!CrzSLd^S@B5!EG~Tzo4m`U?hK2G|KUjx^vCYpc(9;4)shlO+%kfVrg)!sr!eZBb`Xl=z+Ge+0_Ndjoit&&B$Yh1^{Ghx;eOMSvm7`%)krK7M5~0Bkq%Ca+uD1C)kO!({cXMlu83yGBfU6NO(KF`Jce4817U2aAE$5_yn=5c}m=hP?h_fawDvqA9_Nf2yE{uXL`Z_gJyEh)QD7eDdK&1v)SE5S~(yFNHvvp@)E}d91zx4a+XN78Mq+g7>M)Xd4$7P6Nc79R4nmZO-~O(uQ)B|GtEZ~FF+7FBjL-OYl7wGE8c2`qo}Eq0nXBA!sJgP6>y*dvRP&Sj2V~Qaw!M#h$lv`q~*7GBIG3!X_VKu8)$}$bX?XGrf@+_wKbiZfS4xG1a{1iEcj^&aAvYE(a7oI9v0N!|428RzennNf5ge_J^BOTwsINqBbitV4j755wjDtU>Ov=&D{hd+PH^jtq!fDdwYJ930Qj*4w*)XHFJ<N#ix?GPd75N^1??QFwpQ&R>U5ftuqMn3n$g}mLlS2PQbgDJI#vewP+}%MJQ(PkIIRK4`5TV2wkEpJ&)HzZQ9L3SamCJs@`DuFk*Wj+BniU&f|ihyVHI_2lUrn{jHeSMX?}*I?E)N7nt!=)1tQ}qml)V5ggb3=%cMMfGtb>bQ7)eYD@X=Hj3+<dWQguz0@a9ltqK|PQ(#;YaT4Kjr%)GU(!y7#bIsuAh>k=H2UnVeq#elB)+Ti5ae4DAke;AGd1u69eu5#acC!?C60#hwh)E(>QW=}z0cL)(rq0sDaiw|4G@izIlOozgjMGIDv7z%hx8c;*KthlQTk?>{ecs3qd(ce)*vA_l?xAK{igg)h(?S*+b5zqI)nqyoy7R~;1$AJBt3AeS(7yn$BU4;f`~LxuIKE*"""


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
        prefix="galactic-mvp025-", dir=root.parent
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
                    "Le patch MVP-025 ne s'applique pas proprement dans le worktree."
                )
            base.run(
                ("git", "apply", "--binary", "-"),
                cwd=worktree,
                input_bytes=patch,
            )

            if run_checks:
                validation_env = os.environ.copy()
                validation_env.setdefault(
                    "CARGO_TARGET_DIR", str(root / "target" / "mvp025-validation")
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
    parent = root / ".mvp025-backup"
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
            "Prépare MVP-025 : occupants déterministes, forces configurables "
            "et renseignement planétaire borné."
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
            print("MVP-025 est déjà appliqué ; aucune modification nécessaire.")
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
            prefix="galactic-mvp025-verify-", dir=root.parent
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

        print("MVP-025 appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=20, SAVE_VERSION=21, "
            "RULESET_SCHEMA_VERSION=7"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
