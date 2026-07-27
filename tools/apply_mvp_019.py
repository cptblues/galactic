#!/usr/bin/env python3
"""Apply Galactic MVP-019 safely from the exact pushed baseline.

This migration introduces faction-addressed commands and events, deterministic
diplomatic relations and their persistence. Dry-runs are deliberately cheap:
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


MIGRATION = "MVP-019"
BASELINE_SHA = "a4f7996bb2f1d55688580d07042d8f1708a55fc8"
PATCH_SHA256 = "76f19710dd9ea9f1d8f28a9fa5cc3170f091ca53d398e2a4bad2a085023cdda8"

MODIFIED_BLOBS = {
    "README.md": "57a15319af42bc1bd3252e199d79ab29ca1e021d",
    "assets/rulesets/default/factions.ron": "f08dafe79130f9ea474956242afd3405796be2b9",
    "assets/rulesets/default/manifest.ron": "34f63d4d4ccfcf73d969317b6c8bd12e7df43e69",
    "crates/galactic_client/src/craft_ui.rs": "fe3c977e20a65707d94c3dbc1b5dcf15a5849a7f",
    "crates/galactic_client/src/lib.rs": "c4950977383c639923f986496da4d2041e226aa2",
    "crates/galactic_client/src/research_ui.rs": "4fedc549888802785754870e37243a92275eb786",
    "crates/galactic_persistence/src/lib.rs": "296beadd3ff35b534a88fc70c450971ff4058d27",
    "crates/galactic_sim/src/command.rs": "25cd4745f2b3149ea0822c22e44a2452cae6c09d",
    "crates/galactic_sim/src/event.rs": "cbdd72566832668b8d6c50b0236294f82284f5ed",
    "crates/galactic_sim/src/lib.rs": "22ac9548728449cb7fa555486984fb2fdaea0b10",
    "crates/galactic_sim/src/ruleset.rs": "21648a4eeabfafdd68d3132d7ccd2c248fa66431",
    "crates/galactic_sim/src/simulation.rs": "2543f8b70d669efa55647307d7f9cb98c4f5fd56",
    "crates/galactic_sim/src/starting.rs": "2b761de128e72329fe277a1c454993f9670a059d",
    "crates/galactic_sim/src/state.rs": "9b0aa4421a96189af9d86dd40702d551d0979452",
    "docs/mvp_architecture.md": "fd6ed408aeab96846f67659dd22b82cf6fd7825e",
    "docs/ruleset.md": "fbd55fd275f74f70317f69da86d8e3257860e6cb",
}

DEPENDENCY_BLOBS = {
    "tools/apply_mvp_016_b.py": "1557ff3f419abbf6a1b58b897100aa72da80bd38",
}

CREATED_PATHS = ("crates/galactic_sim/src/diplomacy.rs",)
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
    ("cargo", "build", "-p", "galactic_client", "--release"),
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
PATCH_B85 = """c-rlK+j88-lHfbP0^*5~x~D;?Z@e`v&Cn8$%{Y>+6>0CxDijd9yPAa6Hv`?0ITj=4b>HU$Cgy20&-*ry{FC{T&8&MBssMCT@}3j1im*ung{-X1tgNi8tjuvbnFM=#SLrH99=|?2eg5(+nvH{@`h6#zkI8Sr{$bMD?{%VRc--!f4i17&yM55_@9gdE>0j=&S}o)2fB1)Buip+2gBJZi{P}z_TF=OQm8{amJQyw3*@_H;cjLt<e?0rJjFW71ovz4eway4Yco*#iJHZPAE$1sTUk7)e;kU&s{dG-(WVA{r0m)av=sL-+?mh=&vRtQm@Gi-7vdSN4>nWkXjL9TfPgjrM1<86R7~g%KC0UkSkqAKN0Q>I0R(TkV*V)}?0>7-tZ!7#cy!-c6irCU;qwD!%y0}X5%_JFS>4**@4|nz^hzkB1Ow$>lp93brB!%7svI@qF`5Xb`L+E>%WUF9F)*k?HaQC09j0CgAIGv<-pYtG3=J`%b!OO^$^%A7>bd@Gk@})Wf(4)m{3EyYIlmx$!HKp};>l7hSiciw3b;dX)c*IG{STok;$47H9mM8--!&=R_)J@t)<77gjs5c=82i-$c>S_UE0$Hmm%4#Rv4_ck@03>xMXzc6-@b3f3a%eONI^j;sc+lk!IJj8wY7qQ1e?MP*oQFI3IuP9qf;a2|{%fdT@n1R~1nrO_(QSwQpw;b!9fpVg4<UVk2{TEiISJLfCjb2{M@fne_SsT>wsc^VG|N}CTjOyKaxovP@rr+Hct4Q!Dodu?`)0UpFI{gh-!JkNNT;J07-i6`Cw;SIo=!kTR7juRj7wjyKRM`)`=Ij<4#xe_u%7f8fXb!MXsSuyK9@cz7rXq6-H=ISKDs8eB$jg6<IkCnTp2G~t`GYBdEtY8k+36B03?5Wl}u6gM)7Ex!Y7aOY=o~St9YG8Sq{Uj*HBDt(i<Hg9!Ak9>Gb-&Ba^r_K{4{yY)oesu0&nB5bXr(oCHJi;bt&cg3PVQWH9)<6ZTlb8ZTx^Iv)(4f#RRvoR97Ic{&>merMEwhMk>d*&++s@Bd@HSP}kvGMtk0aTt7;%*eCF3|P<qI70;;1{W(F#Z@|b&J`}_P;H>6RK;j8|B_-C4!BgT7-g2(;wmF~9@BRrfBGxyVkEkVVVz50;nN?1MAG#LyPPGz#o|k{7FsF4PM0@HHjWqT)pEUxXX$jB#!HgLK)Cl|;r()v!a7SEFx8+fxi@-$JHhP<<>r28KkRga7XJy4CiCDLD2pgle1_}Q3Uput*EM)IN|tC=@Rt)p#=~Uvz5z^J3!U?t<lkJ#zu_-1P)6h_4062+)+uf9a=rS7(*LLQ0_+&_v>E))75yR$2wid0dGJudjh19SPUlxKeVGSbV({NAS&goPzhhr%&+(t+B?utnJ&<?Ypi~|=1F(HA7Bj;Cjz&~Q;}qUJc>=gbY^{jp^We!-=%HoyOgoO=gRtoDjNVa_z9a^L{cnoqi)3){#3^T@lSMWrStMIDPm1uMXTe+m226pWK4|Rzej6mefV6>~c=vC?G@-Urv>R%DM{^jo4f3{}D=?jxFajXUNc`5Urlj+$!bk*xWt&<wHsNedaII5~+H~f1k5QAdA8g)z(YaN`n44KTn?M~&1k*W^cahzI3QFdybafLJVQk3P*W?%2h5&;y{!0z6I==DpR5bZAEcM9zpVdAEEUU>nhb;ow^1t8i3T;ntafxxqqcEb+nw;5(-99p#{b0<dQxVkkls0EPH(31;Sz6?ENZCwZDh0ZuX(Md`YUHR}VV%Gq(Q=yHz{<caX=FX84T6jWCQY3(KfZ6oP1HEaayh+;6?#Bex&T`ixM~m<1%t2Ig8|im6nU4vWUH{%s=zCSyyKmiY6E>>R>Rw{wv9GThn40TYr`$GQU7?qeGHb_q<`4y9Jgy*W*kbTQO4U*gSQ_ZfyImehd-T`7boAVX)-4(G<Mi;%(@YBb+KBYNddx?J)45L4Zq&xD>B3OKh0A#Z-IljoehKkxyYvDSBqRTCj2&?&s_Tp?Cz#yd_`Ws`Z?A9zD%+!0u5gN{3`zb$CqdE#XntKp1q9Ep94;mnb0e5#JW(kL}2}Mny15bib^cFOlRa`Nl*e>rjQx<SXSRQ=rQ;<j`h)_gxm?1>mh20Y&~z_Y@}R1?jD7m4ltVk!j#B*IR+Vtli4u6S_8Cr35=ZzTa8J6yaZb@UQt}?DHzsv5WIp9*%+i!z*Ub=eNd51;jreq;iWzt4A_7hIsL&D^ixwEFnSf*X?xaM@Pyv*f(u;W*i^7JjQq?}8cW)mK9_^N^$bWtZWUUZsBVpduFL4wtkXY4Wx#)0Mg>!tfMIe)&>yD^48D4RG^n@AlAzzclrNaMj(glcz+#7u$0}P>YFTFU{p|1xxK3ja?In=;ho+Jfeyia+ozlVbjb5YaHozjEHI8WezQL|fIE(iW!%h!*%zoLB$@V{H1R992=^Rg~+Pjwps8QKvs#`p);FBN1AH*$XDOa<q@LX}kx2?h@Hd%vJ30wATmd@`!<F?8+(&ywO=gf9|C>qzBal!Zfh68pJZ})U|_j#HwGN-NW{$`{hS7L{A1=&A3pzbU7lg%BzO7abZHg(u!YN&-zs$Sgf(U*~Y#JN$!PIlVJwbqZXxpeHe4=H8XPv;gzBh7kwl_g`+aO@N;TxxC{N;ePg4aB#64*$e(34Yf%G;|TTwcO&)9Y!rMUJZDwem%>w1`{?$1|(~*eb5b$gBJhE$jV5vPGhza<CM5XqfM=4?47<g{*(&5FBex=Q}QZ7le=thrV-Snp9VlTv+m!6<9prkQXO276}5RAIsTthY_FE!ct>Ty1U9v{0_eff!*(^MXE&oM-yPt17$4`!hxCe>L$d{nmnloqxBPQTKE7O_16^#kbSXNf=fC`CnG~rLAiBgwZBSL99!VAMr{fRFd_?3X=Zi4pIUsSxAzyGUB73+ud#a$9512_Ph4EdMEUz7^OJm&_+r^XhbQ)i=*1?04*UVPyb=qAt$NA5S$&d4YPc|r}L&?^SHy-GWXbMi^8pdt$Cz`g6v|m~8qD%f0S#@vB*mwsT3!7qXtBn`;U~$s1AA-}~eMf|W2!JK~3!5XUDTBJ5ZrEu9hwvBag~e$cf5093RIGm3U9C~tlAt6iTF*bilX#Ie+5%^4r~-O1Td{HU^q2e%eM+54Z=*Ik1<Dwx=4s@7XfoBm_tsEBZMU}>SzB8wSw8}7O_VI*=F5g0k;c$KI<$mSrPu8p(ajY6vafBXzGQFJ0-#%!Lh9HvLkzyU5SaF+-)?KU?6t39qbd)}mcq#G+m<leggBX0hQ9I!lZXDn5z`0#gMIvk%fn*I{Ad};Mbs7VS96{46<7LAa6x~~p1meuN#`rS)5Jfnah9a>Mie#m)i|9n<AUT5*oZ@YyZ0(vjKyYc0w`aD1rs-$`bMGvfNp6+d*kPaWN*P%>D;n0Y9cEa>}S`>{Az1VJz%yliKdIIXqx0J#sFwa&za~S{4A&6IqK0fbNHpLw1!j^@iZBdX+!=!upPq9VDBj!e!ynLQAj22gMUnaAE;59pX^yKFI_e?+EU|H5JcJ8(&<1Sv12UuHgtm#*h^62f$lpf^S~AS2~?9vSAyC!Nz}NqA%+iN@b`$S*w6c*Z!u~wsH%}0@}+tZt#~g!!Du#}6Q*o-jOb6>XHzv9jIRowGVZ<?-h9?Ll1}}?Hr+Z1kskTf>kBfe!%Q7YniNl+VR35!DcICHJ`VdmoLa|+_zRm_^=F3Z5j*iP==WQ6v%z;1i4{Mk;R6RP{~=whbNOJj&NA$%idmQG=zY%jBW2=mw=;0WxSrfv5;+1a=10TA5bG8?4)J3(F8nK^|8F!y1Bz#+yY)SGv%8e9(2!pz(-ddrZnRp&IVFC>1GC)KF8zT%V%-;|)>bC~Za3ww?rWs#NORiqs2CD7e!p$n&dP$Lzs%(>UNP_6l+3SI*JhOJUTtUlcrhpS2wnI4`V<jvY+_RbfpRL)UzJaLqfxqD(Hjl40z}nV3yYI&aBvqi8>{J&t{yAckpv_8x0p3GJ~hXd5iqgf1HuMYN0?#y;dmkyV7R5^3S?^>5qXAL^JqxwN^7*5plT;A+PErJzd&S5k*xW7Da-20qRy^OCDIkXSZm5vd}qxw7V@LXVYN<`5kK2%PhU7;{qa?P?H#3%zo3z03{Kz}tRsxDcIaN&khj&LHNN!@mrH7uplSmxlT&Io?zcPiepLUM2UE@#V<2lzgANSv?WnDPFP$ouccH{t;yyHTc}1K~%j)x|omz2jxTXxOfGYutG3n*m<>`y~<@pb1y3r^$vHOSpu+s;c?H_jWmolP769Cxw1nkdlJ_YngL>Fv~wKU_T&@I1mMQujpw<SDp?5;p#<zVov&|;rt=sO@8^&uqbJP(p!%mHv0(XI_hdX7#uWu4(`6x~QqXMH%e&owrX1ut&f0INJ2*)ev2u#^wUSbCph-~;k%k;VLSOXH#LzLnTTu%Q6$+t3eN@S;uz^Pj<t1YaAw7eD^+{O*76HfuG0u?CHZ@r9r#KqAu_uFlPErt8<>Rqc}}ByKWKrZ@lmrO1mA)nS41^O{p+&5sZpd|Q3kZ&SZ!|FCydp&(%`felo;!hWqc=!|aoruFq=Mg(Tp3C?3Vx923ix*jgFU45oida=Ep><y1JSl_-`Ue}H6b%)cFfSQLJ3YDNZ2Ocm%i<1vYI>mL*T0+&)<rxS~#(>t3Va~XXS?8*m*_OvN_a|gL+V4jZ84o+{;iz^@vjwd(w%KaWF7+Mar9O7&?D1m^z1?ef4hAq+#u%9ovrBvx<fDv`dGMYrR{>a$Io`y@H3vhX1J(^n|9VQIdiU4r#4-yDp5HQ4`!9M6?BeqE>E+pX=g;EH^Jjm#h+mz(jxWxhfkG^cNS5K*TS7MD3*_Ck;8N}YFti&lo=J+cyJwP?=yGE_^LVvbC1QUrUz@km)ZH_7In8cXyqso%vv1OAXwkuHSJ~ukI?wesdcmd*7S3_d;y;b6Z4$KScAI>EuS+<BS%2UiH~-Fm`)JuRnAO*!oE&O+od{pyF4_j6k{kFU?(dJ-fLt!OM14wmqAfKaUGU%k@~`N#8{d5%P8YwfY4B;y8)Vvp^?V8&Smvp}SLkrLN2DGXYKN+BM6}n^?NTc|RVtFI(A2{=yCK&0-VhU@HoPRp$)Vy6=w&cppiQG0(f88mWV*ss#pTz^GzoQHCA)W68P!9A%fatjhje>;zo@bb;B^8&ON7fM4(wC5EkEAxP(z*nbQ$V@dV_zy<qL_B5`+|w(;Roz>omV6xd>>6!NwnxbVUzDbnH+TOykuMiuhx?q8317^9`~^#O<9U8VvK8Ombyl*fU^qzdF4<KYbBDKf648c6t8e50>-eZ)YU0UjO*&rxzE+Aq3?{{}K~Xz~%rpr{%X7-3__9-#tDUl4Kl3lTNRHIO&v|`yNnkTi?@|8TuV+>3689zb8%oqd~yG5{Y1fF_?g1u=qHK=K)?vLnEKbe8AI$1YrA9Z)mjBlAw-#pvLKPx|k)SoBB{ouUm?I7h!M)(5`O2%P^w+&#X^;{3G@fYS%J__I`nWgv|c?OmlvUHz-xv2d>8Ko1EV3o8oVLcSQgRyhA8_236NNg_DuG`|^+v^Mb7ig=1BoW#e2psAUEY`i>LhIntj&=TH73gM2t(eq&WQS;PY|<X71OjX!v-ofjiF#z02~+408OQ}M3KuW!oVDIGXZ_KdC8vnjtxtN+dWk`FIa#UpK4+GWr^BP|k>y)aOx-e3^W6Y-eG479#9cb<z0!9Hgl=q$T0w-`hh%f1Y^_;p+t%f1M=7<|RDO}NFb`tIRXUwW_{T%7)V7XN(q8WsK^SogZH8h3hN&Qvt%Q*Y9r-T;MQ0yn5{68H#gN4le*J(-^U+I-8d6*K+^7s&^L{1Sn%hv7lc>hYUDT05aBUXuyQP+{ZprM-t;F1o5H>j9)#+^~hrhXi<r>aoh>^)nfL!vzSxeagv+jif3b6ogO$GR@9`rdU;*5%O|i@fJJN+6eo4P`q1CSET)Yq)li4KoJL{{pQIszh123Nw%12_N8f@E74<NN{Z^v&4NlyHe!Ya{oAp#xOLn<jCh!;5%oe<h$1F4*yV$0gi_LI7DI3i$~w(_>y*kzXlLpMEfPsUrvM`TO@nWYdYv{EP7$FmF2{oryvPWrKj%gxPmq2O1)(X`L1Dn0{KnAY@SaX_=YFsj8*COh$*+?hx{RvnU!cAE%*z0JM2Hy`cM9811Aus_x@AG|NAzw{uj4owU=W>Fj%ie!I9lpfgT6Mg<-*Lqb&7cuE>AC|ezVei8BR+@;4MH3KdT8*cW*dv2=gXXYqMB!nh34e->*7Q;e5jb5^<%p^bm!$3CM3?qUMFu&n=EU|Ef-`H#np29a5J_@9?mM#QYwvU{@aHB9N|OcJ=z~;_USGv+v^<r{9J_N8ihH7=)VXu%`NNiSst`71;9e1a|PWr@Ucfv2|d<U+1PdaG}D^KO5p;VK88-n2hp9rz;X4^}BSM^t)x#ME69Yriy24(4$gc1y9Uz7#1M!Rys%<Zz_iN);><vQbZLn0W5@eebaus_2et$I@*+_&NN8AiKeAL(lp{=@Z$Wt?=Rz5uYdg39T7s;+5Ud7gi}Z_zVeQlJIk>MKg2bsDV3VoW#iKz&1u*Y3>}c#b)AI9<ktrc(TC)MO6Kcek$7nw*p>j(cKQxz@@+9vfr7K+l`P=8yih+v=u44R_6g$%U77m_T@Od(m2u${+~Yn$HjB{nheQ{tN%NRGb7M>q-R>MU<VBmBUmO?xhlcM^Sy+R?oP2DU7Q$cvP#VobFat`(6TdyJosLzzzMw73S;}Ug+wE0sBwSwmqjCSRA4U7*sCRH!;qr2Tal5-54VkO!n0mVU)YIj5Bq<k?auNx*4p<_ss;@L>lM_I|I;I%Ihb7jOd1x%RyaI_D5UjnlA7Fsay2rukT;xE=*!4NkI4B&{BZo0Ecyfgf?~y*G=(ijIQwk!qESN8X6c&kel&-j$tvq?b<u&T2d`*MWc)!&B2govTQ*;3Wm^be<{r>JPsE`E)yR7ltI5<5IM#-F-!H6dL4M2g2&+yCT8v0FE*Pu%nL~Owk`RP3J?%|&;mN(i?J`Ct?o_Y?*d_B|cWSr2(^HY&Ug3dQBD}x9R5Gf8M1Df)U7P=xY&hlg-JzObZ-*CJyc4Q7ritE%s`9yarRx4p{b2k{0>ypfti!8}*f)B|w9gF1mth+Vvampx)v1Gvvn2GPFi^UQbon$-)9nI987Z~SzO;{Ac(oit;^6a!2v%f>j5oA>$Oy*!XE|w(7uM=EMhc`kN@U13Jygl=r<uHVe#H6J;Yb-$#4iFT3k{J#Z^W8X3Rs(DPswTyI{qT|w8)uuZQ|8cug$EakA_o4Bop}V%w?d{9-NLf}rk6=+KcV9x_GCR5+iZ1{nEiUoM~6&3!*DSw8#K6u#&vMkF=k>A3;q_uI9DGR+4}%mXk2CfL0BYp#f7>^TUf^nl7sRB<*}GfZFxYsNmn@xHVV$?{GcL-c}&I(n6Qmdo-+an;=whIg~C&hy~TWb!v#JGphJ+qmp{J(#l;jQy!@Dgi4!CY8tXN{7(qHgu28k%yoi{j0jof}a(-X1#>&XxxPT^rWgSqajFU`1L8|hW9BI16qRt=|W(VJ}du;`&U@=Dg$QwP2N<5yTSx8yNT}PaRIkz<^mOR#-{lURd`8CGErNM>4PK%C4bj?3|s;9U>n1UNmuq88-gw@LWjtWC((Nqt$uVu0Y_NNk2BY0Cioh8f0C)W6rd6bU*^L{nvG#Qb_Tx;sEq??P=baQP)mkQ$!Fk7N%AH3+pIi0BuDI3D6CPdu#9$E1+%~{ZbHv8FLggSX0YK(<st>>QCx1k*CXmZs+Qya&ko8v9v{L0r$&}0}y$c35f-sc!qGGCph=A_UnThR5<*1@*|Oi>o8vMB>wHQ#LQNR^2C#87+&*s{qPZ$3M1JXqssxz4W}&U8%DRv8Tbv1^3L-&#}?`y#1UVy2z>q-PGEJO)h`qmd?d@Ud8&8wPSRV1m*5Ng4IMjAe7Kt9yxWPs~1$gt(p3X~;h7cbg|VGqh~1In7DuP;3ts%4}MjAoPuG%erzzfmzGq2Qqr_rl3QLdDhAXtY3DnNYUnDy7g7_tzO{dL|dq9Z5Ne8zI4N=H`{?h(RjmvX7%DbMYfM@vwu<4?UVi9VV7<oACJk0$Mf}cTCqzsIzeN*9ky|w*llA7t7pq+uA!wW`8o4#TDmz5zEARNLqqd}H|XXyuO=hp8{tzRZw;NzSzy++G3(EqPNykN!@T9v!Idc=u8kL6-dUj>mn|!b<+$Fdf;q)u8|G|T*UXsJRHYjiy>O4~T?H;w#PxG6s(0lvsXW%wQ>ai(@Uj-`*H%;+0i`&nacm|WdV^siI&V-gE`=*}5o;0hZkni)lKEnuj*@A-Ow!DHuL1h;eZyt3v-Yl{N1lpDx>{{otAVlaNar6G(+?zXuyw0=j7(`fTr8%B-<GbCh*Xp%V7ehq#W5j!@aYq4#IW?VGItVHXZMa7V4jK#42w0iyb`3<w$jewMWd&N%)2{tiu?u`SXj`W&cS>=Utv7bBKzi?U2<}uw(%4YE4L~H_eA|}3X<D|R%SH61ZiQ#7CCk^lFK;zLzLlRe~CVuHfU~D4+h`xAt6)Zr(jP#ecEXL{fUEQQNDn6dw3JSCpQu;KMKaQMbq&kELMw;=+ROj*|Qv0>%!J~!aqPCwNxkkQAa#__)u%bb&b)?t$!+X(DOBJMnUv!ty{=3!xXNCXy|S%RIrV(qj@2!_|~Zbfkr53yfDca_7e%_rzsINmy|i~+KTUymi7$OKCiFUUkmKGHBd%aPE<J{FHr?SRZAy9Z$0aWX9cjw>(TppZN+Z`)V4-`lSk@)`)fx}6KzImz#qP`bjDj9IBgeMg5m=k56S8yAsqFvXlNAeiyGsrZ6_4L3<k`y`x=&ki!gGs4AXg%-NfQD7gL50Ens0#M?G4Q7w_HrpPKleEUez}Or4HNX=Q$N3Q^O|Zj5A*QmToH+-?8SrY2fLfBU8;a}FT^EiV0`LT1qgv>%lrS`SXw_fCOvT!Jc275Hto0IQP0Jt;PaJ~0ns)4R465G$8@L|4ZUKBXmA?cKTpS!Wo<USucRjtQkNu1{&-I1tPR0|CntT`di_IYI$>Ocl?j(`y2S{`X!Rr(`2#Rn2NSuJvus2kSJ2Z8RA1nP=0v=}_x;qxPv!ojnPLGwMzZ#|B0D>-}d!TROu{O=!8oIrEFYZbM%d-a3rtI(C9&u-`<i4Th40UaBrC<{H*YLu;YE;na~!%{Q*4#!}9lO96pdY%~+wGq&8=%-r1DZ#$f&{KN@*oo{9{vdYpC3$%g#8yOc}%={SmiS0sdH!VLg9_FQ2^J2@c-BM{YQ@owKSPnLnN0_eZ!u@(G%8MSO@7f(pZ>HU|IlWoOWz#ZvBgXL7)n^TVw&qy1&1f-DK5|>nmyF-xcHR~Vi|qezkPwupkArmSI^6H>IRR!ZUPg_TUPd#8ox+H7=NO)3Svq!{z;FYza&`#!gWa%WH$-z<lhcA|$_2+EXI|14SH#FL=MDYjo?_gQ-gJPnCP1kGd`%Sj?7TmUMUl)K%_z-dGFz^08qLkmbXaza((@eAL=^2Dj>m`H{V1ADI-TB8yI%O81X6yiBb(`=f9;aczfA)Es>A#OJxGt2TBo5{`aQcPEJ)@hL7B+Had*&qCbBJrS|NA+bCm*;{~<pJtbEn#il6qqnS+FPVrP1Am1-}<9a_ae{bVo$6gDA@fb5{oy$B;LvnuLrLm2xmqo_r?#bF|tOH|5fEy~VO*y|Wkg4Q)V8O$8ARQNQ`h)eWdG9F__ATs7r0d*ok?-NIP#?$NAVRQ1iIxrli%ao=fcbz9LM)WFMdE^|s5M$<hvP0(z9UFmjx+lkr4~?m)!be@uvGI{kpgMakiH2o4X#HDn-lxWP^vi6hj4=ot?Xm;*4409wdI14XT&z|Tz%K5<+ZB@VTMwFHy>-l+y}@iCLfE^>=opi<wL9bePTdgp(rgV@BO3op!++IvOxzpUcco5t<SQOYVmwi*QGbl;kKcI4zok{6wV?~C_V;^1R_&R(b$uc<(E-&&5@VY8&BB<aeiLs+Aua2n!rOX-sI#4*c%&|W8ZB7?1LKh#gZWqugFde%F8t<sxyYzSLTow*{i6iLW-^>4M-x&{Y&0<CqN6pW?C8>>OC6DSmhPlUHm0^VX4iy2U|y&HOfW;nxM;r^S3jSLRB+Y<k?g{H%y0Hvc@lJ^#b_r((q%XH6VZ>J3xR<<$E;Y%w2%yeKQH8duq%#xk|ylw2|#{MrfZT594>fXU1rqSPdSG6<BJiQCt14Skk!QuIpo440iy7N=fh~-o0`tt6OKa)GX;g^&j@`i5W%GP;e(q6+%<PEJ~&vwt+P$}P)IWI`llCX7iX98#k23vUY^>yB+P6R<!$?1QJHO@pT9nPcKPG$e~Qn3_<0aK1Z@dzcfUJ*ar*4?{8_BE+cf|@Kl}6PPcJUx0w4th_lm1rD3km#-<R)-8`%eVaj|v4^QTb@>NU^UW8GTA9qnR<7dGj|wBU^W&8SJw7m{(D!4`O&W;9VR7#q9%qX?ewVnl`uB7AGYj_|T0NK2#eg$NE6I0&K88S6tu#PmyC_8gEg#>MQ!K&ItHhV@?lr7;Y(i1KkbwXkm>>ejZCs`9mc1K8?B-T}tG8Wu{S&5~h~lNc{;CRc<-`_Q!eq6X0+tpwOQJP5n}DnXKWmyp=DDl@T?O(j|n<?!Hlc{e*Hxqan-mMk$}T!SO=Vv&sbk*Vx5FVIe;SeJ~k)l4~O<mWkW?v}MeGrY=a_AF(`J6;)mmlp(c6Zq!7LhH?XVRY?|*yi&<-8wFBRuQ9OH+RLXQ2se`^HFOP?-M=o)&XHsG5G!VBp+RqS%SaW=_tXJO(!H@MJY}B%l}~Q#bsZ!F2bIcL6OYIBJD^lF>0tQ*$)j=t+d&t9~UWI4NThA8n~~{9~F2Z`ZPV7vXN4099@NEVPwV33?P!5e5dqJjj!m=?VwGoK6J6@!vU%{?&L6+z~75b6wNBfTvBEvwyEs7k~9-Q)ecY<hLp}4GfI`Ucv>rxRkmf9<@FeD8)<QQUg#4Uxy7Sj9zMcL^64me<22;nhJmf1pISvbX*>WYOYq=H(9w>yeZ4+o&&MXx?!H<jeRYfFFl8i8abY>1Q@=g$G;f$GTdCgBjS#_W&{<&14PpPd>xdI5|D<c{EMLKOF*pfL9TV_$X5HgMR>rl{3Hxoih?fbgGuz4Sj!}RI*<eQ9PQ;};5#61Rb=CRf`^LA6-v)yzN!~Xk1Zr{&7rzmht<!V_MwK2HWLa?azPC?Z3qdUI;1&w1;W`h~1Ep!;15E}yN|wne1wvbaEmJaY6yl57>b%Cg&FS0<g|Xd+CFgny42!6;gH6hg`m}G0VPIU89;D-36&`t5=nez={cS|_S$RNnjrrvaw%JY8$$cg=@L^igV9gHRgrvA;TeBo+xfmDV2?a2Z0JVWzBIG&&6lUwE_)}0#0j!!j*;5jdfXyS)HS9JG!_W+ar}qS9B&9!Ku<VEz$67EFR3A-aEEcb83{8znD7*DM{dG;~RsVHPChO^LQ;%Q%dr$m^sS(_Vrl1@?kP)7qmg))6P41<|bBob7hPI2#7HX9q^prUF(VNdrjB+fMfl#?T-{+2u0&WL!^Anp5?ImOF!TX`>gT6?2ctFz~f}z&y;?hMAG&C=l`R$+Seb&=7^w88qqvgIpCqcHfSYnD%OOGH{PwLojH4bnr<UH37P3*S4Y{)DC*pPqY>*1*$cj$<b#|zc)eWQBt&f_wXNZ%d9=>sKWEHK0rLHZ&U{Qx+P6abTG)c`nI6@X&gQ8@s{3H5y;X2?AC3ly&ka?hBgJ(eX^ICt0~X5l-XC4tEjUIB$pzN15y0}YR^h1uwFoahN|xWgxSXdg<rw|LOd;o>c5+}b*Ai<@^T^L@uPbzD%ma#wI#yEY`QJbPF)1eP&e!QwgWx|!)t?L)0ai1l_=d%A974is2B=?6t;p_v|)rSr4wLE1^7oydQ8MDyS6?{{nQ-Y9HgH!D=0-D(Xj@TfFTZ!r&`mT`38Jd$uFztyueX;hOroOlYT{mwC^)^RWF^?ALLN>35Un0N1Nq=>j!Wu~d1p!>Z)TdwS`6<wvFPzV9eG|9{F`fF-^A|{R9LiaKl)@~}JE^!Z8xC`X?s`2RLk$8;iPc9<*<xPD>DIixw%GK<lwCc4h&U!s{&&Q2NgGcq?>bIi50APcXvXcTSOUNk#O=MV&6QoO&*rh(9Gx3ayGr+&lmxNH#iuQb0r|;%U0z>I1bM|}HC)yIed(D2Y`a~_-y9xbuH#Ar2dKRyk?{Z}<n7(3e(K9@j?E4U713J!}m%t)g>n6=%^;pGAfF3U<EQO0u2=N2}l57cZSMHiO!r7oof^#oApN`U%?mh&)J=ryqgZdPoRS!T*Rf;*59!gBM@^Tk}7fwqsn`!wPQxAw^B`SKx?W;#Rg^JxsKRNvF_FW6g_<>mGWV(q3FKfliXn}*JAu~ohnJ={8n;Z?J=-{}&-#Hl8i<&VZl?Tq4ZCU7WA47-xh4MHP5iz-5T`#f}3(l(eq0oq2Qk4~{1A>+2UZzVjP3Ocal`|;>V#>TgqI@gDFrzV*V}b?l<SQ^Gc!~qM>JBj5%mrDUXzy3)41<4oc&a$mKf)N&qwrY73Au{x_#&L$kw1laT|G91`h)lbv$#nyn|ygiP^}t@9ZzE9uNIX??AjBpvJsSpCYO!L2&3WG%?hzLl>;x7D;48c#Y{CKXthl5OOrs4tx-D<`-xq=MJs)(O0~!uNH~@C(n_l2$r1}t)8lZS>zo_Yu{oLSmq>H<R7^Dv-CJHDAahp9O}<&P^!n-zhb;b7klC3BbsM$W=0yctJBr_k1(BN`ujn?yb<L2rGG3qOKP*=4No7lstIw6ik2*-D+*wz_MKg|{FiE@^%BzlS-_j7<7xA8V#~yzxV@j$r)UxU?Z43ixb@qE<|3GaERd!AdioBT55__v0-TI||zG;)?qge(APf}Ay@>EObxYDmF6bdy%eC^!jSIYCDda@R{^4V(%0zD%YNdc+0DCF)DNl*DXmPJFJrP9tBVJ7A0KQ*L-0@7wpZ;Gx61J-HoM%b#<JCbZG)Hs!_)ABJ}H^}LrHExKJ`?+;e$U~!-rR2xKEY&e75XF<FLFW47RaxDc{zP7LXLen?-j65UpH{xr4$~*PN>B;^PCWT3apVjWl@O7mW*w@a-wzK|41V$StF98WA|+vgYtEv+bKIolxYoFMx1um$_X0qS`kI9_i=6bvr33}L`L;+;^HWAt|L0W&d8M$*OpNSH96C>Q#)0t{VJQqPsd!F*pi4=Eh416sZcq)A#psr?tv>!GAht!n0;I&gn6E(qaZ~|Y(yxl|Zf%5ELxoVNJ}o8fvnd!D(h4&vCZ5{&Uj~C87IRVx)?GwMD`&!7^0$s+!D4nL7^i4XEOV)IOetRV++$`ntkpYYeS|hzO&3bs4Xjm0x94y1mY`_;Hgv-?s|uIFld94$X7}Qx7wTq=Y~7b3e1UOsq_rtW#9C9rmDZ+QDZ*5o@ugc4TUOUszA}MT^?&St?1~+`2z7v`%B^m@6ZU!)=B%Ayx0nm2{^C(qRr{+>b6GUqH@(ej<y*{b^X~2aW_6)-J(mvqdD%KwP3qRRBoh<Rq}i5p&r7#0=iLUh^Uf4Cu#zc7G-p9EQ{<PJD1~xiP98gJf>r4S88K|h=hvNuwAVT-tVD0yWmes$zvud@-^3T9WIXgCXmMb#MDaU0;SjYwuc+-6MQyLHsFjwkktV_zTgAegC5q+7^2%yMH}$L#Ln>6YB*C$?8=}zKVr_SdLTif!y^bifD#V~wAp)%u_P3m@Z<VEG)0M@S<FmJ7fp)!rBToh0f^U|889(_HSWXEcK6;t~W*d4EEjOB01wC3Yj14~5i|^p61WP)BvAD?;e#dx}F?T6P%=g;qhkZ1w`t9)OxQ1E9{VRG27=5uY?^xkn;YTe`3VI4tIOH7hf=Z&L#s|U`9r4QJdp_TcYH50G)bxJq2o?9Dw!LH0jYg})X=IbZS!$w4ic(kApbj4T#6zc)v2eonDYlDwSmjbFA`PP&Ek=067~0W}A_HIcfAc_FMcdP&!SI!(TV-)c7gbnnF;Ce+{bdNnt&CV@O3ypa$41lh-pB@(tmrW!M~jAbU1Jr<L{{FTT^UF0?i|~gst3KBd~IRsRvmltvb>>Wgx9KILab`ea1qwi0!_d!zJ>*W_v%V|Spv!JI;~2Zvf-pCnII)2@c?PM$baj0cFO9zz>AWzvLZj+R<u)G6c~DZoqf<l>d||P_XHf4C}#-4Qf<`Z%`ny18M?A~oGUWRl9S_gfq#;(KKYD=zan9Aj$e(=D={uh#U(}Z>a<f}sD*eBNuk@K`-lj`tW-dZBwlqS!vphgOfHOJe<6XeR0^*!&2nwib)bCbSwi$3-|$OZ9A?+m@~2y?&XvU$okI3DAMxtS$3`;3<ir?adE*TQ#vwF?0awe7M|PO}*tbO8_cfU$?3flc&#BK&d$dDd`e2}<LrPAh0Yu>x^<l+PF%Ox?kUu+gr*P;?4I8$dX_|sBOf2(EvsuV#Y~4Mydz2BUyo90!&bUBlo+&?IO(I@obdxTvvW;bZn&c358#g`wHhQ4j(g#lzuuV|+Rg{#1#(_FSnU|C-;8MRJio95PtJOx`QiO3f8mHCB(gIQK)6lJVuB8Xqg|q9*Gi&3x+MLk(nX66_aoRbMsZh)3_5C2Z$V;ia=J8}v;aE7LG0bz)*pSN9<*<K<*^YX>wwCV5?Qp1b(pNasZh9%r50lI3qn$7hWKP;z<o}Z33p|k;w!y#Gj>O}pRJf95ycNt0mJ(SVn+vsDpKUw=$6<%^t>3TY+pU8Km0mhYw{l|N2ac1fBRWr2IE|)Q1azBP&f{kEN9vUp%D-@iVu&0N{jh_SJ~+gmZc=LzfDHegn%13d)!ErTU9L+@y%j$Rsyvm35hEwFK2^lSflaHbx%UMCdNrUW2CUSIEc?9C^5SwOD!>ZvNy$QFCvcS?RW3+nU+UM@=vC4_&63*h?1$YhzaS?qM~b+%Karav87=C{P@u&((t)cm`$A1K-LkeD8yVKwR&=z0jpJB;#idtfJ#YmP%QOcPQ<=5EzM@uwfSt6)uyoDaXy$V1GvM5)dLoIf4m@_4A1|K$<I5xF$|hp;X@^64ieP)gL8T<h%^(v=`hUY5sT=|q10#T+cci46UK#Dtl=rQJZks25EnMzWB>`yJ3hnv}L$B#H{V}d}jR#ag*SM9X)xU6f39N&VjSg0{69s-ASnl8NHXTQ`=2?RcM-}}pe0=-XQ<A&X1>hiN=^>03oa8~ODw4PIP-Yu4x(`w}AZ5`dRfsL#ae5c(2xdm`R?--VI#1>cddqLW9k<)`KWjQ1_QK8)OoxL5{OOtwqH>D3_(;QrS?wU)4#g~LUF<-$gOAv8QY6mnC=gsM)pTaW5f@)X)`r=KCB2v{WsiMn#nt-)+I5X&d}!Vb@B~&jt&=o<0THm~ur#vIiD4#)&Y0XaEI|VGnKwG^riZa6^e}kHJ8n9&Gyc}ufQ-ej``a+#w|*G|WRSCT1PliM*f*933Bb<?KNO7!zjexKY>XK@bvK@+(`gD*d%u0(VRpXAFgw;THQ|5}OThr}{C9I!A9N0Ag2lu3arGisSeTM*db8tjGJE{XQ8Jx2KGEguQ}BE6fNs4={-8mu>v!(+Kiu)&x-xg^U@f50`_#DT=s#VIo6V-FNOvqJKW&LtyV9ohB}!0`gAtZ&GQUw2*(rvk;XphGK#nE4-chjU@F%iLTWo$Z>ppX!Wh?Q$85Q6UScDB1UlO$yg3~Q_e$6r=9~>QHlF-)SQ8(-!cxAb;HxqjRE%Fq00UQPOP+Wb)ZOE7{R?hH48DOHziC;IA#lN}^|AZi!a-K&?F4CrjK}o!C+<fai|I;z|6d<>U<JVLbV`^27sWetW8>ze9n?fZTUzqtID39`u%Z_}a$aCiN)}7t3Ebwt#!71EkC?-wxgRNj-5%xx*yNtd92q*8^#o6iWXWz#!PQMLHGMT!CqK-i9`GvuGe<F5WNvjqhr|>wz`;JQ;!gKp}r(9?h2W|iQVHd3P{h)Q!J;0wn-N*Q)l37qc`xLLVH%b^481Y^l2APxW3X@a%u84!4WNqTBEFH(7VOdXk!^VOxwtHSZQ%7xT?xU^M)Q%^%exTK>z$8-_uTC$|PhZ5(&o0lNUDDJFZYlQ8z7nh9pzSp1q?-TQc0<|m_x%iqQpi5bL4_zOew11c6|8dE4Tt?c-Ps=X54rEtId^F0Ug5dNtq?!l;oPG+RvPVQG0hu0f`Ko@$&cVS2Sz{CPIlVc%m?$vT+^z1u;J3;yQ8|n+$!MHy6>7)cJ<VE?bLPU^wzs!+QRKtRbCD3Wr6Ts$z9pKlDo2dRw(@+<xH#T&8(jI8_>vYcq%$M-)ppLx#Djsy=CGI<nRC#K^GLkVJF=0mns5!<fP8#svRTvtlx#D^q}c+7O4|0b6yDIi*utBJK%n+$<2=<l@db)kB^U=%`Z1sOZzBU;`B1QHQkwlIAM{Rg|jNVz_Z8@1$#`)m>#n*AL_TvXkZPfXci9?vkF>yvC8wt@*o^#X3lR+?>1l6sE{s_Y}A{hCw9h(4MnD?R5+&A<nmm6sPNP2R}#F%(HX{X+HWHjH!863R1sI7824;wcx0Wr<LY~dfm4wMq%x*Vpx&&mhVErD3w&y7d7`L8u(F^znCCJYFprara5)D88Hv%7>GbAniMo~|M-9(PlvB^g>$Fa){Bj%52a$TlSybKQx2W=0ly5UVTg)mss|o<D2m+LWfOQsA*GBgCE!yp(2XWSkrg^6wJbW1Z9fhtr`K)uV|8@5IM@u1m**W1ns$u~~%WN?u`2Q=KJTO<0M%)I$H31}PzDjWyc5kCV%nWLL9APfB)^YbZJSz2FMwkz$kW5lsEWjkn!F7@cNiZkr)%9?}E+%cdpD<SN6$_!c=dPmkVyAB+Z(?XDH1BP|RN-1@1K~1Lm>9{pHST*^iBil=5RR!J^o~jd!F9Co2YCnzm7-ZJ+C9AkUZ`Hr0DT`Z(v`XdBxoxPk5k>{5;$tJicH7*2l(@^e>#4d8Thrp{f#J#FQ^~BD$GAjsny?-v02mI$6(>(Lmn*b-93vvnh0DrhasLC;SxK+hGt&ByTt_A(8}CCE}_h$PZbDVF=DVN8je6)JafTUX5oW&vu+Y;o`AL@g|uoV5=ajZ4?1HKMcrPTB%^N41kxIuiuBQ1OO`wub#RBJRyWf}i_%`0jY5^HU>C9T>vS2)va7PzG43|x!fU0C_JC9Q!xdx-M*?S&mcF#n-(F^fu)NVbo^;SMe`|0?4@<twQdJlnAOFZM_6gsys^fiuRe(e3hOWvg%`3;7*^=$F#=7@Bbfy-zlA2qIpG7BCDVwV*qcf<5^5|axU7megRq_TIh`;|4lScp3#pT({`24xLw^8QhrwWgj-#I!`NsM_F0e2mDSDjL$z|<?0s$_chD@eny=&95|`f9e&jIf(l;K7eJs6T?ux2eE`-xaF93^eIh%_@t(Xl<p7&$6a<j5r4v`y>b6^1N~#%D7Iq9d>)(;yJDZZ|R{}!=}9jds_eP;9Il4q!D3`w*d+SwRh`y;Z2cEujpt6f>)Gtbmkl1AeP&R=eDaRI5+B!+)6bDQ?EGX7aff=I=yN*w}nC^o@)>C;zvbM3%59UE<0G4jn$<GH(gw}9(1M{0?ljuZb*9Gg^^DE8NzFn&mW!$zQ2!iv3sy@r-FC&Ca`;%=DAXCxt&?vn|u8Xtyuo_2Qbx&$=+*Zaj%&aKG2#uh_lYI@=UmOZhKrPg`>}CTGs{y!==gqq)oW>cRXw-NxvUOLoyzX+oZPj$ADCteyka{`;MvIckC@2g67zmSziIx`VBxxu&@v<Bm%a4az$q7R~6+&c=ju2tq=`XXjKN$>bru~X94pT_`rX)?7A}Z?gg$_@aFYmjd;ncq}BC9gb`K<N8$xGKqP@Q)I3s2=QQJrd0@B`NIgZN&5Jge<y{d&k&nd#F|JqHVv@=%f_gM5zlvZL*C^K2s9jQnbs_!B&!(Eetl{Ud8{Jmzh6JrJf{lxFh9zusVsj=i*flw^6%$wwn-@3Pgb7^IG1uMCy!!fL60EMVz9P*-2)5MTjLe`hR|RNQWH84f`W31U^=YRYB;#=&Twzoi%x$)#3!QtvV4SQH*PP!|8;4@0nL)7bbz#rY>7Jmb(Yo%ju1)14`ZAS_Ku;Wy&B!W2Y(RO{>bpm2w)r-Lt+|vfKc{&*OsDDUhF?zZ(_$^~|3hPiKGkU>muGFyA`9r~o0SiYC=}t)wTHo5x?E8{hk-xU_bg%x4GOc#v*B>>Hbgcz))~{)6wXtvk+^F(u;v2W`cbRp=z!OKqZ^3Aa0_wH^aNP0JUlgtD+8~?@2zfF+cnXAMDa!g@wr)Lml|z1u}>Wdaim8)2~_SY<wg-(E|p!6wo+r;ZI)IOwO%Uz()cKi!M+-zxc`h&k$1dD%ldVC2Vrl&oW0a`7dLGk1zVf$vO2PC<B+UA5~6QZSb839^T_I3YhtX`jbW9p?RR<_AJ+n7u1rtuTh02AHn_`dLX}&xKO2xrE-jmstbbLxqF)ZmpS6Z<(F)hSC2QE0HxO{#WrQ%0T3%<@m??7;+>4?5rnSURqw{^ZSWN#vr*EwyONz|~!weQHzpt<QlBii2tDT0n*X0vf*ZP30&Qn&A_<+@K?u6a_%5&b5;5BBwkg?LtG^FR(HOX$!riq8^73@xE`O&K^nXh!L3(-B=kxbDf8w{v*f@j&af_#`!_C#q;wjw<go!IS}>crC^XYM?s?OarqwPWQvHq>iTOx77yS0fONZ%%E!z|ac?CiVgdnx&)~ZjHFFjga7jtck?;#nwe69yA{CR9nfM%Z%)fZd(TfG4lionw5bz7-B=KwKCsr2K$#}n)5VDISsI(^`87uHusfK!#y%yjPl3uCPwoP3>sR`HJXisq4yPchfKx?WZZ8@5$R5jM#;!@hj>3`ZV>Nl)C|XHc3=R=LD;1(F}{s}{lPN3`;R=BrC02{GZ^dyd%?TnZh&q=-UTQ0^~LGW#*3bK;Vu>oO?vMloEST;KmAF#OM?IYmwy%M010fO?mo}&K4&RtnLMDkW5phZ2R4!%zyE1Ycm=&+cJ~?i8Gtkd<Gar}HcRsW#Acr3?a;f=Bv>XHj7A;Ez6*n2*D2lAVZi53YqePa0(;jyn5yK<cb{R4w1O=Y>~CO})5zHIBY27~LU>O_hJj@Vv+ttdMY7Y<BDFFyL3aw6sMB-=?4Yir^;~TS(>dTUUz1?54n#0;fbQ!Q5PSqJu_8C4O+b5wu>hXfF)S7t!wR}30iaLTFoD+CE6yf{A9S@w83@i#VV_6VG$?Sqn2%Uh+!Wrx*k*VCITM>)c>M@{UO+d>tQVLF9_WZUs@E{N0S7(^GXFrPi{+9qBG90lcLCVgY6}W11v?xzThI~c1WrR9Hai~?MDm>xv4cc>lmeeKfI)(9$%h+Igt&;MlN83w8MV{;C82yC1LP5>>=aN$&qI#n#2+~xW#)1iN|@g96&?iP9ao(AD_39dC|a!W_d6K;953jgoH4n?fn+Jrhe{~OoZNhq9C-o!foF-z*6@L*6$ci+6PwWpw9q>gD`<QA>U^g~r8fnEou}w7piHL2q1?l%nX{=%aF~o$cmD^)l4KcbDCj3-FW1>li}jB{^8|Q)m1KZs6r4>FCXkZ_IRR3@gaydJJf*#XOaY`^%s3KUA^#ThofbAog5hF4nqt;}E^ZNQ)ki1_%<+R?jO0{4KMm%O5C!tv-RIGIB?PKW>%eQT-l6KH<<L8Q@xrTj`k;4;>Y1P2YFb%65z2Tt-rt{$qo~&%wkOGvLm6w|XgXMX%E~t%V)<t3=77n{-%<wQO|(UR_Zf+olO@*s2GtET3SI;519ZB;(151t1e#O92?nsW1n$9%z^Hxz^g+;+DPSI&AR5zY0IMu^F-&sQXXy;o5oIx(Fc>+VjOmnwXX<`D0ED5MavKb^P=?jbl6_8<mpF-}a!)v0Dn1VNPYsnX;D}+V>!lph@b2HSFa>p8juv<Sg+Iq@R9#q>doqEQA0dLAea;AId;v(WkS<^)*)XGn2Yqr)VEH5hCZFX)On-PKk&#79B!tEA7wmA73}MlrZyZ=@xKNre6Id8v*$Pgy^2PA~PM66_uCp5E?s_Jd$FW*RsE)e(d<Dd!nlR^>1wX09W+xE(9~26x4Y}atfIFd@fr440EafYy-=tQ`rCQoS2>~4f3l+UZb@v}LhL7SMe);J96!ty|{>4-Pbc!O#wa6pxt->vcB_u%Z334Aa66ig;+Q@TmT+H!(vZkVss)qbFf|-M#!}6<X>HrrqT>jI{pchrsIl6)fF8Uc^t1u^kS{wkciq#<so*4@Z1%SR(*eX*NKx4p3DPfcWx^T^5_@H~onift_nFix*k*+l58_-P&4jHI62f0LpC1HAZENpPXeSoN8f$70~0X-&bHA8So#4qTJDP1_^96-SU@%c}rRRWqFc+c3zSKVwUc#ZzE{82g;HUJtLbcQj&+62L0f4TcN5D}E|Ij%L?6=4+Tb8<{}G|Y4fqx2i>{9i#c#}@"""


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
        prefix="galactic-mvp019-", dir=root.parent
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
                    "Le patch MVP-019 ne s'applique pas proprement dans le worktree."
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
    parent = root / "backups" / ".mvp019-backup"
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
            "Prépare MVP-019 : commandes et événements adressés aux factions, "
            "relations déterministes et persistance."
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
            print("MVP-019 est déjà appliqué ; aucune modification nécessaire.")
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
            prefix="galactic-mvp019-verify-", dir=root.parent
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

        print("MVP-019 appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=13, SAVE_VERSION=14, "
            "RULESET_SCHEMA_VERSION=4"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
