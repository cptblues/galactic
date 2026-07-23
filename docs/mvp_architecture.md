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

## MVP-004 — Univers immuable et état mutable

Le moteur distingue désormais explicitement deux sources de données :

```text
UniverseDefinition (seed, systèmes, étoiles, planètes, routes)
        │ immuable et régénérable
        ▼
UniverseRepository (index SystemId / PlanetId en lecture seule)

GameState (temps, sélection, découvertes, colonies, stocks)
        │ mutable et sauvegardé
        ▼
Simulation = UniverseRepository + GameState
```

Règles :

- `GameState` ne contient plus `UniverseDefinition`.
- `Simulation::universe()` ne retourne qu'une référence immuable.
- `UniverseRepository` fournit les accès par `SystemId` et `PlanetId`.
- `GameState::colony()` fournit l'accès à une colonie par `ColonyId`.
- les commandes et ticks modifient uniquement `GameState` ;
- une sauvegarde contient une `UniverseReference` (seed, version, fingerprint)
  et un `MutableGameSave`, jamais une copie des systèmes et planètes ;
- la restauration régénère l'univers, vérifie son fingerprint, puis injecte
  l'état mutable validé.

Version de contrat mutable actuelle : `GAME_STATE_VERSION = 1`.
Version d'enveloppe de sauvegarde actuelle : `SAVE_VERSION = 2`.


## MVP-005 — Temps stratégique déterministe

Le temps métier est désormais indépendant du nombre d'images rendues :

```text
Durée réelle d'une frame
        │ multipliée par x1 / x2 / x4
        ▼
StrategicClock
        │ accumulation entière en nanosecondes
        ▼
Ticks fixes à 10 Hz
        ▼
Production / construction / recherche / missions
```

Règles :

- `StrategicTick` est le timestamp métier sauvegardable ;
- `StrategicDuration` exprime une durée en nombre entier de ticks ;
- `StrategicClock` conserve le tick courant et la fraction de tick restante ;
- la pause bloque uniquement l'horloge de simulation ;
- la caméra et l'interface continuent d'utiliser le temps Bevy normal ;
- `Simulation::advance(Duration)` remplace l'ancien avancement direct en `f32` ;
- changer le framerate ne change pas le nombre de ticks obtenus sur une même durée ;
- la sauvegarde conserve le tick courant, le reliquat et la vitesse ;
- `GAME_STATE_VERSION = 2` et `SAVE_VERSION = 3`.

Fréquence stratégique actuelle : `10 ticks/seconde`.


## MVP-006 — Graphe d'univers et routes

L'univers MVP est maintenant un graphe connexe plutôt qu'un simple ensemble de
positions :

```text
Systèmes générés
        │
        ├── arbre couvrant minimal déterministe
        └── routes locales vers les voisins proches
        ▼
Graphe connexe sans doublon
        ▼
Index d'adjacence UniverseRepository
        ▼
Voisinages / chemin minimal / distance en sauts
```

Règles :

- la seed MVP contient toujours 16 systèmes, dans la fourchette cible 12–20 ;
- un arbre couvrant minimal garantit que tous les systèmes sont accessibles
  depuis `SystemId(0)` ;
- des routes locales supplémentaires évitent un graphe réduit à un simple arbre ;
- les routes sont canoniques, sans boucle et sans doublon ;
- `UniverseRepository::shortest_path()` utilise un BFS déterministe ;
- `UniverseRepository::hop_distance()` retourne le nombre minimal de sauts ;
- l'univers complet reste immuable et est régénéré depuis la seed ;
- la vue Univers ne trace que les routes dont les deux systèmes sont connus ;
- l'instanciation limitée aux systèmes découverts sera traitée par `MVP-007`.

La modification volontaire du graphe incrémente `GENERATION_VERSION` et produit
un nouveau fingerprint de référence pour la seed MVP.
