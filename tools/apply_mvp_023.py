#!/usr/bin/env python3
"""Apply Galactic MVP-023 safely from the exact pushed baseline.

This migration formalizes the one-ring discovery frontier opened by probe
missions, persists its metrics, and exposes them to the player. Dry-runs are
deliberately cheap: Cargo checks only run during a real application or when
explicitly requested.
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


MIGRATION = "MVP-023"
BASELINE_SHA = "747f5ac7b52d5706bb749c5cabc62b744815f425"
PATCH_SHA256 = "fb5489cbdb4e8536d230f51d35fce4d26b0cc19276320d67c02a4e8d0c8bcae3"

MODIFIED_BLOBS = {
    "README.md": "8abfe9bf67e8e6eced6633ade80a4e2553653f16",
    "crates/galactic_client/src/lib.rs": "120cd4aa1f7b9bf5aa02613eec24f757db8efd7d",
    "crates/galactic_persistence/src/lib.rs": "5016b8b8f4413e7bcc50e8fd294f9a2819cbe140",
    "crates/galactic_sim/src/knowledge.rs": "be6a6f3dd6a335d88b4d1ff823235fb46d1dfd98",
    "crates/galactic_sim/src/mission.rs": "7165b984f2adc494230b5179623127eb10ca6e67",
    "crates/galactic_sim/src/state.rs": "cf008d4d28f1902fb0833dadcd060f7ab31cee2e",
    "docs/mvp_architecture.md": "6386bd25b814a4fd2cd67eff75399c48ac4e7620",
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
PATCH_B85 = """c-qxG*>c-Pw(tCkwo~PhAqa_DC<z|B;@Fx@Dt6*)Ij&nZH3c<+Ce>psG#XIMj8!!sF&}UrqxlW}$^DXhPA}L2LfL9AA0#ZNPj{a_dmpsVS1U4`t$9l5-kbAhFJ7J7p-+~@^@vA4`$$gda>dS;wtaMVaNsY`$b5f)adb4A&1U76qv>>7Y5nDwWOjILoscQ~Jtc7ak`X@+DUX;%0!joAvw)^NjtEQf!G<M*i<CwlBN4!|4TBLxDcMi~cjPN}y|n?0AlDfaBY@AY@ij{mMiS;Fj4=e<WL%KobC%MSge>JOA%aGNcyT0Hat*is=a=VE%G2#7;v-?4vMd>4A0Zb4N957K%K}CMz?dfV?<^G-fu>2E@jyRc1#z0fe;EBHiPs4ef{`rZA2JrgxJE=|i9*1=_ksw(liuB~@x8Fg4=}}x7$!(wW*(0LCyp;oGM+L*GvZy*NRAz*nUcGI!z4#D7G1~NyIY1sfjKfFwj#azIRt9m-KNX~{6yT{25<$lh~N$em)zZ6-~Eh(ro`h*xVZZtB4Cm<N%%Ejvq#hWBX%k%nw?mqseriy!H`BihXvBaVjn47B8jjgARSDV1F9%uFmE7;;OmHHAMFttkr%O-A*f;$FV2#bIporhJ+T*FZ(N#qR~!eJB@Ec^5*ztt-u8g%Ozf>`0KIb0^#Ts)_C(_C1$=44Y?r%Xv@FaI_C5cI+V+B;QOZuMvalPAl9AnTC_6`o2S?T!nacn3eH5S-ND6Yi8_*>S#`4-BU(=AC<K=|Ro|20+;nAA>V?@kvVingp?6)u>$9bi={S6awaLxR2f)^Q5dP1H&B_U0{D^fhP;W7)-GIIPCSm34OyiVdJ8!J$WW)&xZ>A`q3BemZ{U<%+S3J9dsrz{D1gzV%L4ET>5<TPMIOJ3^7l9S+m6VT}HXXGr&fw#fkt#}B$_ZxH-#h_n+4-G%chnk-|J7?H?y>p_dVW%`%v(%Q%G=_DVb-*RCv@~G8iz8#9Xqk_OjM$IC*7X@mz!3NxvzkZuP;)?>BWk}YWSOaMEXA;=XD8MI820q+)H<9a!-laBgchkNvkV0_nE*RHfJGE^&1_hg6cm6MKa{NVLqxAB2Nn<5!^z~aYIp-;E$8&vUlHNo)C~wkby;0CdKYv_oWZ?t(<|lzk=#`hOLaP)z*utX1k&<=Fp(z}(aPn-aiH<IO3qwxl{IsmKU1j{zws6H{>18N04C3IF3=9JHTT|vSx_xNjnv!hkkl&5@>?mX<vE(4Hf=8Vo#qrYhR!JzoZ+;~dv!qaSp}|bMEFx8;gAc-xu*Z4w*K+A0lwsMCLHo@6#oePxn?h6;R45bp=F?>p_e6zRG0O3a;ROc>VoKy!w$%HL3GH$F3d_02LNh8bjVQ)<V_pFVo%Q6cWLVUA||Y0MOmB{w?>Smz?5Y8prJRV6c1>pC@~EBsL;JdgButhDJu!0l?43>kZj!zjokNaD_m*V+-ok-QE`9zAbchFLTU$6)(#;!U(jY5)5HfQ@l}?lASTf!1J3``w`}`7_SsX%@!5(3mycm6z21lRaT(~Qeix)2Q2Qar`}4)}6nygI<NbsAYQNtP=|I))jC25_f}dMQWQu>j{Bktg+as^uy`JqK95}{tCy3#i)l{-Y-W!S28bpAr=fm|TPErBBXO8NyKvlEW<%Y)B#;$xWyck^8Eiw8X5Rwf1@P>xd>-szn;%NJgZ;|teCF||i35V-kgTrOOSc(_l{|EpUc`d~bAE0{85)Tvy%q6McpF?brXsc-6z<&rhr0ZIMU!<}3-pUo81(5(pubVxOqZMBl_xNtjU=XlEZl)h9TRT6%z)Jpw+sb^Uc<af&<srM+Fi>wqGpL(DR3lhwAsPKl&Kiv73J7}T$~Xn4a05nnG}~lL6_SvPXYbD4cjs>|zWM$;hh&EbAPDml5V{>8r-%oH(Pt1Ou<;AF%wT%YVF7?EUehGyG&uh-0XaoP82fmDiC!}_ejtc4ggl)?^f*=j+FXj(#jcX{aSLRbO!)f>;yNEZXEe7k#)Pg?^gwcscpitFSa7wJUq5?w?q0lo_BNlhB@Om7NzamBS2Z@4F`cRtee-?dBRA>0*H;u4xoTs9SPgqx>g3{zZ!S`bO!by|R}padT5`Bq^QE4f>(P|L%aT(6tp-?(p};+KFv`?OUNOrgGEfAL^TlJy1IH&akAVLU&N}@{qpeGMg(u`9zfnf6${3CVA>=rBG7T0_>IiU@06~Gx77&E8@x+#|ITL@~|C`PITH83`YaZ=xs2l_r+t14Fl|NK%Z)D#^oe4Y`CtJW4F}}W9#u)AycVn!2pFjtXt4?kU+N!B<@1zFP(`5~kM``RrT%I&=tbnz~pFGgy+y}CJYI6*`26>;s_73`sy#KCNofF6*XtW)FGRQx5`M1^S<gB2VKR1nTfbuss=qBCB@b~0N!;SqRLR=m6x|!7dy4s?!j~nn?#}7d>6CQC?%*uNUNW5VY7+!R6b0hn3Qe!XG9z0D0@q2KZ;^F&nT7ZxH*J$|Ke338CVEQ-c3>pAaXy7`&W8R<Kr%%W2x(CSjsH)4<=v;VMRV1b6s%bS&w~e}0F5Keqz?y?OT^t`-2Zx=iR7ZQTGR)sa7QHI9{6JTfRU{Bfde1od(k*AAo+)aK#6+^%g+z6l)QzGvjB&`ut<h?0d^+ojO%ok;w<K(KhLw#Lwq1R8Gno*+%KJdxQvDWSle)NdPfVxXfJN^?HUg0ks|yDNOWb=fe&~VY1AQ{T?vuY~Y$FH<&dG&dp{r<zn?^9_vJVf&m4uk3XFTbJ)ugsMNZrx6I;l+XpP1vofWvWb2CW_mHx!&5uxMNfkK6li>9H32PpnStZIN&jRw+N52Eq7K?p8FE-B5Y{B1h4x=S-TVw8E7nT_ygW>G)geE-<;S0Po&K?GKD-;&4pkLmXLT<_5MnY)0$9-uJ`q(0E03Bd+2UsT{fm+VP{}8k_b<`v5D0=$yvwv3?tvj`WvoK^Ln-+nyh;PLAh0b{tv}wa59bpeo8=V3fbWC|~*FsSo@crZOmO53P$B*+jJ{_Pb!;Ap2LE0Q>Ci?P0W&NBBHa+XuL$d0v>LF-fl&=76>!0J4=F{OKW%9{$c{oKOlYb3T54MWZz{j3Xo1b{&p^FlL=`M9B5HU%&;-#UfE0%|IJwiBS0tsYK%y#?(vPeL*)$iIGfi>?tId?W*RIn6wtO3$}7%YP*aWk3+yinvx$`!W61Zc^kkUKpm!Nfm+Zemo@ED$F4D}mkJrOIJs2GiA&Yfl6e?{>kJw3Gr6gR-3WXRK4GG~lF@WmcyAFI;!jI!@xo+n{%j0~7JLaK$5AANt>Tp@mrghYou}G@J}tp@<zlg!L0g7z<YEy}awkH`tYXJ3pU0c61>7M++vcR(88FVG!rej$Gg3#{Cu^8e(dN8gr4&F~p`6SY$IG)*+onecM@NSz!xTyhO}{!R0d}Vh=Z>6mE>O6`4gx1rx)CxOruk&%3*MO>d1(<yNWCi_v0B!!icPl=<VT)fp=}Mg2b9cng|nK!Vp4`u(P<AI+k>+D>TSZ<E4s0Wyt+VN1(@Rn`;alEuR3y#a`N+q)TQw6w5E+({m+~qx;8niAv&wU)H(ZSFp%9sQiW=WhJ4-ZB;TA+?@@6g3BY7o2M}+Z93NVT3r&QvWS)sk%=_1J+s-W6c+?s5+Cl6Y%Puo=FZI3Sd=n|D9_MakD+chg--8gKS5r!JY5%bv{*-@#tng54g1!NA;*N0WantcPzoLcKXE{nkjXLUaH!|te=BeRcFzsvi0}T#i+Tyr2y4BebCfguDcd`R9lv4_lOw!V6eB*rbEiuHv@t6+!b1t?SEa`>PnTUQMlbKK`Kao7}RS)v8C|`}YDiU=xpSzREt}Z;ZGCKzXI<pd?fb=@ft7rec3q2Jm=tGxYD~J0peG7*FM~kIA-T$1$>Lqo%IT%jx_Vzm($tZUROwX*Rzk1d+^G2TZ<IF?uT01q7o}um@O`BE7yVH*z_nRmkhdd#}Q(k0<!RQ4tc5w$-?yn3no;z?2sE*oQnX#kM?mDTy8jT+Cn@P_q7NvHWzNjYoiprir(Sf3KZ){*dbY$SI_V-WyBimk_EEXpxXTu_c97%s%kb`wb1@kk9tkgFA3_@GxD^{^;&V9mE@<r7G&@8=*6aJG4M$17#3o;cER1yB3B?Ng@m8vEfP*kAn2dbG7wdcRE<`7NDzx6%;(g4-a<uc?0HhbIBrxO^pMk)@0vHe%ZGNw1YzlpHmEKauiUcY<6Hvx3uZB>awaepBfNI3!eA&lmYULj2f1ys+!SoL_=YD6-#ia+{RTiIM7Fo&Jr>P{3{^ET#zp)sgHFy>~&uk!+kHw?+=>zWRRCB+)y1=GQx=^ma#h###lJQmS}$IU0Y4n%g5S6Mv|N;My)@tKt5n|vos>!mhwHqs{dq_DzV-|_!-6jnIf$^08pSas1(=3ho(QluK+sl+lS?=ZU+X0qyo@MspY5P-Gbz#LivvZhmwvvO0AN9y~+lrA+lN=)%m8W*XFN(EG@)TUWkl2_B9_fYBgq~32!h9hKY!adn{`>5=46;Ds^Cw;X%Dh<|}{bErT>Q#A%*+d@jbZcsRj*c-wg8z<Xy)X|qfh?qv%u)AG%<idil=GTkoikoCQOdkZ606N?`N2W3m3vh@44KandsFe-Il2Px+bjI017;m;F*8-vvSk}A9{@6jGPxSYQJg~WHu(<yjXbAUU@DTiG+_jwF*OB)R1BkX$pMg2#Bvrg%GDFV`*lJ$R|PhikWJW{CVs$#&<xBIf@xu&trN!VB8{_(NF{16oFBE#%z{<PAu=(wLmqW1D7klTc8VI=TKOqLXPVbItfl}{XR)8ianyFq4UF=`@^F866@V9SIP=GYcH%3eD`jV3Sl!8)A_I}rW5^z*DOc(Yyjmk<yQk8@4e6#9rfMW@g%8aQk>2|3u2h5>`27pEb;ZPe>$Q98yl?Pp6X`a8(Kjp?!M;Pq^-HQXru}h+EkLJ_tEJLrsT<MeAIb10jUn-VQX4>a3_2qj7dyT-7p_18PROH2<Uv(Hi}GvPcyd2-)k9{ale^qvL3TbUh1ka6+0r9S<67N$uU3&-NpBz((pY`3<*N2{P?Xq+3X4tfS_L|)=r>?%8J12dWS@(Y_0WaP8@k4LN%|gzWh(5E!Brsr9&NWlNw`pY9W)LN0%_n(stk>cnU0E;{J&EVze|+C#C(o>q|?Rm!a8cNsqEa%*SnlX6Dpzz6Z^_d`-pu^TRypIHkS6y)dt!Hnao?cXyKYW{6-!?c$&7!aZJ7Ru+4sH`=qP!q;NY_vB0BT@M=2y3d94W^Za`iSs|c4>DsS%ke6K=Y}JfuYnfXI<{=ewIJR0cu%amHoqtRl4^1VYoaPl(m4B+IhxOe0pjys+=KrzPnSB|l``7&aVKtY?ym>JYe&SZR4(&Amor)@Z={Klo@_vS-ZO(<&EmLU8wsBOcNo{L8$iadbT)9^U*{7Vy?JR8ICS3&VmzqKZ$b-Bfu$9lyPPdXIx7NF)2{81pVS(Vcu6(W5z7O6HeN^A|8|rZP?<GEJ*-RwOGSvrqoaijEaFcGwlaAuQ+>k4uWJjH24&C94v~q9R;*@X=O0y5Hq}(~Sfi;NzZ1#%^4;v$`3H={1vW5WMG^+}gyH;ps-IZC2+!$zwfqmrQi9KvC?t@x|gsw8vzE@?k>e(lqYgru1Vw4l``wsSd3%?r%pvAZ%{*rMWXhTt8bVvb?-OZ#@4c^Ui^~j+-NgX)UYhCeaQ`zZor`6=CS*z#=UD)#cb}@ghX&>cNo%FM5OKEcd-lEk3+%clI;^e9(OxT7|3=IZr!o~s_6!s8(SOV`-A;VfXxS`yvJ#eXPn01{TLYAGc^ak1D@N}{C4{Vz)=yLAOs~cpUjn&3j=WDqkwtsqJ9g?a1e;~i<)LCBe0yoLs&w_+}tx7J5Ga@r`*-X{S9PuT2EMJ$u$CT+?^Wx19FV8Q|-?|si|8oB7SqbF$l8kvIm-6m*Vk5tdrk{Og_S4A!{_|f%h1_>P%g_=}hVk%I?<8gAk8-gc9&S_5H{ac6@Sdp;Pq7~F?p9!FFT}7}L@pa~{pDyX&z{j+bxa=B16VL^O=USB?gwP*EFQ&zcu<TyDyY&k%qP0LP4EPd(G-D;GeoXqP)9)I4;jZ(hJ-wA2>S52xVx1n)Bv(et>7>5+?-2ehc4x*MVOzfA93TCPTE1lKD*{DiV&ODeweh6d<qXdmZq5H#qsGq%+d7a=ZdVrAp{s0iw1dqVh#^^ipQ3~ShC{KoJ{41@iY@eZ$7~vP4zeUviu+07vr}8GD`&Ec*YXql&6{?$te$T79qUGCLt9cGI$LRNQwl+Mwq)uN(98Td#sOYQh5!;N3U^?S(6!H31P+@JFpeE2m}Di(G8YsDo$7DCz0^(-(l%MqNrdYnt4EjV(lgME}NAOm-*RGnmXY!Q43W+t3Z-~<>k?(V_j_W9GZkco<yGD1f%2f_$gRxQ=^KO9{#~f;LZ;5@zg0i>54)pn5kzVgi}1}NuKy4$3j;t?qS8ZhK=HsuM7cv#qeh9pXu-7)hST`J(_wHr4M*vgPSi2v=Bs&IcgUMNufU70<Sx@i&P@h7VUCsUJTYQ{|Dc0o%H"""


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
        prefix="galactic-mvp023-", dir=root.parent
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
                    "Le patch MVP-023 ne s'applique pas proprement dans le worktree."
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
    parent = root / "backups" / ".mvp023-backup"
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
            "Prépare MVP-023 : frontière de découverte progressive, routes "
            "révélées et résultat de reconnaissance persistant."
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
            print("MVP-023 est déjà appliqué ; aucune modification nécessaire.")
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
            prefix="galactic-mvp023-verify-", dir=root.parent
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

        print("MVP-023 appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=17, SAVE_VERSION=18, "
            "RULESET_SCHEMA_VERSION=5"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
