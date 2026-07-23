#!/usr/bin/env python3
"""
Finalise MVP-004 à partir du commit GitHub réellement poussé :

    8fe88093930873b24a1fbd49d897530bde44ccb8
    "feat add mvp 3 seed"

Ce commit contient déjà presque tout MVP-004. Ce script ne réécrit donc pas
l'architecture. Il :

- vérifie que la séparation univers immuable / état mutable est présente ;
- corrige l'import Rust inutilisé qui bloque Clippy avec -D warnings ;
- corrige l'ancien test Result<Simulation, SaveError> s'il est encore présent ;
- ajoute les répertoires de sauvegarde des scripts au .gitignore ;
- crée une sauvegarde avant chaque écriture ;
- lance fmt, clippy, tests et build release.

Usage :
    python tools/reconcile_mvp_004.py --dry-run
    python tools/reconcile_mvp_004.py
    python tools/reconcile_mvp_004.py --skip-checks
    python tools/reconcile_mvp_004.py --root /chemin/vers/galactic

Le script est idempotent.
"""

from __future__ import annotations

import argparse
import difflib
import shutil
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path


EXPECTED_BASELINE_COMMIT = "8fe88093930873b24a1fbd49d897530bde44ccb8"


@dataclass(frozen=True)
class FileUpdate:
    path: Path
    before: str
    after: str


def run(
    command: list[str],
    *,
    cwd: Path,
    check: bool = True,
    capture: bool = True,
) -> subprocess.CompletedProcess[str]:
    print("$", " ".join(command))
    result = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
    )
    if capture and result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n")
    if check and result.returncode != 0:
        raise SystemExit(
            f"Commande en échec ({result.returncode}) : {' '.join(command)}"
        )
    return result


def find_repo_root(start: Path) -> Path:
    candidates = [start, *start.parents]
    for candidate in candidates:
        if (
            (candidate / ".git").exists()
            and (candidate / "Cargo.toml").exists()
            and (candidate / "crates/galactic_sim/src/state.rs").exists()
            and (candidate / "crates/galactic_persistence/src/lib.rs").exists()
        ):
            return candidate

    raise SystemExit(
        "Racine du dépôt Galactic introuvable. "
        "Utilise --root /chemin/vers/galactic."
    )


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def normalize(text: str) -> str:
    return text.rstrip() + "\n"


def verify_git_baseline(root: Path, force: bool) -> None:
    head = run(["git", "rev-parse", "HEAD"], cwd=root).stdout.strip()

    if head == EXPECTED_BASELINE_COMMIT:
        print(f"Baseline GitHub reconnue : {head}")
        return

    ancestor = run(
        [
            "git",
            "merge-base",
            "--is-ancestor",
            EXPECTED_BASELINE_COMMIT,
            "HEAD",
        ],
        cwd=root,
        check=False,
    )

    if ancestor.returncode == 0:
        print(
            "Le dépôt contient la baseline attendue et possède des commits "
            f"supplémentaires : HEAD={head}"
        )
        return

    if force:
        print(
            "WARNING: la baseline attendue n'est pas dans l'historique local, "
            "mais --force autorise la suite."
        )
        return

    raise SystemExit(
        "Le HEAD local ne correspond pas au commit GitHub analysé et ne le "
        "contient pas dans son historique.\n"
        f"HEAD local : {head}\n"
        f"Baseline attendue : {EXPECTED_BASELINE_COMMIT}\n\n"
        "La branche distante a probablement été réécrite. Synchronise d'abord :\n"
        "  git fetch origin\n"
        "  git status\n"
        "Puis rebase ou reset ta branche selon les changements locaux.\n"
        "Utilise --force uniquement après avoir vérifié le diff."
    )


def verify_mvp_004_architecture(root: Path) -> None:
    state = read_text(root / "crates/galactic_sim/src/state.rs")
    simulation = read_text(root / "crates/galactic_sim/src/simulation.rs")
    universe = read_text(root / "crates/galactic_sim/src/universe.rs")
    persistence = read_text(root / "crates/galactic_persistence/src/lib.rs")
    client = read_text(root / "crates/galactic_client/src/lib.rs")

    failures: list[str] = []

    if "pub struct GameState" not in state:
        failures.append("GameState est absent.")
    if "pub universe:" in state or "UniverseDefinition" in state:
        failures.append(
            "GameState contient encore l'univers généré ou dépend de "
            "UniverseDefinition."
        )
    if "pub version: u32" not in state or "GAME_STATE_VERSION" not in state:
        failures.append("GameState n'a pas de version de contrat mutable.")

    if "pub struct UniverseRepository" not in universe:
        failures.append("UniverseRepository est absent.")
    if "definition: UniverseDefinition" not in universe:
        failures.append(
            "UniverseRepository ne possède pas la définition générée."
        )
    if "pub fn definition(&self) -> &UniverseDefinition" not in universe:
        failures.append(
            "UniverseRepository n'expose pas de lecture immuable de la définition."
        )
    if "definition_mut" in universe or "universe_mut" in universe:
        failures.append(
            "Un accesseur mutable vers l'univers généré a été détecté."
        )

    if "universe: UniverseRepository" not in simulation:
        failures.append(
            "Simulation ne possède pas séparément UniverseRepository."
        )
    if "state: GameState" not in simulation:
        failures.append("Simulation ne possède pas séparément GameState.")
    if "pub fn universe(&self) -> &UniverseDefinition" not in simulation:
        failures.append(
            "Simulation n'expose pas l'univers généré en lecture seule."
        )
    if "pub fn state_mut(&mut self) -> &mut GameState" not in simulation:
        failures.append(
            "Simulation n'expose pas l'état mutable de partie."
        )

    if "pub struct UniverseReference" not in persistence:
        failures.append("UniverseReference est absent de la persistance.")
    if "pub struct MutableGameSave" not in persistence:
        failures.append("MutableGameSave est absent de la persistance.")
    if "pub universe: UniverseReference" not in persistence:
        failures.append(
            "SaveGame ne référence pas séparément l'univers généré."
        )
    if "pub state: MutableGameSave" not in persistence:
        failures.append(
            "SaveGame ne contient pas séparément l'état mutable."
        )
    if "GenerationFingerprintMismatch" not in persistence:
        failures.append(
            "La restauration ne vérifie pas le fingerprint de génération."
        )

    if ".state().universe" in client:
        failures.append(
            "Le client lit encore l'univers via GameState au lieu de Simulation."
        )
    if ".universe()" not in client:
        failures.append(
            "Le client n'utilise pas l'accès immuable Simulation::universe()."
        )

    if failures:
        formatted = "\n".join(f"  - {failure}" for failure in failures)
        raise SystemExit(
            "La structure locale ne correspond pas au MVP-004 déjà présent "
            f"dans le commit GitHub :\n{formatted}\n\n"
            "Le script refuse de réécrire aveuglément ces fichiers."
        )

    print("Architecture MVP-004 détectée et cohérente.")


def patch_persistence(source: str) -> str:
    updated = source

    # Le commit poussé importe GAME_STATE_VERSION deux fois :
    # une fois au niveau du module (inutilisée en build normal), et une fois
    # correctement dans le module de tests.
    updated = updated.replace(
        "    ColonyState, GAME_STATE_VERSION, GameState, SelectionTarget, "
        "Simulation, SimulationBuildError,\n",
        "    ColonyState, GameState, SelectionTarget, Simulation, "
        "SimulationBuildError,\n",
        1,
    )

    # Variante après rustfmt ou modifications manuelles.
    updated = updated.replace(
        "    ColonyState, GAME_STATE_VERSION, GameState, SelectionTarget, Simulation,\n"
        "    SimulationBuildError, TimeSpeed,\n",
        "    ColonyState, GameState, SelectionTarget, Simulation, "
        "SimulationBuildError, TimeSpeed,\n",
        1,
    )

    # Garantit que la constante reste disponible uniquement pour les tests.
    test_header = (
        "#[cfg(test)]\n"
        "mod tests {\n"
        "    use galactic_domain::UniverseConfig;\n"
    )
    expected_test_import = (
        "    use galactic_sim::{GAME_STATE_VERSION, GameCommand, TimeSpeed};\n"
    )
    if test_header in updated and expected_test_import not in updated:
        updated = updated.replace(
            test_header,
            test_header + expected_test_import,
            1,
        )

    # Corrige l'ancienne assertion qui exigeait PartialEq sur Simulation.
    old_assertion = """        assert_eq!(
            restore_from_snapshot(&save),
            Err(SaveError::UnsupportedVersion(999))
        );"""
    new_assertion = """        assert!(matches!(
            restore_from_snapshot(&save),
            Err(SaveError::UnsupportedVersion(999))
        ));"""
    updated = updated.replace(old_assertion, new_assertion)

    return updated


def patch_gitignore(source: str) -> str:
    lines = source.splitlines()
    entry = ".mvp*-backup/"
    if entry not in lines:
        if lines and lines[-1] != "":
            lines.append("")
        lines.extend(
            [
                "# Local backups created by MVP migration scripts",
                entry,
            ]
        )
    return "\n".join(lines)


def collect_updates(root: Path) -> list[FileUpdate]:
    updates: list[FileUpdate] = []

    persistence_path = root / "crates/galactic_persistence/src/lib.rs"
    persistence_before = read_text(persistence_path)
    persistence_after = normalize(patch_persistence(persistence_before))
    if persistence_before != persistence_after:
        updates.append(
            FileUpdate(
                persistence_path,
                persistence_before,
                persistence_after,
            )
        )

    gitignore_path = root / ".gitignore"
    gitignore_before = read_text(gitignore_path)
    gitignore_after = normalize(patch_gitignore(gitignore_before))
    if gitignore_before != gitignore_after:
        updates.append(
            FileUpdate(gitignore_path, gitignore_before, gitignore_after)
        )

    return updates


def show_diff(update: FileUpdate, root: Path) -> None:
    relative = update.path.relative_to(root)
    diff = difflib.unified_diff(
        update.before.splitlines(keepends=True),
        update.after.splitlines(keepends=True),
        fromfile=f"a/{relative}",
        tofile=f"b/{relative}",
    )
    print("".join(diff), end="")


def apply_updates(
    updates: list[FileUpdate],
    *,
    root: Path,
    dry_run: bool,
) -> Path | None:
    if not updates:
        print("Aucun correctif de fichier nécessaire.")
        return None

    if dry_run:
        for update in updates:
            show_diff(update, root)
        return None

    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    backup_root = root / ".mvp004-backup" / timestamp

    for update in updates:
        relative = update.path.relative_to(root)
        backup_path = backup_root / relative
        backup_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(update.path, backup_path)

        update.path.write_text(update.after, encoding="utf-8")
        print(f"+ mis à jour : {relative}")

    print(f"Sauvegarde : {backup_root}")
    return backup_root


def run_quality_checks(root: Path) -> None:
    run(["cargo", "fmt", "--all"], cwd=root, capture=False)
    run(
        [
            "cargo",
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
        cwd=root,
        capture=False,
    )
    run(["cargo", "test", "--workspace"], cwd=root, capture=False)
    run(["cargo", "build", "--release"], cwd=root, capture=False)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Réconcilie et finalise MVP-004 depuis la baseline GitHub."
    )
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--skip-checks", action="store_true")
    parser.add_argument(
        "--force",
        action="store_true",
        help="Ignore uniquement la vérification du commit de baseline.",
    )
    args = parser.parse_args()

    root = find_repo_root(args.root.resolve())
    print(f"Dépôt : {root}")

    verify_git_baseline(root, args.force)

    status = run(["git", "status", "--porcelain"], cwd=root).stdout
    if status.strip():
        print(
            "WARNING: le working tree contient déjà des changements. "
            "Les fichiers modifiés par ce script seront sauvegardés."
        )
        print(status, end="" if status.endswith("\n") else "\n")

    verify_mvp_004_architecture(root)

    updates = collect_updates(root)
    apply_updates(updates, root=root, dry_run=args.dry_run)

    if args.dry_run:
        print(
            f"\nDry-run terminé : {len(updates)} fichier(s) seraient modifiés."
        )
        return 0

    # Relit les sources après correction pour éviter de valider une structure
    # différente de celle effectivement écrite.
    verify_mvp_004_architecture(root)

    if args.skip_checks:
        print(
            "\nChecks ignorés. Lance ensuite :\n"
            "  cargo fmt --all\n"
            "  cargo clippy --workspace --all-targets --all-features -- -D warnings\n"
            "  cargo test --workspace\n"
            "  cargo build --release"
        )
    else:
        run_quality_checks(root)

    print(
        "\nMVP-004 réconcilié avec la version GitHub poussée.\n"
        "Vérifie maintenant :\n"
        "  git diff\n"
        "  git status\n"
        "Puis committe les correctifs sous un commit MVP-004 dédié."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
