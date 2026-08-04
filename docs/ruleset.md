# Ruleset économique

Galactic charge son ruleset au démarrage depuis
`assets/rulesets/default/`. La variable d'environnement
`GALACTIC_RULESET_DIR` permet de sélectionner un autre dossier.

Le ruleset `default` est actuellement en `content_version: 16`. Cette version
actualise les noms et descriptions visibles selon
`docs/galactic_nomenclature_mvp.md`, sans renommer les identifiants stables.

Le ruleset est composé de onze fichiers RON :

- `manifest.ron` : identifiant et versions ;
- `economy.ron` : stockage de base, limites de files et cadence de production ;
- `factions.ron` : factions, types, activation et relations initiales ;
- `buildings.ron` : bâtiments, coûts, durées, effets et prérequis ;
- `technologies.ron` : arbre de recherche, coûts et déblocages ;
- `craftables.ron` : objets fabricables, vaisseaux, coûts, prérequis et capacités ;
- `planetary_analysis.ron` : durée de mission d'analyse, seuil de
  colonisabilité, coût de fondation et profils environnementaux par type de
  planète ;
- `planetary_presence.ron` : profils d'occupation, population et forces
  terrestres ou orbitales ;
- `combat.ron` : rounds, variation, récupération et paramètres globaux du
  combat ;
- `extraction.ron` : ressource, réserve, rendement et durée de récolte des
  sites distants par type de planète ;
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

`factions.ron` définit aussi une relation par défaut et des exceptions
symétriques entre paires de factions. Les valeurs reconnues sont `Unknown`,
`Neutral`, `Hostile` et `Allied`. Ces relations sont consultables et
sauvegardées, mais n'accordent encore aucun droit de gestion et ne déclenchent
aucune diplomatie active.

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
technologies ainsi qu'une liste de capacités numériques. Une fabrication qui
représente un vaisseau ajoute une classe, une vitesse de croisière, une portée
en sauts, une capacité cargo et une consommation de carburant par saut. Les
vaisseaux militaires ajoutent aussi un bloc `combat` avec attaque, défense,
durabilité, classe de cible (`Light`, `Medium`, `Heavy`) et bonus offensifs
optionnels par classe de cible. Le catalogue par défaut contient :

- `light_probe` avec `probe_strength` ;
- `cartographer_satellite` avec `sensor_level`, dédié aux missions d'analyse ;
- `light_cargo`, cargo rapide de 500 unités ;
- `meridian_carrier`, cargo intermédiaire de 1 600 unités ;
- `atlas_cargo`, cargo lourd de 4 200 unités ;
- `needle_interceptor`, militaire léger spécialisé contre les cibles légères ;
- `frigate_bulwark`, militaire moyen polyvalent ;
- `bastion_cruiser`, militaire lourd spécialisé contre les cibles lourdes ;
- `colony_ship` avec `colonization_capacity` ;

Les classes de vaisseau reconnues sont `Probe`, `Cargo`, `Colony`, `Military`
et `Support`. Une défense n'est pas un vaisseau. Une fabrication doit dépendre
d'au moins un bâtiment ayant l'effet
`ShipyardPoints`. Sa durée de base correspond à la cadence du niveau minimal
requis ; améliorer le chantier accélère ensuite la file.

Un comportement entièrement nouveau reste une mécanique du moteur et nécessite
une implémentation Rust.

## Analyse planétaire

`planetary_analysis.ron` configure la durée passée en orbite par une mission
`Analyze`, le seuil minimal d'habitabilité, la limite de colonies,
l'investissement de fondation, le `CraftableId` du vaisseau-colonie et le socle
économique d'une nouvelle colonie : population et niveaux de bâtiments. Pour
chaque `PlanetKind`, il définit aussi le milieu, les potentiels de base,
l'éligibilité à une implantation et les contraintes reconnues. Le vaisseau
référencé doit exister, appartenir à la classe `Colony` et disposer d'une
capacité cargo au moins égale au chargement de fondation. Les bâtiments initiaux
doivent respecter le catalogue et fournir une capacité suffisante pour ce
chargement. Le moteur applique ensuite une variation déterministe bornée à
chaque potentiel afin que deux mondes de même type ne soient pas strictement
identiques.

Les contraintes reconnues sont `ThinAtmosphere`, `GlobalOcean`,
`AridClimate`, `CryogenicClimate`, `ExtremeVolcanism` et `NoSolidSurface`.
Elles sont informatives ; l'éligibilité du type, l'habitabilité, la route, la
technologie, la limite de colonies et la cargaison de fondation sont évaluées
séparément et produisent chacune un motif de refus explicite.

Le ruleset par défaut associe cette fonction à `colony_ship`. Sa capacité cargo
est de 1 400 unités pour un chargement de 1 330 unités. La mission réserve ce
chargement au lancement mais ne le consomme qu'après validation de la cible à
l'arrivée. Au succès, ce chargement devient le stock local de la colonie ;
l'énergie et la capacité de stockage sont dérivées de ses bâtiments, tandis que
son profil de production reprend le rapport d'analyse de la planète.

## Présences planétaires

`planetary_presence.ron` définit des forces réutilisables et des profils
d'occupation pondérés. Une force indique son domaine (`Ground` ou `Orbital`),
ses valeurs d'attaque, de défense et de durabilité, sa classe de cible
(`Light`, `Medium`, `Heavy`) ainsi que le pas utilisé pour arrondir les
estimations du joueur. Un profil choisit une faction occupante éventuelle, une
population bornée et les quantités de chaque force.

Le moteur sélectionne et matérialise ces données de façon déterministe pour
chaque `PlanetId`. La planète colonisée au départ utilise un profil séparé
décrivant sa population et sa garnison. Les identifiants de force et de faction
doivent exister, les poids et statistiques doivent être strictement positifs,
et les bornes doivent rester ordonnées.

`home_neighborhood.guaranteed_profile_id` désigne en outre un profil hostile
appliqué à une planète de chaque système directement voisin du départ lorsque
le tirage normal n'y a placé aucune présence équivalente. Cette garantie
fournit une cible de combat proche sans modifier la distribution du reste de
l'univers.

Le ruleset configure l'état réel, jamais la précision de l'interface. Une sonde
ne révèle que l'identité minimale et le contact ; une mission de Satellite —
Veilleur transforme les valeurs réelles en fourchettes arrondies au retour
du rapport. Les pertes futures modifient l'état réel sans actualiser
automatiquement le dernier renseignement connu.

## Combat

`combat.ron` versionne la résolution de combat V2. Il fixe le nombre maximal de
rounds, l'échelle des dommages, le poids défensif, la variation déterministe et
la récupération par défense détruite. Les vaisseaux utilisables en attaque sont
dérivés des craftables militaires qui possèdent un bloc `ship.combat`.

La puissance offensive de l'attaquant est pondérée par les classes de cible
défensives engagées. Un bonus `offense_multiplier_per_mille: 1400` signifie
donc `x1.40` contre la classe correspondante, sans changer les valeurs
d'attaque affichées du vaisseau.

Modifier une statistique ou un paramètre exige une nouvelle version de contenu.
Ajouter ou retirer un identifiant de vaisseau militaire change l'empreinte
structurelle et rend explicitement incompatibles les sauvegardes précédentes.

## Extraction distante

`extraction.ron` définit un profil pour chaque `PlanetKind`. Un profil choisit
la ressource produite, la réserve initiale, le rendement maximal d'une mission
et sa durée de chargement en ticks stratégiques. Un site stable est généré pour
chaque planète ; il n'est exploitable qu'après analyse et déblocage de
`RemoteExtraction`.

Lorsque `reserves_sites` est actif, une seule mission peut réserver un site à
la fois. La quantité chargée est le minimum entre le rendement du profil, la
réserve restante et la capacité libre de la flotte. Elle est débitée une seule
fois au départ du site, transportée comme cargaison réelle, puis créditée dans
la limite du stockage de la colonie d'origine. Un reliquat reste dans la flotte
amarrée plutôt que d'être perdu.

## Validation et sauvegardes

Le chargement refuse notamment les identifiants invalides ou dupliqués, les
références absentes, les cycles de prérequis, les coûts nuls, les durées ou
capacités invalides et les niveaux de départ incohérents.

Les sauvegardes enregistrent l'identifiant du ruleset, sa version de schéma et
une empreinte structurelle. Les noms et descriptions ne participent pas à cette
empreinte : leur correction ne rend donc pas une sauvegarde incompatible.

Le hot reload pendant une partie n'est pas pris en charge. Il faut redémarrer le
jeu après chaque modification.

Les noms visibles du ruleset `default` suivent
[`docs/universe_bible.md`](universe_bible.md). Les identifiants techniques
restent stables même lorsqu'un libellé ou une description évolue.
