#!/usr/bin/env python3
"""
Corrige l'appel rustfmt de tools/apply_mvp_009.py.

Le dry-run MVP-009 formate chaque source séparément. Sans skip_children,
rustfmt essaie de charger knowledge.rs en formatant lib.rs avant que ce
nouveau module n'existe sur disque.

Usage :
    python tools/fix_apply_mvp_009_rustfmt.py
"""

from __future__ import annotations

import shutil
from datetime import datetime
from pathlib import Path


OLD = """            [
                rustfmt,
                "--edition",
                cargo_edition(root),
                str(temporary),
            ],
"""

NEW = """            [
                rustfmt,
                "--edition",
                cargo_edition(root),
                "--config",
                "skip_children=true",
                str(temporary),
            ],
"""


def find_root(start: Path) -> Path:
    for candidate in [start, *start.parents]:
        if (
            (candidate / ".git").exists()
            and (candidate / "Cargo.toml").exists()
            and (candidate / "tools/apply_mvp_009.py").exists()
        ):
            return candidate
    raise SystemExit("Racine Galactic introuvable.")


def main() -> int:
    root = find_root(Path.cwd().resolve())
    target = root / "tools/apply_mvp_009.py"
    source = target.read_text(encoding="utf-8")

    if "skip_children=true" in source:
        print("Le correctif rustfmt est déjà appliqué.")
        return 0

    if OLD not in source:
        raise SystemExit(
            "Le bloc rustfmt attendu n'a pas été trouvé. "
            "Remplace le script par la version corrigée complète."
        )

    backup_root = (
        root
        / ".mvp009-backup"
        / datetime.now().strftime("%Y%m%d-%H%M%S")
    )
    backup = backup_root / "tools/apply_mvp_009.py"
    backup.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(target, backup)

    source = source.replace(OLD, NEW, 1)
    compile(source, str(target), "exec")
    target.write_text(source, encoding="utf-8")

    print(f"+ updated: {target.relative_to(root)}")
    print(f"Backup directory: {backup_root}")
    print(
        "\nRelance maintenant :\n"
        "  python tools/apply_mvp_009.py --dry-run"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
