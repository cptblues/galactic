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
envoyée vers le système détecté sélectionné : son arrivée révèle la cible,
ses routes directes et exactement le prochain anneau de signaux. La définition
immuable de l'univers regroupe aussi les systèmes en secteurs déterministes ;
la vue globale affiche uniquement les secteurs dont au moins un membre est
connu.

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
| Lancer une reconnaissance | `K` |
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
