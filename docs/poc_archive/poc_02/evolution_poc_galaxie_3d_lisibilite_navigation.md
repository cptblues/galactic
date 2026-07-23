# Spécification d’évolution — POC Galaxie 3D : lisibilité, navigation et densité

**Projet :** Galactic POC  
**Document parent :** `specification_technique_poc_galaxie_3d_bevy.md`  
**Cible :** évolution incrémentale du POC existant  
**Langage :** Rust  
**Moteur :** Bevy 0.19, conformément au POC existant  
**Version du document :** 1.0  
**Nom de version proposé :** POC 0.2 — Strategic Readability  
**Statut :** spécification d’implémentation destinée à Codex

---

## 1. Objet du document

Le premier POC a validé les points suivants :

- rendu d’une galaxie en trois dimensions ;
- déplacement et rotation de la caméra ;
- zoom entre différentes échelles ;
- sélection d’un système ;
- entrée dans un système contenant des planètes et des lunes ;
- retour vers la vue galactique ;
- intérêt visuel général du concept.

La prochaine évolution doit tester le risque principal qui subsiste en dehors des performances :

> **La carte galactique reste-t-elle lisible, navigable et précise lorsqu’elle contient beaucoup de systèmes et plusieurs couches d’informations stratégiques ?**

Le POC 0.2 ne doit pas introduire un vrai gameplay. Il doit simuler une situation stratégique suffisamment riche pour tester :

1. la lisibilité d’une galaxie dense ;
2. la sélection d’objets superposés ;
3. la conservation du contexte entre galaxie, système, planète et lune ;
4. le zoom sémantique ;
5. les filtres cartographiques ;
6. les territoires, routes, flottes et alertes ;
7. la capacité du joueur à rechercher une information précise sans frustration.

---

## 2. Relation avec le POC existant

Cette spécification est un **delta** et ne remplace pas la spécification initiale.

Codex DOIT :

- conserver la génération procédurale existante ;
- conserver les structures de données stables et indépendantes des entités Bevy ;
- conserver les vues `Galaxy` et `System` ;
- conserver le contrôleur de caméra ;
- conserver le picking et la sélection lorsqu’ils sont compatibles ;
- faire évoluer l’architecture par ajout de plugins et de ressources ;
- éviter une réécriture complète du projet.

Codex NE DOIT PAS modifier visuellement ou fonctionnellement les éléments validés sans nécessité technique documentée.

Avant toute modification, Codex doit :

1. lire le `README.md` ;
2. lire la spécification initiale ;
3. lancer `cargo test` ;
4. lancer le POC existant en mode release ;
5. identifier les plugins, ressources et composants déjà présents ;
6. produire un court état des lieux dans `docs/poc_02_implementation_notes.md`.

---

## 3. Résultat attendu

Au lancement, l’utilisateur voit une galaxie comportant par défaut **500 systèmes** et cinq empires fictifs.

La carte doit montrer, selon le niveau de zoom et les filtres actifs :

- les systèmes stellaires ;
- les systèmes majeurs ;
- les territoires des empires ;
- les systèmes inconnus, détectés, explorés et colonisés ;
- les routes principales et secondaires ;
- des flottes en déplacement ;
- des alertes et anomalies ;
- des marqueurs de mondes habitables ;
- des frontières ou zones d’influence ;
- des regroupements visuels lorsque la densité est trop importante.

L’utilisateur doit pouvoir :

- comprendre rapidement la structure générale de la galaxie ;
- isoler une catégorie de systèmes à l’aide de filtres ;
- rechercher un système par son nom ;
- sélectionner précisément un système malgré une superposition visuelle ;
- basculer entre une représentation 3D et une représentation temporairement aplatie ;
- entrer dans un système ;
- sélectionner une planète ou une lune ;
- suivre un fil d’Ariane ;
- revenir à ses positions précédentes ;
- accomplir un scénario de recherche stratégique prédéfini.

---

## 4. Question de validation

Le POC est considéré comme concluant si un utilisateur externe peut accomplir la tâche suivante sans explication orale :

> **Trouver une planète océanique non colonisée, située à trois routes ou moins d’un système allié, et ne se trouvant pas dans un territoire hostile.**

Les données peuvent être entièrement fictives et générées procéduralement.

La tâche sert à tester conjointement :

- filtres ;
- territoires ;
- routes ;
- inspection ;
- navigation ;
- sélection ;
- compréhension des différents niveaux d’échelle.

---

## 5. Principes normatifs

- **DOIT** : exigence obligatoire ;
- **DEVRAIT** : exigence fortement recommandée ;
- **PEUT** : amélioration facultative ;
- **NE DOIT PAS** : comportement explicitement exclu.

Les exigences obligatoires priment sur l’esthétique secondaire.

---

## 6. Périmètre

### 6.1 Inclus

Le POC 0.2 comprend :

- 500 systèmes par défaut ;
- presets de 100, 500 et 1 000 systèmes ;
- cinq empires fictifs ;
- états d’exploration ;
- états de colonisation ;
- territoires et zones d’influence ;
- routes principales et secondaires ;
- flottes factices animées ;
- alertes cartographiques ;
- mondes habitables et anomalies ;
- zoom sémantique ;
- gestion dynamique des labels ;
- filtres d’affichage ;
- recherche par nom ;
- historique de navigation ;
- fil d’Ariane ;
- mode galaxie aplatie ;
- résolution des sélections ambiguës ;
- mission guidée de validation ;
- panneau de diagnostic de lisibilité ;
- tests unitaires et tests d’intégration sur les données.

### 6.2 Exclus

Le POC 0.2 NE DOIT PAS contenir :

- économie réelle ;
- construction ;
- recherche technologique ;
- intelligence artificielle stratégique ;
- diplomatie dynamique ;
- combat ;
- pathfinding complexe ou recalculé en temps réel ;
- conquête de territoire ;
- véritable brouillard de guerre évolutif ;
- sauvegarde complète de campagne ;
- simulation détaillée des flottes ;
- calcul tactique ;
- multijoueur ;
- génération de surface planétaire.

Tous les états stratégiques doivent être générés ou scénarisés au lancement.

---

## 7. Exigences d’expérience utilisateur

### UX-001 — Compréhension immédiate de l’échelle

L’interface DOIT indiquer en permanence l’échelle active :

```text
Galaxie
Secteur
Système
Corps céleste
```

Le joueur ne doit jamais confondre une vue système avec la carte galactique.

### UX-002 — Conservation du contexte

Un fil d’Ariane DOIT être affiché :

```text
Galaxie > Secteur Orion > Helios Prime > Helios III
```

Chaque élément du fil d’Ariane DOIT être cliquable lorsqu’un retour vers ce niveau est possible.

### UX-003 — Historique de navigation

Le POC DOIT conserver au minimum les 20 dernières destinations ou poses de caméra significatives.

Contrôles :

- `Alt + Gauche` : destination précédente ;
- `Alt + Droite` : destination suivante ;
- boutons précédent/suivant dans la barre supérieure.

Une destination significative correspond à :

- ouverture d’un système ;
- focalisation d’un système ;
- focalisation d’une planète ou lune ;
- résultat sélectionné depuis la recherche ;
- retour explicite à la galaxie.

### UX-004 — Retour à la planète ou au système d’origine

Une action « Retour à l’origine » DOIT être disponible.

Pour le POC, l’origine est un système allié généré et marqué comme capitale du joueur.

Raccourci recommandé : `Home`.

### UX-005 — Réduction du bruit

Le joueur DOIT pouvoir masquer indépendamment :

- labels ;
- routes principales ;
- routes secondaires ;
- frontières ;
- zones d’influence ;
- flottes ;
- alertes ;
- anomalies ;
- mondes habitables ;
- systèmes inconnus.

### UX-006 — Feedback de sélection

Le hover, la présélection et la sélection confirmée doivent être visuellement différents.

Exemple :

- hover : halo léger ;
- candidat ambigu : anneau pointillé ;
- sélection : anneau plein et label renforcé ;
- destination de navigation : impulsion ou animation brève.

### UX-007 — Carte 3D et carte aplatie

Le joueur DOIT pouvoir basculer entre :

- `Perspective 3D` ;
- `Projection aplatie`.

Raccourci proposé : `P`.

La projection aplatie ne doit pas modifier les données. Elle applique uniquement une transformation visuelle interpolée des positions verticales vers zéro.

Le retour à la 3D doit restaurer les positions d’origine.

---

## 8. Zoom sémantique

Le zoom ne doit pas uniquement modifier la taille apparente. Il doit modifier la nature des informations affichées.

### 8.1 Niveaux de zoom

Définir les niveaux suivants :

```rust
pub enum SemanticZoomLevel {
    GalaxyOverview,
    Regional,
    Local,
    SystemApproach,
}
```

Le niveau peut être calculé depuis la distance de la caméra à son point de focalisation.

Les seuils doivent être configurables.

### 8.2 Niveau `GalaxyOverview`

Afficher :

- noyau et structure galactique ;
- noms des empires ;
- capitales ;
- systèmes majeurs ;
- régions ou clusters ;
- frontières simplifiées ;
- alertes critiques uniquement.

Masquer :

- planètes ;
- flottes individuelles ;
- routes secondaires ;
- labels des systèmes ordinaires ;
- petites anomalies.

### 8.3 Niveau `Regional`

Afficher :

- systèmes importants ;
- routes principales ;
- frontières ;
- flottes agrégées ;
- mondes habitables marqués ;
- systèmes sélectionnés ou recherchés ;
- alertes importantes.

### 8.4 Niveau `Local`

Afficher :

- tous les systèmes proches ;
- routes principales et secondaires ;
- flottes individuelles ;
- états exploration/colonisation ;
- anomalies ;
- labels dynamiques selon priorité.

### 8.5 Niveau `SystemApproach`

Afficher :

- système focalisé ;
- voisins immédiats ;
- routes directes ;
- aperçu compact du contenu du système ;
- indicateur permettant d’entrer dans le système.

### 8.6 Hystérésis

Le changement de niveau DOIT utiliser une hystérésis afin d’éviter le clignotement près des seuils.

Exemple :

- entrée dans un niveau à une distance donnée ;
- sortie avec une marge de 5 à 10 %.

---

## 9. Gestion des labels

### 9.1 Objectif

Les labels ne doivent pas recouvrir la carte ni se chevaucher excessivement.

### 9.2 Priorité des labels

Attribuer un score à chaque candidat :

```text
+1000 : objet sélectionné
+900  : résultat de recherche
+800  : capitale
+700  : système en alerte critique
+600  : système focalisé
+500  : système majeur
+400  : monde habitable visible via filtre
+300  : flotte importante
+200  : système colonisé
+100  : système ordinaire proche du curseur
```

Des bonus peuvent être ajoutés selon la proximité écran et le niveau de zoom.

### 9.3 Budget de labels

Limiter le nombre de labels visibles :

```text
GalaxyOverview : 12
Regional       : 25
Local          : 50
SystemApproach : 20
```

Les valeurs doivent être configurables.

### 9.4 Collision écran

Avant d’afficher un label :

1. projeter sa position 3D en coordonnées écran ;
2. calculer un rectangle estimé ;
3. vérifier la collision avec les rectangles déjà acceptés ;
4. accepter les labels par ordre décroissant de priorité ;
5. toujours accepter le label sélectionné, même s’il masque un label inférieur.

Une estimation approximative de la largeur du texte est acceptable pour le POC.

### 9.5 Stabilisation

Le choix des labels doit rester stable entre les frames.

Ajouter un bonus temporaire aux labels affichés à la frame précédente pour éviter les changements permanents lors de petits mouvements de caméra.

---

## 10. Sélection dans une scène dense

### 10.1 Problème

Plusieurs systèmes peuvent se projeter à quelques pixels les uns des autres.

Le premier objet touché par un rayon 3D n’est pas toujours celui que le joueur pense sélectionner.

### 10.2 Sélection en espace écran

Dans la vue galactique, la sélection DEVRAIT privilégier une mesure en espace écran.

Algorithme recommandé :

1. projeter les candidats visibles vers l’écran ;
2. calculer leur distance au pointeur ;
3. retenir ceux situés dans un rayon de sélection configurable ;
4. trier selon :
   - distance écran ;
   - priorité visuelle ;
   - profondeur ;
   - importance stratégique ;
5. sélectionner automatiquement le meilleur candidat si l’ambiguïté est faible ;
6. ouvrir un sélecteur ambigu si plusieurs candidats ont un score proche.

### 10.3 Rayon de sélection

Valeurs proposées :

```text
souris : 14 pixels
trackpad : 18 pixels
mode accessibilité : 24 pixels
```

Le rayon ne doit pas dépendre de la taille visuelle exacte de l’étoile.

### 10.4 Sélecteur ambigu

Si au moins deux candidats sont proches :

- afficher un petit panneau contextuel près du curseur ;
- lister de 2 à 6 candidats ;
- montrer nom, icône et profondeur relative ;
- permettre sélection souris ou touches numériques ;
- `Tab` parcourt les candidats ;
- `Échap` ferme le panneau.

### 10.5 Cycle local

Après un clic sur une zone ambiguë, des clics successifs au même endroit peuvent parcourir les candidats pendant 1,5 seconde.

### 10.6 Magnétisme visuel facultatif

Le POC PEUT légèrement attirer le halo de hover vers le candidat le plus proche sans déplacer réellement le curseur.

---

## 11. Recherche

### 11.1 Interface

Ajouter un champ de recherche accessible par :

- clic dans la barre supérieure ;
- raccourci `/` ;
- `Ctrl + F`.

### 11.2 Contenu recherché

Rechercher dans :

- noms de systèmes ;
- noms de planètes ;
- noms de lunes ;
- noms d’empires ;
- tags : océanique, habitable, anomalie, colonisé, etc.

### 11.3 Résultats

Afficher au maximum 10 résultats avec :

- type d’objet ;
- nom ;
- chemin hiérarchique ;
- empire ou état ;
- distance depuis l’origine, si pertinente.

### 11.4 Action

Sélectionner un résultat doit :

1. fermer ou réduire la recherche ;
2. créer une entrée d’historique ;
3. focaliser la caméra ;
4. sélectionner l’objet ;
5. ouvrir le système parent si le résultat est une planète ou une lune.

---

## 12. Données stratégiques factices

### 12.1 Empires

Générer cinq empires :

```rust
pub struct FactionData {
    pub id: FactionId,
    pub name: String,
    pub capital: SystemId,
    pub disposition: FactionDisposition,
    pub ui_color: FactionColor,
}
```

Dispositions :

```rust
pub enum FactionDisposition {
    Player,
    Allied,
    Neutral,
    Rival,
    Hostile,
}
```

Une faction est celle du joueur.

Une faction est alliée.

Au moins une faction est hostile.

### 12.2 Couleurs

Les couleurs doivent être distinctes, mais ne doivent pas être la seule information transmise.

Ajouter :

- symbole ou motif ;
- forme de marqueur ;
- texte dans l’inspecteur.

Prévoir une palette compatible avec les déficiences courantes de perception des couleurs.

### 12.3 État d’exploration

```rust
pub enum ExplorationState {
    Unknown,
    Detected,
    Scanned,
    Surveyed,
}
```

Impact visuel :

- `Unknown` : système masqué ou très atténué ;
- `Detected` : position approximative, contenu inconnu ;
- `Scanned` : étoile et nombre approximatif de planètes ;
- `Surveyed` : informations complètes.

### 12.4 État de contrôle

```rust
pub enum ControlState {
    Unclaimed,
    Outpost(FactionId),
    Colonized(FactionId),
    Capital(FactionId),
    Contested(FactionId, FactionId),
}
```

Aucun état ne change pendant le POC.

### 12.5 Monde habitable

Au moins 8 % des systèmes doivent contenir une planète considérée comme habitable.

Au moins une planète océanique non colonisée doit satisfaire le scénario de validation.

La graine de démonstration doit garantir l’existence de cette cible.

### 12.6 Anomalies et alertes

```rust
pub enum MapAlertKind {
    HostileFleet,
    DistressSignal,
    Anomaly,
    BorderTension,
    Opportunity,
}

pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}
```

Les alertes sont fixes ou animées visuellement, sans logique de résolution.

---

## 13. Territoires et zones d’influence

### 13.1 Objectif

Les territoires doivent être compréhensibles sans transformer la galaxie en carte opaque.

### 13.2 Approche recommandée

Pour le POC, ne pas construire de géométrie politique astronomiquement exacte.

Utiliser une des méthodes suivantes, par ordre de préférence :

1. nuage ou enveloppe translucide par faction autour de ses systèmes ;
2. cellules approximatives en espace galactique ;
3. contours générés sur une projection 2D de la galaxie ;
4. halos locaux autour des systèmes contrôlés.

La solution doit être documentée dans `docs/poc_02_implementation_notes.md`.

### 13.3 Frontières

Les frontières doivent :

- rester discrètes ;
- être plus visibles au zoom global/régional ;
- s’atténuer au zoom local ;
- ne pas masquer les systèmes ;
- pouvoir être désactivées.

### 13.4 Territoire hostile

Le territoire hostile doit être identifiable par :

- couleur ;
- motif ou pulsation légère ;
- libellé dans l’inspecteur ;
- avertissement lors de la focalisation.

---

## 14. Secteurs et clusters

### 14.1 Secteur

Ajouter une structure logique de secteur :

```rust
pub struct SectorData {
    pub id: SectorId,
    pub name: String,
    pub center: Vec3,
    pub systems: Vec<SystemId>,
}
```

### 14.2 Génération

Une approche simple est acceptable :

- grille spatiale ;
- clustering par proximité ;
- attribution au bras galactique le plus proche ;
- partition déterministe en 8 à 16 secteurs.

### 14.3 Utilisation

Les secteurs servent à :

- afficher des labels au zoom global ;
- alimenter le fil d’Ariane ;
- faciliter la recherche ;
- agréger les flottes ;
- réduire le nombre de labels système.

---

## 15. Routes et distance stratégique

### 15.1 Types

```rust
pub enum RouteKind {
    Major,
    Minor,
}
```

### 15.2 Génération

Conserver le graphe existant, puis classifier les routes.

Une route peut être majeure si :

- elle relie une capitale ;
- elle appartient à un chemin entre capitales ;
- elle est utilisée par plusieurs itinéraires factices ;
- elle relie deux secteurs.

### 15.3 Distance de mission

La contrainte « trois routes ou moins » doit être calculée par BFS sur le graphe non pondéré depuis au moins un système allié.

Ce calcul est effectué au chargement ou lors de la génération de la mission, pas chaque frame.

### 15.4 Mise en évidence d’un chemin

Lorsqu’un système est sélectionné, le POC DEVRAIT pouvoir montrer le chemin le plus court depuis l’origine ou depuis l’allié le plus proche.

Raccourci proposé : `K`.

---

## 16. Flottes factices

### 16.1 Modèle

```rust
pub struct FleetData {
    pub id: FleetId,
    pub name: String,
    pub faction: FactionId,
    pub route: Vec<SystemId>,
    pub segment_index: usize,
    pub progress: f32,
    pub importance: FleetImportance,
}
```

### 16.2 Animation

Les flottes se déplacent visuellement le long de routes existantes.

Elles ne prennent aucune décision.

À la fin de leur route :

- elles peuvent recommencer ;
- inverser le trajet ;
- disparaître puis réapparaître.

### 16.3 Agrégation

Au zoom global :

- ne pas afficher toutes les flottes ;
- afficher une icône agrégée par secteur ou route principale ;
- indiquer un nombre approximatif.

Au zoom local :

- afficher les flottes individuelles proches.

---

## 17. Filtres

### 17.1 Ressource

```rust
#[derive(Resource, Clone)]
pub struct MapFilters {
    pub labels: bool,
    pub major_routes: bool,
    pub minor_routes: bool,
    pub borders: bool,
    pub influence: bool,
    pub fleets: bool,
    pub alerts: bool,
    pub anomalies: bool,
    pub habitable_worlds: bool,
    pub unknown_systems: bool,
    pub faction_filter: Option<FactionId>,
    pub exploration_filter: Option<ExplorationState>,
}
```

### 17.2 Presets

Ajouter quatre presets :

```text
Exploration
Diplomatie
Navigation
Minimal
```

Exemples :

- `Exploration` : habitables, anomalies, inconnus, labels utiles ;
- `Diplomatie` : territoires, frontières, capitales, relations ;
- `Navigation` : routes, systèmes, flottes ;
- `Minimal` : sélection, systèmes majeurs, aucun bruit secondaire.

### 17.3 Persistance de session

La persistance sur disque n’est pas obligatoire.

Les filtres doivent rester inchangés pendant les transitions entre vues.

---

## 18. Mission de validation intégrée

### 18.1 Déclenchement

Ajouter un bouton « Lancer le test de navigation » dans le menu debug ou l’aide.

### 18.2 Texte

```text
Objectif : trouvez une planète océanique non colonisée,
située à trois routes ou moins d’un système allié,
et hors de tout territoire hostile.
```

### 18.3 Progression

Étapes indicatives :

1. activer ou utiliser les filtres ;
2. inspecter un système candidat ;
3. ouvrir le système ;
4. sélectionner la planète ;
5. valider la cible.

### 18.4 Validation

Ajouter un bouton « Valider cette planète » dans l’inspecteur planète pendant la mission.

Le programme vérifie :

- `PlanetKind::Ocean` ;
- système non colonisé ;
- distance BFS à un allié `<= 3` ;
- système hors contrôle hostile.

### 18.5 Résultat

Afficher :

- succès ou raison de l’échec ;
- temps écoulé ;
- nombre de sélections ;
- nombre de changements de filtre ;
- nombre de retours/navigation arrière ;
- nombre de sélections ambiguës rencontrées.

Aucune donnée personnelle ni télémétrie externe ne doit être envoyée.

---

## 19. Instrumentation locale

### 19.1 Ressource de session

```rust
#[derive(Resource, Default)]
pub struct UsabilityMetrics {
    pub mission_started_at: Option<Instant>,
    pub selection_count: u32,
    pub ambiguous_selection_count: u32,
    pub navigation_back_count: u32,
    pub filter_change_count: u32,
    pub search_count: u32,
    pub view_transition_count: u32,
}
```

### 19.2 Affichage

Le panneau `F3` doit afficher :

- FPS et frame time ;
- nombre d’entités ;
- niveau de zoom sémantique ;
- nombre de labels candidats/affichés ;
- nombre de systèmes visibles ;
- nombre de flottes individuelles/agrégées ;
- mode 3D/aplati ;
- objet sélectionné ;
- taille de l’historique ;
- nombre de sélections ambiguës.

### 19.3 Export facultatif

Le POC PEUT proposer un export JSON local des métriques de session.

Aucune transmission réseau.

---

## 20. Architecture technique

### 20.1 Plugins à ajouter

Étendre l’architecture existante avec :

```text
StrategicMapPlugin
├── FactionPlugin
├── SectorPlugin
├── TerritoryPlugin
├── SemanticZoomPlugin
├── LabelManagementPlugin
├── MapFilterPlugin
├── SearchPlugin
├── NavigationHistoryPlugin
├── FleetVisualizationPlugin
├── MissionValidationPlugin
└── UsabilityMetricsPlugin
```

Les noms peuvent être adaptés aux conventions du dépôt.

### 20.2 Modules proposés

```text
src/
├── strategic/
│   ├── mod.rs
│   ├── factions.rs
│   ├── exploration.rs
│   ├── control.rs
│   ├── sectors.rs
│   ├── alerts.rs
│   └── mission.rs
├── map/
│   ├── mod.rs
│   ├── semantic_zoom.rs
│   ├── filters.rs
│   ├── labels.rs
│   ├── territories.rs
│   ├── projection.rs
│   └── aggregation.rs
├── navigation/
│   ├── mod.rs
│   ├── history.rs
│   ├── breadcrumb.rs
│   ├── search.rs
│   └── focus.rs
├── fleets/
│   ├── mod.rs
│   ├── data.rs
│   ├── generation.rs
│   └── visuals.rs
└── usability/
    ├── mod.rs
    ├── metrics.rs
    └── mission_ui.rs
```

### 20.3 Séparation données/rendu

Les nouvelles données stratégiques doivent être stockées indépendamment des entités visuelles.

Exemple :

```rust
#[derive(Resource, Clone)]
pub struct StrategicGalaxyData {
    pub factions: Vec<FactionData>,
    pub sectors: Vec<SectorData>,
    pub system_states: HashMap<SystemId, SystemStrategicState>,
    pub alerts: Vec<MapAlertData>,
    pub fleets: Vec<FleetData>,
    pub validation_target: Option<PlanetId>,
}
```

Ne pas stocker de `Entity` dans cette ressource.

### 20.4 État stratégique système

```rust
pub struct SystemStrategicState {
    pub sector: SectorId,
    pub exploration: ExplorationState,
    pub control: ControlState,
    pub influence: Vec<FactionInfluence>,
    pub alerts: Vec<AlertId>,
}
```

### 20.5 Ordonnancement indicatif

```text
Input
→ Camera target update
→ Camera smoothing
→ Semantic zoom evaluation
→ Flatten projection update
→ Visibility classification
→ Label candidate scoring
→ Label collision resolution
→ Picking candidates
→ Selection resolution
→ Navigation history update
→ Fleet visual animation
→ UI update
→ Diagnostics
```

Le calcul lourd ou invariant ne doit pas être exécuté à chaque frame.

---

## 21. Mode aplati

### 21.1 Ressource

```rust
#[derive(Resource)]
pub struct MapProjectionState {
    pub mode: MapProjectionMode,
    pub blend: f32,
    pub target_blend: f32,
}

pub enum MapProjectionMode {
    ThreeDimensional,
    Flattened,
}
```

### 21.2 Transformation

Pour chaque système visuel :

```rust
let displayed_y = original_y * (1.0 - blend);
```

`blend` évolue progressivement entre `0.0` et `1.0`.

Durée indicative : 0,5 seconde.

### 21.3 Contraintes

- la sélection doit fonctionner pendant et après la transition ;
- la caméra ne doit pas sauter ;
- les routes doivent suivre les positions affichées ;
- les territoires doivent rester cohérents ;
- le mode ne doit pas altérer `GalaxyData`.

---

## 22. Agrégation visuelle

### 22.1 Objectif

Éviter d’afficher des centaines de marqueurs identiques au zoom global.

### 22.2 Groupes écran

Une stratégie simple est suffisante :

1. projeter les objets en coordonnées écran ;
2. les répartir dans une grille de cellules ;
3. agréger les objets d’une même cellule ;
4. afficher un marqueur de groupe contenant le nombre d’objets.

### 22.3 Objets concernés

- flottes ;
- alertes mineures ;
- systèmes inconnus ;
- marqueurs d’habitabilité ;
- anomalies.

### 22.4 Dégroupement

Lorsque le joueur zoome :

- les groupes se divisent progressivement ;
- éviter une apparition brutale ;
- conserver la sélection si un objet agrégé était sélectionné.

---

## 23. Interface

### 23.1 Barre supérieure

Ajouter :

- fil d’Ariane ;
- boutons précédent/suivant ;
- recherche ;
- indicateur 3D/aplati ;
- niveau de zoom sémantique en mode debug.

### 23.2 Panneau gauche

Transformer le panneau de filtres en panneau repliable avec :

- presets ;
- calques ;
- filtres de faction ;
- filtres d’exploration ;
- boutons tout afficher/tout masquer.

### 23.3 Inspecteur droit

Ajouter :

- secteur ;
- empire contrôlant ;
- état d’exploration ;
- statut hostile/allié ;
- distance en routes depuis l’origine ;
- alertes ;
- bouton « Montrer le chemin » ;
- bouton « Entrer dans le système » ;
- bouton « Ajouter aux favoris » facultatif.

### 23.4 Mini aide contextuelle

Lorsqu’une sélection est ambiguë pour la première fois, afficher une aide brève :

```text
Plusieurs systèmes sont sous le curseur.
Utilisez Tab ou cliquez dans la liste pour choisir.
```

Ne pas répéter cette aide continuellement.

---

## 24. Contrôles ajoutés

| Action | Contrôle |
|---|---|
| Rechercher | `/` ou `Ctrl + F` |
| Historique précédent | `Alt + Gauche` |
| Historique suivant | `Alt + Droite` |
| Basculer 3D/aplati | `P` |
| Origine/capitale | `Home` |
| Montrer chemin | `K` |
| Parcourir candidats ambigus | `Tab` |
| Fermer panneau contextuel | `Échap` |
| Preset Exploration | `1` avec panneau filtres actif |
| Preset Diplomatie | `2` avec panneau filtres actif |
| Preset Navigation | `3` avec panneau filtres actif |
| Preset Minimal | `4` avec panneau filtres actif |

Les contrôles existants restent inchangés sauf conflit documenté.

---

## 25. Contraintes de performance sans en faire l’objet principal

Le POC 0.2 teste surtout la lisibilité, mais ne doit pas dégrader inutilement le fonctionnement sur machine sans GPU dédié.

### 25.1 Exigences

- ne pas créer un widget Bevy UI permanent pour chaque système ;
- ne pas afficher tous les labels ;
- partager les meshes et matériaux ;
- calculer territoires, secteurs et routes hors boucle de rendu ;
- ne pas effectuer de BFS chaque frame ;
- mettre à jour la résolution de collision des labels uniquement lorsque nécessaire ;
- réduire les effets de bloom et de particules avec un preset `Low` ;
- désactiver ou réduire les flottes et territoires via le preset graphique `Low`.

### 25.2 Presets graphiques minimaux

```rust
pub enum GraphicsPreset {
    Low,
    Medium,
    High,
}
```

`Low` doit au minimum :

- réduire le champ d’étoiles décoratif ;
- désactiver les ombres non essentielles ;
- réduire le bloom ;
- réduire les particules ;
- réduire les détails de territoire ;
- limiter le nombre d’objets animés visibles.

La logique de carte et les informations disponibles doivent rester identiques.

---

## 26. Tests automatisés

### 26.1 Déterminisme stratégique

Même graine et même configuration doivent produire :

- mêmes factions ;
- mêmes capitales ;
- mêmes secteurs ;
- mêmes contrôles de systèmes ;
- mêmes flottes initiales ;
- même cible de mission.

### 26.2 Mission toujours réalisable

Pour la graine de démonstration :

- une planète océanique non colonisée existe ;
- elle est à trois routes ou moins d’un allié ;
- elle est hors territoire hostile.

### 26.3 Cohérence des factions

Vérifier :

- une capitale existante par faction ;
- aucun système capitale partagé ;
- la capitale possède le bon `ControlState` ;
- les factions référencées existent.

### 26.4 Cohérence des secteurs

Vérifier :

- chaque système appartient à un secteur ;
- aucune référence invalide ;
- noms de secteur uniques ;
- centres finis.

### 26.5 Graphe et BFS

Vérifier :

- distance zéro vers soi-même ;
- chemin valide entre nœuds connectés ;
- absence de chemin correctement signalée ;
- résultat `<= 3` pour la cible de mission.

### 26.6 Historique

Tester :

- ajout d’une destination ;
- précédent/suivant ;
- troncature de la branche suivante après une nouvelle navigation ;
- limite à 20 entrées ;
- absence de doublons consécutifs inutiles.

### 26.7 Filtres

Tester :

- presets ;
- activation/désactivation ;
- persistance durant changement de vue ;
- cohérence des valeurs par défaut.

### 26.8 Zoom sémantique

Tester la fonction pure déterminant le niveau depuis une distance.

Tester l’hystérésis.

### 26.9 Scoring de labels

Tester :

- priorité du sélectionné ;
- respect du budget ;
- rejet des collisions ;
- stabilité entre deux frames simulées.

### 26.10 Sélection ambiguë

Tester la fonction pure de classement des candidats à partir de positions écran simulées.

---

## 27. Protocole de test manuel

### Test A — Densité globale

1. Charger le preset 500 systèmes.
2. Afficher routes, territoires et alertes.
3. Tourner et zoomer pendant deux minutes.
4. Vérifier que les informations importantes restent identifiables.

Succès : pas de saturation permanente et possibilité de réduire rapidement le bruit.

### Test B — Superposition

1. Choisir une zone dense.
2. Tenter de sélectionner cinq systèmes proches.
3. Tester le panneau de sélection ambiguë.
4. Basculer en mode aplati.

Succès : le joueur sélectionne le système voulu sans zoom extrême.

### Test C — Contexte

1. Entrer dans un système.
2. Sélectionner une planète.
3. Sélectionner une lune.
4. Utiliser le fil d’Ariane.
5. Utiliser précédent/suivant.
6. Retourner à l’origine.

Succès : aucun doute sur la position courante et aucun retour incohérent.

### Test D — Filtres

1. Activer le preset Exploration.
2. Rechercher les mondes habitables.
3. Passer au preset Diplomatie.
4. Identifier les territoires hostiles.
5. Passer au preset Minimal.

Succès : chaque preset répond clairement à un besoin distinct.

### Test E — Mission

Faire accomplir la mission à un utilisateur qui ne connaît pas le code.

Collecter localement :

- temps ;
- erreurs ;
- hésitations observées ;
- clics ambigus ;
- nombre de changements de filtre.

Succès recommandé : mission réussie en moins de cinq minutes sans instruction orale.

### Test F — Mode Low

1. Utiliser une machine sans GPU dédié ou un preset Low.
2. Charger 500 systèmes.
3. Vérifier navigation et sélection.

Succès : la réduction visuelle ne détruit pas la lisibilité fonctionnelle.

---

## 28. Étapes d’implémentation pour Codex

### Étape 0 — Audit du POC

Livrables :

- `docs/poc_02_implementation_notes.md` ;
- cartographie des plugins existants ;
- liste des éléments réutilisés ;
- liste des divergences éventuelles.

Critère : aucune réécriture engagée avant compréhension du dépôt.

### Étape 1 — Données stratégiques

Livrables :

- factions ;
- états exploration/contrôle ;
- secteurs ;
- alertes ;
- génération déterministe ;
- tests.

Critère : toutes les données sont inspectables sans rendu.

### Étape 2 — Territoires et secteurs

Livrables :

- affichage des secteurs ;
- zones d’influence ;
- frontières ;
- filtres associés.

Critère : les cinq empires sont distinguables au zoom global.

### Étape 3 — Zoom sémantique

Livrables :

- niveaux ;
- hystérésis ;
- règles de visibilité ;
- diagnostic `F3`.

Critère : les informations évoluent sans clignotement lors du zoom.

### Étape 4 — Labels dynamiques

Livrables :

- scoring ;
- budget ;
- collision écran ;
- stabilisation ;
- labels sectoriels.

Critère : le système sélectionné reste toujours étiqueté et la carte ne sature pas.

### Étape 5 — Sélection dense

Livrables :

- candidats écran ;
- classement ;
- panneau ambigu ;
- cycle `Tab` ;
- métriques.

Critère : sélection fiable dans une zone dense.

### Étape 6 — Navigation contextuelle

Livrables :

- fil d’Ariane ;
- historique ;
- précédent/suivant ;
- retour origine ;
- restauration de caméra.

Critère : parcours galaxie → système → planète → lune → galaxie sans perte de contexte.

### Étape 7 — Recherche

Livrables :

- index de recherche ;
- interface ;
- résultats ;
- focalisation ;
- intégration historique.

Critère : retrouver un système connu en quelques secondes.

### Étape 8 — Mode aplati

Livrables :

- projection ;
- transition ;
- routes et picking compatibles ;
- bouton/raccourci.

Critère : la projection aide à résoudre les superpositions sans altérer les données.

### Étape 9 — Flottes et agrégation

Livrables :

- flottes factices ;
- animation ;
- agrégation selon zoom ;
- filtres.

Critère : la carte illustre une activité sans devenir illisible.

### Étape 10 — Mission intégrée

Livrables :

- mission ;
- cible garantie ;
- validation ;
- métriques ;
- écran de résultat.

Critère : scénario complet réalisable.

### Étape 11 — Presets graphiques

Livrables :

- Low/Medium/High ;
- configuration ;
- interface ;
- documentation.

Critère : mode Low fonctionnel sans perte d’information stratégique.

### Étape 12 — Stabilisation

Livrables :

- tests ;
- correction des fuites d’entités ;
- documentation ;
- commandes qualité ;
- captures d’écran.

---

## 29. Critères d’acceptation

### Données

- [ ] 500 systèmes par défaut.
- [ ] Cinq empires générés de manière déterministe.
- [ ] Chaque système appartient à un secteur.
- [ ] Les états d’exploration et de contrôle sont cohérents.
- [ ] La mission possède toujours une cible valide avec la graine de démo.

### Lisibilité

- [ ] Les labels sont limités par budget.
- [ ] Le label sélectionné est toujours visible.
- [ ] Les collisions majeures entre labels sont évitées.
- [ ] Le zoom sémantique modifie clairement les informations affichées.
- [ ] Les territoires peuvent être compris et masqués.
- [ ] Les flottes sont agrégées à grande distance.

### Sélection

- [ ] Un système peut être sélectionné dans une zone dense.
- [ ] Les cas ambigus affichent une liste de candidats.
- [ ] `Tab` parcourt les candidats.
- [ ] Le mode aplati améliore la sélection des systèmes superposés.

### Navigation

- [ ] Le fil d’Ariane est correct.
- [ ] Les boutons précédent/suivant fonctionnent.
- [ ] La caméra est restaurée de manière cohérente.
- [ ] `Home` retourne à l’origine.
- [ ] La recherche focalise et sélectionne correctement les objets.

### Filtres

- [ ] Tous les calques principaux peuvent être masqués.
- [ ] Les quatre presets sont fonctionnels.
- [ ] Les filtres survivent aux changements de vue.

### Mission

- [ ] La mission peut être lancée depuis l’interface.
- [ ] Une bonne cible est acceptée.
- [ ] Une mauvaise cible explique pourquoi elle est refusée.
- [ ] Les métriques locales sont affichées à la fin.

### Robustesse

- [ ] Aucun doublon massif d’entités après plusieurs changements de vue.
- [ ] Le mode 3D/aplati est réversible.
- [ ] La régénération réinitialise correctement les données stratégiques.
- [ ] Les tests passent.
- [ ] Clippy ne retourne aucun warning.
- [ ] Le README décrit les nouvelles fonctionnalités.

---

## 30. Définition de terminé

Le POC 0.2 est terminé lorsque :

1. le POC initial continue de fonctionner ;
2. la galaxie dense est exploitable avec 500 systèmes ;
3. le zoom sémantique réduit efficacement le bruit ;
4. les labels sont hiérarchisés et stables ;
5. les sélections ambiguës peuvent être résolues ;
6. les filtres et presets sont utilisables ;
7. la navigation conserve le contexte ;
8. le mode aplati est fonctionnel ;
9. les territoires et flottes factices enrichissent la carte sans créer de gameplay ;
10. la mission de validation est réalisable ;
11. les métriques locales permettent d’évaluer l’expérience ;
12. le mode graphique Low reste lisible ;
13. les tests et commandes qualité passent ;
14. les limites connues sont documentées.

---

## 31. Commandes de qualité

Avant livraison :

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run --release
```

Codex doit aussi tester manuellement :

```text
100 systèmes — High
500 systèmes — Low
500 systèmes — High
1 000 systèmes — Low / diagnostic uniquement
```

---

## 32. Documentation à mettre à jour

Le `README.md` doit ajouter :

- objectif du POC 0.2 ;
- nouveaux contrôles ;
- filtres et presets ;
- recherche ;
- historique ;
- mode aplati ;
- mission de validation ;
- presets graphiques ;
- limites connues.

Créer également :

```text
docs/poc_02_implementation_notes.md
docs/poc_02_manual_test_protocol.md
docs/poc_02_known_limitations.md
```

---

## 33. Limites acceptables pour ce POC

Sont acceptables :

- territoires approximatifs ;
- texte sans localisation complète ;
- recherche simple et synchrone ;
- animation de flottes non réaliste ;
- agrégation visuelle approximative ;
- collision de quelques labels secondaires ;
- sélection ambiguë nécessitant un panneau contextuel ;
- transitions non parfaitement cinématiques ;
- données stratégiques statiques.

Ne sont pas acceptables :

- perte régulière de la sélection ;
- impossibilité de revenir à la galaxie ;
- interface saturée en permanence ;
- labels clignotant continuellement ;
- modification destructive des données lors du mode aplati ;
- historique incohérent ;
- mission impossible avec la graine de démonstration ;
- forte dépendance à des assets propriétaires ;
- ajout de gameplay hors périmètre.

---

## 34. Consignes finales à Codex

1. Traiter ce document comme une évolution de la spécification initiale.
2. Ne pas réécrire le POC sans justification technique.
3. Préserver la séparation entre données et rendu.
4. Utiliser des identifiants stables, jamais `Entity`, dans les données persistantes.
5. Favoriser les fonctions pures pour le zoom, les filtres, le scoring et le classement des sélections.
6. Implémenter d’abord les données, puis la visualisation.
7. Maintenir le projet compilable après chaque étape.
8. Ajouter des tests avant ou avec chaque algorithme non trivial.
9. Ne pas implémenter d’économie, d’IA, de combat ou de diplomatie dynamique.
10. Privilégier la lisibilité à l’effet visuel.
11. Vérifier systématiquement le comportement avec le preset Low.
12. Documenter toute divergence et toute dette technique créée.
13. Terminer avec les commandes de qualité et le protocole manuel.

Le résultat recherché n’est pas encore un jeu de stratégie. Il s’agit d’un banc d’essai permettant de déterminer si une galaxie 3D dense peut devenir une véritable interface de décision, plutôt qu’un simple décor spectaculaire.
