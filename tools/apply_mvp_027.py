#!/usr/bin/env python3
"""Apply Galactic MVP-027 from the exact post-colonization baseline.

The migration turns a successful colony foundation into a playable colony
with a stable identity, ruleset-driven initialization, independent economy,
player ownership, persistent provenance, and an addressed gameplay event.
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


MIGRATION = "MVP-027"
BASELINE_SHA = '581418f2fd3ec48d9f5ebac1b208f14630081261'
PATCH_SHA256 = 'ce8f3f4c1f8dbf147ec9c39b391c14158fce988145a2608380a0d6316c6bb88a'

MODIFIED_BLOBS = {'README.md': '91f5171e3f46ef21d2063473380ed308eb845e16', 'assets/rulesets/default/manifest.ron': '62253b28bec559da6e824f2c12849440a89f812f', 'assets/rulesets/default/planetary_analysis.ron': 'f66260ca6cae6cdfd665d3c09d3481ceebee437d', 'crates/galactic_client/src/lib.rs': '8f68d166793b7ef5869cdad62416e0c8b8180892', 'crates/galactic_persistence/src/lib.rs': 'e28dda52377b37781f99f09aa23e83ad210707f9', 'crates/galactic_sim/src/analysis.rs': '81e784c8fa610bcb4b9f09bb57dc34c5e680e15f', 'crates/galactic_sim/src/colonization.rs': '5f4989147315fa890fc2635a8b92e63c409d3790', 'crates/galactic_sim/src/event.rs': '7dfd8f602d4233f778a79879ac53d18ad84bd75c', 'crates/galactic_sim/src/mission.rs': '813037958a3a8788fcb382a035ef0b1a1f7b7d9a', 'crates/galactic_sim/src/ruleset.rs': '87f02f2f5f6c44839b458ab45605ab94ce0a9bf2', 'crates/galactic_sim/src/simulation.rs': '5fbdcd96d7d5f69f313c6df5b5627b718881b91b', 'crates/galactic_sim/src/state.rs': 'ee827b33bda32cd81e6b41b07941cd795c0d446e', 'docs/mvp_architecture.md': '04e396254fa485330e3b8fc069f749d0c1d3186d', 'docs/roadmap_galactic_issues.md': 'ce6f93f942b9b5f9cee6ebf148f046e5da985e04', 'docs/ruleset.md': '7b96371eded21853019d9263ac76459366ded684'}

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
PATCH_B85 = """c-qB%O>^AFlHi@cqAW*9O?MOO4|bDmJ?TA~rag+0ZLKJ4W7g|vf<QHir*5E)4@!I%Bi?1_z8iDdh&gP`$(q=h(_Z$>f8t-VnOPqw6bk5WT8>qOO%kZe%F4>j%FN12gwcG?_VzBLocWJmogRPn?Wwm6*+u#LP85gyJv$oA$Ag1`=S>a|FODVw8}$2={r#Q2y*;z%PPf~w^#03V*xumas5fF={4qFS@MPh$Ai7v^7V_(e$2m)apjh2~<SZ%J&&8u6=FBhhl(TsfhkhO<F<YfP4zcHI;Rn3Op!c;OWf}L29hOQsJjdt3m7iYnB|XUCNpx}dF}?ffu^(cV6+v+KYsP{Uhr@x4A$XcaS?<R<-$77C%<ev3-+hb)tmWOuJmM*PLii_(B#t=yYf|_)6TVpRz2)7%FF7ES<s9*V8Ku$k?pK^h2q3?G^K!30Jn(k9JKfJeSAVkK{{AoQyQH{A5H?(F`a=atz`Mw@sH9!A1jNF~&jpq^B?Tl%ma7Gf57fNo0leDjo}b{mo9E#<gPFnzM9DZ=0z8%xbz|(&gTFIqk_2xvqy_#pj~4KM;D@`9D_9JmQ`Xz*7PvMEyh8}^TUZ2yD6CDv!h)s#YL%n_;gKKvi<^u~(q>6O9KbH_{w<FX_iU#NpyyE>F#w*ib3}&cetI(#U8BrPllYtn_@c7Tp-Cb|830ccV2dR13B7XRJociTEMT<L&7fy?ljV26!Wyo8;4A>)7c7phfybC2T=X{{p7)3iVIiPNz*)}IG|H1S@?k)jte@sM{7x5o!7}n96?~ZiXT%wgE(JejgjVrhq8k&tzW0MXBiz&)5<vg%KL8#PW$`GAvlS0=30FS6`xvA?PMuu>`ymzhd%?>+mfZbc;!j}k0ub>5>6B$0$PGfZjP8B~j)n2!EKkMS2@DLLBaX%vJiSJ_0mfR=VqfzZ!3o;#beFK`(4hyM6lcVKKpc1pvz#YUCXw|)B$rW6z2r1AZznWW5t2X89;XG64Sxyw+%FdS<E0-*bHE1q0QAfz&7=aG42R><#nAKmfj{^8MOA@q1W4$%jhd3$>y1I_!T<j9mmSvGVfZf!r|g03@u0V}htIO$iZA`yHEN$Ep0XqP9O#_`Q<N_TeGoTS5B;LvWzgGUyPMJ0Ep06`c6X5Uo=+yjNk8zs!Qm0_2gA+i-2k$k@(rED8XVyo4Eu5sh_ISELrFIMVAK=Kv;-ZrES9q?{~`h{87-pxX3EBW_1b?=uNCu_9p_0Qg$7<``IL2*fZ1ZoCeRr+9ndfM*ZvUxnim{?1tRUr&IImemR&`w88M3S;DJOn=I_L?fKNv}!hfq|RV-*+So2|D@dN(5D56Ce#g~BPkL6SR*P%fO!5Pe$Kft}<8j%|8_FPRR{xkkQPeD;7>DnGR$63ljr}Ok?ZR=I?j;AwR&U|g#cYeyRk|NsxYeTcm0uT@0g!fPKceNmIft87DCBz*QJ~f!KSFp$2^r}Jz+BoOg<4Yf`l{^Y&!2<V$$5|RY27AOyGvMd7ZKNn2&L@ZAU;?V^XgHZ4S+>`DC_;MHw?l!M4EM*q3G33&!2qqbIqW6Zs8tsJ1z&XN_bL0vU-DD@b(ifuV`q5^EXIDRlBbjgPsk{+wMTR6C=$DxrX03B9?tySyY?3auQWE-OXKj5xDHcc+MJSI6J=LC><CHOWlx{M0O``tKkHOE<Uwp3USQ;5cQ74bf2HT?Dr3LgvZ-zYe<Wzz5xOz<eqb@47}qsvS2HQ*&@A=eRT`NSw<i0o*lJ&KG_GK(Dg9ENYK6iNlp!WJY4*$X&p0dm@xWRQ0nC#v0kfWZloUB^>%2$Iaxgj^A+x~G!`=`#I`(FH_Re2T|7$J)4g33t?CqDv@;wp1-|jFm&Ch=XuFQWDe*mjVVgxA{YE-aZl%ZO<0qrpN1K_mIF7TQr(*o@)WKQ<9!m65N(<$gpYw5Khgvrv6;_38jpH#&2P_*vbtED?+(Q-Nkjt6<VjDpuu@K*Eyt#0>O%b``EwfE*Mt6W1nL~Rbrx)%u56r$MM)S^22ORiD?MlJx!J5c7+?deCB3v3{KNn@YUud4OFtF@M&DSJu3V(WRDEN2Q1iB$~}8@izJlB}RwruSOU+lq7oGk42B(L6~(?1=X~G{t}{S*NCA9#Fq^y_B*U)O3yRehZ^BQ-+w7A)MAVpMiaT!Do8Cv)Of&q1z2)o8?!0c2$HVN6eI?CAbJc5NS<-K%MD71XX@OhbYbXNfOVa%jtA^z3L!Jt=pe;t<hg<-viwZ<wnSdhhgZCJ#T(6_V)*e?HgeODEDsI&{)`z2WUeck_q|vvGm(VQxz`&)d<TLV=#hzA@G*KD$ST5hv*_*!rvK?jD=AafWwsDFks;rLkZyZFtA|iqV$U}RFyRM*eQtI<;^!KYGN&C@aux3Q1saMD3d)cB|Y|vXGxI;`~?p$@lEyb(=<&|>1L=m@Dsc{TIjO*8T!vXEvr3NUH$TJe0RwK2_WmNkPoG-Y(5RGe%8-OfObvm)6GbX;g}Y?ePXQ~=@2POUrLMn1)7*yPtN~qZQ-aVXSt7omXl<8;pbu_$G=Kz^*<H7zy_&5&!5M*jU?%f*m?&eV94%ZpV+b)mDkadpRG6#F`h%-(oDKfJtJ0w7CLb0)eO47&E)iU_8tSXzY(qx8;%saXHt*-O(+Nq1w^?BXb6rM)|+9`0h{lwiVG29U}wi~PG@gUU!6Vw{<|qFMnljs!x5;o%5eMTaNvvOk-xyuiw+miP@u!2REDv@vj!BvO@k%{n)u#Mcg>jlI7!f*Ax~jlJ74jO0+#;-1^}AB#3+uxIQ@AS)G7iJi^9(QYmS>bs7+jk{R6ar_78g$*EHIFTM$~<6uT%bOD)9S^)^o4Es$-eFzet?f+7D=J$%+HJIHO&>L4V;*`f4Hv7ux(z~Y(YXlxUM6E`zViDI&EK$xRM%KjvsvOBAEr&f;Ma?6qZrBD;MKNH+5%*MlUjcg^ZCdH)1Ayh(*kft$Fr<!Rz7}F>21R0%~=Fs;mWY<4wEo4rZB8gK=jg+s`QrF07JG4-ZfJU#9j3nMiM@XSze^`<THufy`SJ_nplqVxd@64U@*C~5g?%2XND_NGg{z>XeXCyI*++w}S*7`|94`UH*pU7dMX-Cjr7}?kHt_f4ITC!g%mm>4%dCmDO-5zrhYn{#4on0?aX4gDu1m;}e8catkgor#a9FQkI931`eJfIMoiBLYTVX9s@CVRVEe}+Eof(Loc<+asLhFZa^KD9#wQWsHE$57(E<3$A9Kxda|IU4nmmZQ<QLQ4#6Cn=Ytl!WbMKrAGoq!|(L6&ZDZD#_X-nv%uw%th7Bq-r~f+D@8&za(ikb-d$b%1sTEPPhkh0P%WO+88}neS;fwu_T4-lAwxlf<|$O^rNL5lWH7I{_v#?qtyZo@&IOua6G}Q`l%P7sZf#sGDac9(CAiuW%N;-wXuRVAt}K$N{W^eXj>(x=MWqvG!(_zpP+|xFdhklE)y`VI9gu6l(CwMpSfqoYNR2EN~~D&Y$gn`z+Wsd9v{x$a=wD6c{VF8!ZLibJrpD)npTnubq}_KE-cvUXa%OlDo5it$CE;gjv2n~>|K;!%@%$U2N)7C(dei!75=M$^8D}`y}Zs>MJk<Q!ErpuI-^~)pYZHw{O8X)g3hqHW3cRC&;x7QMQE#+Wa&^WATXMRihSHKrrleG8BM!<!D9qFxIc&=`#%{<NYS|#-)I4chkp`}q1h3a(g@kW(y9JcRi~g?PbI3b%mTfRM9pf(-{V<o=K=T<8ouS=U?dVf4yHIyYO)6n(=?FwkS2CauAy5MF%yVWSyx)0B}-l>uQJ1NjiRs;x!M*5hXvIV8?7J639r*$B%z6_M`q48em1L*yD67hDwDX3+2OaPpMw_2?4WroO+}!`+Pr8fbZ{OU$fzv%uTl>j4$1rJPvilMQxAyI7l|LX%3p5{zO6kPDd-v&w<y*q!W(f^4Y0kNbzA4s*xEc)sBBc^O7#b0is1G4_qW~PXk^-da9dT!>e-CAWwyLt&HBShWi#8P$3cD1x>v-t3IToQr_J8M!}53R+8(ZjwiE>|&(mG8T1P8&VmaUkhx-8rY6qj?(Xc&G>w@Es(z;sW8jpK}KI_u|?P!<|sT%RdN^H@LG0K<<Ori`Jo+KjF2Rt3}>GUh~)kHXo;z#(3(9}`e<08LGQn`J|kR*lhaPK(rbAORs8c%TCuY^qr_`c%GCZ*63MF<609qBo_^5aY1D<4ptQ6aC;ekuJm`52V(b1)qiFH;^wB1h-!Dq5W^Kv?Afl@pL2d(DHZ7=-Qe=7)GeF;m5d7^=)7ibd9<iUO31Czcxl+AWJkbcq3WDY-5>snDq=ll8!ArX#F2d1c^va~iKDuYh=e8m}d<7+@0`uO+XD-!_FdPm<M*5#a1G{Lf(PU7<G*C$vk!CN;EzbYixzZ&v(R%Hb=(%naCxlnanM^hln^m^30tep0?bH1QC4b1VU|ZY@^$Tt$ZoJ|dDSbxct}BQb`eYx<3m7h|#0PM1#B)<uRTN_EP>f|!KS+l)>#Dn>)$%N|i2^ysJ-2W<yb&p#?MSh%Jfq*9Y*c|Fx=S`ibKM<zDe3T>y&6tCvXl+kIMN1n+=7yELbjxhQMy(64`v|KIL%o_;QAzizRTP^Ky0quz$)19VGvH_Aj<T0A7xdYx08`OxEMfK2DQUY2fw!jzjUIPj`&akIoM{|j_Cpm<D_LL3Hqkm_MQQch3i!^31K&NHs>GX#<D^?<HM4q>F>H(PQc$S%o)(5TOJ~8lMJm`%MH(W&XpMUK~3nciIAuD<T_-M&2@K=5i<u}%#x2DN+>mp8>9OspC&z?SIeLIh0Q&K&TuYuFU@Ax~x`!6+E)8M_WdaW3LkfJlwd;_*wRn1JNr{BJO{ZCKoeZ_1FOo6I&f1PrE#$_^#IsQCJjZQGOgCHd&ixfb<&s-yV5vKd*o$?(xkt*i3>q<=7l&D`7t3?!m7;CaB%9xX~2&)K0{uL@34Ca)`;*;6M&Fn3|={&5Uyvu;t{P*DdLySjn@p^2nziT|;!R|ey#6y}v&sa2%9yc(R24Dp*h&R#Spw3yRHlQ8tSo*6O-<IdU)X@HHF{>I+gmf$GA)~yPX%vF<nKoniq$0>wh+dS<!1#$k8FY3(l?aP~b&L2~`zK7x+(2EaWK!C!jvi&!_BLuNGix_%j7hZ(aS#+{tGUGjWhljZ0kp<|XcTu0egy;6zvNmYw=bmu@t$i{=182r4>%7qy=rFylj^dudg_v)$;cXU>CIXv)e;Ps>6D_vYQj?ck$RC~O11SJ9-(F29S)DAg}lYyr!B$OUDln2F1xs1+Wl*0qUvkC=#{p}S_4w@|6YT=&>#bwHrH~nX$K(-mDYA_(&}On$*_c-*}0n=!LBXFbU)gR6vy$MVR6<(&CW{f)zi|=IG;M9y_M(6BDY|CB;zF;GHwP&t!I$SO5`oGZOSa(D(yNC5d_)E8xheQunNh_e^f_+001?V7^f$*^UIhHjoj|M;=eDjw{=`b6cyClr|9hv9plLs)JuwdRpj2P0KXlaJb|N=oiVW>qkQ^ml+S?Bc{F|0@QQ%jOXZ<iR5^I41M9Y9>RbndgDtGx1O52OHe013B&;8_Tw4Y+OjA19w*}5d96{G-UPJ2Xqn6i#0)$@E3~K;;j~c9tN~@n9H7@Rx$}1!jvNlpjtcBRGtUaZMG?~P6K$oX0z&fAh{$&RNQU6_QI&`#tK$>BHG%|udJT8`PuB?ei%6~2**n7={Fw4&?U))M9*6d@+2F?D04w|_Q^o9YnLECXf7k3+a3A<6gTx+PvIcn5PEz~g-l59oCMe${88;On;daO1S8Ht!O)*X+ABCH~HBc91*nT#0fvu<HFsJn@CnT=CBI@JffyJ#h5=z&#EFfK5FiRsM5z9+`%>6hZobb7o<xgXx>OSI*Y&7Ny)LocpHqq`0AF}bg$NIrac&w&Mq?TAovjU>lQnJjA@V9Cj7SC`Hz^yP{U-IVv^e3x<=D6xYMKn5dP^@H)YtBwQFT|V=SP{XdBryGrSYMEAh<)Lo0z(*g-<{!)}UX%`nff*XvSPo~$Uz+SW3r=k@%N2CpM04_knat)<DzWw$U{wU6s%Z;e=I4|P5?{{zFznQbxyznCB?X?<4DYSQNa&asczya6_XBZ5hz^wpxboct(BMO*x>c;SHKr^gqMsnLTnVk(S6RjVK*t?ctv4&(*9MYB>t{`82-VY!t|WjpmOdtM9uXMkH=<G7JTwM$Sg<VyRVZ4C*_4u(RExu=Gl}Xj?FTf~m9~vzMSnW_>611&JnT0dn9t+tq122nw%}d8^hRkL9H4&CP0c~QG^dEr)dN>n0!fhclxx{SZ4F1ZHHE05HC4_YHmDabY>TP}Q#%;KfD)!F{a%SoZ*;V7<`}gfuUjqU5&L+)e{^^>@VukLkRJ{&HalXsVYMH#+pWcIr~%e;5tUr<j)=SuHPQE>u#kliZ}SdFc`@YT2ABtwJVr1w7*1ao@bC_ZDvMG9XTmA3gIHjy=~+}$LLA)Zu)H!KnKHYlDk7v0@Yi3!GO`CYxuir#ZH;%1>}yfK3%Gs5OWsVNA&hGgw+-0}j%~(DPB0G1C3Ye~LjLjc%FoJIuVhY?E2F@5w%ul-^cuB<Nx>TK)w@;eZc~ZB&3BcP&@D0qwxEuYOW;Tvc&T%rU+L864QMH#wi8roZMD120+nd7hSZ-t<v?Z`><`G}9Lr!z?RdX5na%tHqeQ^eqRgksDsWuE))%WpPPUfb@~xR?U&?X$US0_(rM-ORyvo$w>?)Ieui2tUYchkI9;M%JF9L}|#qHM~uAFg4&s;jB;{(brAJ`#<^`I#UmGX0W{fd{iM{93xrS`F1&W@s;^6;hGEfA^TZRlrsOu2jY_p*CAgO_DJB*SHC<{A4ozRfeYRoDNee}n0h6-4O0@*<zdUl-ttRDzie@Tt_tL|(!>+;(AZm6TNzGRmD<Woc51hLQF!%NHTQDSL=VFGiMIS0>w&UW3<-Dy{@Lt>3NP=~9@tQ<LK!+3}VVf2IQn#(Ad7{BE;S-62YSDo>`ab#+QR)up#91-k@Jmw?^gs?kj)t@})!09!fhBG;Y4-oyNuDe$s}#{(0TjiiCU96H8pV^8(M)Udx7wq+lQD<MiH%dn1&fKx#f(J=m$c?br&TFwC3YRbLq=8Wp;dgoZEY6BCGD-LEyNS^<|HEX<BVB}^<!<Y)7F}l>EZZ1buAZ~?s0b{a39+<N;6G587;zn~B7-3fQ$PS&91uSU6Y3!hxQBe?7qZvr=#!F9*p}t5T%JmieX>f&tt!J(tcq;4#s1f4!do$gKHJ(w}y}*^jD-FSd`)Q{(CKi><q#lsZDuN^4Zo><jiY2M_AmWyriv`LB(l@|o0ckH_2II7hI9Xm#n%Q^8JGyv*vnrEq%$9c+w4}6KfE^2wo_WmU7_WTYU<Oa8r&tWJ0ZwWEsghmdhoNjh@)tLi8$?p}1lQHq!XUO0MMOWcjK>MTqLP}YFI85jh5Bl0>QLjIybaoQ+yXTwY1{<uI&OfjA*=KDCq<Nvj^#V^&|F{knOaV!P^ad=!Xz;}t9zkMo(6244Y@&OB8fR+$34)g%R`z@|I6vC@7+DIS}CGH<t?Vntd3VH4)-&9fz!IvIGxJbRPG;9>ZtCcy-0WCNI9HS%K>#acb)XtuVQyqTBM6<GuY}?M%)L%p&!&8T(SR&ZieKpEkD(CRmbEpCwp1#r6*)>M^>xN#bP_ra211URW9afCB;r{JzOdwa<-~wapiuipMn>LsT?9)ArE1cMQm8ZYy)eU;&J0X2L7pBpLriz=LXgfZXDUZ(vxzNxh)IVOk&>l>_wm8?KR|zHG3lU14Tzl(i%3<M{K%E;8rtmrj>4peAV-8xVpbahAKG-w@<8m=%!o#m^j|D3si^r@S#$_0Q;^pD4PIic0wR7Hrph3u#Kdax4tn=YkBPkbDKHR^<58VqQVn1_3Rg!97x4~hll&U!T4T6xg8^wFFSH>sNx-yxm6x?s;nVODyi%o*2M!YCI*{;s5G-!mGloEvd_wQSy|!kp_A=?mprWt#W+GTdP8ijlvt|*SkLc~1pfYQr?b1ev*{@iRjX`2{!vZjxz9m34-e-P&zp<_J~;|EJNMB*+7EnmGkMZ8q?4WzRlZw$tFk<(PyxBtGY3aOG}Ek8)zFqpKMr5<zw$t}ERC}htZ&b;z=MKo8E#><y#PZ`(FQBK8|It(L$?*6sR-5yl>$%&>rP~e(h_Rzgg{2zPLL7h4LA#~Z1IAsP?V4=kBtY4F6DVbvM2;$<fGTAAA@~Bp|HKuztRW6dI<Zu0F2eD{4}^SXrmrU^6098^st1d!_KNQM*Yg8E=FetSSumA-@zFQri(p35oZ{;O3M!S;tV5#+`u09<ctd7cJ|ngGjzShHSBR8&JeH-_Sob!=_}zjxsR0&=ve8%sGdfl?MgXb70s?uhU*x6`K*$vCn<YfHY_Qv8rqz!g7me0Y_~}o>LKhJ60-Jl<!`Hzi@7>2?{<3|Z(hpOXx+ZbM!Gh@s6RS5^1S~1XmB*%a04^Iv~OWXL%Ly0=?X_;6Kgu_uWnI9z2iLhgSU!Ky2fR%`r><84ZHeOp7OdbxEs9TJhzq)9pgG)tI_KR`4%EBt%@~Ak5}UC&ZtvB&(CBL3A0RWB_)8g+%b%C84K)$+5@GKDCJ^5kpon;rLnb|uU{m=_V@G4zhBdVLS^@BxS4;3i?+$m<#N9F)}OYwdH(noX)RCLsc5?>s3}1=9>}ZH@0m|bHwz3dS$9morvgc!yr@cv4=ivaAvv-toQ$+~$9D3T0$yjZPsCd?GrPVW|8CwAkZlJ0G`!VZ_TiK9miGA+gRMWQU7HVbe;#aMKe_E=A>iYSjJ`2~RkL&Q<AO8+f>8RHFRI;<8w=YtOOqn!>?&D#vO*y3{`f;&a5Ca?WClpCK8-J<Sl9#AOAJA@is&O3MyiG*j^H|duEYw(YtsXD!?tn-rVcqPGXm(^ql_mYiIwNwk#(Vmhjb->G%AfBBN(a9hg8m;6?V`!BI_qj;&!Bch0^SlY9k0*e+LWG&`t+fjZ<LNq3h~k25V*5LJweFtW*IO<-6zFg6klQ_e5>Q6=r3*%9iQ_OJWUH_RatiyxNv57RfuTJgq;YLBSKY^n)w>gpMwZa5NMJx{nUZ#Fx$2p<d6{S^LygcC|4}zp+#0qyTs`z4^d8^wT^b6c3D`jeQ5o{ztVK1wyL$=d8w8yXFyQ)4%nJCF0DHxehtkp;<!-m>}n|`gn{3<tH1gl`OF7r5vMOoe%Wq2cfq-83mQqv)93Mi;ES+T{lao2$|9EHAplX1x*!&&3_`rV`|ijHv{r(yUTCROj~SV^KOmuwAA^?p{!8?68SqMac(R5#zPTD;R-X1TAnsI*2EF4q?nvMgRXB}D)8;<X>B;JBev?s<9K;nFUi5Ind#DrrE0CFzaow@T);k9Z@%^;5UyPp7f~LISYAx(m>a4aul%^+xR>t3hZ0A@6O$wHQICWEM0`M@cEh3W5td$|zKZ8(XeqfpLSg7ye+myBt<EB|vW@i^b#oUj)ngg{bQgUCBkOC-WqFZcr7;zOk=AW#jh07bX7S^)5*eD`WbOS;C6M%(u+U0dj&f#Z3$(>7uciBmftLWAvzB>G=kMj4Vs)ANA+KVlj>uTKwpp>lbF43($V=S^vYNyG(YSXsY^};kd!}c}JM!lj#xWZ(sH!Y%?3s8P@`YA?*hr{Gqa>&E<3Lgeh--G9ALBlak4(ny)|7@N&f6)EMpLv&(=?uz)DGwy-UChR>c`R6Zufg>->uNTRvEUeo2d(t5?gij7<wZwFBCQuRCd7HzAAH-^ELuWRhq>KrL}vtWVqG}^H$36t}7zjgxBWfo=XUSRKaJ*w`$l0M}_^zmwR4U;8_?84nE%f<i(rs%>(zZC*4%ZWc!3`C#bFQ;pXN3b=yvJ3Y&9^pQP}%mGo7YeutiP%1K+^xl>|lQX`YgjjVE!xvGmiJa9p1naD;&ap)5}$5obAle4fW)tKMffQX$Cy`3_u=Sy#(j-(HO84_u-$XEonhsuQCghaJ$X^R9_?v9(H#z!i4w_GAeHulEi?ckUOfp@X9Ydn`7WoLfF_<Xs_tqL)?^jDyLg`%5X@Hu{1L|T>=UZ`ztvMhX6-CCf_>+j;OL3IPM8jHdPPubeVdSa&Fx{cyB5=9yPw%E}4wQu;mYb@F>eI|9kU4E<f^!4$J*|*QXJ2ls*;j#ok*OK7JjHW`OFpts<@92aa^LfQB<MIUm!F{zz>(W`XYvhXiYqPENw*2HtWdYWyYlEJfY3h1Pw4;$Q;i@kNj*|Qvd}G0}Ccp113vIC~^7h94A>KE|5@?4qL9f-@yKM^<R}4uN6c%4M0y7c|dWr8M3iJDcWjgQKM5?B-wij@zWxUo&*=ud2;-zE&h;yfPUEJEo(I+>LVN0Q}{OPHZC)VRKNpQ<LIwc%i>#C7Jm$nnNyN;mU?TFdkl8{{&5uI97P06v*=~6OlC|N7Z<k~eq+Z0;!y4Y1WML?svueE(yX)x^nQ9d=yZ<FxKMj0`(LVtTkOxZ|f#2n1~!}-wj#)HHDu<vh{5o3U9&xkP^it>*b`lSEMjF@Vivy9m3k1*68sc+@hUZ^Y=@d4SgB;>0fUYwqtzMh?({KM(D$M&aKkDizxYi(~k=s($E^~PU4e|38D`ukV^G&}w7%@k9b(Q10|&GC!llh@BrW=6XQ6#!qIetrDIi`TOnpb`l3NuDV>W{;&`h;Y=`OtD&y{CpbKgM200+SP9y_U|V3>e*8}IouCtDHy|Hl=47i3p}X9_EI#9{`_Pe4gkKQe%+`@ag+u38)wpE56hbRmU;2%0K*?fk?Z>AtO{Mrb$9_guS^xM;m|@Leb=?NNH67^`!7;?%~UQDr7e#4#i#h1zEtEYO9Y~=X#wgB3uecW`beoPVb)`ZP2YHvAaW(jRF<p>%47|ca)p{<?AOhn3XD1jKyS0|Q?IS?(*^)Wy*0CNc<;M*<*BXvH&A1|j-qsMR2?68v5od4V{dHTk}B<VfvJly2#-APAn@lwf4teQRE5;OF;&~jYB2aAr~QF(myI%Z$|EK6EGyM0<Z;|r>bsmB{hO-i$&4bQ>f=8u3Zm0+cI}d#@^+Sr=kD6oNM&Za2sYcbMRfJh_qhUK9-KJ8zFj)-vJS$mxps&;7&*o8g?RU!zvL#iCULC7r=~h_j#0)UNo%@4!{jXEY8Fdv!lumT|Iff{$-B+bv2OAfgNZCMGokPMjwc85n|ibhmt517rO<Tk)7iL^J+98q92wqoGq<}2Wp~$crqy;fSDxXkG#Ylxo<6H2258h=pC_p00<<Opc-5=14Q-vYFq7A1nuDYJu1%F&=bbw-J^95%st?sYI6CN!_HlX5yL7_p->Gl@E~TNr2T{G`Ku6_d3G7&(6Oi19zRExN*%f_9p!$gb>)y5cNI=&CM;||K4~}heZ_oOui^9G9l#9Ck-SX)mH`Q#dSb}Q)D$i%NSzCx0muZ{t{cVJg)WS{2*p1h~cN;2omv?BR&`bSy9V+7LEEcmZ=4*C_rX9ll`R97JkYnp;y5o4ecJf%a!pqZW+1WK><y%kL-lQtb({<msMae;kv;kqZRLg0c{T&?plSkXL`}%FO{Vr)e|IXMkH~OHJ@vU@B7To?U(nW;s{fQuV^i7ryj2Yq6X*q8w#X&+PqYsrrA0mOA+H~%{cx_fj`|53GTa!Sxo7J+3w5T~h93EVZJTLUelOWje=931p#+X*kM1mV7AIs#nYK%wZ*UJyTv1BfinG5pTU{~2&@+J7HGOd};B4HqpdSSWOvg>fDO0;Iy7qfMmXC^}5^)!pjGowS_9DjQ{J9~Zn`qYm042Ms!D6BE)s2sFR3l4S7R~isz6s0VaQxfI1@LaXPkEeTPtf^(>(e?!`4S$7hK);EyNQ9N;$-{Uetkl}~9`$7uv&v<t{Fo`zE{$0oM3#m$qQ~_c3%Y7074yalct#zgXGRrCr$<tLU4^ol1prD=RQ*|{zsiKNyQ}jY0YDnN3Is9iV4sq7=~K<he6JD3Q9=e!<W%vPdM0*RK!(~i)ikln$57k0Y&B<XCA^Q|fV-xc5{;$4??WexE}uaaRjRaBIV+BY@hxj6(FzR33JfQv2i3X+Mu~Jn%*e!PB!f!-kX`CoA=Mr7r`pkTl}izb<8;z4LLo9x(2R4PcQ%x5bDs2pQc0;qp)$jjc4?^JW=+VDBYBs`YG8>`zxGKmLy$BCs^%#qXrco9wUV|q^&%zcdqa2{OVP5;O3^w6<NFmd-Jf*zQB!L)N{7)#F~6`tzDc7BKAZBowX26okUhrFv16iil*9TKDfgBk?66I*<SyRd=cA*^5O(o!ax}b{RCn>l&SLj&d@b&7!Onoi)#)eILzdefY-_9Z?pN4nVM|7KT_sVRv*`}oW9Rifj5f}5_Jm%a9lxo(7^xTTa~3>~Irr$32A`{1?CiI{{|i$e>&nC?A3iE#U22{EH7StavD@9>+2RphNWA;VS$g-efOUs0bKwJfs4PpKiR=xYQY*9=@DyDaZHbmk0?05wm)00o4IbWo3?pD;v@KW>vvY&B&pjZ8QdcM%rlvS&{$i0}Cd1;96xq9v%0hU8xT%={!t=Y2DW!d7zCO?wsHYJ;_2&_#r7)ataD^wdgwLvgww%F)(r6y>l%QVucmENWm&Jbj=l@|)0sm6tW_}DNUU>HrE##C-`ktSXb~$HX^6MK~l^3XkV4G0Z50R2%sYHPV=Bt)CA&nCx8v_dr5mMLV*$WQCUEKX!9tlGY+%0%*)X|jiVL}m)V+0lgKJHVd^BxlR?juTQ#3d{l(Lg#U*c<7BtDUj1VA6dlu!uh^(dbcMVg=X^fF9hCaKa?V-l<RDF9sqBEVwcX{W$m7aS;@hf{}wQ#_m4CZ^<(H8CPEsEq;V#3o(fBa{(BJKz$r{ndH1k*{dSU1+^bVRkmMZPF60!5ryym7czeevIx!hh=#yIvbZPAcCp}cPGn2p{U7)Ztk4wm0}zuZ)Qh7D9mhVb4)VXq`@8!!r@p9>?|uaV@e77I2JqutpV*%(2%t+ro}$ZG@<9?Hb0A}sW8fB>A;rxc9~>g&qMRY7R|ysbN<vf76IS*b7ncB^*J6D_UA!>m`zpduha!<AsaJ~vr?_0@DDN4J;RAIfrfG%^GcANWT?%7x;8GYG5W~-nuuFhL7cTvXxaqGzdcdk!qPV7#hSCdIB^itn61P1eK?OGkvz`!8>|aNhzA^ReCeE)wy>hJQwZPBKim8HgnMZpXC~Q`+AR!5=(2R#P^L0v^7^6W=kL3E*)bzf3(Ws^eeIR^&Q%e&+#0aSN5P_!({PvLE+nQU*EiT}b`O#?ZdBelQ$pv?9aV;>6ovx)7xkKosj>$;{PwR8`2GzlDfB&Dz$V=Q#7;aQwe6c@fqT?4|pbX>*=ng;z-VFOpd;$bl`in2*!!b(b+>cTYkGk||P;Dn3?wjoxp>T#}g`$+y63DWgfIR5mr-S}|anQfRKCs{Z^}oUYe-z9x{;Bghk1zK|qg~oCkN=9-(gkCxj9hVL3I&`K56^L`QrbV}IB<plPrv&H!2Hy?%JWq=ef&7^!<$9of!5QG|6`)aW6^RKI=vjg|HkY?9k0DFC1&Gc9W%Up57L+Qgo?t$M)qbPR92=Gtww4*+!QGsOCp6s*3p_Ma_tMGo`b(3Gy-A9u2C6+u2b?ScNMjhhRXPmidj)@e`BicYXlDgy*)thfB$QV-ho1IyjwMY06W~7<D|GI<!+fk8j<nFh>R<UbS*mle?x>>6ggCgOe`+)wQ9B$rov#dF$R-X42~2A2X#JHo)IzzUE3!E0WYnW9|Y0}9c+xyK~1zO2$iDc5c({_Ei`-tmT2nhtroQk+9V6yON7ay*J`}6%#TNwcnJq$K=~u`Cp<!{Qji1hLjqoY0mchRj51%4gRY|>Pf+JAF;+#|kE^n^+`~zKi^?ek0|htl59|$?g<$d^Jzz_q|FG3_#O|JKhw?}FVsiihE(B?1Ar1QRYo)=(O1w-XFfq`+6okNMV3kBmr4SAo?ZBCMbplk!z9bpL(c}KX;~{7vO}fzp>*&%?5dg6?j3;)8&jAn&`fEx{>jwc}<sy-r;qC)gP)#7gETYv1Qv26D5bS#Q@0cD8_QECp`EyZmr<CpOAxJW3fmbB?wOuaEII?jqbC%XGgcVX|ZID1?VblV^A7V7N0Le=~4f$h?SVS3D{unpC39#H5u02}Ad+f*eKYf}BV#J`FqlEm4=83SS{Vw3%Z@RG9KmPX5|DC<!Ihyxic4X2t0Et(>?uC4B;qOT+VHY|%>@k_hW7uE%iK^*&aWokn4Deup^Zm)tWp9;zO#7;QN{NV6Yov=mD5)LE8R>0Rh=m{x%&y6mE5BBO6wP=6Ml*eE9^8rWk@30tyN~45)2A%JR0~Tz3(p^6!E+Es*ut06chueinHuND?e}x^>FoV}XAf_PuaNlQ(4vEIUVg9doM527;zf`6ZI$4c;v&AF9pHkB5TN6-h%O_EE?9dzdx|E$B8aqf#7Rd)UQf!=8AC%A++>ZI5gI^`F^O_Ok^GdDZnFC+U=XEfQ=<2~z+^7vEuE9S7r~~4+qR6IX#Y3>e?0*f+u2(t7(=}Kc!`V;LmIBUNkPI(`l@(`o*XSOB@0*yT=kv3YahHz5vT!%R$s{j!Xzn47a%FAstH#HHbP#!2k^j~L`)LpoxS{Kg||sS4#*8(`56WnBPBZ`tb(Q_NS^R`vm}838xFXfc};;#&A<a*ks>emJws_Og99FW*~ks(*}?Ll(g3=FDbUOx6jf+q5Vkdc>~xzMgx!lj0Ko<ZVO#J=W`tE#-~&mTL{#t!_Ae@3O}2I!jiIVqMT&S<QOJt<9CpGOKMNy-(hM}MV@TG(kFdP=Wk^=%NBYjHlOaLhy=YoQ5uQ4KeHF#W`7+5?S3KpwQs>_+k_&(FeZYMTaIy2_Gzw1^XkXCllk_IJgcS{pXQyE5!RT+2Mc~5(m$a_u-z8_sA_~unbnXZITvRC=h>xre`V;ZV=1NS2E}vr5GovWpl}}q^5hIPGInp5I^CBzb6&_BI$^C$WeJBoK?O{%75`c^aJkQIm8tHv*e5Lo?V`sk2aXS=Tguw=Ezu(8|O7JQZIvFEu73LeT(MY%P&`waU8pXqKrww46FggMME->09Vl5y<xSAqRmQg?$j0PD}kr^J+@k8;6(8WQ8UIv>z3_|KZz@7l64Fc%it1>Hq5Rt@%ilvHRnS?BL(v#MZW+=P*m%JX{#BDRHL^Z+62kB-MgG3`k#5Y+ryh%q#aY(VeT7Xj^r^15L7zG<x#u}?J1bF-Z0R6cd+5"""


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
        prefix="galactic-mvp027-", dir=root.parent
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
                    "Le patch MVP-027 ne s'applique pas proprement dans le worktree."
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
    parent = root / ".mvp027-backup"
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
            "Prépare MVP-027 : colonie jouable, identité stable et "
            "initialisation pilotée par le ruleset."
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
            print("MVP-027 est déjà appliqué ; aucune modification nécessaire.")
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
            prefix="galactic-mvp027-verify-", dir=root.parent
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

        print("MVP-027 appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=23, SAVE_VERSION=24, "
            "RULESET_SCHEMA_VERSION=10"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
