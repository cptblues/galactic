#!/usr/bin/env python3
"""Apply the Galactic Linux shared-GPU memory hotfix safely.

The hotfix configures wgpu for bounded-memory allocations, stops identical
presentation values from being written every frame, and adds opt-in Linux RSS
diagnostics. A dry-run validates the exact baseline and patch without invoking
Cargo or modifying the repository.
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


MIGRATION = "HOTFIX-MEMORY-001"
BASELINE_SHA = "702b5794b27027b3eb2d87e1da8253d2bd187850"
PATCH_SHA256 = "8377c9bf0cc7e35425722e8f2e223aa684df02942448bd8ce1d014b475b32a2f"

MODIFIED_BLOBS = {
    "crates/galactic_client/src/craft_ui.rs": "dcccd903d9c24d33c61960907d7bd5fe362b3c3d",
    "crates/galactic_client/src/lib.rs": "16de37d66a6ba1ba42b6a7872405e0519d36abc1",
    "crates/galactic_client/src/research_ui.rs": "b177be3ccbc3c5e5d9e8de6503a90b2edddedec2",
}
DEPENDENCY_BLOBS = {
    "tools/apply_mvp_016_b.py": "1557ff3f419abbf6a1b58b897100aa72da80bd38",
}
EXPECTED_PATHS = frozenset(MODIFIED_BLOBS)

TARGETED_CHECK_COMMANDS = (
    ("cargo", "fmt", "--all", "--", "--check"),
    (
        "cargo",
        "check",
        "-p",
        "galactic_client",
        "--all-targets",
        "--all-features",
    ),
    (
        "cargo",
        "clippy",
        "-p",
        "galactic_client",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ),
    ("cargo", "test", "--workspace"),
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
PATCH_B85 = """c-rkf+j84Dmhb)wL{sIaBAU_7mMnL&<2dO|s^TPLCz+b<?qbm-B(p=2YF_NPV^_`VzV8?8)AV=bPxedp99&3%H_1-=w$*;HDDVI{I5!*s2S=VipOc-Pg`X1Vfg3w1O&%<qz;RRGwcWs{VfrA6U3@i9?aa60gq&>%xaEf){Xjg|b-n$*Wx0EM`%~wP4Ep`O@px-zXJ_L+w|c$a=KcQluViO;ylW2jNe@4Vy9A2nA<0%AG+=Ys3qSGC{J>AIj4gtH%Pb`+{g5Uz^7o9!S6>_3M+o@_%1)wynq(V6@N4&xxkb9<Bd<7*Vph3mk@z78j-x0|0B(nfzxxW%L*mZ~pd77e2rxYf6VQ}|&`TMBsDJPBc4jm79MA&bhR`6PZB_o}dmasy8V5!H(5Z^%y@wnd1;gq{5JG9<W=s7*0vRzQp(JuQClMpLiYj)Gz~tdWqA_4LdlM}d0afXVf7pW6ZzSi&B*nkZH#A<-n7HRonEEvCSZQP@Y3zp!qpO406PYz~Bx;i=KJ!y2AmrPBe&5#AfsQWVC<fI)d~Mhz+AD<DS#h)FZK&>nf3{ZBIgC*9!JbD)6VI|d_iX5nM;j}-Xl1Q_3piBmskujbQ*+Qqt<Hc0N$SmJAKfSjC{&BW1jO4(&X1jyNm73aa`PmEmf;Q1nbkO>7oY>w1<2$oN$GMnJ7u4*o!D7wKz_K&(%EboK@s$F=(^c53jj~=7>kZ0Y}X{^@>3T?I53Iw<!$JvIs&Vh2AK!V-Us#-#W4k%#?Tq(=?VM#I>;7&X!4?jrYVRKp#SlhE~EJBZ+@62rT{&Psl&1Q$6}S83fLQ{t9OqAG@E@Bh3RRO#V*xeUIJsIOX%QrOcUV3apZyezK;E+6JPy<y_Bux{PLou<A;biGr_<k;MCsG95Pn<%T+-5FqlO+V%!x4iWV_lbuRn`8w7c%tIBDLLbCAPmpRC62F=-7=wHw{p+-W3xs|w1K<yRco|+`_8E*jA%E7u_K))~~&T3^jo@cKZK8f*!&Ydg}=p`03WAIVu2x~TI#R+2zvXRqhNkxS+DAQd(1$#z1&jjhn@w?ZWeK4zaU@TFPu>lifFr4;HG8y)}7O+05t+3*pvzNfS8BzE+n;m9p^e(vy-THp~pPVzqFO4%=0|9EzsRMRQdwEkvqZaa}+byhBneMtB)45Q!fpvVT0IlawZIuoA)L3^NzZxrU#<#Y9b^bL%s@$msx@khQwdPH?fl)zpX)l?*vbl{DTnAlL`cdXapjATS)J9UGg))u;>cmG;5XA=j5B57=oEd!!rW=#s$CP~x#@%jPJEyrpI1xqVdAu+gCbMRfeRD8?d1qn{N4K$N(6wK5Jez&}&Zn0r^epoO?}YxC(IkCjbX(wGM<6gXKBZ!IJ59m<wO2vXgjdH-=q%_Gu*$3S8msVW<caSr!YBbjPxKyC4J~N*IQG2-<zsGw7UIj`+aZAyz+CtIt=|sEe8K`~M+4&1IUt!jahibx5JU@5R`#W<I7#b)8(`nEH3SD0vg{R9lU>}vT)WlV>cOI;S?I~mBW-dgH50ySoJ4h3Oz*SObkD8B5}sY-ar9v}3+bhydH`iZC$Kt4-(f}+M`AYPU*0AlKqU)MtNAyY0;?c}?@5>%4F?l+--e@!Io?A(0z;a)DN)7bMYNC_6gWw0(+hYrBd=EY^6O{Ll0L-}T!Ue=y}x`5RDfCCkh<Lceyb<it;n?`0u9kbdNXnsMS*my98hX%qt6)6$Q%4H1tXcdi`j_jdt<}E@_hKnVNl$8d37f`2j89ft~9)u%`TkS=sY`oad`CR`H_A6^!U}uKkX;a51+k!bqXb?9hf;1J9=+Bp=VrQI6+3QD;#xF_nap8jH&@<6)c#wmPQvRFeh<+-v;5NvFo5aWH|}WYPB;{XV4+nq=VB5d$^(r`;5Yl4y?(9qHbj(C>$ttSHlDF@8W1_C)ABXFEIwY7EGn=4WATG=n6F;T&Squ=!%%j$P+NV3lq$B6oM8XMXM`Q@)g`$P<`JCp8jZ(r#~uE^g51Qnk1a(r=hb-&ZB%y7AJ}Q-ang>Y;P<Oa6+)ZxMV25nENocQdx5D#MG;lUOFqaOdo}^T4@n>?$#$?e|2e+uaZX+;bLk-6T)9tC&WjlR1|Q6e46iy@Wy~ZSbim;66u!P?eMTB@ze9Kb1xoeL(!6KYDtrGtS(VIOu%<YV1`Q03mB*!1E;EBV9P_#iM?Y1kyio#Z&B0ib1Y^+NMUj}IRPSNOY$ssR_9;|m@P}C3BwjywePtfF&v`-oJ_~&WKZl%DbtRT?<)D>Ji2FSo+sf8V!~iS-U%#^9>Z!4Jf6XR2iG&q9*P#Lvy(6jY)#DQfJ&5;G)svB%K(nEw8k@#Ea%cSYZ5yE>q?RH=j5K!xFrqX^|fBEYQF^~?+tu}`6Ngk@Di;bGMEqM;Jko{ct%&B9n&lhi}{y7ptViV{XtvH@v@ZFVDShBkii+!Y+=n6*L$cdo+MEdVFPk#&aDC);Lw7?q}9n6$Gm1oF;N|`91lO<kYoQbvl#Ljjf8wo&Y@oZ1#E(RzRndBOLFzYFI<7xlT;J@!W96YR^T^)`0$C91NupVJ16)Y(^V9+Vij31MK(+uOaIIew6KKX>$XE_wr~xsG>>b*3X8c8P#94aOiF<ypXCFMfctKgg{e^oS+#BJ>u?@e0S)V4OU5)paBeleR%BS`X0--NwIvzc{@ND2*Pu|ur8BG0m^l<|c4BT0d`fL(Z^#2O=nu#F#O2Nx23HC@LWSxCewcme;OqtMHmVmk-8%a+2Wp<+u9@T6#mc<%0NZ?kn?Vn7j9JoAUHa&~wj5eIE907jU6Yj)C)8&6*%C4S&e-Ok>IR0Dg_mIX?I^Yt&sZ6C2;-<5`DYKk_T_9=#sd3j9ufbT5g1v#zDqQUI8jI}VL0os5C#%`W<Y@AzCZ8N!mo*L#Ib$0HDd(M84XM>OWiK?gV0v83SiwDg|LMGYW7uE?!apvBzTE1Ic8KnjyTY}V6CEIfOgq)X^q3XxjY9oPgV|eR@%|9aLc_?2#$xHo`dE{@0|PGjQFs`1`UBlHIuw@&0K_xzUnnIcb+gxln2>)w>&vL?U09$a$ZP#EXRb;CFi<xk|c*<kM(+*g<ezp8?exNZE3UDR6NCrOs`p(@wU3}mZxCFOEraEZ>eWvg^xFN3j$^~L{q};104ZIUxs$#(vTV`;%>BD0!KkLj>J>$n#ordt790&M#T{<PGe`}De}*^kB0p*-#~%|Fg%R)6Ui0d<FU`3AWDpU(X5=F;TyAb%2=GCv3jk?j7+Hz%;Kh~i>GU{TRsPzM;8?M(R>nlt2X|Shn4v*MSpK%4u`<4VZU#V2ld=aqbNxImBN1MgGwxOBEEWW@SSJ0Wc&%|@Nf};ZIR<|;9ivyt;4T^w-cka`I`;K#>66*hz5c9e=x+vjVTR*`-zW!SL0ai65t<n>>S_xkhvuEHoh<#R#K)|{=i(u8VMthf=f9^JMzv6X0z8Hj50Z`52Qy1Q>zbNbpWeCV~V>xLTju`-%HQYB_hK~A7Pfx2ZZQ$wLa2l)z-yTTNmflUz{_yiwp+!eKb>67&T?-h&F~1>F%`a)rJD_MiriGaRBV?fi~6U*l-RM6@LL%NR$hXORU)!1#49%a)1WJ8Cy5N`!o2fA&&j!)Q0CW+FZ=!YaQu!jTLgDwz#QKiRnq1hn(e^zsLa8UO6ER3^8@>kIf0rT;n}+U!S-t6*jbddA+L5%U|L4aZKlb*x5{+P7EgI6p1$(n1j)#)MKFp(r44nCR^iJKLE{)&Uazwb1g}=xi4fSehslr*b%0^^4<3~H*$u$70o=^>OQoY@1?zlif@FU!9z@jqiLUe)w`prSM9Q6P<xr5gH~hyG$;GcF?>Z2YhwATfPY9gxiAl$!~lzXoEKE3HGgGn7e!hejymUo7;|p!@0ml!O^hAdzdbiuNYCaWhdcU0n*|+~f^~{R^l}v93$`rBMP^NSj}nns<@rusM1>4IT+oC$jcm9+)q@5rHE$}`R%`Q_-WVa@9(}*A#e-5v&j$3sAenWP;Owha42}A|TT-7*Q0+Q+D-!o=$=kaFX?v}aG_8bWgQ|8aXjmW1!?rNF8eXIL16A?(=;`U3=dWIp*DnrVe)_LBhtE%*lK=hJe-qr?7fNe|&XStlZoLdC+nY!W<NBJ^HL4B0dwUr=5{Be<LiE{)mYA{$lL|~fn@b<Jnj*89dhpO%7<CrMXz$o}ZLbjh-GlZ<RslFGsAYx0<;HeA9N>%z#&!x1lUp0x#-OqS!Om;w{L{Zv+Ud4nnak?O8}a}IlYoH!58)EZhHOeD6Xz1laUZ}+iI2_f*0+6g3yD6=+$^NE7U2%U{0*hvnd81+C(eq^nu{v1a*0BA2wCd1_a%mM2XR-&!UVnrX=snEYws}zz5zB*XzKWZ4u4%+<Lzn@J07?AI#uf7|DMr|mUn+Q0Dp-m{q<)CFu&xBp^&)}+&j|S-`*~H;l=CH3{nc+uZ`75B^?d>Y&|j>P5Kr0)?wj*_F0yuQ7Ctfncu^UP<vpZ#=*E7gv?sRQ5Jgqdz7WPq9{Bg(O5Yg+qnQ<;fcNQ^q||kNc3jOezuj!NwQr8a;qkQwaJX2(jfbOi`H^cC`QF-s?mj6ve?M6m6M)E9JMh)5{uQVv?kuJZ?EF#*aen%#ZeVUZOn4fr*u&gxDts<&<B-M5)E@P=r2GFKwPJ<#9Ve!Lc7ci6}EK)f3!OsGsW54ADM%pt~kYczRQklY9=&!&UxirVz;^2<<|#ncyrKPZ{?#)TVIpCtEua(R>76cZVcI8>@19Jfq}Sq#_ECIMrD!B_onL61T+}38^KG&1(KCn2e-%?y1B>tX^N&_gIqO;eB-`3%J&4$x9NY~U~jhCz5+Vza*)OD21z??Vf21NJ%io+dCUrDvtwAA2eez~@i8aO3DF&3?ZC6m<DG;gk4Ctk2QxrrKW_s;W~*C%N2&g09bG&>NuAK8HB~m1yLseh3F$q=q1B5Kjv2R^s5_d?{`2X{tJ`pHP4iX8ZB=Ogo?e|rPVCv7`07_6K+J>fjcv|i`POWbW3h!X+8vL~aUa<=n3}tTTk)$FP4IPL23$7S5_p?)>!5n&5zPj(nG&X<nWBjri<DF5rzl$1h*+6KRR|`|GJdz_J`po-?Sj~=ucMBwrsHAX>emztnwH*_MVpU4VR!e$*zbaQrH*KkZROUy<~Z*(;=I#_vw}(xp`-CuAoL>gL>RTi%xFWqTN0)!Eo$hmkBkpQ;@z#@aBb^9cMS4^8spH~K&gJ^er)aaV0%5$)JEF}g>B`hUy-+F1iWVnT)ob#^Z7nlk}$!QGgRU(LtbJujIFiG=2q9-YyxOe-E~^qU}86f<Ze<$wabiBD43s_<Rvw%(kQNjuf2P%-CM<ocYbey2k{$rm+uqY*||8ugUv?YqSlS&x0!D5#EN>qh6`JuxQPR+BDS&ns&uij^IApg_O7dj>@6JE4F~z_x~)w<r|!$vQcy~QSu;q*#%{p(U&h_stnpvR*(|7*yPDHoc3L)?Oa|s~dh4Y@%Rp89tPuBS1OF$CRJ)Hfu#**~9z1J)-9eQMxAjL$tpA;Oqb02F>WdEQebH&Pc)cUK4#kou#=2%beF?v=xwTp=cUPCLTz#EG_gi{Ms{WXp*sCw&9VwP{Ytwn-_vINCJKcNxe8X-!om9i4<LC!&B6-s++RG?kV<Wm!<QLea=Otse!z3kdP7Yt5KF6~``}FD2tCvqsXJkGa!T|c#-mVh=B+lM+JEh>W^gk%jJW{0zScFYu9YcxS$-8(c>o?zOBgLIIiZN)#?Km`Oc$wls&Ddm!WYY+6XV<$xd9(EwhQLk6;Fzr6ZYenFj@8jxVC{SP38xll*Me-QhA-veMeR5+{uZ%B?2A~N+K3e^Vi#ag4tRLcXfhlyq1+$#MZO_rhUH3i;U}3B;PH!c0K*GK=!sN@;*{k_yck<bc~yiNvv#}MiLHK5u}EPfG+Xp%`Chk@8%y?Xv8O=h)Tv1p!>C3*yI4tyxJrSNCBK`^%;_F9=H2>bI>5-c+99?j&Z?)$W_h%71!Tc&FdAS?>aPOK_Ep@HP#6Mgox}2Rmfh)rwYw|iZM4tjZOp>j+T~5lio{;3!C+5l%KF5$W#5{PO)V}=nJf!xu$)P1plv=e$~SsO0Y1hv2RO+A?$=V~J_({yK@m)#l<Vh2Wwhi}#J`o#Q_EveVB61$Cc9H}A1S^!Y@#^3e^pX&aXU@Z@8;_{UtHf=_rC{quaB_kiDechAYsnpz~X6netGjy)%J~%`OQp;pV(4wpehrJg29UUIH*+1!!KDdwvgz_9t(*b?N5glPpT}^<v(rb^F;*ugZ6``kt&A?l9q_r<UGm(uy^Nrdac}`w8Q_lL9r@wYvW@T;5CM3pNCtG`~7|;wiSH0G>Ub$ut`QC%flkOiz5C@mX@z(b5U|q369#{u537$_PSre;{`Nv@Y$wr=T;V}P5HSh+|3wObbo9OrS9KZc-I)KTj1N<6xZHeZPbSL>E=%r1r?nR_R#wp_eWK4U%jE5^SW$7i<{?~o%~X>zA)Z@dilTn)LiUK>`(d_elZ?Q_smj|Y~~x)iq)HwGQPF4XwF|KQRQm3pKJ2zXE68y`nxy8_;}NWq6JGb1ZI|eoc+fQ6nw;gm@0=qU0}|Fgz_&gzqNDoU5E>WPyb%h1in3B?@p0TS)8EMhFMX55@o3Z_T<ygxRChtbMoLc3O)Qh44vRA!6z0A?c*bde;{jlm_3Q(fW5O+GPOHXN}5)SSx%$70i@7`JpBsi)cy!FD~*T!{c@m;#;GEwoYIp_fWlK#h!>65s~T@sRd*%yhS?sfe(R~Y@x;@`u~=QP0Wt3Hv0bt8a5}2^a+q>x^NW0FoA^c+)<eNVtXOqhyfd1w=j@ci@EN?t6B6dZ$}CEc0lW{R%Yb?dDnc7&g#gGNbQ|urRa^bi7hq)=;YR4;&YT#K?3mYAdM-v>yswJDCIqhv!t*d$F}LtYa88KC-agBqINl!)&5@LIDP=T{!H;D3i`nC>NX`s>;ej55dy4OLaU<YK0wt3o7duO>ovkqc-u(l4n)8=$5$5|m)gfmVOpJ5qB8n6H4C6_0{m--6I6(l52tHR<61_C1uax<%S>Bh%{FzoMA!hAm9U!>x-ZMJsIhE<tB+lT(CMg@HBYP&YtxmU;K1Rt{oM0~5dq(b#OKdGQp{CiGP_x@DCn+lvl_%<v34yrbU|D2h8#s$`wMm}FhuJ`wXRG8@a`d$1|0!i#`EIMnGRw@qO9|N>3}LlG-ap>ziNrCics%VZ#h4L>6%!4b<$K}fV_D2ox$r_3=`^0Qay8Fbw}#d>{jr?VQVy<kfYO@0Nd3!`Ho_QfT#{8mN=Y1H)D3_|Qpd)&)EtlFCYU_qBeXJ}nvD@ls{9aDeu$7CK39UY%ZyYBOWJj@!rY;@<GOSOMWCBvK1~emh+if8roR{vi{!nV{6>flD}F)XY%rOe(UE11=X-<s+}-#W1WK)~{Uw1CJj+x+8u1?sfzR@fh2-Fy{9cGmVf^P$hWvIv8B*rKulcJXF@+fc`lfgL*^m=iose(e{2qQU<o^IR1$y2"""


def configure_shared_guards() -> None:
    base.BASELINE_SHA = BASELINE_SHA
    base.PATCH_SHA256 = PATCH_SHA256
    base.BASELINE_BLOBS = {**MODIFIED_BLOBS, **DEPENDENCY_BLOBS}
    base.CREATED_PATHS = ()
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
        prefix="galactic-memory-hotfix-", dir=root.parent
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
                    "Le hotfix mémoire ne s'applique pas proprement dans le worktree."
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
    parent = root / "backups" / ".memory-hotfix-001-backup"
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
            "Applique le hotfix Linux/Iris Xe : allocations wgpu bornées, "
            "invalidations de rendu ciblées et diagnostic RSS optionnel."
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
            print("HOTFIX-MEMORY-001 est déjà appliqué ; aucune modification nécessaire.")
            return 0

        if run_checks:
            base.ensure_command("cargo")

        base.verify_baseline(root, force=args.force)
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

        backup = apply_to_main(root, candidate, force=args.force)
        print("HOTFIX-MEMORY-001 appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Diagnostic : GALACTIC_MEMORY_DIAGNOSTICS=1 "
            "cargo run --release"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
