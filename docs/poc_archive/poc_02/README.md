# Galactic POC

Prototype desktop Bevy 0.19 pour visualiser une galaxie 3D procedurale, ouvrir un systeme stellaire et tester la lisibilite d'une carte strategique dense.

## Prerequis

- Rust stable recent avec edition 2024.
- Pilotes GPU compatibles wgpu.
- Linux, Windows ou macOS.

## Lancement

```bash
cargo run --release
```

Commandes de qualite :

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Controles

| Action | Controle |
|---|---|
| Rotation camera | Bouton droit + deplacement |
| Pan camera | Bouton central, ou `Maj` + bouton droit |
| Zoom | Molette |
| Selection | Clic gauche |
| Entrer dans le systeme | Double-clic ou `Entree` |
| Retour galaxie | `Echap` ou `Retour arriere` |
| Focaliser selection | `F` |
| Pause orbites | `Espace` |
| Regenerer meme graine | `R` |
| Nouvelle graine | `N` |
| Routes | `L` |
| Orbites | `O` |
| Labels | `T` |
| Aide | `F1` |
| Debug | `F3` |
| Preset graphique | `F4` |
| Recherche | `/` ou `Ctrl+F` |
| Historique | `Alt+Gauche` / `Alt+Droite` |
| Projection 3D/aplatie | `P` |
| Origine/capitale | `Home` |
| Chemin vers selection | `K` |
| Presets filtres | `1`, `2`, `3`, `4` |
| Presets densite | `Ctrl+1`, `Ctrl+2`, `Ctrl+3` |
| Calques | `B`, `I`, `V`, `A`, `G`, `H`, `U` |
| Mission | `M`, puis `Y` sur une planete |

## Architecture

Le code suit les plugins de la specification :

- `data` : identifiants stables, modeles de galaxie/systeme/corps, ressources globales.
- `generation` : generation deterministe ChaCha8, noms, routes, tests.
- `views` : entites de vue galaxie et vue systeme.
- `camera` : camera orbitale lissee.
- `interaction` : picking mesh Bevy 0.19, selection, raccourcis.
- `rendering` : meshes/materials partages, starfield, routes et orbites via gizmos.
- `ui` et `diagnostics` : HUD, inspecteur, aide, FPS et debug.
- `strategic` : factions, secteurs, controle, alertes, flottes, BFS et mission.
- `map` : filtres, zoom semantique, labels, projection et selection dense.
- `navigation` : recherche, historique, fil d'Ariane et chemins.
- `usability` : metriques locales et validation mission.

## Generation

La graine par defaut est `42`. La configuration par defaut genere 500 systemes, 4 bras spiraux, une epaisseur verticale faible, un bulbe central et quelques systemes hors bras. Les routes relient les voisins proches sans representer un pathfinding.

## POC 0.2

Le POC 0.2 ajoute une couche strategique statique :

- cinq empires fictifs avec capitales, dispositions et couleurs distinctes ;
- secteurs, etats d'exploration, controle, alertes, mondes habitables et anomalies ;
- routes majeures/mineures, flottes factices et halos locaux de territoire ;
- zoom semantique, labels dynamiques avec budget et collision approximative ;
- filtres cartographiques, recherche, historique, fil d'Ariane et mode aplati ;
- mission locale : trouver une planete oceanique non colonisee, a trois routes ou moins d'un allie, hors territoire hostile.

## Notes MCP

Aucun MCP Bevy officiel n'est integre. Le POC reste volontairement autonome et cible les APIs natives Bevy 0.19, notamment `MeshPickingPlugin` pour la selection.

## Limites connues

- Pas de champ de saisie de graine dans l'UI ; `R`, `N` et les presets de densite couvrent la regeneration.
- La transition camera galaxie/systeme est fonctionnelle mais volontairement simple.
- Les territoires sont des halos locaux approximatifs, pas des frontieres politiques exactes.
- Les orbites, routes et overlays strategiques utilisent des gizmos 3D, suffisants pour le POC.
- La recherche est simple, synchrone et limitee a 10 resultats.
