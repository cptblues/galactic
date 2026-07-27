# Ruleset économique

Galactic charge son ruleset au démarrage depuis
`assets/rulesets/default/`. La variable d'environnement
`GALACTIC_RULESET_DIR` permet de sélectionner un autre dossier.

Le ruleset est composé de sept fichiers RON :

- `manifest.ron` : identifiant et versions ;
- `economy.ron` : stockage de base, limites de files et cadence de production ;
- `factions.ron` : factions, types et activation initiale ;
- `buildings.ron` : bâtiments, coûts, durées, effets et prérequis ;
- `technologies.ron` : arbre de recherche, coûts et déblocages ;
- `craftables.ron` : objets fabricables, coûts, durées, prérequis et capacités ;
- `starting_scenario.ron` : faction joueur, colonie, ressources et bâtiments initiaux.

Les durées de construction et de fabrication sont exprimées en secondes dans
les données puis converties en ticks stratégiques au chargement. La fréquence
de dix ticks par seconde reste une règle du moteur.

## Identifiants et effets

Les identifiants sont stables, en minuscules, avec des chiffres et des `_`.
Ils sont enregistrés dans l'état de partie et ne doivent pas être renommés
après publication d'un ruleset.

Les identifiants numériques de faction sont également stables. Une faction
`Player` active représente le joueur. Les factions `Neutral` et `FutureAi`
peuvent rester inactives : elles sont persistées et disponibles pour les
prochains systèmes de relations et d'IA, sans exécuter de boucle d'action.

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

Les fabrications possèdent un `CraftableId` textuel, une catégorie, un coût,
une durée de base, une quantité produite, des prérequis de bâtiments et de
technologies ainsi qu'une liste de capacités numériques. Le catalogue par
défaut contient :

- `light_probe` avec `probe_strength` ;
- `light_cargo` avec `cargo_capacity` ;
- `colony_ship` avec `colonization_capacity`.

Les catégories `Defense`, `Military` et `Support` sont déjà reconnues, sans
imposer de contenu actif. Une fabrication doit dépendre d'au moins un bâtiment
ayant l'effet `ShipyardPoints`. Sa durée de base correspond à la cadence du
niveau minimal requis ; améliorer le chantier accélère ensuite la file.

Un comportement entièrement nouveau reste une mécanique du moteur et nécessite
une implémentation Rust.

## Validation et sauvegardes

Le chargement refuse notamment les identifiants invalides ou dupliqués, les
références absentes, les cycles de prérequis, les coûts nuls, les durées ou
capacités invalides et les niveaux de départ incohérents.

Les sauvegardes enregistrent l'identifiant du ruleset, sa version de schéma et
une empreinte structurelle. Les noms et descriptions ne participent pas à cette
empreinte : leur correction ne rend donc pas une sauvegarde incompatible.

Le hot reload pendant une partie n'est pas pris en charge. Il faut redémarrer le
jeu après chaque modification.
