#!/usr/bin/env python3
"""Fix the Bevy B0001 startup panic introduced by Galactic MVP-029.

The hotfix makes the transport launch and cargo-preset button queries explicitly
disjoint, then adds a regression test that initializes the affected Bevy system.
It targets a repository where apply_mvp_029.py has already been applied.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


MIGRATION = "MVP-029-HOTFIX-001"
BASELINE_SHA = "ea6e91ccfb7dd72b151db728a5183ffa5c3b3f86"
TARGET_PATH = Path("crates/galactic_client/src/lib.rs")
BEFORE_SHA256 = "df9483a36732417a6ae96d3f7dda47c97dab8db881cd0e15fc963850271d001b"
AFTER_SHA256 = "f1966a255bf9ca34a029696fd50188278cb0e766aa5fba48ddbd10b9f43ef241"

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

OLD_QUERY = b"""\
    mut launch_buttons: Query<
        (&Interaction, &mut BackgroundColor, &mut Outline),
        With<ManagementTransportLaunchButton>,
    >,
    mut preset_buttons: Query<(
        &ManagementTransportPresetButton,
        &Interaction,
        &mut BackgroundColor,
        &mut Outline,
    )>,
"""

NEW_QUERY = b"""\
    mut launch_buttons: ManagementTransportLaunchStyleQuery,
    mut preset_buttons: ManagementTransportPresetStyleQuery,
"""

OLD_ALIAS_ANCHOR = b"""\
#[derive(Component)]
struct ManagementTransportPresetButton {
    preset: TransportCargoPreset,
}

#[derive(Component)]
struct ManagementQueueProgressFill;
"""

NEW_ALIAS_ANCHOR = b"""\
#[derive(Component)]
struct ManagementTransportPresetButton {
    preset: TransportCargoPreset,
}

type ManagementTransportLaunchStyleQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut Outline,
    ),
    (
        With<ManagementTransportLaunchButton>,
        Without<ManagementTransportPresetButton>,
    ),
>;

type ManagementTransportPresetStyleQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ManagementTransportPresetButton,
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut Outline,
    ),
    Without<ManagementTransportLaunchButton>,
>;

#[derive(Component)]
struct ManagementQueueProgressFill;
"""

OLD_TEST_ANCHOR = b"""\
    #[test]
    fn renderer_favors_bounded_memory_allocations() {
"""

NEW_TEST_ANCHOR = b"""\
    #[test]
    fn transport_management_queries_are_disjoint() {
        let mut world = World::new();
        let mut system = IntoSystem::into_system(update_colony_management_transport);

        system.initialize(&mut world);
    }

    #[test]
    fn renderer_favors_bounded_memory_allocations() {
"""


class HotfixError(RuntimeError):
    """A safe, user-facing hotfix failure."""


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def run(
    command: tuple[str, ...],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
    input_bytes: bytes | None = None,
    capture: bool = False,
    check: bool = True,
) -> subprocess.CompletedProcess[bytes]:
    print("+", " ".join(command))
    try:
        return subprocess.run(
            command,
            cwd=cwd,
            env=env,
            input=input_bytes,
            stdout=subprocess.PIPE if capture else None,
            stderr=subprocess.PIPE if capture else None,
            check=check,
        )
    except FileNotFoundError as error:
        raise HotfixError(f"Commande introuvable : {command[0]}") from error
    except subprocess.CalledProcessError as error:
        if capture:
            if error.stdout:
                sys.stderr.buffer.write(error.stdout)
            if error.stderr:
                sys.stderr.buffer.write(error.stderr)
        raise HotfixError(
            f"La commande a échoué ({error.returncode}) : {' '.join(command)}"
        ) from error


def resolve_root(value: str) -> Path:
    root = Path(value).expanduser().resolve()
    if not (root / ".git").exists():
        raise HotfixError(f"{root} n'est pas la racine d'un dépôt Git.")
    if not (root / "Cargo.toml").is_file():
        raise HotfixError(f"{root} ne contient pas Cargo.toml.")
    if not (root / TARGET_PATH).is_file():
        raise HotfixError(f"Fichier cible absent : {TARGET_PATH}")
    return root


def head_sha(root: Path) -> str:
    result = run(
        ("git", "rev-parse", "HEAD"),
        cwd=root,
        capture=True,
    )
    return result.stdout.decode("ascii").strip()


def transform(source: bytes, *, force: bool) -> bytes:
    source_digest = digest(source)
    if source_digest != BEFORE_SHA256 and not force:
        raise HotfixError(
            f"{TARGET_PATH} ne correspond pas à MVP-029 appliqué "
            f"({source_digest}, attendu {BEFORE_SHA256})."
        )

    if source.count(OLD_QUERY) != 1:
        raise HotfixError(
            "La requête de transport attendue est absente ou ambiguë ; "
            "aucune modification n'a été effectuée."
        )
    if source.count(OLD_ALIAS_ANCHOR) != 1:
        raise HotfixError(
            "Le point d'insertion des alias est absent ou ambigu ; "
            "aucune modification n'a été effectuée."
        )
    if source.count(OLD_TEST_ANCHOR) != 1:
        raise HotfixError(
            "Le point d'insertion du test est absent ou ambigu ; "
            "aucune modification n'a été effectuée."
        )

    candidate = source.replace(OLD_QUERY, NEW_QUERY, 1)
    candidate = candidate.replace(OLD_ALIAS_ANCHOR, NEW_ALIAS_ANCHOR, 1)
    candidate = candidate.replace(OLD_TEST_ANCHOR, NEW_TEST_ANCHOR, 1)
    candidate_digest = digest(candidate)
    if candidate_digest != AFTER_SHA256:
        raise HotfixError(
            "Le résultat ne correspond pas au correctif canonique "
            f"({candidate_digest}, attendu {AFTER_SHA256})."
        )
    return candidate


def verify_head(root: Path, *, force: bool) -> str:
    actual = head_sha(root)
    if actual != BASELINE_SHA and not force:
        raise HotfixError(
            f"HEAD vaut {actual}, attendu {BASELINE_SHA}. "
            "Utilisez --force seulement si vous avez vérifié la divergence."
        )
    return actual


def validation_environment(root: Path) -> dict[str, str]:
    environment = os.environ.copy()
    environment.setdefault(
        "CARGO_TARGET_DIR",
        str(root / "target" / "mvp-validation"),
    )
    return environment


def validate_in_worktree(
    root: Path,
    *,
    candidate: bytes,
    run_checks: bool,
) -> None:
    dirty_patch = run(
        ("git", "diff", "--binary", "HEAD", "--"),
        cwd=root,
        capture=True,
    ).stdout
    if not dirty_patch:
        raise HotfixError(
            "Le dépôt ne contient pas le patch MVP-029 non commité attendu."
        )

    with tempfile.TemporaryDirectory(
        prefix="galactic-mvp029-hotfix-", dir=root.parent
    ) as temporary:
        worktree = Path(temporary) / "worktree"
        added = False
        try:
            run(
                ("git", "worktree", "add", "--detach", str(worktree), head_sha(root)),
                cwd=root,
            )
            added = True
            run(
                ("git", "apply", "--binary", "-"),
                cwd=worktree,
                input_bytes=dirty_patch,
            )

            validation_target = worktree / TARGET_PATH
            validation_source = validation_target.read_bytes()
            if digest(validation_source) != BEFORE_SHA256:
                raise HotfixError(
                    "La copie de validation ne reproduit pas exactement MVP-029."
                )
            validation_target.write_bytes(candidate)

            if run_checks:
                print("Contrôles Rust complets dans la copie de validation :")
                environment = validation_environment(root)
                for command in CHECK_COMMANDS:
                    run(command, cwd=worktree, env=environment)
            else:
                print("Contrôles Cargo non demandés pour ce dry-run.")

            run(("git", "diff", "--check"), cwd=worktree)
            final_source = validation_target.read_bytes()
            if digest(final_source) != AFTER_SHA256:
                raise HotfixError(
                    "Un contrôle a modifié le correctif canonique dans la copie "
                    "de validation."
                )
        finally:
            if added:
                run(
                    ("git", "worktree", "remove", "--force", str(worktree)),
                    cwd=root,
                    check=False,
                )


def make_backup(root: Path, source: bytes, actual_head: str) -> Path:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
    destination = root / ".mvp029-hotfix-backup" / stamp
    counter = 1
    while destination.exists():
        destination = root / ".mvp029-hotfix-backup" / f"{stamp}-{counter}"
        counter += 1

    backup_target = destination / TARGET_PATH
    backup_target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(root / TARGET_PATH, backup_target)
    if backup_target.read_bytes() != source:
        raise HotfixError("La vérification de la sauvegarde a échoué.")

    manifest = {
        "migration": MIGRATION,
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "baseline_sha": BASELINE_SHA,
        "actual_head_sha": actual_head,
        "target_path": str(TARGET_PATH),
        "before_sha256": BEFORE_SHA256,
        "after_sha256": AFTER_SHA256,
    }
    (destination / "manifest.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    return destination


def apply_candidate(root: Path, candidate: bytes, actual_head: str) -> Path:
    target = root / TARGET_PATH
    source = target.read_bytes()
    if digest(source) != BEFORE_SHA256:
        raise HotfixError(
            "Le fichier cible a changé depuis la validation ; "
            "aucune modification n'a été effectuée."
        )

    backup = make_backup(root, source, actual_head)
    temporary = target.with_name(f".{target.name}.mvp029-hotfix.tmp")
    try:
        temporary.write_bytes(candidate)
        shutil.copymode(target, temporary)
        if digest(temporary.read_bytes()) != AFTER_SHA256:
            raise HotfixError("La vérification du fichier temporaire a échoué.")
        os.replace(temporary, target)
    finally:
        temporary.unlink(missing_ok=True)

    if digest(target.read_bytes()) != AFTER_SHA256:
        raise HotfixError(
            f"Échec de la vérification finale. Restaurez {backup / TARGET_PATH}."
        )
    return backup


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Corrige le panic Bevy B0001 au démarrage après MVP-029."
    )
    parser.add_argument(
        "--root",
        default=".",
        help="racine du dépôt Galactic (défaut : répertoire courant)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="valide le correctif sans modifier le dépôt",
    )
    parser.add_argument(
        "--checks",
        action="store_true",
        help="lance les cinq contrôles Cargo pendant un dry-run",
    )
    parser.add_argument(
        "--skip-checks",
        action="store_true",
        help="ignore les cinq contrôles Cargo pendant l'application (déconseillé)",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="ignore la garde HEAD/fichier (dangereux ; résultat exact toujours exigé)",
    )
    args = parser.parse_args()
    if args.checks and args.skip_checks:
        parser.error("--checks est incompatible avec --skip-checks")
    return args


def main() -> int:
    args = parse_args()
    try:
        root = resolve_root(args.root)
        target = root / TARGET_PATH
        source = target.read_bytes()
        source_digest = digest(source)

        if source_digest == AFTER_SHA256:
            print(f"{MIGRATION} est déjà appliqué ; aucune modification nécessaire.")
            return 0

        actual_head = verify_head(root, force=args.force)
        candidate = transform(source, force=args.force)
        run_checks = args.checks or (not args.dry_run and not args.skip_checks)

        if args.skip_checks:
            print(
                "AVERTISSEMENT : --skip-checks est déconseillé ; "
                "les contrôles Rust ne seront pas exécutés.",
                file=sys.stderr,
            )

        if args.dry_run and not run_checks:
            print(
                f"Dry-run réussi : {TARGET_PATH} peut recevoir {MIGRATION} "
                "sans modification."
            )
            return 0

        validate_in_worktree(root, candidate=candidate, run_checks=run_checks)
        if args.dry_run:
            print(f"Dry-run avec contrôles réussi : {MIGRATION} est applicable.")
            return 0

        backup = apply_candidate(root, candidate, actual_head)
        print(f"{MIGRATION} appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        return 0
    except HotfixError as error:
        print(f"ERREUR : {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
