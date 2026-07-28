# Galactic

Prototype Bevy 0.19 en transition du POC valide vers un MVP de strategie
spatiale solo.

Le POC valide est conserve comme reference dans `docs/poc_archive/poc_02`. Le
code actif est maintenant une base MVP propre avec separation domaine,
simulation, persistance et client Bevy.

## Lancement

```bash
cargo run --release
```

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
ses routes directes et exactement le prochain anneau de signaux.

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
| Reconstruire les vues Bevy | `R` |

## Baseline

- POC valide: `docs/poc_archive/poc_02`
- Performance POC constatee localement: environ 10 FPS en debug, 60 FPS en
  release.
- Base MVP active: scene minimale de 16 systemes pour valider le decouplage avant
  de rebrancher les workflows de gameplay.
