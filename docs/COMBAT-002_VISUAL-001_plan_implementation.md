# COMBAT-002 / VISUAL-001 — Plan d’implémentation du combat tactique et de l’identité visuelle

> **Projet :** Galactic  
> **Dépôt :** `cptblues/galactic`  
> **Base inspectée :** `main` — commit `2895a3c157eab602932ba790803f05b2f4aa2ed8` (`feat: upgrade fight & buildings`)  
> **Date de cadrage :** 15 août 2026  
> **Objectif :** faire évoluer le combat orbital vers une expérience de commandement visuelle, lisible et tactique, tout en introduisant un pipeline d’assets permettant un visuel distinct pour chaque vaisseau, défense, bâtiment et type/variante de planète.

---

## 1. Résumé de la direction retenue

Le système de combat actuel possède déjà une bonne base métier :

- combat déterministe ;
- résolution round par round ;
- dégâts simultanés ;
- groupes de combat distincts via `CombatStackId` ;
- intégrité et quantité par groupe ;
- classes de cible `Light`, `Medium`, `Heavy` ;
- rôle tactique `Line` / `Support` ;
- doctrines ;
- contres entre doctrines ;
- renseignement progressif ;
- IA tactique déterministe ;
- retraite ;
- auto-résolution ;
- combat en attente sérialisable ;
- finalisation stratégique atomique ;
- interface plein écran dédiée.

Le chantier ne doit donc **pas remplacer le moteur**.

La direction cible est :

```text
PRÉPARER
   ↓
ORGANISER LES GROUPES
   ↓
DONNER UN PLAN
   ↓
LANCER L’ASSAUT
   ↓
OBSERVER LE ROUND
   ↓
MAINTENIR LE PLAN OU INTERVENIR
   ↓
ASSUMER LES CONSÉQUENCES
```

Le joueur reste un commandant de flotte.

Le système ne doit pas devenir :

- un RTS ;
- un jeu de déplacement case par case ;
- une simulation physique ;
- un jeu de microgestion par vaisseau.

La profondeur doit venir de la préparation, des priorités, des réserves, du renseignement et de quelques interventions importantes.

---

# PARTIE I — ÉTAT ACTUEL

## 2. État du combat dans le dépôt

### 2.1 Modules principaux

Le moteur de combat est actuellement réparti notamment dans :

```text
crates/galactic_sim/src/combat.rs
crates/galactic_sim/src/combat/
├── ai.rs
├── doctrine.rs
├── intel.rs
├── retreat.rs
├── rounds.rs
├── session.rs
├── state.rs
└── view.rs
```

L’interface se trouve principalement dans :

```text
crates/galactic_client/src/combat_ui.rs
```

Le ruleset principal :

```text
assets/rulesets/default/combat.ron
```

La conception historique :

```text
docs/COMBAT-001_combat_orbital_tactique.md
```

---

## 2.2 Ce qu’il faut absolument conserver

### Source de vérité côté simulation

`galactic_sim` doit rester la seule source de vérité concernant :

- groupes ;
- quantités ;
- intégrité ;
- doctrines ;
- ordres ;
- résultat des rounds ;
- pertes ;
- retraite ;
- fin du combat.

Le client Bevy ne doit jamais calculer un résultat tactique.

Il ne doit que :

1. présenter l’état ;
2. construire une commande ;
3. envoyer la commande ;
4. recevoir le nouvel état / événement ;
5. l’animer.

---

### Résolution déterministe

Une même combinaison :

```text
snapshot
+ seed
+ plan initial
+ décisions
```

doit toujours produire le même résultat.

Cela reste essentiel pour :

- sauvegardes ;
- tests ;
- rapports ;
- debugging ;
- équilibrage ;
- future IA stratégique ;
- éventuel replay simplifié.

---

### Finalisation atomique

Les quantités de la flotte stratégique ne doivent toujours pas être modifiées après chaque round.

Le combat travaille sur son état intermédiaire puis applique le résultat final une seule fois.

---

## 2.3 Limite principale actuelle

Le moteur manipule déjà plusieurs groupes indépendants, mais le joueur prend actuellement surtout une décision globale :

```text
Choisir doctrine
→ Valider
→ Résoudre round
→ Lire résultat
→ Recommencer
```

L’objectif de COMBAT-002 est de passer à :

```text
Doctrine
+
organisation
+
rôle des groupes
+
priorités
+
réserve
+
interventions ponctuelles
```

sans ajouter de position physique réelle.

---

## 2.4 Limite visuelle actuelle

L’interface de combat dispose déjà de trois zones :

```text
25 % flotte alliée
50 % zone centrale
25 % ennemi
```

Mais la zone centrale sert encore surtout de support au journal textuel.

Elle doit devenir une **carte tactique animée**.

---

# PARTIE II — DÉCISIONS DE GAMEPLAY

## 3. Nouveau modèle mental du combat

Une bataille doit être divisée en quatre moments.

### Phase 1 — Briefing

Le joueur découvre :

- planète ;
- objectif ;
- flotte engagée ;
- forces connues ;
- niveau de renseignement ;
- estimation qualitative ;
- contraintes éventuelles.

### Phase 2 — Planification

Le joueur organise :

- doctrine initiale ;
- groupes tactiques ;
- rôle de chaque groupe ;
- priorité de cible ;
- réserve éventuelle.

### Phase 3 — Engagement

Le moteur exécute un round.

Le joueur observe :

- trajectoires ;
- tirs ;
- groupes engagés ;
- contre adverse ;
- pertes ;
- nouvelles informations.

### Phase 4 — Commandement

Le joueur choisit :

```text
[ Continuer le plan ]
```

ou utilise une intervention :

```text
[ Changer doctrine ]
[ Concentrer les tirs ]
[ Engager la réserve ]
```

Puis le round suivant commence.

---

# PARTIE III — COMBAT-002

# 4. COMBAT-002-A — Assainir et recalibrer le système existant

## Objectif

Stabiliser COMBAT-001 avant toute extension du modèle.

Aucun nouveau système majeur dans ce checkpoint.

---

## 4.1 Corriger la présentation de la répétition

Le moteur applique actuellement la pénalité de répétition à l’efficacité offensive.

L’interface ne doit pas présenter cette pénalité comme une réduction des dégâts reçus.

### À faire

Vérifier et harmoniser :

```text
combat/doctrine.rs
combat/rounds.rs
combat/view.rs
combat_ui.rs
```

Créer un type de preview dont les champs correspondent exactement à l’effet métier.

Exemple :

```rust
pub struct RepetitionPenaltyPreview {
    pub consecutive_uses_if_chosen: u8,
    pub outgoing_damage_multiplier_per_mille: u32,
}
```

---

## 4.2 Revoir les valeurs des doctrines

Le ruleset actuel doit être réévalué pour que chaque doctrine ait :

- un avantage visible ;
- un coût visible ;
- une situation d’usage claire ;
- un contre identifiable.

En particulier, vérifier `ConcentratedAssault`.

Le fantasme recommandé :

```text
plus offensif
+
meilleure concentration
-
plus exposé / moins flexible
```

Éviter qu’une doctrine soit simultanément :

```text
meilleure attaque
+
meilleure défense
```

sans contrepartie claire.

---

## 4.3 Réduire le nombre de rounds standard

Le ruleset actuel utilise un maximum élevé.

Valeur cible recommandée pour les playtests :

```text
5 ou 6 rounds
```

Le code reste totalement configurable.

Ne jamais introduire de constante métier supposant cinq rounds.

---

## 4.4 Vérifier les rôles réellement atteignables

`CombatTacticalRole::Support` existe déjà.

Vérifier quels craftables actuels peuvent effectivement être utilisés dans un combat et obtenir ce rôle.

Si le rôle Support reste non atteignable dans le ruleset actuel :

### Option recommandée

Préparer au moins un futur vaisseau militaire de soutien ou permettre à une catégorie explicitement choisie de participer au combat.

Ne pas forcer artificiellement les cargos civils à devenir des unités de combat uniquement pour activer la mécanique.

---

## 4.5 Ajouter des scénarios de balance reproductibles

Créer des fixtures couvrant :

### Duel équilibré

```text
Intercepteurs + Frégates
vs
défense moyenne
```

### Cible lourde

Vérifier l’intérêt d’Assaut concentré.

### Groupe endommagé

Vérifier Écran défensif.

### Répétition

Comparer :

```text
Assaut
Assaut
Assaut
```

avec :

```text
Assaut
Équilibré
Assaut
```

### Critères d’acceptation

- toutes les doctrines ont un cas utile ;
- les textes UI reflètent les calculs ;
- la répétition est comprise ;
- le nombre de rounds standard produit une bataille courte ;
- aucun changement de structure de sauvegarde inutile.

---

## 4.6 Script recommandé

```text
tools/apply_combat_002_a.py
```

Le script doit supporter :

```text
--dry-run
--root
--force
```

Créer :

```text
.combat002a-backup/<date>/
```

Puis lancer :

```bash
cargo fmt --all
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release
```

---

# 5. COMBAT-002-B — Introduire le plan de bataille

## Objectif

Ajouter une vraie préparation tactique sans ajouter de coordonnées spatiales.

---

## 5.1 Nouveau type `CombatPlan`

Ajouter dans la simulation un modèle ressemblant conceptuellement à :

```rust
pub struct CombatPlan {
    pub doctrine: CombatDoctrineId,
    pub groups: Vec<CombatGroupPlan>,
}
```

Puis :

```rust
pub struct CombatGroupPlan {
    pub id: CombatGroupPlanId,
    pub stacks: Vec<CombatStackId>,
    pub role: CombatGroupRole,
    pub target_priority: CombatTargetPriority,
}
```

---

## 5.2 Rôles de groupe V1

Limiter fortement la première version.

```rust
pub enum CombatGroupRole {
    Assault,
    Screen,
    Bombardment,
    Reserve,
}
```

### Assault

- participe pleinement à l’offensive ;
- suit sa priorité de cible ;
- exposition normale.

### Screen

- protège en priorité les groupes fragiles / soutien ;
- puissance offensive réduite ;
- détourne une partie des dégâts.

### Bombardment

- améliore la pression sur cibles lourdes / orbitales ;
- moins adapté contre unités légères ;
- peut être plus exposé à certains contres.

### Reserve

- ne fournit pas ou fournit très peu de puissance offensive ;
- subit moins de pression ;
- peut être engagée plus tard.

---

## 5.3 Priorités de cible

Première version :

```rust
pub enum CombatTargetPriority {
    Any,
    Light,
    Medium,
    Heavy,
    Damaged,
    Support,
}
```

Le choix ne désigne pas forcément une pile précise.

Il modifie les poids dans le système de ciblage existant.

---

## 5.4 Réutiliser `target_weights`

Ne pas introduire un deuxième système de ciblage.

Le pipeline devient conceptuellement :

```text
poids de base de la cible
×
effet doctrine
×
effet rôle du groupe attaquant
×
priorité configurée
×
effet défensif adverse
```

Puis la distribution actuelle continue.

---

## 5.5 Pas de division arbitraire des piles en V1

Si le joueur possède :

```text
12 Frégates — Garde
```

la V1 ne doit pas proposer :

```text
6 Garde dans Alpha
6 Garde dans Beta
```

Cela obligerait à créer des sous-identités et compliquerait :

- dégâts ;
- sauvegardes ;
- rapport ;
- réassemblage ;
- interface.

Pour la V1 :

> un `CombatStackId` appartient à un seul groupe tactique.

Une division de stack pourra devenir une extension future.

---

## 5.6 Plan par défaut

Pour que le joueur ne soit jamais bloqué par la préparation :

Créer automatiquement un plan valide.

Exemple :

```text
tous les groupes opérationnels
→ Groupe Alpha
→ Assault
→ Any
→ BalancedEngagement
```

Le bouton :

```text
LANCER L’ASSAUT
```

reste donc toujours disponible après validation du plan.

---

## 5.7 Validation métier

Le moteur doit refuser :

- stack inconnue ;
- stack présente dans deux groupes ;
- groupe vide ;
- groupe au-delà du maximum supporté ;
- priorité invalide ;
- plan pour un combat finalisé ;
- plan envoyé par une faction non autorisée.

---

## 5.8 Limite de groupes

V1 recommandée :

```text
maximum 3 groupes :
Alpha
Beta
Gamma
```

C’est suffisant pour créer :

```text
ligne principale
+
spécialiste
+
réserve
```

sans surcharger l’écran.

---

## 5.9 Tests

Ajouter au minimum :

- plan par défaut valide ;
- doublon de stack rejeté ;
- stack manquante gérée selon la règle choisie ;
- groupe réserve correctement identifié ;
- priorité Heavy modifie réellement la distribution ;
- Screen protège réellement une cible ;
- plan identique + seed identique = même résultat.

---

# 6. COMBAT-002-C — Commandes, état et sauvegarde

## Objectif

Intégrer la planification au modèle persistant.

---

## 6.1 Étendre les phases

Faire évoluer conceptuellement la phase :

```rust
FleetCombatPhase
```

vers quelque chose comme :

```rust
Planning,
AwaitingCommand,
Resolving,
Completed,
```

Attention :

`Resolving` peut rester un état extrêmement court côté métier.

Le client ne doit pas demander au moteur de rester dans un état instable pendant l’animation.

---

## 6.2 Nouvelle commande

Ajouter une action :

```rust
GameAction::ConfirmCombatPlan {
    mission_id,
    plan,
}
```

Éviter une succession de dix micro-commandes de configuration si le plan peut être validé atomiquement.

Le client peut éditer localement le draft.

La simulation ne reçoit le plan complet qu’à la validation.

---

## 6.3 Draft client vs plan métier

### Côté client

```text
CombatPlanDraft
```

modifiable librement.

### Côté simulation

```text
CombatPlan
```

validé.

Cela permet :

- annulation locale ;
- changement d’affectation instantané ;
- aucune mutation métier tant que le joueur n’a pas cliqué sur Lancer.

---

## 6.4 Sauvegarde

Le plan validé doit être sérialisé avec le `PendingCombat`.

Sauvegarder :

- doctrine active ;
- composition des groupes ;
- rôles ;
- priorités ;
- réserve ;
- points de commandement restants ;
- interventions déjà utilisées si nécessaire.

Le draft UI non validé n’a pas besoin d’être sauvegardé en V1.

Après reload :

- réouvrir le combat ;
- reconstruire le draft depuis le plan validé si celui-ci existe ;
- reprendre exactement au prochain choix métier stable.

---

# 7. COMBAT-002-D — Refaire la zone centrale en carte tactique

## Objectif

Faire du centre de l’écran le cœur de l’expérience.

---

## 7.1 Découper `combat_ui.rs`

Le fichier est déjà très volumineux.

Avant de continuer à l’étendre, créer un module :

```text
crates/galactic_client/src/combat_ui/
├── mod.rs
├── battlefield.rs
├── doctrine_panel.rs
├── group_panel.rs
├── report.rs
└── visual_effects.rs
```

Le découpage exact peut varier, mais éviter un unique fichier dépassant plusieurs milliers de lignes.

---

## 7.2 Composition visuelle recommandée

```text
┌──────────────────────────────────────────────────────────────────────┐
│ ASSAUT ORBITAL — KHEPRI IV             ROUND 1/6                    │
├──────────────────┬────────────────────────────────┬─────────────────┤
│ FLOTTE            │                                │ CONTACTS        │
│                   │          PLANÈTE               │                 │
│ [Garde] x12       │            ◉                   │ [???]           │
│ [Riposte] x8      │       ╭──────────╮             │ Carapace ≈ 2    │
│ [Verdict] x2      │       │ ORBITE   │             │ Floraison       │
│                   │       ╰──────────╯             │                 │
│ GROUPES           │                                │ INTEL 62 %      │
│ Alpha             │ Alpha ───────────────► cible   │                 │
│ Beta              │ Beta  ───────────────► cible   │                 │
│ Gamma — réserve   │ Gamma        [RÉSERVE]         │                 │
├──────────────────┴────────────────────────────────┴─────────────────┤
│ PLAN : Engagement équilibré                                         │
│ [LANCER L’ASSAUT]                                                    │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 7.3 La carte reste schématique

Les positions sont déterminées par le client à partir du rôle.

Exemple :

```text
Assault      → orbite intérieure
Screen       → proche du groupe protégé
Bombardment  → orbite extérieure
Reserve      → arrière de la formation
```

Ces positions :

- ne sont pas sauvegardées ;
- n’entrent pas dans les dégâts ;
- n’ont aucune unité physique ;
- servent uniquement à rendre le plan compréhensible.

---

## 7.4 Représentation des groupes

Chaque groupe affiche :

- nom Alpha/Beta/Gamma ;
- miniature des unités principales ;
- quantité totale ;
- rôle ;
- priorité ;
- intégrité agrégée ;
- état.

Ne pas utiliser uniquement une couleur.

Ajouter :

- forme ;
- icône ;
- texte.

---

## 7.5 Trajectoires

Utiliser des lignes ou arcs UI :

### attaque

```text
──────────►
```

### protection

```text
- - - - - ◯
```

### réserve

pas de trajectoire active.

### bombardement

arc plus long / pointillé.

Ces lignes sont générées à partir du draft du plan.

---

# 8. COMBAT-002-E — Interventions et points de commandement

## Objectif

Permettre de s’adapter sans refaire entièrement le plan à chaque round.

---

## 8.1 Ressource de commandement

Valeur de départ recommandée :

```text
3 PC
```

Le nombre doit être configurable.

Affichage :

```text
COMMANDEMENT   ● ● ●
```

---

## 8.2 Actions V1

Limiter à trois interventions.

### Changer de doctrine

Coût recommandé :

```text
1 PC
```

### Concentrer les tirs

Coût recommandé :

```text
1 PC
```

Le joueur choisit :

```text
Light
Medium
Heavy
Damaged
Support
```

pour un round.

### Engager la réserve

Coût recommandé :

```text
1 PC
```

Transforme un groupe `Reserve` en rôle actif.

Le changement est permanent pour les rounds restants sauf nouvelle intervention.

---

## 8.3 Continuer sans coût

L’action principale après un round devient :

```text
[ CONTINUER LE PLAN ]
```

C’est essentiel.

Le joueur ne doit pas être forcé d’ouvrir six cartes de doctrine toutes les trente secondes.

---

## 8.4 Futur possible

Ne pas implémenter maintenant, mais prévoir l’extension :

- repli d’un groupe ;
- changement de priorité ;
- couverture d’urgence ;
- brouillage ;
- surcharge offensive ;
- extraction d’une unité critique.

---

# 9. COMBAT-002-F — Résolution visuelle des rounds

## Objectif

Faire voir les conséquences de la simulation au lieu de les résumer uniquement par du texte.

---

## 9.1 Enrichir `CombatRoundRecord`

Aujourd’hui, le record contient surtout :

- doctrines ;
- dégâts globaux ;
- pertes ;
- événements.

Pour animer correctement, ajouter une représentation déterministe des échanges.

Exemple :

```rust
pub struct CombatStackExchange {
    pub attacker: CombatStackId,
    pub target: CombatStackId,
    pub allocated_damage: u128,
}
```

ou une structure plus compacte si plusieurs attaquants contribuent à un pool global.

Le but n’est pas de simuler chaque projectile.

Le but est de pouvoir dire :

```text
Alpha a principalement engagé Carapace
Beta a bombardé Floraison
Gamma est resté en réserve
```

---

## 9.2 Séquence d’animation

Durée cible :

```text
1,0 à 1,8 seconde
```

Séquence :

1. trajectoires actives s’illuminent ;
2. groupes attaquants pulsent ;
3. traits de tir / impulsions ;
4. groupes touchés flashent ;
5. perte affichée ;
6. intégrité mise à jour ;
7. événement majeur affiché ;
8. prochain état.

---

## 9.3 Toujours skippable

`Espace` doit terminer immédiatement l’animation.

Le résultat final doit rester lisible sans animation.

---

## 9.4 Effets légers

Pas de particules coûteuses obligatoires.

Privilégier :

- sprites ;
- opacity ;
- scaling ;
- lignes ;
- translation UI légère ;
- flash ;
- shake extrêmement discret.

Objectif :

```text
lisible > spectaculaire
```

---

# 10. COMBAT-002-G — IA tactique et rapports

## Objectif

Faire utiliser le même vocabulaire tactique au joueur et à l’IA.

---

## 10.1 IA

L’IA doit progressivement choisir :

- doctrine ;
- rôle de ses groupes ;
- priorités ;
- réserve ;
- intervention.

Ne pas chercher une IA optimale.

Préférer :

```text
profil
+
heuristiques
+
poids
+
quelques réactions
```

---

## 10.2 Profils futurs

### Consortium

- discipliné ;
- protège les unités coûteuses ;
- réserve stable ;
- changement de doctrine peu fréquent.

### Confins

- plan moins homogène ;
- plus opportuniste ;
- davantage de concentration locale.

### Sylve

- agressive ;
- accepte des pertes ;
- contournement ;
- pression sur les unités isolées ;
- formes orbitales visuellement organiques.

---

## 10.3 Rapports persistants

Le rapport final doit conserver suffisamment de données pour raconter la bataille.

Ajouter ou associer :

```text
round_history
plan initial
interventions
```

Le rapport peut alors afficher :

```text
Round 1 — Analyse tactique
Round 2 — Assaut concentré
Round 3 — Gamma engagé
Round 4 — Défenses orbitales détruites
Round 5 — Victoire
```

---

# PARTIE IV — VISUAL-001 : PIPELINE D’ASSETS

# 11. Objectif de la passe visuelle

Chaque entité importante doit devenir identifiable immédiatement sans devoir lire son nom.

Le joueur doit apprendre visuellement :

```text
silhouette
→ fonction
→ danger
→ faction
```

Le visuel doit être utilisé dans plusieurs endroits et non uniquement comme décoration.

---

# 12. Inventaire actuel à couvrir

## 12.1 Vaisseaux — 9 visuels distincts

Le ruleset actuel contient :

| ID | Nom | Catégorie |
|---|---|---|
| `light_probe` | Sonde — Œil | Probe |
| `cartographer_satellite` | Satellite — Veilleur | Probe |
| `light_cargo` | Caboteur — Relais | Transport |
| `meridian_carrier` | Porteur — Navette | Transport |
| `atlas_cargo` | Cargo — Chargeur | Transport |
| `needle_interceptor` | Intercepteur — Riposte | Military |
| `frigate_bulwark` | Frégate — Garde | Military |
| `bastion_cruiser` | Croiseur — Verdict | Military |
| `colony_ship` | Arche coloniale — Essor | Colony |

Chaque entrée doit posséder un visuel propre.

---

## 12.2 Bâtiments — 8 visuels distincts

| ID | Nom |
|---|---|
| `metal_mine` | Mine de métal |
| `crystal_extractor` | Extracteur de cristal |
| `fuel_refinery` | Raffinerie de carburant |
| `power_plant` | Centrale énergétique |
| `warehouse` | Entrepôt |
| `construction_center` | Centre de construction |
| `research_lab` | Laboratoire |
| `shipyard` | Chantier naval |

---

## 12.3 Forces et défenses — 13 visuels distincts

### Consortium

```text
consortium_security
consortium_bastion
```

### Confins

```text
confins_militia
confins_guard
local_bastion
confins_dock_sentry
confins_independent_battery
```

### Sylve

```text
sylve_thorn
sylve_stalker
sylve_carapace
sylve_root
sylve_bloom
sylve_ancient
```

Important :

les forces `Ground` et `Orbital` doivent être visuellement distinguables.

---

## 12.4 Planètes

Les types actuels exploités par le rendu procédural sont :

```text
Rocky
Ocean
Desert
Ice
GasGiant
Volcanic
```

Un simple visuel unique par type serait déjà une amélioration, mais la cible recommandée est plus ambitieuse :

> plusieurs variantes par type, choisies de manière déterministe par planète.

---

# 13. VISUAL-001-A — Créer l’architecture d’assets

## 13.1 Ne pas ajouter les chemins dans le ruleset métier

Éviter :

```ron
(
    id: "frigate_bulwark",
    image: "..."
)
```

directement dans `craftables.ron`.

Pourquoi :

- le chemin d’image n’est pas une règle métier ;
- un remplacement graphique ne doit pas modifier l’empreinte du ruleset ;
- un changement artistique ne doit pas invalider une sauvegarde ;
- `galactic_sim` doit pouvoir fonctionner sans rendu.

---

## 13.2 Créer un manifest visuel client

Proposition :

```text
assets/visuals/manifest.ron
```

Exemple :

```ron
(
    version: 1,

    craftables: {
        "light_probe": "visuals/ships/consortium/light_probe.png",
        "frigate_bulwark": "visuals/ships/consortium/frigate_bulwark.png",
    },

    buildings: {
        "metal_mine": "visuals/buildings/consortium/metal_mine.png",
    },

    planetary_forces: {
        "sylve_thorn": "visuals/forces/sylve/sylve_thorn.png",
    },
)
```

Les types exacts devront être adaptés à la manière dont les IDs sont désérialisés dans le projet.

---

## 13.3 Nouveau resource client

Créer par exemple :

```rust
#[derive(Resource)]
pub struct VisualAssets {
    ...
}
```

Ou séparer :

```rust
ShipVisualAssets
BuildingVisualAssets
ForceVisualAssets
PlanetVisualAssets
```

Recommandation :

un catalogue central + méthodes typées.

Exemple :

```rust
visuals.ship(craftable_id)
visuals.building(building_id)
visuals.force(force_id)
visuals.planet_preview(kind, variant)
```

---

## 13.4 Fallback obligatoire

Aucun asset manquant ne doit crasher le jeu.

Prévoir :

```text
assets/visuals/fallback/unknown_entity.png
assets/visuals/fallback/unknown_planet.png
```

En développement :

- log warning ;
- afficher fallback.

---

## 13.5 Validation du manifest

Au chargement, vérifier :

- doublons ;
- chemin vide ;
- version supportée ;
- ID inconnu ;
- entrée manquante pour un élément obligatoire.

Ajouter un test garantissant que tous les IDs du ruleset par défaut ont un visuel.

---

# 14. Organisation recommandée des fichiers

```text
assets/
└── visuals/
    ├── manifest.ron
    │
    ├── fallback/
    │   ├── unknown_entity.png
    │   └── unknown_planet.png
    │
    ├── ships/
    │   └── consortium/
    │       ├── light_probe.png
    │       ├── cartographer_satellite.png
    │       ├── light_cargo.png
    │       ├── meridian_carrier.png
    │       ├── atlas_cargo.png
    │       ├── needle_interceptor.png
    │       ├── frigate_bulwark.png
    │       ├── bastion_cruiser.png
    │       └── colony_ship.png
    │
    ├── buildings/
    │   └── consortium/
    │       ├── metal_mine.png
    │       ├── crystal_extractor.png
    │       ├── fuel_refinery.png
    │       ├── power_plant.png
    │       ├── warehouse.png
    │       ├── construction_center.png
    │       ├── research_lab.png
    │       └── shipyard.png
    │
    ├── forces/
    │   ├── consortium/
    │   ├── confins/
    │   └── sylve/
    │
    ├── planets/
    │   ├── rocky/
    │   ├── ocean/
    │   ├── desert/
    │   ├── ice/
    │   ├── gas_giant/
    │   └── volcanic/
    │
    └── effects/
        └── combat/
```

---

# 15. Format des illustrations

## Recommandation V1

Utiliser principalement :

```text
PNG
fond transparent pour entités
format carré
```

Résolution source recommandée :

```text
512 × 512
```

Cela permet d’utiliser le même asset :

- en carte ;
- en miniature ;
- en combat ;
- en inspecteur.

Éviter dans un premier temps de maintenir :

```text
icon_32
icon_64
portrait_256
portrait_512
```

pour chaque entité.

Bevy peut redimensionner une source unique.

Des versions optimisées pourront être ajoutées plus tard si le profiling le justifie.

---

# 16. Direction visuelle des catégories

## 16.1 Vaisseaux

Une silhouette doit permettre de reconnaître la fonction.

### Sonde

- petite ;
- capteurs visibles ;
- asymétrie possible ;
- faible masse.

### Satellite

- structure orbitale ;
- panneaux / antennes ;
- silhouette stationnaire.

### Caboteur

- cargo compact ;
- modules de soute visibles.

### Porteur

- forme plus longue ;
- conteneurs ;
- profil logistique.

### Cargo lourd

- massif ;
- très grande section centrale ;
- impression de lenteur.

### Intercepteur

- fin ;
- agressif ;
- moteurs dominants ;
- pointe.

### Frégate

- silhouette équilibrée ;
- blindage visible ;
- escorte.

### Croiseur

- lourd ;
- large ;
- avant fortement armé ;
- présence imposante.

### Arche coloniale

- très volumineuse ;
- modules d’habitation ;
- silhouette unique ;
- éviter qu’elle ressemble simplement à un cargo XXL.

---

## 16.2 Bâtiments

Les bâtiments doivent être compris même en petite carte.

### Mine métal

- puits ;
- excavatrices ;
- convoyeurs.

### Extracteur cristal

- structures autour de formations cristallines.

### Raffinerie

- réservoirs ;
- tuyauterie ;
- cheminées.

### Centrale

- cœur énergétique ;
- tours / échangeurs.

### Entrepôt

- hangars ;
- containers ;
- architecture large et basse.

### Centre de construction

- grues ;
- plateformes ;
- structures en assemblage.

### Laboratoire

- dômes ;
- antennes ;
- architecture plus propre / précise.

### Chantier naval

- docks ;
- bras de construction ;
- coque en assemblage.

---

## 16.3 Sylve

La Sylve ne doit pas donner l’impression d’utiliser des bâtiments humains repeints en vert.

Direction :

```text
croissance
chitine
fibres
organes
racines
bioluminescence
asymétrie
```

Les unités orbitales peuvent évoquer :

- spores ;
- fleurs ;
- organismes ;
- carapaces ;
- branches.

---

## 16.4 Confins

Direction :

```text
réemploi
assemblage
plaques rapportées
structures industrielles
silhouettes fonctionnelles
variations locales
```

---

# 17. VISUAL-001-B — Intégrer les assets de vaisseaux

## Points d’insertion

Les visuels doivent être utilisés au minimum dans :

### Chantier naval

Carte de craft.

### Gestion de flotte

Liste/composition.

### Assistant de mission

Flotte sélectionnée.

### Combat

Colonne alliée.

### Carte tactique

Token du groupe.

### Rapport de combat

Pertes et survivants.

---

## 17.1 Combat

Aujourd’hui le combat utilise une icône générique d’unité.

Remplacer progressivement cette représentation par :

```text
CombatUnitRef::Ship(id)
→ visual catalog
→ image du vaisseau
```

Pour un groupe composé de plusieurs stacks :

- afficher jusqu’à trois petites silhouettes ;
- mettre l’unité dominante au premier plan.

---

# 18. VISUAL-001-C — Intégrer les forces et défenses

## Objectif

Une Carapace, une Floraison et un Ancien doivent être reconnaissables sans lire leur nom.

---

## 18.1 Respect du renseignement

Point crucial :

un asset ne doit pas révéler une unité que le renseignement n’a pas identifiée.

Si `CombatStackView.identity == None` :

afficher :

```text
signature inconnue
```

avec un visuel générique.

Ne jamais faire :

```text
identity cachée
+
image exacte de Carapace
```

Ce serait une fuite d’information.

---

## 18.2 Niveaux de révélation possibles

### Identité inconnue

```text
silhouette brouillée / contact
```

### Classe connue

```text
silhouette générique Light / Medium / Heavy
```

### Identité révélée

asset exact.

Ainsi le visuel suit exactement les mêmes règles que le texte.

---

# 19. VISUAL-001-D — Visuels de bâtiments

## Points d’insertion

### Construction

Chaque carte de bâtiment affiche :

- illustration ;
- nom ;
- niveau ;
- effet ;
- coût ;
- temps ;
- prérequis.

### Vue colonie

Ajouter un aperçu du bâtiment sélectionné.

### File de construction

Petite miniature.

### Prérequis de recherche

Si une technologie exige :

```text
Laboratoire niveau 3
```

afficher la petite icône du laboratoire.

---

## 19.1 Les niveaux ne nécessitent pas dix images

Ne pas produire :

```text
metal_mine_lvl1.png
...
metal_mine_lvl10.png
```

dans la V1.

Une illustration par type suffit.

Le niveau est montré par :

- badge ;
- texte ;
- barre ;
- éventuel cadre.

Plus tard, on pourra avoir 2 ou 3 paliers visuels :

```text
Tier I
Tier II
Tier III
```

si cela apporte suffisamment de valeur.

---

# 20. VISUAL-001-E — Aperçus de planète

## Objectif

Éviter que toutes les planètes d’un même `PlanetKind` donnent l’impression d’être la même planète.

---

## 20.1 Conserver le rendu procédural 3D

Le projet dispose déjà d’un système de textures procédurales par `PlanetKind`.

Il est utile car il :

- ne dépend pas d’assets externes ;
- est léger ;
- s’adapte aux presets graphiques ;
- produit une texture pour la sphère.

Il ne faut pas nécessairement le supprimer.

---

## 20.2 Ajouter une couche `PlanetVisualVariant`

Créer côté client une variante déterministe.

Concept :

```rust
pub struct PlanetVisualVariant {
    pub index: u8,
}
```

Calculée à partir de :

```text
PlanetId
+ PlanetKind
```

Elle ne fait pas partie de la simulation économique.

Elle sert uniquement au rendu.

---

## 20.3 Plusieurs portraits par type

Cible raisonnable :

```text
3 variantes par type
```

Soit :

```text
6 types × 3 = 18 portraits
```

Exemple :

```text
assets/visuals/planets/ocean/ocean_01.png
assets/visuals/planets/ocean/ocean_02.png
assets/visuals/planets/ocean/ocean_03.png
```

Une planète donnée choisit toujours la même variante.

---

## 20.4 Où afficher le portrait de planète

### Inspecteur galaxie

Grand aperçu en haut.

### Gestion de colonie

Bannière / vignette principale.

### Mission

Cible de mission.

### Combat

Planète au centre de la carte tactique.

### Rapport

Petite vignette de la planète attaquée.

---

## 20.5 Différenciation supplémentaire

Sans produire une image unique pour toutes les planètes, combiner :

```text
portrait de base
+
anneau éventuel
+
atmosphère
+
teinte légère
+
occupation/faction en overlay
```

Attention :

la teinte de faction ne doit pas transformer la couleur naturelle de la planète.

Préférer un badge ou halo orbital.

---

## 20.6 Variante avancée future

À long terme, la texture procédurale 3D elle-même pourrait utiliser un seed lié à `PlanetId`.

Aujourd’hui, la génération est surtout dépendante du type.

Une évolution future pourrait faire :

```rust
procedural_planet_texture(
    kind,
    planet_visual_seed,
    preset,
)
```

pour produire réellement des surfaces différentes entre deux planètes océaniques.

Ce changement peut être réalisé indépendamment des portraits.

---

# 21. VISUAL-001-F — Passe visuelle spécifique au combat

## Assets recommandés

```text
effects/combat/laser_thin.png
effects/combat/projectile_heavy.png
effects/combat/impact.png
effects/combat/contact_unknown.png
effects/combat/orbit_marker.png
effects/combat/reserve_marker.png
```

Ne pas multiplier les effets avant d’avoir validé la lisibilité.

---

## 21.1 Carte tactique

Les assets exacts doivent apparaître seulement quand l’identité est connue.

Exemple :

```text
Alpha
[image Frégate]
[image Intercepteur]
```

contre :

```text
Contact lourd
[image silhouette Heavy inconnue]
```

---

# 22. Manifest visuel : proposition de structure

Exemple conceptuel :

```ron
(
    version: 1,

    ships: [
        (
            id: "light_probe",
            image: "visuals/ships/consortium/light_probe.png",
        ),
        (
            id: "needle_interceptor",
            image: "visuals/ships/consortium/needle_interceptor.png",
        ),
    ],

    buildings: [
        (
            id: "metal_mine",
            image: "visuals/buildings/consortium/metal_mine.png",
        ),
    ],

    forces: [
        (
            id: "sylve_thorn",
            image: "visuals/forces/sylve/sylve_thorn.png",
        ),
    ],

    planets: [
        (
            kind: Ocean,
            variants: [
                "visuals/planets/ocean/ocean_01.png",
                "visuals/planets/ocean/ocean_02.png",
                "visuals/planets/ocean/ocean_03.png",
            ],
        ),
    ],
)
```

Le format exact doit suivre les conventions serde/RON du projet.

---

# 23. Gestion des assets manquants

## En développement

Si une entrée est absente :

```text
WARN visual asset missing for frigate_bulwark
```

Puis fallback.

## En test

Le ruleset `default` doit être complet.

Créer un test :

```text
every_default_craftable_has_visual
every_default_building_has_visual
every_default_planetary_force_has_visual
every_planet_kind_has_preview
```

---

# PARTIE V — ORDRE D’EXÉCUTION RECOMMANDÉ

# 24. Roadmap concrète

Je recommande cet ordre.

---

## Étape 1 — COMBAT-002-A

**Assainissement et équilibrage.**

Pourquoi d’abord :

ne pas bâtir une nouvelle UI sur des effets encore incohérents.

---

## Étape 2 — VISUAL-001-A

**Créer le manifest et le loader visuel.**

Aucun changement massif d’UI.

Objectif :

pouvoir demander partout :

```rust
visuals.ship(id)
```

---

## Étape 3 — VISUAL-001-B minimal

Créer d’abord les 9 entrées vaisseaux avec placeholders différents si les illustrations finales ne sont pas encore prêtes.

Cela permet de développer les futurs écrans avec la bonne architecture.

---

## Étape 4 — COMBAT-002-B

Ajouter `CombatPlan`, groupes, rôles, priorités.

---

## Étape 5 — COMBAT-002-C

Commandes, validation, sauvegarde.

---

## Étape 6 — COMBAT-002-D

Construire la carte tactique en utilisant directement le nouveau `VisualAssets`.

---

## Étape 7 — COMBAT-002-E

Points de commandement et interventions.

---

## Étape 8 — COMBAT-002-F

Résolution visuelle.

---

## Étape 9 — VISUAL-001-C / D / E

Déployer le catalogue graphique sur :

- défenses ;
- bâtiments ;
- planètes ;
- autres panneaux.

---

## Étape 10 — COMBAT-002-G

IA, rapport détaillé et équilibrage final.

---

# 25. Découpage en scripts de migration

Pour rester cohérent avec la méthode actuelle du projet :

```text
tools/apply_combat_002_a.py
tools/apply_visual_001_a.py
tools/apply_visual_001_b.py
tools/apply_combat_002_b.py
tools/apply_combat_002_c.py
tools/apply_combat_002_d.py
tools/apply_combat_002_e.py
tools/apply_combat_002_f.py
tools/apply_visual_001_c.py
tools/apply_visual_001_d.py
tools/apply_visual_001_e.py
tools/apply_combat_002_g.py
```

Chaque script :

- idempotent ;
- `--dry-run` ;
- `--root` ;
- `--force` ;
- backup dédié ;
- refuse autant que possible un état source inattendu ;
- ne masque pas une erreur de compilation ;
- documente précisément les fichiers modifiés.

---

# 26. Contrôles obligatoires à chaque checkpoint

```bash
cargo fmt --all
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release
```

Éviter `--skip-checks`.

S’il existe, il doit rester une option de secours explicitement déconseillée.

---

# PARTIE VI — CRITÈRES D’ACCEPTATION

# 27. Combat

- [x] Le joueur voit sa flotte et la cible avant le premier round.
- [x] Le joueur peut organiser au maximum trois groupes.
- [x] Chaque stack appartient à un seul groupe.
- [x] Chaque groupe possède un rôle.
- [x] Chaque groupe possède une priorité de cible.
- [x] Le plan possède une doctrine active.
- [x] Le joueur peut lancer un plan par défaut sans microgestion.
- [x] Le plan est sauvegardable.
- [x] Un même plan est déterministe.
- [x] La réserve a un effet métier réel.
- [x] L’écran défensif a un effet métier réel.
- [x] Les priorités modifient réellement le ciblage.
- [x] Le joueur peut continuer le plan sans refaire un choix complet.
- [x] Les interventions consomment une ressource limitée.
- [x] Les animations ne modifient jamais le résultat.
- [x] La retraite reste disponible.
- [x] L’auto-résolution utilise toujours le même moteur.

---

# 28. Visuels

- [x] Chaque vaisseau actuel possède un visuel distinct.
- [x] Chaque bâtiment actuel possède un visuel distinct.
- [x] Chaque force/défense actuelle possède un visuel distinct.
- [x] Ground et Orbital sont immédiatement distinguables.
- [x] Les factions possèdent une identité graphique reconnaissable.
- [x] Un asset ennemi exact n’est jamais affiché si son identité est cachée.
- [x] Chaque type de planète possède plusieurs aperçus.
- [x] Une planète garde toujours la même variante visuelle.
- [x] L’absence d’un asset utilise un fallback.
- [x] Un asset manquant ne provoque jamais de panic.
- [x] Les chemins d’assets ne sont pas stockés dans le ruleset métier.
- [x] Les assets sont réutilisés dans plusieurs écrans.
- [x] L’interface reste correcte en 1280×720.
- [x] L’interface reste correcte en 1920×1080.

---

# 29. Tests visuels manuels recommandés

## 1280×720

Vérifier :

- cartes ;
- doctrine ;
- trois groupes ;
- portraits ;
- aucun overflow ;
- actions toujours visibles.

## 1920×1080

Vérifier :

- images suffisamment grandes ;
- centre tactique exploite l’espace ;
- pas de gigantesques zones vides.

## Faible renseignement

Vérifier :

- aucune image ennemie exacte ne fuit ;
- silhouettes inconnues cohérentes.

## Combat avec plusieurs types alliés

Vérifier :

- Intercepteur ;
- Frégate ;
- Croiseur ;

immédiatement différenciables.

## Colonie

Vérifier que les huit bâtiments ne ressemblent plus à huit lignes textuelles équivalentes.

---

# PARTIE VII — RISQUES ET GARDE-FOUS

# 30. Risque : transformer Galactic en RTS

Garde-fou :

```text
aucune coordonnée métier
aucune vitesse tactique
aucune portée calculée en pixels
aucun pathfinding
```

---

# 31. Risque : surcharger le joueur

Garde-fou :

```text
3 groupes maximum
4 rôles
6 priorités
3 PC environ
```

Le plan par défaut doit être immédiatement jouable.

---

# 32. Risque : trop d’assets à produire

L’inventaire initial représente déjà :

```text
9 vaisseaux
8 bâtiments
13 forces
18 portraits de planète si 3 variantes × 6 types
```

Total potentiel :

```text
48 illustrations
```

Il ne faut pas bloquer le développement en attendant 48 illustrations finales.

Approche recommandée :

### Phase A

assets placeholders uniques et nommés.

### Phase B

vaisseaux militaires + combat.

### Phase C

bâtiments.

### Phase D

forces Sylve / Confins.

### Phase E

portraits de planète.

L’architecture doit permettre de remplacer une image sans modifier le code.

---

# 33. Risque : fuite de renseignement par l’image

Le système de rendu doit toujours se baser sur la view filtrée.

Pour l’ennemi :

```text
EnemyIntelView
```

et jamais directement :

```text
PendingCombat.state.defender
```

Le choix d’image doit respecter :

```text
Unknown
→ silhouette inconnue

Class known
→ silhouette générique de classe

Identity known
→ asset exact
```

---

# 34. Risque : couplage des assets au ruleset

Ne pas faire dépendre `galactic_sim` du fichier de visuels.

Le serveur/sim doit pouvoir fonctionner sans :

```text
assets/visuals/
```

Le manifest graphique appartient au client.

---

# PARTIE VIII — CIBLE FINALE

# 35. Expérience recherchée

Le joueur sélectionne une planète hostile.

Il lance une flotte.

À l’arrivée, le jeu ouvre :

```text
ASSAUT ORBITAL — KHEPRI IV
```

Au centre se trouve un vrai aperçu de Khepri IV.

À gauche :

```text
12 × Frégate — Garde
8 × Intercepteur — Riposte
2 × Croiseur — Verdict
```

avec leurs vraies illustrations.

À droite :

```text
CONTACT LOURD
Floraison probable
Signature inconnue
```

dont les illustrations dépendent réellement du renseignement disponible.

Le joueur construit :

```text
ALPHA
Frégates
Assault
Priorité Heavy

BETA
Intercepteurs
Screen
Priorité Light

GAMMA
Croiseurs
Reserve
```

Il choisit :

```text
ENGAGEMENT ÉQUILIBRÉ
```

Puis :

```text
LANCER L’ASSAUT
```

Les trajectoires apparaissent.

Le round se résout.

Une Floraison est identifiée.

Alpha est endommagé.

Le jeu demande :

```text
PLAN MAINTENU

[ Continuer ]
[ Engager Gamma — 1 PC ]
[ Changer doctrine — 1 PC ]
```

Le joueur engage Gamma.

Les Croiseurs — Verdict apparaissent alors réellement dans l’orbite intérieure avec leur silhouette propre.

La bataille devient compréhensible par l’image avant même de lire le journal.

C’est la cible recommandée pour faire du combat de **Galactic** une mécanique de commandement qui reste cohérente avec le reste du jeu de gestion.

---

# 36. Prochain checkpoint recommandé

Commencer par :

```text
COMBAT-002-A
```

Puis :

```text
VISUAL-001-A
```

avant de construire le nouveau battlefield.

Cela garantit que :

1. les effets métier sont cohérents ;
2. le catalogue visuel existe ;
3. le nouveau modèle de plan peut ensuite être développé ;
4. l’interface tactique est construite directement sur les bonnes abstractions.

Le premier chantier ne devrait donc pas encore être « refaire l’écran ».

Il devrait être :

```text
stabiliser le combat
→ préparer le pipeline visuel
→ introduire le plan métier
→ seulement ensuite refaire l’écran
```
