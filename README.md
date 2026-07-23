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
peuvent etre recreees depuis l'etat metier sans conserver d'`Entity`.

Documentation courte: `docs/mvp_architecture.md`.

## Controles Actuels

| Action | Controle |
|---|---|
| Pause simulation | `Espace` |
| Vitesse x1 | `1` |
| Vitesse x2 | `2` |
| Vitesse x4 | `3` |
| Reconstruire les vues Bevy | `R` |

## Baseline

- POC valide: `docs/poc_archive/poc_02`
- Performance POC constatee localement: environ 10 FPS en debug, 60 FPS en
  release.
- Base MVP active: scene minimale de 16 systemes pour valider le decouplage avant
  de rebrancher les workflows de gameplay.
