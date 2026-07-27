#!/usr/bin/env python3
"""Met à jour la roadmap Galactic dans Cadylo.

Le mode par défaut est un dry-run sans accès réseau.

Exemples :
    python3 update_cadylo_roadmap.py --dry-run
    python3 update_cadylo_roadmap.py --dry-run --emit-curl
    python3 update_cadylo_roadmap.py --apply
    python3 update_cadylo_roadmap.py --apply --create-missing
    python3 update_cadylo_roadmap.py --apply --create-missing \
        --create-field 'status="backlog"'

Par défaut, les créations reprennent les champs communs de l'issue ENG-25
(projet, statut, assignation, priorité et labels lorsque l'API les expose).
--create-common-json et --create-field permettent de les compléter ou de les
remplacer. Le script ajoute lui-même "title" et "description".
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import sys
import tempfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


DEFAULT_BASE_URL = "https://cadylo.app/api/v1"
DEFAULT_STATE_PATH = Path("backups/.cadylo-roadmap-state.json")
IDENTIFIER_RE = re.compile(r"^[A-Z][A-Z0-9]*-\d+$")
FIELD_NAME_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
CREATE_DIRECT_FIELDS = (
    "project_id",
    "workspace_id",
    "team_id",
    "status",
    "status_id",
    "assignee_id",
    "priority",
    "priority_id",
    "label_ids",
    "labels",
    "milestone_id",
    "cycle_id",
)


@dataclass(frozen=True)
class IssueChange:
    key: str
    title: str
    description: str
    identifier: str | None = None

    @property
    def is_new(self) -> bool:
        return self.identifier is None

    def payload(self) -> dict[str, str]:
        return {"title": self.title, "description": self.description}


def clean_markdown(value: str) -> str:
    return "\n".join(line.rstrip() for line in value.strip().splitlines())


def change(
    identifier: str,
    title: str,
    description: str,
) -> IssueChange:
    return IssueChange(
        key=identifier,
        identifier=identifier,
        title=title,
        description=clean_markdown(description),
    )


def creation(key: str, title: str, description: str) -> IssueChange:
    return IssueChange(
        key=key,
        title=title,
        description=clean_markdown(description),
    )


EXISTING_CHANGES: tuple[IssueChange, ...] = (
    change(
        "ENG-25",
        "MVP-017 — Ajouter une file générique de craft au chantier spatial",
        """
## Objectif

Introduire une file de fabrication générique au chantier spatial, alimentée par
des définitions de contenu configurables et réutilisable par les futurs
vaisseaux, sondes, transports, défenses et modules de soutien.

## Périmètre

- Identifiants `CraftableId` stables, sans enum Rust par objet fabriqué.
- Catalogue de craft chargé depuis le ruleset.
- Coûts, durées, prérequis, capacités et textes définis par les données.
- Réservation des ressources et validation des prérequis à la mise en file.
- File séquentielle avec progression uniquement sur les ticks stratégiques.
- Commandes et événements métier génériques.
- Sauvegarde, restauration et validation de la version du catalogue.
- Interface minimale du chantier et retours d'erreur explicites.

## Hors périmètre

- Composition des flottes.
- Déplacement et missions.
- Résolution des combats.

## Critères d'acceptation

- Un craftable utilisant un comportement déjà pris en charge peut être ajouté
  ou équilibré sans modifier le code Rust.
- Deux exécutions identiques produisent le même résultat de simulation.
- La file et sa progression survivent à une sauvegarde/reprise.
""",
    ),
    change(
        "ENG-26",
        "MVP-018 — Généraliser la propriété avec les factions",
        """
## Objectif

Généraliser la propriété des colonies et planètes avec des factions stables,
sans confondre connaissance, occupation et contrôle territorial.

## Périmètre

- `FactionId` stable et définitions de factions configurables.
- Propriétaire réel distinct des informations connues par le joueur.
- États minimaux de contrôle : neutre, hostile, contesté, sécurisé, colonisé.
- Compatibilité des colonies existantes avec la faction du joueur.
- Événements métier lors d'un changement de propriétaire ou de contrôle.
- Persistance et migration de l'état territorial.

## Hors périmètre

- Diplomatie avancée.
- Résolution des attaques.
- Modèle détaillé de population.

## Critères d'acceptation

- Plusieurs factions peuvent posséder ou occuper des objets du monde.
- Une planète peut être connue sans être contrôlée.
- L'état est déterministe et sauvegardé.
""",
    ),
    change(
        "ENG-27",
        "MVP-019 — Introduire les commandes génériques et relations dormantes",
        """
## Objectif

Poser les contrats génériques nécessaires aux futures interactions entre
factions, sans implémenter prématurément un système diplomatique complet.

## Périmètre

- Commandes et événements adressés par `FactionId`.
- Relations minimales configurables : inconnue, neutre, hostile, alliée.
- Valeur par défaut et évolution déterministes.
- API de simulation exploitable par les missions et le contrôle territorial.
- Persistance des relations.

## Hors périmètre

- Négociations, traités, réputation et IA diplomatique.
- Interface diplomatique dédiée.

## Critères d'acceptation

- Les systèmes futurs peuvent interroger une relation sans dépendre de la
  faction du joueur codée en dur.
- Les relations non encore actives n'altèrent pas la boucle de jeu existante.
""",
    ),
    change(
        "ENG-28",
        "MVP-020 — Définir les flottes, vaisseaux et capacités",
        """
## Objectif

Définir les vaisseaux et la composition des flottes sur un catalogue
configurable, sans encore déplacer ni engager les flottes.

## Périmètre

- `ShipId`, `FleetId` et capacités métier stables.
- Catalogue configurable : coût, temps de craft, vitesse, cargo, portée,
  capteurs, attaque, défense et capacités spéciales reconnues.
- Création et modification déterministes d'une flotte depuis des unités
  disponibles.
- Calculs agrégés de capacité, vitesse et cargo.
- Validation des compositions.
- Persistance des vaisseaux et flottes.

## Hors périmètre

- Trajets et consommation en mission.
- Combat.
- Simulateur de résultat.

## Critères d'acceptation

- Un vaisseau utilisant des capacités existantes peut être ajouté ou équilibré
  sans modifier Rust.
- Une flotte ne peut pas utiliser deux fois le même vaisseau.
- Les agrégats sont stables et couverts par des tests.
""",
    ),
    change(
        "ENG-29",
        "MVP-021 — Implémenter le moteur de trajet et la machine d'état des missions",
        """
## Objectif

Fournir un moteur générique de déplacement et une machine d'état de mission,
réutilisables par la reconnaissance, l'attaque, le transport et la colonisation.

## Périmètre

- Ordre de mission avec origine, cible, flotte, type et instant de départ.
- États : préparation, trajet aller, résolution, retour, terminée ou annulée.
- Durées calculées à partir du graphe galactique et de la flotte.
- Progression exclusivement sur les ticks stratégiques.
- Verrouillage des vaisseaux engagés.
- Événements métier aux transitions.
- Sauvegarde/reprise exacte d'une mission en cours.

## Hors périmètre

- Résolution propre à chaque type de mission.
- Combat et gains de ressources.

## Critères d'acceptation

- Une mission reprise depuis une sauvegarde termine au même tick.
- Une flotte engagée ne peut pas recevoir un ordre incompatible.
- Les transitions invalides sont refusées explicitement.
""",
    ),
    change(
        "ENG-30",
        "MVP-022 — Ajouter la sonde et la mission de reconnaissance",
        """
## Objectif

Permettre d'envoyer une sonde afin d'obtenir un renseignement progressif,
daté et potentiellement incomplet sur une planète cible.

## Périmètre

- Sonde définie dans le catalogue de craft.
- Mission de reconnaissance basée sur le moteur de trajet.
- Niveaux de connaissance distincts de l'état réel de la cible.
- Renseignement avec date d'observation, précision et fraîcheur.
- Révélation progressive : faction, ressources estimées, infrastructures,
  forces et défenses selon les capacités de la sonde.
- Rapport de reconnaissance persistant.

## Hors périmètre

- Analyse détaillée de colonisabilité.
- Détection active ou destruction des sondes.
- Simulateur de combat.

## Critères d'acceptation

- L'interface ne révèle jamais directement une donnée réelle non observée.
- Un renseignement ancien reste consultable avec sa date et son incertitude.
- La mission est déterministe et sauvegardable.
""",
    ),
    change(
        "ENG-32",
        "MVP-024 — Ajouter l'analyse planétaire et les règles de colonisabilité",
        """
## Objectif

Transformer les renseignements disponibles en analyse planétaire exploitable
et déterminer explicitement si une planète peut être colonisée.

## Périmètre

- Mission ou action d'analyse nécessitant la technologie adaptée.
- Caractéristiques configurables : habitabilité, environnement, ressources et
  contraintes d'installation.
- Résultat connu séparé des données réelles de la planète.
- Règles déterministes de colonisabilité avec motifs de refus.
- Rapport d'analyse daté et sauvegardé.
- Présentation claire des conditions remplies et manquantes.

## Hors périmètre

- Attaque et sécurisation.
- Création de la colonie.
- Extraction distante.

## Critères d'acceptation

- Une planète inconnue ou insuffisamment analysée ne peut pas être déclarée
  colonisable.
- Le moteur retourne les raisons précises d'un refus.
- Les règles d'équilibrage sont configurables lorsqu'elles utilisent des
  caractéristiques déjà reconnues.
""",
    ),
    change(
        "ENG-33",
        "MVP-025 — Ajouter les occupants, forces et défenses planétaires",
        """
## Objectif

Représenter la présence hostile ou neutre sur une planète afin de préparer
l'attaque sans figer encore la formule de combat.

## Périmètre

- Population ou faction occupante.
- Forces stationnées et défenses orbitales ou terrestres.
- Définitions configurables des unités et défenses utilisant des statistiques
  reconnues par la simulation.
- État réel distinct du dernier renseignement connu du joueur.
- Mise à jour et persistance déterministes des forces.
- Affichage estimatif fondé uniquement sur les renseignements disponibles.

## Hors périmètre

- Mission d'attaque.
- Résolution des combats.
- Prédiction de victoire.

## Critères d'acceptation

- Une cible peut posséder des forces inconnues ou partiellement estimées.
- Les données réelles ne fuient pas dans l'interface du joueur.
- Les forces et défenses survivent à une sauvegarde/reprise.
""",
    ),
    change(
        "ENG-34",
        "MVP-026 — Implémenter le vaisseau-colonie et la mission de colonisation",
        """
## Objectif

Permettre l'envoi d'un vaisseau-colonie vers une planète analysée et éligible,
sans transformer automatiquement une victoire militaire en colonie.

## Périmètre

- Vaisseau-colonie et chargement initial définis dans les catalogues.
- Mission de colonisation basée sur le moteur de trajet.
- Validation de l'habitabilité, des technologies et des ressources.
- Exigence d'une planète inhabitée ou préalablement sécurisée.
- Consommation du module de colonisation au succès.
- Événements métier préparant l'initialisation de la nouvelle colonie.

## Hors périmètre

- Combat pendant la colonisation.
- Interface complète de gestion multi-colonies.

## Critères d'acceptation

- Une planète hostile non sécurisée refuse la colonisation avec un motif clair.
- Les ressources et le vaisseau ne sont consommés qu'au moment défini par la
  règle de mission.
- La mission est déterministe et sauvegardable.
""",
    ),
    change(
        "ENG-35",
        "MVP-027 — Initialiser une nouvelle colonie jouable",
        """
## Objectif

Créer une colonie complète et immédiatement jouable à la réussite d'une mission
de colonisation.

## Périmètre

- Identité stable de la colonie et rattachement à la planète.
- Stocks, bâtiments, énergie et capacités initiales issus du scénario/ruleset.
- Transfert du chargement initial de la mission.
- Attribution à la faction du joueur et mise à jour du contrôle territorial.
- Événements de création et persistance.
- Compatibilité avec les systèmes de production, construction et recherche.

## Critères d'acceptation

- La nouvelle colonie fonctionne avec les mêmes règles qu'une colonie initiale.
- Aucune donnée initiale de contenu n'est dupliquée en dur dans le code Rust.
- Une sauvegarde/reprise conserve exactement son état.
""",
    ),
    change(
        "ENG-36",
        "MVP-028 — Ajouter la gestion multi-colonies",
        """
## Objectif

Permettre au joueur de gérer plusieurs colonies sans dupliquer les systèmes
économiques, de construction et de recherche.

## Périmètre

- Liste stable des colonies possédées par la faction du joueur.
- Sélection de la colonie active dans l'interface de gestion.
- Stocks, bâtiments, énergie et files de construction propres à chaque colonie.
- Recherche restant globale au joueur et alimentée par tous ses laboratoires.
- Validation des commandes avec un `ColonyId` explicite.
- Suppression des dépendances implicites à une colonie unique.
- Sauvegarde/reprise de la sélection et de toutes les colonies.

## Hors périmètre

- Transport automatique de ressources entre colonies.
- Gouverneurs, automatisation et spécialisation avancée.

## Critères d'acceptation

- Une action sur une colonie ne modifie pas silencieusement une autre colonie.
- Les productions scientifiques de toutes les colonies contribuent à la même
  recherche globale.
- La navigation entre colonies ne change pas leur simulation déterministe.
""",
    ),
    change(
        "ENG-37",
        "MVP-029 — Ajouter les missions de transport entre colonies",
        """
## Objectif

Permettre le transport de ressources entre colonies via une flotte et le moteur
de missions générique.

## Périmètre

- Ordre de transport avec origine, destination et cargaison.
- Validation du stock disponible et de la capacité cargo.
- Réservation ou retrait des ressources selon une règle explicite.
- Trajet aller, livraison et retour éventuel.
- Gestion déterministe d'une annulation ou d'une destination devenue invalide.
- Rapport de mission et persistance.

## Hors périmètre

- Extraction distante.
- Interception et combat en trajet.

## Critères d'acceptation

- Aucune duplication ou perte silencieuse de ressources n'est possible.
- Une reprise de sauvegarde conserve cargaison et phase de mission.
- Les erreurs de capacité ou de stock sont explicites.
""",
    ),
)


NEW_ISSUES: tuple[IssueChange, ...] = (
    creation(
        "MVP-016-B",
        "MVP-016-B — Externaliser le ruleset économique V1",
        """
## Objectif

Rendre configurable tout le contenu et l'équilibrage économique existants avant
d'ajouter le craft, les vaisseaux et les missions.

## Périmètre

- Ruleset externe versionné chargé au démarrage.
- Fichiers dédiés au manifeste, à l'économie, aux bâtiments, aux technologies et
  au scénario initial.
- Identifiants textuels stables pour les bâtiments, technologies, ressources et
  capacités.
- Noms, descriptions, coûts, durées, progressions, productions, stockages,
  énergie, prérequis, déblocages et limites de files configurables.
- Ressources, bâtiments et technologies de départ configurables.
- Conversion des durées en ticks au chargement.
- Validation des doublons, références absentes, cycles, valeurs invalides et
  effets inconnus.
- `ruleset_id`, version de schéma et empreinte structurelle dans les sauvegardes.
- Un changement de texte seul ne rend pas une sauvegarde incompatible.

## Contraintes

- Les algorithmes, commandes, événements et comportements fondamentaux restent
  implémentés en Rust.
- Pas de hot reload pendant une partie pour cette première version.
- Les textes français peuvent rester dans les catalogues.

## Critères d'acceptation

- Les coûts, durées, textes et progressions existants se modifient sans toucher
  au code Rust.
- Un bâtiment ou une technologie utilisant des effets déjà connus peut être
  ajouté par configuration.
- Un ruleset invalide est refusé avec des erreurs précises.
- La simulation reste déterministe.
""",
    ),
    creation(
        "MVP-025-B",
        "MVP-025-B — Ajouter les attaques, le combat V1 et les rapports",
        """
## Objectif

Introduire une première boucle d'attaque déterministe entre une flotte et les
forces d'une planète, avec un rapport exploitable par la sécurisation future.

## Périmètre

- Mission d'attaque basée sur le moteur de trajet.
- Instantané explicite de la flotte attaquante et de la défense réelle.
- Fonction pure `resolve_combat` paramétrée par des règles de combat versionnées.
- Résultat minimal : vainqueur, pertes, survivants, ressources récupérables,
  dommages et évolution du contrôle territorial.
- Application atomique du résultat à l'état de simulation.
- Rapport de combat persistant et consultable.
- Couverture des cas limites : égalité, destruction mutuelle, cible devenue
  invalide et reprise de sauvegarde.

## Hors périmètre

- Simulateur précombat.
- Combat tactique contrôlé directement.
- Diplomatie avancée et interception en trajet.

## Critères d'acceptation

- Une même entrée et une même graine produisent exactement le même rapport.
- L'interface du joueur ne reçoit pas d'informations défensives non observées
  avant le combat.
- Les pertes et gains ne peuvent pas être appliqués deux fois.
- Une planète n'est colonisable après attaque que si les règles la déclarent
  sécurisée.
""",
    ),
    creation(
        "MVP-029-B",
        "MVP-029-B — Ajouter les sites d'extraction et la récolte distante",
        """
## Objectif

Réutiliser les flottes, le cargo et les missions de transport pour exploiter des
sites distants après stabilisation de la boucle militaire et multi-colonies.

## Périmètre

- Sites d'extraction configurables découverts par exploration ou analyse.
- Conditions d'accès, rendement, capacité et éventuel épuisement.
- Mission de récolte avec trajet, chargement, retour et livraison.
- Limitation par le cargo, le temps et les capacités de la flotte.
- Réservation d'un site pendant une opération si nécessaire.
- Rapport de récolte et persistance.

## Hors périmètre

- Combat automatique pour le contrôle du site.
- Marché ou commerce.

## Critères d'acceptation

- Une mission ne crée jamais plus de ressources que le site n'en fournit.
- Une reprise de sauvegarde conserve le site, la cargaison et la phase.
- Les valeurs d'équilibrage sont modifiables dans le ruleset.
""",
    ),
)


class CadyloError(RuntimeError):
    """Erreur réseau ou réponse Cadylo invalide."""


class CadyloClient:
    def __init__(self, base_url: str, token: str, timeout: float) -> None:
        self.base_url = base_url.rstrip("/")
        self.token = token
        self.timeout = timeout

    def request(
        self,
        method: str,
        path: str,
        payload: Mapping[str, Any] | None = None,
        *,
        idempotency_key: str | None = None,
    ) -> Any:
        url = f"{self.base_url}/{path.lstrip('/')}"
        body = None
        headers = {
            "Accept": "application/json",
            "Authorization": f"Bearer {self.token}",
        }
        if payload is not None:
            body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
            headers["Content-Type"] = "application/json"
        if idempotency_key is not None:
            headers["Idempotency-Key"] = idempotency_key

        request = Request(url, data=body, headers=headers, method=method)
        try:
            with urlopen(request, timeout=self.timeout) as response:
                response_body = response.read()
        except HTTPError as exc:
            error_body = exc.read().decode("utf-8", errors="replace")
            raise CadyloError(
                f"{method} {url} a échoué avec HTTP {exc.code}: {error_body}"
            ) from exc
        except URLError as exc:
            raise CadyloError(f"{method} {url} a échoué: {exc.reason}") from exc

        if not response_body:
            return None
        try:
            return json.loads(response_body)
        except json.JSONDecodeError as exc:
            preview = response_body.decode("utf-8", errors="replace")[:500]
            raise CadyloError(
                f"{method} {url} a renvoyé un JSON invalide: {preview}"
            ) from exc

    def get_issue(self, identifier: str) -> Any:
        return self.request("GET", f"issues/{identifier}")

    def patch_issue(self, identifier: str, payload: Mapping[str, Any]) -> Any:
        return self.request("PATCH", f"issues/{identifier}", payload)

    def create_issue(
        self,
        payload: Mapping[str, Any],
        *,
        roadmap_key: str,
    ) -> Any:
        return self.request(
            "POST",
            "issues",
            payload,
            idempotency_key=f"galactic-roadmap-{roadmap_key.lower()}-v1",
        )


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Met à jour la roadmap Galactic dans Cadylo.",
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--dry-run",
        action="store_true",
        help="Affiche les opérations sans accès réseau (mode par défaut).",
    )
    mode.add_argument(
        "--apply",
        action="store_true",
        help="Exécute réellement les requêtes GET, PATCH et éventuellement POST.",
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path.cwd(),
        help="Racine où écrire sauvegardes et état (défaut : dossier courant).",
    )
    parser.add_argument(
        "--base-url",
        default=os.environ.get("CADYLO_BASE_URL", DEFAULT_BASE_URL),
        help=f"URL de l'API (défaut : {DEFAULT_BASE_URL}).",
    )
    parser.add_argument(
        "--token-env",
        default="CADYLO_TOKEN",
        help="Nom de la variable contenant le jeton (défaut : CADYLO_TOKEN).",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=20.0,
        help="Timeout HTTP en secondes (défaut : 20).",
    )
    parser.add_argument(
        "--create-missing",
        action="store_true",
        help="Autorise le POST des trois nouvelles issues.",
    )
    parser.add_argument(
        "--create-common-json",
        type=Path,
        help=(
            "JSON facultatif des champs communs de création; ses valeurs "
            "remplacent celles copiées depuis --create-from."
        ),
    )
    parser.add_argument(
        "--create-from",
        default="ENG-25",
        metavar="ENG-NN",
        help=(
            "Issue servant de modèle aux créations (défaut : ENG-25). "
            "Seuls les champs communs sûrs sont repris."
        ),
    )
    parser.add_argument(
        "--no-create-from",
        action="store_const",
        const=None,
        dest="create_from",
        help=(
            "Ne copie aucune issue modèle; utilisez alors --create-field "
            "ou --create-common-json."
        ),
    )
    parser.add_argument(
        "--create-field",
        action="append",
        default=[],
        metavar="NOM=VALEUR_JSON",
        help=(
            "Ajoute ou remplace un champ commun de création; option répétable. "
            "Exemples: --create-field 'status=\"backlog\"' ou "
            "--create-field 'label_ids=[\"uuid\"]'."
        ),
    )
    parser.add_argument(
        "--new-id",
        action="append",
        default=[],
        metavar="MVP-XXX=ENG-NN",
        help=(
            "Associe une nouvelle issue déjà créée à son identifiant Cadylo; "
            "option répétable."
        ),
    )
    parser.add_argument(
        "--state",
        type=Path,
        default=DEFAULT_STATE_PATH,
        help=(
            "Fichier d'état relatif à --root, ou chemin absolu "
            f"(défaut : {DEFAULT_STATE_PATH})."
        ),
    )
    parser.add_argument(
        "--only",
        nargs="+",
        help="Limite aux identifiants ENG-NN ou clés MVP-XXX indiqués.",
    )
    parser.add_argument(
        "--emit-curl",
        action="store_true",
        help="Affiche les commandes curl équivalentes pendant le dry-run.",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help=(
            "Continue un PATCH si la lecture/sauvegarde distante échoue. "
            "Usage déconseillé."
        ),
    )
    return parser.parse_args(argv)


def resolve_under_root(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def load_json_object(path: Path, *, allow_missing: bool = False) -> dict[str, Any]:
    if allow_missing and not path.exists():
        return {}
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise CadyloError(f"Fichier introuvable: {path}") from exc
    except json.JSONDecodeError as exc:
        raise CadyloError(f"JSON invalide dans {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise CadyloError(f"{path} doit contenir un objet JSON.")
    return value


def atomic_write_json(path: Path, value: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        dir=path.parent,
        prefix=f".{path.name}.",
        suffix=".tmp",
        delete=False,
    ) as handle:
        json.dump(value, handle, ensure_ascii=False, indent=2, sort_keys=True)
        handle.write("\n")
        temporary_path = Path(handle.name)
    temporary_path.replace(path)


def parse_new_ids(values: Iterable[str]) -> dict[str, str]:
    result: dict[str, str] = {}
    known_keys = {issue.key for issue in NEW_ISSUES}
    for value in values:
        key, separator, identifier = value.partition("=")
        if not separator or key not in known_keys or not IDENTIFIER_RE.fullmatch(identifier):
            expected = ", ".join(sorted(known_keys))
            raise CadyloError(
                f"--new-id invalide: {value!r}. Format attendu "
                f"MVP-XXX=ENG-NN; clés autorisées: {expected}."
            )
        if key in result and result[key] != identifier:
            raise CadyloError(f"Deux identifiants différents sont fournis pour {key}.")
        result[key] = identifier
    return result


def parse_create_fields(values: Iterable[str]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for value in values:
        name, separator, raw = value.partition("=")
        if not separator or not FIELD_NAME_RE.fullmatch(name):
            raise CadyloError(
                f"--create-field invalide: {value!r}. Format attendu "
                "NOM=VALEUR_JSON."
            )
        if name in {"title", "description"}:
            raise CadyloError(
                f"--create-field ne peut pas redéfinir {name!r}."
            )
        try:
            parsed = json.loads(raw)
        except json.JSONDecodeError:
            parsed = raw
        result[name] = parsed
    return result


def load_state(path: Path) -> dict[str, Any]:
    state = load_json_object(path, allow_missing=True)
    if not state:
        return {"version": 1, "created": {}}
    if state.get("version") != 1 or not isinstance(state.get("created"), dict):
        raise CadyloError(f"Format d'état non reconnu dans {path}.")
    return state


def state_identifiers(state: Mapping[str, Any]) -> dict[str, str]:
    result: dict[str, str] = {}
    created = state.get("created", {})
    if not isinstance(created, dict):
        return result
    for key, record in created.items():
        if isinstance(record, dict):
            identifier = record.get("identifier")
            if isinstance(identifier, str) and IDENTIFIER_RE.fullmatch(identifier):
                result[key] = identifier
    return result


def extract_identifier(value: Any) -> str | None:
    if isinstance(value, dict):
        for key in ("identifier", "issue_identifier", "key"):
            candidate = value.get(key)
            if isinstance(candidate, str) and IDENTIFIER_RE.fullmatch(candidate):
                return candidate
        for nested in value.values():
            candidate = extract_identifier(nested)
            if candidate is not None:
                return candidate
    elif isinstance(value, list):
        for nested in value:
            candidate = extract_identifier(nested)
            if candidate is not None:
                return candidate
    return None


def extract_issue_fields(value: Any) -> Mapping[str, Any] | None:
    if isinstance(value, dict):
        if "title" in value or "description" in value:
            return value
        for key in ("data", "issue", "result"):
            nested = value.get(key)
            found = extract_issue_fields(nested)
            if found is not None:
                return found
    return None


def nested_identifier(
    issue: Mapping[str, Any],
    field: str,
) -> str | None:
    value = issue.get(field)
    if not isinstance(value, Mapping):
        return None
    for key in ("id", "uuid"):
        candidate = value.get(key)
        if isinstance(candidate, str) and candidate:
            return candidate
    return None


def nested_value(
    issue: Mapping[str, Any],
    field: str,
) -> str | int | None:
    value = issue.get(field)
    if not isinstance(value, Mapping):
        return None
    for key in ("slug", "key", "value", "name", "id"):
        candidate = value.get(key)
        if isinstance(candidate, (str, int)) and not isinstance(candidate, bool):
            return candidate
    return None


def create_common_from_template(response: Any) -> dict[str, Any]:
    issue = extract_issue_fields(response)
    if issue is None:
        raise CadyloError(
            "La réponse de l'issue modèle ne contient pas d'objet issue "
            "reconnaissable."
        )

    common: dict[str, Any] = {}
    for field in CREATE_DIRECT_FIELDS:
        value = issue.get(field)
        if field == "labels" and isinstance(value, list):
            if all(
                isinstance(item, (str, int)) and not isinstance(item, bool)
                for item in value
            ):
                common[field] = value
        elif value is not None and not isinstance(value, Mapping):
            common[field] = value

    for source, target in (
        ("project", "project_id"),
        ("workspace", "workspace_id"),
        ("team", "team_id"),
        ("assignee", "assignee_id"),
        ("milestone", "milestone_id"),
        ("cycle", "cycle_id"),
    ):
        if target not in common:
            identifier = nested_identifier(issue, source)
            if identifier is not None:
                common[target] = identifier

    for source in ("status", "priority"):
        if source not in common and f"{source}_id" not in common:
            value = nested_value(issue, source)
            if value is not None:
                common[source] = value

    if "label_ids" not in common and "labels" not in common:
        labels = issue.get("labels")
        if isinstance(labels, list):
            label_ids = [
                item.get("id")
                for item in labels
                if isinstance(item, Mapping) and isinstance(item.get("id"), str)
            ]
            if label_ids and len(label_ids) == len(labels):
                common["label_ids"] = label_ids

    if not common:
        raise CadyloError(
            "Aucun champ commun de création n'a pu être extrait de l'issue "
            "modèle. Utilisez --create-field ou --create-common-json."
        )
    return common


def select_changes(
    only: Sequence[str] | None,
) -> tuple[tuple[IssueChange, ...], tuple[IssueChange, ...]]:
    if not only:
        return EXISTING_CHANGES, NEW_ISSUES
    requested = set(only)
    available = {issue.key for issue in EXISTING_CHANGES + NEW_ISSUES}
    unknown = sorted(requested - available)
    if unknown:
        raise CadyloError(
            "Cible(s) inconnue(s) pour --only: "
            f"{', '.join(unknown)}. Valeurs autorisées: "
            f"{', '.join(sorted(available))}."
        )
    return (
        tuple(issue for issue in EXISTING_CHANGES if issue.key in requested),
        tuple(issue for issue in NEW_ISSUES if issue.key in requested),
    )


def create_payload(
    issue: IssueChange,
    common: Mapping[str, Any],
) -> dict[str, Any]:
    forbidden = {"title", "description"} & common.keys()
    if forbidden:
        raise CadyloError(
            "Les champs communs de création ne doivent pas définir: "
            f"{', '.join(sorted(forbidden))}."
        )
    return {**common, **issue.payload()}


def render_curl(
    method: str,
    url: str,
    payload: Mapping[str, Any],
    *,
    idempotency_key: str | None = None,
) -> str:
    lines = [
        f"curl --request {method} \\",
        f'  --url "{url}" \\',
        '  --header "Authorization: Bearer $CADYLO_TOKEN" \\',
        '  --header "Content-Type: application/json" \\',
    ]
    if idempotency_key is not None:
        lines.append(f'  --header "Idempotency-Key: {idempotency_key}" \\')
    body = json.dumps(payload, ensure_ascii=False, indent=2)
    lines.append(f"  --data {shlex.quote(body)}")
    return "\n".join(lines)


def print_dry_run(
    base_url: str,
    existing: Sequence[IssueChange],
    new: Sequence[IssueChange],
    resolved_new_ids: Mapping[str, str],
    create_common: Mapping[str, Any],
    *,
    emit_curl: bool,
    create_missing: bool,
    create_from: str | None,
) -> None:
    print("DRY-RUN — aucune requête réseau ne sera exécutée.")
    print(f"\nPATCH prévus : {len(existing)}")
    for issue in existing:
        print(f"- {issue.identifier}: {issue.title}")
        if emit_curl:
            print()
            print(
                render_curl(
                    "PATCH",
                    f"{base_url.rstrip('/')}/issues/{issue.identifier}",
                    issue.payload(),
                )
            )
            print()

    mapped_new = [issue for issue in new if issue.key in resolved_new_ids]
    for issue in mapped_new:
        identifier = resolved_new_ids[issue.key]
        print(f"- {identifier} ({issue.key}): {issue.title}")
        if emit_curl:
            print()
            print(
                render_curl(
                    "PATCH",
                    f"{base_url.rstrip('/')}/issues/{identifier}",
                    issue.payload(),
                )
            )
            print()

    missing = [issue for issue in new if issue.key not in resolved_new_ids]
    print(f"\nPOST nécessaires : {len(missing)}")
    if missing and create_missing and create_from is not None:
        print(
            f"  Champs communs copiés depuis {create_from} lors de --apply; "
            "les options explicites restent prioritaires."
        )
    for issue in missing:
        marker = "sera créé" if create_missing else "non exécuté sans --create-missing"
        print(f"- {issue.key}: {issue.title} [{marker}]")
        if emit_curl:
            print()
            payload = create_payload(issue, create_common)
            if create_from is not None:
                print(
                    f"# Ajouter au JSON ci-dessous les champs communs de "
                    f"{create_from}; --apply les récupère automatiquement."
                )
            print(
                render_curl(
                    "POST",
                    f"{base_url.rstrip('/')}/issues",
                    payload,
                    idempotency_key=(
                        f"galactic-roadmap-{issue.key.lower()}-v1"
                    ),
                )
            )
            print()

    untouched = (
        "ENG-31, ENG-48, ENG-49, ENG-38, ENG-50 et ENG-39 à ENG-46"
    )
    print(f"\nIssues volontairement inchangées : {untouched}.")


def backup_remote_issues(
    client: CadyloClient,
    identifiers: Sequence[str],
    backup_dir: Path,
    *,
    force: bool,
) -> dict[str, Any]:
    snapshots: dict[str, Any] = {}
    failures: list[str] = []
    for identifier in dict.fromkeys(identifiers):
        try:
            snapshots[identifier] = client.get_issue(identifier)
        except CadyloError as exc:
            failures.append(str(exc))
            if not force:
                raise CadyloError(
                    "Lecture distante impossible; aucune modification effectuée. "
                    "Corrigez l'accès ou relancez avec --force (déconseillé).\n"
                    f"{exc}"
                ) from exc

    backup_dir.mkdir(parents=True, exist_ok=False)
    atomic_write_json(
        backup_dir / "issues_before.json",
        {
            "created_at": datetime.now(timezone.utc).isoformat(),
            "issues": snapshots,
            "read_failures": failures,
        },
    )
    return snapshots


def payload_diff(
    current_response: Any,
    desired: Mapping[str, Any],
) -> dict[str, Any]:
    current = extract_issue_fields(current_response)
    if current is None:
        return dict(desired)
    return {key: value for key, value in desired.items() if current.get(key) != value}


def apply_changes(
    args: argparse.Namespace,
    existing: Sequence[IssueChange],
    new: Sequence[IssueChange],
    state_path: Path,
    state: dict[str, Any],
    resolved_new_ids: dict[str, str],
    explicit_create_common: Mapping[str, Any],
) -> int:
    token = os.environ.get(args.token_env)
    if not token:
        raise CadyloError(
            f"La variable {args.token_env} est absente ou vide. "
            "Aucun appel réseau n'a été effectué."
        )
    client = CadyloClient(args.base_url, token, args.timeout)

    mapped_changes = [
        (issue.identifier or "", issue) for issue in existing
    ] + [
        (resolved_new_ids[issue.key], issue)
        for issue in new
        if issue.key in resolved_new_ids
    ]

    missing = [issue for issue in new if issue.key not in resolved_new_ids]
    backup_identifiers = [identifier for identifier, _issue in mapped_changes]
    if (
        missing
        and args.create_missing
        and args.create_from is not None
        and args.create_from not in backup_identifiers
    ):
        backup_identifiers.append(args.create_from)

    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    backup_dir = args.root / "backups" / ".cadylo-backup" / timestamp
    snapshots = backup_remote_issues(
        client,
        backup_identifiers,
        backup_dir,
        force=args.force,
    )
    print(f"Sauvegarde distante créée : {backup_dir}")

    create_common = dict(explicit_create_common)
    if missing and args.create_missing and args.create_from is not None:
        template_response = snapshots.get(args.create_from)
        if template_response is None:
            if not create_common:
                raise CadyloError(
                    f"L'issue modèle {args.create_from} n'a pas pu être lue "
                    "et aucun champ de création explicite n'a été fourni."
                )
            print(
                f"AVERTISSEMENT: modèle {args.create_from} indisponible; "
                "utilisation des champs explicites uniquement."
            )
        else:
            template_common = create_common_from_template(template_response)
            create_common = {**template_common, **create_common}
            print(
                f"CREATE modèle {args.create_from}: "
                f"{', '.join(sorted(template_common))}"
            )

    created_state = state.setdefault("created", {})
    if not isinstance(created_state, dict):
        raise CadyloError("Le champ created du fichier d'état est invalide.")
    state_changed = False
    for issue in new:
        identifier = resolved_new_ids.get(issue.key)
        if identifier is None:
            continue
        previous = created_state.get(issue.key)
        record = dict(previous) if isinstance(previous, dict) else {}
        if record.get("identifier") != identifier:
            record["identifier"] = identifier
            record["recorded_at"] = datetime.now(timezone.utc).isoformat()
            created_state[issue.key] = record
            state_changed = True
    if state_changed:
        atomic_write_json(state_path, state)

    patch_count = 0
    skip_count = 0
    for identifier, issue in mapped_changes:
        desired = issue.payload()
        patch = payload_diff(snapshots.get(identifier), desired)
        if not patch:
            print(f"SKIP  {identifier}: déjà à jour")
            skip_count += 1
            continue
        client.patch_issue(identifier, patch)
        print(f"PATCH {identifier}: {', '.join(sorted(patch))}")
        patch_count += 1

    create_count = 0
    if missing and not args.create_missing:
        print(
            "SKIP  créations: utilisez --create-missing après avoir vérifié "
            "le dry-run."
        )
    elif missing:
        if not create_common:
            raise CadyloError(
                "Aucun champ commun de création disponible. Utilisez "
                "--create-from, --create-field ou --create-common-json."
            )
        for issue in missing:
            payload = create_payload(issue, create_common)
            response = client.create_issue(payload, roadmap_key=issue.key)
            identifier = extract_identifier(response)
            record = {
                "identifier": identifier,
                "created_at": datetime.now(timezone.utc).isoformat(),
                "response": response,
            }
            created_state[issue.key] = record
            atomic_write_json(state_path, state)
            if identifier is None:
                raise CadyloError(
                    f"{issue.key} semble créée, mais aucun identifiant ENG-NN "
                    f"n'a été trouvé dans la réponse. Réponse conservée dans "
                    f"{state_path}; relancez avec --new-id {issue.key}=ENG-NN."
                )
            print(f"POST  {issue.key} -> {identifier}")
            create_count += 1

    print(
        f"Terminé : {patch_count} PATCH, {create_count} POST, "
        f"{skip_count} déjà à jour."
    )
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    args.root = args.root.expanduser().resolve()
    if not args.root.is_dir():
        raise CadyloError(f"--root n'est pas un dossier: {args.root}")
    if args.timeout <= 0:
        raise CadyloError("--timeout doit être strictement positif.")
    if args.create_from is not None and not IDENTIFIER_RE.fullmatch(
        args.create_from
    ):
        raise CadyloError(
            f"--create-from invalide: {args.create_from!r}. "
            "Format attendu: ENG-NN."
        )

    state_path = resolve_under_root(args.root, args.state.expanduser())
    state = load_state(state_path)
    explicit_new_ids = parse_new_ids(args.new_id)
    resolved_new_ids = {**state_identifiers(state), **explicit_new_ids}
    existing, new = select_changes(args.only)

    create_common: dict[str, Any] = {}
    if args.create_common_json is not None:
        common_path = resolve_under_root(
            args.root,
            args.create_common_json.expanduser(),
        )
        create_common = load_json_object(common_path)
        forbidden = {"title", "description"} & create_common.keys()
        if forbidden:
            raise CadyloError(
                f"{common_path} ne doit pas définir: "
                f"{', '.join(sorted(forbidden))}."
            )
    create_common.update(parse_create_fields(args.create_field))

    unresolved_new = [
        issue for issue in new if issue.key not in resolved_new_ids
    ]
    if (
        args.apply
        and args.create_missing
        and unresolved_new
        and args.create_from is None
        and not create_common
    ):
        raise CadyloError(
            "--create-missing exige une issue modèle (--create-from, actif "
            "par défaut) ou des champs explicites via --create-field ou "
            "--create-common-json. Aucun appel réseau n'a été effectué."
        )

    if not args.apply:
        print_dry_run(
            args.base_url,
            existing,
            new,
            resolved_new_ids,
            create_common,
            emit_curl=args.emit_curl,
            create_missing=args.create_missing,
            create_from=args.create_from,
        )
        return 0

    return apply_changes(
        args,
        existing,
        new,
        state_path,
        state,
        resolved_new_ids,
        create_common,
    )


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except CadyloError as exc:
        print(f"ERREUR: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc
