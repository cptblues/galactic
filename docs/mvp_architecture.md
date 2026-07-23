# Architecture MVP

Cette base est le point de depart de `MVP-002`. Elle remplace le POC actif par un
workspace propre, tout en conservant le POC valide dans `docs/poc_archive/poc_02`.

## Crates

- `galactic_domain` contient les identifiants stables, la carte stellaire, les
  ressources metier et la generation deterministe de l'univers.
- `galactic_sim` contient l'etat de partie, les commandes, les evenements et la
  boucle de simulation.
- `galactic_persistence` contient le modele de snapshot/restauration. Le format
  disque concret viendra plus tard.
- `galactic_client` est le seul crate qui depend de Bevy. Il gere fenetre,
  camera, meshes, UI et synchronisation presentation/simulation.
- La racine `galactic` reste un binaire fin pour conserver `cargo run --release`.

Flux de dependances:

```text
galactic -> galactic_client -> galactic_sim -> galactic_domain
galactic_persistence ---------> galactic_sim -> galactic_domain
```

`galactic_domain`, `galactic_sim` et `galactic_persistence` ne dependent pas de
Bevy. Les types `Entity`, `Camera3d`, `Mesh3d`, `Text` et autres composants
visuels restent dans `galactic_client`.

## Flux De Donnees

1. Le client Bevy transforme les entrees clavier/souris en `GameCommand`.
2. `galactic_sim` applique les commandes sur `GameState`.
3. La simulation retourne des `GameEvent` purs, sans reference Bevy.
4. Le client consomme ces evenements pour mettre a jour l'UI et les vues.
5. Les vues peuvent etre despawnees puis recreees a partir de `GameState`.

Dans le client actuel, `R` reconstruit les entites de vue Bevy depuis la
simulation sans reinitialiser `GameState`.

## Regles

- Aucune logique de production, mission, colonie ou economie ne doit utiliser un
  `Entity` Bevy.
- Les composants Bevy representent une vue, jamais la source de verite metier.
- Les identifiants metier sont des newtypes stables: `SystemId`, `PlanetId`,
  `ColonyId`, `FleetId`, `MissionId`.
- Une sauvegarde doit pouvoir reconstruire l'univers depuis une graine et un etat
  mutable minimal.

## Baseline Actuelle

- POC valide archive: `docs/poc_archive/poc_02`.
- Baseline POC observee par l'utilisateur: environ 10 FPS en debug, 60 FPS en
  release sur le poste actuel.
- Base MVP active: scene Bevy minimale de 16 systemes, simulation testable sans
  camera ni rendu 3D.
