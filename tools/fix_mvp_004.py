#!/usr/bin/env python3
"""Correctif idempotent pour les deux erreurs Clippy de MVP-004.

Usage depuis la racine du dépôt :
    python tools/fix_mvp_004.py
    python tools/fix_mvp_004.py --dry-run
    python tools/fix_mvp_004.py --skip-checks

Le script corrige à la fois le fichier Rust déjà généré et, lorsqu'il est
présent, le générateur tools/apply_mvp_004.py.
"""

from __future__ import annotations

import argparse
import difflib
import subprocess
from pathlib import Path


def find_root(start: Path) -> Path:
    for candidate in (start, *start.parents):
        if (
            (candidate / "Cargo.toml").exists()
            and (candidate / "crates/galactic_persistence/src/lib.rs").exists()
        ):
            return candidate
    raise SystemExit("Racine du dépôt Galactic introuvable. Utilise --root.")


def run(command: list[str], cwd: Path) -> None:
    print("$", " ".join(command))
    completed = subprocess.run(command, cwd=cwd)
    if completed.returncode != 0:
        raise SystemExit(
            f"Commande en échec ({completed.returncode}) : {' '.join(command)}"
        )


def patch_persistence(source: str) -> str:
    source = source.replace(
        """use galactic_sim::{
    ColonyState, GAME_STATE_VERSION, GameState, SelectionTarget, Simulation,
    SimulationBuildError, TimeSpeed,
};""",
        """use galactic_sim::{
    ColonyState, GameState, SelectionTarget, Simulation, SimulationBuildError, TimeSpeed,
};""",
    )

    source = source.replace(
        """mod tests {
    use galactic_domain::UniverseConfig;
    use galactic_sim::{GameCommand, TimeSpeed};""",
        """mod tests {
    use galactic_domain::UniverseConfig;
    use galactic_sim::{GAME_STATE_VERSION, GameCommand, TimeSpeed};""",
    )

    source = source.replace(
        """        assert_eq!(
            restore_from_snapshot(&save),
            Err(SaveError::UnsupportedVersion(999))
        );""",
        """        assert!(matches!(
            restore_from_snapshot(&save),
            Err(SaveError::UnsupportedVersion(999))
        ));""",
    )
    return source


def patch_generator(source: str) -> str:
    source = source.replace(
        """use galactic_sim::{{
    ColonyState, GAME_STATE_VERSION, GameState, SelectionTarget, Simulation,
    SimulationBuildError, TimeSpeed,
}};""",
        """use galactic_sim::{{
    ColonyState, GameState, SelectionTarget, Simulation, SimulationBuildError, TimeSpeed,
}};""",
    )

    source = source.replace(
        """mod tests {{
    use galactic_domain::UniverseConfig;
    use galactic_sim::{{GameCommand, TimeSpeed}};""",
        """mod tests {{
    use galactic_domain::UniverseConfig;
    use galactic_sim::{{GAME_STATE_VERSION, GameCommand, TimeSpeed}};""",
    )

    source = source.replace(
        """        assert_eq!(
            restore_from_snapshot(&save),
            Err(SaveError::UnsupportedVersion(999))
        );""",
        """        assert!(matches!(
            restore_from_snapshot(&save),
            Err(SaveError::UnsupportedVersion(999))
        ));""",
    )
    return source


def update(path: Path, transform, dry_run: bool) -> bool:
    if not path.exists():
        return False

    before = path.read_text(encoding="utf-8")
    after = transform(before)
    if after == before:
        print(f"= déjà corrigé : {path}")
        return False

    if dry_run:
        print(
            "".join(
                difflib.unified_diff(
                    before.splitlines(keepends=True),
                    after.splitlines(keepends=True),
                    fromfile=str(path),
                    tofile=str(path),
                )
            )
        )
    else:
        path.write_text(after, encoding="utf-8")
        print(f"+ corrigé : {path}")
    return True


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--skip-checks", action="store_true")
    args = parser.parse_args()

    root = find_root(args.root.resolve())
    changed = False
    changed |= update(
        root / "crates/galactic_persistence/src/lib.rs",
        patch_persistence,
        args.dry_run,
    )
    changed |= update(
        root / "tools/apply_mvp_004.py",
        patch_generator,
        args.dry_run,
    )

    if args.dry_run:
        print("Dry-run terminé.")
        return 0

    if not changed:
        print("Aucune modification nécessaire.")

    if not args.skip_checks:
        run(["cargo", "fmt", "--all"], root)
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
            root,
        )
        run(["cargo", "test", "--workspace"], root)

    print("Correctif MVP-004 appliqué.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
