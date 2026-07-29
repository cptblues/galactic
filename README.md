# Galactic

Prototype Bevy 0.19 en transition du POC valide vers un MVP de strategie
spatiale solo.

Le POC valide est conserve comme reference dans `docs/poc_archive/poc_02`. Le
code actif est maintenant une base MVP propre avec separation domaine,
simulation, persistance et client Bevy.

## Lancement

```bash
cargo run --release
cargo run --release -- --scale test    # 16 systèmes
cargo run --release -- --scale stress  # 128 systèmes
```

Sans option, le client utilise le preset jouable **MVP 64 systèmes**.

## Commandes Qualite

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release
```

## Architecture

- Bevy: `0.19`
- Racine executable: `galactic`
- Client Bevy: `crates/galactic_client`
- Domaine metier: `crates/galactic_domain`
- Simulation: `crates/galactic_sim`
- Persistance: `crates/galactic_persistence`

Le domaine, la simulation et la persistance ne dependent pas de Bevy. Les vues
peuvent etre recreees depuis l'etat metier sans conserver d'`Entity`. Le moteur
de mission calcule les trajets, verrouille les flottes et progresse uniquement
sur les ticks stratégiques. Une Sonde Luciole construite au chantier peut être
envoyée vers un système ou une planète détectés. Une cible planétaire locale est
identifiée sans révéler ses voisines ; une cible interstellaire ouvre ensuite la
frontière de découverte normale. La définition immuable de l'univers regroupe
aussi les systèmes en secteurs déterministes ; la vue globale affiche uniquement
les secteurs dont au moins un membre est connu.

Documentation courte: `docs/mvp_architecture.md`.

## Ruleset

Le contenu économique actif est chargé depuis `assets/rulesets/default/` au
démarrage. Les coûts, durées, textes, bâtiments, technologies, vaisseaux,
capacités, factions, relations initiales, limites de files et données de départ
peuvent être modifiés sans recompiler le jeu.

Guide de configuration : `docs/ruleset.md`.
Référence éditoriale : `docs/universe_bible.md`.

## Controles Actuels

| Action | Controle |
|---|---|
| Pause simulation | `Espace` |
| Vitesse x1 | `1` |
| Vitesse x2 | `2` |
| Vitesse x4 | `3` |
| Gestion planétaire | `C` |
| Recherche | `T` |
| Chantier orbital | `Y` |
| Sonder le système ou la planète sélectionnés | `K` |
| Analyser la planète sondée sélectionnée | `L` |
| Basculer projection 3D / 2,5D | `P` |
| Reconstruire les vues Bevy | `R` |

## Baseline

- POC valide: `docs/poc_archive/poc_02`
- Performance POC constatee localement: environ 10 FPS en debug, 60 FPS en
  release.
- Base MVP active : 64 systèmes ; les presets Test 16 et Stress 128 restent
  disponibles au lancement.

## MVP-023-C — Projection 2,5D et échelles galactiques

Le client démarre par défaut sur une galaxie reproductible de 64 systèmes.
`--scale test` conserve la carte de 16 systèmes pour les itérations rapides et
`--scale stress` charge 128 systèmes pour les mesures manuelles.

Dans la vue Univers, `P` anime le passage entre les positions 3D et leur
projection aplatie. Cette interpolation ne modifie jamais la définition de
l'univers, les routes, les distances ou les durées de mission. Étoiles, labels,
halos, routes et picking utilisent à chaque frame les mêmes positions affichées.

## MVP-023-D — Socle visuel Univers et Système

La projection 3D amplifie uniquement la hauteur affichée de la galaxie, sans
modifier les coordonnées métier. Au départ, la carte complète les systèmes
réellement connus ou détectés par une bulle bornée de signaux observés proches
de Port-Sillage. Ces signaux faibles n'ont ni nom, ni sélection, ni route :
ils ne constituent pas une connaissance de gameplay. Les routes révélées sont
tracées en pointillés, avec une cadence distincte pour les liaisons connues,
partielles et inter-secteurs.

Dans la vue Système, les six types de planète utilisent des textures
procédurales partagées de 64×32 pixels sur un mesh UV commun. Les planètes
identifiées tournent lentement sur elles-mêmes et suivent une orbite visuelle
indépendante des ticks de simulation. Atmosphères, anneaux et halos réutilisent
des meshes et matériaux partagés ; les corps seulement détectés restent des
silhouettes et ne révèlent aucune donnée cachée.

## MVP-023-E — Sondes planétaires et trajectoires

Les corps détectés d'un système reçoivent une désignation orbitale stable
(`Port-Sillage I`, `II`, etc.) avant que leur véritable identité soit connue.
Sélectionner l'un de ces corps puis appuyer sur `K` lance une Sonde Luciole :
à l'arrivée, cette planète seule passe au niveau `Probed`, révélant son nom et
son type sans anticiper l'analyse de sa colonisabilité.

Une mission locale reste dans la vue Système et sa durée augmente avec le rang
orbital de la cible. Une mission vers un autre système est animée le long de sa
route dans la vue Univers ; sa durée dépend du nombre de sauts connus. Le même
repère visuel suit l'aller puis le retour et sa position est calculée depuis les
ticks stratégiques, indépendamment du nombre d'images par seconde.

## MVP-024 — Analyse planétaire et colonisabilité

Une planète identifiée peut être analysée avec `L` après acquisition de
Spectrométrie planétaire. Le rapport horodaté révèle l'environnement, les
contraintes d'installation, l'habitabilité exacte et quatre potentiels de
ressources déterministes. Il est conservé dans la sauvegarde séparément des
données réelles de l'univers.

L'inspecteur évalue aussi la colonisabilité sans lancer de colonie : il indique
les blocages précis liés au type de monde, à l'habitabilité, aux routes connues,
à la technologie, à la cargaison de fondation et à la limite de colonies. Les
seuils, profils planétaires, coûts et contraintes viennent du ruleset externe.

## MVP-025 — Occupants, forces et défenses planétaires

Chaque planète possède désormais une présence réelle déterministe : inoccupée,
neutre, hostile ou contrôlée par le joueur. Population, unités terrestres et
défenses orbitales sont conservées dans l'état mutable et la sauvegarde. Le
catalogue `planetary_presence.ron` définit les unités, leurs statistiques et les
profils d'occupation utilisés par la seed.

Ces données réelles ne sont jamais lues directement par l'inspecteur. Une sonde
ne révèle qu'un contact global et masque l'identité comme les types d'unités.
L'analyse remplace ce contact par un rapport daté avec faction, population et
effectifs sous forme de fourchettes. Les valeurs exactes restent réservées aux
colonies possédées. Une présence étrangère non sécurisée bloque explicitement
la colonisation.

## MVP-025-B — Attaques et combat V1

Une planète analysée et occupée par une faction étrangère peut être attaquée
avec `M`. Le raccourci réutilise une flotte militaire disponible ou regroupe
les Frégates Rempart présentes dans la colonie de départ. La mission suit le
même trajet aller-retour que la reconnaissance puis résout un combat
déterministe à l'arrivée.

Le rapport persistant détaille les forces engagées, pertes, survivants,
dommages, récupération et contrôle territorial. Une cible modifiée pendant le
trajet invalide proprement l'attaque sans appliquer l'ancien instantané. Afin
que cette boucle soit immédiatement testable, chaque système voisin du départ
contient au moins une petite patrouille hostile ; les autres planètes
conservent leur distribution déterministe et peuvent légitimement être vides.
