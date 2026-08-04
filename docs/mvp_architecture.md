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

- la seed et l'algorithme sont communs aux presets Test 16, MVP 64 et
  Stress 128 systèmes ;
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
- planète mère : première planète de ce système, `Nacre` ;
- habitabilité minimale validée : 80 ;
- faction joueur : `Consortium` ;
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

Depuis MVP-022, la touche `K` lance une mission de reconnaissance réelle vers le
système détecté sélectionné. La commande de progression directe reste réservée
aux tests de simulation et n'est plus exposée dans l'interface.

Dans la vue Système, `Tab` sélectionne successivement les planètes visibles.
Dans la vue Univers, `Tab` continue de parcourir les systèmes visibles.

Versions après migration :

- `GAME_STATE_VERSION = 4` ;
- `SAVE_VERSION = 5`.

La seed, la version de génération et le fingerprint de l'univers ne changent
pas.


## MVP-010 — Inspecteurs et informations partielles

Le panneau d'informations est désormais piloté par un contenu d'inspection
structuré plutôt que par un simple dump des objets du domaine.

Matrice d'affichage :

```text
Unknown    aucune donnée exploitable
Detected   signal et placeholders
Probed     identité minimale
Analyzed   valeurs exactes disponibles
Colonized  valeurs exactes et données économiques
```

Règles :

- un système détecté ne révèle ni son nom, ni sa classe, ni ses coordonnées
  chiffrées, ni le nombre exact de routes ou de corps ;
- un système sondé révèle son identité et des estimations clairement étiquetées ;
- un système analysé révèle les valeurs exactes disponibles ;
- une planète détectée masque nom, type et habitabilité ;
- une planète sondée révèle son identité, son nom et son type général, sans
  habitabilité ni potentiels ;
- une planète analysée révèle son habitabilité exacte ;
- les stocks, potentiels et bâtiments ne sont affichés que pour une colonie ;
- chaque niveau indique explicitement l'action nécessaire pour progresser ;
- la couleur et le badge de l'inspecteur changent avec le niveau de connaissance ;
- les données absentes du modèle courant, notamment les lunes, sont annoncées
  comme indisponibles au lieu d'être inventées.

La police embarquée par défaut de Bevy est remplacée dans l'interface par
`FontSource::SansSerif`, avec la fonctionnalité `system_font_discovery`.
Les caractères français tels que `é`, `è`, `à`, `É` et `Métal` sont ainsi
rendus par une police installée sur le système.

Cette étape ne modifie ni l'état de simulation, ni les versions de sauvegarde,
ni la génération de l'univers.


## MVP-010-B — Picking, survol et ambiguïtés

La sélection des objets stratégiques utilise un test en espace écran :

```text
position visuelle actuelle
        ↓ projection caméra
position en pixels
        ↓ distance au curseur
candidats dans un rayon constant
```

Le picking s'appuie sur le `Transform` réellement affiché. Il ne lit pas
directement la position métier de l'univers. Le futur mode aplati pourra donc
déplacer visuellement les systèmes sans désynchroniser la sélection.

Comportement :

- clic gauche : sélectionner la meilleure cible ;
- double-clic sur un système accessible : le sélectionner puis l'ouvrir ;
- double-clic sur une planète : recentrer la caméra système ;
- survol : afficher un halo et un tooltip respectant le niveau de connaissance ;
- plusieurs cibles : ouvrir un panneau d'ambiguïté ;
- `Tab` / `Maj+Tab` : parcourir les candidats ambigus ;
- `Entrée` : conserver la cible active et fermer le panneau ;
- `Échap` : fermer le panneau ;
- les contrôles clavier historiques restent disponibles.

Classement déterministe des candidats :

1. distance en pixels au curseur ;
2. priorité visuelle — sélection, colonie, objet connu ;
3. profondeur par rapport à la caméra ;
4. identifiant métier stable pour départager les égalités.

Les systèmes utilisent un rayon de sélection de 18 pixels et les planètes un
rayon de 16 pixels. Les objets inconnus ne sont jamais instanciés et ne peuvent
donc pas devenir candidats.

Les panneaux UI marqués comme bloqueurs empêchent les clics de traverser
l'interface vers la scène 3D.


## MVP-011 — Registre de ressources et énergie

Le modèle économique distingue maintenant deux concepts :

```text
Ressources stockées
├── Métal
├── Cristal
└── Carburant

Énergie
├── capacité produite
├── capacité consommée
└── bilan = production - consommation
```

L'énergie n'est plus un stock dépensable. Une allocation augmente la
consommation sans réduire la production.

`ResourceLedger` possède :

- un stock total ;
- une liste de réservations identifiées ;
- un stock disponible calculé ;
- des opérations atomiques de crédit et débit ;
- `reserve`, `commit` et `release` ;
- une validation des doublons, sur-réservations et identifiants.

Une dépense ou réservation insuffisamment financée ne modifie aucune donnée.
Les réservations sont soustraites du disponible et empêchent les doubles
dépenses avant leur engagement définitif.

`EconomicCost` combine un coût en Métal/Cristal/Carburant et une capacité
énergétique requise. Les catalogues de bâtiments et crafts pourront utiliser
ce format dans les étapes suivantes.

La colonie initiale commence avec :

- 600 Métal ;
- 300 Cristal ;
- 220 Carburant ;
- 80 unités de production énergétique ;
- 30 unités de consommation énergétique.

Le HUD de colonie affiche stock total, disponible, réservé, production,
consommation et bilan énergétique.

Versions après migration :

- `GAME_STATE_VERSION = 5` ;
- `SAVE_VERSION = 6`.

MVP-012 ajoutera la production par tick et les capacités maximales de
stockage. MVP-013 ajoutera les coûts réels du catalogue de bâtiments.


## MVP-012 — Production planétaire et capacités de stockage

La production est exécutée uniquement à partir des ticks stratégiques :

```text
durée réelle
    ↓ StrategicClock
nombre entier de ticks
    ↓
production de chaque colonie
    ↓
crédit plafonné par la capacité
```

Une production possède un reliquat fixe au millième d'unité. Le reliquat est
sauvegardé afin que plusieurs découpages de frames produisent exactement le
même état.

Règles temporaires centralisées, en attendant le catalogue MVP-013 :

- Mine de métal niveau 1 : 2,50 unités/s à potentiel 100 ;
- Extracteur cristallin niveau 1 : 1,25 unité/s à potentiel 100 ;
- Raffinerie niveau 1 : 0,75 unité/s à potentiel 100 ;
- chaque taux est multiplié par le niveau et le potentiel planétaire ;
- capacité de base : 1 000 / 800 / 600 ;
- chaque niveau d'entrepôt ajoute 4 000 / 3 200 / 2 400.

L'énergie suit une règle proportionnelle documentée :

```text
production énergétique effective
    = capacité nominale × potentiel énergétique / 100

si production effective >= consommation
    efficacité des extracteurs = 100 %
sinon
    efficacité = production effective / consommation
```

Tous les extracteurs sont ralentis par le même facteur. Une production
énergétique effective nulle bloque la production mais conserve le reliquat
fractionnaire déjà acquis.

Quand un stockage est plein :

- le stock ne dépasse jamais sa capacité ;
- la production excédentaire est perdue ;
- aucun reliquat caché n'est accumulé pour contourner la saturation.

L'inspecteur de colonie affiche :

- stock total, disponible et réservé ;
- capacité de chaque ressource ;
- production effective par seconde ;
- temps estimé avant saturation ;
- énergie nominale et effective ;
- consommation, efficacité et bilan.

Versions après migration :

- `GAME_STATE_VERSION = 6` ;
- `SAVE_VERSION = 7`.

MVP-013 remplacera les constantes de production et de stockage par les
définitions du catalogue de bâtiments.


## MVP-013 — Catalogue de bâtiments et cadence de production

Les huit bâtiments du scope MVP sont définis dans :

```text
assets/data/buildings.catalog
```

Ce fichier contient pour chaque bâtiment :

- nom ;
- niveau maximal ;
- coût de base ;
- croissance du coût ;
- durée de base ;
- croissance de la durée ;
- consommation énergétique par niveau ;
- effet par niveau ;
- prérequis.

Le chargeur valide au démarrage :

- présence exacte des huit bâtiments ;
- unicité des définitions ;
- niveaux maximaux ;
- coûts et durées ;
- prérequis existants et sans doublon ;
- niveaux de prérequis valides ;
- absence de dépendance sur soi-même ;
- absence de cycle.

La simulation ne contient plus les constantes propres aux mines, à la centrale
ou à l'entrepôt. Production, capacité de stockage et bilan énergétique lisent
le catalogue central. Modifier une valeur du fichier ne nécessite donc aucune
modification des systèmes de simulation.

Le fichier possède une version et un fingerprint. Les sauvegardes refusent un
catalogue incompatible afin d'éviter qu'une partie change silencieusement de
règles économiques.

### Cadence des ressources

L'horloge reste à 10 ticks stratégiques par seconde, mais les stocks ne sont
crédités que toutes les 5 secondes stratégiques :

```text
50 ticks accumulés
    ↓
un crédit de production agrégé
    ↓
un événement ProductionRefreshed
```

Les ticks incomplets de la fenêtre sont sauvegardés par colonie. Les résultats
restent identiques quel que soit le framerate ou la vitesse de jeu.

Cette cadence concerne les stocks de ressources. Les futures constructions,
recherches et missions pourront conserver leur propre fréquence métier.

Versions après migration :

- `GAME_STATE_VERSION = 7` ;
- `SAVE_VERSION = 8`.

Le checkpoint `MVP-013-B` est réservé à une amélioration visuelle complète des
ressources avant la file de construction de MVP-014.


## MVP-013-B — Affichage des ressources et de l’énergie

Un tableau économique compact complète l’inspecteur détaillé lorsqu’une
colonie du joueur est sélectionnée.

Le tableau contient quatre cartes :

```text
Métal | Cristal | Carburant | Énergie
```

Chaque ressource stockée affiche :

- stock total et capacité ;
- quantité disponible ;
- quantité réservée ;
- production effective par seconde ;
- temps avant saturation ;
- jauge de remplissage ;
- delta du dernier crédit de production.

Les états visuels sont :

```text
STABLE
INDISPONIBLE — RÉSERVÉ
PRESQUE PLEIN
PLEIN — PRODUCTION PERDUE
DÉFICIT ÉNERGÉTIQUE
```

La carte Énergie distingue production effective, consommation, capacité libre,
bilan et rendement des extracteurs. L’énergie reste une capacité et n’est
jamais présentée comme un stock.

L’en-tête affiche la colonie active, la prochaine actualisation des stocks et
la cadence de cinq secondes stratégiques. Les deltas ne sont affichés qu’après
un événement `ProductionRefreshed`, pendant une courte durée réelle.

Survoler une carte affiche une aide contextuelle expliquant stocks,
réservations, capacité ou rendement énergétique.

Le tableau est placé entre les panneaux latéraux et reste lisible en
1280×720. Il est masqué lorsqu’aucune colonie du joueur n’est sélectionnée.

Cette étape est purement visuelle :

- aucune modification du domaine ;
- aucune nouvelle commande ;
- aucune modification de sauvegarde ;
- aucune modification des versions d’état ;
- aucune modification de la cadence de simulation.

MVP-014 pourra réutiliser les mêmes cartes pour signaler le coût et les
ressources manquantes d’une construction.


## MVP-014 — File de construction et améliorations

Chaque colonie possède une file de construction séquentielle de cinq ordres
maximum.

Lancer une amélioration :

1. calcule le prochain niveau en tenant compte des ordres déjà en file ;
2. valide le niveau maximal et les prérequis ;
3. vérifie l’énergie projetée ;
4. vérifie les ressources disponibles ;
5. réserve le coût dans `ResourceLedger` ;
6. ajoute l’ordre à la file.

Les ressources restent dans le stock total mais disparaissent du disponible.
Elles sont consommées définitivement lorsque la construction se termine.

La progression utilise exclusivement les ticks stratégiques. Pause, x1, x2 et
x4 produisent donc le même résultat métier. Une file peut terminer plusieurs
ordres dans un même lot de ticks.

À l’achèvement :

- la réservation est engagée ;
- le niveau du bâtiment augmente ;
- le réseau énergétique de la colonie est recalculé ;
- la production et le stockage utilisent automatiquement le nouveau niveau ;
- un événement `ConstructionCompleted` est émis.

L’interface affiche les huit bâtiments en quatre rangées de deux boutons. Un
bouton disponible montre niveau actuel, niveau cible, coût et durée. Un bouton
bloqué explique directement la cause : ressources manquantes, prérequis,
énergie, niveau maximal ou file pleine.

La file, sa progression et les réservations survivent aux sauvegardes.

Versions après migration :

- `GAME_STATE_VERSION = 8` ;
- `SAVE_VERSION = 9`.

### Simplification du tableau de ressources

Les informations de debug ont été retirées :

- dernier crédit ;
- prochain crédit ;
- cadence de rafraîchissement ;
- libellés `STABLE` et `PRESQUE PLEIN`.

La jauge suffit à communiquer le remplissage. Les avertissements textuels sont
conservés uniquement lorsqu’ils ont une conséquence métier :

- `PLEIN — PRODUCTION BLOQUÉE` ;
- `DÉFICIT ÉNERGÉTIQUE`.


## MVP-015 — Écran de gestion planétaire

La gestion économique n’est plus affichée sous forme de panneaux superposés à
la vue stratégique.

Un écran dédié s’ouvre avec la touche `C` ou le bouton `Gestion colonie`. Il
occupe l’espace central entre la barre supérieure et les bords de la fenêtre.
La vue 3D reste en arrière-plan mais la caméra et le picking sont neutralisés
pendant la gestion.

L’écran est organisé en quatre niveaux de lecture :

1. en-tête de la colonie active ;
2. bandeau Métal, Cristal, Carburant et Énergie ;
3. liste compacte des bâtiments ;
4. détail du bâtiment sélectionné et file de construction.

### Sélection de la colonie

Les boutons précédent et suivant parcourent uniquement les colonies du joueur.
Changer de colonie met aussi à jour la sélection stratégique vers sa planète.
L’architecture fonctionne déjà avec une seule colonie et prépare MVP-028.

### Bâtiments

La liste montre uniquement le nom, le niveau actif et le niveau déjà planifié.
Sélectionner un bâtiment ouvre sa fiche détaillée :

- niveau actif ;
- niveau après la file ;
- niveau maximal ;
- effet actuel ;
- effet après la file ;
- effet du prochain niveau ;
- coût ;
- durée catalogue et durée effective ;
- énergie projetée ;
- raison précise d’un blocage.

Le bouton principal lance l’amélioration sans outil de debug.

### File de construction

La colonne droite distingue clairement :

- ordre en cours ;
- progression ;
- temps restant ;
- coût réservé ;
- ordres en attente ;
- capacité utilisée de la file.

Les refus, ajouts et achèvements apparaissent comme un message non bloquant au
bas de l’écran.

### HUD stratégique

L’inspecteur de planète conserve seulement un résumé économique et indique la
touche `C`. Les informations détaillées ne sont plus répétées dans plusieurs
panneaux.

Cette étape est client-only :

- aucune modification du domaine ;
- aucune modification de la simulation ;
- aucune migration de sauvegarde ;
- `GAME_STATE_VERSION` reste 8 ;
- `SAVE_VERSION` reste 9.


## MVP-016 — Recherche et arbre technologique minimal

La recherche est une progression globale au joueur. Tous les Instituts d'analyse des
colonies du joueur contribuent à une seule production scientifique et à une
file commune de six projets maximum.

### Technologies

L'arbre minimal contient :

1. Détection longue portée ;
2. Propulsion avancée ;
3. Soutes agrandies ;
4. Extraction automatisée ;
5. Analyse planétaire ;
6. Colonisation avancée.

Les dépendances sont déterministes :

```text
Détection longue portée
├── Propulsion avancée
│   └── Soutes agrandies
│       ├── Extraction automatisée
│       └── Colonisation avancée
└── Analyse planétaire
    └── Colonisation avancée
```

Chaque définition possède un nom, une description, un coût en milli-points,
des prérequis et une capacité débloquée. Les capacités sont des clés métier
stables qui seront consommées par les missions, crafts et commandes des
checkpoints suivants.

### Production scientifique

Le Laboratoire du catalogue produit des milli-points par tick et par niveau.
La production globale additionne les niveaux actifs de Laboratoire de toutes
les colonies du joueur.

Sans Laboratoire actif, aucune technologie ne peut être ajoutée à la file.
Une amélioration de Laboratoire agit dès le tick stratégique où sa construction
se termine.

La simulation traite production, construction et recherche tick par tick à
l'intérieur d'un lot de temps. Le résultat reste donc identique quel que soit
le découpage des frames, y compris lorsqu'un Laboratoire termine pendant le
lot.

### File globale

Une technologie peut être ajoutée lorsque :

- un Laboratoire produit des points ;
- ses prérequis sont acquis ou placés avant elle dans la file ;
- elle n'est ni acquise ni déjà planifiée ;
- la file n'est pas pleine.

Une technologie terminée ne peut pas être relancée. Les points excédentaires
d'un lot passent au projet suivant.

### Interface

La touche `T` et le bouton `Recherche` ouvrent un écran dédié au-dessus de la
vue stratégique. Il présente :

- les six technologies et leur état ;
- les prérequis et le déblocage ;
- le coût scientifique ;
- la durée estimée avec la production actuelle ;
- le projet actif, sa progression et les projets en attente ;
- les refus et achèvements sous forme de messages non bloquants.

L'écran de recherche et l'écran de gestion planétaire sont mutuellement
exclusifs. La caméra stratégique est verrouillée lorsqu'un de ces écrans est
ouvert.

### Persistance

Le snapshot conserve l'ensemble acquis, la file et la progression du projet
actif. Il stocke également la version et l'empreinte du catalogue
technologique.

Versions après migration :

- `GAME_STATE_VERSION = 9` ;
- `SAVE_VERSION = 10` ;
- `RESEARCH_CATALOG_VERSION = 1`.


## MVP-016-B — Ruleset économique externe

Le contenu économique est chargé au démarrage depuis
`assets/rulesets/default/`. Les bâtiments, technologies, valeurs économiques
et conditions initiales ne sont plus compilés dans le binaire.

Les identifiants de contenu sont des clés textuelles stables. Le chargeur
compile les fichiers RON vers les types sûrs de la simulation et refuse les
références inconnues, doublons, cycles et valeurs invalides.

La sauvegarde conserve l'identité, le schéma, la version de contenu et
l'empreinte structurelle du ruleset. Un changement de texte ne modifie pas
l'empreinte.

Versions après migration :

- `GAME_STATE_VERSION = 10` ;
- `SAVE_VERSION = 11` ;
- `RULESET_SCHEMA_VERSION = 1`.


## MVP-017 — File générique de craft

Chaque colonie possède une file de fabrication séquentielle et un inventaire
de produits terminés. Les définitions sont chargées depuis
`craftables.ron` : identifiant, textes, catégorie, coût, durée, quantité,
prérequis et capacités.

### Réservation et progression

L'ajout d'un ordre valide les bâtiments, les technologies, la capacité de la
file et les ressources. Le coût est réservé atomiquement dès l'ajout, puis
consommé uniquement à l'achèvement.

Le Chantier naval produit des milli-points de travail à chaque tick
stratégique. La durée configurée correspond au niveau minimal requis ; les
niveaux supplémentaires accélèrent donc la fabrication sans changer le
contenu du catalogue. Les points excédentaires passent à l'ordre suivant.

### Interface et persistance

La touche `Y` ouvre l'écran Chantier. Il présente le catalogue, les prérequis,
le coût, la durée estimée, la file active et l'inventaire de la colonie.
L'écran est mutuellement exclusif avec la gestion planétaire et la recherche.

La sauvegarde conserve les ordres, leur progression, leurs réservations et
l'inventaire. La reconstruction vérifie chaque identifiant et les données
structurelles nécessaires aux ordres en cours.

Versions après migration :

- `GAME_STATE_VERSION = 11` ;
- `SAVE_VERSION = 12` ;
- `RULESET_SCHEMA_VERSION = 2` ;
- `CRAFTABLE_CATALOG_VERSION = 1`.


## MVP-018 — Propriété et factions

Tout objet possédable expose désormais un `Owner`, indépendant de son type
concret. Une colonie utilise `Owner::Faction(FactionId)` ; `Owner::Unowned`
réserve le cas des futurs objets sans contrôle territorial.

Les contrôles de gestion passent par une autorisation unique qui vérifie
l'existence et l'activation de la faction émettrice, puis sa correspondance
avec le propriétaire. Construction, craft et recherche reçoivent explicitement
la faction agissante. L'enveloppe de commande avec émetteur reste le périmètre
du MVP-019.

Les factions sont chargées depuis `factions.ron`. Le ruleset par défaut contient
une faction joueur active, une faction neutre inactive et une future faction IA
inactive. Ces factions dormantes sont persistées mais n'exécutent aucune boucle
d'action.

Versions après migration :

- `GAME_STATE_VERSION = 12` ;
- `SAVE_VERSION = 13` ;
- `RULESET_SCHEMA_VERSION = 3`.


## MVP-019 — Commandes génériques et relations dormantes

Une action métier est désormais transportée par `GameCommand`, qui contient
toujours la faction émettrice, le tick d'émission et un `GameAction`. La
simulation refuse explicitement une faction inconnue ou inactive ainsi qu'une
commande émise pour un autre tick. Le joueur, un replay et une future IA
peuvent donc produire le même contrat d'entrée.

Les sorties utilisent une enveloppe `GameEvent` avec faction destinataire,
tick d'occurrence et `GameEventKind`. L'interface Bevy ne modifie toujours
jamais directement les files ou les stocks.

Les relations `Unknown`, `Neutral`, `Hostile` et `Allied` sont symétriques,
triées et déterministes. Leur état initial vient de `factions.ron`, leur API
est disponible pour les futures missions et le contrôle territorial, et leur
état est sauvegardé. Elles ne changent encore ni les autorisations de gestion
ni la boucle du joueur. Aucune source de commandes IA n'est exécutée.

Versions après migration :

- `GAME_STATE_VERSION = 13` ;
- `SAVE_VERSION = 14` ;
- `RULESET_SCHEMA_VERSION = 4`.


## MVP-020 — Flottes, vaisseaux et capacités

Les fabrications de vaisseaux décrivent maintenant dans `craftables.ron` leur
classe, leur vitesse de croisière, leur portée en sauts, leur capacité cargo et
leur consommation de carburant par saut. Les valeurs restent configurables,
tandis que les règles d'agrégation appartiennent à la simulation.

Une flotte possède un `FleetId` stable, un `Owner`, une localisation, une
composition générique, une cargaison et une affectation éventuelle à une
mission. Une flotte mixte utilise la vitesse et la portée de son vaisseau le
plus contraignant ; les capacités cargo et consommations sont additionnées.

Former une flotte est une commande atomique :

1. valider la faction et la colonie ;
2. valider chaque classe et quantité demandée ;
3. vérifier l'inventaire disponible au sol ;
4. retirer les vaisseaux de cet inventaire ;
5. créer une seule flotte possédée par la faction.

Une seconde formation ne peut donc pas réutiliser les mêmes unités. Les flottes,
leur composition, leur localisation, leur cargaison, leur affectation et le
prochain identifiant sont sauvegardés. Le moteur de trajet et les missions
restent hors périmètre jusqu'à MVP-021.

Versions après migration :

- `GAME_STATE_VERSION = 14` ;
- `SAVE_VERSION = 15` ;
- `RULESET_SCHEMA_VERSION = 5` ;
- `CRAFTABLE_CATALOG_VERSION = 2`.


## MVP-020-B — Bible d'univers et nomenclature V1

Le ruleset `default` suit désormais une référence éditoriale unique :
`docs/universe_bible.md`. Elle fixe le ton de science-fiction de frontière, les
familles lexicales et les noms canoniques des factions, infrastructures,
recherches et premiers vaisseaux.

Les identifiants techniques restent inchangés. Les nouveaux libellés du ruleset
n'altèrent donc ni les coûts, ni les capacités, ni son empreinte structurelle.
`content_version` passe à 6.

Le générateur utilise seize noms de systèmes fixes pour le preset Test et les
réemploie avec un suffixe de cycle aux échelles supérieures. Les planètes non
baptisées suivent la convention astronomique `Système b`,
`Système c`, tandis que la planète mère devient `Nacre` et la colonie initiale
`Port-Sillage`. Les tirages aléatoires historiques sont consommés à l'identique
afin que cette passe éditoriale ne modifie pas les propriétés physiques de
l'univers.

Les noms faisant partie du fingerprint de l'univers, cette modification
volontaire porte `GENERATION_VERSION` à 3 et renouvelle le fingerprint de
référence. Les anciennes sauvegardes de développement sont incompatibles.
`GAME_STATE_VERSION`, `SAVE_VERSION` et `RULESET_SCHEMA_VERSION` restent
respectivement à 14, 15 et 5.


## MVP-021 — Moteur de trajet et machine d'état des missions

Le moteur commun accepte les types `Probe`, `Transport`, `Harvest` et
`Colonize`, sans encore exécuter leur résolution propre. Un ordre contient la
flotte, l'origine, la cible, le type de mission et le tick de départ.

La planification utilise un BFS déterministe limité aux routes actuellement
accessibles dans l’état de partie. Elle refuse une cible cachée, une route absente, une
portée insuffisante, une flotte étrangère ou déjà affectée. La durée aller est
calculée par `ceil(16_000 × sauts / vitesse_de_croisière)`. Le coût de
carburant couvre l'aller-retour.

Au lancement, le carburant est réservé atomiquement dans la colonie d'origine
et la flotte est verrouillée. Une annulation pendant la préparation libère les
deux. Au départ, la réservation est débitée puis la mission suit exclusivement
les ticks stratégiques :

`Preparation → Outbound → OnSite → Returning → Completed`

Les transitions `Cancelled` et `Failed` sont terminales et les transitions
invalides sont refusées explicitement. Chaque transition produit un événement ;
la fin ou l'annulation produit aussi un rapport conservé dans l'état mutable.
La route, les échéances, la phase, la réservation, les missions et leurs
rapports sont sauvegardés. Une reprise termine donc au même tick que
l'exécution originale.

La reconnaissance, les transports de ressources, la récolte et la colonisation
restent hors de cette étape : leurs effets se brancheront sur la phase
`OnSite` dans les checkpoints suivants.

Versions après migration :

- `GAME_STATE_VERSION = 15` ;
- `SAVE_VERSION = 16` ;
- `RULESET_SCHEMA_VERSION = 5` (inchangé).


## MVP-022 — Sonde et mission de reconnaissance

La reconnaissance branche le premier effet métier concret sur la phase
`OnSite`. Une mission `Probe` exige une Sonde — Œil et une cible actuellement
`Detected`. Les cibles inconnues, déjà sondées ou sans route accessible sont
refusées explicitement.

La commande joueur `LaunchProbe` est atomique. Elle réutilise d'abord la flotte
de sondes inactive de plus petit identifiant qui est amarrée à la colonie
d'origine. À défaut, elle forme une flotte d'une Sonde — Œil depuis
l'inventaire du chantier. Une validation échouée ne retire aucune sonde et ne
crée aucune flotte orpheline.

À l'arrivée, le système passe immédiatement à `Probed`. Les changements de
connaissance et un `MissionResolution` sont émis sans rechargement. Le résultat
persisté indique la cible, les niveaux avant/après et le nombre de systèmes et
planètes révélés. Le rapport final reprend ce résultat après le retour, puis la
flotte est de nouveau amarrée et réutilisable.

Le HUD conserve une présentation minimale jusqu'à MVP-030 : il affiche la
première mission active, sa cible masquée tant qu'elle n'est pas sondée, sa
phase et le temps jusqu'à la prochaine transition. La touche `K` lance la
reconnaissance vers le système détecté sélectionné.

Versions après migration :

- `GAME_STATE_VERSION = 16` ;
- `SAVE_VERSION = 17` ;
- `RULESET_SCHEMA_VERSION = 5` (inchangé).


## MVP-023 — Frontière de découverte progressive

Une reconnaissance réussie produit désormais un `DiscoveryFrontier`
déterministe. Le résultat distingue les changements de connaissance généraux,
les systèmes voisins qui passent précisément de `Unknown` à `Detected` et les
routes directes de la cible qui deviennent visibles pendant cette opération.

L'ouverture est strictement limitée à un anneau du graphe : les nouveaux
signaux restent `Detected`, leurs identités et détails demeurent masqués, et
leurs propres voisins ne sont pas propagés avant une reconnaissance ultérieure.
Une seconde application sur le même système ne produit aucun changement.

`ProbeMissionResult` conserve les nombres de nouveaux signaux et de routes
révélées. Ces métriques suivent la mission et son rapport dans la sauvegarde ;
le client les affiche dans la notification de résolution et reconstruit la
carte à partir des événements de connaissance.

Versions après migration :

- `GAME_STATE_VERSION = 17` ;
- `SAVE_VERSION = 18` ;
- `RULESET_SCHEMA_VERSION = 5` (inchangé).


## MVP-023-B — Secteurs galactiques déterministes

`UniverseDefinition` contient désormais des `SectorDefinition` immuables. Un
secteur possède un `SectorId`, un nom, un centre géométrique, la liste triée de
ses systèmes et les routes passerelles qui le relient aux autres secteurs.
`UniverseRepository` indexe ces données sans dépendance à Bevy et refuse les
appartenances manquantes, multiples ou incohérentes.

La génération choisit environ `sqrt(nombre de systèmes)` centres, répartis par
éloignement et départagés avec la seed. Une propagation multi-source sur le
graphe affecte ensuite chaque système exactement une fois. Chaque secteur est
donc connexe, reproductible et cohérent avec les routes. Le preset Test de
16 systèmes produit 4 secteurs ; l'échelle MVP active de 64 systèmes en
produit 8, dans la cible de 6 à 10.

En vue globale, le client affiche les noms sectoriels uniquement au niveau
`Overview`. Un secteur n'apparaît que si au moins un de ses systèmes est connu,
et son repère est calculé depuis les seuls membres connus : la position ou le
nombre des systèmes inconnus ne fuitent pas. Les routes passerelles déjà
visibles utilisent une couleur distincte.

Les secteurs font partie de la définition générée et de son fingerprint.
`GENERATION_VERSION` passe donc à 4 et le fingerprint de référence change. Les
sauvegardes de développement précédentes sont incompatibles. Les contrats
mutables restent inchangés :

- `GAME_STATE_VERSION = 17` ;
- `SAVE_VERSION = 18` ;
- `RULESET_SCHEMA_VERSION = 5`.


## MVP-023-C — Projection aplatie et presets d'échelle

`UniverseScalePreset` expose trois configurations déterministes partageant la
seed MVP :

- `Test` : 16 systèmes pour les tests et itérations rapides ;
- `Mvp` : 64 systèmes, configuration jouable et valeur par défaut ;
- `Stress` : 128 systèmes, réservé aux mesures.

Le client accepte `--scale test|mvp|stress` uniquement au démarrage. Le nombre
de systèmes reste enregistré dans `UniverseReference`, donc une sauvegarde
reconstruit toujours son univers d'origine indépendamment du preset choisi lors
d'un lancement ultérieur.

La touche `P` alterne entre la disposition 3D et une projection sur le plan
galactique. `projection_mix` progresse sur 0,65 seconde et atteint exactement
`0` ou `1`, sans accumulation de dérive. Cette valeur commune positionne les
systèmes, labels, halos, routes et repères sectoriels avant le calcul du
picking écran. Les coordonnées `WorldPosition`, le graphe, les distances et les
durées de mission ne sont jamais modifiés.

Le preset graphique reste `Low`. Un test de budget vérifie que la scène MVP
reste à 64 systèmes, 8 secteurs et au plus trois routes par système généré.
Le changement de preset ne modifiant pas l'algorithme pour une taille donnée,
`GENERATION_VERSION = 4` reste inchangée.

## MVP-023-D — Socle visuel Univers et Système

Ce checkpoint reste entièrement dans `galactic_client`. Il ne modifie ni
`UniverseDefinition`, ni `GameState`, ni les versions de génération, de
sauvegarde ou de ruleset.

La vue Univers sépare désormais trois niveaux de présentation :

1. `Known` : identité et classe stellaire révélées ;
2. `Detected` : signal sélectionnable ouvert par la frontière de découverte ;
3. `Observed` : étoile inconnue proche, visible mais non sélectionnable.

`Observed` n'est pas un niveau de `KnowledgeLevel`. La bulle initiale retient
au plus 14 systèmes proches de `MVP_HOME_SYSTEM_ID`, de façon déterministe.
Elle n'ajoute aucune entrée à `system_knowledge` et `visible_routes()` continue
donc d'ignorer ses systèmes. Le mode debug peut toujours afficher et
sélectionner le graphe complet explicitement.

La position spatiale affichée applique un facteur vertical de `3,4` à l'axe Y.
La projection aplatie conserve exactement Y=0 et l'interpolation existante
reste commune aux étoiles, labels, halos, routes et picking. Les routes sont
découpées en tirets ; leur couleur et leur cadence distinguent les liaisons
connues, partielles et inter-secteurs.

La vue Système utilise un seul mesh UV pour les planètes et six textures
procédurales déterministes de 64×32 pixels. Les matériaux, halos, atmosphères et
anneaux restent partagés. La rotation axiale et l'orbite sont uniquement
visuelles, lentes et calculées avec le temps Bevy : elles ne changent ni la
position métier, ni les durées de mission. Un corps `Detected` conserve le mesh
simple et le matériau de silhouette afin de ne pas révéler son type.

## MVP-023-E — Sondes planétaires et trajectoires visibles

`MissionTarget` distingue désormais une cible `System(SystemId)` d'une cible
`Planet { system_id, planet_id }`. Le planificateur valide l'appartenance de la
planète à son système et autorise une route locale réduite au système d'origine.
Le temps d'un trajet interstellaire reste calculé par saut connu ; le temps
local ajoute un travail orbital déterministe croissant avec l'index de la
planète. Le carburant aller-retour comprend au moins une étape pour une cible
planétaire locale.

À l'arrivée d'une reconnaissance planétaire, seule la cible progresse de
`Detected` à `Probed`. Ses planètes voisines conservent leur niveau courant et
aucune donnée d'habitabilité n'est révélée avant l'analyse planétaire. Les
missions vers un système conservent la découverte de frontière introduite en
MVP-023.

Le client nomme provisoirement les corps détectés selon leur orbite
(`Nom-du-système I`, `II`, etc.). La touche `K` construit la même commande
atomique de reconnaissance pour le système ou la planète sélectionnés.
Un repère cyan interpole sa position depuis les bornes de phase persistées :
sur les orbites dans la vue Système pour une cible locale, et sur la liste des
systèmes de la route dans la vue Univers pour une mission interstellaire.
L'aller progresse de 0 à 1 et le retour de 1 à 0 exclusivement selon les ticks
stratégiques ; le rendu n'ajoute aucun asset ni état métier par image.

La forme persistée des missions change :

- `GAME_STATE_VERSION = 18` ;
- `SAVE_VERSION = 19` ;
- `RULESET_SCHEMA_VERSION = 5` (inchangé) ;
- `GENERATION_VERSION = 4` (inchangé).

## MVP-024 — Analyse planétaire et règles de colonisabilité

Ce checkpoint introduit le modèle de rapport planétaire exact : une planète ne
peut être analysée qu'après avoir atteint `Probed` et après le déblocage
`AnalyzePlanets`. Depuis MVP-030-A5, le chemin joueur n'utilise plus d'action
instantanée : `MissionKind::Analyze` produit le `PlanetAnalysisReport` au
retour du Satellite — Veilleur, puis élève la connaissance à `Analyzed` et
l'ajoute à `GameState::planet_analysis_reports`.

Le rapport est un instantané persistant distinct de la définition réelle de
l'univers. Il contient l'environnement, l'habitabilité exacte, quatre
potentiels économiques et un ensemble compact de contraintes d'installation.
La restauration valide l'unicité, la cible, le niveau de connaissance,
l'horodatage et la concordance du rapport avec la planète et le ruleset actifs.

`planetary_analysis.ron` pilote le seuil minimal d'habitabilité, la limite de
colonies, la cargaison de fondation et les profils de chaque type de planète.
Une variation stable dérivée du `PlanetId` différencie les mondes de même type
sans état aléatoire mutable.

La fonction `assess_planet_colonizability` ne modifie aucun état. Elle retourne
tous les motifs de blocage présents : analyse insuffisante, rapport absent,
monde déjà colonisé, technologie manquante, environnement incompatible,
habitabilité trop faible, route connue absente, limite atteinte ou cargaison
insuffisante. Le client affiche ces motifs et le coût exact dans l'inspecteur ;
la création effective d'une colonie reste hors périmètre de ce checkpoint.

Versions après ce checkpoint :

- `GAME_STATE_VERSION = 19` ;
- `SAVE_VERSION = 20` ;
- `RULESET_SCHEMA_VERSION = 6`.

## MVP-025 — Occupants, forces et défenses planétaires

Chaque planète possède désormais un `PlanetaryPresence` réel, initialisé de
façon déterministe depuis la graine de l'univers et son `PlanetId`. Cette
présence regroupe une faction occupante éventuelle, une population et des
forces stationnées au sol ou en orbite. Les profils, leurs poids et les
statistiques de chaque force sont définis dans `planetary_presence.ron`.

L'état réel reste séparé du dernier renseignement du joueur. Une sonde produit
un rapport `Contact` limité à un signal d'occupation. L'analyse élève ce rapport
à `Surveyed` et expose seulement des fourchettes quantifiées, arrondies selon
la précision de chaque définition. Les colonies du joueur disposent d'un
rapport `Exact`. Le client ne lit que ce rapport pour afficher une présence,
une relation diplomatique, une population et des forces estimées.

Les pertes futures passent par `apply_planetary_force_losses`. Cette mutation
est atomique, refuse les quantités invalides, incrémente la révision de la
présence et ne rafraîchit pas implicitement le renseignement connu. Elle
préserve ainsi la distinction entre réalité et information lors du futur
combat.

Une planète occupée par une autre faction ajoute un motif explicite au verdict
de colonisabilité. L'attaque et la résolution du combat restent hors périmètre
de ce checkpoint.

Versions après ce checkpoint :

- `GAME_STATE_VERSION = 20` ;
- `SAVE_VERSION = 21` ;
- `RULESET_SCHEMA_VERSION = 7`.

## MVP-025-B — Attaques, combat V1 et rapports

`MissionKind::Attack` réutilise le planificateur et la machine d'état du moteur
de trajet. Au lancement, la simulation fige un `AttackMissionCommitment`
contenant la flotte attaquante, la présence défensive réelle, leurs révisions
et une graine dérivée de l'univers, de la mission et de la planète. La
résolution ne consulte donc aucune valeur mutable cachée.

`resolve_combat` est une fonction pure paramétrée par `combat.ron`. Elle simule
des rounds simultanés bornés, puis produit vainqueur, pertes, survivants,
dommages, récupération et changement de contrôle. L'application travaille sur
une copie de l'état, revalide propriétaire, forces et flotte, puis publie
atomiquement le résultat. Un rapport déjà présent interdit toute seconde
application.

Le dernier renseignement estimatif reste la seule source de l'interface avant
l'arrivée. Après une résolution effective, un rapport de combat persistant
rend les valeurs engagées consultables et le renseignement local devient
exact. Une cible ayant changé pendant le trajet produit au contraire un
rapport d'invalidation sans révéler l'ancien instantané défensif.

Pour garantir un adversaire de test sans supprimer les mondes vides, les règles
de présence imposent une patrouille hostile sur une planète de chaque système
directement voisin du départ. Le reste de la génération garde ses profils
pondérés.

Versions après ce checkpoint :

- `GAME_STATE_VERSION = 21` ;
- `SAVE_VERSION = 22` ;
- `RULESET_SCHEMA_VERSION = 8`.

## MVP-026 — Mission de colonisation et fondation préparée

`MissionKind::Colonize` exige une cible planétaire analysée, déclarée éligible
par `assess_planet_colonizability`, et une flotte composée exactement d'une
Arche coloniale — Essor. Le raccourci joueur forme cette flotte depuis l'inventaire si
aucune arche inactive n'est déjà amarrée.

Le carburant et le chargement suivent deux engagements distincts. Le carburant
est consommé au départ comme pour les autres missions. Le coût de fondation
reste réservé dans la colonie d'origine pendant le trajet : il est validé et
consommé seulement lorsque la planète reste libre, habitable, technologiquement
accessible et sous la limite de colonies à l'arrivée. Un refus conserve
l'Arche, puis libère le chargement à son retour.

Un succès consomme l'Arche et crée un `ColonyFoundation` persistant contenant
la mission source, le propriétaire, la cible, le chargement et le tick de
préparation. Cette fondation verrouille la planète contre une seconde mission
mais ne crée ni `ColonyId`, ni économie, ni bâtiments : MVP-027 la transformera
en colonie jouable. Le chargeur vérifie l'unicité, la mission de succès, la
cible, le propriétaire, le chargement et l'horodatage.

Versions après ce checkpoint :

- `GAME_STATE_VERSION = 22` ;
- `SAVE_VERSION = 23` ;
- `RULESET_SCHEMA_VERSION = 9`.

## MVP-027 — Initialisation d'une colonie jouable

À l'arrivée réussie, la fondation est conservée comme provenance immuable et
produit immédiatement un `ColonyState`. `GameState::next_colony_id` alloue
l'identité stable ; la colonie référence sa mission fondatrice afin que le
chargement puisse vérifier la chaîne mission → fondation → colonie sans
dépendre d'une `Entity` Bevy.

Le stock initial est exactement le chargement consommé par la mission. Les
bâtiments et la population initiale viennent de `planetary_analysis.ron`,
l'énergie et la capacité sont dérivées du catalogue de bâtiments, et le profil
de production vient du rapport d'analyse persistant. Aucun contenu économique
initial n'est dupliqué dans le moteur Rust.

L'initialisation met atomiquement à jour :

- la présence réelle et le contrôle territorial ;
- les connaissances planète et système au niveau `Colonized` ;
- le renseignement local exact ;
- les files de construction et de craft indépendantes ;
- l'événement `ColonyEstablished`.

La fondation historique n'est plus comptée séparément lorsque sa colonie
existe. L'écran de gestion sait déjà énumérer les colonies possédées ; le
travail de navigation et de synthèse explicitement multi-colonies reste dans
MVP-028.

Versions après ce checkpoint :

- `GAME_STATE_VERSION = 23` ;
- `SAVE_VERSION = 24` ;
- `RULESET_SCHEMA_VERSION = 10`.

## MVP-028 — Gestion multi-colonies

`GameState::active_colony_id` est l'unique sélection de colonie active pour le
joueur. Elle est modifiée par `GameAction::SelectColony`, validée contre le
propriétaire de la colonie et sauvegardée avec le reste de l'état. La commande
met aussi la sélection stratégique sur la planète concernée, mais ne touche à
aucun état économique.

Les écrans de gestion planétaire et de chantier orbital lisent la même
sélection. Leur navigation parcourt `GameState::player_colony_ids()`, triée par
identifiant stable, et toutes les commandes locales conservent un `ColonyId`
explicite. Les lancements de mission utilisent également la colonie active
comme origine au lieu de revenir implicitement à la colonie mère.

Stocks, énergie, bâtiments, files de construction, chantier et inventaire
restent portés par chaque `ColonyState`. La recherche reste portée une seule
fois par `GameState::research` ; sa cadence additionne les laboratoires de
toutes les colonies contrôlées et ignore les colonies étrangères.

Versions après ce checkpoint :

- `GAME_STATE_VERSION = 24` ;
- `SAVE_VERSION = 25` ;
- `RULESET_SCHEMA_VERSION` reste 10.

## MVP-029 — Transport déterministe entre colonies

`GameAction::LaunchTransport` porte explicitement les identifiants des colonies
d'origine et de destination ainsi que les trois quantités de cargaison. Le
moteur choisit de façon stable le cargo léger amarré de plus petit identifiant,
ou forme la flotte minimale depuis l'inventaire. Le lancement est atomique :
propriété, route, stock, capacité et absence de cargaison préexistante sont
validés avant toute mutation.

Le carburant et la cargaison utilisent deux réservations distinctes. Au départ,
elles sont consommées et la cargaison devient un état réel de la flotte. À
l'arrivée, `TransportMissionState` enregistre la quantité livrée et le statut de
la destination. Le stockage applique un crédit plafonné ; tout reliquat repart
avec la flotte puis revient à l'origine. Si ce dernier stockage est lui aussi
plein, le reliquat reste visible dans la flotte amarrée au lieu d'être supprimé.

L'annulation avant départ libère les deux réservations. Une colonie de
destination supprimée ou devenue étrangère refuse toute livraison et provoque
un retour avec un rapport d'échec explicite. L'ordre, la phase, les réservations,
la cargaison de flotte et `TransportMissionResult` sont tous inclus dans
`GameState`, donc dans la sauvegarde et dans sa validation de cohérence.

Versions après ce checkpoint :

- `GAME_STATE_VERSION = 25` ;
- `SAVE_VERSION = 26` ;
- `RULESET_SCHEMA_VERSION` reste 10.

## MVP-029-B — Récolte distante déterministe

Le ruleset externe ajoute `extraction.ron` et un profil par `PlanetKind`.
Chaque profil fixe la ressource, la réserve initiale du site, le rendement
maximal d'une mission et le nombre de ticks passés au chargement. La génération
crée un `ExtractionSiteId` stable dérivé du `PlanetId`, tandis que
`ExtractionSiteState` porte uniquement la réserve mutable et l'éventuelle
mission qui la réserve.

`GameAction::LaunchHarvest` identifie la colonie d'origine et le site. Le moteur
exige une planète analysée, `RemoteExtraction`, une route valide et un cargo
vide disponible ou formable. Le site est réservé atomiquement au lancement.
À la fin du chargement, le prélèvement est borné simultanément par le rendement,
la réserve restante et la capacité de la flotte ; la réserve est débitée une
seule fois et devient une cargaison réelle pendant le retour.

Au retour, le stockage de la colonie reçoit ce qu'il peut accepter et le
reliquat demeure dans la flotte amarrée. Annulation, épuisement, résultat et
rapport sont explicites. La validation croise les réservations avec les
missions actives, et la sauvegarde conserve site, phase et cargaison afin qu'une
reprise produise le même état final qu'une exécution continue.

Versions après ce checkpoint :

- `GAME_STATE_VERSION = 26` ;
- `SAVE_VERSION = 27` ;
- `RULESET_SCHEMA_VERSION = 11`.

## MVP-030 — HUD des flottes et missions

Le module `fleet_ui` est le point d'entrée joueur pour la gestion des groupes de
vaisseaux et les missions. Il s'appuie sur les commandes exposées par
`galactic_sim` (`FormFleet`, `RenameFleet`, `DisbandFleet`, `LaunchMission`,
`CancelMission`) et conserve les noms de flotte dans l'état sauvegardé.

L'écran s'ouvre avec `V` et se ferme avec `V` ou `Échap`, selon le même
mécanisme d'exclusion mutuelle que la gestion planétaire, la recherche et le
chantier orbital. Il comporte quatre onglets. « Flottes » liste les flottes
contrôlées par le joueur (nom, composition, localisation, cargaison,
affectation), permet de sélectionner un groupement, de le renommer et de
dissoudre une flotte uniquement si elle est inactive, amarrée et sans cargaison.
Le compositeur incrémente chaque type de vaisseau du catalogue depuis le stock
au dock jusqu'à une nouvelle `FleetComposition`, formée par `FormFleet`.

« Lancer une mission » suit l'ordre flotte -> type de mission -> destination ->
paramètres -> validation. Le choix de type est filtré par la composition de la
flotte sélectionnée : une Sonde — Œil lance `Probe`, un Satellite — Veilleur
lance `Analyze`, les cargos vides lancent `Transport` ou `Harvest`, une Arche
Pionnière lance `Colonize` et les flottes militaires compatibles lancent
`Attack`. Les destinations sont ensuite calculées dynamiquement selon les règles
déjà appliquées par le moteur, puis revalidées par `plan_mission` pour afficher
route, durée, coût carburant et erreurs avant validation.

« Missions actives » affiche jusqu'à seize missions non terminées avec leur
cible, leur phase et le temps restant avant leur prochaine étape, calculé avec
la même fonction que la ligne de statut existante. Sélectionner « Origine » ou
« Cible » sur une ligne envoie `SelectSystem` ou `SelectPlanet`, réutilisant la
sélection stratégique déjà pilotée par le reste de l'interface. « Annuler »
n'apparaît que pour une mission encore en préparation. « Rapports » réutilise
telle quelle la description textuelle déjà produite pour le flux d'événements
afin de résumer chaque résolution de mission passée.

Toutes les cibles proposées respectent les niveaux de connaissance existants :
une planète seulement détectée n'affiche jamais son nom réel avant d'avoir été
sondée. Les refus (composition invalide, ressources insuffisantes, portée,
cible non éligible, annulation impossible) réutilisent les messages français
déjà validés pour les raccourcis clavier, désormais partagés via des fonctions
`pub(crate)`.

## MVP-030-B — Navigation galactique avancée

Ce checkpoint reste entièrement dans `galactic_client` : aucun changement à
`galactic_sim` ni `galactic_domain`. Il ajoute un historique de navigation, une
recherche globale, des filtres, un budget de labels anti-encombrement, une
agrégation des missions lointaines et un surlignage de la mission
sélectionnée, tous strictement dérivés des niveaux de connaissance déjà
appliqués ailleurs dans le jeu.

`NavigationHistory` empile des `ViewSnapshot` (mode, focus, distance, yaw,
pitch des deux vues, sélection) à chaque transition de navigation initiée par
le joueur — entrée/sortie de système, résultat de recherche, clic de fil
d'Ariane. `Retour arrière` et `]` restaurent respectivement la pile précédente
et suivante en réappliquant `SelectSystem`/`SelectPlanet`, jamais en écrivant
directement `GameState.selected`. Une nouvelle navigation vide la pile avant.

Le fil d'Ariane (Galaxie › secteur › système › planète) est entièrement dérivé
chaque frame depuis le mode de vue courant et la sélection ; le secteur reste
un concept de présentation, cliquer dessus recentre la vue Univers en zoom
Overview sans introduire de nouveau mode caméra.

La recherche s'ouvre avec `/` et reconstruit son index à la volée à chaque
frame où le panneau est ouvert, en filtrant systématiquement par le niveau de
connaissance avant tout autre critère : un système non sondé ou une planète
dont l'identité n'est pas révélée n'apparaissent jamais, quelle que soit la
requête. Les flottes du joueur utilisent leur nom persistant dans les libellés
de navigation, tandis que les missions restent indexées avec leur type, leur
cible et leur identifiant. Les filtres (`B`) composent avec cette même porte de
connaissance : ils ne peuvent que restreindre les résultats, jamais révéler un
objet inconnu.

Le budget de labels attribue un score de priorité par système (sélection,
colonie du joueur, distance au centre de la caméra), retient les douze
meilleurs en zoom Overview et les trente meilleurs en zoom Regional, masque
les chevauchements en espace écran, puis applique une hystérésis de 0,4
seconde avant de cacher un label perdant pour éviter le scintillement. Les
missions lointaines sont regroupées par secteur en zoom Overview au lieu
d'afficher un point par mission, ce qui garde la carte lisible avec dix
missions actives ou plus.

Sélectionner « Surligner » sur une ligne de mission active dans le HUD
flottes bascule un `Resource SelectedMission` lu par le rendu de la vue
Univers et de la vue Système : la route complète de la mission est tracée en
surbrillance et ses points d'origine et de destination reçoivent un marqueur
distinct, dérivés chaque frame de l'état de simulation sans aucune géométrie
mise en cache.

## MVP-030-A5 — Analyse planétaire par satellite

`MissionKind::Analyze` remplace le bouton d'analyse instantané. Une mission
d'analyse exige une cible planétaire au niveau exact `Probed`, le déblocage
`AnalyzePlanets`, une flotte joueur inactive et amarrée composée d'un
Satellite — Veilleur, ainsi qu'une route valide depuis la colonie active. Le ruleset
ajoute `cartographer_satellite` dans `craftables.ron` et
`analysis_duration_seconds` dans `planetary_analysis.ron`.

La mission suit la machine commune `Preparation -> Outbound -> OnSite ->
Returning -> Completed`. La phase `OnSite` correspond à l'analyse orbitale ; le
rapport exact n'est pas créé à l'arrivée, mais seulement au retour en
`Completed`. À ce moment, le moteur produit un `AnalyzeMissionResult`, ajoute le
`PlanetAnalysisReport` persistant, élève la connaissance à `Analyzed` et
rafraîchit le renseignement planétaire en précision `Surveyed`.

L'interface masque désormais les informations exigeant une analyse : une planète
`Probed` ne révèle que son existence, sa désignation, son nom, son type général
et un contact potentiel. Habitabilité, environnement, ressources, site
d'extraction, occupant, population, forces et défenses restent indisponibles
jusqu'au rapport du Satellite — Veilleur.

Versions après ce checkpoint :

- `GAME_STATE_VERSION = 27` ;
- `SAVE_VERSION = 28` ;
- `RULESET_SCHEMA_VERSION` reste 11 ;
- `content_version` du ruleset `default` = 13.

## MVP-030-A6 — Diversification des vaisseaux et du combat

Le catalogue de craftables contient désormais trois transports progressifs
(`light_cargo`, `meridian_carrier`, `atlas_cargo`) et trois coques militaires
(`needle_interceptor`, `frigate_bulwark`, `bastion_cruiser`). Les statistiques
de combat ne sont plus listées dans `combat.ron` : elles sont portées par le
bloc `ship.combat` de chaque craftable militaire, avec attaque, défense,
durabilité, classe de cible et bonus offensifs par classe.

`combat.ron` version 2 ne garde que les paramètres globaux de résolution :
rounds, échelle des dommages, poids défensif, variance déterministe et
récupération. `CombatRules` dérive ses vaisseaux des craftables militaires et
les snapshots d'attaque persistent classe et bonus au moment du lancement.
Les forces de `planetary_presence.ron` possèdent aussi une classe de cible
`Light`, `Medium` ou `Heavy`; l'offensive attaquante est pondérée par ces
classes pendant `resolve_combat`.

L'interface affiche les rôles et stats dans le chantier, ajoute une estimation
d'attaque dans le planificateur de mission et sépare la liste des rapports de
leur détail tactique pour éviter les superpositions.

Versions après ce checkpoint :

- `GAME_STATE_VERSION = 28` ;
- `SAVE_VERSION = 29` ;
- `RULESET_SCHEMA_VERSION = 12` ;
- `content_version` du ruleset `default` = 14.

## MVP-030-A7 — Polish UX et cadrage narratif

Checkpoint de contenu et d'interface avant la persistance. Il ne modifie pas le
modèle de sauvegarde ni les identifiants techniques du ruleset.

Le HUD de ressources distingue désormais :

- `Stock total` : quantité physiquement stockée dans la colonie ;
- `Disponible maintenant` : quantité utilisable après réservations ;
- `Réservé par ordres/missions` : ressources bloquées par files et missions ;
- la formule `Disponible = Stock - Réservé`.

Les panneaux de recherche distinguent explicitement la recherche technologique
de la recherche de carte. Les raccourcis globaux ignorent l'entrée clavier tant
qu'une recherche ou un filtre de navigation est actif, ce qui évite de lancer
des commandes pendant une saisie.

Le helper permanent devient un briefing léger du Consortium, masquable et
réouvrable via `?`. Une modal d'introduction séparée affiche le pitch de départ
au lancement et peut être fermée pour la session.

Le ruleset `default` actualise uniquement les noms et descriptions visibles
selon `docs/galactic_nomenclature_mvp.md`. Les IDs restent inchangés :
`light_probe`, `cartographer_satellite`, `research_lab`, `shipyard`, etc.
Les systèmes des anciennes séries `-2`, `-3` et `-4` reçoivent des noms propres
uniques ; les planètes dérivent automatiquement de ces noms. Ce changement
renouvelle le fingerprint d'univers et incrémente `GENERATION_VERSION`.

L'anneau de planète colonisée utilise un mesh dédié plus fin que l'anneau de
territoire système, tout en conservant la teinte de contrôle joueur.

Versions après ce checkpoint :

- `GAME_STATE_VERSION` reste 28 ;
- `SAVE_VERSION` reste 29 ;
- `GENERATION_VERSION = 5` ;
- `RULESET_SCHEMA_VERSION` reste 12 ;
- `content_version` du ruleset `default` = 16.

## MVP-032-A — Objectifs scénarisés du Consortium

Le client ajoute un panneau `Objectifs du Consortium`, accessible par `O` et
par la barre basse. Il sert de fil directeur immersif : la campagne principale
guide Port-Sillage depuis la production initiale jusqu'à la fondation d'une
deuxième colonie, avec des objectifs facultatifs pour l'exploration lointaine,
les planètes occupées, les récoltes importantes, les flottes mixtes et les
colonies à risque.

L'état d'objectifs est volontairement une ressource client locale à la session :
`ObjectiveProgressState` conserve les objectifs terminés, les dotations déjà
réclamées, l'objectif sélectionné et le feedback ; `ObjectiveUiState` conserve
l'affichage des conseils. Aucun `GameAction`, aucun champ de `GameState` et
aucune donnée de sauvegarde ne sont ajoutés dans cette passe, car MVP-031 est
reporté pour privilégier la validation du gameplay.

Les objectifs se valident uniquement depuis l'état réel de simulation :
niveaux de bâtiments, recherches, inventaires, flottes, connaissances,
rapports de mission, rapports d'analyse, rapports de combat et colonies du
joueur. Les dotations sont de petites ressources créditées une seule fois par
session sur la colonie active ou la colonie mère, avec `credit_capped` pour
respecter les capacités de stockage.

Versions après ce checkpoint :

- `GAME_STATE_VERSION` reste 28 ;
- `SAVE_VERSION` reste 29 ;
- `GENERATION_VERSION` reste 5 ;
- `RULESET_SCHEMA_VERSION` reste 12 ;
- `content_version` du ruleset `default` reste 16.

## Refactor technique — Modularisation interne

Passe technique pure, sans changement de comportement ni de gameplay, menée
en préparation d'une refonte de design du jeu. Objectif : éliminer les
fichiers monolithiques à trop grosses responsabilités, réduire le couplage
entre modules, et sécuriser le code encore non testé avant d'y toucher.

**`galactic_client/src/lib.rs`** passe de 9109 à 1669 lignes. Son contenu est
éclaté en `src/presentation/` par responsabilité : `strategic_navigation.rs`
(historique, fil d'Ariane, `StrategicNavigation`), `components.rs` (marqueurs
Bevy, `ColonyManagementState`, `UiAction`), `scene.rs` (spawn de la scène 3D
et de l'UI), `input.rs` (picking souris, dispatch d'input), `shortcuts.rs`
(raccourcis clavier et disponibilité des actions), `strategic_camera.rs`
(orbit/pan/zoom), `overlays.rs` (rendu gizmo des routes et missions),
`universe_labels.rs` (budget de labels, hystérésis), `resource_hud.rs` et
`inspector_panel.rs` (formatage pur du HUD et des panneaux d'inspection),
`colony_management_ui.rs` (panneau de gestion de colonie),
`procedural_materials.rs` (textures procédurales). `lib.rs` ne conserve plus
que `run()`, `ClientPlugin` (wiring des systèmes, ordre inchangé) et les
enums de configuration graphique. Les quatre modules UI préexistants
(`fleet_ui.rs`, `craft_ui.rs`, `research_ui.rs`, `navigation_ui.rs`) ne sont
pas déplacés : ils étaient déjà correctement isolés.

Le couplage bidirectionnel entre `lib.rs` et ces quatre modules — chacun
possédant son propre booléen d'ouverture, consulté individuellement par les
systèmes de caméra et d'input pour l'exclusion mutuelle des panneaux — est
remplacé par une Resource unique `OpenPanel` (`None`, `Fleet`, `Craft`,
`Research`, `Navigation`, `Colony`). Ouvrir un panneau devient une seule
écriture qui ferme implicitement tous les autres ; `input.rs` et
`strategic_camera.rs` n'ont plus besoin de connaître les Resources internes
de chaque module UI pour arbitrer le blocage caméra/raccourcis. Une
asymétrie préexistante (le panneau Flotte ne bloquait pas la caméra,
contrairement aux quatre autres) a été strictement préservée à l'identique.

Avant ce découpage, `fleet_ui.rs` et `craft_ui.rs` (les deux plus gros
modules UI, sans aucun test) ont reçu une couverture de tests sur leurs
fonctions pures — en particulier le filtrage des cibles de mission par
`KnowledgeLevel` — pour servir de filet de non-régression comportementale.

Côté `galactic_sim`, `mission.rs` (4340 lignes, tous les types de mission
réunis) est réparti en un fichier coordinateur (types communs,
`plan_mission`, `launch_mission`, `cancel_mission`, dispatcher court de
`validate_mission_state`) et un dossier `mission/` avec un fichier par type
(`probe.rs`, `transport.rs`, `harvest.rs`, `colonize.rs`, `attack.rs`),
chacun portant sa fonction de lancement et sa validation de transition
extraite du `match` géant d'origine. Les tests restent groupés dans le
fichier coordinateur : ce sont des tests d'intégration partageant des
fixtures de scénario communes à plusieurs types de mission.

`simulation.rs` (2033 lignes) est réduit à son cœur (`Simulation::new`,
`from_parts`, accesseurs, `apply_command` — dispatcher de `GameAction` déjà
propre, conservé tel quel —, `advance`, `validate_command`, et les tests) et
trois sous-modules imbriqués sous `simulation/` (seule structure compatible
avec l'accès aux champs privés `universe`/`state` de `Simulation` depuis un
autre fichier) : `selection.rs` (sélection de système/planète/colonie
active), `build_error.rs` (`SimulationBuildError`), `reconstruction.rs`
(`validate_state`, la validation d'un `GameState` reconstruit).

`galactic_persistence/src/lib.rs` (1339 lignes, seul fichier de toute la
crate, non consommée par le binaire jouable) est réparti en `save.rs`
(types de sauvegarde), `snapshot.rs` (`snapshot_from_simulation`) et
`restore.rs` (`restore_from_snapshot`), tests conservés ensemble dans
`lib.rs` (tests d'aller-retour exerçant systématiquement les deux
fonctions).

Chaque étape a été vérifiée indépendamment par les quatre commandes qualité
(`cargo fmt --all --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, `cargo test --workspace`, `cargo build
--release`) avec un compte de tests strictement inchangé par crate, complété
par un lancement manuel en `--scale stress`.
