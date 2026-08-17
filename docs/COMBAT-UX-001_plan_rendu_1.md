# COMBAT-UX-001 — Plan d’implémentation pour atteindre le rendu 1

> **Projet :** Galactic  
> **Cible visuelle :** premier mockup “Nouvelle expérience : Assaut orbital”  
> **Objectif :** transformer l’écran de combat actuel en une expérience claire, progressive, visuelle et gratifiante, sans refaire le moteur de combat existant.

---

# 1. Vision cible

Le combat doit suivre une séquence claire :

```text
BRIEFING
↓
PLANIFICATION
↓
COMBAT
↓
INTERVENTION
↓
RÉSULTAT
↓
RAPPORT DÉTAILLÉ
```

À chaque étape, le joueur ne doit avoir qu’une décision principale à prendre.

Le système actuel possède déjà la plupart des briques métier :

- combat round par round ;
- groupes Alpha / Beta / Gamma ;
- rôles tactiques ;
- priorités de cible ;
- doctrines ;
- réserve ;
- points de commandement ;
- focus fire ;
- renseignement ;
- historique des rounds ;
- échanges par groupe ;
- rapport final.

Le chantier est donc principalement :

```text
UX
+
hiérarchisation de l’information
+
mise en scène
+
feedback visuel
```

---

# 2. Principes de design

## 2.1 Une décision à la fois

Ne pas afficher simultanément :

- briefing ;
- plan ;
- doctrines ;
- interventions ;
- rapport ;
- logs détaillés.

Chaque phase doit masquer ce qui n’est pas utile.

## 2.2 Le combat devient un mode dédié

Pendant le combat :

- masquer les panneaux stratégiques inutiles ;
- masquer ou fortement atténuer la navigation générale ;
- masquer le panneau de planète normal ;
- garder uniquement les ressources si elles restent utiles ;
- assombrir fortement le système solaire derrière ;
- réserver la majorité de l’écran au combat.

Le combat doit visuellement être un événement important.

## 2.3 La carte tactique devient l’élément principal

La carte ne doit plus être un simple cadre central.

Elle doit montrer :

- planète cible ;
- groupes alliés ;
- contacts ennemis ;
- trajectoires ;
- rôle des groupes ;
- priorité de cible ;
- état des groupes ;
- impacts du round.

## 2.4 Ne pas exposer directement les structures internes

Éviter une UX trop proche de :

```text
CombatPlan
CombatGroupRole
CombatTargetPriority
CombatIntervention
```

L’utilisateur doit lire :

```text
Alpha — Assaut
Priorité : Défenses lourdes
```

et non réfléchir aux concepts techniques sous-jacents.

---

# 3. Nouvelle machine d’état UI

Introduire une phase UI plus explicite.

Exemple :

```rust
pub enum CombatUiPhase {
    Briefing,
    Planning,
    ExecutingRound,
    RoundSummary,
    FinalResult,
    DetailedReport,
}
```

La simulation métier reste indépendante.

Cette enum est principalement client-side.

---

# 4. Phase 1 — Briefing

## Objectif

Faire comprendre immédiatement :

```text
où ?
pourquoi ?
avec quoi ?
contre quoi ?
avec quel niveau d’incertitude ?
```

## 4.1 Contenu

Afficher :

### Titre

```text
ASSAUT ORBITAL
Hélianthe d
```

### Objectif

```text
Neutraliser les défenses orbitales
```

### Flotte

```text
5 Croiseurs
5 Frégates
5 Intercepteurs
```

avec miniatures.

### Renseignement

```text
25 %
```

### Estimation ennemie

```text
4 contacts détectés
Composition incertaine
Forces estimées : moyennes
```

### CTA

```text
[ PRÉPARER L’ASSAUT ]
```

## 4.2 Pas encore visible

Ne pas montrer :

- points de commandement ;
- focus fire ;
- engagement de réserve ;
- six doctrines complètes ;
- rapport détaillé ;
- chronologie.

---

# 5. Phase 2 — Planification

## Objectif

Permettre au joueur de comprendre son plan par l’image.

## 5.1 Layout recommandé

```text
┌────────────┬──────────────────────────────┬──────────────┐
│ VOS FORCES │       CARTE TACTIQUE         │ PARAMÈTRES   │
│            │                              │ DU GROUPE    │
│ Alpha      │         planète              │              │
│ Beta       │                              │ Rôle         │
│ Gamma      │ Alpha ───────► Ennemi       │ Priorité     │
│            │ Beta  ───────► Ennemi       │              │
│            │ Gamma : réserve              │ Doctrine     │
└────────────┴──────────────────────────────┴──────────────┘
```

## 5.2 Groupes Alpha / Beta / Gamma

Afficher des cartes avec :

```text
ALPHA
Croiseurs x5
ASSAUT
Priorité : lourd
```

```text
BETA
Frégates x5
ÉCRAN
Priorité : léger
```

```text
GAMMA
Intercepteurs x5
RÉSERVE
```

Chaque groupe doit avoir :

- couleur / accent propre ;
- icône ;
- unités ;
- rôle ;
- priorité.

## 5.3 Sélection d’un groupe

Quand Alpha est sélectionné, le panneau de droite affiche uniquement les paramètres d’Alpha.

### Rôle

```text
[ Assaut ]
[ Écran ]
[ Bombardement ]
[ Réserve ]
```

### Priorité

```text
[ Toutes ]
[ Légères ]
[ Moyennes ]
[ Lourdes ]
[ Endommagées ]
[ Soutien ]
```

Ne plus afficher trois petits boutons “Assigner / Rôle / Cible” sans contexte.

## 5.4 Affectation des unités

Pour le MVP :

```text
stack sélectionné
+
gros boutons Alpha / Beta / Gamma
```

Le drag & drop pourra venir plus tard.

---

# 6. Doctrine initiale simplifiée

Dans l’écran principal, afficher seulement :

```text
PRUDENT
ÉQUILIBRÉ
AGRESSIF
```

Correspondance possible :

```text
Prudent
→ DefensiveScreen

Équilibré
→ BalancedEngagement

Agressif
→ ConcentratedAssault
```

Puis :

```text
Tactiques avancées ▼
```

ouvre :

- Analyse tactique ;
- Contournement ;
- Formation dispersée ;
- autres doctrines existantes.

Le moteur conserve les doctrines actuelles.

---

# 7. Plan recommandé

Ajouter :

```text
[ ★ PLAN RECOMMANDÉ ]
```

Fonction :

- construit automatiquement un plan valide ;
- Alpha = groupe principal ;
- Beta = écran si pertinent ;
- Gamma = réserve si plusieurs groupes existent ;
- doctrine équilibrée ;
- priorités cohérentes.

Le joueur débutant doit pouvoir jouer sans comprendre toute la profondeur.

---

# 8. Lancer l’assaut

Bouton principal :

```text
[ LANCER L’ASSAUT ]
```

Il doit être très visible.

Avant lancement :

- draft confirmé ;
- doctrine initiale gratuite ;
- aucune intervention possible ;
- aucune dépense de PC.

---

# 9. Phase 3 — Exécution du round

## Objectif

Faire disparaître l’interface de configuration et montrer l’action.

La majorité de l’écran devient :

```text
carte tactique animée
```

Afficher :

- planète ;
- formations ;
- contacts ;
- tirs ;
- impacts ;
- dégâts ;
- apparition de nouveaux contacts.

## 9.1 Animation

Durée cible :

```text
1 à 2 secondes
```

Séquence :

```text
1. trajectoires s’illuminent
2. groupes attaquent
3. projectiles / traits
4. impacts
5. pertes
6. intégrité mise à jour
7. événement important
```

Toujours skippable.

## 9.2 Utiliser les données déjà disponibles

Réutiliser :

```text
CombatStackExchange
CombatStackLoss
CombatRoundRecord
```

Ne jamais simuler visuellement un événement différent du résultat métier.

---

# 10. Phase 4 — Résumé / intervention

Après chaque round :

```text
ROUND 2 TERMINÉ
```

Afficher un résumé très court.

Exemple :

```text
Alpha      82 %    -2 unités
Beta       94 %    -1 unité
Gamma      100 %   Réserve

ENNEMI
2 détruits
1 endommagé
1 nouveau contact
```

## 10.1 Points de commandement

Les PC apparaissent seulement après le premier engagement.

```text
POINTS DE COMMANDEMENT

● ● ○
2 disponibles
```

## 10.2 Interventions

Ne montrer que les interventions réellement pertinentes.

Exemple :

```text
[ FOCUS LOURD — 1 PC ]
[ ENGAGER GAMMA — 1 PC ]
[ CHANGER DOCTRINE — 1 PC ]
```

Toujours afficher :

```text
[ CONTINUER LE PLAN ]
```

comme action principale gratuite.

## 10.3 Contextualiser les interventions

Ne pas afficher :

```text
Réserve Alpha
Réserve Beta
Réserve Gamma
```

si seul Gamma est réellement en réserve.

Ne pas afficher :

```text
Focus soutien
```

si aucun soutien ennemi n’est connu.

L’UI doit filtrer selon :

- renseignement ;
- plan courant ;
- état des groupes ;
- PC disponibles.

---

# 11. Plan actif read-only

Après le round 0, les groupes restent visibles, mais non modifiables gratuitement.

Afficher :

```text
PLAN ACTIF
```

et non :

```text
PLAN DE BATAILLE MODIFIABLE
```

Les changements passent uniquement par les interventions.

---

# 12. Phase 5 — Résultat

## Objectif

Faire du résultat un moment gratifiant.

L’écran actuel de texte doit devenir secondaire.

## 12.1 Écran victoire

Afficher en priorité :

```text
VICTOIRE

HÉLIANTHE d
Défenses orbitales neutralisées
```

avec la planète en grand.

## 12.2 Résumé clair

### Votre flotte

```text
15 engagés
14 survivants
1 perdu
```

### Ennemi

```text
4 groupes détectés
4 neutralisés
0 en fuite
```

## 12.3 Pertes

Afficher visuellement :

```text
1 × Intercepteur — Riposte
```

avec asset correspondant.

## 12.4 Butin

Afficher comme cartes / blocs :

```text
+324 Métal
+153 Cristal
+54 Carburant
```

avec les icônes déjà disponibles.

## 12.5 Moment décisif

Ajouter une phrase synthétique.

Exemple :

```text
MOMENT DÉCISIF

Round 3
Alpha a percé les défenses lourdes.
```

V1 : prendre le round ayant causé le plus de dégâts ou la première destruction majeure.

## 12.6 Actions finales

```text
[ VOIR LE RAPPORT DÉTAILLÉ ]
[ CONTINUER ]
```

`Continuer` doit être le CTA principal.

---

# 13. Défaite / retraite

Même structure.

## Défaite

```text
DÉFAITE
Flotte repoussée
```

## Retraite

```text
RETRAITE
Flotte extraite du combat
```

Toujours afficher :

- pertes ;
- survivants ;
- conséquence ;
- éventuel butin récupéré ;
- retour flotte.

---

# 14. Phase 6 — Rapport détaillé

Le rapport détaillé devient optionnel.

## Onglets

```text
[ RÉSUMÉ ]
[ DÉROULEMENT ]
[ STATISTIQUES ]
[ UNITÉS ]
```

## Résumé

Afficher :

- résultat ;
- durée ;
- nombre de rounds ;
- pertes totales ;
- dégâts infligés ;
- dégâts reçus.

## Déroulement

Chronologie :

```text
00:00 Engagement commencé
00:15 Alpha touche le contact lourd
00:17 Contact moyen détruit
00:32 Gamma engagé
00:48 Contact lourd détruit
01:37 Victoire
```

## Statistiques

Afficher :

- dégâts par groupe ;
- pertes par groupe ;
- doctrine utilisée ;
- interventions ;
- efficacité du plan.

## Unités

```text
Croiseurs      5 engagés   5 survivants
Frégates       5 engagées  4 survivantes
Intercepteurs  5 engagés   5 survivants
```

---

# 15. Nettoyage de l’écran

Pendant le combat :

## Masquer

- panneau de planète stratégique ;
- onglets colonie ;
- menus secondaires ;
- texte superposé derrière ;
- éléments non liés au combat.

## Garder éventuellement

- ressources ;
- bouton retour uniquement si autorisé ;
- navigation minimale.

---

# 16. Fond

Réutiliser le système solaire comme arrière-plan, mais :

```text
opacity très faible
+
overlay noir
+
léger blur si possible
```

La planète cible peut rester visible pour garder la continuité avec la carte.

---

# 17. Assets

Réutiliser les assets existants.

## Alliés

```text
needle_interceptor
frigate_bulwark
bastion_cruiser
etc.
```

## Ennemi

Respecter le renseignement :

```text
contact_unknown
contact_light
contact_medium
contact_heavy
```

Asset exact seulement si identité révélée.

---

# 18. Carte tactique

Créer des tokens visuels plutôt que de grandes cartes rectangulaires.

Exemple :

```text
[ image croiseur ]
ALPHA
5 unités
82 %
```

Les groupes doivent occuper une petite zone autour de la planète.

---

# 19. Trajectoires

Ajouter :

```text
ligne continue
→ attaque

ligne pointillée
→ écran / couverture

arc
→ bombardement

pas de ligne
→ réserve
```

La couleur doit rester secondaire au symbole / texte.

---

# 20. Feedback des dégâts

Au moment de l’impact :

```text
-18 % intégrité
```

ou :

```text
-2 unités
```

pendant une courte durée.

---

# 21. Renseignement

Le niveau de renseignement doit être visible dans le briefing et dans le combat.

Exemple :

```text
RENSEIGNEMENT 25 %
```

Mais éviter de répéter l’information dans trois endroits.

---

# 22. Tutoriel premier combat

Ajouter un onboarding en 3 étapes.

## Étape 1

```text
ORGANISEZ VOTRE FLOTTE

Placez vos Croiseurs dans Alpha.
```

## Étape 2

```text
GARDEZ UNE RÉSERVE

Placez vos Intercepteurs dans Gamma.
```

## Étape 3

```text
CHOISISSEZ UNE POSTURE

Engagement équilibré recommandé.
```

Chaque étape met visuellement en évidence seulement la zone concernée.

Après le premier combat, le tutoriel disparaît.

Conserver :

```text
Plan recommandé
```

pour les joueurs qui veulent aller vite.

---

# 23. Découpage technique recommandé

## COMBAT-UX-001-A — Phases UI

Créer :

```text
Briefing
Planning
ExecutingRound
RoundSummary
FinalResult
DetailedReport
```

Objectif : masquer / afficher les bons contrôles.

## COMBAT-UX-001-B — Briefing

Créer le nouvel écran initial.

Aucun changement métier.

## COMBAT-UX-001-C — Planification visuelle

Refaire :

```text
group_panel
battlefield
doctrine selection
```

avec :

- sélection groupe ;
- panneau paramètres ;
- trajectoires ;
- plan recommandé.

## COMBAT-UX-001-D — Simplification doctrines

Ajouter :

```text
Prudent
Équilibré
Agressif
Tactiques avancées
```

Mapping vers les doctrines existantes.

## COMBAT-UX-001-E — Exécution immersive

Masquer les contrôles de planification pendant l’animation.

Utiliser :

```text
CombatStackExchange
CombatStackLoss
```

pour les effets.

## COMBAT-UX-001-F — Round summary

Créer le panneau :

```text
ROUND TERMINÉ
```

avec :

- état groupes ;
- état ennemi ;
- interventions contextuelles ;
- continuer le plan.

## COMBAT-UX-001-G — Résultat

Remplacer le rapport brut initial par :

```text
Victoire / Défaite / Retraite
+
pertes
+
butin
+
conséquence
+
moment décisif
```

## COMBAT-UX-001-H — Rapport détaillé

Déplacer les données techniques vers un écran secondaire.

## COMBAT-UX-001-I — Tutoriel combat

Ajouter le tutoriel guidé du premier combat.

---

# 24. Proposition d’arborescence

```text
crates/galactic_client/src/combat_ui/
├── mod.rs
├── briefing.rs
├── planning.rs
├── battlefield.rs
├── group_panel.rs
├── doctrine_panel.rs
├── round_execution.rs
├── round_summary.rs
├── final_result.rs
├── detailed_report.rs
└── tutorial.rs
```

Ne pas forcément créer tous les fichiers immédiatement.

Mais éviter de continuer à concentrer toute la logique dans un seul gros fichier.

---

# 25. Source de vérité

Toujours conserver :

```text
galactic_sim
```

comme source métier.

Le client :

```text
présente
anime
filtre
guide
```

Il ne :

```text
calcule pas les dégâts
n’invente pas les pertes
ne décide pas des résultats
```

---

# 26. Script de migration

Chaque checkpoint suit la méthode habituelle.

Exemple :

```text
tools/apply_combat_ux_001_a.py
tools/apply_combat_ux_001_b.py
tools/apply_combat_ux_001_c.py
...
```

Chaque script :

```text
--dry-run
--root
--force
```

Backup :

```text
.mvp-combat-ux-001-x-backup/<date>/
```

---

# 27. Checks obligatoires

À chaque checkpoint :

```bash
cargo fmt --all
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release
```

---

# 28. Critères d’acceptation UX

Le rendu cible est atteint lorsque :

- [ ] le joueur comprend immédiatement l’objectif de la bataille ;
- [ ] le premier écran ne montre pas les interventions avancées ;
- [ ] la planification est visuelle ;
- [ ] Alpha / Beta / Gamma sont reconnaissables au premier coup d’œil ;
- [ ] le rôle de chaque groupe est compréhensible sans lire une documentation ;
- [ ] la cible prioritaire est visible sur la carte ;
- [ ] un plan recommandé permet de lancer rapidement le combat ;
- [ ] la doctrine initiale est simple à choisir ;
- [ ] les tactiques avancées restent accessibles sans surcharger l’écran ;
- [ ] pendant la résolution, la configuration disparaît ;
- [ ] le joueur voit clairement qui attaque quoi ;
- [ ] les pertes sont visibles dans la scène ;
- [ ] après un round, le joueur sait ce qui a changé ;
- [ ] les PC apparaissent seulement quand ils deviennent utiles ;
- [ ] les interventions sont contextuelles ;
- [ ] « Continuer le plan » est toujours évident ;
- [ ] la victoire donne une vraie sensation de récompense ;
- [ ] le butin est immédiatement visible ;
- [ ] les pertes alliées sont immédiatement visibles ;
- [ ] le rapport détaillé est secondaire ;
- [ ] le reste de l’interface stratégique ne pollue plus visuellement le combat.

---

# 29. Critères d’acceptation premier combat

Un joueur qui ne connaît pas le système doit pouvoir :

```text
1. comprendre la situation
2. utiliser le plan recommandé
3. lancer le combat
4. voir ce qui se passe
5. comprendre pourquoi une intervention est proposée
6. finir la bataille
7. comprendre le résultat
```

sans devoir lire un document externe.

---

# 30. Ce qui ne doit pas être changé pour atteindre ce rendu

Pas besoin de refaire :

- calcul de dégâts ;
- seed ;
- moteur round ;
- groupes ;
- doctrines métier ;
- priorité de ciblage ;
- réserve ;
- points de commandement ;
- historique.

Le rendu 1 est principalement atteignable avec :

```text
réorganisation UI
+
phases claires
+
mise en scène
+
animations légères
+
meilleure hiérarchie
```

---

# 31. Ordre recommandé

```text
COMBAT-002-HOTFIX
↓
COMBAT-UX-001-A — phases
↓
COMBAT-UX-001-B — briefing
↓
COMBAT-UX-001-C — planification
↓
COMBAT-UX-001-D — doctrines simplifiées
↓
COMBAT-UX-001-E — résolution
↓
COMBAT-UX-001-F — intervention
↓
COMBAT-UX-001-G — résultat
↓
COMBAT-UX-001-H — rapport
↓
COMBAT-UX-001-I — tutoriel
```

---

# 32. Priorité de développement

Si le temps est limité, les trois changements les plus rentables sont :

```text
1. Séparer Briefing / Planning / Combat / Result
2. Refaire le résultat de bataille
3. Masquer les contrôles inutiles selon la phase
```

Même avant toutes les animations, ces trois changements devraient déjà rendre le combat beaucoup plus compréhensible et satisfaisant.

---

# 33. Cible finale

Le joueur doit ressentir :

```text
Je comprends la situation.
↓
Je construis un plan.
↓
Je lance l’assaut.
↓
Je vois les conséquences.
↓
Je décide si j’interviens.
↓
Je gagne ou je perds.
↓
Je comprends immédiatement pourquoi.
```

Le système doit donner l’impression de **commander une bataille**, et non de configurer une structure de données puis lire un log.
