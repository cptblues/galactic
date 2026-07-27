# Ruleset économique

Galactic charge son ruleset au démarrage depuis
`assets/rulesets/default/`. La variable d'environnement
`GALACTIC_RULESET_DIR` permet de sélectionner un autre dossier.

Le ruleset est composé de cinq fichiers RON :

- `manifest.ron` : identifiant et versions ;
- `economy.ron` : stockage de base, limites de files et cadence de production ;
- `buildings.ron` : bâtiments, coûts, durées, effets et prérequis ;
- `technologies.ron` : arbre de recherche, coûts et déblocages ;
- `starting_scenario.ron` : faction, colonie, ressources et bâtiments initiaux.

Les durées de construction sont exprimées en secondes dans les données puis
converties en ticks stratégiques au chargement. La fréquence de dix ticks par
seconde reste une règle du moteur.

## Identifiants et effets

Les identifiants sont stables, en minuscules, avec des chiffres et des `_`.
Ils sont enregistrés dans l'état de partie et ne doivent pas être renommés
après publication d'un ruleset.

Un nouveau bâtiment ou une nouvelle technologie peut être ajouté sans modifier
Rust s'il utilise un effet ou un déblocage déjà pris en charge. Les effets de
bâtiment disponibles sont :

- `MetalProduction`, `CrystalProduction` et `FuelProduction` ;
- `EnergyProduction` ;
- `Storage` ;
- `ConstructionSpeed` ;
- `ResearchPoints` ;
- `ShipyardPoints`.

Les déblocages technologiques disponibles sont :

- `DetectUnknownSystems` ;
- `InterstellarTravel` ;
- `ExpandedCargo` ;
- `RemoteExtraction` ;
- `AnalyzePlanets` ;
- `FoundColonies`.

Un comportement entièrement nouveau reste une mécanique du moteur et nécessite
une implémentation Rust.

## Validation et sauvegardes

Le chargement refuse notamment les identifiants invalides ou dupliqués, les
références absentes, les cycles de prérequis, les coûts scientifiques nuls, les
durées invalides et les niveaux de départ incohérents.

Les sauvegardes enregistrent l'identifiant du ruleset, sa version de schéma et
une empreinte structurelle. Les noms et descriptions ne participent pas à cette
empreinte : leur correction ne rend donc pas une sauvegarde incompatible.

Le hot reload pendant une partie n'est pas pris en charge. Il faut redémarrer le
jeu après chaque modification.
