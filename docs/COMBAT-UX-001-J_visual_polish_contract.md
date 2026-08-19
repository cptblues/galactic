# COMBAT-UX-001-J — Visual Polish Contract

> **Projet :** Galactic  
> **Référence visuelle obligatoire :** `example_combat.png`  
> **Objectif :** faire converger l’écran de combat actuel vers le design de `example_combat.png`, avec une priorité absolue donnée à la lisibilité, à la hiérarchie visuelle, à la mise en scène et à la qualité perçue.
>
> Ce document est un **contrat d’implémentation visuelle**. Il ne s’agit pas d’une simple source d’inspiration.

---

# 0. Règle principale

La référence :

```text
example_combat.png
```

est la **source de vérité visuelle**.

Le code existant est la source de vérité fonctionnelle.

En cas de conflit :

```text
apparence / composition / hiérarchie
→ example_combat.png

comportement / simulation / règles
→ code métier existant
```

Le but n’est pas :

> améliorer un peu l’écran actuel.

Le but est :

> faire converger la composition de l’écran actuel vers `example_combat.png`.

---

# 1. Pourquoi cette passe existe

Une première implémentation a déjà été réalisée, mais le résultat n’est pas suffisamment convaincant visuellement.

Les problèmes typiques à éviter sont :

- reprendre les composants existants sans revoir réellement la composition ;
- transformer la référence en une succession de panels techniques ;
- utiliser trop de cadres rectangulaires ;
- conserver des textes trop petits ;
- conserver des zones vides importantes ;
- conserver des listes de boutons techniques ;
- garder une carte tactique trop petite ;
- conserver la navigation stratégique visible derrière le combat ;
- afficher trop d’informations en même temps ;
- considérer que “ça compile” signifie que le travail est terminé.

Cette passe doit donc être **plus stricte que les précédentes**.

---

# 2. Étape obligatoire avant tout code

Avant de modifier un fichier :

## 2.1 Localiser la référence

Trouver :

```text
example_combat.png
```

dans le dépôt.

Par exemple :

```bash
find . -name "example_combat.png"
```

L’ouvrir et l’inspecter visuellement.

## 2.2 Inspecter le code actuel

Lire entièrement les fichiers actuels liés à l’UI de combat.

Au minimum :

```text
crates/galactic_client/src/combat_ui.rs
crates/galactic_client/src/combat_ui/
crates/galactic_client/src/combat_ui/battlefield.rs
crates/galactic_client/src/combat_ui/group_panel.rs
```

et tous les sous-modules réellement présents.

Ne pas supposer leur contenu.

## 2.3 Gap analysis obligatoire

Avant tout patch, produire une analyse sous cette forme :

| Zone | État actuel | Référence `example_combat.png` | Écart | Modification requise |
|---|---|---|---|---|
| Header | ... | ... | ... | ... |
| Briefing | ... | ... | ... | ... |
| Forces | ... | ... | ... | ... |
| Carte tactique | ... | ... | ... | ... |
| Paramètres | ... | ... | ... | ... |
| Doctrine | ... | ... | ... | ... |
| Round | ... | ... | ... | ... |
| Commandement | ... | ... | ... | ... |
| Résultat | ... | ... | ... | ... |
| Rapport | ... | ... | ... | ... |

Ne pas commencer l’implémentation avant cette analyse.

---

# 3. Objectif visuel global

L’écran doit donner l’impression d’un :

```text
centre de commandement spatial
```

et non d’un :

```text
outil d’administration / interface debug
```

Direction recherchée :

```text
dark sci-fi
sobre
cinématique
hiérarchisé
lisible
compact
tactique
immersif
```

---

# 4. Hiérarchie de l’écran

La règle la plus importante est :

> **Une seule zone doit dominer visuellement à chaque phase.**

## Briefing

Zone dominante :

```text
objectif + planète + situation
```

## Planification

Zone dominante :

```text
carte tactique
```

## Exécution du round

Zone dominante :

```text
bataille / trajectoires / impacts
```

## Intervention

Zone dominante :

```text
résumé du round + choix de commandement
```

## Résultat

Zone dominante :

```text
victoire/défaite + planète + gains/pertes
```

---

# 5. Layout général

Sur :

```text
1920 × 1080
```

la surface combat doit occuper environ :

```text
90 à 96 % de la largeur
82 à 90 % de la hauteur utile
```

Ne pas coller tous les composants aux bords.

## 5.1 Max width

Sur les grands écrans :

```text
2560
3440
3840
```

ne pas étirer automatiquement les colonnes latérales.

Créer une composition centrée avec une largeur maximale.

Valeur cible indicative :

```text
max-width ≈ 2200 à 2500 px
```

La largeur supplémentaire doit surtout :

- donner de l’air ;
- agrandir la carte tactique ;
- améliorer l’espacement.

Elle ne doit pas transformer les colonnes en bandes géantes.

---

# 6. Responsive

Créer trois profils minimum :

```text
Compact
Standard
Wide
```

Exemple :

```text
Compact  < 1500 px
Standard 1500–2400 px
Wide     > 2400 px
```

## Compact

Priorité :

```text
lisibilité
```

Réduire :

- padding ;
- textes secondaires ;
- détails non essentiels.

Ne jamais réduire le texte principal à une taille illisible.

## Standard

C’est le layout de référence.

## Wide

Utiliser l’espace pour :

```text
carte tactique plus large
meilleure respiration
plus grandes illustrations
```

Pas pour :

```text
des panels latéraux énormes
```

---

# 7. Design tokens

Créer ou réutiliser des constantes communes.

Éviter des valeurs arbitraires partout.

## 7.1 Spacing

Utiliser principalement :

```text
4
8
12
16
24
32
```

## 7.2 Typographie

Cibles indicatives :

```text
titre écran        20–24 px
titre important    16–18 px
titre section      13–15 px
information        12–14 px
texte secondaire   11–12 px
```

Ne pas utiliser du 9 px pour de l’information importante.

## 7.3 Bordures

```text
1 px
fines
désaturées
```

Ne pas entourer chaque enfant d’un cadre cyan.

## 7.4 Radius

```text
4–8 px
```

---

# 8. Palette

Direction générale :

```text
fond principal         noir / bleu très sombre
surface panel          bleu-noir légèrement plus clair
texte principal        presque blanc
texte secondaire       gris bleu
allié principal        cyan / vert
Beta                    bleu
Gamma                   violet
ennemi                  rouge / orange
warning                 ambre
succès                  vert
```

Éviter :

- saturation excessive ;
- glow partout ;
- cyan autour de tout.

---

# 9. Background

Pendant le combat, masquer ou atténuer fortement :

- UI galaxie ;
- panneau planète standard ;
- menus colonie ;
- autres panneaux ;
- éléments textuels derrière.

## 9.1 Fond système

Le système solaire peut rester visible.

Mais appliquer :

```text
dark overlay
+
faible contraste
+
opacité réduite
```

Le fond ne doit jamais concurrencer le combat.

---

# 10. Header combat

Le header doit être compact et informatif.

Exemple :

```text
ASSAUT ORBITAL — HÉLIANTHE d

ROUND 2 / 6
RENSEIGNEMENT 25 %
```

Ne pas faire un bandeau de debug.

Afficher prioritairement :

- planète ;
- phase ;
- round ;
- éventuellement renseignement.

---

# 11. Phase Briefing

Le briefing est un écran dédié.

Il ne doit pas déjà ressembler à la planification.

## 11.1 Composition

Structure proche de :

```text
┌────────────────────────────────────────────┐
│ ASSAUT ORBITAL                             │
│ Hélianthe d                                │
│                                            │
│ OBJECTIF                                   │
│ Neutraliser les défenses orbitales         │
│                                            │
│ RENSEIGNEMENT                              │
│ 25 %                                       │
│                                            │
│ VOTRE FLOTTE                               │
│ [Croiseur] x5                              │
│ [Frégate] x5                               │
│ [Intercepteur] x5                          │
│                                            │
│ ESTIMATION ENNEMIE                         │
│ 4 contacts                                 │
│ Composition incertaine                     │
│                                            │
│              [ PRÉPARER L’ASSAUT ]         │
└────────────────────────────────────────────┘
```

## 11.2 Planète

La planète cible doit être visuellement présente.

Réutiliser son asset / aperçu existant.

## 11.3 Ce qui doit être caché

Ne pas afficher :

- PC ;
- focus ;
- réserve ;
- doctrines avancées ;
- chronologie ;
- rapport.

---

# 12. Phase Planification — Layout exact

Le layout doit se rapprocher de :

```text
┌─────────────────┬─────────────────────────────────┬─────────────────┐
│ VOS FORCES      │                                 │ PARAMÈTRES      │
│                 │                                 │ SÉLECTIONNÉS    │
│ ALPHA           │                                 │                 │
│ [ships]         │                                 │ ALPHA           │
│                 │       CARTE TACTIQUE            │                 │
│ BETA            │                                 │ RÔLE            │
│ [ships]         │             PLANÈTE             │ [Assaut]        │
│                 │                                 │ [Écran]         │
│ GAMMA           │ Alliés             Contacts     │ [Bombardement]  │
│ [ships]         │                                 │ [Réserve]       │
│                 │                                 │                 │
│ Plan recommandé │                                 │ PRIORITÉ        │
│                 │                                 │ ...             │
└─────────────────┴─────────────────────────────────┴─────────────────┘
```

---

# 13. Proportions

Cible :

```text
Forces        : 18–22 %
Carte         : 55–62 %
Paramètres    : 20–24 %
```

La carte doit toujours être plus grande que chacune des colonnes.

---

# 14. Colonne Forces

Les groupes doivent être compacts.

Exemple Alpha :

```text
ALPHA

[image croiseur]
Croiseur — Verdict
x5

ASSAUT
Priorité : lourde
```

Utiliser les assets de vaisseaux.

Pas seulement une icône générique.

Lorsqu’un stack est sélectionné :

- bordure plus claire ;
- background légèrement différent ;
- pas de glow excessif.

---

# 15. Alpha / Beta / Gamma

Identité :

```text
Alpha
cyan / vert

Beta
bleu

Gamma
violet
```

Ces couleurs doivent se retrouver :

- liste gauche ;
- tokens carte ;
- trajectoires ;
- résumé round.

---

# 16. Carte tactique

C’est la priorité numéro 1.

La carte doit ressembler à une visualisation de bataille.

Pas à une grille de cards.

## 16.1 Éléments obligatoires

Afficher :

```text
planète centrale
2–3 orbites subtiles
tokens alliés
tokens ennemis
trajectoires
labels courts
```

## 16.2 Planète centrale

Taille suffisante pour devenir un point d’ancrage visuel.

Éviter une simple icône minuscule.

## 16.3 Positions

Positions visuelles seulement.

Exemple :

```text
Alpha
→ gauche / haut gauche

Beta
→ gauche / bas gauche

Gamma réserve
→ plus loin / bas

Contacts
→ droite
```

Ne jamais ajouter des coordonnées métier.

---

# 17. Tokens alliés

Un groupe tactique doit être compact.

Exemple :

```text
ALPHA
[asset croiseur]
x5
82 %
```

Pas un grand rectangle avec beaucoup de texte.

---

# 18. Tokens ennemis

Même règle.

Exemple :

```text
◇ CONTACT LOURD
Défenses orbitales
Intégrité : ?
```

## 18.1 Respect du renseignement

Si identité inconnue :

utiliser :

```text
unknown_signature
unknown_light
unknown_medium
unknown_heavy
```

Selon les informations réellement connues.

Ne jamais afficher l’asset exact d’une unité cachée.

---

# 19. Trajectoires

Afficher visuellement le plan.

## Assaut

```text
────────────►
```

## Écran

```text
- - - - - - ◯
```

ou arc de couverture.

## Bombardement

```text
⌒⌒⌒⌒⌒►
```

arc / ligne pointillée longue.

## Réserve

Pas de trajectoire offensive.

---

# 20. Selected state

Quand Alpha est sélectionné :

- son token est accentué ;
- sa trajectoire est accentuée ;
- ses paramètres apparaissent à droite.

Les autres groupes restent visibles mais secondaires.

---

# 21. Panneau Paramètres

Ne pas utiliser des boutons cycliques opaques.

L’utilisateur doit voir toutes les valeurs.

## 21.1 Header

```text
PARAMÈTRES SÉLECTIONNÉS

ALPHA
```

## 21.2 Rôle

```text
RÔLE

[ ASSAUT ] [ ÉCRAN ]
[ BOMB.  ] [ RÉSERVE ]
```

## 21.3 Priorité

```text
PRIORITÉ

[ Toutes ]
[ Légères ]
[ Moyennes ]
[ Lourdes ]
[ Endommagées ]
[ Soutien ]
```

## 21.4 État sélectionné

Le bouton actif doit être très évident.

Utiliser :

- background ;
- border ;
- icône ;
- éventuellement petite coche.

---

# 22. Doctrine

Simplifier le premier niveau.

Afficher :

```text
PRUDENT
ÉQUILIBRÉ
AGRESSIF
```

Sous ces trois choix :

```text
TACTIQUES AVANCÉES ▼
```

permet d’accéder aux autres doctrines existantes.

Ne pas recréer six gros rectangles de doctrine sur toute la largeur.

---

# 23. Plan recommandé

Ajouter un CTA secondaire :

```text
★ PLAN RECOMMANDÉ
```

Il doit être visible mais secondaire par rapport à :

```text
LANCER L’ASSAUT
```

---

# 24. CTA principal

Bouton :

```text
LANCER L’ASSAUT
```

Doit être :

- grand ;
- lisible ;
- contrasté ;
- clairement le bouton principal.

---

# 25. Phase Exécution

Au lancement, disparaissent :

- paramètres ;
- plan recommandé ;
- boutons de rôle ;
- boutons priorité ;
- configuration doctrine.

La carte devient dominante.

---

# 26. Mise en scène du round

Utiliser les événements métier existants.

Ne pas générer une nouvelle simulation.

Timeline indicative :

```text
0.0s trajectoires actives
0.2s départ des tirs
0.5s impacts
0.8s pertes
1.2s mise à jour états
1.5s fin
```

---

# 27. Effets

Effets légers :

- projectile / ligne ;
- flash ;
- scale pulse ;
- petit shake ;
- nombre de pertes ;
- destruction.

Pas besoin de centaines de particules ou d’une simulation 3D complexe.

---

# 28. Phase Round Summary

Après l’animation :

```text
ROUND 2 TERMINÉ

Alpha
82 %
-2 unités

Beta
94 %
-1 unité

Gamma
Réserve
```

## 28.1 Ennemi

```text
ENNEMI

2 détruits
1 endommagé
1 nouveau contact
```

---

# 29. Commandement

Visible uniquement après le premier round.

```text
POINTS DE COMMANDEMENT

● ● ○
2 disponibles
```

---

# 30. Interventions contextuelles

Afficher uniquement les options valides.

Exemple :

```text
FOCUS LOURD
1 PC

ENGAGER GAMMA
1 PC

CHANGER DOCTRINE
1 PC
```

Ne pas afficher toutes les options possibles simultanément si elles ne sont pas pertinentes.

---

# 31. Action principale

Toujours proposer :

```text
CONTINUER LE PLAN
```

comme CTA principal.

---

# 32. Phase Résultat

Priorité numéro 2 après la carte tactique.

Le résultat doit être émotionnellement satisfaisant.

---

# 33. Victoire

Structure cible :

```text
                 VICTOIRE

              HÉLIANTHE d

     Défenses orbitales neutralisées


VOTRE FLOTTE                 ENNEMI

15 engagés                   4 groupes
14 survivants                4 détruits
1 perte


                 BUTIN

+324 Métal
+153 Cristal
+54 Carburant


MOMENT DÉCISIF

Round 3
Alpha a percé les défenses lourdes


[ RAPPORT DÉTAILLÉ ]     [ CONTINUER ]
```

---

# 34. Planète résultat

La planète doit être grande et centrale.

C’est la cible conquise / attaquée.

Elle doit donner du poids à la victoire.

---

# 35. Pertes

Afficher avec assets.

Exemple :

```text
[Intercepteur]
1 perdu
```

---

# 36. Butin

Utiliser les icônes existantes :

```text
Métal
Cristal
Carburant
```

avec valeur importante.

---

# 37. Moment décisif

V1 :

déterminer un événement simple :

- round avec plus de dégâts ;
- première destruction lourde ;
- engagement de réserve ;
- dernier groupe détruit.

---

# 38. Défaite

Même qualité visuelle.

Exemple :

```text
DÉFAITE

Flotte repoussée

Pertes
Survivants
Dégâts infligés
Conséquence
```

---

# 39. Retraite

Exemple :

```text
RETRAITE

Flotte extraite

Survivants
Pertes
Coût du repli
Destination retour
```

---

# 40. Rapport détaillé

Le rapport technique ne doit plus être affiché automatiquement.

Bouton :

```text
VOIR LE RAPPORT DÉTAILLÉ
```

Créer :

```text
Résumé
Déroulement
Statistiques
Unités
```

---

# 41. Rapport Résumé

Afficher :

- résultat ;
- rounds ;
- dégâts ;
- pertes ;
- gain.

---

# 42. Rapport Déroulement

Chronologie :

```text
Round 1
Alpha engage...

Round 2
Beta...

Round 3
Gamma...
```

---

# 43. Rapport Statistiques

Afficher :

- dégâts Alpha ;
- dégâts Beta ;
- dégâts Gamma ;
- dégâts reçus ;
- doctrine ;
- interventions.

---

# 44. Rapport Unités

Tableau lisible.

---

# 45. Gestion des espaces vides

Ne jamais laisser une grande partie de l’écran vide sans intention visuelle.

Les grands espaces doivent servir :

- respiration ;
- planète ;
- carte ;
- illustration.

---

# 46. Éviter les micro-textes

Aucune information majeure ne doit être affichée en texte minuscule.

Si le contenu ne tient pas :

- simplifier ;
- cacher en tooltip ;
- déplacer dans rapport détaillé.

Ne pas juste réduire la police.

---

# 47. Tooltips

Utiliser les tooltips pour :

- détails doctrine ;
- formule ;
- explication statistique ;
- bonus secondaire.

Pas pour les informations nécessaires à la décision principale.

---

# 48. États hover / selected / disabled

Tous les boutons interactifs doivent avoir :

```text
normal
hover
selected
disabled
```

visuellement distincts.

---

# 49. Disabled

Un bouton désactivé doit :

- être clairement inactif ;
- éventuellement expliquer pourquoi.

Exemple tooltip :

```text
Aucun groupe en réserve.
```

---

# 50. Architecture Bevy

Ne pas refaire le moteur métier.

`galactic_sim` reste la source de vérité.

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
change pas le résultat
invente pas les pertes
```

---

# 51. Découpage recommandé

Si les modules existent déjà, les adapter.

Sinon envisager :

```text
combat_ui/
├── mod.rs
├── briefing.rs
├── planning.rs
├── battlefield.rs
├── group_panel.rs
├── doctrine_panel.rs
├── round_execution.rs
├── round_summary.rs
├── final_result.rs
└── detailed_report.rs
```

Ne pas créer tous les fichiers juste pour suivre la liste si ce n’est pas utile.

---

# 52. Patches obligatoirement séparés

Ne pas faire une énorme modification unique.

## J1 — Composition globale

Objectifs :

- max-width ;
- responsive ;
- combat mode ;
- background ;
- header ;
- proportions.

## J2 — Planning

Objectifs :

- colonne forces ;
- carte tactique ;
- paramètres ;
- trajectoires ;
- doctrine simplifiée.

## J3 — Exécution et round summary

Objectifs :

- masquer config ;
- animation ;
- résultats round ;
- interventions.

## J4 — Résultat

Objectifs :

- victoire ;
- défaite ;
- retraite ;
- butin ;
- moment décisif.

## J5 — Rapport

Objectifs :

- onglets ;
- chronologie ;
- stats.

## J6 — Polish final

Aucune nouvelle fonctionnalité.

Uniquement :

- spacing ;
- typographie ;
- alignement ;
- couleurs ;
- tailles ;
- contrastes ;
- états ;
- simplification ;
- suppression du bruit.

---

# 53. Boucle visuelle obligatoire

Après chaque J1 / J2 / J3 / J4 :

```text
1. build
2. run
3. screenshot
4. comparer avec example_combat.png
5. identifier les écarts
6. corriger
7. refaire screenshot
```

---

# 54. Comparaison obligatoire après screenshot

Produire une liste :

```text
ÉCARTS RESTANTS

[ ] carte encore trop petite
[ ] paramètres trop larges
[ ] texte secondaire trop petit
[ ] planète trop petite
[ ] groupes trop hauts
[ ] trop de bordures
[ ] CTA principal pas assez visible
[ ] résultat manque d’impact
```

Ne pas déclarer la tâche terminée tant que les écarts importants ne sont pas traités.

---

# 55. Critère de fin visuelle

La tâche n’est PAS terminée quand :

```text
cargo check passe
```

Elle est terminée quand :

```text
capture jeu
VS
example_combat.png
```

présentent une hiérarchie et une composition clairement comparables.

---

# 56. Définition de Done — Planning

- [ ] la carte tactique occupe la majorité de l’espace ;
- [ ] Alpha/Beta/Gamma sont compacts ;
- [ ] les assets de vaisseaux sont visibles ;
- [ ] la planète est centrale ;
- [ ] les contacts sont compacts ;
- [ ] les trajectoires sont visibles ;
- [ ] le groupe sélectionné est évident ;
- [ ] le rôle se choisit directement ;
- [ ] la priorité se choisit directement ;
- [ ] le CTA Lancer est évident ;
- [ ] les doctrines ne prennent pas toute la largeur.

---

# 57. Définition de Done — Round

- [ ] l’interface de configuration disparaît ;
- [ ] l’action est dominante ;
- [ ] les impacts sont visibles ;
- [ ] les pertes sont visibles ;
- [ ] le joueur comprend qui a frappé quoi ;
- [ ] le résumé du round est clair ;
- [ ] les interventions sont contextuelles ;
- [ ] Continuer le plan est évident.

---

# 58. Définition de Done — Résultat

- [ ] VICTOIRE / DÉFAITE domine l’écran ;
- [ ] la planète est bien visible ;
- [ ] les pertes alliées sont immédiatement visibles ;
- [ ] les pertes ennemies sont immédiatement visibles ;
- [ ] le butin est bien visible ;
- [ ] le résultat n’est pas un gros bloc texte ;
- [ ] le rapport détaillé est secondaire ;
- [ ] Continuer est le CTA principal.

---

# 59. Tests résolutions

Vérifier au minimum :

```text
1280×720
1920×1080
2560×1440
3440×1440
```

---

# 60. Test ultrawide

Vérifier particulièrement :

- aucune colonne absurdement large ;
- texte non étiré ;
- carte tactique utilise l’espace ;
- composition centrée ;
- max-width cohérente.

---

# 61. Test faible renseignement

Vérifier :

- aucun asset exact caché n’est affiché ;
- contacts lisibles ;
- incertitude clairement représentée.

---

# 62. Test forte composition

Exemple :

```text
3 stacks alliés
5 contacts ennemis
```

Aucun overlap.

---

# 63. Test victoire / défaite / retraite

Les trois résultats doivent avoir une qualité visuelle cohérente.

---

# 64. Checks Rust

À chaque checkpoint :

```bash
cargo fmt --all
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release
```

Ne jamais prétendre qu’une commande a réussi sans l’avoir exécutée.

---

# 65. Ne pas modifier

Hors nécessité absolue, ne pas modifier :

- calcul des dégâts ;
- seed ;
- règles combat ;
- balance ;
- sauvegardes ;
- IA métier ;
- définition des doctrines ;
- logique de mission.

---

# 66. Interdictions

Ne pas résoudre un problème UI en :

- supprimant une mécanique ;
- ajoutant une règle métier ;
- révélant des données cachées.

---

# 67. Échecs visuels à éviter

## 67.1 Trop de cards

Ne pas mettre chaque information dans une card avec border.

Utiliser aussi :

- espace ;
- alignement ;
- typographie ;
- couleur ;
- séparateurs.

## 67.2 Tous les boutons identiques

Créer une hiérarchie :

```text
CTA primaire
CTA secondaire
bouton option
bouton technique
```

## 67.3 Carte trop petite

La carte tactique doit dominer.

## 67.4 Réduire la police pour faire tenir

Simplifier le contenu à la place.

## 67.5 Copier seulement les couleurs

Les éléments essentiels sont :

```text
proportions
hiérarchie
densité
placement
taille carte
taille planète
nombre d’actions visibles
```

## 67.6 Finir sans screenshot

Interdit pour cette tâche.

---

# 68. Première réponse attendue de Claude Code

Avant de coder :

## A. Analyse

```text
HEAD actuel
fichiers concernés
architecture actuelle
```

## B. Gap analysis

Tableau complet.

## C. Plan

```text
J1
J2
J3
J4
J5
J6
```

avec fichiers concernés.

## D. Risques

Identifier :

- responsive ;
- conflit Bevy UI ;
- données absentes ;
- assets manquants.

---

# 69. Ensuite seulement : implémentation

Après validation du plan :

implémenter J1.

Puis :

```text
screenshot
analyse
polish
```

avant J2.

---

# 70. Priorités absolues

Si le temps manque :

```text
P0
Carte tactique
Layout / proportions
Résultat

P1
Group parameters
Round summary
Doctrine simplifiée

P2
Animations fines
Tooltips
Micro-polish
```

---

# 71. Rappel final

Le résultat recherché n’est pas :

> une version légèrement plus propre de l’écran actuel.

Le résultat recherché est :

> une interface qui, mise côte à côte avec `example_combat.png`, donne immédiatement l’impression d’être la même direction de design et la même composition.

---

# 72. Prompt court à utiliser avec ce document

Après avoir placé ce fichier dans le dépôt, utiliser :

```text
Lis entièrement COMBAT-UX-001-J_visual_polish_contract.md.

La référence visuelle obligatoire est example_combat.png.

Ne modifie aucun fichier pour l’instant.

Commence par :
1. inspecter le HEAD actuel ;
2. lire tous les fichiers de combat concernés ;
3. ouvrir et analyser example_combat.png ;
4. produire le gap analysis demandé dans le document ;
5. proposer J1 à J6 avec les fichiers concernés.

Je validerai le plan avant que tu modifies le code.

Important :
- example_combat.png est la source de vérité visuelle ;
- ne considère pas la compilation comme définition de Done ;
- chaque étape de code devra être suivie d’une comparaison par screenshot avec la référence.
```
