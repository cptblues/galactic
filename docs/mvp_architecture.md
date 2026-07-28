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
- planète mère : première planète de ce système, `Nacre` ;
- habitabilité minimale validée : 80 ;
- faction joueur : `Expédition Aster` ;
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


## MVP-010 — Inspecteurs et informations partielles

Le panneau d'informations est désormais piloté par un contenu d'inspection
structuré plutôt que par un simple dump des objets du domaine.

Matrice d'affichage :

```text
Unknown    aucune donnée exploitable
Detected   signal et placeholders
Probed     identité et estimations
Analyzed   valeurs exactes disponibles
Colonized  valeurs exactes et données économiques
```

Règles :

- un système détecté ne révèle ni son nom, ni sa classe, ni ses coordonnées
  chiffrées, ni le nombre exact de routes ou de corps ;
- un système sondé révèle son identité et des estimations clairement étiquetées ;
- un système analysé révèle les valeurs exactes disponibles ;
- une planète détectée masque nom, type et habitabilité ;
- une planète sondée révèle son identité et une fourchette qualitative
  d'habitabilité ;
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

- Fosse sidérurgique niveau 1 : 2,50 unités/s à potentiel 100 ;
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

1. Veille sidérale ;
2. Propulsion à flux ;
3. Architecture de soute ;
4. Prospection autonome ;
5. Spectrométrie planétaire ;
6. Ingénierie d'implantation.

Les dépendances sont déterministes :

```text
Veille sidérale
├── Propulsion à flux
│   └── Architecture de soute
│       ├── Prospection autonome
│       └── Ingénierie d'implantation
└── Spectrométrie planétaire
    └── Ingénierie d'implantation
```

Chaque définition possède un nom, une description, un coût en milli-points,
des prérequis et une capacité débloquée. Les capacités sont des clés métier
stables qui seront consommées par les missions, crafts et commandes des
checkpoints suivants.

### Production scientifique

L'Institut d'analyse du catalogue produit des milli-points par tick et par niveau.
La production globale additionne les niveaux actifs d'Institut d'analyse de toutes
les colonies du joueur.

Sans Institut d'analyse actif, aucune technologie ne peut être ajoutée à la file.
Une amélioration d'Institut d'analyse agit dès le tick stratégique où sa construction
se termine.

La simulation traite production, construction et recherche tick par tick à
l'intérieur d'un lot de temps. Le résultat reste donc identique quel que soit
le découpage des frames, y compris lorsqu'un Institut d'analyse termine pendant le
lot.

### File globale

Une technologie peut être ajoutée lorsque :

- un Institut d'analyse produit des points ;
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

Le Chantier orbital produit des milli-points de travail à chaque tick
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
