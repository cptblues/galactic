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


## MVP-007 — Vue Univers limitée au voisinage découvert

La scène Bevy ne représente plus systématiquement tous les systèmes générés.

```text
Systèmes connus
        │
        ├── affichage complet
        └── voisins directs
                │
                ▼
        systèmes détectés
                │ silhouette / signal
                ▼
Frontière visible de la carte
```

Règles :

- les systèmes connus utilisent leur classe et leur nom ;
- les voisins directs inconnus sont représentés comme signaux détectés ;
- les systèmes situés au-delà de cette frontière ne sont pas instanciés ;
- seules les routes connu↔connu et connu↔détecté sont affichées ;
- le mode debug `F3` permet d'afficher temporairement tout le graphe ;
- le preset actif reste `Low` avec un mesh partagé très simple ;
- le zoom utilise trois niveaux sémantiques :
  - `Overview` : sélection et colonies seulement ;
  - `Regional` : labels des systèmes connus ;
  - `Local` : tous les labels de la frontière visible ;
- `WASD` déplace la caméra et `Q/E` contrôle le zoom ;
- `Tab` sélectionne le prochain système visible et `F` le recentre ;
- `Entrée` ouvre une vue Système légère et `Échap` revient à l'Univers ;
- le retour à l'Univers conserve le focus, le zoom et la sélection.

Les niveaux persistants `Inconnu`, `Détecté`, `Sondé`, `Analysé` et
`Colonisé` seront introduits par `MVP-009`. Pour MVP-007, la détection est une
frontière dérivée du graphe et ne modifie pas encore le format de sauvegarde.


## MVP-008 — Système de départ et planète mère

Les paramètres de nouvelle partie sont maintenant séparés de la génération de
l'univers :

```text
UniverseConfig
    seed / nombre de systèmes
            │
            ▼
UniverseDefinition immuable

StartingScenario
    faction joueur
    système et planète de départ
    colonie et stocks
    bâtiments initiaux
    profil de ressources
    connaissances initiales
            │
            ▼
GameState mutable
```

Configuration MVP :

- système natal : `SystemId(0)` ;
- planète mère : première planète de ce système, `Aster Prime` ;
- habitabilité minimale validée : 80 ;
- faction joueur : `Aster Expedition` ;
- une colonie initiale ;
- stocks initiaux : 600 métal, 300 cristal, 220 carburant, 80 énergie ;
- profil planétaire équilibré : 100/100/100/100 ;
- bâtiments niveau 1 :
  - mine de métal ;
  - extracteur de cristal ;
  - raffinerie de carburant ;
  - centrale énergétique ;
  - entrepôt ;
  - centre de construction ;
- laboratoire et chantier spatial au niveau 0 ;
- seul le système natal est connu ;
- ses voisins apparaissent comme signaux détectés via la frontière MVP-007 ;
- la sélection initiale vise directement la planète mère ;
- la vue initiale du client est la vue Système.

`StartingScenario` est configurable sans modifier la seed, la version de
génération ou le fingerprint de l'univers.

Versions après migration :

- `GAME_STATE_VERSION = 3` ;
- `SAVE_VERSION = 4`.


## MVP-008b + MVP-009 — Caméra souris et connaissance progressive

### Navigation de caméra

Les deux vues stratégiques possèdent désormais leur propre contexte orbital :

- clic droit maintenu : rotation autour du point observé ;
- clic molette maintenu : déplacement du point observé ;
- molette : zoom ;
- `WASD` et `Q/E` restent disponibles comme commandes de secours ;
- les angles, distances et points de focus Univers/Système sont conservés lors
  des transitions ;
- le déplacement souris utilise le delta brut accumulé de la frame, sans être
  multiplié par le delta temporel.

### Connaissance progressive

La liste binaire des systèmes connus est remplacée par deux collections
persistantes :

```text
system_knowledge: Vec<SystemKnowledge>
planet_knowledge: Vec<PlanetKnowledge>
```

Niveaux :

```text
Unknown -> Detected -> Probed -> Analyzed -> Colonized
```

Règles :

- une connaissance ne peut jamais régresser ;
- l'absence d'entrée équivaut à `Unknown` ;
- `Detected` affiche seulement un signal ou une silhouette ;
- `Probed` révèle l'identité et permet d'ouvrir un système ;
- `Analyzed` révèle les détails exacts disponibles ;
- `Colonized` est réservé aux objets possédant une colonie ;
- sonder un système détecte ses planètes et ses voisins directs ;
- les routes sont visibles lorsque leurs deux extrémités sont détectées et
  qu'au moins l'une est sondée ;
- le système natal et la planète mère commencent `Colonized` ;
- les autres planètes du système natal commencent `Detected` ;
- les voisins directs du système natal commencent `Detected`.

Tant que les missions de sonde ne sont pas implémentées, la touche `K` fait
progresser la cible sélectionnée jusqu'à `Analyzed`. Elle ne peut jamais
coloniser.

Dans la vue Système, `Tab` sélectionne successivement les planètes visibles.
Dans la vue Univers, `Tab` continue de parcourir les systèmes visibles.

Versions après migration :

- `GAME_STATE_VERSION = 4` ;
- `SAVE_VERSION = 5`.

La seed, la version de génération et le fingerprint de l'univers ne changent
pas.
