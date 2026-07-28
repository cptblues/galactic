#!/usr/bin/env python3
"""Apply Galactic MVP-022 safely from the exact pushed baseline.

This migration adds the first playable reconnaissance mission on top of the
generic mission engine, including immediate knowledge reveal at probe arrival. Dry-runs are deliberately cheap:
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


MIGRATION = "MVP-022"
BASELINE_SHA = "f039adb8282840eee0d76e719a750b53a5827a4b"
PATCH_SHA256 = "bd7a3a55226189cb9280faab8173f43447cd7ea3f5e3463073387d6e5be47dc9"

MODIFIED_BLOBS = {
    'README.md': 'e3cc49d75e904dd0083cb99883daf7332a8009df',
    'crates/galactic_client/src/lib.rs': '69d414a584ba2c743fb3cbd8b2c3d3fbd3ad2588',
    'crates/galactic_persistence/src/lib.rs': 'dcb01d577afd674103afe1331e324fb21d24b4ee',
    'crates/galactic_sim/src/command.rs': 'c12577fd26335742a512456955e30f70414d473d',
    'crates/galactic_sim/src/event.rs': '72c8a95a74692458ae45793a9f4598a9661106fe',
    'crates/galactic_sim/src/mission.rs': '7314fd25af193933f05d2b267d74915ec1a9ff84',
    'crates/galactic_sim/src/simulation.rs': 'e5425bf5ac6fe2067e0081f01cbc6ac398d70722',
    'crates/galactic_sim/src/state.rs': '34174fda9debe9d62552275f521d0d4b40acf0fd',
    'docs/mvp_architecture.md': '2d2a8e20f5be5ee34d63d256214df97ea39a4ae7',
}

DEPENDENCY_BLOBS = {
    'tools/apply_mvp_016_b.py': '1557ff3f419abbf6a1b58b897100aa72da80bd38',
}

CREATED_PATHS = ()
EXPECTED_PATHS = frozenset(MODIFIED_BLOBS)

TARGETED_CHECK_COMMANDS = (
    ("cargo", "fmt", "--all", "--", "--check"),
    (
        "cargo",
        "check",
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
PATCH_B85 = """c-rNC?Q$EplIVXv1*}vh<cgw6iu$5spX10%;))%6B_;Qqn@v%p8Bm&JIK#|`9K}-Ad5HS}r}kI8uc0^DC%N4XU<SZohLoJ_-L1<X5;=p9Mx)VRXf%RwK4)877h%r4$Hzy{U%WmVECP03eeZ;Ez<*_Y+xI62!C<iOozMBfIU5a!yOT+0Yimod+1cFOG;06#uWW03(%)m7@OPiVPv1)~5_VpMQ2;%sJmOx)JM0x_L9+0|nD<%au`FB^k(Y-_%y^CumORbEEcarcvlz<qB?rLboGrZ!ez7n3)%5^c$=FrFvksImuJAR_Q_fQEr<`*vxGchqMW6B9%h`hGAx~N6#ToOHIOFLR{0u%jJBsr#zdpnE*dob!k#?|;MVMs>k?%!*5pfnli+Sq(#Ph7rplX`HsO0l`l;k=52k4h+a*^^Z<E)6oUkbhexH?-|k%|UF|L2Tl&`y5$;R4@hgU)8#TlO6w;3R?JvsZ;5CeR~7pQlBbbLJJyzw}}n86Y8NcmDw>=xp-%D!IP<z!6Zu0B6~Cmf!up09@XE$hi;gv+V9e1m9qcG5iA1)4LB>cfaG1Jm!bz@a68GECUpHX&PQZYl9B!uosD6;Go2O`AL!He9F#(#Lpftu9h<|^)Ey0qDVOq#TiyY!q_?J@3PI^A&>|BJihxdzx$AKoZ-6<L6|3L=s}OFB|sj6&S&Qch-~MEeZ@2Cb{Tm9bnb;P-#6^+@QmHy>tpU;a_FBPp2~-Z3PDMF9_C(z@BWVOHgDJ~B(W5xipT~q;lZpE*+HRis#Yb&Q!(90bVP$#>X=G(*e9Q`FMzlsAnHz_uQVT~lV^`FyvPG=`!hcZ0ez3N)PEd>=L49Z^VMbK%G^B&CZmZr7>ve4KbUyt%3R%wSh1_i;e?HLa3#ZETFC`abXh?x-s$vxhuuEunD1aQ(+m83<t_MgBC$SwKf+b2ef<VT)o0(t$-9UL7yK2!;*s|I6u1Y6@~^*tJNx?0>!aDp-%n1DUe8{>=y$fT{Sz)JSd}~}ISG6RlIM%>!z?@xBUs^mb{a1D$&&M+-`T|Kuf-A<P{<!|FJThPU&lNvB5e0w9f{Vp-Uv&;riybmk6Bv8-5z@{R>jUXVtHq0BGv_uiv{~G6i8zdjbFl&%SsUX;vujto{Fy3lV8JJJs|Wv53Z1wOMFVeUU^0AU%pL~b1rM1CKneGf5Fd-i?32|c_|+sbK>0Jhx{FmK~Mz{9MQ#axF<k{5oJs7T|6to?gChlpDY$$9Ar~=D8Kd8K-5Q+_h>k?^CHiaxV!XHA`Uh9>C_rF+kk}#MN*z_z>hCCdQTqGs*X3daYXHv>~TXpf*u~&cr^tGZVud$NmfQpXFfK(b^nHIQ^_`-$6j;|VodOAZ2lW;eDnT|>jc3zBu_N?VcGlxkW!;Awn0DG1^%L@zj|QPI)xH_l#Ci%J#J{#PogBgo&jxo7bKu(Ie{oUfJ&sv499z<%M8c+6{VN!ewJM(Y3>(!_h){6o_J|6WuJaQfWC~EMgI9W{Q58n__H3{ddA)?vCiiT@@HyA+%g^k_YCJuwKDjr$nx2ebcmoU8<|dDgdk0}jg@qgEO@uV#R<=e-E>RFGoAiz((B!`Blz+ScC*#cjZKNYisG!28d;e@Xk+*`0dXnbNu=)|fWQHy9}N4Wp~SkkTt?TxUnD(r%}I^DtMG#6WWZtl%`#emU3>0RK`VnHdkTtN!L`@7)o7as%AP*c(SjcVmqrtk_jNN(JXAsuW)bOLHY_+B4=PM*i`R)o0fDi=VLl^J-3aqtN!KArvz+8quLonzWIr&~&<21A{O#9V194D&?e=<h4-Xor4)l9ggNxnTjOw=I`r6Dx1S*=yAi#37c@agk3sIUqYI!Z!`)GTwe*mO6nheYJE*X$_<%N+4Y}O(@{CxT;NNo@q_<B|%TN0s;pcV*vgD{(AIElR#h^QnCqF~|WpzUUlx?0-{SjM1~pcTO{LLYhKYg*Z&T6fm#_1C5MayUP$r?Uu^v_|SN*T(>8WR_p@xNGWo(`gJOIOD%Aqa+m+Ma|mPOQiJISQ+=EbG>ne?ptZAY>#&K`lD^6g`NHW$R-B(ccmyn*4jeZuo_t)A}Y;M7YrtTXqoFRnnM!EXlNBsr8nHIDeZ@}8eaxVaeW!|%C&^N1nh3GET1wRarmgKG#4T|QEZ?fGz5mJEKjG%<Z*`Zf4`-iBv7$*fL-*k60tVfnRrn{Z&|+A*7{qp8DV(u$#~>Bv7^BNWRls5d~0`7R_ZBYB);1)1j&98izb?+{M)RA64=QxVMcPqTlpV2d+VrG7muDYdvU$sxtbJrN0M?kOQBtj{hFt<OL!!eq;Bso16`g7YRAPeNA0|Y-x)*+b`+?>KG&Hb6hL#KWK`dlZ<KMgy}Pr6PJ`{;-HIa@%K*xwu~D%{0sR$PV90|Bl&squM8J8J>hYwa@SiId*Mws~DdIfqDlvt%q(f+iHHr+Xnh$1Z`D^3vX&qhx51xVi;1O1%<zq2yL$9l|?(d{K1Zj1iq|n5p?uIlh*!$^UZZ{0^5+#A+Y7GrKmzu)*F4?5;|DX};!l1<uawO8J6kOF3-1tiJ9<<=u4MSZMS0KO(zuw~?ba{zyS7A<WGx{OiPw#L4Ek2Q!84UCGF}*uZid_5oLOHau$LJ=*|H3lDZ^Y|D{3^;nR<%d~i@x3i@~U<6KLzH0iwSO*_9H;n-c{t+x7UmxCH~I?zerOugJ5JR4+R<Q?V|<`bg)0^?~PlQU%6~R-oq$<BbP`0C2^F{eA}WEX}mo0z*6DpF{&o2dZx{uxg=#jxDt_Tp+s$8vvu(dODBvk6eC?F9xJ(kOjY((N`VSY#b{*-;4|90c^H8<*}ai3Z`dRGZLmb&6zVPTeh~tyQQ@j8!lT*w^$hGgt+J{MOI#FfkU$&K<CU=~^9mn5qRIm<^qq}Co&eeq9pzAOwINH*UPp`uoAT!xBb72kftaKLPX|)c=;QL1-5XK%#$o?hxV2iZ$;}nLmDN)y?L`^zMot?9U8|78OcFf~VlaeoVa438b6dl{`jkxlB)ou$C8oq&pbp3pY)rf+%;{^-!23H`OS2@MVb9&08?7g`C9t|LowSJvOEG4Uramy;oeH{F%9ozd^ij}kEYrIW>VeLE&024Y{2b(PKvh77#Th!(qX-><jdkJ^+!(<cxUB*U4V_m_rH_gDafRz$q~y43u639!mVgQ_0i(ru_o1bmLo}=aZc)LD<05J;|0P(PvhdvpFz|d;R9|ob_X6Z!A2}E1#0U5ew{B)$zSg{yoq~QmNaTwd?HGAcYa``E8BGXwG+PS}WA<FFtN}(fmp6@*cGEaL)=F-csoL_a&t%HcGNW;nv#PCO7(rn%P7q!=gYm^IE6%&k-Kx6!0{%I~nX_m{%s#!U6Ao^WAl*nqTliTu*Kk9HQ(R8VB+aQ(`Kz(~*Is(XGg0~&=sp2C*jV^bDic&%s!RquK?I9$rD9qsd9o=~rvCFE|BnBJCSCr-e_(X`m$*sIaI102S%7=Trn*>(*FoyNGaMC&Ozo#&soDctrrK`-<}6dn`Yb5KD(#wkYo#{-LcX6)=V`K_8PB>UEtPgE5Dw5>M^oOyc%ICbV9!QUiPJS8Fx5V<8*a~}E`=7`F9lDpr|e(x-{)O2;!eQ`=(F-G&PUqov;P(5m!H3c-rizo_~zL&p<PbK6VNWlY;&?b+U<{(qnGqKp{jQ2`?T6DH1#n<dPd|4;sFtGzxt_gUKZ4?B4ZbFI+Y|LoJz<1Co*dRo#IQ+;r%TosZx|uJ})xdb)PDmOjRFXyg+Kn?AH34OrBBY8M%I}2HoC5_sC9#bc1g&uDtxw(``x<ttDV&{fI%~B#6P8Wvd1U3T*NlRF9EJjR$#R&dg>z{78z?70n5JDN5J{BdZ<)^T$nFn>QOfx{{l9MNtz4HQj8(3JoX)KA<G4lqd|908_3{UrP%RZO2;?ukbsXfD~l$0mOf%vyn;}M|dpttGS%eQA_Vjwnj<e$FfW6ecY~GBgY*1IL!opX`>)eK1l*ND9NTE(U?I!@)4*NJ{VMrbL>vgDKy!h4EvMahcS-Zx<#lhGWt5TLK@`Qk7(W1O+!J2R8yzC<~Ae=eiv7J3$va>r`V9zm>B~Ugdj^}@;E#x1A;RC+ZK;0nne!`l5sDF0aw#U)&lmJ|AKp~K?9_bCnKTE*dmJILf<t;nK0LI{s7Q~1NPe_ubiKych>=@wWRs4;RQx)RqGjgD^UBx?cpkD>f-?YHhJ#*JR>h9d4yVElxRw+7{Xc*8kfL(Q1}1L12n^$OyMO*j0YGQ6$=gMqkw!FXT^LT`Z!s~UVKqAS5rv@v&&>@^O2I4?U%RBEOrCeI8sR{dWIRvNdy`l0Z;XcJQKZVz+}y?ZnYJWZB!lTjIC|dO3%p}NZa?)b5OT{7Bnb@LWt3Fq<_LBe3qo)1!#;PHw9ltoabK@*>yV&%e96oGZ5%ccmHJGd`@Iak$aj8=MMk}R~Fb8Ypnes31u82^A=!%NpSvnG<_GNZ%1yu09aM6n|12FAs8BlBLQSdp|32Me6bWi4)7C@Oyy>*6PXG@G9Fe30nkgat^&gtX-ILDpj)g4YG&*oycco~B(Yj0x6)Mj<OhF#(S;`S-j5x&NCJl6GbKGS-Y+YF6-}rABKFQFdm{?(oa}G!^mi#>uvUxsLzeM0pYdNFb;&7R#?XAKqEf(Wz&)y7xk<$YXrKImjr}Msy4YgiWkWN7Et~z^HNqHZ_XbQ8)dyI=TA<E!T88ROrvaaPpf7dx_=GZGhGJ!`NSV^gw1(#j6w(LT5-U@BS5?#Y*o8w@ZVH$BQZ`c?U}21#?z77xaN|LXw|=TcGUBfMyRO=5I!xkucrl$Wu9lVmK($gvbT}R1($mkgABI1IWo*?Iq`kUgxi@9icQ~3$_wo!i#tfs0uc5V=hZmRUNlJbaxhW`m??DI7AwjdBnkW*m&ETO4pH^K|HP`Tff{mc(%zB>jTzaBS7V(8FFWnn&zCAfT{_gPf<(qG3hezL@9v%1DxcBFF`ugbf`K#IMm){=!`F$N8|NR7eJNnz{@$<veH~$60JNf$M+rK|Qe$i*6dQSr$^YjAd-)!1!PjZKuf@9`;pjaosZe{h^MH&V(;IIOmOon7tfUg{4suU6Hxetm^n0J+)a*~5E77KPX8xDssChecSJ22qCE<rKqZXh3F^CCuA*gVWLBVx3H=y5v{qxxZOnCLi`FHW$q^g{W{pW6MQ4U?zbzl>r07uPRwX5KzOeF?<y;^_2<NM6(0er$kH#H7iMG#MADhoJpy!3%zvdh?nmpbAH`o?~IuC~ed4P`yA{U%mY5>(klW<2PR%>H2LGlp?6AYx(Lj?q}VdVef%syH|MKn$V1TrYuu?osFRmpBk|aH7WS0D+&+L^7BAz?%IB1WAu%c#D>_Kh0RT()smFP1?yOd(n~L!c?#`w&&M~UBRRw<XaXJukU0DY_cHn$R*4<0CD!^-w`Y28v`f%*6dJa+6O;8yHTQwD-lKX&dxYjRFnt=BQfPjI#;(yNjl0*`jGQT@I`i8vSs+|HS=R`Fewbd@dN@a(Ji~KI?I(2H1?4fFz&{_3f}O!&XE@qD-#>3Zrqj@r`>akwVLF=8pRi5*^Vh$2wjMvm0G6%ccr;bV>A*nabb3wbkBV~PK^`vX;8MoCIFMd9JeXECGOiktl}-J-uf<dekE7L8<M|={R1t|sBt^eUk(Yfb7#}_+3lzV+c?SjhOoZ~`10@e%Vb`hso7P-;m_@unp_N|AkX%EYRsoTHW@^FJZ+v%wvYvuq<U`?@9F6H*%vi)sHJ*v8js-ELPNYRd5j(i!6ci%JH05(Z<fYivQ74GD@B|Tg``F3z?~i8RA03~7w4AbHdkn-s+J%L-qR1WvZhi7Yz*7ujrHJZ2JH%)Oz^j+$p%)$ff@U8!gl<J2TJo+q${!UWwm+m8efW1!GxaJHu{PZ%QlB_VN<@|0EV=>*P*!qS(gg0_CSp6G&%Sz&N5oE_pO&NU8z<28<6c!4^64Ag0JTqnom_@XP;QVkbcLmwIa?9bxbq(gdey3ZR?c;V%{dh7KpQb>^O<&z<Nm|8f&%Lvg5B{RMU(B0_xs~<qk}+hS%3k@gVHKgB35z}G!7G=Ho61a+Ba&K+oPwU(vbItBYoqqF2~4Xx4oPe5ba^OmMUpS<Py4debXs}RWSbwNF3-$?I|oHd!5@}_1SmDB#EI-#n##t4;;HoGTLCedcdVAGOz-ddOZx{Qhl(ePi4QRo>Mx54E<qC>JP$N!NUd_aH<(H85C?%kHAyOAO|ZYZp|9wy0-Bk&xzRrbceo+*jr|oNj?(_cy%zr)T&v#wgP2I36RHe+?q<gkknHA(RgQXZ$21!e0$eFSaF!x(TrO|aTFw>Aws!#<f$%{E_B$nVskpOOPgj|X$Sh-l6Ow)+_a;%G40c?I*~g&*tAK%R0yO3E87-r$_3Qb*{<R<B6z3_GpRtYwl}AWCu8$#O*3t5wokGuBByw7?C*OApoot4=6-PCuQ$bIBkdDh79$TZY6O$&tT<J&G`zQ^dLwp_`|5<S2)7_fcqsNV=~o$>Wj-d$UXg?TLPc#zajRFy-?E<gQI!?+lX5RF8?7E0t(Hs<Qwm&ELRChJS*4u;B~tM)-f4vuUTPs5q^gp$MH`iRq!6+t^;~*b0a6PX>YGBc@nHNa@Q%xvi!~t@ty^G!EW}dnXi#YnljU`vy%36vrs0CEfm$x9CcBiGVpk~c24gB?AJh`IFw?butL5(wCUQQtsu)o^wP!tZg36GX+JsoGmJu7qZ>x}tjJqxGdXyAkEzYIF<XU-GRY-BK?a^c&j0c0g(eBRq!Tx&OOKYZ`e`y7g4EB(N?FoyfE!#!M<ZmL=LXlr4Y4{uAM_8SLi9ClcF5(3pTek(3UhY-mYsYBGp{jCRtXvC9?>H{^J|va6psE`pZ6G!QE!sCYr}(vMOm!Y=Su~>MS_%-&BzX`*XxDQ_4v>;#JbwI`JwL~IOE7T*m2b#T6Fd;l&aas#JSJF|C1o(iWwIOyf}uF1y`740NOM`P85*%-9Qc|dKBuN(X1zrZD=t*9Yo^w)s*>}9N3*o@V@%ayPtCXSGI%C&i8X|MP?j@P|7U%9sIQfSx)|p^o)6nR7&Re4aYf-4xL5OyttsZCf-wmnI7_5aDMwK?^6H%&yDaid!>5e*(jB$^>M-zVxR3KZ8V&nnQ$iTb^o<BoD|5C`jJRxO^Il`$)W+UysbQf8HI=9-<yT=+WK;J7BQKHLcr(bxj4zzBVzldeFQW?8d-Av6f6OLQ^}#lot-XhLW)fE=Id%LtDC@Ol<#MFpd(BHxTg@uc;^P*Dpgo<*G{8)hJR%!xw7uW7V(ZOeMD^5kot5uoT;|ld5cG0wX!vtk6qvX)<jFzPkX@S1Lm8G%z(;y0s1b4`jG?YFJCznD%MgF+N-kx+vn<Z)y@)I9)&;&i%!w&25DmLK7<BL8VG3F;IK9A$uQ-bl8>~ezl}QBp+EUdwxU_Lr1<FX$YM|9iM=a+`)4atSWf|`;)X}1s@mnv;4X=Z0z$JNA>?4PMgIW=hF^w#V(f(e4ZzYmJsR!7U4)X>SmU)!CQwAtV_`@X2t;glOSI;X&tXnByJ!{V+>pJ#DG*?BO<YE~-pXWR+5mcwMm+_Ssg+XPI$Q^;v!5BwA9#0zRi*zAl{IV%EQhkK+1wPd`YSbzaaSS}Dz2DR-O9In)aaGa9skQQg@}Q6i=hnzhHQ*$?+F~gQ?qOVOJS5d}JQ?>#I})8;vN(DsLB$v(NJgqo#wqv1Wk`fvZK8FoKzoa8TVAs&3yg78*wt>e{79pp)>Qne#7aSMTe%1<alA(V)e(@X!$V=siOj`^ii--N>k8b9M3vja-Tr7_k#eo1(^0OUoz?KI7*FpnRca6Iyxr5Mu5)%)yn*CQ%?T^SZiI2DS=(ZO9#so#<_pn|hQqS$t`~9NyoX?-rfDCcZ|@B}afY$NXh^LeXnx?qFNG)0X*Wgd^bpg-`;D;_Q=&i2{!$0&wo3GYq3AG2paoB-d3sIW{Vth}A{U8$a&x4g;KGCM7)H{)ncXlG7`L!Mf~qLQTY8j#GUNyX0W34}k_t2#PAaJc^ehg^IVjp;r5cBZ`&LoIWNphTQRx;n`<1q;v?o1tS(9QYvImufp&htYfeeET7+GrygeNM&lUU@F(LsNIe-%=0W!9TbbQnmRHe>lEMo`4z!i$&)8QKydL-yH{C`p#g%acXudr@>vVHJ->W2cu~rOSj`7`jZ&!qkG2u@wd^3?j}bIB6@1qigAp2q+LA+QibLB@xy-U@zw^Mpp&@HOw&7BP8#|A|w9=QX}(9(-7LDkT+;b9V-fB(Ogb|Q9x`Mqj*Wu>j8TXKS^kd9k4lQcm<`z1>zKt3orm9GARSLRY2{6j$;bLt3tC8z^t-bQ7oYd!qr@1^iIWF!0Sez7sv&)<!$*O0Gl=M`Z&}|I^`wwg3r6U_dvS5p7mMH{cEh-XL{Zc0|G}%5Seeu22?-xo%}pYY&PYxENX3(lE?1UlLiCVl3{iUg?$s?L4uJtR#pyTRwg*Y5GngJ;=RoV?eK13>S}&fVh}0EFyl!~*)$w^OX`f<JQku7S5+Zs_Y|q7Fx3R<)4Cv)U9SbE>oAzWP(6(Ew$}OmvVN^E!PPPKvZLEt45f;?^MPw2E2y$)$($$J+!Gr@VJ8^Z)2fo5@P@+Dv<D&Pzj39+p^~*6FlCzqA5}CWWJSb6ZrjAGrEZjh9rL_IwR|l`nMk^`apDdCHlSXdjA~iCwbpdiB~NlMnq`;aGIK$IpQleBr1CUmg`tc(5}BoDV`dWSR>7q>kzM6S1<DZI57Qdy*+*3Yd~Qv<HeXshbj(N6RxK|MHe0$7+;p=h*Zf${X|+^}L+knKxiHmnZE*M2N_Fo_9b&i6ERML8v}2!hu(x?sWH@fQWZfUi?H3X0(ijlWM!g?h$&{3^s9Y3y(3Erut<$!ejw6Yslqtt2C{yO56T~O-Q<UV>XD;a`=(-V*wSnkL2nH3E)7nr=re~)n5qW&7R5v=MJGE$yx2h}`P{&qVOMtAwS1+p2)hY~)5K~LHLSZd7eU{@hA4~aVK6Dy=HMg=PZL3&A8X)B+D3Sj!Dl1z#R{7%1&s~N6DlbS+r9_eHs87BxOY3h?M44O3q!5@9Il;<Y{%x&S31TYbwRzLEbVPa;JqohbU~<_@Du8gXEV4_L%Rv@IyyaRfhkV-;mllB8G#X*Z#JIn`s|{5e2i2iO5wh4(OwY_v_2_9!W-1Vrs<dq`R+)QEr<ct#E1x4W`v%ux&&-A@{C?efTBN=SeDzbM5YdI4GxJC@l{*t--83e<x@k&bWi>12zY3$o!@uoVsyk41L;T~0rBCinQ$n%E99*u@)>-d<qigU28$)yqip}~hp|o=P?Gtw*q_xWBg2e7hu?J9B6;~<&`e@ejTGzeTE+G1@e1zoDHpZ}~cdpxz1nkD0?d!&!*Q-MNtmir-|FEfk(tIrbz3I(E23V#$*4*!ST1lLutFm{t3XF9;7rGCW1bux6ikuPObN}<Y>)*)UjlEM~COQG3yinNWl@>cYdla%V-VtF=YmLj$gy!-zV$bnT9Wqr?ASh25F)w=OU1#hPgP%Pn8)0OY)$ZufbJW$?uyt*Ko|jpseR^tfW><|u%K<$T+ZHyuoRz8_1}H64X`8^;Bw!^(yCMM=WxgjF(uPA$)2n66$^Za7!j(i(@=iJ*MR2iDeHNa7i5WH2Ok?uiZJ6rGL4UIIpt&~fb2<xL+ITIc8>v7#FNJ4vqpgX`(zXrlm$8{({fXOJ##>I-+FG$cl;BjlNAKqta(!GhCYn^}x0~)ZH`buFyFQIv{a!~@>`&WBMHWjlSO^ANLqI!7HOG&e9HlczG<j3qfgv6!VP(e<>i1u)>VmSruLK0WBt-_YN*K#s+Vy$;{BNt8_cGidS{Iu#wCcYdHpLmt|1sE<vf(3+L*09YDMqrQSQ&J(4!2p+)*1-`oyFZ*0UNbFN0PxxE?T;>V;|r3(bE2{g4uD@&HfmH29*#s?c}wGc-vX$QA=*(_bZDVDW%_c8VOnX(Zxp2f3?iEuC6n@LF-6unS7@K&8wPe^0Gg1k4!Y@mC^00!7APzAPXY?<N>qWX3VaaH(FeYN|D9;!_foOanNxpRZ`P*Y?InSOihlJN~vu~EpOjAW@KX3xqxG_n_9Wz6ScIhEf<58381Rzv#0fAGc5xq=2E8<RLjLdAQ9=kKbrJM53`T72qiLZ9NlSNwN5+x5oR1L&YZ_Vw<i4uVVoy3A>_r5j*{UH?ANWv>C_qH)jk?$G2agJdN3!xt(^^i^2ZRLL?t;oU8q<sr`E(pY6qOP&hYKMmUJZ}93(EZ9Fc9^9nIagD2Vz+bol;u<-WeZ!D#uk4N6$y->$rve~d_1ZgA5-;As8UApzEQl;4la<b2TSp?%sd7!8NxKKyCSh|pV^3Q>g7lo=3Z+Eh$sB*S}TpbegO58*L|BK=vJ0cmT*o!qZVtb~vMz>I=rMnFB`pLl3zsnV;R^Ldi;hW?CD27Q4EpB>byJMyMel^UiQS8Jr6Xo-$8NYt6F&9N9hP3<5l!+x^1{Z?g3YczlCuGiSS)^@F1b7NQ;HPtNj*{5O-O~kIw-MHqSz83_oTiq6QI5rxqz1g<eTx%wqXVEo-O|w^<IX&z{nE_rI7f4X!4yGK&#+oFo_Q8c(B7WR3dw0C&+SMs9Z*Y}bD|Jgd`*6e<G|c~+aUIqmh7Y0SDuk87gf-%7OkF2=y1d~=-ByKpU&R~~yT?$}G(b@xq+gxucTKk6XL=lumc;g5m@6U{aTUEkE3NDOn0UF@aqskruJj%M9pW6Ob_%2aeRTZBNHuwn<Uvv;@v>PKg+8xwl^^6e_A=RlOaB#vuuD3wO`+`I>m>d8u{n)l<D|Cbt{ko_TOGvtXD4=Awd8S8ER7^xMj(J|vgNEw+s})yyEpVmbKg41U|`)yBy*lRCe=E@6LhTyZ6`gN-0sy?qkmE!DY^?#b>hw=_5T8wD^qp2km)>Fe1CWK8X3GEhJDuzh*?;Bs@gMttxOpjis(UYe-&*Tbv+VYEgt$|9$lMJLONqWE9_urNY_w}@K%}eaGk{Ub1%!+U&N(Zs~e0u6s9jqZ(GROQXb2@9PwIXl~}`c$FA_Y_8k$QHRq}UfN6ADDfI3+7*`HI<Dj_ZsanH3a1SxK3Y7mp^5?BTTl&nG8Oh;oVx8pD4;gDI$#-E@vF-6rpN+@cy*0C&X(F;UO^{JeW(wjvlkv{^d@z{oolmy6_twi4WVY0vEXXWNXP$R4VG#Xw<~%D?#v+6=FODlSjq<EXlg-x>n$=&T5u$=nEstDPSS4)lTd}7r6L|9Y7tG03y)Kp)sTTlKQ*Q(S<<F`oEx(Qxjx4#9tS`n<CA#f1%x>wSTiaWDz_9@%We|;yiy1YbzeG?(gsJSJd?k3Mc-0V;sy$bOP;G5CIbr&L)F4C`&XN34gIFgkn(OHK_Gp^~BK{Y#y&-U3%@^3@PL{;)Or2*YsTY@|-8ww6y&vs85sL%fD+#_MYOo6NUDL@{2GM6cnh$gWsg<G<D<X8DpQ3pmrN!gc8g$TW(*GKw-y0BPJA=(ILUI5o^6rk?!U6PM8tt@@I-ap25<vH&Te`G<`m#Dy(E`bB^Q)F_Y@4>i+>=%9H3bkjAESp<+~bKea>e~eKdf~rX)_RV5jJJaYQ=k%Da#5RcaBy|1LquZ!@J&(Yi#?OJfPWgLt&x4hKLDcnX3hEFJ*hxss+kq(+#{1)|KYsU1REWBKommgX_W6z#d2qTkBDS2)=Fbgf&;A)o<4A^*d}0qGByX#hQqU4&V$ivDM~edn|OaH6_=r4VSJj-&SbCG=Sx8ALJ_w9s8m+5|bs2gIos3eqg&c={oz-4;MI=huQ7>#;_M-8?!yu!q_ZVSGI0bwuQ-lf4qH<iBZ?THs*TQS;XpVYF!G4%U|(vu@WB{B;jI@$Pb+2N?2PD1E{@E2UcrNK`|s32G>UGJ>o7N6mapUU`~r!x;GMA?y_#cV^d(7_EoAL>si?p*wAiZ%OcXY-kyy1Ci7r0@aMzfelS_nwpNX-FsoHDk)@6-YG)R4k*@W7k*A*}b8!TspdC5jms<<Iz-@VP7y>UN!mO?Xs)d)~R8>biz$t@F+~N}Y+ATN%7dLz3*>xX7Os;rj-eRNOzf!-kqr75e=ZQ#sQueqjdQ?~U1a^iPbz@GRXeVZji+I-RclXIhWGxiF!te3QB}^i84eQaMry%jO$9OLS8kr%cizre)SOhS)O|K;VjDxYa&&Pwo?)Lufc`!EVr?Il2o5t6as_S68f50}yKX`C__xnY}GdAt8ErwsbA_v@Rx3339b;EOvt`ADTgO@3R_&ZZ+FwWQ$)xuQ*7=h^sLYT9B?FI&TS<fk65ex=DHB?S)!0DoRy3HBB<ZS6>BoOaDU`*=W2cDs=#zfK_cJ|E~Cfdn?_R{16FCgV9-K9%+e_Qu#|5RkZ6rbJwlbt<}z3BQkgoGVMk-Rz@6o{X^1(025OE~zTEg-HJd|8B<y6MU8JR%a+E)&KP*aQrnV=t(Qz-T+0C63CQvrSM3?3Jf7AK~5AObw3OCtTCA$f<1%P=y{kn_ls&bj>3cEByhC&G<7sOt_4S3=cad8KOg6&-_^!!@8b(pc>)&cp>j>zJiH4kzn@O*{OGaro)K$*NFTqSK<!v$RH$nHyPe+r8g-3d#XLU^c-*Q#t{MqCq){j)RIDw-jYZ*ytCWi#;`E_k5_pqHUiYVOz(b2O1hAd$3*96b&fzxND}dxehU<F3H9RmyH`giN2jxs!>^BCKi7&t)ovJ*bie!18{ocbXY-R!6oIh+{Kx;1*SA-v@9IQ>m0r~@T0aNf5h;t(TJZ(mFURL|YzGJhe?e(5>_O8lh|FnErL%b^_CkT0ztafujGRAb<?Zd(t?~#SJ`_tuzt9~Ttd=T!1+ze9vhsdslm_BTZCtCkCh;cd3};#-0r$$>J+z2Bn<<|c85LApPt?;uqM@YqpOS*Z(9SgP)frvkO<{S!E#!g)vIn&wf+dQVy`O_2#mXzTJDZA$AjI-QHY72m{w2?WVTS>99?nC-f<n=Q^EPg1Ox-p;V0Zt(anFHo_8F~cbn6P#s@uIktK&H!2GZHYrG~1A7vjy-1yffDidoY#huwYfFOvd6!K~^4Fy0j|F1*haVZ{KMPvFWo5`ZLKUeaZCgjxVN`iz2LpkyD(s$@M&-0A?%3X~Ouo|q0Cx}chJ;z@~Jkf$raE_G@p0%B*iE!#7}xq)K?-xO5Ap`lbXgbfqh0-^2!X-K&bJP-t8NCH9f?>_MY<Qd4>Jh0#L$8wcpf)7AXvN%s^ZYzd?TRRmaf_`A=AUrbBt+%8Y(>Y~f0|f$1!GQT1Jc^A(gn8gdeMam*@;aOPjq=!efq+$XgAtI5EK%<g#Ld~)-@PF11lY8ol_fE4AOxCinI%sXPsB;KhiuBi2rsP<i6sC4#1rm*PpdU9sJjnXmg{)oK_duFE@=BRqEpHKL6ph`i#4D<ili2}1PWMY6%5E+(gbGBJZDRisV4pqHj9%<S;tn2Iz&<%4~N%{T%4Horh|*`sTXT=@&5}DSp6v"""


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
        prefix="galactic-mvp022-", dir=root.parent
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
                    "Le patch MVP-022 ne s'applique pas proprement dans le worktree."
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
    parent = root / "backups" / ".mvp022-backup"
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
            "Prépare MVP-022 : sonde légère, lancement de reconnaissance et "
            "révélation immédiate des systèmes à l’arrivée."
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
            print("MVP-022 est déjà appliqué ; aucune modification nécessaire.")
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
            prefix="galactic-mvp022-verify-", dir=root.parent
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

        print("MVP-022 appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=16, SAVE_VERSION=17, "
            "RULESET_SCHEMA_VERSION=5"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
