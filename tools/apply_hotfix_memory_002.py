#!/usr/bin/env python3
"""Apply the Galactic stable-font-atlas memory hotfix safely.

This incremental hotfix expects HOTFIX-MEMORY-001 to be present in the
worktree on top of commit 702b5794. It freezes the discovered system font into
one stable Bevy font asset so dynamic French text reuses its glyph atlases.
Dry-run validates the exact state and patch without invoking Cargo or changing
the repository.
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


MIGRATION = "HOTFIX-MEMORY-002"
BASELINE_SHA = "702b5794b27027b3eb2d87e1da8253d2bd187850"
PATCH_SHA256 = "0501221caf3869cdd4dca160157e85deef96645d4c6b81ad6d5036bfa7c899b6"

BASELINE_INDEX_BLOBS = {
    "crates/galactic_client/src/craft_ui.rs": "dcccd903d9c24d33c61960907d7bd5fe362b3c3d",
    "crates/galactic_client/src/lib.rs": "16de37d66a6ba1ba42b6a7872405e0519d36abc1",
    "crates/galactic_client/src/research_ui.rs": "b177be3ccbc3c5e5d9e8de6503a90b2edddedec2",
}
HOTFIX_001_WORKTREE_BLOBS = {
    "crates/galactic_client/src/craft_ui.rs": "c6698ab5a533a6f575448501d4ca81bf27874276",
    "crates/galactic_client/src/lib.rs": "dcb2c43296a78232f2a9d654e82c0a939b914cbb",
    "crates/galactic_client/src/research_ui.rs": "4f61ffcefde64b6849fbd64d4204cb6deb7bde5f",
}
FINAL_LIB_BLOB = "cba25c5e5247ead17d435fa858b7ae100004be0d"
HOTFIX_001_PATHS = frozenset(HOTFIX_001_WORKTREE_BLOBS)
MODIFIED_PATH = "crates/galactic_client/src/lib.rs"

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

PATCH_TEXT = r"""diff --git a/crates/galactic_client/src/lib.rs b/crates/galactic_client/src/lib.rs
index dcb2c43..cba25c5 100644
--- a/crates/galactic_client/src/lib.rs
+++ b/crates/galactic_client/src/lib.rs
@@ -12,7 +12,7 @@ use bevy::render::{
     RenderPlugin,
     settings::{MemoryHints, RenderCreation, WgpuSettings},
 };
-use bevy::text::FontSource;
+use bevy::text::{FontAtlasSet, FontCx, FontSource};
 use bevy::window::{PresentMode, PrimaryWindow};
 use galactic_domain::{
     PlanetId, PlanetKind, ResourceStock, StarClass, SystemId, UniverseConfig, UniverseScalePreset,
@@ -150,7 +150,13 @@ impl Plugin for PresentationPlugin {
     fn build(&self, app: &mut App) {
         app.add_systems(
             Startup,
-            (spawn_scene, spawn_strategic_view, spawn_ui).chain(),
+            (
+                install_stable_ui_font,
+                spawn_scene,
+                spawn_strategic_view,
+                spawn_ui,
+            )
+                .chain(),
         )
         .configure_sets(
             Update,
@@ -275,6 +281,7 @@ struct MemoryDiagnosticSources<'w, 's> {
     meshes: Res<'w, Assets<Mesh>>,
     materials: Res<'w, Assets<StandardMaterial>>,
     images: Res<'w, Assets<Image>>,
+    font_atlases: Res<'w, FontAtlasSet>,
 }
 
 #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
@@ -784,6 +791,27 @@ fn log_startup() {
     info!("Galactic MVP client starting on Bevy 0.19");
 }
 
+fn install_stable_ui_font(mut fonts: ResMut<Assets<Font>>, mut font_cx: ResMut<FontCx>) {
+    let Some(font_data) = stable_system_sans_serif_data(&mut font_cx) else {
+        warn!(
+            "No system sans-serif font could be frozen; using Bevy's ASCII fallback font instead"
+        );
+        return;
+    };
+
+    fonts
+        .insert(AssetId::default(), Font::from_bytes(font_data))
+        .expect("Bevy's default font asset should already be reserved");
+}
+
+fn stable_system_sans_serif_data(font_cx: &mut FontCx) -> Option<Vec<u8>> {
+    let family_name = font_cx.get_family(&FontSource::SansSerif)?.to_owned();
+    let family = font_cx.context.collection.family_by_name(&family_name)?;
+    let font = family.default_font()?;
+    let data = font.load(Some(&mut font_cx.context.source_cache))?;
+    Some(data.as_ref().to_vec())
+}
+
 fn log_memory_diagnostics(
     time: Res<Time>,
     mut diagnostics: ResMut<MemoryDiagnostics>,
@@ -797,7 +825,7 @@ fn log_memory_diagnostics(
     let process = process_memory_snapshot();
     info!(
         target: "galactic_memory",
-        "rss={} MiB anon={} MiB file={} MiB shmem={} MiB swap={} MiB | entities={} strategic={} meshes={} materials={} images={} pending_events={} missions={} reports={}",
+        "rss={} MiB anon={} MiB file={} MiB shmem={} MiB swap={} MiB | entities={} strategic={} meshes={} materials={} images={} font_atlas={} MiB pending_events={} missions={} reports={}",
         kib_to_mib(process.rss_kib),
         kib_to_mib(process.anonymous_kib),
         kib_to_mib(process.file_kib),
@@ -808,6 +836,7 @@ fn log_memory_diagnostics(
         sources.meshes.len(),
         sources.materials.len(),
         sources.images.len(),
+        kib_to_mib(sources.font_atlases.total_bytes(&sources.images) / 1024),
         sources.simulation.pending_events.len(),
         state.missions.len(),
         state.mission_reports.len(),
@@ -1918,7 +1947,6 @@ fn spawn_action_button(
 
 fn ui_text_font(size: f32) -> TextFont {
     TextFont {
-        font: FontSource::SansSerif,
         font_size: FontSize::Px(size),
         ..default()
     }
@@ -4920,7 +4948,14 @@ fn mission_error_text(error: galactic_sim::MissionError) -> String {
 
 #[cfg(test)]
 mod tests {
+    use std::any::TypeId;
+
     use super::*;
+    use bevy::camera::{ComputedCameraValues, RenderTargetInfo, visibility::VisibleEntities};
+    use bevy::sprite::update_text2d_layout;
+    use bevy::text::{
+        LayoutCx, RemSize, ScaleCx, TextIterScratch, TextPipeline, detect_text_needs_rerender,
+    };
 
     #[test]
     fn renderer_favors_bounded_memory_allocations() {
@@ -5297,8 +5332,91 @@ VmSwap:\t      2048 kB
     }
 
     #[test]
-    fn ui_font_uses_a_system_sans_serif() {
-        assert!(matches!(ui_text_font(14.0).font, FontSource::SansSerif));
+    fn ui_font_uses_the_stable_default_asset() {
+        assert!(matches!(
+            ui_text_font(14.0).font,
+            FontSource::Handle(handle) if handle == Handle::default()
+        ));
+    }
+
+    #[test]
+    fn changing_french_text_reuses_its_font_atlas() {
+        let mut app = App::new();
+        app.init_resource::<Assets<Font>>()
+            .init_resource::<Assets<Image>>()
+            .init_resource::<Assets<TextureAtlasLayout>>()
+            .init_resource::<FontAtlasSet>()
+            .init_resource::<TextPipeline>()
+            .init_resource::<FontCx>()
+            .init_resource::<LayoutCx>()
+            .init_resource::<ScaleCx>()
+            .init_resource::<TextIterScratch>()
+            .init_resource::<RemSize>()
+            .add_systems(
+                Update,
+                (detect_text_needs_rerender, update_text2d_layout).chain(),
+            );
+
+        let font_data = {
+            let mut font_cx = app.world_mut().resource_mut::<FontCx>();
+            stable_system_sans_serif_data(&mut font_cx)
+                .unwrap_or_else(|| bevy::text::DEFAULT_FONT_DATA.to_vec())
+        };
+        app.world_mut()
+            .resource_mut::<Assets<Font>>()
+            .insert(AssetId::default(), Font::from_bytes(font_data))
+            .expect("default font handle should be available");
+        let stable_font_data = {
+            let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
+            let mut font = fonts
+                .get_mut(AssetId::default())
+                .expect("the stable font was just inserted");
+            font.alias = "Galactic Stable Sans".into();
+            font.data.clone()
+        };
+        app.world_mut()
+            .resource_mut::<FontCx>()
+            .collection
+            .register_fonts(stable_font_data, None);
+
+        let mut visible_entities = VisibleEntities::default();
+        visible_entities.push(Entity::PLACEHOLDER, TypeId::of::<Sprite>());
+        app.world_mut().spawn((
+            Camera {
+                computed: ComputedCameraValues {
+                    target_info: Some(RenderTargetInfo {
+                        physical_size: UVec2::splat(1_000),
+                        scale_factor: 1.0,
+                    }),
+                    ..default()
+                },
+                ..default()
+            },
+            visible_entities,
+        ));
+        let text_entity = app
+            .world_mut()
+            .spawn((Text2d::new("Hélianthe 0"), ui_text_font(18.0)))
+            .id();
+
+        app.update();
+        let initial_images = app.world().resource::<Assets<Image>>().len();
+        assert!(initial_images > 0);
+
+        for sample in 1..120 {
+            app.world_mut()
+                .entity_mut(text_entity)
+                .get_mut::<Text2d>()
+                .expect("text entity should still exist")
+                .0 = format!("Hélianthe {}", sample % 10);
+            app.update();
+        }
+
+        let final_images = app.world().resource::<Assets<Image>>().len();
+        assert!(
+            final_images <= initial_images + 1,
+            "font atlas images grew from {initial_images} to {final_images}",
+        );
     }
 
     #[test]
"""


def embedded_patch() -> bytes:
    patch = PATCH_TEXT.encode("utf-8")
    actual = hashlib.sha256(patch).hexdigest()
    if actual != PATCH_SHA256:
        raise base.MigrationError(
            f"Patch embarqué corrompu : SHA-256={actual}, attendu {PATCH_SHA256}."
        )
    return patch


def selected_checks(*, full_checks: bool):
    return FULL_CHECK_COMMANDS if full_checks else TARGETED_CHECK_COMMANDS


def verify_hotfix_001_state(root: Path, *, force: bool) -> None:
    problems: list[str] = []
    current = base.head_sha(root)
    if current != BASELINE_SHA:
        problems.append(f"HEAD={current}, attendu {BASELINE_SHA}")

    for relative, expected in BASELINE_INDEX_BLOBS.items():
        actual = base.index_blob(root, relative)
        if actual != expected:
            problems.append(
                f"blob index {relative}={actual or '<absent>'}, attendu {expected}"
            )

    for relative, expected in HOTFIX_001_WORKTREE_BLOBS.items():
        actual = base.worktree_blob(root, relative)
        if actual != expected:
            problems.append(
                f"blob de travail {relative}={actual or '<absent>'}, attendu {expected}"
            )

    changed = base.changed_paths(root)
    if changed != HOTFIX_001_PATHS:
        missing = sorted(HOTFIX_001_PATHS - changed)
        extra = sorted(changed - HOTFIX_001_PATHS)
        if missing:
            problems.append("fichiers HOTFIX-001 absents : " + ", ".join(missing))
        if extra:
            problems.append("autres fichiers suivis modifiés : " + ", ".join(extra))

    if not problems:
        return
    details = "\n  - ".join(problems)
    if force:
        print(
            "AVERTISSEMENT --force : garde incrémentale ignorée :\n"
            f"  - {details}",
            file=sys.stderr,
        )
        return
    raise base.MigrationError(
        "État incompatible : HOTFIX-MEMORY-001 doit être appliqué mais non "
        "commité sur la baseline 702b5794.\n"
        f"  - {details}\n"
        "Utilisez --force uniquement après avoir vérifié manuellement les écarts."
    )


def validate_final_state(worktree: Path) -> None:
    actual_paths = base.changed_paths(worktree)
    if actual_paths != HOTFIX_001_PATHS:
        raise base.MigrationError(
            "Périmètre final invalide : "
            f"{', '.join(sorted(actual_paths)) or '<vide>'}."
        )
    actual_lib = base.worktree_blob(worktree, MODIFIED_PATH)
    if actual_lib != FINAL_LIB_BLOB:
        raise base.MigrationError(
            f"Blob final {MODIFIED_PATH}={actual_lib}, attendu {FINAL_LIB_BLOB}."
        )


def validated_patch(
    root: Path,
    patch: bytes,
    *,
    run_checks: bool,
    full_checks: bool,
) -> bytes:
    with tempfile.TemporaryDirectory(
        prefix="galactic-memory-hotfix-002-", dir=root.parent
    ) as temporary:
        worktree = Path(temporary) / "worktree"
        added = False
        try:
            base.run(
                ("git", "worktree", "add", "--detach", str(worktree), BASELINE_SHA),
                cwd=root,
            )
            added = True

            for relative in sorted(HOTFIX_001_PATHS):
                destination = worktree / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(root / relative, destination)

            if not base.patch_check(worktree, patch):
                raise base.MigrationError(
                    "Le correctif d'atlas ne s'applique pas à HOTFIX-MEMORY-001."
                )
            base.run(("git", "apply", "-"), cwd=worktree, input_bytes=patch)

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
            validate_final_state(worktree)
            return patch
        finally:
            if added:
                base.run(
                    ("git", "worktree", "remove", "--force", str(worktree)),
                    cwd=root,
                    check=False,
                )


def make_backup(root: Path, patch: bytes) -> Path:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
    parent = root / "backups" / ".memory-hotfix-002-backup"
    destination = parent / stamp
    counter = 1
    while destination.exists():
        destination = parent / f"{stamp}-{counter}"
        counter += 1
    destination.mkdir(parents=True)

    source = root / MODIFIED_PATH
    target = destination / MODIFIED_PATH
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)

    manifest = {
        "migration": MIGRATION,
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "baseline_sha": BASELINE_SHA,
        "actual_head_sha": base.head_sha(root),
        "validated_patch_sha256": hashlib.sha256(patch).hexdigest(),
        "backed_up_paths": [MODIFIED_PATH],
    }
    (destination / "manifest.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    return destination


def apply_to_main(root: Path, patch: bytes, *, force: bool) -> Path:
    verify_hotfix_001_state(root, force=force)
    if not base.patch_check(root, patch):
        raise base.MigrationError(
            "Le patch validé ne s'applique plus au dépôt principal. "
            "Aucun fichier source n'a été modifié."
        )
    backup = make_backup(root, patch)
    verify_hotfix_001_state(root, force=force)
    if not base.patch_check(root, patch):
        raise base.MigrationError(
            "Le dépôt a changé pendant la sauvegarde. "
            "Aucun fichier source n'a été modifié."
        )
    base.run(("git", "apply", "-"), cwd=root, input_bytes=patch)
    return backup


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Applique le hotfix des atlas de police Bevy après "
            "HOTFIX-MEMORY-001 non commité."
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
        help="valide baseline, état HOTFIX-001 et patch sans compiler ni modifier",
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
        base.ensure_command("git")
        run_checks = (
            args.checks
            or args.full_checks
            or (not args.dry_run and not args.skip_checks)
        )

        root = base.resolve_root(args.root)
        patch = embedded_patch()

        if base.worktree_blob(root, MODIFIED_PATH) == FINAL_LIB_BLOB:
            print("HOTFIX-MEMORY-002 est déjà appliqué ; aucune modification nécessaire.")
            return 0

        if run_checks:
            base.ensure_command("cargo")

        verify_hotfix_001_state(root, force=args.force)
        candidate = validated_patch(
            root,
            patch,
            run_checks=run_checks,
            full_checks=args.full_checks,
        )

        if args.dry_run:
            checks_label = " avec contrôles Cargo" if run_checks else ""
            print(
                f"Dry-run réussi{checks_label} : HOTFIX-001, patch et périmètre "
                "valides. Le dépôt principal n'a pas été modifié."
            )
            return 0

        backup = apply_to_main(root, candidate, force=args.force)
        print("HOTFIX-MEMORY-002 appliqué avec succès.")
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
