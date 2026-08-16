# COMBAT-002-HOTFIX — Correctifs post-review du combat tactique

> **Projet :** Galactic  
> **Dépôt :** `cptblues/galactic`  
> **HEAD inspecté :** `d80afb66f5295ed4a511e72413a8389f8ce9a54d` — `feat: upgrade fight and visual`  
> **Parent :** `2895a3c157eab602932ba790803f05b2f4aa2ed8` — `feat: upgrade fight & buildings`  
> **Objectif :** verrouiller les invariants de COMBAT-002 avant de poursuivre les nouvelles features.
>
> Ce checkpoint doit rester un **hotfix de robustesse** : pas de nouveau système tactique, pas de nouvelle mécanique de combat, pas de refonte visuelle supplémentaire.

---

# 1. Résumé

Le HEAD actuel est globalement sain et va dans la bonne direction :

- combat ramené à 6 rounds ;
- `ConcentratedAssault` désormais réellement risqué ;
- plan de bataille Alpha / Beta / Gamma ;
- rôles `Assault`, `Screen`, `Bombardment`, `Reserve` ;
- priorités de cible ;
- 3 points de commandement ;
- `FocusFire` ;
- engagement de réserve ;
- historique des rounds ;
- échanges par groupe pour les animations ;
- catalogue d’assets séparé du ruleset ;
- visuels de vaisseaux, bâtiments, forces et planètes ;
- fallbacks respectant le renseignement.

La review a toutefois identifié plusieurs cas où les règles métier et l’interface peuvent diverger.

Les trois correctifs les plus importants sont :

1. **le plan peut être modifié gratuitement après le début du combat**, ce qui permet de contourner les points de commandement ;
2. **une pile détruite peut rendre le plan invalide au round suivant**, avec risque de combat bloqué ;
3. **`initial_plan` ne représente pas forcément le vrai plan effectivement lancé par le joueur**.

Ce hotfix doit corriger ces trois problèmes avant tout nouveau développement.

---

# 2. Priorités

| Priorité | Correctif | Gravité |
|---|---|---|
| P0 | Verrouiller l’édition libre du plan après le round 0 | Exploit gameplay |
| P0 | Rendre un plan valide malgré les piles détruites | Risque de combat bloqué |
| P1 | Snapshot correct de `initial_plan` | Rapport / persistance incorrects |
| P1 | Doctrine initiale gratuite et intégrée au plan | Cohérence gameplay |
| P1 | Interdire les interventions avant le premier round | Cohérence gameplay |
| P1 | Refuser l’engagement d’une réserve sans survivant | PC consommable inutilement |
| P1 | Resynchroniser le draft après un round | UI potentiellement obsolète |
| P1 | Empêcher le lancement avec un draft non confirmé | Risque d’exécuter un ancien plan |
| P2 | Renforcer la validation des combats au chargement | Robustesse sauvegarde |
| P2 | Optimiser les assets PNG | Mémoire / taille distribution |
| P2 | Ajouter une CI automatique | Qualité de livraison |

---

# 3. P0 — Verrouiller le plan après le début du combat

## Problème

`ConfirmCombatPlan` peut actuellement être envoyé tant qu’un combat est en attente.

La simulation vérifie :

- que le combat existe ;
- que le joueur est autorisé ;
- que le plan est valide.

Elle ne vérifie pas que le combat est encore au round 0.

Côté client, le panneau de planification est également actif lorsque :

```rust
CombatUiPhase::AwaitingDoctrine
```

Or cette phase est utilisée :

- avant le premier round ;
- entre deux rounds.

Le joueur peut donc, après le round 1 :

```text
Gamma : Reserve
        ↓ modification gratuite
Gamma : Assault
        ↓
Confirmer le plan
```

et éviter :

```text
Engager la réserve — 1 PC
```

Même problème pour une modification gratuite des priorités de cible.

## Correctif simulation

Ajouter une erreur explicite :

```rust
pub enum CombatCommandError {
    ...
    PlanLocked { round: u16 },
}
```

Dans :

```rust
confirm_combat_plan(...)
```

ajouter après l’autorisation :

```rust
if pending.round() > 0 {
    return Err(CombatCommandError::PlanLocked {
        round: pending.round(),
    });
}
```

Le verrouillage doit être imposé **dans `galactic_sim`**, même si le client désactive correctement ses boutons.

Le client ne doit jamais être la protection métier.

## Correctif UI

Le panneau Alpha / Beta / Gamma reste visible après le début de la bataille, mais devient **read-only**.

Après le round 0 :

```text
PLAN ACTIF

Alpha — Assault — Heavy
Beta  — Screen  — Light
Gamma — Reserve — Any

Les modifications passent par le commandement.
```

Désactiver :

- `AssignSelected`
- `CycleRole`
- `CyclePriority`
- `Confirm`
- `Reset`

Les groupes doivent rester consultables.

## Tests

### Simulation

```text
confirm_plan_before_first_round_is_allowed
confirm_plan_after_first_round_is_rejected
rejected_plan_change_does_not_mutate_pending_combat
```

### Client

```text
plan_buttons_are_enabled_at_round_zero
plan_buttons_are_disabled_after_round_zero
```

---

# 4. P0 — Ne pas invalider un plan lorsqu’une pile est détruite

## Problème

Dans `CombatPlan::validate_for_side`, les piles « connues » sont actuellement construites avec :

```rust
side.stacks
    .iter()
    .filter(|stack| stack.surviving_quantity > 0)
```

Cela implique qu’une pile détruite devient immédiatement une `UnknownStack`.

Scénario :

```text
Round 1

Alpha
- Frégates
- Intercepteurs

Les Intercepteurs sont détruits.

Plan persistant :
Alpha = [Frégates, Intercepteurs]
```

Au round suivant, la validation rencontre l’ID des Intercepteurs.

Mais cet ID n’est plus dans l’ensemble `known`.

Résultat possible :

```text
CombatPlanValidationError::UnknownStack(...)
```

Le joueur peut donc ne plus pouvoir lancer le round suivant alors que des unités sont toujours en vie.

## Correctif recommandé

Séparer conceptuellement :

```text
known stacks
```

et :

```text
required operational stacks
```

### `known`

Toutes les piles appartenant au combat, vivantes ou détruites.

```rust
let known: BTreeSet<_> = side
    .stacks
    .iter()
    .map(|stack| stack.stack_id)
    .collect();
```

### `required`

Uniquement les piles encore opérationnelles.

```rust
let required: BTreeSet<_> = side
    .stacks
    .iter()
    .filter(|stack| stack.surviving_quantity > 0)
    .map(|stack| stack.stack_id)
    .collect();
```

Puis :

- une référence hors de `known` → `UnknownStack` ;
- un doublon → `DuplicateStack` ;
- toute pile `required` doit être présente → `MissingStack` ;
- une pile détruite peut rester dans son groupe historique ;
- une pile détruite peut également être absente d’un plan runtime reconstruit.

Cette solution est préférable à supprimer systématiquement les IDs détruits du plan car elle :

- conserve l’identité historique du groupe ;
- ne casse pas `initial_plan` ;
- accepte les plans persistants existants ;
- simplifie les rapports ;
- évite les mutations artificielles du plan à chaque perte.

## Cas des groupes dont toutes les unités sont détruites

Un groupe :

```text
Gamma
stacks = [stack_7]
```

peut devenir non opérationnel si `stack_7` est détruit.

Il ne doit pas devenir une erreur métier.

Le groupe existe toujours dans le plan historique, mais :

```text
operational_stack_count == 0
```

Le moteur le traite simplement comme inactif.

## Tests

Ajouter dans `combat/plan.rs` :

```text
destroyed_stack_referenced_by_plan_is_still_known
plan_may_omit_a_destroyed_stack
all_operational_stacks_must_still_be_covered
truly_unknown_stack_is_rejected
destroyed_only_group_does_not_block_next_round
```

Ajouter un test intégration/session :

```text
combat_can_continue_after_one_attacker_stack_is_destroyed
```

Ce dernier est important : il doit réellement résoudre deux rounds.

---

# 5. P1 — Corriger le snapshot de `initial_plan`

## Problème

À la création du combat :

```rust
initial_plan: Some(default_plan.clone()),
plan: Some(default_plan),
```

Le rapport considère donc immédiatement le plan automatique comme plan initial.

Mais le joueur peut ensuite, avant le round 1 :

- déplacer des stacks ;
- changer les rôles ;
- changer les priorités ;
- choisir une autre doctrine.

Le rapport final risque donc de raconter :

```text
Plan initial : Balanced / Alpha Assault
```

alors que le joueur avait réellement lancé :

```text
ConcentratedAssault
Alpha Assault
Beta Screen
Gamma Reserve
```

## Correctif recommandé

À `begin_pending_combat` :

```rust
initial_plan: None,
plan: Some(default_plan),
```

Le `plan` reste utilisable immédiatement.

Mais `initial_plan` est seulement figé lorsque le **premier round est réellement lancé**.

Dans la préparation / exécution du round 1 :

```rust
if pending.state.round == 0 && pending.initial_plan.is_none() {
    pending.initial_plan = Some(prepared.persistent_plan.clone());
}
```

Le snapshot doit être pris :

1. après les choix gratuits de préparation ;
2. avant toute résolution du premier round ;
3. une seule fois.

Ensuite `initial_plan` devient immutable.

## Invariant

Après :

```text
round > 0
```

on doit avoir :

```text
initial_plan.is_some()
```

## Tests

```text
new_pending_combat_has_no_initial_plan_snapshot
first_round_snapshots_the_effective_plan
initial_plan_does_not_change_after_round_one
initial_plan_contains_the_selected_initial_doctrine
```

---

# 6. P1 — La doctrine initiale doit être gratuite

## Problème

Le plan automatique démarre sur :

```rust
BalancedEngagement
```

Le coût d’une doctrine est actuellement calculé en comparant la doctrine sélectionnée au plan persistant.

Ainsi, avant le premier round :

```text
Balanced → ConcentratedAssault
```

peut coûter :

```text
1 PC
```

Or la doctrine choisie avant le lancement de l’assaut fait partie de la **préparation initiale**, pas d’une intervention pendant la bataille.

Le joueur doit pouvoir choisir librement sa doctrine de départ.

## Règle cible

### Round 0

```text
choix de doctrine = gratuit
```

### Round >= 1

```text
changement de doctrine = coût configuré
```

## Correctif simulation

Dans la préparation de commande :

```rust
if let Some(doctrine) = doctrine
    && persistent_plan.doctrine != doctrine
{
    if pending.state.round > 0 {
        add_command_point_cost(
            &mut command_point_cost,
            command_rules.change_doctrine_cost(),
        );
    }

    persistent_plan.doctrine = doctrine;
}
```

La doctrine sélectionnée au round 0 doit ensuite être incluse dans `initial_plan`.

## Correctif client

`doctrine_change_cost(...)` doit retourner `0` au round 0.

Idéalement, lorsqu’une carte de doctrine est sélectionnée au round 0 :

- mettre à jour la doctrine du `CombatPlanDraft` ;
- marquer le draft `dirty`.

Après le début du combat :

- la carte devient une sélection de changement de doctrine payant.

Cela rend l’interface cohérente :

```text
PRÉPARATION
Doctrine initiale : libre
```

puis :

```text
COMMANDEMENT
Changer doctrine : 1 PC
```

## Tests

```text
initial_doctrine_change_costs_zero_command_points
doctrine_change_after_round_one_costs_command_points
initial_plan_snapshots_selected_doctrine
```

---

# 7. P1 — Interdire les interventions au round 0

## Problème

`FocusFire` et `CommitReserve` sont des interventions de commandement.

Elles ne devraient pas servir pendant la préparation.

Avant le round 1 :

- le joueur peut déjà régler la priorité de chaque groupe ;
- le joueur peut déjà choisir si Gamma est `Reserve` ou `Assault`.

Dépenser un PC avant que le combat ait commencé n’a donc pas de sens.

## Règle cible

```text
round == 0
→ aucune intervention de commandement
```

Les choix initiaux passent uniquement par `CombatPlan`.

```text
round >= 1
→ interventions disponibles
```

## Correctif simulation

Ajouter par exemple :

```rust
CombatInterventionError::CombatNotStarted
```

Dans `prepare_round_command` :

```rust
if pending.state.round == 0 && intervention.is_some() {
    return Err(CombatCommandError::InvalidIntervention(
        CombatInterventionError::CombatNotStarted,
    ));
}
```

## Correctif UI

Au round 0 :

```text
COMMANDEMENT ● ● ●
Disponible après le premier engagement.
```

Masquer ou désactiver :

- Focus Fire ;
- Engager réserve.

## Tests

```text
focus_fire_is_rejected_before_first_round
commit_reserve_is_rejected_before_first_round
interventions_are_available_after_first_round
```

---

# 8. P1 — Ne pas dépenser un PC pour une réserve détruite

## Problème

`CommitReserve` vérifie actuellement essentiellement :

```text
le groupe existe
le vecteur stacks n’est pas vide
le rôle est Reserve
```

Mais :

```text
stacks.len() > 0
```

ne signifie pas :

```text
au moins une unité encore opérationnelle
```

Une réserve peut avoir subi des dégâts et être entièrement détruite.

Le joueur ne doit pas pouvoir consommer 1 PC pour engager une réserve qui n’existe plus militairement.

## Correctif

Ajouter un helper :

```rust
fn group_has_operational_stack(
    side: &CombatSideState,
    group: &CombatGroupPlan,
) -> bool
```

Puis refuser l’intervention si aucun stack du groupe ne possède :

```rust
surviving_quantity > 0
```

Erreur recommandée :

```rust
CombatInterventionError::ReserveGroupInoperable(CombatGroupPlanId)
```

## Client

Le bouton :

```text
Engager Gamma
```

doit être désactivé si Gamma ne possède plus de survivant.

Afficher éventuellement :

```text
Gamma — réserve détruite
```

## Tests

```text
destroyed_reserve_cannot_be_committed
destroyed_reserve_does_not_spend_command_points
live_reserve_can_still_be_committed
```

---

# 9. P1 — Resynchroniser le draft après les rounds

## Problème

`sync_combat_plan_draft` ne reconstruit pas le draft si :

```text
mission_id identique
+
draft déjà présent
```

Après une modification métier du plan, par exemple :

```text
CommitReserve(Gamma)
Reserve → Assault
```

le plan métier peut changer sans que le draft client soit reconstruit.

Après une destruction, les piles survivantes changent également.

Le panneau peut donc présenter un état obsolète.

## Correctif recommandé

Ajouter à `CombatPlanDraftState` :

```rust
synced_round: Option<u16>,
```

éventuellement aussi :

```rust
synced_plan: Option<CombatPlan>,
```

Le draft doit être reconstruit si :

```text
mission change
OU
round change
OU
plan métier change alors que le draft n’a aucune édition locale non confirmée
```

Après le round 0, le plan est read-only : reconstruire systématiquement sur changement de round est donc sûr.

## Variante minimale

Comme le draft devient non éditable après le round 0 :

```rust
if pending.round() > 0 && state.synced_round != Some(pending.round()) {
    state.rebuild(mission_id, pending);
}
```

Cela suffit pour le hotfix.

## Tests

```text
draft_rebuilds_when_round_changes
committed_reserve_is_shown_as_assault_after_resolution
destroyed_stack_does_not_remain_as_an_active_draft_assignment
```

---

# 10. P1 — Ne jamais lancer un plan différent de ce qui est affiché

## Problème UX

Le joueur peut modifier le draft puis ne pas cliquer sur :

```text
Confirmer le plan
```

et lancer l’assaut via :

```text
Entrée
```

Le moteur utilise alors le dernier plan persistant, pas nécessairement les modifications actuellement affichées dans le draft.

Le joueur peut croire avoir préparé :

```text
Gamma = Reserve
```

alors que le moteur utilise encore :

```text
Gamma = Assault
```

## Correctif minimal recommandé

Si :

```text
round == 0
&& draft.dirty
```

le bouton de lancement devient indisponible.

Label :

```text
Confirmez le plan avant l’assaut
```

Et `Entrée` ne doit rien exécuter.

## Amélioration UX future

À terme, le meilleur flux serait :

```text
Lancer l’assaut
=
valider le draft
+
lancer le round 1
```

dans une seule intention utilisateur.

Mais pour ce hotfix, éviter d’introduire une nouvelle commande atomique est acceptable.

## Tests

```text
dirty_initial_plan_blocks_launch
confirmed_initial_plan_allows_launch
reset_plan_clears_dirty_state
```

---

# 11. P2 — Validation supplémentaire lors du chargement

## État actuel

La reconstruction vérifie déjà notamment :

- le round maximum ;
- collisions d’IDs ;
- hull dans les limites ;
- points de commandement dans les limites.

Le plan tactique devrait maintenant rejoindre ces invariants.

## Correctifs recommandés

Ajouter des méthodes de validation sur `PendingCombat`.

Exemples :

```rust
pub(crate) fn current_plan_is_valid(&self) -> bool
pub(crate) fn initial_plan_state_is_valid(&self) -> bool
```

### Invariants

Si `plan.is_some()` :

- aucun ID vraiment inconnu ;
- aucun doublon ;
- tous les stacks opérationnels couverts.

Si :

```text
state.round > 0
```

alors :

```text
initial_plan.is_some()
```

Si `initial_plan.is_some()` :

- IDs appartenant bien au combat ;
- groupes uniques ;
- aucun ID inventé.

## Nouvelles erreurs possibles

```rust
PendingCombatInvalidPlan(MissionId)
PendingCombatMissingInitialPlan(MissionId)
PendingCombatInvalidInitialPlan(MissionId)
```

## Tests persistence

```text
save_load_preserves_locked_combat_plan
save_load_preserves_initial_plan
save_load_preserves_command_points
corrupted_pending_plan_is_rejected
combat_can_continue_after_reload_with_destroyed_stack
```

Le dernier test est particulièrement important.

---

# 12. P2 — Optimisation des assets visuels

## État

Le pipeline d’assets est correctement séparé du ruleset.

Cependant les PNG sources actuels sont très grands pour des éléments souvent affichés entre 32 et 128 px.

Plusieurs fichiers dépassent largement 1 Mo.

Le catalogue demande également le chargement de l’ensemble du manifest au démarrage.

## Recommandation

Ne pas inclure une réarchitecture du loader dans ce hotfix.

Faire seulement une passe de préparation / mesure.

### Résolutions cibles

```text
vaisseaux        : 512 × 512 max
bâtiments        : 512 × 512 max
forces           : 512 × 512 max
fallback/contact : 128 à 256 px
planètes         : 768 × 768 max
```

Conserver les images HD originales hors des assets runtime si nécessaire.

## À mesurer

Avant / après :

```text
taille totale assets/visuals/
temps de démarrage
RAM après chargement
VRAM estimée
```

Ne pas optimiser à l’aveugle.

## Checkpoint séparé recommandé

```text
VISUAL-001-OPT
```

Ne pas mélanger ce travail au correctif métier du combat.

---

# 13. P2 — Ajouter une CI automatique

## État

Le workflow actuel de build playtest est manuel :

```yaml
workflow_dispatch
```

Il ne donne donc aucun statut au commit lors d’un simple push.

## Recommandation

Créer séparément :

```text
.github/workflows/ci.yml
```

Pour les `push` / `pull_request` :

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

Le `cargo build --release` peut rester :

- local à chaque checkpoint ;
- ou dans un job CI séparé si le coût est acceptable.

Le workflow playtest Windows/macOS reste manuel.

---

# 14. Ordre d’implémentation recommandé

Le hotfix doit être réalisé dans cet ordre :

## Étape 1

Corriger `CombatPlan::validate_for_side`.

Objectif :

```text
une perte de stack ne peut jamais bloquer le combat suivant
```

## Étape 2

Ajouter `PlanLocked`.

Objectif :

```text
aucune édition libre du plan après le premier round
```

## Étape 3

Corriger `initial_plan`.

Objectif :

```text
snapshot au moment réel du lancement
```

## Étape 4

Rendre la doctrine initiale gratuite.

Objectif :

```text
la préparation ne consomme pas de PC
```

## Étape 5

Interdire les interventions au round 0.

## Étape 6

Sécuriser `CommitReserve`.

## Étape 7

Verrouiller / resynchroniser le draft client.

## Étape 8

Empêcher un lancement avec un draft sale.

## Étape 9

Ajouter les invariants de reconstruction.

---

# 15. Fichiers principalement concernés

```text
crates/galactic_sim/src/combat/plan.rs
crates/galactic_sim/src/combat/session.rs
crates/galactic_sim/src/combat/view.rs
crates/galactic_sim/src/command.rs
crates/galactic_sim/src/event.rs
crates/galactic_sim/src/simulation/build_error.rs
crates/galactic_sim/src/simulation/reconstruction.rs

crates/galactic_client/src/combat_ui.rs
crates/galactic_client/src/combat_ui/group_panel.rs
```

Les modifications de :

```text
rounds.rs
```

doivent rester minimales voire nulles.

Le problème identifié concerne surtout :

```text
orchestration
validation
UI state
```

et non l’algorithme de dégâts.

---

# 16. Script de migration

Créer :

```text
tools/apply_combat_002_hotfix.py
```

Le script doit être idempotent.

Arguments :

```text
--dry-run
--root
--force
```

Éventuellement :

```text
--skip-checks
```

mais cette option doit être explicitement déconseillée.

## Backup

Créer :

```text
.mvp-combat002-hotfix-backup/<date>/
```

Cela reste couvert par :

```gitignore
.mvp*-backup/
```

## Préconditions du script

Le script doit vérifier autant que possible que le dépôt correspond au HEAD attendu.

Repères recommandés :

```text
combat rules version == 6
maximum_rounds == 6
CombatPlan existe
CombatIntervention existe
EntityVisualCatalog existe
```

Si les structures attendues diffèrent :

```text
refuser le patch
```

sauf :

```text
--force
```

Ne pas faire de remplacement aveugle de très grands blocs si une transformation ciblée par repères est possible.

---

# 17. Contrôles obligatoires

Après application :

```bash
cargo fmt --all
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release
```

Ne pas considérer le checkpoint validé si un de ces contrôles échoue.

---

# 18. Scénarios de test manuels

## Scénario A — Plan initial

Créer :

```text
Alpha = Assault
Beta = Screen
Gamma = Reserve
Doctrine = ConcentratedAssault
```

Vérifier :

- aucune dépense de PC ;
- le rapport final contient ce plan exact comme `initial_plan`.

## Scénario B — Tentative de triche après round 1

Après le premier round :

- tenter d’assigner une pile à Gamma ;
- tenter de changer Gamma Reserve → Assault ;
- tenter de changer une priorité de groupe.

Résultat attendu :

```text
édition libre impossible
```

Seules les interventions de commandement permettent un changement.

## Scénario C — Destruction partielle de la flotte

Avoir au moins deux stacks.

Faire détruire complètement un des deux au round 1.

Résultat attendu :

```text
le round 2 reste exécutable
```

Aucun :

```text
UnknownStack
```

## Scénario D — Réserve détruite

Gamma est en réserve.

Gamma est détruit avant d’être engagé.

Résultat attendu :

```text
Engager Gamma
```

désactivé.

Aucun PC ne peut être perdu.

## Scénario E — Draft non confirmé

Modifier une affectation sans confirmer.

Appuyer sur Entrée.

Résultat attendu :

```text
aucun lancement
+
message indiquant de confirmer le plan
```

## Scénario F — Sauvegarde

Sauvegarder :

```text
après round 1
avec une pile détruite
avec 2 PC restants
avec Gamma encore en réserve
```

Recharger.

Résultat attendu :

- plan cohérent ;
- Gamma toujours réserve ;
- 2 PC ;
- round suivant jouable ;
- historique conservé.

---

# 19. Critères d’acceptation

Le hotfix est terminé lorsque :

- [x] une pile détruite ne peut plus invalider le plan courant ;
- [x] le combat continue normalement après destruction d’un seul stack ;
- [x] `ConfirmCombatPlan` est impossible après le round 0 ;
- [x] les boutons de plan sont read-only après le round 0 ;
- [x] la doctrine initiale ne coûte aucun PC ;
- [x] un changement de doctrine après le round 1 coûte bien le coût configuré ;
- [x] `FocusFire` est impossible avant le premier round ;
- [x] `CommitReserve` est impossible avant le premier round ;
- [x] une réserve détruite ne peut pas être engagée ;
- [x] aucune intervention rejetée ne consomme de PC ;
- [x] `initial_plan` correspond au plan réellement lancé ;
- [x] `initial_plan` reste immutable ensuite ;
- [x] le draft se resynchronise après un round ;
- [x] un draft non confirmé ne peut pas être ignoré silencieusement au lancement ;
- [x] sauvegarde/reload en milieu de combat fonctionne après une destruction ;
- [x] `cargo fmt` passe ;
- [x] `cargo check` passe ;
- [x] `cargo clippy -D warnings` passe ;
- [x] `cargo test` passe ;
- [x] `cargo build --release` passe.

---

# 20. Hors scope du hotfix

Ne pas profiter de ce patch pour ajouter :

- nouvelles doctrines ;
- nouveaux rôles ;
- nouvelles priorités ;
- quatrième groupe ;
- nouvelle ressource tactique ;
- vraie position spatiale ;
- physique des projectiles ;
- IA de faction avancée ;
- refonte du battlefield ;
- nouveaux assets ;
- lazy loading complet des assets.

Ces sujets doivent rester dans leurs checkpoints dédiés.

---

# 21. État cible après le hotfix

Avant le combat :

```text
PLANIFICATION

Alpha — Assault
Beta  — Screen
Gamma — Reserve

Doctrine — ConcentratedAssault

PC : ● ● ●

[ CONFIRMER LE PLAN ]
[ LANCER L’ASSAUT ]
```

Aucun coût de commandement.

Après le round 1 :

```text
PLAN ACTIF — verrouillé

Alpha — Assault
Beta  — Screen
Gamma — Reserve

PC : ● ● ●

[ Continuer le plan ]
[ Focus Fire — 1 PC ]
[ Engager Gamma — 1 PC ]
[ Changer doctrine — 1 PC ]
```

Si Beta est détruit :

```text
Beta — DÉTRUIT
```

mais le round suivant reste parfaitement valide.

Si Gamma est détruit :

```text
Gamma — RÉSERVE DÉTRUITE
```

et :

```text
Engager Gamma
```

est indisponible.

Le rapport final doit ensuite pouvoir raconter exactement :

```text
Plan initial
↓
rounds
↓
interventions
↓
plan final
↓
résultat
```

sans divergence entre ce que le joueur a vu, ce que le moteur a utilisé et ce que la sauvegarde conserve.
