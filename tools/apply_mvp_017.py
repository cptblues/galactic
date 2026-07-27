#!/usr/bin/env python3
"""Apply Galactic MVP-017 safely from the exact pushed baseline.

This migration adds a ruleset-driven craft catalogue, deterministic per-colony
shipyard queues, inventory, persistence, generic commands/events and a minimal
Bevy shipyard screen. It validates everything in a detached worktree before
writing to the main worktree.
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


MIGRATION = "MVP-017"
BASELINE_SHA = "090556c3976d2ea27112447fbd09ffe8dc251924"
PATCH_SHA256 = "e8225cead45bc09f34305a1b57b276280858609afd21618d314d94f3c43ac6b5"

MODIFIED_BLOBS = {
    "README.md": "54cfa3b297219568491bae8e5326c520cc49ac2c",
    "assets/rulesets/default/economy.ron": "9176426e71b87950f0aed86af69c988e7b4928ae",
    "assets/rulesets/default/manifest.ron": "338087e32651e68a9eb6834045078af7d390899e",
    "crates/galactic_client/src/lib.rs": "9702cc6b6d311e15a060a053faef5f75b07c7778",
    "crates/galactic_client/src/research_ui.rs": "4f944b494617b03731e6be15a7fce601b6197ea1",
    "crates/galactic_persistence/src/lib.rs": "629338a074ea4cb4076ac805d2f72d5cc7a57e68",
    "crates/galactic_sim/src/command.rs": "0762f1fb9c54ea043c39549f53b4896fad85ee97",
    "crates/galactic_sim/src/event.rs": "3949b117eee8e6ec3006ca8b2c221044fe947416",
    "crates/galactic_sim/src/lib.rs": "64026a3665024f6412d6b3ffac25cd5aeb85aa6e",
    "crates/galactic_sim/src/research.rs": "6f398b89a7380d19dfc7991be48f1a4c55ce0893",
    "crates/galactic_sim/src/ruleset.rs": "3c930128f31aadce66f6cb7f67d00846b2d012bc",
    "crates/galactic_sim/src/simulation.rs": "15c62116089d476f0a256a076a9524200f043624",
    "crates/galactic_sim/src/state.rs": "e918edac1104041c50fb5f6c086e1d5acf1eba98",
    "crates/galactic_sim/src/time.rs": "233bf0e00a6443e7f9f8dbb0cf564a9a271b8a7c",
    "docs/mvp_architecture.md": "a5da78739cbbf9213dc8081d02f94311d71b8619",
    "docs/ruleset.md": "7ae8b906753f4f14c11782b9b0a7c42acc8af76c",
}

DEPENDENCY_BLOBS = {
    "tools/apply_mvp_016_b.py": "1557ff3f419abbf6a1b58b897100aa72da80bd38",
}

CREATED_PATHS = (
    "assets/rulesets/default/craftables.ron",
    "crates/galactic_client/src/craft_ui.rs",
    "crates/galactic_sim/src/craft.rs",
)

EXPECTED_PATHS = frozenset((*MODIFIED_BLOBS, *CREATED_PATHS))

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
PATCH_B85 = """c-rl~%W@k@k|?_8S46SxNI(&U0Pqb}v5Q4fRXnX%lPsOCu8~1WAVFpeug(N0u_c<#_Uw<{yFE6m)?Ht_n_2Qt=1Z=-$2&42GeNPsdd7~dn}y7br-z5X!oz)#42Nue{UVvO=+Vj1!IS4l&GCSp>)$KMWDtK~Tbuo1v~k{Sp6~7KcD8p|yVcs>++0~-UpH&6tgWqCwZHq0t#9l#cG(*I-DB|hWZGYh<H<alC({Y*PZ#Mt?y|RoX+L{3zMA!-w11h*<Nkb+#?Zps<_i1W@7T#=6ld{#g{`n>G1QvO<H>^EeuiJuaq`zi%%c808L~K=v;Jk2Ufh0WgLt+`GWIsgpl0?c<xOOd2JtXjjOLHtvS_igKDhlnj?y%`h?~%K25sN|`#fu~!6Lo=9K(-!{9%s&J-_|8d4eOScm2!BbTqw4@XII}!-zBJkPQ>)WMw^`v%z#S!3OXx^f8OlIh)0cD`<e-{y+0HX5;A~878-%GnPe@jHPjZI-WuOl#OEcmw3@!S-Xe94Wsik=@YcG#>$#IR`$h1U0I{C!pi)xNU#N3pJ8&bNckGe)fEdM7L`zAtJBzKYg-!}HI}OXiM_^&WHI~DhKIN9x9pSkwu5gw?%Pd#yMb@lKCvGFU9gAQC;|Y?qXec3pAW_7698lk=twWm#LL6WXfjV==GhE}7me`S&!QKo08pN%3v391d$oW+-^W+iSn7mI5juY#W$`GP#47`H1BMW_pGL!ZbPjKuDU9~KV!@S3{GJvWmoJt(mha~l|JQ7`qW*9=Y|}=DcCH>x7Ne2a#udB4g={riY^~kcY{NRPtkqW582r16(+t4dW$gxkp(}RTFZ>n$tLdNkFBx>%>L|ImocCtw^gLc|*xx777%0Z-=@e$dMz^0YZvUR<3Jv0{pC+?8^}qVl1Zl+x7kH7vwx3VOuy`2}2|xi@e0ceR&C+<BU~3th&Tc<*{G)Aw7(barNe1}ekDG39u)pGqX?opduW-cf*J(EIvf4PFN24z5v|0^-#C1kb?Jam3E@Jq(+p0Si&jC$(1F;W!8IEB9pxfMX%E5qPU3!0AAZpI9HNaIvi)1uNCKmwEIF0`bWD)lREJBTdon0og>nI(p0xgTLa4hZm8>b0HRjzZ5tctDB8^lPsVXUifasZk|=gBC6ouj*@)$PH4i6<BHOX&P68ZBbCa}y|GOz8v7p9V1H5Wf+o#Azv>%u}Fnvni0kX?mW(8qw~_=5d2vPR9s<1z^hz_!iO1QPc-oFjz1^D8QH`9<jw7_FMsqoXH*v(PYy?lg_q6lXhi1+4*O~6Ik%sVw6=z5L(n8e(J+;HG=H6T1%lwAI3Ae?&0tQDDoP&b{s9%sc2G4PC(umjFU+M$SVbz@oaSa_j%0X@p&`?WCKoyymB!?eufeXfFsKmDUzKc^f;u!7UD+RrXhPRg&aGrs>lIo^s8e>XDRI9p#NJ`oq(7%JZR#lBlxafjq%1LUS5}QU89uXu6ymB?aj`1vpLw<-i)?8wzOQOoni)TmPGE`X>74I6s3Uq>NU^iQ<NMHv}svrv#Zf&o6tsJ8c8rj#sL@NMcgBqvCFnZ3BZ#WrF8r2w#noD6=C}iM8j!3gk~-^I%1iMt1^xz$q*=V$+DQm#JX&3?6!7y;%2k6(-~~;4XUk+)ll)mSVeJV+PE_K2Un)HqDhjd0c2nu-(?_Wj-#F_F|CW$08MA!%ZXBMg^mQjTcMK&EG!Xhxkne#2$iOOuRltl%A+jpbJ4E1NSbK|)2vpolAPJ!>PLg~Ew7weu^U6q+}qwHLG#fg_Wbp$^;UbQ3nY4gdQLoGNSMdKxFtaXV5M<90exx#D!@62<J~S$pV49vce`Id=C4)n1CR}qNw>@C@bSR?o+aaM_ale)Ar9{-O{Zyt|Ni4*iju;i6m*XV4fX@Dn8WE9a6>#Cp=Q-!r^q|vi=_WV5NB%#!DhKYhS8&M4YjYReI-Z()q}k}y)a(^Y5@SKL6Ay5^o$)Tkw*ZgH(ktUi+OLHj7CXs7N<R!;JZfd`)rzkW=RG3hHaRH+~{v*?FPWb0hTSGyoMA(^LaD@iiCRV={&-Tyh36ki~&E*2Jr4QM$jbV#fV;>#2JWMeJJ`<@(PC$r{9mJ{daLH<`Qv&a=w_)kqJOIuom`9m<1|Z@Om~IUE>0Z#-TW07P)suBamN_GpRxK;w!FI52q=Jc)d(6nqsL9Hj5_lsCORq-(95B1wx0vk^S=471>-Q-R_Ba_$d*9SR_5v<$FWezY2^*i`n7H!PB$epL@qIo*ezD%aV-_`xZq2Dl%J~*J%IL87JEu_W)R<#YHlqG6{$#j6dTv>=H~Y&~U=}d+h<JrbB{NmpvFS=ImfLtIJXZ@!7123l2n_uT8i6jcAf4vtO&*yt^3<2E7cpPCU+PP$Zo%W)0Mz-%kW`@(o&4&HjY{IJz|QA3zX5<B@237{>!#ntVCx<2(k4#I=H@uLo)-C``?3UJUMbUnlW<R}#CY`PD7~Wk+&;*(GQa{RN_^^`7H?aP<94Aab?4&FvU>A>U%Z))f8YT2Og0!7Gp)Ng}*%R4AP-V5g^5iV_VK(5uoiD7{fKs#4M<!399Omo?IqHn&8N`ST4>8Ke%s`(+TP$yHpFq_+O1fUM9Cm(;*0HJ!yEY@AQ0BZDqq#XX^Vq0IA=)TeJGU)`6?D8`)^LjjX(3Zf)7Zx@**XdVqQW};|MxCmvEG;4G~(S(#$tF!feb_#UOq+{+g{^QdaMx)GKsQi3hOebz3J(nDZ_N`+x8D!q~6JQ125AlZ?bWmFb4JR3pup?wp68r#NG8WB2{fdBZOi}Zc!&r6Ezy)o$3u0W&EOisW6Od(Q&^C%NPvY|h$R39y1O@Q;*)_0qpzldEI{Isa9sLz?DV{9Ge1X2FU3EZ|Lqh&+dT}w5aD;{w-~q9(lIbF|^x+o(80*C;LA++5j-0|Dakrg{1(Ap|zn;bBSPW7I<<skL9=>m|huJ=GSpLl0g16#_Bnh<#4>R0!eP%S#VCrpdzH(SKG$3EZgKvy_`wjL#lKJH~y!<{E-GA(dIRez&ktfsX{P8_4&lv#sWIBqyOLLaY)$db_m_9KH;2#YdN5A=;#G9uA=^8<ta_5I@%A1g*QYh^bjHO$|`QAgTG)-f@E3VutA>cm+;oBAEfOMJS(VP~dHj7edM@VIdmmsX2Vxc${+C`?sG+t6~<tDXObJ(i2H1gjI)ZsqnNzeOaFu&}w*U_lkeH8<nn9OUS>NRurCj18)Sno1M!>p|Fp|-K@FFOMn0qGA2_$RPeyWQwRoYiElHuazy*?}itBj?i;;BB-q8tEnq>wtbWNETU_eNT@k{K;xyE6_l*srZZi@y03qy;2R3cBM7?+wErQ#d%a~HQPH427lZ5x3z_T_qKB#zU0aTC)4?{n3HV;KL7dX<fZ4EGn&0k90A&!Tg{I6os%J)=Nz@`b}ize1U|nGM@<?qY`zC-*y|&P10L5tacS(6i<bPgAGe8Q!T#p(TFsr>v0r|Ev+9GCsfg9u9X~kF1RJuJ5@C<v+TA9gZF_OXBD4W0)uo6=xP%WxgdC#4b*3890-?rcXgqc(f&5fP3qsp|HqA)2?;+>xvRC5q44=9O=h<|$@JjHbcxdeX_NE_>sZAn6#pctQF0>O^lk+KH!??V$JiRe#Md!C5FuQ>UhNJjHZ;+(?P_4_J!t)dLh!(2Fc;W-=^nLFlGGMmn2gQ96uvUAQXqO;iE!qHkdt1LX5!<4_9om&Un`K1owNfa%-5-;|Aa+6f!)SUQjsAR$md%=Fm2sOgWV>$HqbC&5K}94`va3iRQz)9FjwY?Rj3Yq38kRF@3&)k|yb=kmKrP$LP)oT;yB#8zyWSlTB1<75qqFNDLLs*MFy2F}PrW9Dx(iSf?r2Hw!EXHLg1q2UJM?#B^Iibo{uKdlXbsD=Ndxt(NY3V>xG<<B0{eio>9DL4x_&Z!ZxjYi|6<Zh(1g}y2l#MIzq^MqQV^qLpZI}I`JH9SEyYcvh_G<r<wy&ET}|-%@!-YT@zDu8eRXhleDJISF7Mu5=31VJ&{6<vd*5Z))USiUrtVycZe(K+4}=l2#-ZI{tN-zT|LbbQCu=*V5=N@l^3DHW|DV5k!|1YH(|N#%&hj5tpT_AJ<a3bA*&}xQKl_)_?9FNjwD$N+tC9Jyz1_+!X*JP0KZ#P^Tl6!E&hbF~0ciTx0aZ6YaPeJGmEi^+;r_!k_zyLDtgO6u?xMW!TDtqzKa=kMmqj*DhSxoDUfyN@oxeB~&w1*)JUza5I|11ST6^@DWT(B|-%iBlBzKb}e=h%*+X*5Q@qcTh0BzO#7izaPKibL5R@H<SOK$v!h%G@r=#^)-T3;a0_{5aGFA`H=*hT6O9m9;0Y!2XjU%<SC_XJwAOG@&iJhxD2Ij#Pyt>*hY0{h6iL((_e9(!z|y<^Wq9+#!Zm1*&|r}AA)969sGPJY&2nrqTqm)5#Y95rjG7FD%qs6jGqw-4QbL|qqc5hX0V#TS0cWlGcsX9v$-{_xWgdve5{9(;dt3~!EKzQ{?|rIyTMiJH{%Et|nu)C<pD6@Ig1;yd%?C!A$_Q%G7keZ!H-+FF7P6zwq4O1Y#KIPCeSK0VpGlK{5&U;pr=wX2U!TAi)mS}Niu#|`?+<+aV-JDi?~V{N?`d{xY|QDZ0G{IeXo{Aw0M`n%H!AF#M*yamvySLWbaGPqYBUTA@!kLOXAWSSH#%j3Og8vps2yid0I&%ot5+F1LDzbfVQr?;O+97dCvEhcPw{+D>(ye~elb`ufM-TbPBcv-2*tTJhL%w$hoPdaOFa-ccuA0$Iw?p$UJT#8FqHTEvy>(H^-b?8*^t}OeDxAB+TPVQzuxswUDL~PhF>=vz%)KHx!Sgi{QdYdHZ4*lKP4Kpj7^Sf?qZWY(t$Wx@`bg}L_=~iR>RhYj+zz+WV%b(7UPT1|gvS$bE>G89p)eufqa=R#1TK4$C)^-qe9g36pP%AXi-zeYNkl{wz1%6dEe{*ROp!Huu1lax0DFF~tSo?|Il=6SvFzo1z{q*Aa?Dl`2vM0x<uU@`5{{Gp~=|5cArx90WzW2&wUH)2d&|DcqHw(DieRG;NNm)Wo-3Yn&l@aBOXmJsHX)=8&CdozX5n7Xi70Itkm36$g1u9*n&_b0gmfQ?m+gvm#W+JBh?ozyCiT#gvDzB13Tqg6kmyr28h6bHKp78NUA4#wnp#MISnAf}b`g|ItgDyLXvu~u={cJJ+<_|GQjDvVzT?A40@_5<&B|upcIqM>!QqQ3w>++dVWUTJM=%T)Bh-MvmOHb8sZtCruhqTF1O|OYUF<l?a;z3PJt=okkKU?O{MbgAOChS}GmH7GC`Y5kuH2Wjywq_y?^d@V&_SEZ1Ux1(APpLiNAZi#uZR(%39Cm6@$>Gr*n8&#Y0R)RsnH&8xjpvJWqOTe$fXOL5cu)qrqpTmz;sEr_u5SXUC@yi;^y)e&hU@!Ji)92a?MIk(i|(zMSc!J|T4LTx5qI^_M*qNU(gZCm&DX{^_SLtx7vn3wJTB^?4wy_94i*^4hta%$$-a)PJ3pmv<LujgAHOQxJk>T)`DVJmO|{(Ss=CQMo6YkWa+5*GI=iUc8g2U*1s11&-B&xWBETAKo-X1Xrz{<hc1XI<u@EqpA56|vA4XnzJrE$Tir)!?%SUCI5pYC2B8MCShYbgchC+i0C>2}-1S0FdU|p-izYB?~MZxIuHBl^M!2oM101dyHZ9-Yl8D7hQzKXd3dw>UN08k8S1d%}<Nx|uK9E<0sy5R{6`7N%6d-F4}z9WpJr$Ql^OKe0)ke!g=+w>lFdawX@Duo~Ts<!&^hDCn?)${f<yZzs66j9`j=4!*}yNN-KN>Ijy30_l!830)N@>ktUH3?LzOeJX9?c5o3MyJce%};9)n!w}66anr({`G$`cwRtvx1VcSoeh#~Hk~BU4?Hzj1KNNC_XgABOeP57X?m@KD)}^p<reBB{tNBkS}J}NER!s9KdTf#$fr8MB=D}`0>fLeE<kcs1}%v(Knm$AdPMZDUNZL{ZMB;sS`-KgAl6$@nxO24Zc`vw@vFW>`}8FcaVJ;Z#n?A1;$&3C+5<~%5>yl)>-(-?hF|nBiN2|Um_Yy9DD-WCv|?Ubq2D9JN^`?vU};5F_&pb+h8C%nD4Ein<gDvz%mf2hgv7##W8Qmi?ntnOW~*?1rf+axT;f}8&ZiiN1vo+OgvmQ&YQ%p9u%M~OmGMplm#F<pdhDH$4M6&p2d1uG_GOaphyo9ALClVjP<v2P{VoW`nS46UW0KF|KES6H((!>d$I+|@<O%U6z$HF?V)=UvCgKD*!zbRuCrvBeRs_#MO34R9oO^jD;lHVLuK+k$^m!;lijPQoUwZrhjwqJlGO#ExLl*ODy^$jzS$*O$-Dc;8QFJQc(2p_(@ccK%W04^TdIap8ze44ewq9m%^^6L!NAk~VqqyhNuvj%d$$CjU0`A6%QF9beYGu`&SYEY?-N-0SSu?tdk`ancwR)360)9yYB8=ddG=Z@P3+gXn*<EgXJXKQ6zTOPB(lwpr;k%?XV#X0#nlqFME}~t<6!f4TI1W(8Bp#TdN{rXopko(zKfkm>nbpmqkQ=?ro@@s!gGm`bsiQkHt@~j;9F7fUnac>kMl?5k7KeSBIf)gp+;?F}hKer^%eCtn7rkW(17&q*oGv@)!=u{OWJPPVCgzJVlC+=t+L0%gvZcCfpCy>kf@*%ue^Hxy_I%OZ$(QSJRL`%!VFOZ4a+y=D)zBP7d{N3(Q`XA0ZZ@wrx}VE{CipKC#V!E-viTG!MAyR>T!k<ARmSvf(ztT;#G@0M115+|>DZZ>gGxDR$m1E&5nf(p+CkxQkylahip0WM^1kU_MN!yGWo+818?gg8`G}!K1M5ODbRBW_zl4%eTu<UchC{Baq#K`LThu<O51-8FptS(yYhq+7<Z4XSYnVu6`2sH{?^C|>j?%QI!<=F?gHs^}9g2X<mNuaHJ4}X#(@e77^j*y?!*N5B2!SfxK46Te!Rwn91sS8w0M)rVz(Hrg_~pA=kBqQB|D%AYinHV<39OhiTkub!{7sSe;-!{%S7e?DP!$XE!k<)#EgqYgrX__Qpi4<nhVN9__{?>yG)mY#8j%7TLy`NJ=d>hf7|0X7?M>~zx3%kQ?9OJZ&<WiVB8NE#M{w}O!mTMdiUW?y(t|gKZnfq@Oo8hyzY$Nccz}$qY}x+MO~$nPnJymtzk!@8t4n(4(bt#K%&pk$WNNvi9j3t0M^qtb=L=B)cZ1-pDY~+nwaFGIgWmiSouV9x&{U{7q-z#a>rZD%JTUEi#7BMcw*9=YrDGpRTkZs1t0bUg8|M76K$9G-U@n%oRE^K@YsTHC04&jXhJ7@|8_#y~p5mEXiN@gzpN+eSyQCW1NFg)=T%7FalLGHSflEg1Q-R9J_&Bs{#D6)5ybrO)D;k6%fje4(aWtAgl@b2D0_O>=-m_>mEYCN65z&-<!nY>EGK;+qB@&ErmYWm70+BGeoJEv)F%q_FGul0ghx<IxF0HYNS``lWT$PZ^dpzMtMly~pYivz_MiQd{Jf^t~|H&+!vq+j9L%(+1f8ll5QyuG@uEu1{_M7jL$-tF%%+uUSa@FKEVA-fRN3>d@j4G!7WfL}vUf%6~^E&Q-)7!Tqus8oQO(r#He$@o6nIw)2T!!Kr%F=?ed_10krfu!2)rdB|=PV^Jb@`T&Mj7hUDdsMyUR2${Cx3Q=+SmGZom<+Sb_(I>z|tmH#N~|?H6esG%ao$2Dr_Dh0LI9}RrR@vG|zaX{5SPGVu+p)8G1nI!`WQL{jYxUEh7OESAH>@0b`3Y+i+T74hdHZMWi+wGZHcdRJu?j0sGG(rdH$!0%ObtI3i0BJV7^^`52|=i!_?d3(P{8tw~tebFZb{Jomw|hF2K!6UqXK>FF@vkJnZe%vw>)WgqEwhv{@o89;oo`i<e*8KtN~&YQhtCad48gq4i%bsWB;_cyEFN@pg~;??*ej!De|b%nJ8b67$Jb06voZxs56O$&T)A;+by(ZmFtAUFJC10aNa#m8LgXbU(FV8h6F#^pPtqA@kS$y`{JvElZnU+w)GKk;?1*?jKw?Bw9==!fIO-r4ctA5MF(j!t@~M~5$8JPDl2JYwx0Ci-cW+;uOQyg+A6$T-$MB%b@Hcf@H9C2Afc=kvGqdc^^+o&f3q>yHGHP;Npp-&oVhpJq1nCM>soDHg4M|Lo=A!4F44mmA?};~S0Ew`dTY4gAXlnVT&iYL!ta2%t;Y2*cemg;L>_0ZncKOi0Y?u~Mi#N}ONZz{W0NJ8-e3IG2$;kws0l<w|-$^Le&b%+i-n>`FNvDSa-CCdCwhV$rkaYMB+?y}KQod3cQ@XsadjFoYIlw#Y7P0pd@W!(JahIr^6g9$fIh48VmMDUq<2d$1u88}2=T@ENn#M{`rgCE@UgMX9zTP9T{Y<qK;ta*QO?g9jhzV+N7~<}8R)())bkd%3z%;(rNv#q*c>&_)?3Vhq#8wFIaj=O3s+ixyAVEW+ei==VWVvvO+U39JkJk;<)@_s8s&O_0E(Cfa*HP2chLM1?20>h<GfRO2%bxg*po7*H#ryn^D<3wHSOr;}5}cvPL>zujj0hW$6%0zii60MeL9v%dwtwrjZrm(@=AEaly?8`ZuR2rR5RjgXJOfM}(gW5GrDfDFhtfal=s?C8bWkp$3c!<cf;QMkbDf3pll5+sP+X&gF)DRD9Hsb@_pI>S^WOg2BG!P2mfU%Uo*92}q62SBDmtd5f{lHp0-<0kEC4wDh6Z#}NE$(mlXLjHK$bBG-zN;Oq|pS66+fw)oBuPYYYT!h6zok-W0urz26t|t><+{UU1J0WMN7DMzckcZ=#Vw38)?%3W-#2Qyh>Kdjjj}*g|0xD?^#XKGTY4UC|eLvySkUWMw6vj|sx9N0oMd_FVN~?X+I)62auH*FO`$;@-8=FwlVP##d(x4oiT=wESD_#fhWP*9n7cr}Sys4Xb)qEFUTh{g){}{+t`gAcGv5&Z?lks8<s|&sB?$CR0Hi}{GpchdFIt$I}d6H!?>+hw>?xR0xaW2_0Xpd>m);gr)H&eRH0|s|8?w2KugPzT5>_b`Jn+Xg$3K<Via1L=gXCJYYv?R)z-Ebj;G)eZ6CU&!0zCZ$;O4Yuu8CJ6*G|{6$g5$Tq6&T=dLYu&KH{iR#71p^mpvo>do@9&RFzI8CS$RB?0X7;Zn`n=*vC?66C)%Af;mu_za%2Q@B4*tC6dh2}Zqz5^l6>W|K-IIf8R-|wTBKi*orswxCflx5W=yhSAqeNot2iBwrtcM%k=7D6c*v=11UN+*@1rcMNN?~`YkI{cP|+?b(efQ>REA@2GM-H{y20RU+&Hta&ZNG180{OkJsz0HTb_&M4<1I&u01u+c!=tYM1}dHhLWrHf#-~@;H;9Ib<C42et0D==oqJO2CvGy=X=lc8umOxleCWfsC!h|awB?dT-dJWlH{K%w&tCk7N@daHXX-3JbiOdbcf>JFep{Lr5Q2Ao_H^sDLH}nUU%Z{*^&+N)k3R0(6PIY(e>KWZj1vnl!E8t>X$5b0|0`Vz+;}8(^`8uVQcx#pr;%WmIe_D!-YV~WB<|+t16Mo5eEcPKW1RQp2roOIu<F0iCw~JPwx<3=#w!%f@4!=+tPFXTKbMi=aGP$Udz;FFnuUDN{Hn*CoRU^>|e(Hck!T?EzWE0y3v#x6zO4Xowb9fc{K?HP%P{0zcIV2eMII5wstyzYQNyX-_&&l&oyXEf)6`Mi(Is(cT(4KfG0{%eqV-DkI$i&uRp62Kh8!Ht}d7{6^vHBu@%1E&vnz*fn-~ovY$qvogb&L+TM1=KzZtbjgn??19;l-N7@az6co9gOJY=U0m@?Q?RJ9rStU5HG5$d^9J2NGi)7BCN0f>)d!$Hfe}od_qb%(|8YSn=G-K!GWmdS|WpAg|>G!vr&3LoZ+1S`d<Iwiz=F0l|dX=75*4EZ4_4eI&Y`xvsVr%Wj4*TxAm9_D7AavbD@_2>eqmhRm-mI*lIZ_r8cZ_xRc!pmCnd_}G*?iFLeiX!*!j*J?jIzt;(F}BfWQ?w?!t{89z2ZPOcN^PmZL38?Ww|8vG&z-{UTZ{{QhY|Qvzq2~->1poBId>GfzoEG?BbH{Bu$!Ymb0McHkQkH4|)$swe$d_LTEkkKU%d`i@?#wBzgo86<KhR6eM}HFQ`#@%W+eLLvOI35*ZreBqoIuGt@UwmFbI9kTgaYfRj}@HoH|pv~?^KJ1ebQ$-)`lf_mjvT+3TkbuCW@T(d6>M8#HwB~hfhBzSIbbsBpB&+V=4Mtf5LRD=h*f~ofs3_?}oRe4Mv0*kthi|`SkLe)0T=KBp@dd4Gc_DF<Lbwu1v9(MzPEqztl9fAd|n_%P~uz`)3^N2+@03sEj%Bv`i#;5VzydhS#C0RB{9IR64R?83**98f;<JTAhr|MG#pOK?%&RQV)7}0!4x}r{VCEIpMqdQ*q(<ig0u|1>|CwCno$wp^)8#moXXK#ydI+37RotGR&c+SM*l|6jFFIyZ>f#zWbR-ku$X)tg2LBGEsdK3?yV}Vopu9n|kda4mxdzN2&EM8^RX!^QJaPbHTOnYeSQ=>l3xh5dAw+)b4+t_GrE3y=<q0D11-x0}z0+S3)D0R9G5~tTqv@O97gs&i43U(b>q{j-SXsb5eFiNo2{DT+cmaV2<6U3U8@CTTA2k~dKv(ag^cLjJT1`WULP@~^n9mK=7D%;0eX$&Gcj0Zr{{r!y<O9P%)8!mwrku<bTt-<W@8b%FM)+-5z6SjKwxr-|fPKKc2W?=VTn=iyoL%!#}1fU>X%r65bsw%;0s*O_xcd7CFe4gQY!pc;R;Zii{e4v(ULl*0<mwnB<?8ac-RX2yHr3_YCyfw&7shDe*72@vZaBp+-e6!iy+1nj<wzn#ayH0DRVy{!43sQ|;6r&on9Z@v3jUyk0yt<ghDLqL&AxDQB%oG@ZBBBSZwBdEH&&S;6rrrFFoQD5qO(W=&E<^z#TT`4wIVcebs}Vl>m%)n7%SBfBV$r%8P*ie~q7MChrDz~Bw@&eBoPjXX78r@B?BwY5=-}k=N11iG%aV-_aF%wv^_bIRP$t?PpvU-MN9I$gVgx8<l>?MAPq>ym1HOxxr-U}mxIM98n3DA?#T-4eR0Chw#NuFc{xFGm2x@3BQ&qrlGzhU*TFmV|;;lPcA99ycMOR?U2KfBV*E1j8Q+6K;@TpN?sQ{l`)8=Bz?p8%mmPR9Fh4D1mmq)=YL>vzSc^++m=AvMJEar#69<l@!_(?x5w)yxA3QoA)+1uFIjhbjM>$G>zi#cJSDL)$w6y_$N_72*B+GGS;fAokwfBkB`)!ttJzRN^CCe;4EJSX75Vt^OI(#+Du2;R<FGKfj=X|AlPrgq925-g&)J)1udrsx&rFlD@YbTo<6i|Zd!yyozfqc*;NH3HT#$6s=1KZ^$!n9JCHJ~D)1`NcS!)WuIxMg;y&h8t}6r%6Kg^TX+6m|W;*{B{vf$OW;t5HAI{@`)V8Vx@<|GrY=^N##%Q%|io-JnFWb=^v7bp0+5BvZgrUq^3C}{xF%oAK{E8Xym+JrBjTFgaXZU!238M4>U^r?0G35wr*}C(Y3N|oJHw{Ee)B!X|lW-Lz_m~NHlA|bW5HkdLfKyoh2~uS&YE}q_c%UD$(jp%>_u?5ODF*x}UJ@5)jo^hveefvpl@@&>DTw^6;jEYxL#I!#i-+=u4M}H|nlY!SZlHZ+*5n7sB4@!Rw>m>!Xv?<CibGY#{_a;1(rCIwUau?iYX%UI2a)pD$qJ9^#eb2AadrTjc1k^*1XF8<O^(=qkqb68v(9OtJ8nbO@uIz!xYjxg379W+hgqhaMO(sP=ZlTTn)_rRlrnOp40=9lIXv6aqDt*N%~06GwFkrNwq#oYuXn+c#ZzpXf3&kQchWBF=ULsI1J|yDhS6g8-sniH7Y-Gy^#tq<r*3Hm>tpr`o2|0-8<qB6$Ujf@KgV$G#HeDkFWG8>KBf>y~fWDiCPTx4zO3HuotIjm)-}JB=95w8gazyvTW^s+!(87}z3esyBfedE9ZL!^sbx`Zsk#r_SyMqElyg$3iD``JSe+z$SrPwG8kDu}Gi|;gGn!Y!OBH&Mz<f5X*-$jg?;&L+%SZ?v5FE!;0lF;;z^rFkz#;M|SDX7RLlU*#z;J;#7$9Ag^p`OLUJtB~6jdlo_}j_QS#Rqu%M+!I|Pb4Ko-3@IYmbJvO8i*Co_UP-V4mUAbSbGwYsgpjT;GH$WsTVPA6-%Em5->)v)_tCIt)G=9^M-~EE`-*8AFQAY^`-MD(V^Lhwo)IodX2hkf}&3d4L^v<;lj&r>XYiet2wn(qgS%j=ay?L6<^0&ZjjUT;9H2a|IW%g=!yD$hlO@kGk#ped4U7`4yMgGRP;}Rvjpwevatq^~hVYYzP%jr0lJ;xtFjmlQt;BRKo6D!(*+Z%{hJ&F%dGjR!2-Ip&;&rW_i#O&a`L%a}j(qNr@H@e|qQ_qji4xaU%AHO*Aw(z9`J^A?*+dul#*~!7-*~>c*^Yq8#S3e(|JZZ4Dr+!RoB)^&;isHb_^e#Zx><y<W?<K3bBPa!dig@ZGOd1c8dCk%|Ij++E*#<NJQx9Q}Y@PAu*f4SM$91yfxTuc2*td4WmJ%WEhN&Zv+Ak)V0U-0(#l0s-XGervZ~Qw-_*n*=UD4gE^`@B&{G;Z^a2b`QVz`x1s1<)mlWzCf@ee<q^<JI4{QjunB!Tt~X>b)y`k+$Scj&UXpVitOBeJOSu0=iJ+pw@}WVUc2C?pyLpkKIofQ{D`LU3CPb(myydIim*C$&AQD9hq>-i!bGRZaGw<LBg8${dwZHJ22M8uDCT$hnZ(bkp<*_LNa`v#F%7HO)mlUwzlf+Mb}B>h_pqY->v}ww;<~a;N3L3uj_?N1&1eJ}66eLb0d(#>{*nVaROlY<Gt3VYAuU>JK(|Hj53Jo@V?;Oiw{>z(l1H{(Sdc$VRBGgW3!z4R_p@AROC}pnbH3EL%oXRtxl04Og-xvv3Px<HO-}c5Rp-8cdrZ@IJFC*$F#Dn@&LBZ@VS69ZyL@7b1$1jGLIK-2=LgxM&5J@)K3HKfBr*Ykc)biW0j?bnxiWBj|TLo2D3%{wf+J1HLE3lC#BRo{ZT1GDaopJZ59OfCS%1)9DO}Su_{`mEw&NKo2+_mdTB@{Rxc@)wTc~E);J7QDg-+D-S3?#z=1h>&2en|C-J7&c^2MW<;Ce(ICEhG+B&BWf(6zz=du#T4b+p?e3D>rfFsu#<TSSqSORc3hvYZ)G+C*%l5+BPBbuUay%xVOv@$n<M(H29FtQf{i5US*Kz*|I=D&nr)tCToH$3><}>W*rXl`tn$7)Yc-z>n`i#>7g*$~Gusf2!#j1C?8*K~jq0ng5i=*KXrrB`SDbv7fSk1Z3GZ^H=Fm*mr2kCnEsLnn5SI#cI#^c3mv*q)HKlM(2dUkYrbk;jOIe2<@fEj(eK<8lEWWwB~8j6fq^y3L{-ytOG@ns6Lra$O@3-o2pG+*jnT6^#i*&1TdJmo?HdEgpFBq7ivH{_)*TAGK**x*04)e&Zc?a>BSa;RJH@ZjW!m)?5)D7~0^>K(p(_VUHgJ(TL5dV(+^_8TIOt_fV5GfSFd?Olu!OUxOQK@nRts2w#8MeB|{U^R>PTr((0EtpN3En$IX%Z>Uf#?&X4&&R}F!7S4hR29rdpqVmGtr_ut5^}-gCuzeZPHPWv;1nA)PSe__TK!YL5xVe)b&plO`tKiG;f)9e-%meuyQ4UIhl6Y;ppf^@ryuaH$UFrprCzsv{t0n_VBr1*`pn(8^LT{wAEV?-GvtmUb4)-^x0|T`Q~C0nhdmxkkY01pm=I94n*;2!H(!9RQd>28U9E!_9*+jRKa8)5w@|aoT6pkDh8L0kvBQ6#Cft>>CWdK`uDU3JltB?sx$j9@P;)sH0ode7Ug35XWa|uFHGxS>=O0cu#RJfsc%l+TKF7FOQ7S*4E@q<MO`eLlU0Pd-yGWL9CVr15b2h8ouy}h0#Rj}xdO8KZ?0U@cLTY|gP(Aqz>)ev!x_w40z+YGZhe|)W{Y(%3k`i!Kls}Nuy|1i*s+iVdp2Sx7ZopgxQ%>oDXN`K5C{=afa^hYAxbw=u<y5>Jtb@2JRLA0>9H<%py&BGR`780?Llvj7gbSMmz5D0?u%ha$$Op_{g`ztayF7a`N8(yD9F~>)xFiGnhfVta*jVw7F}DFJrS2?^BX0`I2U7L+X5(H)Ar*^PMs!+pJQ(M<YpWmnBbkw5FuNsVDIEne)h9Z<nGYV88jfEu(uOVm8Xt{7s=+cca%(rw*v(R)^L1?vP=EBq43%r6eG7Qt5+ll3zK9iK(f#ZX2i-*t_nY7v?i&q+f)Mj2Ol@!90*?+RYILUYa{oS7T~JTC<iZL`elKDybmPJp07ry<t(Xd|K?e_E2l&RaYI!hhKPcCkgMsnZNKdc3ti`k?k)<=&hVZR3l3M6<<Oi<0Z{~B-Azz_jUO5-n)0X6C6f^(_)D!?_Ux7yM%0}W&xXO)VbGDx*Lb!+ruX!UiyUn^M_krEHGObym3-ESZ?mNGwzaKCwEH`q&ll2l!7ZYz#dkQDi4UNX*QXt|ubpyk6L*6?8${+hC0}Nat(bx_qYL=rtAtR2yPv)1szKA1K3k=?kOsSJ@p?7tzk}o<G9ei0QP3D%+F|(3^bf>UnA9;Ok0E<7tQx|WQ1{A2v=qdR)nB|N81cVZd>lN#OU>KUEIJXVI)kQJfl0oSz88oMGwQ_K-4rYMTFAq%kym=gs=WkPWGt!^u+^8(MfDVF!QmHR6eZ`Ce=13_X2Bi$3yh-xF8{hQKuSutSU?m2?OdUl*LCf*L383baZ-VJP^$CDCzlF~|$d-}hAu6}iX=%T>E$(t<K_RCblnkd{RN$r_xR!7ecn|7IsgUUV_b!<f!A*#b`AiUjIyb&ptAX=Z5j98fMjkxJ#~t@;IpSm`khorIQ&mgz@at?VE0CQfKmh)Dch@~_EiIh<TAM3$%6DJe>o?w8#>)M}5Zp=UQH0d~tRBlCvYG1RV=;-zYr|g0UY~aC$RTF_koxZnmQ1M9ZibQHo(Org$}*;I)!=EneQPG8V~rQ1+Cn8sC`gOoo8a^v#?uPR*;>sa$;&sxG#<h}ob=<KimzT>E(xKW5wSRst=%T9+(@jYk-}6*&6rMNi;&Ykv+Z_eh)=pgot4o=JDxoKDB_7Kj4SbgyEWa+qB-FVK;6a&Pm-o0;#c&I(nezJt;Q0G&wZ;q9Sv%&9-2lN4b&KXG|1(m;#ih5y&$6HcUQ?yak2HDHbp4Wj?#}jF6=&zSztk{R3HCwV;VBi-URQ%lWMN@4<1O{GrJ=!_yWH2Sp_4{8`e0dlQ8D!?v45dm=+q|6I~le84Bnh8oV|W>H+m_J7NB=1KJFY;uyS^_$B@d3L51Zh}<~O24)9KI<`eUD(6#ZP+qzmC>)Z5gP_$ChNp;28VVJ2;p{AYDclX9?25L@s^KvM1QTGHLo}s2(Q{KC8r8HvwR83(N)~!zh>`K546^g#6+rjsOO*071WjNle_fqSoX2>3em+gVfgfYrjI+<y*u!3~2W}D(5j^c2qi%l&h|2nX;4{V+_f3){Mcu_uX5u>?ayL98c_ifb>f>jOB?OfEy@(BaRmg!|V!*&O`>2yZHeSFIlfy3>gU~n7zCyklm<{$_J`EY)xe|m;ZbQwxbIaqDQYHCFxbu;8Iue~*XjRas13zdh^ISr4^V*uxigiW`Cs;qml=XW%8MgdrhS$}4(ybttlj8hc=9?8q#~-b1(ilt&4TYpKau4-&v9G(UPUdN=>;rS-`qDu-m1++kKXk2$MhYOnAQ!56bE!K9BS}g!5XlW52dmT`b{`gec3V`L5FKxg;x2x9=t(1L>FH#lHWT&v&7K~S#~o}=toMT;H^y%tmK5O&oR^wI%k1r?@vzKFZI=GDG~9x=t)(GX3265*9j%+Yld<d=6((Sr1%8DYaB}eG@oG^5aAfaNNDvgK;8@T+<imrJ;v>GhkOw<IE|*fX-rwSM>Qk^keJV+pNm;_m>Uov2O7$#h)l+BbqoTZOX3<CAp;2DRL{1Srh>`B-`LiLj`el^$l*hY2fYANXL|F4Yohr=lRtbn!Y=t6QhatgO&@NEy^chQzfCr6rp1pnJ1;$r?&`BLUZwa~^nAEZ4vS%s(RPb`Lh*z|oJ+6v{`#M=TW=0zo;nII$thDICr*FBTGS0qIils0{HcCy7tl7->dXr-UUs*jaO-IFfkm}q3N3Ap!p%=#0NpHb{T`}=RimEGriL8VG4Cj{TWW-F;{)Gf&`D3>`Ah&t)W{mv{5IeYn&WTj{s@+{EL{B8=wCBq?U$v8~JFI~os&z<}+y~P$A*I}Ltdp}Z#M1v%Q1Rymefh3db3L(zHN=xj)ACjic@#R==+LsJe1U59P#?bJmL4-!uJgP(%QNn#H+7~xlw6upGE36MzTm|0401ZYmQxhbP)5l(ncs`!@=J1o`{;xV3*Fl-_2f<UjZeB5Y~KJ~%AGgT0rQu4`gOcg^Rtw8VsH9lZ8P59%-P>OLw6&Y=p!}EpG39%v#I6@@4r>zTTQyCEl11~Vt3UQ-fuK0JdV3oRe`V~rFJD_G;h~$gU+neX4fXlt+z%mmI=1F-C;VN%tH;M$EIQbH#eHwj+v6mHQ7oRTiq09C<@M0dF1A;7Gw`0qo}Zxa-VEH&w8j;vW;MrU7nmg7nK4VpU!$59I6MNMJcxdoW>pO)$N4r^ONaho#trXD@SpgMRGOy)K>2szg1*HITf+*$<5pEwhmj^gYCJ7#A2&N%jE|JKH8v%;~obnJG_;a&5v;~vJ{#q+jlj0p7p>Plpd9{pgtcEeo$1iQZsOv)IYJ>*T(y=)dvr%M}vWh=>e|7EV`Ww;WeKEcf}Yd(1tTgPeyF_;6u0`;7HU+tDQI*`M&$nWHRMRb1(&}9Y@Joa@QZb!6#sB)_Y*se_TxMt3}~39QVP_OW8Cx-u7(g7`ITI&66?lzQE<4Fcbe$Xkl8B>fp*zWaFEen?wX;xSv6)3?}!utf5;_`HUs^2yL0w<R0gdOTB*gk?Q;IM`o6GJ1e^!qjWBD@~pd`b6L|(E@4K`;aSbitJUJli^L<-Q==ejx!+N~VP>S{TUOjBbnfJ+cTQJEMsL$YSVO^0v@Jda@?{&5!DMRct`kVzy!#cPKsTxrHxac5%4R-kPiUF?ec%nnhp~|7Y&v}&O|FSlWcE1{hqtl1rOPgmiPNk?loBNsU<wQWIZmglq_2Rqz+htYj};?0;Xlqj<AHba3?LLQEp~k(koJE7o4Ml<G?_EOuU<MGzDlPTm>OlN&I}#S?a$bCp)UCm<R9h1+pjvy0s>On*{@DvmKsesLYdWxGMG7JKpP+G_Hate87~oUp&EB|wvzvr8$0fApA?h8oXHFL`&1nqh(weR+#pwB<{VrY^Eh1#qz!|y?&lJTUx!kv`-421Ucl<stjVN}T=Bnm@_FlQ;$Qhwu6wdKD<iDv%;EV)Z}~*uf}pc+^VYrG$jtcK;$rT%s)6!h8FGZGb(6CMs0&TnAXzuBJs2iS+Ziu+n<+4ioXDe_P=gWw{qo~>oDJ;`oPrzZmk0SRJSYX=^XlLD4oXX|Kl`sh`V8^Q9b#$%TIuc{O`zlswDF%WKas<e4Zd|$7EC<Z)G>dHj^T=dXvmpnBSqdmhyv<*i#%QJdDTv_RgIk|D>|oB!Hw^Vxt<-bBwCG<DxPyweKfcKSL$uEA7SP`py4&gW2jzXc%{O&C2;35S)!Ef^y&+2Y)o~QPPsy7_zUGCTKfa+3frP-T@p8oYM<!SC+6e;#YOTNbDDEdR_WSGF5Q5~2-IMolqWtD9es3Dkn-=Ag@_NvF+|i&@R(V?G3fjyP;k}PCx5G9$E&)5A9*mN6?sAz`75D?D;szyvfNGbqM&6)vP1!24A-2t7RDp5$*9<It>HTE=B!ElwJB1$2BqR2nuFx3*N>A?O<X;)2J9H0!iy$_GUr?`o^aC;fy<G&&;<JHNL{ET2E{HC{v_KMqF}}Yo$7@)w%hL?R~FeQTKs*9Z8G(@7NdO>#uG2U$!o<mnj0sf##b3%%p8m%o?HFPC?NiWlJFkTJvTx+LD|0GO%!j5&^c(U2#00EUi5uzqM;>~(gBq^VR9*kX-W-|4EOGjZ8QxD!hww?g6j7~N9O+Gufzs31NrjLf-2+#_1arWhhvhm&AvA<v9-7ECX8q&S7;S$3$JJA@+b|wNq=i5kC4^;f()z?#jqh?AFoP4E<(!AEJEIxu0iiFF^;w9X5a3Gq;QHaM|C~XR!HOEM}KuK7n$p99@yF$-t*FQ8b|M3&oz(JZ#DzI2fguro8F`RdUKpiYJ6I*3yY}d-(b33|M#YSakfT-aeENpT>qAj)%jX09>BgavmAk_H&FoL;eEaqfVc0}*~>=@#CJCOM$5(YEkPevD48rw-|3rtz7&gW=p*BNK`k6(Q1@i6MJ<C+T37sXa-J5@jI%Yr#AY=k)3g&=^B=Y3XqnDxRgfse9gJVV?E1&(l=hv-ge6_4=XyYbSEO43!6B`cXcv%IgS5J*-Z^I_ga0Ze-Fj=?qR1%*6msB`E+az=81k4udcg7#11d%dD2faaYQpb67mg7Suxc2SCYwesaDe;l3lLx_EoC?Vi}7Pdu%v17xENs+8EO=%#vxghQKhf(Z+S2c_J}7y!oan8v*%1$_It$eYaSOH9HPDpj79>nOZ_)$JnXjJY}v}P*p>l8E~fXWpd((S-t3Q}@vH_@YhWiFG8-Lxo+_#?pD@p(Q^n3}uazSTsjE@?kjpmYs*B+sQ{v-$Gl*8Phw2e_yTJoE!>(Q;bvkErg%p*ttNVOocw9P%YD-6+x*J4NTy?h%uzsb#3;^Myz`PZ}@yvyhSogq|>DE3}EbJ;LhZ?Lu?5ZvcYvYJ^%K0{!dD1XEbG1${OI{ew=M3~2&W*KlnXgxTs<2q@39s93Js^51T7^m{9&k8jm}Q0%IP_gw*s%||!ge8NPwIJ8uxyvoX&V&263as-bS8a&5ULh_!~$4(l{>*Ne+%@Jw9kf>lVEeYrRAaSJD9%{#I3{nqNx-f(A*=`?R&Q_#@b)+f~_I%g8RhRvPd@$t`-#!LF|66=zi(B;nHV^Wy@6I5OL{6y4&^1B~KqiCy?GtK{vrA{qj44Uvw&2ae4esDqplf)vo|ngeJnll%S;BvDd7!3?=dU(0E%7Y+c8g@3G{EtoepTKgyX0!mjE(h>cP2li7b04-+(-vS#2kWSm@hu%a9uzqf@na}ZKgGX%FK)}(?LQpg+0H_}2%9~8+9%I?mrB2m{-cF7?gFvBC(hdlgU>A1kpzw8uuS$REwv|3eEuRK4`tqgNN7hEP#Ha@yu`Nhty<_%N{qg)yu<$sz>VzAigxc;QUL;-hkj`akj_qej^K#HM{W*Eb;FRT&F#8ar~rhd41S;&en^UMsoTi&_5imsPsF(U3~T9pHT+;eyTa5(yJe3y1&-scV_KFV*UU$-wWNy;Z8lmX=eIc<0fn1akU`tG-r-bQa?-o}2#F@;kq*<(R2%cI+gk?`=Z)+;M!tM#QkZxVMi5I1YM%=8+AEo4?!9B<HS(<!_lFVdZIB57b2#VX~E@EPx`DVO9zy8AkX2O3<h(zIIz-MK>!cCuumv4SprC?LBX$FNn*h3%);ne%G6h)03z@kLFmoANsFuDX_LI;@)%#C*@Kx^z{NM%m!kCdB~76OPTfD{LNg80iBAJXOR@<Mg6dw+<}G<KZ$&(Zo!(n)GBDm`NrbCMo(~e#=WYC-DcLbAllz)^d){sFP7MXv2@Pev<S+Sd7y??9HGLU(NL=$CCjXLNprHKH<*@BuLwVQN8u|gh`<CB?iewGIzeBYV@%8P&(e2eOM7Ya)1y+&#jgnkoudmeYsX*Kgg?mXXd(DIpx^dzHutY*9_{8Yjrcvl`|A-H8@z9iq31nTg<14bg$?3hZi-VU~}Ao<LQ9mZ)J|~=RCLXwt;+{Afpl#NPA<LDa-9#<fjQ)CY@a3ctu(svnyIHP9*GBK^~7mVfp&8aR)n%qro-ilou`YfFrsm_w<9VYwNGy8b$dowx`wYj<05R{72h;P}O$hW1xn`eSmVoYB8a1P9vhN<*gtua9_ekmOWEw;A}PTisIB&20?dt@thy!*X&$Ghc928o}K)3cy|2qMep$F#o5tGgLOO}FWS-=YU=sX*}=2k^Wzss-WI-epeH|{V*5vbIy*TyJbQWPVPbCepASx+G+5hHKc2+t1uk?5Los{}mFZoi$)Go!3Kxki=k!ntf{@LqpT+|qZPxbVxJoVp4QBqQ9>N~kI^)f;VdCJAMr9Lul_8RSoAVUHEq)?ClSM(LSCQt*U+h#or>_pqj$zTB9Gx8zioNj<z)}By_X|>|H)_wzwGd0tL&Et1$o(W5_0Vno0#_vI+jKO_(BjaG|N2$UcTY&hQm;FBP*M%!r(@iU1^R_G*hZjgz{$!C*JNeOdv^T8k7vDCCojK0s)sTN<Rog5tJ)}*Go-kXq?2OAX5nd^^%N5JqWN?T8q{cXoj*g)QPDgZ_EJpl`|M&GL68&0Us`VfrcT}S7`9bhUNLk<#juT8UhIC1x4g%Ls}k5{lp~zkHvS04KE{ykIfq3L8?h&5`je|Z!Ysf<){s9)5WRiAf_{)Xh{%_xATdhD_1~p%-u89R!5Jgsb3zsmK&}Fc$mq&ZMzQjFruO<WJHc_R0+Q0dTuk2efJej_03L*MqAk&2kZE(Le`mbEoYKvyr8WgsD&G$_1{;;`4SD;~1eTPOhQX>YrD!$-rBDr7d!-m?6?STu|ERKOg@b}qmFpR~DLH$^!Q7ndykKD5HGOldWucJaXJ{~0brQrqf9(y4r-D5^3~V)<n_IbSY~@fQzKGxsxm*@OYZ-_w0kv|-wOct)&ahNc2tX?os6ni(-O}J7l8=?>TOs-tpx&L(&YKd?dtzdr3I#W?1KsvAly1z8=RL!yMFJGsOG$(NB3@rAMFs>%A=7=a+)!2glJ~`OO;)0y4R-~~uL9@SxkNU0cTj+x-K-3f;gGGbUjR*x9uai2M@oB#2|miw{-YSttqJ_`ytK#)^1KghV{dcsyxnd#`{#qfPG^g?TdnQQ&6V}_^$Pv0tgWq8>Fc}i*m|e2%hvD@JorryuSU@%o};DU^g5fz<Kw~2;}y1I*&0^3R7{Hpg79ELp%6}DJi<fM<AfU;$-*u46Z^HgZfLxd(ViMB>)84qCe!yLyfSro8BH!?x<^G=Y&0of$>!v<YRY$GYuzHMX9*fu&XRF_I*Vg@++bhsXt4YpN!`UA2C74Xl#RAvpp36eiTyVa3K4N(W23Rj*5L0BVuL!pMQ86L#2F^m*&ck&euS-J6;hL~<=(hO<-BX|i@LcAqF3X0-MdK6g&eDp>dAV-@l@Q>qvX7JL-PV+J8y5cI@{4kv)SoH{k^TtYTH>fQM{Q&AzZ$8W1FqDY4KKQ_0YEH+DsnnPssB@thV0h-S?QE-_vUZl2MEZi1GYRBE0c#rl`PUKDaF!-L}RpEVU2@KT#1$iaTJkIjXN>%<fA)Y;K}dUvT7hVH&*sN{!!st*1cmE8&H59V<i*qm;yu;l|$X`EIirZ*|T$H~ZBv#B8S+P0W%+_B*&7@VDK<^`N5~lPAipu45U$dUo*Q=<ML+=ib4KgJ(aV9-nr33O}C74>TC!6H$5%x>_{4&JvO)omLKCK70A%_|FH(WS#n4ocJ4&IjZT2rflYu=9$e5`#!fmJ6Q#(Z9CBz%QcI6hSZ^x*`x%6ipJSm5u$N5Y~VN7N{~SnlFYxqx6x{Mn$7L={xI%uSHuO~ObIIJg0wLK2Rg7b;XfNv2ms-2KyqQXYaAF-w4wm(cXA>%+!x|R*?r9;E;7rvBNH6?7!4KCh>IVJr#9q!E{quO)lX^#I2=`!mRpcYh6y@RuA2vl)^RtmR%D7WIFe)Bw@;(KKBCvJPy6vCN|UK*S#Kf*irg#NMDBwOK&hFT1t!;Xp%L(9XoO%V9YMYXfULk(;L8B$ft?6~xGVs<B($#g_{mStj!uuxdZ&j!9z8$ky*@fQB?5#wJHKUZ!7-|o?Fa^8mwj@4a)bvVKlhGayymCJP<Hi)gJ%bzPaO7)a;sJYPmZ1*{PgUsmupBjg1Xc^liN0XB(ybbhm-OhBIUcBvkKbfwA*DbC;j-@wEym#6W-1~G%eq$i&4x7xO&7B15oiCHxSI$%od=$>X;d;A9;?qlpUe9xFOAmducp`W-lE#6rjf(78`pAi_Nx{GQ@1etmQ;R)`PA!OVOi|>dmEUE4IHCd6|Te*7O?6ZCu_^CU!tC6I-C`p(<)_+uJy|?Y+jH;@HltR9I_mUEq0iyP+*4EDF68ld2^tNWpm4!B)*FHdExszkeL)gQtVZwXcq&u$x+LMD-Qs1_h^hDq=>@rqMuNz%)9t{GiD~p^t^cL<8PkfqCjhQnU28pP9*cy;F3D(qIo1Cc$qLUY>~Pfyt{bv)lqG<dHLXQKxw3Io$IvbFLT<+hg{OVHak=8EWDA{$A3m9nu1J#r_Q8aNQV$K19wZgC*;jd^9Ur=v!Vd@0nPsH{IXMcQqk*+8KNXT?X1pGK|r22Q+j9!R{u4V6OxO29av4crWB+&U<stp}-Puo!sgg-*4(t)FmmgP1L8|*)%l<&%k;5Tyrq88J|3Fq}9>M2p)ve^62(;y&|zbH$9^hcemsu{aj@W96Q8x+q-*?t;F5JvRUDLE7(oVQd=6f2%Xh@Lb5DnlEa!`{;@0_9Jo1QQa0y;AvZF~eL5|!F@hei5qCHSP5McaVFXk<ZwXD`)W&R*F8r%7xP()dP72aw9)DGXrpM8I9)k?Lns7+k3`p7v*hlUJ_WPSvhr>9jr1(G#2<c%I^l7jIUylvnWOEQ6$5|F##CMpoL`kOQg^!{&&wOdsHd?f5o10QPG-W&Gh{=?nZwBlEn(GFne5O1$OC`W^>90)kt1R_(wl-*%+q;c+TOp@+lIFiv-w=8B_w$%t0NX@2#`$G5VVG-o)mh%U$#LxabDBn3O_#(#$g_WkYUGbGR`Z<{jejkeyzyp*x#JJRpI_%@&UNmxO8ZO)rCGGMyWQspf1+3`MpGa8dT#Ue+}$u}O>jAZ`TBGfBid8|Tlp#lK0Ax;$7YeQEB~qCKlRFW?!I?=E!qMl;0|)~P0%3PTc9NEwHj>#ujzuyoD;kWkRoX?Qo>Jf2t+7OXKBJcuUh6#FpmCYMI%^~9imA)Z|#;eVN&~WE8BtNXW5FG#V^#xh8)R_l<?#sE)$2i^Z3KON3m4qy%|vC-gJo3T6+Qb094oAGhK2xMaw6-E{jdEDuyY~V;^Vz%Xl1h+mF4TAhHe4DZUC;|L8Yp+&o@2VV=(nXs()H8s?7eMU6OGp1q3B$KpQL!UMHdwc<$418Qp+`Q}4tZ`T`6uG-P|CU^F4Mp^ItdX58va+OTQ#$kA&wb&{r494xP{&ojXi(9Q$v^UtPW-zu}Dz+KhW%*&#CLT6z;$hSJqetxd>sRZo_V)VsT_!-cKEPARi9y5~7BObgWT5COn<cY&luY90%9?6zr*c~j-4M3QlWgfGxlwyQ=yv<l(I}=WS-1ORlwF?2c#@^Q&yz74y~Sy(R9kn+M80cS$n(eK^7z%Vjjwxb<I<Sxo_>q>!b-}$_J25de$+cXJ2*R%+qOZ+v4{K^OxT>&0d2zw1)MYIT23FLGZ0ZZG|r;*BAy4W+U{#vJ%2o{&!g2Pg=0igQA55%*MKY`TLkaH^~bUnSALX>9KN(5T9cl7>hofDkwyc6iu|IR5<Y#rUlYNEEZpm>ltk;|#+3+<mJ=Ns`lFHxG9ygda+u)U7B5E)-ie7H_lO#{g<l)(?+Z2L#qS(`R6`BZkI#cJc0|b>)>J?k(Q*!dWS+>XNK_GjjM#hnrk@|Rs`!2Q+&wm1!dT8NI?9HjZ{-la<`j8vcyVpD5((A(NG(Jw&qaR1J#X~qToiM&8)uKG%fZafjHJ=KmPhuwU7Yu3M`<pdzhxEb2+5ih7wbM0^XI;P)5w3hDKuahyIf>$Ay9Y&GkL;j7eyaekx6+d5Zmin<=ku*g?sl61N1cR*G!g=tGNHwFN!(|Q;cx!5yjqk#;b4S1shSmJ%S5+GY2jMqarw2=;)2P<Idv7S!!z`Y3m+v;>8JYJW#|MXH1YEV5y#@ahWLBQ*JDlaC&1Zf1TDQ_xorWr%1&J${j&<VDAEIEc|mD3<wW-BiH@-?n{em3WiqvMzUvc_hd_9QIN-_&zg7%iGs$;n9OtylSlQTl(Qz9`1}#;z6A8@49Gim+7S6UuT`BhZP=p8u9ZxdkF0R2whx>OK!?WI&F5k5vei5_&`_Y`SbpBKvA2l=G`uk{U5mJ~p{<k)T@|pk;S!lt!5_ZXB34CFt9;B)2eb4tswF~vhe*Xu*cO+YU+#90oG_Uh*q!E8!tsh{SWUm>k<|)STTw)4weeniHy%XIW@mS}IcQaMnpBOHD73N|*JXiSueCYTHnrI<o8i^OY>v*amGoF!d#zC0(OhLfYfc=5=9*VFmE6IO=mP8yDe_;TAK+{4v~8~{JWm(66AFB}Eu~esWaZ^KkDs%CfyT5fr)M~R+*0Svd3XgJk4(pMgu{TQV@h*UTP@XUOSKjdp3%G7^^xN3`;^$Via@io!<&xed4Tdp%b<E^fb!-mK=tkbl}ckVRJ{NdTFoQaPa*^un-1xQA@Cf^=ao#>$MG2BHPddUyb>6DqDi%IAM2{1;-DrLXACH1B+W@u+oP>#MvzqTK(eOkMwZvPl?xdLY_~dWV~Y9=Nx-_ek;M5u3AY*bDDNSmmxAemR?tm*c~#F)><I3!lPpm|naH|DdLaDWRU)CX75KP-b?6{art8C_ZuieeCoda?EugiQZbb2fI<1C}>$rNdN4+#|F@@9|Rab4f;!5h8Sl8LLR<&TnRG1A$jcA4(HpH<KZAmm4;GPs!+}Ex&^rj)iD{}eY*xBGqXQoTnarjN=y~<1H_r7x$Yti0r6gdWZmdWq<XRe7am%O_GMz0=kJ?TYkIwCqy4iLnue%)v#E|z6P2_edKHa5<Ot+?52cLtrfRnb5%+bEV~WGO<32FNku(6qp$O+@u+dixoG58^x<-+rD0tz&})iY_MkJew{uhKH)d1xS(^%YcI1{(THDfRN09o{lnBD-vz`mocyrTvV&MJGaBJropE`w<)1uoeieRm5{RN4mnIFTcixrR}!u^*k2;RY@q(+0vC_5FqXi<JbiWAG<|ZzsX<I?AFR-Oaq!~hDfuX1Q49>C{=c9!9pMfz9|x3iE}IOd{p`{BYSu%R40;}&Mm5I+z>3f(fq2o@Allj8X*RdxXfNJ#t`Y<*bBqgqCZ@5q)z|>80sq-1^PymTz}aT$?cYI_O)fabvF-|6XK(XN4WP64mOZAgrw6ai_wAPaaB_5dbZ~O`WAE_b?BLnUAIw5;n|SARW$kyr)5lNjKmPT9DZjMa&*T`80G=}*r6k7k&$z4rJkc(Z@qk_zx1U+GV1wJw<0wVrEgQtMMFJb~ZNL}%(c30_2AFhy`)|10NJ_K(@lYv^aRV9sL3?Cc0g}p<|AhPHLpu}Lx>H2w5$qT4UU~Z&MVAR~A9kKhBEaJ&pG&61l?byD4fN!wjR86<QFT1GUc@8Xf5OX=At-RlFu0X9;mFB1d;)8ZF&$0-))d?U-_EaRSo`+hQ|f3GnYWWEI`a_e4*BH)eHq?<PLaak++a|X1%hHQU7Q1Rf?xgXJ~j&VBzT0U6Ujp`H)uXuT*Vil5b;}H7$=2|9`Xs#Z$CE>99jPom#Tro5us#ED8%!Xwb4T$GijW_8|}jhkU)|rodZA$MOc8~P1FHeK!C-#7~|;xr9n1}GOH~bWf9`T=Md_16kqe;ep`O<AKZH}e~0k?DXt!%?*!mZC_NBY;IJ$x75GZr+GZeWXSaWc0t=WRvOg?5jLuVx+=-*lIDY`nfPjKUT1*5-M&w&r8;Bcn$sAED9VgIh#`ghi9pziWu~6(Q{3iF3?R))}bxk~OaNr}7M6k{mC?q3SxcztdG*|#`>Et+o1|U7Bh-(=4g;6pgI&HxLaWMUkoxp<%cMxk<K_QO-*B(ZH0nL~V9>O%I=^%}{A&Kt>b7Rv!ur@c~o$8Qpi<LEkD0G9Ru^VBlp$&(#AZnBHF(JK~bE3de57B7E2h?C3T!{VCF>q?UGbsz+{%<S>=>PXCdLbwRL;aCBdNzxYq{xLLVnFF7)_J}y!t#{D%V1-Djv~l1DK;d59(#jvM*{4Odwt8_DNJq#Gz=*wYH{!%z=UKBor>jn%*g@&_yGgS&43(@1s-LXNaObN==Sd^O?^7)BRnlcM6$<sCNgY-1?_Cuyy5_lMhI<*S;Zfq$${!&hGbiUi?40ANUkCQFvN~yXfYi|eOfbNwn0;;AVc%%0;CW2_UE^3y0}V_1_32cqlp4_lN}?NaY2#DjAA`BjxriS0b570#O0BKE2SJJyvF$>`tY=ighdba&J%=J?Bgb)t{4lV6e!0;;|V_i4!}W@A&ahJ0_z2K0wBe^njrL1bJ%>OMXW-~3-ciO(KcA5<|g8AGKtf|pv{S|9y}pZ8e8_WE2LIPUIqFYWI*m9K#K`d4;itPQ*U6DFmrT(<%k5>#YYF~E)05+W?w|0+aUtoDJ9UV#No-o)3bx`pB)9{m%N-O*^w6S^cx}7?L_hJ`ChBp+-h&_Z0+rMq&lOHA<P+X=>VAQp758n#~*}dAp`=X+mbi>#UWNG4goKY(u52(gNO0tDgh!tA+%gUfafp9y~TYuj*0UE?-3TK3G8{G$kAesm=8P)_ZnXA12QK^7xP0C;Aouw_LIq9Z2_iBWHD4eOFp>883~{2@<yIPk<=m)i;Jq_zJ@GJk+C?q$hR6c2)cL@3#9!mIMsl3uz`8sTr{ZfzKAAGHbnFzlgto)VD8F!##x)<g}fp6Iv2JTrRSWMn93EkiA}(2gOJvT;b#Jq^Xz(h{uk^}kZQVKk{?xwFN=|RuOVUM!$@g@MCTd?7sZw|D1_X+_#n_Clbs78P!k%GMB)##l;|u`3GOW$E7@9;8qRmu;WI2V29(C)q)alCGa}g$JBBzgBK}aT)-Z$tU>C19i_(>K(IZZ0&V?@^N*7!d9Z%;#Lz`Tx56yz_5Q{;U%nSLAGcopM^CIcbni#7xFEm{OuQk;b+>V5}!qslG(`au3QQBy9h$;zjQX!*8Wg6rlLFpr*D;Wd^u52D)1cJZC+{}|nUGgcWg*X&Zi{9dJ2+#&7GbI*JNZx&G1R;7W6{rTy&Xh|R6T!|`*6@>H=xX2NJ0+`1SuAdlG-m7%EU&SemU(5(y5<rklPrOz9%73lJXs>_(3@(q7z@!~h?-IcA>3O-xjdpo-6*pVv^WDCB^Q?zf$uzi%Y}uv^niY}lv-W#GhVO{Xfc%q=|TB7$%5jj5YMwzK_kF}F0Zs$4bY6eeG(625IWvA*xTo5tBz7=89?&(l;;I`E0o*Y&wl}_jsyy>U1o&u2@-d-XlXSN>B$h4JbUj9rU^BJrcI=W4^e;xDKf)FRm-um7F{FJ2i*^m6^3D@00w?{8ac~u%El;oMw-+wq%@otl_OZcQ8G<Q(j}pn<ajM3!*0zGd!&$v;3Lfy`B^F~Te!clFg;8_V#U7n#)Lll(S$3Aa&sbZg){}NuTVKW3J{Vy!Rlm@&E+P3%@3UgLDIq^?qh9LpaBS@z+^g)#`I$3+ANgX57IRZX*Pnf0Xmc5ys~Z?5~6bwu!SBbEC|Frm3Ym+a8{P}6W$L|i^*an8<o^mcP&Issiz=;1GE74M8=Ct8o?YT$}bs@8i6A3Q@{pe2QNP!f_G*v0jz%-UnI15=gM5d#TISwz`bJHON1E(MRr<!zr}>>Ss>>K3Z=47kktx@m-Lev*<z!Nbjo-R`1Jn+cbX;8"""


def configure_shared_guards() -> None:
    """Configure the proven MVP-016-B safety helpers for this migration."""

    base.BASELINE_SHA = BASELINE_SHA
    base.PATCH_SHA256 = PATCH_SHA256
    base.BASELINE_BLOBS = {**MODIFIED_BLOBS, **DEPENDENCY_BLOBS}
    base.CREATED_PATHS = CREATED_PATHS
    base.DELETED_PATHS = ()
    base.EXPECTED_PATHS = EXPECTED_PATHS
    base.CHECK_COMMANDS = CHECK_COMMANDS
    base.PATCH_B85 = PATCH_B85


def validated_patch(root: Path, embedded_patch: bytes, *, skip_checks: bool) -> bytes:
    with tempfile.TemporaryDirectory(
        prefix="galactic-mvp017-", dir=root.parent
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
                    "Le patch MVP-017 ne s'applique pas proprement dans le worktree."
                )
            base.run(
                ("git", "apply", "--binary", "-"),
                cwd=worktree,
                input_bytes=embedded_patch,
            )

            if skip_checks:
                print(
                    "AVERTISSEMENT : contrôles Cargo ignorés à la demande. "
                    "Cette option est fortement déconseillée.",
                    file=sys.stderr,
                )
            else:
                validation_env = os.environ.copy()
                validation_env.setdefault(
                    "CARGO_TARGET_DIR", str(root / "target" / "mvp017-validation")
                )
                for command in CHECK_COMMANDS:
                    base.run(command, cwd=worktree, env=validation_env)

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
    parent = root / "backups" / ".mvp017-backup"
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
            "Prépare MVP-017 : catalogue configurable, file générique de craft, "
            "inventaire, persistance et écran Chantier."
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
        help="valide le patch et les contrôles dans un worktree sans modifier le dépôt",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="ignore les gardes SHA/blobs (dangereux ; le patch doit toujours s'appliquer)",
    )
    parser.add_argument(
        "--skip-checks",
        action="store_true",
        help="ignore les cinq contrôles Cargo (fortement déconseillé)",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        configure_shared_guards()
        base.ensure_command("git")
        if not args.skip_checks:
            base.ensure_command("cargo")
        root = base.resolve_root(args.root)
        patch = base.decode_patch()

        if base.patch_check(root, patch, reverse=True):
            print("MVP-017 est déjà appliqué ; aucune modification nécessaire.")
            return 0

        base.verify_baseline(root, force=args.force)
        candidate = validated_patch(root, patch, skip_checks=args.skip_checks)

        if args.dry_run:
            print(
                "Dry-run réussi : patch, périmètre et validations acceptés. "
                "Le dépôt principal n'a pas été modifié."
            )
            return 0

        with tempfile.TemporaryDirectory(
            prefix="galactic-mvp017-verify-", dir=root.parent
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

        print("MVP-017 appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=11, SAVE_VERSION=12, "
            "RULESET_SCHEMA_VERSION=2"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
