#!/usr/bin/env python3
"""Create Galactic MVP issues in Cadylo in their explicit backlog order.

Examples:
    export CADYLO_TOKEN='...'
    python tools/create_galactic_mvp_cadylo_issues.py --dry-run
    python tools/create_galactic_mvp_cadylo_issues.py --from MVP-010-B
    python tools/create_galactic_mvp_cadylo_issues.py \
        --only MVP-010-B --only MVP-023-B --only MVP-023-C --only MVP-030-B
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

API_URL = "https://cadylo.app/api/v1/issues"
HERE = Path(__file__).resolve().parent
RESULTS_FILE = HERE / "cadylo_creation_results.jsonl"
KEY_RE = re.compile(r"^MVP-(\d{1,3})(?:-([A-Z]))?$", re.IGNORECASE)


def find_issues_file() -> Path:
    candidates = [
        HERE / "galactic_mvp_cadylo_issues.json",
        HERE.parent / "docs" / "galactic_mvp_cadylo_issues.json",
        HERE / "docs" / "galactic_mvp_cadylo_issues.json",
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate
    raise SystemExit(
        "galactic_mvp_cadylo_issues.json introuvable près du script."
    )


def issue_key(issue: dict) -> str:
    if issue.get("key"):
        return str(issue["key"]).upper()
    suffix = str(issue.get("suffix", "")).strip().upper()
    key = f"MVP-{int(issue['n']):03d}"
    return key + (f"-{suffix}" if suffix else "")


def key_order(value: str) -> int:
    normalized = value.strip().upper()
    if normalized.isdigit():
        normalized = f"MVP-{int(normalized):03d}"
    elif re.fullmatch(r"\d{1,3}-[A-Z]", normalized):
        number, suffix = normalized.split("-", 1)
        normalized = f"MVP-{int(number):03d}-{suffix}"

    match = KEY_RE.fullmatch(normalized)
    if not match:
        raise argparse.ArgumentTypeError(
            f"Identifiant invalide : {value!r}. Exemple : MVP-010-B"
        )

    number = int(match.group(1))
    suffix = match.group(2)
    offset = 0 if suffix is None else ord(suffix.upper()) - ord("A") + 1
    return number * 100 + offset * 10


def issue_order(issue: dict) -> int:
    if "order" in issue:
        return int(issue["order"])
    return key_order(issue_key(issue))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--from",
        dest="start",
        default="MVP-001",
        help="Première issue à créer, ex. MVP-010-B ou 10-B.",
    )
    parser.add_argument(
        "--only",
        action="append",
        default=[],
        metavar="KEY",
        help="Créer uniquement cette issue. Répétable.",
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="Afficher l'ordre des issues puis quitter.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Afficher les payloads sans appeler l'API.",
    )
    parser.add_argument(
        "--delay",
        type=float,
        default=0.25,
        help="Délai entre les appels API.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    issues_file = find_issues_file()
    issues = json.loads(issues_file.read_text(encoding="utf-8"))
    issues.sort(key=lambda issue: (issue_order(issue), issue_key(issue)))

    if args.list:
        for issue in issues:
            print(f"{issue_key(issue):10} {issue['title']}")
        return 0

    if args.only:
        selected = {key_order(key) for key in args.only}
        issues = [issue for issue in issues if issue_order(issue) in selected]
        found = {issue_order(issue) for issue in issues}
        missing = selected - found
        if missing:
            print(
                "Une ou plusieurs issues demandées sont absentes du JSON.",
                file=sys.stderr,
            )
            return 2
    else:
        start_order = key_order(args.start)
        issues = [issue for issue in issues if issue_order(issue) >= start_order]

    token = os.environ.get("CADYLO_TOKEN")
    if not args.dry_run and not token:
        print("Missing CADYLO_TOKEN environment variable.", file=sys.stderr)
        return 2

    for issue in issues:
        key = issue_key(issue)
        payload = {
            "team_key": issue["team_key"],
            "title": issue["title"],
            "description": issue["description"],
            "priority": issue["priority"],
            "estimate": issue["estimate"],
            "project_id": issue["project_id"],
            "assignee_id": issue["assignee_id"],
        }

        if args.dry_run:
            print(f"# {key}")
            print(json.dumps(payload, ensure_ascii=False, indent=2))
            continue

        data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        request = urllib.request.Request(
            API_URL,
            data=data,
            method="POST",
            headers={
                "Authorization": f"Bearer {token}",
                "Content-Type": "application/json",
                "Accept": "application/json",
                "User-Agent": "Galactic-MVP-Backlog-Creator/2.0",
            },
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                body = response.read().decode("utf-8", errors="replace")
                record = {
                    "key": key,
                    "n": issue["n"],
                    "title": issue["title"],
                    "http_status": response.status,
                    "response": json.loads(body) if body else None,
                }
                with RESULTS_FILE.open("a", encoding="utf-8") as handle:
                    handle.write(json.dumps(record, ensure_ascii=False) + "\n")
                print(f"[OK] {key} — {issue['title']} ({response.status})")
        except urllib.error.HTTPError as exc:
            body = exc.read().decode("utf-8", errors="replace")
            print(
                f"[ERROR] {key} -> HTTP {exc.code}: {body}",
                file=sys.stderr,
            )
            print(
                f"Resume with: python3 {Path(__file__).name} --from {key}",
                file=sys.stderr,
            )
            return 1
        except (urllib.error.URLError, TimeoutError) as exc:
            print(f"[ERROR] {key} -> {exc}", file=sys.stderr)
            print(
                f"Resume with: python3 {Path(__file__).name} --from {key}",
                file=sys.stderr,
            )
            return 1

        time.sleep(max(0.0, args.delay))

    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BrokenPipeError:
        raise SystemExit(0)
