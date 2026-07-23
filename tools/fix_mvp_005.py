#!/usr/bin/env python3
"""
Corrige TimeSpeed pour satisfaire Clippy avec -D warnings, puis relance les checks.

Usage :
    python tools/fix_mvp_005.py --dry-run
    python tools/fix_mvp_005.py
    python tools/fix_mvp_005.py --skip-checks
"""

from __future__ import annotations

import argparse
import difflib
import shutil
import subprocess
from datetime import datetime
from pathlib import Path


OLD_ENUM = """#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeSpeed {
    Paused,
    X1,
    X2,
    X4,
}
"""

NEW_ENUM = """#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TimeSpeed {
    Paused,
    #[default]
    X1,
    X2,
    X4,
}
"""

OLD_IMPL = """
impl Default for TimeSpeed {
    fn default() -> Self {
        Self::X1
    }
}
"""


def run(command: list[str], *, cwd: Path) -> None:
    print("$", " ".join(command))
    result = subprocess.run(command, cwd=cwd)
    if result.returncode != 0:
        raise SystemExit(
            f"Commande en échec ({result.returncode}) : {' '.join(command)}"
        )


def find_root(start: Path) -> Path:
    for candidate in [start, *start.parents]:
        if (
            (candidate / ".git").exists()
            and (candidate / "Cargo.toml").exists()
            and (candidate / "crates/galactic_sim/src/time.rs").exists()
        ):
            return candidate
    raise SystemExit("Racine Galactic introuvable. Utilise --root.")


def patch_time(source: str) -> str:
    if NEW_ENUM in source and OLD_IMPL not in source:
        return source

    updated = source
    if OLD_ENUM in updated:
        updated = updated.replace(OLD_ENUM, NEW_ENUM, 1)
    elif NEW_ENUM not in updated:
        raise SystemExit(
            "Le bloc TimeSpeed attendu n'a pas été trouvé dans time.rs."
        )

    updated = updated.replace(OLD_IMPL, "\n", 1)
    return updated.rstrip() + "\n"


def patch_generator(source: str) -> str:
    old_enum_escaped = OLD_ENUM.replace("\n", "\\n")
    new_enum_escaped = NEW_ENUM.replace("\n", "\\n")
    old_impl_escaped = OLD_IMPL.replace("\n", "\\n")

    updated = source
    if old_enum_escaped in updated:
        updated = updated.replace(old_enum_escaped, new_enum_escaped, 1)
    updated = updated.replace(old_impl_escaped, "\\n", 1)
    return updated


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--skip-checks", action="store_true")
    args = parser.parse_args()

    root = find_root(args.root.resolve())
    print(f"Repository: {root}")

    targets = [
        root / "crates/galactic_sim/src/time.rs",
        root / "tools/apply_mvp_005.py",
    ]

    changes: list[tuple[Path, str, str]] = []

    for path in targets:
        if not path.exists():
            if path.name == "apply_mvp_005.py":
                print(f"= absent, ignoré : {path.relative_to(root)}")
                continue
            raise SystemExit(f"Fichier requis absent : {path}")

        before = path.read_text(encoding="utf-8")
        after = (
            patch_time(before)
            if path.name == "time.rs"
            else patch_generator(before)
        )

        if before != after:
            changes.append((path, before, after))
        else:
            print(f"= inchangé : {path.relative_to(root)}")

    if args.dry_run:
        for path, before, after in changes:
            relative = path.relative_to(root)
            print(
                "".join(
                    difflib.unified_diff(
                        before.splitlines(keepends=True),
                        after.splitlines(keepends=True),
                        fromfile=f"a/{relative}",
                        tofile=f"b/{relative}",
                    )
                ),
                end="",
            )
        print(f"\nDry-run terminé : {len(changes)} fichier(s) seraient modifiés.")
        return 0

    if changes:
        backup_root = (
            root
            / ".mvp005-backup"
            / datetime.now().strftime("%Y%m%d-%H%M%S")
        )

        for path, before, after in changes:
            relative = path.relative_to(root)
            backup = backup_root / relative
            backup.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, backup)
            path.write_text(after, encoding="utf-8")
            print(f"+ updated: {relative}")

        print(f"Backup directory: {backup_root}")
    else:
        print("Le correctif est déjà appliqué.")

    if not args.skip_checks:
        run(["cargo", "fmt", "--all"], cwd=root)
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
        )
        run(["cargo", "test", "--workspace"], cwd=root)
        run(["cargo", "build", "--release"], cwd=root)
    else:
        print(
            "\nChecks ignorés. Lance ensuite :\n"
            "  cargo fmt --all\n"
            "  cargo clippy --workspace --all-targets --all-features -- -D warnings\n"
            "  cargo test --workspace\n"
            "  cargo build --release"
        )

    print("\nCorrectif MVP-005 appliqué.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
