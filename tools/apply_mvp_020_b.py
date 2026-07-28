#!/usr/bin/env python3
"""Apply Galactic MVP-020-B safely from the exact pushed baseline.

This editorial migration introduces a universe bible, coherent display names,
canonical system and planet names, and an explicit generation fingerprint.
Dry-run performs no Cargo build unless --checks is explicitly requested.
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


MIGRATION = "MVP-020-B"
BASELINE_SHA = "885d51ba41b03dd780ad038eb7a65aeb14b698cd"

MODIFIED_BLOBS = {
    "README.md": "ca214e6355e1297a4c7e3e62fe60f98333fc45ca",
    "assets/rulesets/default/buildings.ron": "ed5adf2bdb5ab5d23a915e1ca8d68570230410a6",
    "assets/rulesets/default/craftables.ron": "53bb514312943a38b62290701fe6cb855f7cc6d8",
    "assets/rulesets/default/factions.ron": "3fe6627b8b48abc9ea9cc7a529aceb5fcd1079a2",
    "assets/rulesets/default/manifest.ron": "e5570898b4f8e4ad6df28b4ec91101078f56990a",
    "assets/rulesets/default/starting_scenario.ron": "9cb0521fa67a433cc8275442b82bf96bf1fbae75",
    "assets/rulesets/default/technologies.ron": "10a52d46637a490a0ffa81cca0cfce3fcb4ac37f",
    "crates/galactic_client/src/craft_ui.rs": "ca1343884f25b301e0e10e1002f1172dd031889f",
    "crates/galactic_client/src/lib.rs": "f88b5010f3d820d8aab78d08e5628c1cbfe719e0",
    "crates/galactic_client/src/research_ui.rs": "edb10bc3eb4214d11ce1972227a6b34b61dbda8d",
    "crates/galactic_domain/src/world.rs": "85b464eddf6366579200ac3d01790ae61c44e848",
    "crates/galactic_sim/src/building_catalog.rs": "83a64ab3756238300d64811456310961a602d1c7",
    "crates/galactic_sim/src/state.rs": "f8d692f6f460f72e578718522cc2828a71185c14",
    "docs/galactic_mvp_cadrage_et_backlog.md": "860af9e25ec444fd5ecad1d20ec3f10b79b4e339",
    "docs/mvp_architecture.md": "6dd0c3eac13de8a91f14bedb1918f8cb5155ef5b",
    "docs/ruleset.md": "f07aa0228f2c2a0d94975e791eb684d3b941dd2d",
}

DEPENDENCY_BLOBS = {
    "tools/apply_mvp_016_b.py": "1557ff3f419abbf6a1b58b897100aa72da80bd38",
}

CREATED_PATHS = ("docs/universe_bible.md",)
EXPECTED_PATHS = frozenset((*MODIFIED_BLOBS, *CREATED_PATHS))

TARGETED_CHECK_COMMANDS = (
    ("cargo", "fmt", "--all"),
    (
        "cargo",
        "check",
        "-p",
        "galactic_domain",
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
        "galactic_domain",
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
        "galactic_domain",
        "-p",
        "galactic_sim",
        "-p",
        "galactic_persistence",
        "-p",
        "galactic_client",
    ),
)

FULL_CHECK_COMMANDS = (
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


UNIVERSE_BIBLE = """\
# Bible d'univers et nomenclature V1

Cette bible fixe l'identité éditoriale du ruleset `default`. Elle sert de
référence pour tout nouveau nom visible par le joueur.

## Promesse

Galactic est une science-fiction de frontière sobre et lisible. L'Expédition
Aster s'établit dans les Confins d'Orphée, une région isolée depuis la Rupture
des anciennes routes. Depuis Port-Sillage, le joueur reconstruit une chaîne
industrielle, cartographie des systèmes silencieux et rencontre des puissances
dont les intentions restent incertaines.

Le monde doit évoquer :

- une exploration méthodique plutôt qu'une aventure magique ;
- une technologie industrielle compréhensible ;
- des distances, des délais et une logistique qui comptent ;
- des traces d'un espace humain fragmenté, sans expliquer trop tôt tous ses
  mystères.

## Ton et langue

- Tous les textes d'interface et noms communs sont en français.
- Les noms propres sont courts, prononçables et distincts au premier regard.
- Le vocabulaire maritime est réservé au voyage, au fret et aux colonies.
- Le vocabulaire de lumière sert à la détection, aux sondes et à l'énergie.
- Les termes militaires restent fonctionnels et sobres.
- Éviter les anglicismes, les numéros de modèle gratuits et les superlatifs
  comme `ultime`, `suprême` ou `méga`.
- Un nom évocateur ne doit jamais masquer la fonction : `Sonde Luciole` reste
  identifiable comme une sonde.

## Repères canoniques

| Élément | Nom |
|---|---|
| Région | Confins d'Orphée |
| Faction joueur | Expédition Aster |
| Faction neutre | Communes des Confins |
| Puissance hostile dormante | Directoire Vesper |
| Système natal | Hélianthe |
| Planète mère | Nacre |
| Colonie initiale | Port-Sillage |

### Factions

- **Expédition Aster** : organisation scientifique et industrielle envoyée
  pour rouvrir la frontière. Son lexique privilégie navigation, veille,
  assemblage et implantation.
- **Communes des Confins** : habitats autonomes sans autorité centrale unique.
  Les futurs noms associés doivent évoquer havres, relais, comptoirs et
  communautés.
- **Directoire Vesper** : puissance structurée, distante et potentiellement
  hostile. Son lexique futur privilégiera ordre, protocoles, cohortes et
  désignations froides.

## Systèmes et planètes

Les seize systèmes du preset MVP ont un nom propre fixe :

1. Hélianthe
2. Vespera
3. Néréide
4. Talos
5. Cyrène
6. Ophira
7. Méroé
8. Eidolon
9. Sélène
10. Praxia
11. Ilyr
12. Calder
13. Thémis
14. Orphéon
15. Nacréon
16. Arkan

Règles :

- un système porte un nom propre unique ;
- une planète non baptisée reprend le nom du système suivi d'une lettre
  astronomique minuscule : `Vespera b`, `Vespera c` ;
- une planète habitée ou scénaristiquement importante peut recevoir un nom
  propre court, comme `Nacre` ;
- une colonie reçoit un nom d'implantation distinct de sa planète :
  `Port-Sillage`, `Relais-Cyrène`, `Havre-Néréide` ;
- ne pas employer `Prime`, `Major` ou `Minor` sans nécessité astronomique ou
  politique explicite.

## Infrastructures

| Identifiant stable | Nom affiché | Famille |
|---|---|---|
| `metal_mine` | Fosse sidérurgique | extraction |
| `crystal_extractor` | Extracteur cristallin | extraction |
| `fuel_refinery` | Raffinerie de volatils | transformation |
| `power_plant` | Réacteur hélionique | énergie |
| `warehouse` | Dépôt logistique | logistique |
| `construction_center` | Atelier d'assemblage | industrie |
| `research_lab` | Institut d'analyse | science |
| `shipyard` | Chantier orbital | industrie orbitale |

Un nouveau bâtiment suit la formule `fonction + précision éventuelle`. Sa
description commence par ce qu'il fait, puis précise son rôle dans la boucle de
jeu.

## Recherches

| Identifiant stable | Nom affiché | Déblocage |
|---|---|---|
| `spatial_detection` | Veille sidérale | cartographie des signaux |
| `propulsion` | Propulsion à flux | transit interstellaire |
| `cargo_capacity` | Architecture de soute | soutes modulaires |
| `remote_extraction` | Prospection autonome | extraction hors-colonie |
| `planetary_analysis` | Spectrométrie planétaire | diagnostic des mondes |
| `colonization` | Ingénierie d'implantation | fondation d'avant-postes |

Une technologie décrit une discipline ou une méthode, pas seulement son bonus.
Le libellé de déblocage décrit l'action rendue possible.

## Vaisseaux

| Identifiant stable | Nom affiché | Rôle |
|---|---|---|
| `light_probe` | Sonde Luciole | reconnaissance rapide |
| `light_cargo` | Caboteur Sillage | fret à courte portée |
| `colony_ship` | Arche Pionnière | fondation de colonie |

Formule recommandée :

- sonde : phénomène lumineux ou instrument d'observation ;
- cargo : vocabulaire maritime ou logistique ;
- colonisation : arche, implantation ou départ ;
- militaire : fonction tactique suivie d'un nom de classe ;
- soutien : rôle opérationnel immédiatement lisible.

Les classes futures peuvent être introduites sous la forme
`Frégate Rempart`, `Croiseur Vigie` ou `Ravitailleur Estuaire`. Un même nom de
classe ne doit pas être réutilisé pour deux rôles.

## Contrat technique

- Les identifiants RON en `snake_case` sont des clés de sauvegarde et ne sont
  jamais renommés pour une raison éditoriale.
- Les noms et descriptions du ruleset peuvent évoluer avec
  `content_version`; ils ne modifient pas son empreinte structurelle.
- Les noms générés des systèmes et planètes participent au fingerprint de
  l'univers. Toute modification exige une nouvelle `GENERATION_VERSION` et une
  nouvelle valeur de référence.
- Une nouvelle mécanique conserve un nom fonctionnel dans Rust et reçoit son
  nom d'univers dans les données affichées.
"""


WORLD_GENERATION_BEFORE = """\
    StarSystem {
        id,
        name: system_name(index, rng),
        position,
        star,
        planets: generate_planets(id, index, rng),
    }
}
"""

WORLD_GENERATION_AFTER = """\
    let name = system_name(index, rng);
    let planets = generate_planets(id, index, &name, rng);

    StarSystem {
        id,
        name,
        position,
        star,
        planets,
    }
}
"""

WORLD_PLANETS_BEFORE = """\
fn generate_planets(system_id: SystemId, system_index: usize, rng: &mut ChaCha8Rng) -> Vec<Planet> {
    let count = if system_index == 0 {
        3
    } else {
        rng.random_range(1..=5)
    };

    (0..count)
        .map(|index| {
            let id = PlanetId::from_system_index(system_id, index as u32);
            if system_index == 0 && index == 0 {
                return Planet {
                    id,
                    name: "Aster Prime".to_string(),
                    kind: PlanetKind::Ocean,
                    habitability: 92,
                };
            }

            let kind = random_planet_kind(rng);
            Planet {
                id,
                name: planet_name(system_index, index),
                kind,
                habitability: habitability_for(kind, rng),
            }
        })
        .collect()
}
"""

WORLD_PLANETS_AFTER = """\
fn generate_planets(
    system_id: SystemId,
    system_index: usize,
    system_name: &str,
    rng: &mut ChaCha8Rng,
) -> Vec<Planet> {
    let count = if system_index == 0 {
        3
    } else {
        rng.random_range(1..=5)
    };

    (0..count)
        .map(|index| {
            let id = PlanetId::from_system_index(system_id, index as u32);
            if system_index == 0 && index == 0 {
                return Planet {
                    id,
                    name: "Nacre".to_string(),
                    kind: PlanetKind::Ocean,
                    habitability: 92,
                };
            }

            let kind = random_planet_kind(rng);
            Planet {
                id,
                name: planet_name(system_name, index),
                kind,
                habitability: habitability_for(kind, rng),
            }
        })
        .collect()
}
"""

WORLD_NAMES_BEFORE = """\
fn system_name(index: usize, rng: &mut ChaCha8Rng) -> String {
    const PREFIXES: &[&str] = &[
        "Aster", "Nova", "Kepler", "Vega", "Orion", "Lyra", "Cygni", "Helio",
    ];
    const SUFFIXES: &[&str] = &[
        "Reach", "Gate", "Hold", "Bastion", "Drift", "Crown", "Harbor", "Span",
    ];

    if index == 0 {
        "Aster".to_string()
    } else {
        format!(
            "{} {}",
            PREFIXES[rng.random_range(0..PREFIXES.len())],
            SUFFIXES[rng.random_range(0..SUFFIXES.len())]
        )
    }
}

fn planet_name(system_index: usize, planet_index: usize) -> String {
    format!("P{}-{}", system_index + 1, planet_index + 1)
}
"""

WORLD_NAMES_AFTER = """\
fn system_name(index: usize, rng: &mut ChaCha8Rng) -> String {
    const NAMES: &[&str] = &[
        "Hélianthe",
        "Vespera",
        "Néréide",
        "Talos",
        "Cyrène",
        "Ophira",
        "Méroé",
        "Eidolon",
        "Sélène",
        "Praxia",
        "Ilyr",
        "Calder",
        "Thémis",
        "Orphéon",
        "Nacréon",
        "Arkan",
    ];

    if index == 0 {
        return NAMES[0].to_string();
    }

    // Preserve the two random draws used by generation version 2 so that this
    // editorial migration changes identities, not physical world properties.
    let _ = rng.random_range(0..8);
    let _ = rng.random_range(0..8);

    let base = NAMES[index % NAMES.len()];
    let cycle = index / NAMES.len();
    if cycle == 0 {
        base.to_string()
    } else {
        format!("{base}-{}", cycle + 1)
    }
}

fn planet_name(system_name: &str, planet_index: usize) -> String {
    const DESIGNATORS: &[&str] = &["b", "c", "d", "e", "f", "g", "h", "i"];
    let designator = DESIGNATORS.get(planet_index).copied().unwrap_or("x");
    format!("{system_name} {designator}")
}
"""

WORLD_NAMES_TEST = """\

    #[test]
    fn canonical_mvp_names_are_stable_and_unique() {
        let universe = generate_universe(UniverseConfig::mvp());
        let system_names = universe
            .systems
            .iter()
            .map(|system| system.name.as_str())
            .collect::<Vec<_>>();
        let unique_names = system_names.iter().copied().collect::<BTreeSet<_>>();

        assert_eq!(
            system_names,
            [
                "Hélianthe",
                "Vespera",
                "Néréide",
                "Talos",
                "Cyrène",
                "Ophira",
                "Méroé",
                "Eidolon",
                "Sélène",
                "Praxia",
                "Ilyr",
                "Calder",
                "Thémis",
                "Orphéon",
                "Nacréon",
                "Arkan",
            ],
        );
        assert_eq!(unique_names.len(), system_names.len());
        assert_eq!(universe.systems[0].planets[0].name, "Nacre");
        assert_eq!(universe.systems[0].planets[1].name, "Hélianthe c");
        assert_eq!(universe.systems[1].planets[0].name, "Vespera b");
    }
"""

ARCHITECTURE_ADDITION = """\


## MVP-020-B — Bible d'univers et nomenclature V1

Le ruleset `default` suit désormais une référence éditoriale unique :
`docs/universe_bible.md`. Elle fixe le ton de science-fiction de frontière, les
familles lexicales et les noms canoniques des factions, infrastructures,
recherches et premiers vaisseaux.

Les identifiants techniques restent inchangés. Les nouveaux libellés du ruleset
n'altèrent donc ni les coûts, ni les capacités, ni son empreinte structurelle.
`content_version` passe à 6.

Le générateur utilise seize noms de systèmes fixes pour le preset MVP. Les
planètes non baptisées suivent la convention astronomique `Système b`,
`Système c`, tandis que la planète mère devient `Nacre` et la colonie initiale
`Port-Sillage`. Les tirages aléatoires historiques sont consommés à l'identique
afin que cette passe éditoriale ne modifie pas les propriétés physiques de
l'univers.

Les noms faisant partie du fingerprint de l'univers, cette modification
volontaire porte `GENERATION_VERSION` à 3 et renouvelle le fingerprint de
référence. Les anciennes sauvegardes de développement sont incompatibles.
`GAME_STATE_VERSION`, `SAVE_VERSION` et `RULESET_SCHEMA_VERSION` restent
respectivement à 14, 15 et 5.
"""


REPLACEMENTS = {
    "README.md": (
        (
            "Guide de configuration : `docs/ruleset.md`.\n",
            "Guide de configuration : `docs/ruleset.md`.\n"
            "Référence éditoriale : `docs/universe_bible.md`.\n",
        ),
        ("| Chantier spatial | `Y` |", "| Chantier orbital | `Y` |"),
    ),
    "assets/rulesets/default/buildings.ron": (
        ("Mine de métal", "Fosse sidérurgique"),
        (
            "Extrait le métal nécessaire aux infrastructures et aux futurs vaisseaux.",
            "Extrait et trie les minerais métalliques destinés aux infrastructures.",
        ),
        ("Extracteur de cristal", "Extracteur cristallin"),
        (
            "Produit les cristaux employés par les systèmes avancés.",
            "Isole les cristaux techniques employés par les systèmes de précision.",
        ),
        ("Raffinerie de carburant", "Raffinerie de volatils"),
        (
            "Raffine le carburant des convois et des opérations orbitales.",
            "Transforme les composés volatils en carburant pour les opérations orbitales.",
        ),
        ("Centrale énergétique", "Réacteur hélionique"),
        (
            "Alimente les bâtiments de la colonie.",
            "Fournit l'énergie stable nécessaire aux installations de la colonie.",
        ),
        ("Entrepôt", "Dépôt logistique"),
        (
            "Augmente la capacité de stockage des trois ressources.",
            "Sécurise et augmente les réserves de métal, de cristal et de carburant.",
        ),
        ("Centre de construction", "Atelier d'assemblage"),
        (
            "Accélère les améliorations d'infrastructure.",
            "Coordonne les chantiers au sol et accélère les extensions d'infrastructure.",
        ),
        ("Laboratoire", "Institut d'analyse"),
        (
            "Produit les points scientifiques de la recherche globale.",
            "Mutualise les relevés de la colonie et produit les données de recherche.",
        ),
        ("Chantier spatial", "Chantier orbital"),
        (
            "Prépare la production des futurs équipements orbitaux.",
            "Assemble sondes, transports et arches dans les cales orbitales.",
        ),
    ),
    "assets/rulesets/default/craftables.ron": (
        ("Sonde légère", "Sonde Luciole"),
        (
            "Un éclaireur automatisé destiné aux premières opérations de reconnaissance.",
            "Un éclaireur automatisé rapide, conçu pour cartographier la frontière.",
        ),
        ("Cargo léger", "Caboteur Sillage"),
        (
            "Un transport orbital modeste, homologué pour déplacer du matériel utile.",
            "Un transport fiable à courte portée, adapté aux premières lignes logistiques.",
        ),
        ("Vaisseau-colonie", "Arche Pionnière"),
        (
            "Une administration complète emballée dans une coque pressurisée.",
            "Une implantation autonome réunie dans une coque lente et lourdement équipée.",
        ),
    ),
    "assets/rulesets/default/factions.ron": (
        ("Aster Expedition", "Expédition Aster"),
        ("Mondes indépendants", "Communes des Confins"),
        ("Directoire dormant", "Directoire Vesper"),
    ),
    "assets/rulesets/default/manifest.ron": (
        ("content_version: 5", "content_version: 6"),
    ),
    "assets/rulesets/default/starting_scenario.ron": (
        ('colony_name: "Aster Prime Colony"', 'colony_name: "Port-Sillage"'),
    ),
    "assets/rulesets/default/technologies.ron": (
        ("Détection spatiale", "Veille sidérale"),
        (
            "Améliore la détection des systèmes et prépare les missions de reconnaissance.",
            "Corrèle les signaux faibles afin de repérer les systèmes encore inconnus.",
        ),
        ("Détection des systèmes inconnus", "Cartographie des signaux inconnus"),
        ('name: "Propulsion"', 'name: "Propulsion à flux"'),
        (
            "Débloque les moteurs nécessaires aux déplacements interstellaires.",
            "Stabilise des moteurs capables de soutenir un transit interstellaire.",
        ),
        ("Voyage interstellaire", "Transit interstellaire"),
        ("Capacité cargo", "Architecture de soute"),
        (
            "Augmente la capacité logistique des futurs vaisseaux et transports.",
            "Standardise des modules de fret compacts pour les flottes logistiques.",
        ),
        ("Soutes cargo améliorées", "Soutes modulaires"),
        ("Extraction distante", "Prospection autonome"),
        (
            "Autorise l'exploitation de ressources sans colonie permanente.",
            "Coordonne l'exploitation temporaire de sites éloignés sans implantation.",
        ),
        ("Missions d'extraction distante", "Extraction hors-colonie"),
        ("Analyse planétaire", "Spectrométrie planétaire"),
        (
            "Révèle les caractéristiques nécessaires à l'évaluation des mondes.",
            "Établit le profil physique et biologique complet d'un monde observé.",
        ),
        ("Analyse exacte des planètes", "Diagnostic planétaire complet"),
        ('name: "Colonisation"', 'name: "Ingénierie d\'implantation"'),
        (
            "Débloque la fondation de nouvelles colonies sur les mondes compatibles.",
            "Formalise les infrastructures nécessaires à une colonie autonome.",
        ),
        ("Fondation de colonies", "Fondation d'avant-postes"),
    ),
    "crates/galactic_client/src/craft_ui.rs": (
        (
            'Text::new("Chantier spatial  [Y]")',
            'Text::new("Chantier orbital  [Y]")',
        ),
        (
            '"Chantier spatial  [Y]".to_string()',
            '"Chantier orbital  [Y]".to_string()',
        ),
        ("Construis un Chantier spatial", "Construis un Chantier orbital"),
        ("Chantier spatial requis", "Chantier orbital requis"),
    ),
    "crates/galactic_client/src/lib.rs": (
        ("MINE DE MÉTAL", "FOSSE SIDÉRURGIQUE"),
        ("Aster Prime Colony", "Port-Sillage"),
    ),
    "crates/galactic_client/src/research_ui.rs": (
        ("Laboratoires cumulés", "Instituts d'analyse cumulés"),
        ("Construis un Laboratoire", "Construis un Institut d'analyse"),
        (
            'ResearchError::NoResearchCapacity => "Laboratoire requis".to_string(),',
            'ResearchError::NoResearchCapacity => "Institut d\'analyse requis".to_string(),',
        ),
        (
            'assert!(text.contains("Laboratoire requis"));',
            'assert!(text.contains("Institut d\'analyse requis"));',
        ),
        ('text.contains("Détection")', 'text.contains("VEILLE SIDÉRALE")'),
    ),
    "crates/galactic_sim/src/building_catalog.rs": (
        ("Mine de métal", "Fosse sidérurgique"),
    ),
    "crates/galactic_sim/src/state.rs": (
        ("Aster Prime Colony", "Port-Sillage"),
    ),
    "docs/galactic_mvp_cadrage_et_backlog.md": (
        (
            "3. Construire Laboratoire et Chantier spatial.",
            "3. Construire Institut d'analyse et Chantier orbital.",
        ),
        (
            """1. Mine de métal
2. Extracteur de cristal
3. Raffinerie de carburant
4. Centrale énergétique
5. Entrepôt
6. Centre de construction
7. Laboratoire de recherche
8. Chantier spatial""",
            """1. Fosse sidérurgique
2. Extracteur cristallin
3. Raffinerie de volatils
4. Réacteur hélionique
5. Dépôt logistique
6. Atelier d'assemblage
7. Institut d'analyse
8. Chantier orbital""",
        ),
        (
            "Technologies initiales : Détection spatiale, Propulsion, Capacité cargo, Extraction distante, Analyse planétaire et Colonisation.",
            "Technologies initiales : Veille sidérale, Propulsion à flux, Architecture de\n"
            "soute, Prospection autonome, Spectrométrie planétaire et Ingénierie\n"
            "d'implantation.",
        ),
        (
            "Les trois unités actives du MVP sont la Sonde légère, le Cargo léger et le Vaisseau-colonie. Une flotte possède un propriétaire, une localisation, une composition, une cargaison et éventuellement une mission.",
            "Les trois unités actives du MVP sont la Sonde Luciole, le Caboteur Sillage et\n"
            "l'Arche Pionnière. Une flotte possède un propriétaire, une localisation, une\n"
            "composition, une cargaison et éventuellement une mission.",
        ),
        ("- Chantier spatial : crafts et files.", "- Chantier orbital : crafts et files."),
    ),
    "docs/mvp_architecture.md": (
        ("`Aster Prime`", "`Nacre`"),
        ("`Aster Expedition`", "`Expédition Aster`"),
        ("Mine de métal niveau 1", "Fosse sidérurgique niveau 1"),
        ("Extracteur de cristal niveau 1", "Extracteur cristallin niveau 1"),
        ("Tous les Laboratoires des", "Tous les Instituts d'analyse des"),
        (
            """1. Détection spatiale ;
2. Propulsion ;
3. Capacité cargo ;
4. Extraction distante ;
5. Analyse planétaire ;
6. Colonisation.""",
            """1. Veille sidérale ;
2. Propulsion à flux ;
3. Architecture de soute ;
4. Prospection autonome ;
5. Spectrométrie planétaire ;
6. Ingénierie d'implantation.""",
        ),
        (
            """Détection spatiale
├── Propulsion
│   └── Capacité cargo
│       ├── Extraction distante
│       └── Colonisation
└── Analyse planétaire
    └── Colonisation""",
            """Veille sidérale
├── Propulsion à flux
│   └── Architecture de soute
│       ├── Prospection autonome
│       └── Ingénierie d'implantation
└── Spectrométrie planétaire
    └── Ingénierie d'implantation""",
        ),
        ("Le Laboratoire du catalogue", "L'Institut d'analyse du catalogue"),
        (
            "les niveaux actifs de Laboratoire",
            "les niveaux actifs d'Institut d'analyse",
        ),
        ("Sans Laboratoire actif", "Sans Institut d'analyse actif"),
        ("Une amélioration de Laboratoire", "Une amélioration d'Institut d'analyse"),
        ("lorsqu'un Laboratoire termine", "lorsqu'un Institut d'analyse termine"),
        ("un Laboratoire produit", "un Institut d'analyse produit"),
        ("Le Chantier spatial produit", "Le Chantier orbital produit"),
    ),
    "docs/ruleset.md": (
        (
            "Le hot reload pendant une partie n'est pas pris en charge. Il faut redémarrer le\n"
            "jeu après chaque modification.\n",
            "Le hot reload pendant une partie n'est pas pris en charge. Il faut redémarrer le\n"
            "jeu après chaque modification.\n\n"
            "Les noms visibles du ruleset `default` suivent\n"
            "[`docs/universe_bible.md`](universe_bible.md). Les identifiants techniques\n"
            "restent stables même lorsqu'un libellé ou une description évolue.\n",
        ),
    ),
}


def replace_once(root: Path, relative: str, before: str, after: str) -> None:
    path = root / relative
    text = path.read_text(encoding="utf-8")
    count = text.count(before)
    if count != 1:
        raise base.MigrationError(
            f"{relative}: motif attendu exactement une fois, trouvé {count}: {before[:80]!r}"
        )
    path.write_text(text.replace(before, after, 1), encoding="utf-8")


def append_once(root: Path, relative: str, marker: str, addition: str) -> None:
    path = root / relative
    text = path.read_text(encoding="utf-8")
    if marker in text:
        raise base.MigrationError(f"{relative}: contenu {MIGRATION} déjà présent")
    if not text.endswith("\n"):
        raise base.MigrationError(f"{relative}: fin de fichier inattendue")
    path.write_text(text + addition, encoding="utf-8")


def transform_world(root: Path) -> None:
    path = "crates/galactic_domain/src/world.rs"
    replace_once(
        root,
        path,
        "pub const GENERATION_VERSION: u32 = 2;",
        "pub const GENERATION_VERSION: u32 = 3;",
    )
    replace_once(
        root,
        path,
        "pub const MVP_REFERENCE_FINGERPRINT: u64 = 12539308657388844103;",
        "pub const MVP_REFERENCE_FINGERPRINT: u64 = 202568768259003109;",
    )
    replace_once(root, path, WORLD_GENERATION_BEFORE, WORLD_GENERATION_AFTER)
    replace_once(root, path, WORLD_PLANETS_BEFORE, WORLD_PLANETS_AFTER)
    replace_once(root, path, WORLD_NAMES_BEFORE, WORLD_NAMES_AFTER)
    replace_once(
        root,
        path,
        """        assert_eq!(planet.kind, PlanetKind::Ocean);
        assert!(planet.habitability >= 90);
    }
""",
        """        assert_eq!(planet.kind, PlanetKind::Ocean);
        assert!(planet.habitability >= 90);
    }
"""
        + WORLD_NAMES_TEST,
    )


def transform_tree(root: Path) -> None:
    for relative, replacements in REPLACEMENTS.items():
        for before, after in replacements:
            replace_once(root, relative, before, after)

    cadrage = root / "docs/galactic_mvp_cadrage_et_backlog.md"
    cadrage_text = cadrage.read_text(encoding="utf-8")
    if not cadrage_text.endswith("\n"):
        cadrage.write_text(cadrage_text + "\n", encoding="utf-8")

    transform_world(root)
    append_once(
        root,
        "docs/mvp_architecture.md",
        "## MVP-020-B — Bible d'univers et nomenclature V1",
        ARCHITECTURE_ADDITION,
    )

    bible = root / CREATED_PATHS[0]
    if bible.exists():
        raise base.MigrationError(f"{CREATED_PATHS[0]} existe déjà")
    bible.write_text(UNIVERSE_BIBLE, encoding="utf-8")


def configure_shared_guards() -> None:
    base.BASELINE_SHA = BASELINE_SHA
    base.BASELINE_BLOBS = {**MODIFIED_BLOBS, **DEPENDENCY_BLOBS}
    base.CREATED_PATHS = CREATED_PATHS
    base.DELETED_PATHS = ()
    base.EXPECTED_PATHS = EXPECTED_PATHS


def selected_checks(*, full_checks: bool):
    return FULL_CHECK_COMMANDS if full_checks else TARGETED_CHECK_COMMANDS


def validate_expected_diff(worktree: Path) -> None:
    result = base.run(
        ("git", "diff", "--name-only", "HEAD", "--"),
        cwd=worktree,
        capture=True,
    )
    found = frozenset(
        line.decode("utf-8") for line in result.stdout.splitlines() if line
    )
    if found != EXPECTED_PATHS:
        missing = sorted(EXPECTED_PATHS - found)
        unexpected = sorted(found - EXPECTED_PATHS)
        raise base.MigrationError(
            f"périmètre inattendu ; manquants={missing}, inattendus={unexpected}"
        )


def validated_patch(
    root: Path,
    *,
    run_checks: bool,
    full_checks: bool,
) -> bytes:
    with tempfile.TemporaryDirectory(
        prefix="galactic-mvp020b-", dir=root.parent
    ) as temporary:
        worktree = Path(temporary) / "worktree"
        added = False
        try:
            base.run(
                ("git", "worktree", "add", "--detach", str(worktree), base.head_sha(root)),
                cwd=root,
            )
            added = True
            transform_tree(worktree)
            base.run(("git", "add", "-N", "--", *CREATED_PATHS), cwd=worktree)
            base.run(("git", "diff", "--check"), cwd=worktree)
            validate_expected_diff(worktree)

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
            validate_expected_diff(worktree)
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
    parent = root / "backups" / ".mvp020b-backup"
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


def already_applied(root: Path) -> bool:
    bible = root / CREATED_PATHS[0]
    if not bible.is_file():
        return False
    world = (root / "crates/galactic_domain/src/world.rs").read_text(
        encoding="utf-8"
    )
    manifest = (root / "assets/rulesets/default/manifest.ron").read_text(
        encoding="utf-8"
    )
    scenario = (root / "assets/rulesets/default/starting_scenario.ron").read_text(
        encoding="utf-8"
    )
    return (
        "pub const GENERATION_VERSION: u32 = 3;" in world
        and "content_version: 6" in manifest
        and 'colony_name: "Port-Sillage"' in scenario
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Prépare MVP-020-B : bible d'univers, nomenclature cohérente "
            "et noms canoniques déterministes."
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
        help="valide baseline, transformations et périmètre sans compiler ni modifier",
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
        if run_checks:
            base.ensure_command("cargo")

        root = base.resolve_root(args.root)
        if already_applied(root):
            print(f"{MIGRATION} est déjà appliqué ; aucune modification nécessaire.")
            return 0

        base.verify_baseline(root, force=args.force)
        if args.skip_checks and not args.dry_run:
            print(
                "AVERTISSEMENT : contrôles Cargo ignorés pendant l'application.",
                file=sys.stderr,
            )
        candidate = validated_patch(
            root,
            run_checks=run_checks,
            full_checks=args.full_checks,
        )

        if args.dry_run:
            checks_label = " avec contrôles Cargo" if run_checks else ""
            print(
                f"Dry-run réussi{checks_label} : baseline, transformations et "
                "périmètre valides. Le dépôt principal n'a pas été modifié."
            )
            return 0

        with tempfile.TemporaryDirectory(
            prefix="galactic-mvp020b-verify-", dir=root.parent
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

        print(f"{MIGRATION} appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GENERATION_VERSION=3, content_version=6, "
            "GAME_STATE_VERSION=14, SAVE_VERSION=15, RULESET_SCHEMA_VERSION=5"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
