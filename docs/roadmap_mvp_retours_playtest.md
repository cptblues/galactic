# Galactic — Roadmap des issues MVP issues des retours de playtest

> Document de travail pour transformer les retours du MVP actuel en checkpoints implémentables, testables et committables séparément.
>
> Principe directeur : stabiliser l’interface avant d’ajouter de nouveaux systèmes, puis approfondir la boucle économique, les flottes et les missions avant d’introduire les attaques ennemies.

---

## Sommaire

| Issue | Intitulé | Priorité | Taille estimée | Statut proposé |
|---|---|---:|---:|---|
| MVP-030-A1 | Stabiliser l’interface stratégique | P0 | 5 pts | À faire |
| MVP-030-A2 | Clarifier l’économie planétaire et la recherche | P1 | 8 pts | À faire |
| MVP-030-A3 | Ajouter les fabrications par lot et l’annulation des files | P1 | 8 pts | À faire |
| MVP-030-A4 | Créer un planificateur de missions complet | P0 | 13 pts | À faire |
| MVP-030-A5 | Transformer l’analyse planétaire en mission satellite | P1 | 8 pts | Implémenté |
| MVP-030-A6 | Diversifier les vaisseaux et les rôles de combat | P1 | 13 pts | Implémenté |
| MVP-030-A7 | Polir l’UX flotte, ressources et cadrage narratif | P1 | 5 pts | Implémenté |
| MVP-031 | Finaliser sauvegarde, chargement et migrations V1 | P0 | 8 pts | Reporté après MVP-033 |
| MVP-032 | Ajouter onboarding et objectifs contextuels | P1 | 5 pts | MVP-032-A implémenté |
| MVP-033 | Implémenter la condition de réussite du MVP | P1 | 3 pts | À faire |
| MVP-034 | Ajouter les presets graphiques | P2 | 5 pts | À faire |
| MVP-035 | Intégrer diagnostics et benchmark reproductible | P2 | 5 pts | À faire |
| MVP-036 | Auditer et compléter les tests métier | P0 | 8 pts | À faire |
| MVP-037 | Ajouter un smoke test de la boucle complète | P0 | 8 pts | À faire |
| MVP-038 | Équilibrer, polir et packager le MVP de playtest | P1 | 8 pts | À faire |
| MVP-039 | Ajouter les attaques ennemies et les défenses planétaires | P2 | 13 pts | Après MVP |

---

# Phase 1 — Stabilisation de l’interface

<a id="mvp-030-a1"></a>

## MVP-030-A1 — Stabiliser l’interface stratégique

| Métadonnée | Valeur |
|---|---|
| Priorité | P0 |
| Estimation | 5 points |
| Statut | Implémenté |
| Dépendances | Interface actuelle du MVP-030 |

### Objectif

Supprimer les ambiguïtés visuelles, caractères cassés et superpositions afin que les écrans économiques soient utilisables sans explication externe.

### Périmètre

- Déplacer le fil d’Ariane dans une zone qui ne peut pas recouvrir les panneaux de gestion.
- Corriger les boutons précédent/suivant actuellement affichés comme des cases vides.
- Remplacer les caractères Unicode fragiles par des icônes intégrées ou des libellés textuels.
- Remplacer les chaînes ambiguës comme `Niveau 3 □ 4 en file`.
- Afficher séparément :
  - niveau actuel ;
  - niveau prévu ;
  - effet actuel ;
  - effet du prochain niveau ;
  - variation produite.
- Remplacer les suites de valeurs non nommées comme `5000 / 4000 / 3000`.
- Conserver une barre de navigation économique permanente :
  - Bâtiments ;
  - Recherche ;
  - Chantier ;
  - Flottes.
- Ouvrir un panneau ferme automatiquement le panneau économique précédent.
- Conserver la colonie active lors d’un changement de panneau.
- Ajouter un marqueur visuel clair sur les planètes colonisées.
- Distinguer visuellement la colonie active des autres colonies.

### Présentation attendue d’un bâtiment

```text
MINE PATRIOTIQUE DE MÉTAL

Niveau actuel : 3
Production actuelle : 48 métal/h
Énergie actuelle : -12

Prochain niveau : 4
Production prévue : 65 métal/h  (+17)
Énergie prévue : -17           (-5)

Coût : 520 métal · 260 cristal · 90 carburant
Durée : 01:24
```

### Hors périmètre

- Nouvelle règle économique.
- Nouveaux bâtiments.
- Nouveaux vaisseaux.
- Refonte complète de la direction artistique.
- Glisser-déposer des files.

### Critères d’acceptation

- [ ] Aucun caractère de remplacement ou carré vide n’est visible.
- [ ] Aucun fil d’Ariane ne recouvre les écrans de gestion.
- [ ] Tous les boutons possèdent une icône valide ou un texte explicite.
- [ ] Le niveau actuel et le prochain niveau sont compréhensibles immédiatement.
- [ ] Les effets actuels et futurs sont nommés.
- [ ] Ouvrir Recherche ferme Bâtiments.
- [ ] Ouvrir Chantier ferme Recherche.
- [ ] La colonie active est conservée entre les panneaux.
- [ ] Une planète colonisée est identifiable sans être sélectionnée.
- [ ] L’interface reste lisible en 1280×720 et 1920×1080.

---

# Phase 2 — Économie planétaire et progression

<a id="mvp-030-a2"></a>

## MVP-030-A2 — Clarifier l’économie planétaire et la recherche

| Métadonnée | Valeur |
|---|---|
| Priorité | P1 |
| Estimation | 8 points |
| Statut | À faire |
| Dépendances | MVP-030-A1 |

### Objectif

Donner une identité économique claire à chaque planète et rendre la progression technologique plus longue, lisible et structurée.

### Périmètre — identité planétaire

- Formaliser la production réelle :

```text
production réelle
= production du bâtiment
× potentiel planétaire
× efficacité énergétique
```

- Afficher le potentiel planétaire pour :
  - métal ;
  - cristal ;
  - carburant ;
  - énergie.
- Afficher la production théorique et la production réellement obtenue.
- Donner à chaque type de planète :
  - une ressource forte ;
  - éventuellement une ressource secondaire ;
  - une ou deux ressources moins favorables.
- Éviter qu’une ressource soit totalement inutilisable.
- Afficher une spécialisation synthétique :
  - monde métallurgique ;
  - monde cristallin ;
  - monde énergétique ;
  - monde carburant ;
  - monde équilibré.

### Périmètre — recherche

- Afficher la production scientifique globale.
- Afficher la progression en points de la recherche active.
- Afficher le temps restant.
- Afficher les Instituts de vérité appliquée qui contribuent à la production.
- Conserver une recherche globale au joueur.
- Ne pas ajouter de stock de science consommable pour cette version.
- Ajouter des prérequis de bâtiments aux technologies.
- Renforcer les prérequis de la technologie de colonisation.
- Augmenter le coût et la durée des technologies avancées.
- Rendre l’accès à l’Arche coloniale — Essor sensiblement plus difficile.

### Chaîne de progression recommandée

```text
Détection longue portée
└── Propulsion avancée
    ├── Soutes agrandies
    │   └── Extraction automatisée
    └── Analyse planétaire
        └── Colonisation avancée
```

### Prérequis recommandés pour Colonisation avancée

- Détection longue portée terminée.
- Propulsion avancée terminée.
- Soutes agrandies terminée.
- Analyse planétaire terminée.
- Laboratoire niveau 4.
- Centre de construction niveau 3.

### Prérequis recommandés pour l’Arche coloniale — Essor

- Colonisation avancée terminée.
- Chantier naval niveau 3.
- Entrepôt niveau 3.
- Centre de construction niveau 4.
- Coût élevé dans les trois ressources.
- Temps de fabrication significatif.

### Hors périmètre

- Plusieurs recherches simultanées.
- Ressource scientifique stockable.
- Recherche propre à chaque colonie.
- Arbre technologique massif.
- Spécialisation irréversible des colonies.

### Critères d’acceptation

- [ ] Le joueur comprend pourquoi une planète produit davantage une ressource.
- [ ] La production affichée correspond exactement à la simulation.
- [ ] La production scientifique globale est visible.
- [ ] La progression scientifique est exprimée en points et en temps restant.
- [ ] Une technologie affiche tous ses prérequis manquants.
- [ ] Une technologie acquise ne peut pas être relancée.
- [ ] L’accès à la colonisation nécessite plusieurs étapes économiques et technologiques.
- [ ] Une première colonisation ne peut pas être atteinte par simple enchaînement rapide des recherches.

---

# Phase 3 — Files, quantités et commandes de production

<a id="mvp-030-a3"></a>

## MVP-030-A3 — Ajouter les fabrications par lot et l’annulation des files

| Métadonnée | Valeur |
|---|---|
| Priorité | P1 |
| Estimation | 8 points |
| Statut | À faire |
| Dépendances | MVP-030-A1, MVP-030-A2 |

### Objectif

Permettre au joueur de commander plusieurs vaisseaux ou défenses en une seule action et de corriger ses choix en annulant une file.

### Périmètre — production par lot

- Ajouter un champ numérique de quantité.
- Ajouter les actions :
  - `-10` ;
  - `-1` ;
  - `+1` ;
  - `+10` ;
  - `MAX`.
- Afficher :
  - coût unitaire ;
  - coût total ;
  - durée unitaire ;
  - durée totale ;
  - quantité finançable.
- Ajouter une commande métier générique de fabrication par lot.
- Stocker une entrée agrégée plutôt que cinquante entrées indépendantes.
- Fabriquer les unités une par une.
- Mettre à jour les quantités terminées et restantes.
- Réserver à la mise en file les ressources correspondant à la quantité commandée.

### Modèle recommandé

```text
CraftBatch
- craftable_id
- quantity_requested
- quantity_completed
- quantity_remaining
- current_item_progress
```

### Périmètre — annulation

- Autoriser l’annulation d’une fabrication.
- Conserver les unités déjà terminées.
- Annuler l’unité en cours.
- Rembourser les unités non terminées.
- Perdre la progression de l’unité en cours.
- Autoriser l’annulation d’une construction de bâtiment.
- Autoriser l’annulation d’une recherche.
- Libérer les réservations correspondantes.
- Journaliser l’annulation.

### Politique MVP

- Pas de fabrication automatique avec des ressources futures.
- Le joueur ne peut commander que ce qu’il peut financer immédiatement.
- Pas de réordonnancement par glisser-déposer.
- Annuler puis remettre en file suffit.

### Hors périmètre

- Auto-production permanente.
- Priorités automatiques de consommation.
- Production parallèle.
- Déplacement manuel des entrées dans la file.
- Production infinie jusqu’à épuisement.

### Critères d’acceptation

- [ ] Le joueur peut saisir `50` et ajouter cinquante unités à la file.
- [ ] Le coût total est calculé avant validation.
- [ ] `MAX` ne dépasse ni le stock ni les limites numériques.
- [ ] Les unités sont produites une à une.
- [ ] La progression survit à une sauvegarde/reprise.
- [ ] L’annulation ne duplique ni ne détruit silencieusement de ressources.
- [ ] Les unités déjà terminées restent disponibles.
- [ ] Une entrée annulée disparaît correctement de la file.

---

# Phase 4 — Missions et logistique

<a id="mvp-030-a4"></a>

## MVP-030-A4 — Créer un planificateur de missions complet

| Métadonnée | Valeur |
|---|---|
| Priorité | P0 |
| Estimation | 13 points |
| Statut | À faire |
| Dépendances | MVP-030-A1, MVP-030-A3, moteur de missions existant |

### Objectif

Permettre au joueur de préparer chaque mission sans raccourci caché ni choix automatique de flotte.

### Périmètre — assistant commun

Créer un assistant en étapes :

1. gestion des flottes : création, sélection, nommage, dissolution contrôlée ;
2. préparation : choix d'une flotte, puis des missions compatibles avec sa
   composition ;
3. destination : choix parmi les cibles disponibles, avec durée, route,
   carburant, blocages et informations utiles ;
4. paramètres : cargaison ou options propres à la mission ;
5. récapitulatif et validation ;
6. suivi dans la liste des missions actives.

### Informations obligatoires

- Colonie d’origine.
- Destination.
- Type de mission.
- Flotte choisie.
- Composition.
- Capacité cargo.
- Portée.
- Vitesse.
- Coût en carburant.
- Nombre de sauts.
- Durée aller.
- Durée sur place.
- Durée retour.
- Heure ou tick d’arrivée.
- Erreurs et prérequis.

### Périmètre — transport

- Permettre la saisie exacte de :
  - métal ;
  - cristal ;
  - carburant.
- Ajouter `MAX` par ressource.
- Afficher la capacité utilisée et restante.
- Réserver la place nécessaire au carburant du trajet.
- Refuser une cargaison supérieure à la capacité.
- Refuser une cargaison supérieure au stock.
- Permettre de sélectionner plusieurs cargos.
- Livrer dans la limite du stockage de destination.
- Ramener le surplus si nécessaire.

### Périmètre — récolte

- Remplacer l’action opaque de récolte par une mission préparée.
- Sélectionner une planète analysée.
- Afficher le site d’extraction.
- Afficher :
  - ressource ;
  - réserve ;
  - rendement ;
  - temps de chargement ;
  - état de réservation.
- Choisir explicitement la flotte de récolte.
- Calculer la quantité récupérable selon :
  - capacité cargo ;
  - réserve du site ;
  - rendement ;
  - temps sur place.
- Réserver le site pendant la mission.
- Afficher clairement si la planète est :
  - libre ;
  - sécurisée ;
  - hostile ;
  - déjà colonisée.

### Périmètre — suivi

- Liste permanente des missions actives.
- Affichage de l’ETA.
- Affichage de la phase.
- Sélection d’une mission.
- Focalisation de l’origine.
- Focalisation de la destination.
- Mise en évidence du trajet.
- Consultation du rapport terminé.

### Hors périmètre

- Installation permanente sur une planète non colonisée.
- Marché.
- Commerce automatisé.
- Interception en trajet.
- Escorte automatique.
- Piraterie.
- Plusieurs destinations dans une même mission.

### Critères d’acceptation

- [ ] Une mission peut être lancée entièrement depuis l’interface.
- [ ] Le joueur choisit la flotte utilisée.
- [ ] Le joueur saisit librement sa cargaison.
- [ ] La capacité et le carburant sont recalculés immédiatement.
- [ ] Les erreurs de portée, stock, capacité et route sont explicites.
- [ ] La récolte affiche sa quantité prévue avant lancement.
- [ ] Une mission sélectionnée met en évidence son trajet.
- [ ] La liste reste lisible avec au moins dix missions.
- [ ] Aucune mission ne duplique ou ne perd silencieusement de ressources.

---

# Phase 5 — Exploration avancée

<a id="mvp-030-a5"></a>

## MVP-030-A5 — Transformer l’analyse planétaire en mission satellite

| Métadonnée | Valeur |
|---|---|
| Priorité | P1 |
| Estimation | 8 points |
| Statut | Implémenté |
| Dépendances | MVP-030-A3, MVP-030-A4 |

### Objectif

Remplacer l’analyse instantanée d’une planète par une mission visible, progressive et coûteuse.

### Nouveau craftable

**Satellite — Veilleur**

Caractéristiques minimales :

- coût ;
- durée de fabrication ;
- vitesse ;
- portée ;
- niveau de capteurs ;
- technologie requise ;
- bâtiment requis.

### Nouvelle mission

```text
MissionKind::Analyze
```

### Phases

```text
Préparation
→ trajet aller
→ mise en orbite
→ analyse
→ retour
→ rapport terminé
```

### Répartition des informations

#### Sonde — Œil

Révèle :

- existence ;
- désignation ;
- nom ;
- type général ;
- présence potentielle.

#### Satellite — Veilleur

Révèle :

- habitabilité ;
- environnement ;
- contraintes ;
- potentiel des ressources ;
- site d’extraction ;
- colonisabilité ;
- occupant estimé ;
- forces et défenses estimées.

### Paramètres configurables

- Durée d’analyse configurée dans `planetary_analysis.ron`.
- Distance locale ou interstellaire calculée par le planificateur commun.
- Conditions de retour portées par les phases persistées de mission.

### Hors périmètre

- Satellite permanent.
- Réseau orbital.
- Destruction de satellites.
- Contre-espionnage.
- Brouillage.
- Analyse simultanée de plusieurs planètes par un même satellite.

### Critères d’acceptation

- [x] Une planète doit être sondée avant d’être analysée.
- [x] L’analyse ne produit pas immédiatement un rapport.
- [x] Le satellite doit être fabriqué.
- [x] La mission progresse uniquement sur les ticks stratégiques.
- [x] La sauvegarde conserve la phase d’analyse.
- [x] Le rapport n’est révélé qu’à la phase prévue.
- [x] Les données inconnues ne fuient pas dans l’interface.

---

# Phase 6 — Diversification des flottes et du combat

<a id="mvp-030-a6"></a>

## MVP-030-A6 — Diversifier les vaisseaux et les rôles de combat

| Métadonnée | Valeur |
|---|---|
| Priorité | P1 |
| Estimation | 13 points |
| Statut | À faire |
| Dépendances | MVP-030-A3, MVP-030-A4 |

### Objectif

Créer des choix de flotte simples mais significatifs pour le transport, la reconnaissance, l’analyse, le combat et la colonisation.

### Vaisseaux de transport

#### Caboteur — Relais

- Faible coût.
- Rapide.
- Faible capacité.
- Faible durabilité.

#### Porteur — Navette

- Coût moyen.
- Vitesse moyenne.
- Capacité moyenne.
- Usage polyvalent.

#### Cargo — Chargeur

- Coût élevé.
- Lent.
- Très grande capacité.
- Vulnérable sans escorte.

### Vaisseaux militaires

#### Intercepteur — Riposte

- Léger.
- Rapide.
- Efficace contre les unités légères.
- Fragile contre les unités lourdes.

#### Frégate — Garde

- Moyen.
- Polyvalent.
- Aucun bonus extrême.

#### Croiseur — Verdict

- Lourd.
- Lent.
- Résistant.
- Efficace contre les unités lourdes et les défenses importantes.

### Classes de cible MVP

- Léger.
- Moyen.
- Lourd.

### Bonus recommandés

- Intercepteur : bonus contre Léger.
- Frégate : neutre.
- Croiseur : bonus contre Lourd.

### Données à externaliser

- coût ;
- durée ;
- vitesse ;
- cargo ;
- portée ;
- consommation ;
- attaque ;
- défense ;
- durabilité ;
- classe ;
- bonus de cible.

### Hors périmètre

- Armes élémentaires.
- Boucliers spécialisés.
- Pénétration d’armure.
- Personnalisation de modules.
- Officiers.
- Expérience des équipages.
- Combat tactique en temps réel.

### Critères d’acceptation

- [x] Le joueur dispose d’au moins trois choix de transport.
- [x] Le joueur dispose d’au moins trois choix militaires.
- [x] Chaque vaisseau possède un rôle lisible.
- [x] Les bonus de cible sont visibles avant une attaque.
- [x] Les résultats restent déterministes.
- [x] Les statistiques viennent du ruleset.
- [x] Ajouter un vaisseau utilisant les comportements existants ne nécessite pas de modifier le cœur du moteur.

---

<a id="mvp-030-a7"></a>

## MVP-030-A7 — Polir l’UX flotte, ressources et cadrage narratif

| Métadonnée | Valeur |
|---|---|
| Priorité | P1 |
| Estimation | 5 points |
| Statut | Implémenté |
| Dépendances | MVP-030-A1 à MVP-030-A6 |

### Objectif

Lever les dernières ambiguïtés de playtest avant la sauvegarde : ressources
réservées, recherche carte vs recherche techno, helpers trop visibles et cadre
narratif encore trop neutre.

### Périmètre

- Clarifier `Stock total`, `Disponible maintenant` et `Réservé par ordres/missions`.
- Renommer la recherche de technologies et la recherche de carte pour éviter
  toute confusion.
- Empêcher les raccourcis globaux de se déclencher pendant une saisie ou un
  filtre de navigation.
- Remplacer le helper permanent par un briefing réouvrable/masquable via `?`.
- Introduire le cadrage satirique original du Consortium.
- Renommer les libellés visibles du ruleset `default` sans changer les IDs.
- Documenter le remplacement des anciennes séries de systèmes `-2`, `-3` et
  `-4` par des noms propres.

### Hors périmètre

- Sauvegarde de l'état du briefing.
- Système complet de quêtes et récompenses.
- Refonte complète des noms de planètes au-delà de leur dérivation depuis le
  système.
- Nouvelle mécanique de faction ou diplomatie.

### Critères d’acceptation

- [x] Le joueur comprend pourquoi une ressource stockée n'est pas disponible.
- [x] La recherche techno et la recherche carte sont libellées distinctement.
- [x] Les raccourcis ne réagissent pas pendant la saisie de recherche/filtres.
- [x] Le briefing initial peut être masqué et rouvert.
- [x] Le ruleset garde ses identifiants techniques.
- [x] Les séries de systèmes `-2`, `-3` et `-4` reçoivent des noms propres.
- [x] `content_version` du ruleset `default` est incrémentée à 16.

---

# Phase 7 — Persistance et partie complète

<a id="mvp-031"></a>

## MVP-031 — Finaliser sauvegarde, chargement et migrations V1

| Métadonnée | Valeur |
|---|---|
| Priorité | P0 |
| Estimation | 8 points |
| Statut | À faire |
| Dépendances | MVP-030-A3 à MVP-030-A7 |

### Objectif

Permettre au joueur de sauvegarder une partie sur disque, la recharger et continuer sans perte fonctionnelle.

### Périmètre

- Sérialiser la sauvegarde dans un format stable.
- Ajouter :
  - sauvegarde manuelle ;
  - chargement ;
  - sauvegarde rapide ;
  - autosauvegardes tournantes.
- Ajouter un écran de sauvegardes.
- Afficher :
  - nom ;
  - date ;
  - version ;
  - temps de jeu ;
  - nombre de colonies.
- Conserver :
  - univers ;
  - seed ;
  - factions ;
  - colonies ;
  - ressources ;
  - bâtiments ;
  - recherches ;
  - files ;
  - flottes ;
  - missions ;
  - rapports ;
  - sites d’extraction ;
  - sélection ;
  - navigation pertinente.
- Gérer les fichiers corrompus.
- Produire une erreur explicite pour les versions incompatibles.
- Créer un squelette de migration.
- Tester la reprise pendant chaque phase de mission.

### Hors périmètre

- Sauvegarde cloud.
- Synchronisation Steam.
- Plusieurs profils utilisateur.
- Rejeu complet de commandes.

### Critères d’acceptation

- [ ] Une partie peut être sauvegardée puis chargée depuis l’interface.
- [ ] Les identifiants stables sont conservés.
- [ ] Les files reprennent au bon état.
- [ ] Les missions reprennent à la bonne phase.
- [ ] Les cargaisons sont conservées.
- [ ] Une sauvegarde incompatible ne fait pas crasher le jeu.
- [ ] Une migration simple peut être ajoutée sans réécrire tout le système.

---

<a id="mvp-032"></a>

## MVP-032 — Ajouter onboarding et objectifs contextuels

| Métadonnée | Valeur |
|---|---|
| Priorité | P1 |
| Estimation | 5 points |
| Statut | MVP-032-A implémenté |
| Dépendances | MVP-030-A1 à MVP-030-A7 ; MVP-031 reporté |

### Objectif

Guider un nouveau joueur jusqu’à sa première colonisation sans documentation externe, puis proposer des objectifs facultatifs qui poussent à explorer des zones plus lointaines sans transformer la boucle en liste obligatoire.

### Suite d’objectifs recommandée

1. Observer les ressources de Port-Sillage.
2. Améliorer un bâtiment de production.
3. Construire un Laboratoire.
4. Lancer une première recherche.
5. Construire une Sonde — Œil.
6. Sonder une planète.
7. Construire un Satellite — Veilleur.
8. Analyser la planète.
9. Récolter une ressource distante.
10. Construire une flotte militaire.
11. Sécuriser une planète si nécessaire.
12. Débloquer Colonisation avancée.
13. Construire une Arche coloniale — Essor.
14. Fonder une deuxième colonie.

### Objectifs facultatifs recommandés

- Sonder un système situé à au moins trois sauts.
- Analyser une planète occupée hors du voisinage immédiat.
- Récolter un site distant rare.
- Gagner un combat avec une flotte mixte.
- Fonder une colonie dans une zone à risque.

Les récompenses doivent rester modestes et explicables : petite quantité de
ressources, accélération courte, lot de renseignement, ou priorité de file. Un
objectif ne doit pas devenir le meilleur moyen d'optimiser l'économie.

### Périmètre

- Objectif courant.
- Progression.
- Aide contextuelle.
- Mise en évidence du panneau pertinent.
- Possibilité de masquer les conseils.
- Possibilité de les réactiver.
- Validation depuis l’état réel de simulation.
- Progression sauvegardée plus tard avec MVP-031.
- Objectifs facultatifs avec récompenses modestes.
- Ton narratif cohérent avec le Consortium.

### Critères d’acceptation

- [ ] Un nouveau joueur atteint sa première sonde sans aide orale.
- [ ] Un objectif ne se valide pas depuis un simple clic d’interface.
- [x] Les conseils ne bloquent pas la navigation.
- [ ] La progression est sauvegardée.
- [x] Les objectifs utilisent les systèmes réels du jeu.
- [x] Les objectifs facultatifs incitent à explorer plus loin sans bloquer la partie.
- [x] Les récompenses ne dominent pas la production normale.

### MVP-032-A implémenté

Le client ajoute un panneau `Objectifs du Consortium` accessible par `O` et
par la barre basse. La progression est volontairement locale à la session pour
ne pas rouvrir MVP-031 : aucun champ n'est ajouté à `GameState` et aucune
version de sauvegarde n'est modifiée.

Les objectifs principaux guident jusqu'à une deuxième colonie. Les objectifs
facultatifs encouragent les systèmes lointains, les planètes occupées, les
récoltes substantielles, les flottes mixtes et les colonies risquées. Toutes
les validations lisent l'état réel de simulation : bâtiments, recherches,
inventaires, flottes, niveaux de connaissance, rapports de mission, rapports
d'analyse et colonies du joueur.

Les récompenses sont des petites dotations de ressources créditées une seule
fois par session, plafonnées par la capacité de stockage de la colonie active
ou de Port-Sillage.

---

<a id="mvp-033"></a>

## MVP-033 — Implémenter la condition de réussite du MVP

| Métadonnée | Valeur |
|---|---|
| Priorité | P1 |
| Estimation | 3 points |
| Statut | À faire |
| Dépendances | MVP-030-A2, MVP-031, MVP-032 |

### Objectif

Donner une conclusion mesurable à la vertical slice.

### Condition recommandée

- Trois colonies.
- Huit systèmes sondés.
- Technologie finale atteinte.
- Au moins une récolte distante réussie.
- Au moins une planète sécurisée ou une mission militaire réussie.

### Périmètre

- Règles configurables.
- Panneau de progression.
- Événement de réussite.
- Résumé de partie.
- Temps de jeu.
- Colonies.
- Systèmes explorés.
- Recherches.
- Missions.
- Possibilité de continuer après la réussite.

### Critères d’acceptation

- [ ] La réussite se déclenche uniquement lorsque toutes les conditions sont remplies.
- [ ] Les seuils viennent du ruleset.
- [ ] Le résumé affiche les statistiques principales.
- [ ] La partie reste jouable après l’écran de réussite.

---

# Phase 8 — Performance, tests et distribution

<a id="mvp-034"></a>

## MVP-034 — Ajouter les presets graphiques

| Métadonnée | Valeur |
|---|---|
| Priorité | P2 |
| Estimation | 5 points |
| Statut | À faire |
| Dépendances | Interface stabilisée |

### Périmètre

- Presets Low, Medium et High.
- Paramètres configurables :
  - bloom ;
  - HDR ;
  - ombres ;
  - particules ;
  - labels ;
  - nébuleuses ;
  - densité visuelle ;
  - résolution interne ;
  - qualité des textures procédurales.
- Changement à chaud lorsque possible.
- Sauvegarde du preset.

### Critères d’acceptation

- [ ] Low désactive les effets coûteux sans casser la lisibilité.
- [ ] Medium constitue le réglage par défaut.
- [ ] High améliore le rendu sans modifier la simulation.
- [ ] Le preset est visible dans les options.
- [ ] Le choix est sauvegardé.

---

<a id="mvp-035"></a>

## MVP-035 — Intégrer diagnostics et benchmark reproductible

| Métadonnée | Valeur |
|---|---|
| Priorité | P2 |
| Estimation | 5 points |
| Statut | À faire |
| Dépendances | MVP-034 |

### Périmètre

- FPS.
- Temps de frame.
- Moyenne.
- Minimum.
- Percentile 95.
- Entités.
- Meshes.
- Matériaux.
- Images.
- Mémoire.
- Seed fixe.
- Séquence de caméra reproductible.
- Tests 720p et 1080p.
- Tests Low, Medium et High.
- Export texte, CSV ou JSON.

### Critères d’acceptation

- [ ] Deux exécutions utilisent les mêmes données et la même caméra.
- [ ] Les résultats peuvent être comparés.
- [ ] Les effets majeurs peuvent être isolés.
- [ ] Une régression est détectable avant release.

---

<a id="mvp-036"></a>

## MVP-036 — Auditer et compléter les tests métier

| Métadonnée | Valeur |
|---|---|
| Priorité | P0 |
| Estimation | 8 points |
| Statut | À faire |
| Dépendances | MVP-030-A2 à MVP-031 |

### Périmètre

Tester systématiquement :

- génération seedée ;
- identifiants ;
- économie ;
- potentiel planétaire ;
- énergie ;
- saturation du stockage ;
- réservations ;
- bâtiments ;
- recherches ;
- crafts par lot ;
- annulations ;
- flottes ;
- routes ;
- missions ;
- transport ;
- récolte ;
- analyse satellite ;
- combat ;
- colonisation ;
- sauvegarde ;
- reprise ;
- droits de faction ;
- niveaux de connaissance.

### Critères d’acceptation

- [ ] Les tests sont indépendants du rendu.
- [ ] Aucun test ne dépend du framerate réel.
- [ ] Les cas d’échec critiques sont couverts.
- [ ] Les reprises de sauvegarde sont testées pendant les missions.
- [ ] `cargo test --workspace` passe de façon reproductible.

---

<a id="mvp-037"></a>

## MVP-037 — Ajouter un smoke test de la boucle complète

| Métadonnée | Valeur |
|---|---|
| Priorité | P0 |
| Estimation | 8 points |
| Statut | À faire |
| Dépendances | MVP-031, MVP-036 |

### Scénario

```text
Nouvelle partie
→ améliorer l’économie
→ produire de la science
→ rechercher la sonde
→ construire une sonde
→ sonder une planète
→ construire un satellite
→ analyser la planète
→ récolter des ressources
→ construire une flotte
→ combattre si nécessaire
→ rechercher la colonisation
→ construire une Arche
→ fonder une deuxième colonie
→ sauvegarder
→ charger
→ continuer
```

### Critères d’acceptation

- [ ] Le scénario atteint une deuxième colonie.
- [ ] Aucun stock n’est dupliqué.
- [ ] Aucun vaisseau n’est dupliqué.
- [ ] Aucune mission n’est appliquée deux fois.
- [ ] Le chargement conserve l’état exact.
- [ ] Le test est documenté.
- [ ] Le test peut être lancé avant chaque release.

---

<a id="mvp-038"></a>

## MVP-038 — Équilibrer, polir et packager le MVP de playtest

| Métadonnée | Valeur |
|---|---|
| Priorité | P1 |
| Estimation | 8 points |
| Statut | À faire |
| Dépendances | MVP-033 à MVP-037 |

### Objectif

Transformer les systèmes terminés en une expérience courte, compréhensible et distribuable.

### Périmètre

- Viser une réussite en 60 à 90 minutes.
- Réduire les temps morts.
- Corriger les blocages économiques.
- Équilibrer :
  - productions ;
  - coûts ;
  - temps ;
  - recherches ;
  - flottes ;
  - récoltes ;
  - colonisation.
- Polir :
  - notifications ;
  - erreurs ;
  - transitions ;
  - navigation ;
  - rapports.
- Vérifier 720p et 1080p.
- Créer des builds Windows et macOS.
- Ajouter un guide de playtest.
- Ajouter une checklist de retours.
- Tester avec une personne ne connaissant pas le projet.

### Critères d’acceptation

- [ ] Un playtesteur externe atteint une deuxième colonie sans commande debug.
- [ ] La condition de réussite est atteignable dans la durée cible.
- [ ] Les commandes qualité passent.
- [ ] Les builds Windows et macOS démarrent.
- [ ] Les assets sont inclus dans les packages.
- [ ] Un guide de test est fourni.

---

# Phase post-MVP — Univers réactif

<a id="mvp-039"></a>

## MVP-039 — Ajouter les attaques ennemies et les défenses planétaires

| Métadonnée | Valeur |
|---|---|
| Priorité | P2 |
| Estimation | 13 points |
| Statut | Après MVP |
| Dépendances | MVP-030-A3, MVP-030-A6, MVP-031, MVP-036 |

### Objectif

Faire de l’univers un acteur capable de menacer les colonies du joueur et donner une utilité concrète aux défenses planétaires.

### Périmètre — défenses

- Catalogue de défenses.
- Production par quantité.
- Défense légère.
- Défense moyenne.
- Défense lourde.
- Défense orbitale.
- Classes Léger, Moyen et Lourd.
- Bonus contre certains attaquants.
- Inventaire de défenses réellement construites.
- Suppression des défenses inventées sur les colonies joueur.
- Affichage séparé :
  - garnison ;
  - défense ;
  - population ;
  - dernier renseignement.

### Périmètre — IA hostile

- Activer une faction ennemie.
- Ajouter une source de commandes IA.
- Sélectionner une cible connue.
- Former une flotte.
- Lancer une attaque.
- Ajouter une fréquence d’attaque configurable.
- Ajouter une limite de pression.
- Éviter d’attaquer trop tôt.
- Éviter de cibler sans cesse la même colonie.

### Périmètre — attaque d’une colonie

Pour la première version :

- alerte d’approche ;
- ETA ;
- combat orbital ;
- pertes ;
- destruction de défenses ;
- pillage limité ;
- retour de la flotte ennemie ;
- rapport persistant.

### Décision MVP recommandée

Une défaite ne transfère pas immédiatement la colonie à l’ennemi.

Conséquences possibles :

- perte de vaisseaux ;
- perte de défenses ;
- pillage de ressources ;
- réduction temporaire de production ;
- réparations nécessaires.

### Hors périmètre

- Conquête totale de colonie.
- Occupation longue.
- Diplomatie avancée.
- Guerre de factions complète.
- Interception en trajet.
- Alliance.
- Traités.
- Joueur contre joueur.

### Critères d’acceptation

- [ ] Une défense affichée a réellement été construite.
- [ ] Une faction hostile peut lancer une attaque.
- [ ] Le joueur reçoit une alerte avant l’arrivée.
- [ ] Les défenses participent au combat.
- [ ] Les pertes sont persistantes.
- [ ] Le pillage respecte la capacité cargo.
- [ ] Une attaque reprise depuis une sauvegarde se termine au même tick.
- [ ] Une défaite ne désynchronise pas la présence planétaire et la colonie.
- [ ] Les attaques sont configurables et testables.

---

# Backlog hors périmètre du MVP

Les idées suivantes sont conservées mais ne doivent pas entrer dans le scope immédiat :

- Installation permanente sur une planète distante.
- Avant-postes miniers.
- Satellites permanents.
- Interception en trajet.
- Escortes.
- Marché.
- Commerce automatisé.
- Gouverneurs.
- Modules de vaisseaux.
- Officiers.
- Expérience des équipages.
- Diplomatie avancée.
- Conquête totale de colonies.
- Guerre entre factions.
- Multijoueur.
- Sauvegarde cloud.
- Intégration Steamworks.

---

# Ordre d’implémentation recommandé

```text
MVP-030-A1  Stabilisation interface
MVP-030-A2  Économie planétaire et recherche
MVP-030-A3  Lots et annulation des files
MVP-030-A4  Planificateur de missions
MVP-030-A5  Analyse par satellite
MVP-030-A6  Diversification des flottes
MVP-030-A7  Polish UX et cadrage narratif
MVP-031     Sauvegarde et migrations
MVP-032     Onboarding et objectifs
MVP-033     Condition de réussite
MVP-034     Presets graphiques
MVP-035     Benchmark
MVP-036     Audit des tests
MVP-037     Smoke test complet
MVP-038     Équilibrage et packaging
MVP-039     Attaques ennemies et défenses
```

---

# Définition globale de réussite du MVP

Le MVP consolidé est considéré comme jouable lorsqu’un nouveau joueur peut :

1. identifier sa colonie active ;
2. comprendre les ressources de sa planète ;
3. comprendre ses bâtiments ;
4. améliorer sa production ;
5. comprendre la production scientifique ;
6. progresser dans les prérequis ;
7. fabriquer plusieurs unités en une commande ;
8. annuler une file ;
9. préparer un transport avec une cargaison exacte ;
10. envoyer une sonde ;
11. envoyer un satellite d’analyse ;
12. choisir une flotte de récolte ;
13. choisir entre plusieurs cargos ;
14. choisir entre plusieurs vaisseaux militaires ;
15. combattre ;
16. débloquer difficilement l’Arche coloniale — Essor ;
17. fonder une deuxième colonie ;
18. sauvegarder ;
19. recharger ;
20. atteindre la condition de réussite sans commande debug.
