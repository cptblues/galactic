# Spécification technique — POC de visualisation galactique 3D

**Projet :** Galactic POC  
**Cible :** application desktop native  
**Langage :** Rust  
**Moteur :** Bevy 0.19  
**Version du document :** 1.0  
**Statut :** spécification d’implémentation destinée à Codex

---

## 1. Mission

Réaliser un prototype technique permettant de valider la faisabilité et l’intérêt visuel d’une carte galactique en trois dimensions.

Le prototype doit afficher une galaxie générée procéduralement, permettre de la parcourir avec une caméra fluide, sélectionner un système stellaire, puis ouvrir une vue détaillée de ce système contenant une étoile, plusieurs planètes, des lunes et éventuellement une ceinture d’astéroïdes.

Le POC n’est pas un jeu complet. Il doit prioriser :

1. la qualité et la lisibilité de la visualisation 3D ;
2. la navigation entre l’échelle galactique et l’échelle d’un système ;
3. la génération procédurale déterministe ;
4. la sélection et l’inspection des objets ;
5. une architecture suffisamment propre pour être réutilisée.

---

## 2. Résultat attendu

Au lancement, l’utilisateur arrive directement dans une vue 3D d’une galaxie en spirale.

Il peut :

- tourner autour de la galaxie ;
- déplacer le point de focalisation de la caméra ;
- zoomer et dézoomer ;
- survoler une étoile ;
- sélectionner un système ;
- consulter un panneau récapitulatif ;
- double-cliquer ou appuyer sur `Entrée` pour ouvrir le système ;
- observer l’étoile, les planètes, les lunes, les orbites et une éventuelle ceinture d’astéroïdes ;
- sélectionner une planète ou une lune ;
- revenir à la galaxie ;
- régénérer la galaxie avec une nouvelle graine ;
- réutiliser une graine connue afin d’obtenir exactement la même galaxie.

Le prototype doit être exécutable avec :

```bash
cargo run --release
```

---

## 3. Principes normatifs

Les termes suivants ont un sens précis :

- **DOIT** : exigence obligatoire ;
- **DEVRAIT** : exigence fortement recommandée ;
- **PEUT** : fonctionnalité facultative ;
- **NE DOIT PAS** : comportement explicitement exclu.

Codex doit implémenter les exigences obligatoires avant les améliorations facultatives.

---

## 4. Hypothèses techniques

### 4.1 Versions

Utiliser :

```toml
[package]
name = "galactic-poc"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy = "0.19"
rand = "0.9"
rand_chacha = "0.9"
```

Ne pas ajouter de moteur physique.

Ne pas introduire de dépendance tierce pour :

- la caméra ;
- la sélection ;
- l’interface ;
- la génération de galaxie ;
- les lignes d’orbite.

Une dépendance supplémentaire ne doit être ajoutée que si l’implémentation native Bevy bloque réellement le POC et doit alors être justifiée dans le `README.md`.

### 4.2 Plateformes ciblées

Le POC cible en priorité :

- Windows ;
- Linux ;
- macOS.

Le WebAssembly, Android et iOS sont hors périmètre.

### 4.3 Assets

La première version DOIT fonctionner sans asset externe obligatoire.

Les objets sont construits avec :

- primitives Bevy ;
- matériaux procéduraux ;
- couleurs ;
- lumières ;
- texte avec la police par défaut disponible dans Bevy ou une police libre ajoutée dans `assets/fonts`.

Une texture de fond ou une police peut être ajoutée ensuite, mais le programme ne doit pas dépendre d’un pack graphique complexe.

---

## 5. Périmètre fonctionnel

### 5.1 Inclus

Le POC comprend :

- une galaxie 3D en spirale ;
- 200 systèmes par défaut ;
- une graine déterministe ;
- une caméra orbitale avec pan et zoom ;
- une sélection par pointeur ;
- une mise en évidence au survol ;
- un panneau d’inspection ;
- une vue détaillée d’un système ;
- des planètes et des lunes ;
- des orbites visibles ;
- une animation orbitale simplifiée ;
- une ceinture d’astéroïdes optionnelle ;
- un retour vers la vue galaxie ;
- un compteur FPS ;
- quelques options de débogage ;
- des tests unitaires sur la génération.

### 5.2 Exclus

Le POC NE DOIT PAS contenir :

- économie ;
- construction de bâtiments ;
- recherche ;
- factions jouables ;
- intelligence artificielle stratégique ;
- diplomatie ;
- combat ;
- pathfinding de flotte ;
- sauvegarde de partie ;
- simulation physique ;
- gravité newtonienne ;
- échelle astronomique réelle ;
- multijoueur ;
- éditeur de galaxie ;
- génération de terrain planétaire ;
- déplacement libre à la surface d’une planète.

Des métadonnées fictives comme « habitable », « riche en minerais » ou « colonisée » peuvent être générées uniquement pour enrichir l’inspection visuelle.

---

## 6. Parcours utilisateur de référence

### 6.1 Lancement

1. La fenêtre s’ouvre en 1600 × 900, redimensionnable.
2. Une galaxie est générée à partir d’une graine par défaut.
3. La caméra cadre automatiquement toute la galaxie.
4. Le panneau d’aide présente brièvement les contrôles.
5. Le FPS courant et la graine sont visibles.

### 6.2 Exploration galactique

1. L’utilisateur tourne la caméra avec le bouton droit.
2. Il effectue un pan avec le bouton central ou `Maj + bouton droit`.
3. Il zoome avec la molette.
4. Le système sous le pointeur reçoit un halo.
5. Un clic gauche sélectionne le système.
6. Le panneau d’inspection affiche ses propriétés.
7. La touche `F` recentre la caméra sur le système sélectionné.

### 6.3 Ouverture d’un système

1. L’utilisateur double-clique un système sélectionné ou appuie sur `Entrée`.
2. Une transition de caméra de 0,8 à 1,2 seconde rapproche le point de vue.
3. Les objets de la galaxie sont masqués ou despawnés.
4. Les objets du système sont créés.
5. La caméra passe en mode orbital autour de l’étoile.

### 6.4 Inspection d’un corps céleste

1. L’utilisateur survole puis sélectionne une planète ou une lune.
2. Le panneau affiche son nom, son type, sa taille stylisée, sa distance orbitale et ses métadonnées.
3. La touche `F` focalise la caméra sur l’objet.
4. La touche `Espace` met en pause ou reprend l’animation orbitale.

### 6.5 Retour

1. `Échap` ou `Retour arrière` ramène à la vue galaxie.
2. La sélection galactique précédente est restaurée.
3. La caméra revient près de sa position antérieure ou recadre le système quitté.

---

## 7. Exigences fonctionnelles

### FR-001 — Génération déterministe

La galaxie DOIT être générée avec un générateur pseudo-aléatoire initialisé par une graine `u64`.

Avec la même graine, la même version de l’algorithme et la même configuration, le résultat doit être identique.

Le générateur recommandé est `ChaCha8Rng`.

### FR-002 — Nombre de systèmes configurable

Le nombre de systèmes doit être configurable dans une ressource :

```rust
#[derive(Resource, Clone)]
pub struct GalaxyConfig {
    pub seed: u64,
    pub system_count: usize,
    pub arm_count: usize,
    pub radius: f32,
    pub thickness: f32,
    pub spiral_turns: f32,
    pub arm_spread: f32,
    pub min_system_distance: f32,
}
```

Valeurs par défaut :

```rust
GalaxyConfig {
    seed: 42,
    system_count: 200,
    arm_count: 4,
    radius: 100.0,
    thickness: 8.0,
    spiral_turns: 2.2,
    arm_spread: 0.22,
    min_system_distance: 1.8,
}
```

### FR-003 — Contenu d’un système

Chaque système DOIT contenir :

- exactement une étoile ;
- de 2 à 9 planètes ;
- de 0 à 4 lunes par planète ;
- zéro ou une ceinture d’astéroïdes ;
- un nom unique dans la galaxie ;
- des métadonnées générées.

### FR-004 — Types d’étoiles

Implémenter au minimum :

```rust
pub enum StarClass {
    Blue,
    White,
    Yellow,
    Orange,
    Red,
}
```

Chaque classe influence la couleur visuelle, la taille stylisée, la luminosité, une température fictive affichée et les probabilités de types planétaires.

### FR-005 — Types de planètes

Implémenter au minimum :

```rust
pub enum PlanetKind {
    Rocky,
    Desert,
    Ocean,
    Ice,
    Volcanic,
    GasGiant,
}
```

Chaque type influence la couleur, le rayon visuel, la distance orbitale probable, le nombre de lunes probable et l’habitabilité fictive.

### FR-006 — États de vue

Utiliser un état Bevy :

```rust
#[derive(States, Default, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ViewState {
    #[default]
    Galaxy,
    System,
}
```

Les objets propres à une vue doivent être identifiables et nettoyés dans `OnExit`.

### FR-007 — Sélection

Une ressource unique doit représenter la sélection :

```rust
#[derive(Resource, Default)]
pub struct Selection {
    pub hovered: Option<SelectableId>,
    pub selected: Option<SelectableId>,
}
```

Les identifiants persistants NE DOIVENT PAS dépendre de `Entity`.

Utiliser des newtypes stables :

```rust
pub struct SystemId(pub u32);
pub struct PlanetId(pub u32);
pub struct MoonId(pub u32);
```

### FR-008 — Ouverture d’un système

L’entrée dans un système DOIT être possible par double-clic sur un système ou par la touche `Entrée` lorsqu’un système est sélectionné.

### FR-009 — Régénération

Prévoir :

- `R` : régénération avec la graine actuelle ;
- `N` : nouvelle graine ;
- une zone ou commande permettant de renseigner une graine, si cela reste simple avec Bevy UI.

La régénération doit nettoyer tous les objets visuels précédents.

### FR-010 — Pause d’animation

Dans la vue système :

- `Espace` met en pause ou reprend les orbites ;
- la caméra et l’interface restent interactives ;
- la pause peut utiliser le temps virtuel Bevy ou une ressource spécifique à l’animation.

---

## 8. Exigences visuelles

### VR-001 — Forme galactique

La galaxie doit évoquer clairement un disque spiralé : centre plus dense, plusieurs bras, légère épaisseur verticale, dispersion irrégulière et quelques systèmes hors bras.

Elle ne doit pas être un simple nuage sphérique uniforme.

### VR-002 — Lisibilité

À l’échelle galactique :

- les systèmes sont représentés par des points lumineux ou de petites sphères ;
- seuls le système survolé, le système sélectionné et quelques systèmes remarquables affichent un label ;
- les objets ne doivent pas clignoter ou changer brutalement de taille ;
- le système sélectionné doit rester identifiable à tout niveau de zoom raisonnable.

### VR-003 — Profondeur

La caméra doit permettre de constater que les systèmes n’ont pas tous la même coordonnée verticale.

L’épaisseur doit rester assez faible pour conserver la lecture d’un disque galactique.

### VR-004 — Étoiles

Chaque étoile doit utiliser une sphère ou un billboard, un matériau émissif, une intensité dépendant de sa classe, un halo ou bloom si disponible, et un indicateur distinct lorsque sélectionnée.

### VR-005 — Vue système

La vue système doit afficher :

- étoile centrale ;
- orbites circulaires ou légèrement elliptiques ;
- planètes à des tailles exagérées ;
- lunes rattachées visuellement à leur planète ;
- inclinaisons orbitales légères ;
- profondeur suffisante pour confirmer la nature 3D de la scène.

### VR-006 — Échelle stylisée

Ne jamais utiliser les vraies proportions astronomiques.

Utiliser trois échelles distinctes : taille visuelle des corps, rayon orbital et distance entre systèmes.

La lisibilité prime sur le réalisme.

### VR-007 — Fond spatial

Le fond doit être sombre et contenir un champ d’étoiles discret.

Implémentation recommandée :

- 1 000 à 3 000 points ou petits billboards ;
- distribution sur une grande sphère autour de la caméra ;
- pas de collision ni picking ;
- matériau non éclairé ou émissif ;
- densité assez faible pour ne pas masquer les systèmes interactifs.

### VR-008 — Routes galactiques

Afficher des liaisons entre systèmes proches est recommandé.

Algorithme :

1. pour chaque système, chercher ses trois voisins les plus proches ;
2. ajouter une arête si elle n’existe pas ;
3. ne pas créer plus de six liens par système ;
4. afficher les routes avec une opacité faible ;
5. accentuer les routes du système sélectionné.

Les routes ne représentent pas encore un vrai pathfinding.

---

## 9. Contrôles

| Action | Contrôle |
|---|---|
| Rotation caméra | Bouton droit + déplacement |
| Pan caméra | Bouton central, ou `Maj` + bouton droit |
| Zoom | Molette |
| Sélection | Clic gauche |
| Entrer dans le système | Double-clic ou `Entrée` |
| Retour à la galaxie | `Échap` ou `Retour arrière` |
| Focaliser la sélection | `F` |
| Pause des orbites | `Espace` |
| Régénérer même graine | `R` |
| Générer nouvelle graine | `N` |
| Afficher/masquer les routes | `L` |
| Afficher/masquer les orbites | `O` |
| Afficher/masquer les labels | `T` |
| Afficher/masquer debug | `F3` |
| Quitter | Fermeture de fenêtre |

Les contrôles doivent être rappelés dans un panneau d’aide repliable.

---

## 10. Modèle de données

Les données de galaxie doivent être indépendantes des entités de rendu.

### 10.1 Racine

```rust
#[derive(Resource, Clone, Debug)]
pub struct GalaxyData {
    pub seed: u64,
    pub systems: Vec<StarSystemData>,
    pub routes: Vec<GalaxyRoute>,
}
```

### 10.2 Système

```rust
#[derive(Clone, Debug)]
pub struct StarSystemData {
    pub id: SystemId,
    pub name: String,
    pub position: Vec3,
    pub star: StarData,
    pub planets: Vec<PlanetData>,
    pub asteroid_belt: Option<AsteroidBeltData>,
    pub tags: SystemTags,
}
```

### 10.3 Étoile

```rust
#[derive(Clone, Debug)]
pub struct StarData {
    pub class: StarClass,
    pub visual_radius: f32,
    pub luminosity: f32,
    pub temperature_kelvin: u32,
}
```

### 10.4 Planète

```rust
#[derive(Clone, Debug)]
pub struct PlanetData {
    pub id: PlanetId,
    pub name: String,
    pub kind: PlanetKind,
    pub visual_radius: f32,
    pub orbit_radius: f32,
    pub orbit_speed: f32,
    pub orbit_phase: f32,
    pub orbit_inclination: f32,
    pub habitability: u8,
    pub moons: Vec<MoonData>,
}
```

### 10.5 Lune

```rust
#[derive(Clone, Debug)]
pub struct MoonData {
    pub id: MoonId,
    pub name: String,
    pub visual_radius: f32,
    pub orbit_radius: f32,
    pub orbit_speed: f32,
    pub orbit_phase: f32,
    pub orbit_inclination: f32,
}
```

### 10.6 Route

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct GalaxyRoute {
    pub a: SystemId,
    pub b: SystemId,
}
```

### 10.7 Tags d’inspection

```rust
#[derive(Clone, Debug, Default)]
pub struct SystemTags {
    pub has_habitable_world: bool,
    pub mineral_rich: bool,
    pub anomaly_detected: bool,
}
```

---

## 11. Génération procédurale

### 11.1 Algorithme des bras spiraux

Pour chaque système :

1. choisir un bras `arm_index` ;
2. tirer une valeur normalisée `u` dans `[0, 1]` ;
3. calculer un rayon biaisé vers le centre ;
4. calculer un angle dépendant du rayon ;
5. ajouter un bruit angulaire ;
6. ajouter un bruit radial ;
7. ajouter une hauteur `y` ;
8. rejeter le point s’il est trop proche d’un système existant ;
9. après un nombre maximal d’essais, accepter le meilleur point disponible.

Formulation indicative :

```rust
let radius = config.radius * u.powf(0.65);
let arm_base = arm_index as f32 * TAU / config.arm_count as f32;
let spiral_angle = arm_base
    + config.spiral_turns * TAU * (radius / config.radius)
    + angular_noise;

let x = radius * spiral_angle.cos() + radial_noise;
let z = radius * spiral_angle.sin() + radial_noise_2;
let normalized_radius = radius / config.radius;
let y_scale = 0.35 + normalized_radius * 0.65;
let y = random_signed() * config.thickness * y_scale;
```

Le résultat exact peut différer, mais doit respecter les critères visuels.

### 11.2 Bulbe central

Environ 10 à 15 % des systèmes doivent être générés dans un bulbe central plus dense.

Le bulbe peut utiliser une distribution sphérique aplatie.

### 11.3 Systèmes périphériques

Environ 3 à 8 % des systèmes peuvent être placés hors des bras afin d’éviter une apparence trop régulière.

### 11.4 Distance minimale

La génération doit tenter de respecter `min_system_distance`.

Une recherche quadratique est acceptable pour 200 systèmes. Ne pas implémenter de structure spatiale complexe dans le premier POC.

### 11.5 Génération d’un système

Ordre conseillé :

1. générer la classe de l’étoile ;
2. déterminer le nombre de planètes ;
3. générer des rayons orbitaux croissants ;
4. attribuer les types selon la distance à l’étoile ;
5. générer les lunes ;
6. décider d’une ceinture d’astéroïdes ;
7. générer les tags.

Les rayons orbitaux doivent être strictement croissants.

```rust
let mut orbit = 7.0;
for _planet_index in 0..planet_count {
    orbit += rng.random_range(4.0..9.0);
    // Génération de la planète.
}
```

### 11.6 Nommage

Créer un générateur interne combinant des syllabes.

```text
Préfixes : Al, Ar, Bel, Cer, Dra, Eri, Hel, Kor, Ly, Nex, Or, Pra, Sol, Tal, Vel
Milieux : a, e, i, o, u, ae, io, ar, en, on
Suffixes : ia, on, us, ar, is, ea, Prime, Minor, Major
```

Les noms doivent être uniques dans la galaxie.

Les planètes peuvent être nommées `NomDuSystème I`, `NomDuSystème II`, etc. Les lunes peuvent être nommées `NomDePlanète-a`, `NomDePlanète-b`, etc.

---

## 12. Architecture Bevy

### 12.1 Plugins

Créer des plugins séparés :

```text
AppPlugin
├── DataPlugin
├── GalaxyGenerationPlugin
├── GalaxyViewPlugin
├── SystemViewPlugin
├── CameraControlPlugin
├── InteractionPlugin
├── UiPlugin
└── DiagnosticsPlugin
```

#### `DataPlugin`

- types de données ;
- ressources globales ;
- identifiants stables ;
- configuration.

#### `GalaxyGenerationPlugin`

- génération de `GalaxyData` ;
- création des routes ;
- nouvelle graine ;
- tests de génération.

#### `GalaxyViewPlugin`

- spawn et despawn des systèmes visuels ;
- rendu des routes ;
- mise en évidence ;
- affichage des labels galactiques.

#### `SystemViewPlugin`

- spawn et despawn des corps du système ;
- orbites ;
- animation ;
- ceinture d’astéroïdes ;
- transitions d’entrée et de sortie.

#### `CameraControlPlugin`

- contrôleur orbital ;
- pan ;
- zoom ;
- focalisation ;
- transition fluide.

#### `InteractionPlugin`

- picking ;
- survol ;
- clic ;
- double-clic ;
- mise à jour de `Selection`.

#### `UiPlugin`

- panneau système ;
- panneau corps céleste ;
- aide ;
- graine ;
- FPS ;
- messages d’état.

#### `DiagnosticsPlugin`

- FPS ;
- nombre d’entités ;
- paramètres de debug ;
- éventuellement durée de génération.

### 12.2 Arborescence

```text
galactic-poc/
├── Cargo.toml
├── README.md
├── assets/
│   └── fonts/
└── src/
    ├── main.rs
    ├── app.rs
    ├── state.rs
    ├── data/
    │   ├── mod.rs
    │   ├── galaxy.rs
    │   ├── system.rs
    │   ├── body.rs
    │   └── ids.rs
    ├── generation/
    │   ├── mod.rs
    │   ├── galaxy.rs
    │   ├── system.rs
    │   ├── names.rs
    │   └── routes.rs
    ├── camera/
    │   ├── mod.rs
    │   ├── controller.rs
    │   └── transition.rs
    ├── interaction/
    │   ├── mod.rs
    │   ├── picking.rs
    │   └── selection.rs
    ├── views/
    │   ├── mod.rs
    │   ├── galaxy.rs
    │   └── system.rs
    ├── rendering/
    │   ├── mod.rs
    │   ├── materials.rs
    │   ├── orbits.rs
    │   ├── routes.rs
    │   └── starfield.rs
    ├── ui/
    │   ├── mod.rs
    │   ├── hud.rs
    │   ├── inspector.rs
    │   └── help.rs
    └── diagnostics/
        └── mod.rs
```

Cette séparation peut être légèrement simplifiée si certains fichiers restent très courts.

### 12.3 Entités de vue

Marquer toutes les entités créées par une vue :

```rust
#[derive(Component)]
pub struct GalaxyViewEntity;

#[derive(Component)]
pub struct SystemViewEntity;
```

À la sortie d’une vue, despawn récursivement les entités portant le marqueur correspondant.

### 12.4 Composants visuels

```rust
#[derive(Component)]
pub struct StarSystemVisual {
    pub id: SystemId,
}

#[derive(Component)]
pub struct StarVisual;

#[derive(Component)]
pub struct PlanetVisual {
    pub id: PlanetId,
}

#[derive(Component)]
pub struct MoonVisual {
    pub id: MoonId,
}

#[derive(Component)]
pub struct OrbitVisual;

#[derive(Component)]
pub struct Selectable {
    pub id: SelectableId,
}
```

Éviter de dupliquer les données complètes dans les composants visuels.

---

## 13. Caméra

### 13.1 Contrôleur orbital

Utiliser un composant :

```rust
#[derive(Component)]
pub struct OrbitCamera {
    pub focus: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub rotate_sensitivity: f32,
    pub pan_sensitivity: f32,
    pub zoom_sensitivity: f32,
}
```

À chaque frame :

1. mettre à jour yaw, pitch, distance et focus selon les entrées ;
2. borner le pitch afin d’éviter le retournement ;
3. calculer la position depuis les coordonnées sphériques ;
4. appliquer `Transform::looking_at(focus, Vec3::Y)`.

### 13.2 Valeurs indicatives

Vue galaxie :

```text
distance initiale : 170
distance minimale : 8
distance maximale : 400
pitch initial : -0,55 rad
```

Vue système :

```text
distance initiale : 55
distance minimale : 2
distance maximale : 150
pitch initial : -0,45 rad
```

### 13.3 Lissage

Le mouvement doit être lissé.

```rust
let alpha = 1.0 - (-smoothing * time.delta_secs()).exp();
current = current.lerp(target, alpha);
```

### 13.4 Transition entre vues

Une ressource de transition suffit :

```rust
#[derive(Resource, Default)]
pub struct CameraTransition {
    pub active: bool,
    pub elapsed: f32,
    pub duration: f32,
    pub from: CameraPose,
    pub to: CameraPose,
    pub pending_view: Option<ViewState>,
}
```

Utiliser une courbe `smoothstep` ou cubic ease-in-out.

Pendant une transition :

- désactiver les entrées caméra ;
- conserver l’interface ;
- changer l’état de vue au moment approprié ;
- éviter tout saut de caméra visible.

---

## 14. Picking et interaction

### 14.1 Stratégie

Utiliser le picking de meshes intégré à Bevy ou `MeshRayCast`.

Les systèmes et corps célestes doivent posséder une géométrie de sélection légèrement plus grande que leur géométrie visuelle lorsque nécessaire.

Le picking doit retourner l’objet visible le plus proche.

### 14.2 Survol

Lorsqu’un objet devient survolé :

- augmenter légèrement son échelle ou son intensité ;
- afficher son nom ;
- afficher un contour, un anneau ou un halo ;
- restaurer le matériau précédent à la fin du survol.

Ne pas recréer un nouveau matériau à chaque frame.

Préparer des handles de matériaux partagés : normal, hovered et selected.

### 14.3 Double-clic

Implémenter une détection simple : mémoriser la date du dernier clic et l’identifiant cliqué, puis considérer comme double-clic deux clics sur le même objet dans une fenêtre de 350 ms.

### 14.4 Priorité de sélection

Dans la vue système :

1. lune ;
2. planète ;
3. étoile ;
4. aucun objet.

La ceinture d’astéroïdes n’a pas besoin d’être sélectionnable dans la première version.

---

## 15. Rendu

### 15.1 Configuration caméra

La caméra principale devrait utiliser :

- rendu 3D ;
- HDR si simple à activer ;
- bloom ;
- fond noir ou bleu-noir ;
- distance de clipping adaptée à la galaxie.

Éviter les effets coûteux non indispensables.

### 15.2 Matériaux

Préparer des fonctions de création :

```rust
fn star_material(class: StarClass) -> StandardMaterial;
fn planet_material(kind: PlanetKind) -> StandardMaterial;
fn moon_material() -> StandardMaterial;
fn selection_material() -> StandardMaterial;
```

Les étoiles utilisent une émission forte. Les planètes peuvent être légèrement rugueuses, sans textures dans la première version.

### 15.3 Éclairage

Vue galaxie : privilégier les matériaux émissifs ; une lumière ambiante très faible suffit.

Vue système : ajouter une `PointLight` au niveau de l’étoile et une lumière ambiante faible. S’assurer que la face non éclairée des planètes reste lisible sans devenir plate.

### 15.4 Orbites

Pour le POC, les orbites peuvent être dessinées avec des gizmos 3D chaque frame.

```rust
fn orbit_points(
    radius: f32,
    inclination: f32,
    segments: usize,
) -> impl Iterator<Item = Vec3>;
```

Valeurs recommandées :

```text
64 segments par orbite planétaire
32 segments par orbite lunaire
```

Si les gizmos provoquent un problème visuel ou de performance, remplacer par un mesh de ligne réutilisable.

### 15.5 Routes

Même approche que les orbites : gizmos ou lignes simples dans le POC, faible opacité et couleur renforcée pour les routes liées au système sélectionné.

### 15.6 Ceinture d’astéroïdes

Créer au maximum une ceinture par système.

Première implémentation :

- 100 à 300 petits astéroïdes ;
- meshes partagés ;
- transformations différentes ;
- rayon et hauteur bruités ;
- rotation individuelle facultative ;
- aucune collision.

Ne pas générer un mesh unique par astéroïde. Réutiliser un ou quelques handles de mesh.

---

## 16. Animation orbitale

### 16.1 Modèle

L’animation est purement visuelle.

```rust
let angle = orbit_phase + elapsed * orbit_speed;
let local = Vec3::new(
    orbit_radius * angle.cos(),
    0.0,
    orbit_radius * angle.sin(),
);
let inclined = inclination_rotation * local;
```

Les lunes utilisent la position de leur planète comme origine.

### 16.2 Hiérarchie

Option recommandée :

- planète enfant d’un pivot orbital ;
- pivot en rotation autour de l’étoile ;
- lune enfant d’un pivot rattaché à la planète.

Une mise à jour directe des transforms reste acceptable si elle simplifie le code.

### 16.3 Vitesse

Les vitesses doivent être stylisées et assez lentes pour permettre l’inspection.

La planète la plus proche peut effectuer une orbite en 20 à 40 secondes. Les planètes externes doivent tourner plus lentement.

---

## 17. Interface utilisateur

### 17.1 Disposition

```text
┌───────────────────────────────────────────────────────────────┐
│ Vue / graine / nombre de systèmes / FPS                      │
│                                                               │
│                  Zone de rendu 3D                             │
│                                             ┌───────────────┐ │
│                                             │ Inspecteur    │ │
│                                             │               │ │
│                                             └───────────────┘ │
│ Aide et raccourcis                                            │
└───────────────────────────────────────────────────────────────┘
```

### 17.2 Barre supérieure

Afficher :

- `GALAXIE` ou `SYSTÈME : <nom>` ;
- graine ;
- nombre de systèmes ;
- FPS ;
- état des options routes, orbites et labels.

### 17.3 Inspecteur système

Afficher :

- nom ;
- identifiant ;
- position ;
- classe d’étoile ;
- nombre de planètes ;
- nombre total de lunes ;
- présence d’une ceinture ;
- tags ;
- action « Ouvrir le système ».

### 17.4 Inspecteur planète

Afficher :

- nom ;
- type ;
- rayon visuel ;
- rayon orbital ;
- nombre de lunes ;
- habitabilité ;
- action « Focaliser ».

### 17.5 Inspecteur lune

Afficher :

- nom ;
- planète parente ;
- rayon ;
- rayon orbital ;
- action « Focaliser ».

### 17.6 Feedback

Afficher brièvement des notifications :

- « Galaxie générée — graine 42 » ;
- « Routes masquées » ;
- « Animation en pause » ;
- « Aucun système sélectionné ».

---

## 18. Performance et budgets

### 18.1 Cible principale

En build `--release`, viser 60 FPS à 1920 × 1080 sur une machine desktop milieu de gamme, avec 200 systèmes, bloom activé, routes visibles et interface visible.

Le POC ne doit pas chercher à garantir cette cible sur matériel intégré ancien.

### 18.2 Scénarios de test

Créer des presets :

```text
Small  : 50 systèmes
Default: 200 systèmes
Large  : 500 systèmes
Stress : 1 000 systèmes
```

Le preset `Default` est la référence qualitative. Le preset `Stress` sert uniquement au diagnostic.

### 18.3 Contraintes

- partager les meshes des systèmes ;
- partager les matériaux par classe ;
- ne pas créer de label permanent pour les 200 systèmes ;
- ne pas recalculer les routes chaque frame ;
- ne pas régénérer les meshes à chaque frame ;
- ne pas faire de recherche quadratique pendant le rendu ;
- ne pas logguer chaque entité à chaque frame.

### 18.4 Mesures

Le panneau debug `F3` doit afficher au minimum :

- FPS ;
- frame time ;
- nombre d’entités ;
- nombre de systèmes ;
- système sélectionné ;
- vue active.

---

## 19. Tests automatisés

### 19.1 Génération déterministe

```rust
#[test]
fn same_seed_produces_same_galaxy() {
    // Générer deux galaxies et comparer leurs données significatives.
}
```

### 19.2 Unicité des identifiants

Vérifier les IDs système, planète et lune.

### 19.3 Unicité des noms système

Tous les noms système doivent être distincts.

### 19.4 Bornes

Vérifier :

- nombre de planètes entre 2 et 9 ;
- nombre de lunes entre 0 et 4 ;
- habitabilité entre 0 et 100 ;
- rayons orbitaux strictement croissants ;
- positions finies ;
- absence de `NaN`.

### 19.5 Routes

Vérifier :

- pas de route vers soi-même ;
- pas de doublon inversé ;
- références vers des systèmes existants.

### 19.6 Régression de seed

Ajouter un test léger pour la graine `42` : nombre exact de systèmes, nom du premier système, classe de sa première étoile et nombre de ses planètes.

---

## 20. Gestion des erreurs

Le programme ne doit pas paniquer pour une interaction normale.

Utiliser des retours anticipés et des logs pour :

- sélection absente ;
- identifiant introuvable ;
- système sans donnée ;
- caméra absente ;
- fenêtre non disponible ;
- erreur d’asset non critique.

Une configuration invalide doit être corrigée ou rejetée :

- `arm_count >= 1` ;
- `system_count >= 1` ;
- `radius > 0` ;
- `min_system_distance >= 0`.

---

## 21. Journalisation

Utiliser les macros Bevy :

```rust
info!();
warn!();
error!();
debug!();
```

Logs attendus :

- démarrage et version ;
- graine ;
- durée de génération ;
- nombre de systèmes, routes et corps ;
- changement de vue ;
- erreurs non fatales.

Ne pas polluer la sortie avec les positions de chaque objet.

---

## 22. Étapes d’implémentation pour Codex

Codex doit suivre cet ordre.

### Étape 1 — Bootstrap

Livrables : projet compilable, fenêtre configurée, `AppPlugin`, arborescence, `README.md` et commandes qualité.

Critère : `cargo run` ouvre une scène 3D vide sans erreur.

### Étape 2 — Modèle de données

Livrables : identifiants, enums, structures galaxie/système/planète/lune, configuration et tests de base.

Critère : `cargo test` passe.

### Étape 3 — Génération procédurale

Livrables : galaxie spiralée, noms, systèmes, routes, déterminisme et logs de génération.

Critère : deux exécutions avec la graine 42 génèrent les mêmes données.

### Étape 4 — Vue galaxie

Livrables : systèmes visibles, fond spatial, couleurs par classe, routes et caméra cadrée.

Critère : la forme spiralée et son épaisseur 3D sont immédiatement perceptibles.

### Étape 5 — Caméra

Livrables : rotation, pan, zoom, lissage et limites.

Critère : aucun retournement brutal, clipping majeur ou perte du point de focalisation.

### Étape 6 — Picking et sélection

Livrables : hover, clic, halo, ressource `Selection` et focalisation avec `F`.

Critère : sélectionner une étoile au centre et une étoile en périphérie est fiable.

### Étape 7 — Interface

Livrables : barre supérieure, panneau inspecteur, aide, FPS et graine.

Critère : les informations de l’objet sélectionné correspondent aux données générées.

### Étape 8 — Vue système

Livrables : transition, spawn étoile/planètes/lunes, orbites, picking et retour galaxie.

Critère : le passage galaxie → système → galaxie fonctionne plusieurs fois sans fuite visible d’entités.

### Étape 9 — Animation et effets

Livrables : orbites animées, pause, lumière, émission, bloom et ceinture d’astéroïdes.

Critère : la scène reste lisible et stable à 60 FPS dans le preset par défaut sur la machine de développement.

### Étape 10 — Stabilisation

Livrables : formatage, Clippy, tests, documentation, nettoyage et presets de charge.

Critère : toutes les commandes de qualité passent.

---

## 23. Commandes de qualité

Avant livraison, exécuter :

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run --release
```

Ne pas désactiver un warning Clippy sans justification.

---

## 24. Critères d’acceptation

### Génération

- [ ] La galaxie contient 200 systèmes par défaut.
- [ ] La galaxie a une forme spiralée identifiable.
- [ ] Une épaisseur 3D est visible.
- [ ] La graine rend la génération déterministe.
- [ ] Les noms système sont uniques.
- [ ] Chaque système contient une étoile, des planètes et éventuellement des lunes.

### Navigation

- [ ] Rotation, pan et zoom fonctionnent.
- [ ] La caméra est fluide.
- [ ] `F` focalise l’objet sélectionné.
- [ ] La caméra ne se retourne pas brutalement.
- [ ] Les distances minimales et maximales sont respectées.

### Interaction

- [ ] Le hover est visible.
- [ ] Le clic sélectionne l’objet correct.
- [ ] Un système peut être ouvert.
- [ ] Une planète ou une lune peut être sélectionnée.
- [ ] Le retour à la galaxie fonctionne.

### Visuel

- [ ] Les classes d’étoiles sont visuellement distinctes.
- [ ] Les types de planètes sont visuellement distincts.
- [ ] Les orbites sont visibles mais discrètes.
- [ ] Le système sélectionné reste identifiable.
- [ ] Les labels ne saturent pas la vue.
- [ ] La galaxie paraît être un volume 3D et non une image plane.

### Robustesse

- [ ] Les changements de vue ne dupliquent pas les entités.
- [ ] `R` régénère proprement.
- [ ] `N` produit une nouvelle galaxie.
- [ ] Tous les tests passent.
- [ ] Clippy ne retourne aucun warning.
- [ ] Le README explique le lancement et les contrôles.

---

## 25. Définition de « terminé »

Le POC est terminé lorsque :

1. l’application compile sur Bevy 0.19 ;
2. elle fonctionne sans asset commercial ;
3. la galaxie 3D est visuellement convaincante à l’échelle du prototype ;
4. l’utilisateur peut entrer dans plusieurs systèmes successifs ;
5. les systèmes présentent plusieurs planètes et lunes ;
6. la sélection et la caméra sont suffisamment fiables pour une démonstration ;
7. la génération est reproductible ;
8. le code est organisé en plugins et modules ;
9. les tests et outils de qualité passent ;
10. les limites et améliorations futures sont documentées.

---

## 26. Améliorations après validation du POC

Ne traiter ces éléments qu’après acceptation :

- LOD et billboards avancés ;
- instancing GPU ;
- nuages de poussière galactique ;
- nébuleuses ;
- shaders personnalisés ;
- atmosphères planétaires ;
- anneaux de planètes ;
- étoiles binaires ;
- stations ;
- anomalies ;
- filtres de carte ;
- brouillard de guerre ;
- routes dépendant d’une portée technologique ;
- système de colonisation simplifié ;
- première boucle de gameplay ;
- sauvegarde ;
- IA.

---

## 27. Consignes finales à Codex

1. Lire la spécification complète avant de modifier le dépôt.
2. Implémenter les étapes dans l’ordre.
3. Maintenir le projet compilable après chaque étape.
4. Préférer une solution simple et lisible à une abstraction prématurée.
5. Séparer strictement les données procédurales et les entités visuelles.
6. Ne jamais utiliser `Entity` comme identifiant persistant.
7. Ne pas ajouter de gameplay hors périmètre.
8. Ne pas ajouter un moteur physique.
9. Documenter toute divergence par rapport à la spécification.
10. Terminer en exécutant les quatre commandes de qualité.
11. Fournir dans le README les prérequis, le lancement, les contrôles, l’architecture, les options de génération, les limites connues et des captures d’écran si disponibles.

Le meilleur résultat attendu n’est pas une simulation complexe. Il s’agit d’un explorateur galactique 3D simple, stable, lisible et suffisamment spectaculaire pour décider si cette direction visuelle mérite une phase de production.
