# Roadmap Galactic — Issues MVP

> Document généré depuis l’export JSON Cadylo fourni le 27 juillet 2026.
> Statuts resynchronisés manuellement le 31 juillet 2026 avec `docs/mvp_architecture.md`
> et l'état réel du code (MVP-017 à MVP-023-C étaient marquées à tort « À faire »).

## Vue d’ensemble

- **45 issues** au total
- **37 terminées**
- **8 à faire**

## Sommaire de la roadmap

| État | MVP | Issue | Priorité | Estimation |
|---:|---|---|---:|---:|
| ✅ | [MVP-001](#eng-9) — Auditer et figer le POC comme baseline du MVP | [ENG-9](https://cadylo.app/galactic/issue/ENG-9) | P1 | 3 |
| ✅ | [MVP-002](#eng-10) — Séparer simulation, domaine, rendu et interface | [ENG-10](https://cadylo.app/galactic/issue/ENG-10) | P1 | 8 |
| ✅ | [MVP-003](#eng-11) — Fixer la seed MVP et introduire des identifiants stables | [ENG-11](https://cadylo.app/galactic/issue/ENG-11) | P1 | 5 |
| ✅ | [MVP-004](#eng-12) — Séparer l'univers généré de l'état mutable de partie | [ENG-12](https://cadylo.app/galactic/issue/ENG-12) | P1 | 8 |
| ✅ | [MVP-005](#eng-13) — Implémenter le temps stratégique, la pause et les vitesses | [ENG-13](https://cadylo.app/galactic/issue/ENG-13) | P1 | 5 |
| ✅ | [MVP-006](#eng-14) — Générer le graphe d'univers et les routes entre systèmes | [ENG-14](https://cadylo.app/galactic/issue/ENG-14) | P1 | 8 |
| ✅ | [MVP-007](#eng-15) — Adapter la vue Univers au voisinage découvert | [ENG-15](https://cadylo.app/galactic/issue/ENG-15) | P1 | 8 |
| ✅ | [MVP-008](#eng-16) — Définir le système de départ et la planète mère | [ENG-16](https://cadylo.app/galactic/issue/ENG-16) | P1 | 5 |
| ✅ | [MVP-009](#eng-17) — Implémenter les niveaux de connaissance des objets | [ENG-17](https://cadylo.app/galactic/issue/ENG-17) | P1 | 8 |
| ✅ | [MVP-010](#eng-18) — Adapter les inspecteurs aux informations partielles | [ENG-18](https://cadylo.app/galactic/issue/ENG-18) | P1 | 5 |
| ✅ | [MVP-010-B](#eng-47) — Implémenter le picking souris, le survol et les sélections ambiguës | [ENG-47](https://cadylo.app/galactic/issue/ENG-47) | P1 | 8 |
| ✅ | [MVP-011](#eng-19) — Implémenter le registre de ressources et l'énergie | [ENG-19](https://cadylo.app/galactic/issue/ENG-19) | P1 | 8 |
| ✅ | [MVP-012](#eng-20) — Ajouter production planétaire et capacités de stockage | [ENG-20](https://cadylo.app/galactic/issue/ENG-20) | P1 | 8 |
| ✅ | [MVP-013](#eng-21) — Définir le catalogue des bâtiments du MVP | [ENG-21](https://cadylo.app/galactic/issue/ENG-21) | P1 | 5 |
| ✅ | [MVP-014](#eng-22) — Implémenter la file de construction et les améliorations | [ENG-22](https://cadylo.app/galactic/issue/ENG-22) | P1 | 8 |
| ✅ | [MVP-015](#eng-23) — Construire l'écran de gestion planétaire | [ENG-23](https://cadylo.app/galactic/issue/ENG-23) | P1 | 8 |
| ✅ | [MVP-016](#eng-24) — Implémenter la recherche et l'arbre technologique minimal | [ENG-24](https://cadylo.app/galactic/issue/ENG-24) | P1 | 8 |
| ✅ | [MVP-016-B](#eng-51) — Externaliser le ruleset économique V1 | [ENG-51](https://cadylo.app/galactic/issue/ENG-51) | P1 | — |
| ✅ | [MVP-017](#eng-25) — Ajouter une file générique de craft au chantier spatial | [ENG-25](https://cadylo.app/galactic/issue/ENG-25) | P1 | 8 |
| ✅ | [MVP-018](#eng-26) — Généraliser la propriété avec les factions | [ENG-26](https://cadylo.app/galactic/issue/ENG-26) | P1 | 5 |
| ✅ | [MVP-019](#eng-27) — Introduire les commandes génériques et relations dormantes | [ENG-27](https://cadylo.app/galactic/issue/ENG-27) | P2 | 8 |
| ✅ | [MVP-020](#eng-28) — Définir les flottes, vaisseaux et capacités | [ENG-28](https://cadylo.app/galactic/issue/ENG-28) | P1 | 8 |
| ✅ | [MVP-021](#eng-29) — Implémenter le moteur de trajet et la machine d'état des missions | [ENG-29](https://cadylo.app/galactic/issue/ENG-29) | P1 | 8 |
| ✅ | [MVP-022](#eng-30) — Ajouter la sonde et la mission de reconnaissance | [ENG-30](https://cadylo.app/galactic/issue/ENG-30) | P1 | 8 |
| ✅ | [MVP-023](#eng-31) — Propager la découverte aux systèmes suivants | [ENG-31](https://cadylo.app/galactic/issue/ENG-31) | P1 | 5 |
| ✅ | [MVP-023-B](#eng-48) — Structurer la galaxie en secteurs déterministes | [ENG-48](https://cadylo.app/galactic/issue/ENG-48) | P1 | 8 |
| ✅ | [MVP-023-C](#eng-49) — Ajouter la projection aplatie et les presets d'échelle galactique | [ENG-49](https://cadylo.app/galactic/issue/ENG-49) | P1 | 8 |
| ✅ | [MVP-024](#eng-32) — Ajouter l'analyse planétaire et les règles de colonisabilité | [ENG-32](https://cadylo.app/galactic/issue/ENG-32) | P1 | 8 |
| ✅ | [MVP-025](#eng-33) — Ajouter les occupants, forces et défenses planétaires | [ENG-33](https://cadylo.app/galactic/issue/ENG-33) | P1 | 5 |
| ✅ | [MVP-025-B](#eng-52) — Ajouter les attaques, le combat V1 et les rapports | [ENG-52](https://cadylo.app/galactic/issue/ENG-52) | P1 | — |
| ✅ | [MVP-026](#eng-34) — Implémenter le vaisseau-colonie et la mission de colonisation | [ENG-34](https://cadylo.app/galactic/issue/ENG-34) | P1 | 8 |
| ✅ | [MVP-027](#eng-35) — Initialiser une nouvelle colonie jouable | [ENG-35](https://cadylo.app/galactic/issue/ENG-35) | P1 | 8 |
| ✅ | [MVP-028](#eng-36) — Ajouter la gestion multi-colonies | [ENG-36](https://cadylo.app/galactic/issue/ENG-36) | P1 | 8 |
| ✅ | [MVP-029](#eng-37) — Ajouter les missions de transport entre colonies | [ENG-37](https://cadylo.app/galactic/issue/ENG-37) | P1 | 5 |
| ✅ | [MVP-029-B](#eng-53) — Ajouter les sites d'extraction et la récolte distante | [ENG-53](https://cadylo.app/galactic/issue/ENG-53) | P1 | — |
| ✅ | [MVP-030](#eng-38) — Créer le HUD des flottes et missions | [ENG-38](https://cadylo.app/galactic/issue/ENG-38) | P2 | 8 |
| ✅ | [MVP-030-B](#eng-50) — Ajouter la navigation galactique avancée | [ENG-50](https://cadylo.app/galactic/issue/ENG-50) | P1 | 8 |
| ⬜ | [MVP-031](#eng-39) — Implémenter sauvegarde, chargement et migration V1 | [ENG-39](https://cadylo.app/galactic/issue/ENG-39) | P1 | 8 |
| ⬜ | [MVP-032](#eng-40) — Ajouter onboarding et objectifs contextuels | [ENG-40](https://cadylo.app/galactic/issue/ENG-40) | P2 | 5 |
| ⬜ | [MVP-033](#eng-41) — Définir et implémenter la condition de réussite du MVP | [ENG-41](https://cadylo.app/galactic/issue/ENG-41) | P2 | 3 |
| ⬜ | [MVP-034](#eng-42) — Ajouter les presets graphiques et le mode GPU intégré | [ENG-42](https://cadylo.app/galactic/issue/ENG-42) | P2 | 5 |
| ⬜ | [MVP-035](#eng-43) — Intégrer diagnostics et benchmark reproductible | [ENG-43](https://cadylo.app/galactic/issue/ENG-43) | P2 | 5 |
| ⬜ | [MVP-036](#eng-44) — Couvrir déterminisme et règles métier par des tests | [ENG-44](https://cadylo.app/galactic/issue/ENG-44) | P1 | 8 |
| ⬜ | [MVP-037](#eng-45) — Ajouter un smoke test de la boucle complète | [ENG-45](https://cadylo.app/galactic/issue/ENG-45) | P1 | 8 |
| ⬜ | [MVP-038](#eng-46) — Équilibrer, polir et packager le MVP de playtest | [ENG-46](https://cadylo.app/galactic/issue/ENG-46) | P2 | 8 |

---

## Détail des issues

<a id="eng-9"></a>

## MVP-001 — Auditer et figer le POC comme baseline du MVP

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-9](https://cadylo.app/galactic/issue/ENG-9) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 3 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Transformer le POC validé en point de départ stable et documenté avant l'ajout de gameplay.

### Contenu
- Créer une branche ou un tag de référence du POC actuel.
- Inventorier les plugins, states, ressources, composants et systèmes existants.
- Identifier les éléments à conserver, refactorer ou supprimer.
- Mesurer la scène de référence en build release sur le poste actuel.
- Documenter les contrôles et les limites connues.

### Critères d'acceptation
- [ ] Le POC compile et s'exécute avec `cargo run --release`.
- [ ] Une baseline visuelle et une baseline de performance sont consignées dans le dépôt.
- [ ] Aucun comportement existant n'est perdu sans justification.
- [ ] Le README indique clairement la version de Bevy et les commandes qualité.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-10"></a>

## MVP-002 — Séparer simulation, domaine, rendu et interface

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-10](https://cadylo.app/galactic/issue/ENG-10) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Mettre en place une architecture qui permette d'ajouter économie, missions et colonies sans coupler le gameplay aux entités visuelles Bevy.

### Contenu
- Créer des modules ou crates distincts pour le domaine, la simulation, la persistance et le client Bevy.
- Conserver la donnée métier hors des composants purement visuels.
- Définir les événements entre simulation et présentation.
- Définir les responsabilités des plugins Bevy.
- Ajouter une documentation d'architecture courte.

### Critères d'acceptation
- [ ] La simulation peut être testée sans caméra ni rendu 3D.
- [ ] Aucune logique de production ou de mission ne dépend d'un `Entity` Bevy.
- [ ] Les vues peuvent être despawnées et recréées sans perdre l'état métier.
- [ ] Les principaux flux de données sont documentés.

### Dépendances
- MVP-001

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-11"></a>

## MVP-003 — Fixer la seed MVP et introduire des identifiants stables

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-11](https://cadylo.app/galactic/issue/ENG-11) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 5 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Garantir que le même univers et les mêmes objets sont générés pendant toute la durée du MVP.

### Contenu
- Centraliser la seed MVP dans une configuration unique.
- Créer des newtypes stables pour univers, système, étoile, planète, lune, faction, flotte, mission et colonie.
- Rendre la génération déterministe avec un RNG seedé.
- Définir une stratégie reproductible d'attribution des identifiants.
- Ajouter un hash ou numéro de version de génération.

### Critères d'acceptation
- [ ] Deux générations avec la même seed produisent les mêmes objets et identifiants.
- [ ] Aucun identifiant persistant ne dépend de `Entity`.
- [ ] La seed active est visible dans l'interface de debug.
- [ ] Un test de régression protège la seed de référence.

### Dépendances
- MVP-002

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-12"></a>

## MVP-004 — Séparer l'univers généré de l'état mutable de partie

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-12](https://cadylo.app/galactic/issue/ENG-12) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Distinguer ce qui est dérivé de la seed de ce qui résulte des actions du joueur.

### Contenu
- Définir `UniverseDefinition` ou équivalent pour la donnée générée immuable.
- Définir `GameState` pour découvertes, stocks, bâtiments, recherches, flottes et colonies.
- Mettre en place des repositories ou index d'accès par identifiant stable.
- Empêcher les données de génération d'être modifiées directement.
- Prévoir un numéro de version du format d'état.

### Critères d'acceptation
- [ ] La régénération depuis la seed reconstruit la définition initiale.
- [ ] Les actions du joueur ne modifient que l'état mutable.
- [ ] Un monde visuel peut être reconstruit depuis définition + état.
- [ ] Les tests couvrent l'accès à un système, une planète et une colonie par ID.

### Dépendances
- MVP-003

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-13"></a>

## MVP-005 — Implémenter le temps stratégique, la pause et les vitesses

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-13](https://cadylo.app/galactic/issue/ENG-13) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 5 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Fournir une horloge de simulation stable pour la production, les constructions, les recherches et les missions.

### Contenu
- Créer un compteur de ticks stratégiques indépendant du framerate.
- Ajouter pause, vitesse x1, x2 et x4.
- Maintenir caméra et interface interactives pendant la pause.
- Définir des timestamps ou durées de simulation sérialisables.
- Ajouter les contrôles et l'indicateur de vitesse au HUD.

### Critères d'acceptation
- [ ] La simulation donne le même résultat à FPS différents.
- [ ] Pause et reprise ne dupliquent ni ne sautent des ticks.
- [ ] Les vitesses modifient la simulation, pas la sensibilité de caméra.
- [ ] Le tick courant peut être sauvegardé.

### Dépendances
- MVP-002
- MVP-004

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-14"></a>

## MVP-006 — Générer le graphe d'univers et les routes entre systèmes

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-14](https://cadylo.app/galactic/issue/ENG-14) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Transformer la carte globale en graphe navigable plutôt qu'en simple nuage de systèmes.

### Contenu
- Définir un univers MVP de 12 à 20 systèmes.
- Générer ou figer les routes pour la seed de référence.
- Garantir la connexité du graphe et éviter les doublons.
- Calculer les voisinages et chemins par nombre de sauts.
- Afficher les routes découvertes dans la vue Univers.

### Critères d'acceptation
- [x] Tous les systèmes prévus sont accessibles depuis le système de départ.
- [ ] Les routes sont identiques pour la seed de référence.
- [ ] Un chemin peut être calculé entre deux systèmes connectés.
- [ ] Les routes inconnues ne sont pas affichées au joueur.

### Dépendances
- MVP-003
- MVP-004

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-15"></a>

## MVP-007 — Adapter la vue Univers au voisinage découvert

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-15](https://cadylo.app/galactic/issue/ENG-15) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Faire évoluer la vue galactique du POC pour afficher seulement les systèmes pertinents et conserver de bonnes performances.

### Contenu
- Afficher les systèmes connus, détectés et leurs routes visibles.
- Ne pas instancier les systèmes totalement inconnus.
- Ajouter un niveau de détail sémantique selon le zoom.
- Conserver navigation, sélection, zoom et transition vers la vue Système.
- Prévoir un mode debug permettant d'afficher tout le graphe.

### Critères d'acceptation
- [ ] La vue normale ne révèle aucun système hors frontière de découverte.
- [ ] Le système sélectionné reste lisible à tous les niveaux de zoom utiles.
- [ ] La carte fonctionne avec le preset graphique Low.
- [ ] Le passage Univers → Système → Univers conserve le contexte.

### Dépendances
- MVP-006

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-16"></a>

## MVP-008 — Définir le système de départ et la planète mère

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-16](https://cadylo.app/galactic/issue/ENG-16) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 5 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Créer un point de départ cohérent, habitable et immédiatement jouable dans la seed MVP.

### Contenu
- Choisir un système de départ stable dans la seed de référence.
- Garantir une planète mère habitable avec des ressources équilibrées.
- Créer la faction joueur, sa première colonie et ses stocks initiaux.
- Définir les niveaux de bâtiments de départ.
- Révéler uniquement les informations initiales prévues.

### Critères d'acceptation
- [ ] Une nouvelle partie démarre toujours au même endroit avec la seed MVP.
- [ ] La planète mère peut soutenir la boucle de départ sans blocage.
- [ ] Le joueur voit son système et quelques systèmes voisins détectés.
- [ ] Les données de départ sont configurables sans modifier la génération générale.

### Dépendances
- MVP-004
- MVP-006

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-17"></a>

## MVP-009 — Implémenter les niveaux de connaissance des objets

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-17](https://cadylo.app/galactic/issue/ENG-17) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Rendre l'exploration utile en séparant visibilité géométrique et connaissance des propriétés.

### Contenu
- Créer les niveaux Inconnu, Détecté, Sondé, Analysé et Colonisé.
- Appliquer les niveaux aux systèmes et corps célestes.
- Définir précisément les champs révélés à chaque niveau.
- Stocker les découvertes dans l'état de partie.
- Émettre des événements lors d'un changement de niveau.

### Critères d'acceptation
- [ ] Un objet visible peut afficher des valeurs inconnues.
- [ ] Les informations révélées restent connues après changement de vue.
- [ ] Les niveaux sont sauvegardables.
- [ ] La matrice des données visibles par niveau est testée.

### Dépendances
- MVP-004
- MVP-008

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-18"></a>

## MVP-010 — Adapter les inspecteurs aux informations partielles

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-18](https://cadylo.app/galactic/issue/ENG-18) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 5 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 24/07/2026 |

### Objectif
Mettre à jour le HUD pour communiquer clairement ce qui est connu, estimé ou inconnu.

### Contenu
- Afficher des placeholders cohérents pour les données inconnues.
- Distinguer estimation et valeur précise.
- Expliquer l'action nécessaire pour révéler une information.
- Mettre à jour les inspecteurs système, planète et lune.
- Ajouter des icônes ou styles par niveau de connaissance.

### Critères d'acceptation
- [ ] Le joueur comprend pourquoi une donnée est masquée.
- [ ] Aucune information secrète ne fuit dans le HUD ou les tooltips.
- [ ] Le passage au niveau supérieur actualise l'inspecteur immédiatement.
- [ ] La lisibilité reste correcte en preset Low.

### Dépendances
- MVP-009

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-47"></a>

## MVP-010-B — Implémenter le picking souris, le survol et les sélections ambiguës

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-47](https://cadylo.app/galactic/issue/ENG-47) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 24/07/2026 |

### Objectif
Rendre la carte galactique précise et agréable à utiliser à la souris avant la multiplication des écrans de gestion.

### Contenu
- Implémenter un picking en espace écran pour les systèmes, planètes et lunes visibles.
- Ajouter un état de survol stable avec halo, présélection et tooltip respectant le niveau de connaissance.
- Ajouter clic gauche pour sélectionner et double-clic pour focaliser ou ouvrir l'objet.
- Classer les candidats par distance au curseur, profondeur et priorité visuelle.
- Ouvrir un sélecteur contextuel lorsque plusieurs objets se recouvrent.
- Permettre de parcourir les candidats ambigus au clavier sans perdre la souris.
- Conserver les contrôles clavier et les identifiants métier comme solution de repli.
- Préparer le picking à utiliser les positions visuelles transformées du futur mode aplati.

### Critères d'acceptation
- [ ] Un système ou corps céleste visible peut être sélectionné uniquement à la souris.
- [ ] Un objet inconnu ou non rendu ne peut jamais être sélectionné.
- [ ] Le survol et les tooltips ne révèlent aucune information interdite.
- [ ] Une superposition de plusieurs candidats produit un choix déterministe et compréhensible.
- [ ] La sélection met immédiatement à jour l'inspecteur correspondant.
- [ ] Le comportement est utilisable en 1280×720 et 1920×1080.

### Dépendances
- MVP-010

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-19"></a>

## MVP-011 — Implémenter le registre de ressources et l'énergie

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-19](https://cadylo.app/galactic/issue/ENG-19) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 24/07/2026 |

### Objectif
Créer le socle économique commun aux constructions, recherches, crafts et missions.

### Contenu
- Définir Métal, Cristal et Carburant comme ressources stockées.
- Définir l'énergie comme capacité produite et consommée.
- Créer des opérations atomiques de crédit, débit et réservation.
- Empêcher les stocks négatifs et les doubles dépenses.
- Préparer les coûts sérialisables et configurables.

### Critères d'acceptation
- [ ] Une dépense insuffisamment financée échoue sans modifier les stocks.
- [ ] Les coûts peuvent combiner les trois ressources.
- [ ] Le bilan énergétique est calculable par colonie.
- [ ] Les opérations économiques sont couvertes par des tests unitaires.

### Dépendances
- MVP-004

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-20"></a>

## MVP-012 — Ajouter production planétaire et capacités de stockage

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-20](https://cadylo.app/galactic/issue/ENG-20) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 24/07/2026 |

### Objectif
Faire produire les colonies en fonction de leurs bâtiments et caractéristiques planétaires.

### Contenu
- Calculer les productions par tick de simulation.
- Appliquer bonus et malus simples liés à la planète.
- Ajouter des capacités de stockage par ressource.
- Limiter ou arrêter la production lorsque le stockage est plein.
- Afficher production actuelle, capacité et temps avant saturation.

### Critères d'acceptation
- [ ] Les ressources augmentent de façon indépendante du framerate.
- [ ] Une énergie insuffisante réduit ou bloque les productions concernées selon une règle documentée.
- [ ] Les stocks ne dépassent pas leur capacité.
- [ ] Pause et vitesses produisent les résultats attendus.

### Dépendances
- MVP-005
- MVP-011

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-21"></a>

## MVP-013 — Définir le catalogue des bâtiments du MVP

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-21](https://cadylo.app/galactic/issue/ENG-21) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 5 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 24/07/2026 |

### Objectif
Formaliser un petit ensemble de bâtiments configurables avec coûts, durées, effets et prérequis.

### Contenu
- Créer Mine de métal, Extracteur de cristal, Raffinerie de carburant et Centrale énergétique.
- Créer Entrepôt, Centre de construction, Laboratoire et Chantier spatial.
- Définir les formules de coût et durée par niveau.
- Définir les effets de chaque niveau.
- Stocker les définitions dans des assets ou fichiers de données simples.

### Critères d'acceptation
- [ ] Chaque bâtiment possède une définition unique et validée.
- [ ] Les coûts et effets peuvent être ajustés sans modifier les systèmes de simulation.
- [ ] Les prérequis invalides sont détectés au chargement.
- [ ] Le catalogue ne dépasse pas le scope MVP sans décision explicite.

### Dépendances
- MVP-011
- MVP-012

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-22"></a>

## MVP-014 — Implémenter la file de construction et les améliorations

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-22](https://cadylo.app/galactic/issue/ENG-22) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 24/07/2026 |

### Objectif
Permettre au joueur de construire et améliorer les bâtiments de chaque colonie.

### Contenu
- Créer une file de construction par colonie.
- Valider ressources, énergie, niveau maximal et prérequis.
- Réserver ou débiter les ressources au lancement selon une règle unique.
- Faire progresser la construction avec le temps stratégique.
- Appliquer l'effet du nouveau niveau à la fin.

### Critères d'acceptation
- [ ] Un bâtiment peut passer du niveau N au niveau N+1.
- [ ] Une construction impossible explique la cause.
- [ ] Une construction se termine correctement à x1, x2 et x4.
- [ ] La file survit à une sauvegarde/chargement.

### Dépendances
- MVP-005
- MVP-013

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-23"></a>

## MVP-015 — Construire l'écran de gestion planétaire

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-23](https://cadylo.app/galactic/issue/ENG-23) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 27/07/2026 |

### Objectif
Donner au joueur une interface centrale pour comprendre et faire évoluer sa colonie.

### Contenu
- Afficher stocks, productions, énergie et stockage.
- Afficher bâtiments, niveaux, effets actuels et prochains niveaux.
- Afficher coûts, durées, prérequis et raisons de verrouillage.
- Afficher la file de construction.
- Permettre de sélectionner la colonie active.

### Critères d'acceptation
- [ ] Le joueur peut améliorer un bâtiment sans passer par un outil debug.
- [ ] Toutes les valeurs affichées viennent de la simulation.
- [ ] Le HUD reste utilisable en 1280×720.
- [ ] Les erreurs de lancement sont visibles et non bloquantes.

### Dépendances
- MVP-014

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-24"></a>

## MVP-016 — Implémenter la recherche et l'arbre technologique minimal

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-24](https://cadylo.app/galactic/issue/ENG-24) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 27/07/2026 |

### Objectif
Débloquer progressivement exploration, logistique et colonisation avec un arbre court et lisible.

### Contenu
- Définir Détection spatiale, Propulsion, Capacité cargo, Extraction distante, Analyse planétaire et Colonisation.
- Créer coûts, durées et prérequis de recherche.
- Créer une file de recherche globale ou par colonie, puis documenter le choix.
- Lier les recherches au niveau du Laboratoire.
- Émettre des événements de déblocage.

### Critères d'acceptation
- [ ] Les six technologies peuvent être recherchées dans un ordre valide.
- [ ] Les technologies débloquent des actions ou crafts réels.
- [ ] Une technologie déjà acquise ne peut pas être relancée.
- [ ] La progression est sauvegardable et testée.

### Dépendances
- MVP-005
- MVP-013
- MVP-014

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-51"></a>

## MVP-016-B — Externaliser le ruleset économique V1

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-51](https://cadylo.app/galactic/issue/ENG-51) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | Non estimée |
| Créée le | 27/07/2026 |
| Mise à jour le | 29/07/2026 |

### Objectif

Rendre configurable tout le contenu et l'équilibrage économique existants avant
d'ajouter le craft, les vaisseaux et les missions.

### Périmètre

- Ruleset externe versionné chargé au démarrage.
- Fichiers dédiés au manifeste, à l'économie, aux bâtiments, aux technologies et
  au scénario initial.
- Identifiants textuels stables pour les bâtiments, technologies, ressources et
  capacités.
- Noms, descriptions, coûts, durées, progressions, productions, stockages,
  énergie, prérequis, déblocages et limites de files configurables.
- Ressources, bâtiments et technologies de départ configurables.
- Conversion des durées en ticks au chargement.
- Validation des doublons, références absentes, cycles, valeurs invalides et
  effets inconnus.
- `ruleset_id`, version de schéma et empreinte structurelle dans les sauvegardes.
- Un changement de texte seul ne rend pas une sauvegarde incompatible.

### Contraintes

- Les algorithmes, commandes, événements et comportements fondamentaux restent
  implémentés en Rust.
- Pas de hot reload pendant une partie pour cette première version.
- Les textes français peuvent rester dans les catalogues.

### Critères d'acceptation

- Les coûts, durées, textes et progressions existants se modifient sans toucher
  au code Rust.
- Un bâtiment ou une technologie utilisant des effets déjà connus peut être
  ajouté par configuration.
- Un ruleset invalide est refusé avec des erreurs précises.
- La simulation reste déterministe.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-25"></a>

## MVP-017 — Ajouter une file générique de craft au chantier spatial

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-25](https://cadylo.app/galactic/issue/ENG-25) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 31/07/2026 |

### Objectif

Introduire une file de fabrication générique au chantier spatial, alimentée par
des définitions de contenu configurables et réutilisable par les futurs
vaisseaux, sondes, transports, défenses et modules de soutien.

### Périmètre

- Identifiants `CraftableId` stables, sans enum Rust par objet fabriqué.
- Catalogue de craft chargé depuis le ruleset.
- Coûts, durées, prérequis, capacités et textes définis par les données.
- Réservation des ressources et validation des prérequis à la mise en file.
- File séquentielle avec progression uniquement sur les ticks stratégiques.
- Commandes et événements métier génériques.
- Sauvegarde, restauration et validation de la version du catalogue.
- Interface minimale du chantier et retours d'erreur explicites.

### Hors périmètre

- Composition des flottes.
- Déplacement et missions.
- Résolution des combats.

### Critères d'acceptation

- Un craftable utilisant un comportement déjà pris en charge peut être ajouté
  ou équilibré sans modifier le code Rust.
- Deux exécutions identiques produisent le même résultat de simulation.
- La file et sa progression survivent à une sauvegarde/reprise.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-26"></a>

## MVP-018 — Généraliser la propriété avec les factions

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-26](https://cadylo.app/galactic/issue/ENG-26) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 5 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 31/07/2026 |

### Objectif

Généraliser la propriété des colonies et planètes avec des factions stables,
sans confondre connaissance, occupation et contrôle territorial.

### Périmètre

- `FactionId` stable et définitions de factions configurables.
- Propriétaire réel distinct des informations connues par le joueur.
- États minimaux de contrôle : neutre, hostile, contesté, sécurisé, colonisé.
- Compatibilité des colonies existantes avec la faction du joueur.
- Événements métier lors d'un changement de propriétaire ou de contrôle.
- Persistance et migration de l'état territorial.

### Hors périmètre

- Diplomatie avancée.
- Résolution des attaques.
- Modèle détaillé de population.

### Critères d'acceptation

- Plusieurs factions peuvent posséder ou occuper des objets du monde.
- Une planète peut être connue sans être contrôlée.
- L'état est déterministe et sauvegardé.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-27"></a>

## MVP-019 — Introduire les commandes génériques et relations dormantes

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-27](https://cadylo.app/galactic/issue/ENG-27) |
| Statut | Terminée |
| Priorité | P2 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 31/07/2026 |

### Objectif

Poser les contrats génériques nécessaires aux futures interactions entre
factions, sans implémenter prématurément un système diplomatique complet.

### Périmètre

- Commandes et événements adressés par `FactionId`.
- Relations minimales configurables : inconnue, neutre, hostile, alliée.
- Valeur par défaut et évolution déterministes.
- API de simulation exploitable par les missions et le contrôle territorial.
- Persistance des relations.

### Hors périmètre

- Négociations, traités, réputation et IA diplomatique.
- Interface diplomatique dédiée.

### Critères d'acceptation

- Les systèmes futurs peuvent interroger une relation sans dépendre de la
  faction du joueur codée en dur.
- Les relations non encore actives n'altèrent pas la boucle de jeu existante.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-28"></a>

## MVP-020 — Définir les flottes, vaisseaux et capacités

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-28](https://cadylo.app/galactic/issue/ENG-28) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 31/07/2026 |

### Objectif

Définir les vaisseaux et la composition des flottes sur un catalogue
configurable, sans encore déplacer ni engager les flottes.

### Périmètre

- `ShipId`, `FleetId` et capacités métier stables.
- Catalogue configurable : coût, temps de craft, vitesse, cargo, portée,
  capteurs, attaque, défense et capacités spéciales reconnues.
- Création et modification déterministes d'une flotte depuis des unités
  disponibles.
- Calculs agrégés de capacité, vitesse et cargo.
- Validation des compositions.
- Persistance des vaisseaux et flottes.

### Hors périmètre

- Trajets et consommation en mission.
- Combat.
- Simulateur de résultat.

### Critères d'acceptation

- Un vaisseau utilisant des capacités existantes peut être ajouté ou équilibré
  sans modifier Rust.
- Une flotte ne peut pas utiliser deux fois le même vaisseau.
- Les agrégats sont stables et couverts par des tests.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-29"></a>

## MVP-021 — Implémenter le moteur de trajet et la machine d'état des missions

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-29](https://cadylo.app/galactic/issue/ENG-29) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 31/07/2026 |

### Objectif

Fournir un moteur générique de déplacement et une machine d'état de mission,
réutilisables par la reconnaissance, l'attaque, le transport et la colonisation.

### Périmètre

- Ordre de mission avec origine, cible, flotte, type et instant de départ.
- États : préparation, trajet aller, résolution, retour, terminée ou annulée.
- Durées calculées à partir du graphe galactique et de la flotte.
- Progression exclusivement sur les ticks stratégiques.
- Verrouillage des vaisseaux engagés.
- Événements métier aux transitions.
- Sauvegarde/reprise exacte d'une mission en cours.

### Hors périmètre

- Résolution propre à chaque type de mission.
- Combat et gains de ressources.

### Critères d'acceptation

- Une mission reprise depuis une sauvegarde termine au même tick.
- Une flotte engagée ne peut pas recevoir un ordre incompatible.
- Les transitions invalides sont refusées explicitement.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-30"></a>

## MVP-022 — Ajouter la sonde et la mission de reconnaissance

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-30](https://cadylo.app/galactic/issue/ENG-30) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 31/07/2026 |

### Objectif

Permettre d'envoyer une sonde afin d'obtenir un renseignement progressif,
daté et potentiellement incomplet sur une planète cible.

### Périmètre

- Sonde définie dans le catalogue de craft.
- Mission de reconnaissance basée sur le moteur de trajet.
- Niveaux de connaissance distincts de l'état réel de la cible.
- Renseignement avec date d'observation, précision et fraîcheur.
- Révélation progressive : faction, ressources estimées, infrastructures,
  forces et défenses selon les capacités de la sonde.
- Rapport de reconnaissance persistant.

### Hors périmètre

- Analyse détaillée de colonisabilité.
- Détection active ou destruction des sondes.
- Simulateur de combat.

### Critères d'acceptation

- L'interface ne révèle jamais directement une donnée réelle non observée.
- Un renseignement ancien reste consultable avec sa date et son incertitude.
- La mission est déterministe et sauvegardable.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-31"></a>

## MVP-023 — Propager la découverte aux systèmes suivants

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-31](https://cadylo.app/galactic/issue/ENG-31) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 5 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 31/07/2026 |

### Objectif
Créer une frontière d'exploration progressive après chaque sondage.

### Contenu
- Lorsqu'un système est sondé, révéler ses routes directes autorisées.
- Faire apparaître les systèmes au bout de ces routes au niveau Détecté.
- Conserver les détails de ces nouveaux systèmes masqués.
- Mettre à jour la carte et les notifications.
- Éviter les révélations récursives involontaires.

### Critères d'acceptation
- [x] Sonder un système ouvre exactement le prochain anneau de découverte prévu.
- [x] Les systèmes nouvellement détectés sont sélectionnables mais non détaillés.
- [x] Les routes affichées correspondent à la connaissance du joueur.
- [x] La progression est reproductible après sauvegarde/chargement.

### Dépendances
- MVP-022

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-48"></a>

## MVP-023-B — Structurer la galaxie en secteurs déterministes

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-48](https://cadylo.app/galactic/issue/ENG-48) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 31/07/2026 |

### Objectif
Donner une structure mentale et géographique à la galaxie avant d'augmenter fortement le nombre de systèmes.

### Contenu
- Créer `SectorId` et `SectorDefinition` dans la définition immuable de l'univers.
- Répartir les systèmes en secteurs déterministes à partir de la seed.
- Viser 6 à 10 secteurs pour le preset MVP étendu.
- Définir centre, membres, nom et routes passerelles de chaque secteur.
- Garantir qu'un système appartient à un seul secteur.
- Afficher les noms et repères sectoriels selon le niveau de zoom.
- Respecter les connaissances du joueur afin qu'un secteur ne révèle pas ses systèmes inconnus.
- Versionner la génération et le fingerprint si la définition de l'univers change.

### Critères d'acceptation
- [x] Une même seed et un même preset produisent les mêmes secteurs et identifiants.
- [x] Tous les systèmes appartiennent exactement à un secteur.
- [x] Les secteurs restent cohérents avec le graphe et ses routes intersectorielles.
- [x] Les labels sectoriels ne révèlent aucun système inconnu.
- [x] La vue globale permet d'identifier plusieurs régions distinctes sans afficher tous les labels.
- [x] Les données sectorielles sont accessibles sans dépendre des entités Bevy.

### Dépendances
- MVP-023
- MVP-010-B

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-49"></a>

## MVP-023-C — Ajouter la projection aplatie et les presets d'échelle galactique

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-49](https://cadylo.app/galactic/issue/ENG-49) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 31/07/2026 |

### Objectif
Permettre de lire facilement la galaxie en 2,5D et augmenter sa taille sans sacrifier la navigation ni la lisibilité.

### Contenu
- Ajouter les presets Test 16 systèmes, MVP 64 systèmes et Stress 128 systèmes.
- Utiliser 64 systèmes comme cible jouable initiale, avec validation ultérieure d'un preset 96.
- Ajouter un basculement 3D ↔ projection aplatie avec la touche `P`.
- Interpoler la transition visuelle plutôt que téléporter les systèmes.
- Garder positions, routes, distances et durées stratégiques inchangées dans la simulation.
- Faire utiliser au picking les positions réellement affichées.
- Adapter labels, LOD, routes et repères sectoriels aux deux projections.
- Ajouter un test de performance simple du preset MVP en mode graphique Low.

### Critères d'acceptation
- [x] Le preset MVP génère 64 systèmes reproductibles et plusieurs secteurs.
- [x] Le passage 3D ↔ aplati ne modifie aucune distance ou route métier.
- [x] La sélection souris reste exacte pendant et après la transition.
- [x] Les systèmes inconnus restent masqués dans les deux projections.
- [x] Le retour en 3D restaure la disposition attendue sans dérive.
- [x] Le preset Test reste disponible pour les tests rapides et unitaires.
- [x] Le preset Stress permet de mesurer 128 systèmes sans devenir la configuration par défaut.

### Dépendances
- MVP-023-B
- MVP-010-B

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-32"></a>

## MVP-024 — Ajouter l'analyse planétaire et les règles de colonisabilité

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-32](https://cadylo.app/galactic/issue/ENG-32) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 29/07/2026 |

### Objectif

Transformer les renseignements disponibles en analyse planétaire exploitable
et déterminer explicitement si une planète peut être colonisée.

### Périmètre

- Mission ou action d'analyse nécessitant la technologie adaptée.
- Caractéristiques configurables : habitabilité, environnement, ressources et
  contraintes d'installation.
- Résultat connu séparé des données réelles de la planète.
- Règles déterministes de colonisabilité avec motifs de refus.
- Rapport d'analyse daté et sauvegardé.
- Présentation claire des conditions remplies et manquantes.

### Hors périmètre

- Attaque et sécurisation.
- Création de la colonie.
- Extraction distante.

### Critères d'acceptation

- [x] Une planète inconnue ou insuffisamment analysée ne peut pas être déclarée
  colonisable.
- [x] Le moteur retourne les raisons précises d'un refus.
- [x] Les règles d'équilibrage sont configurables lorsqu'elles utilisent des
  caractéristiques déjà reconnues.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-33"></a>

## MVP-025 — Ajouter les occupants, forces et défenses planétaires

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-33](https://cadylo.app/galactic/issue/ENG-33) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 5 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 29/07/2026 |

### Objectif

Représenter la présence hostile ou neutre sur une planète afin de préparer
l'attaque sans figer encore la formule de combat.

### Périmètre

- Population ou faction occupante.
- Forces stationnées et défenses orbitales ou terrestres.
- Définitions configurables des unités et défenses utilisant des statistiques
  reconnues par la simulation.
- État réel distinct du dernier renseignement connu du joueur.
- Mise à jour et persistance déterministes des forces.
- Affichage estimatif fondé uniquement sur les renseignements disponibles.

### Hors périmètre

- Mission d'attaque.
- Résolution des combats.
- Prédiction de victoire.

### Critères d'acceptation

- [x] Une cible peut posséder des forces inconnues ou partiellement estimées.
- [x] Les données réelles ne fuient pas dans l'interface du joueur.
- [x] Les forces et défenses survivent à une sauvegarde/reprise.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-52"></a>

## MVP-025-B — Ajouter les attaques, le combat V1 et les rapports

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-52](https://cadylo.app/galactic/issue/ENG-52) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | Non estimée |
| Créée le | 27/07/2026 |
| Mise à jour le | 27/07/2026 |

### Objectif

Introduire une première boucle d'attaque déterministe entre une flotte et les
forces d'une planète, avec un rapport exploitable par la sécurisation future.

### Périmètre

- Mission d'attaque basée sur le moteur de trajet.
- Instantané explicite de la flotte attaquante et de la défense réelle.
- Fonction pure `resolve_combat` paramétrée par des règles de combat versionnées.
- Résultat minimal : vainqueur, pertes, survivants, ressources récupérables,
  dommages et évolution du contrôle territorial.
- Application atomique du résultat à l'état de simulation.
- Rapport de combat persistant et consultable.
- Couverture des cas limites : égalité, destruction mutuelle, cible devenue
  invalide et reprise de sauvegarde.

### Hors périmètre

- Simulateur précombat.
- Combat tactique contrôlé directement.
- Diplomatie avancée et interception en trajet.

### Critères d'acceptation

- [x] Une même entrée et une même graine produisent exactement le même rapport.
- [x] L'interface du joueur ne reçoit pas d'informations défensives non observées
  avant le combat.
- [x] Les pertes et gains ne peuvent pas être appliqués deux fois.
- [x] Une planète n'est colonisable après attaque que si les règles la déclarent
  sécurisée.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-34"></a>

## MVP-026 — Implémenter le vaisseau-colonie et la mission de colonisation

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-34](https://cadylo.app/galactic/issue/ENG-34) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 27/07/2026 |

### Objectif

Permettre l'envoi d'un vaisseau-colonie vers une planète analysée et éligible,
sans transformer automatiquement une victoire militaire en colonie.

### Périmètre

- Vaisseau-colonie et chargement initial définis dans les catalogues.
- Mission de colonisation basée sur le moteur de trajet.
- Validation de l'habitabilité, des technologies et des ressources.
- Exigence d'une planète inhabitée ou préalablement sécurisée.
- Consommation du module de colonisation au succès.
- Événements métier préparant l'initialisation de la nouvelle colonie.

### Hors périmètre

- Combat pendant la colonisation.
- Interface complète de gestion multi-colonies.

### Critères d'acceptation

- [x] Une planète hostile non sécurisée refuse la colonisation avec un motif
  clair.
- [x] Les ressources et le vaisseau ne sont consommés qu'au moment défini par
  la règle de mission.
- [x] La mission est déterministe et sauvegardable.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-35"></a>

## MVP-027 — Initialiser une nouvelle colonie jouable

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-35](https://cadylo.app/galactic/issue/ENG-35) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 27/07/2026 |

### Objectif

Créer une colonie complète et immédiatement jouable à la réussite d'une mission
de colonisation.

### Périmètre

- Identité stable de la colonie et rattachement à la planète.
- Stocks, bâtiments, énergie et capacités initiales issus du scénario/ruleset.
- Transfert du chargement initial de la mission.
- Attribution à la faction du joueur et mise à jour du contrôle territorial.
- Événements de création et persistance.
- Compatibilité avec les systèmes de production, construction et recherche.

### Critères d'acceptation

- [x] La nouvelle colonie fonctionne avec les mêmes règles qu'une colonie
  initiale.
- [x] Aucune donnée initiale de contenu n'est dupliquée en dur dans le code
  Rust.
- [x] Une sauvegarde/reprise conserve exactement son état.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-36"></a>

## MVP-028 — Ajouter la gestion multi-colonies

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-36](https://cadylo.app/galactic/issue/ENG-36) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 27/07/2026 |

### Objectif

Permettre au joueur de gérer plusieurs colonies sans dupliquer les systèmes
économiques, de construction et de recherche.

### Périmètre

- Liste stable des colonies possédées par la faction du joueur.
- Sélection de la colonie active dans l'interface de gestion.
- Stocks, bâtiments, énergie et files de construction propres à chaque colonie.
- Recherche restant globale au joueur et alimentée par tous ses laboratoires.
- Validation des commandes avec un `ColonyId` explicite.
- Suppression des dépendances implicites à une colonie unique.
- Sauvegarde/reprise de la sélection et de toutes les colonies.

### Hors périmètre

- Transport automatique de ressources entre colonies.
- Gouverneurs, automatisation et spécialisation avancée.

### Critères d'acceptation

- [x] Une action sur une colonie ne modifie pas silencieusement une autre
  colonie.
- [x] Les productions scientifiques de toutes les colonies contribuent à la
  même recherche globale.
- [x] La navigation entre colonies ne change pas leur simulation déterministe.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-37"></a>

## MVP-029 — Ajouter les missions de transport entre colonies

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-37](https://cadylo.app/galactic/issue/ENG-37) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 5 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 27/07/2026 |

### Objectif

Permettre le transport de ressources entre colonies via une flotte et le moteur
de missions générique.

### Périmètre

- Ordre de transport avec origine, destination et cargaison.
- Validation du stock disponible et de la capacité cargo.
- Réservation ou retrait des ressources selon une règle explicite.
- Trajet aller, livraison et retour éventuel.
- Gestion déterministe d'une annulation ou d'une destination devenue invalide.
- Rapport de mission et persistance.

### Hors périmètre

- Extraction distante.
- Interception et combat en trajet.

### Critères d'acceptation

- [x] Aucune duplication ou perte silencieuse de ressources n'est possible.
- [x] Une reprise de sauvegarde conserve cargaison et phase de mission.
- [x] Les erreurs de capacité ou de stock sont explicites.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-53"></a>

## MVP-029-B — Ajouter les sites d'extraction et la récolte distante

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-53](https://cadylo.app/galactic/issue/ENG-53) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | Non estimée |
| Créée le | 27/07/2026 |
| Mise à jour le | 27/07/2026 |

### Objectif

Réutiliser les flottes, le cargo et les missions de transport pour exploiter des
sites distants après stabilisation de la boucle militaire et multi-colonies.

### Périmètre

- Sites d'extraction configurables découverts par exploration ou analyse.
- Conditions d'accès, rendement, capacité et éventuel épuisement.
- Mission de récolte avec trajet, chargement, retour et livraison.
- Limitation par le cargo, le temps et les capacités de la flotte.
- Réservation d'un site pendant une opération si nécessaire.
- Rapport de récolte et persistance.

### Hors périmètre

- Combat automatique pour le contrôle du site.
- Marché ou commerce.

### Critères d'acceptation

- [x] Une mission ne crée jamais plus de ressources que le site n'en fournit.
- [x] Une reprise de sauvegarde conserve le site, la cargaison et la phase.
- [x] Les valeurs d'équilibrage sont modifiables dans le ruleset.

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-38"></a>

## MVP-030 — Créer le HUD des flottes et missions

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-38](https://cadylo.app/galactic/issue/ENG-38) |
| Statut | Terminée |
| Priorité | P2 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 31/07/2026 |

### Objectif
Permettre de préparer, lancer et suivre les missions sans outil de debug.

### Contenu
- Créer un écran de composition de flotte.
- Créer un assistant de choix de destination et mission.
- Afficher chemin, durée, coût, cargaison et validations.
- Afficher les missions actives avec ETA et état.
- Afficher les rapports terminés.

### Critères d'acceptation
- [x] Le joueur peut lancer les missions du MVP depuis l'interface.
- [x] Les erreurs de portée, route, technologie et capacité sont explicites.
- [x] La sélection d'une mission permet de focaliser son origine ou sa destination.
- [x] La liste reste lisible avec au moins dix missions.

### Dépendances
- MVP-022
- MVP-021
- MVP-029
- MVP-024

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-50"></a>

## MVP-030-B — Ajouter la navigation galactique avancée

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-50](https://cadylo.app/galactic/issue/ENG-50) |
| Statut | Terminée |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 01/08/2026 |

### Objectif
Permettre au joueur de retrouver et comprendre rapidement les objets importants lorsque colonies, flottes et missions se multiplient.

### Contenu
- Ajouter une recherche globale pour systèmes, planètes connues, colonies, flottes et missions.
- Ajouter historique précédent/suivant et fil d'Ariane de navigation.
- Restaurer vue, focus, zoom et sélection lors d'un retour dans l'historique.
- Ajouter des filtres pour connaissances, secteurs, colonies, ressources, flottes et missions.
- Ajouter un budget de labels avec priorités, collisions et stabilisation temporelle.
- Agréger flottes et missions à grande distance puis détailler au zoom local.
- Mettre en évidence routes de mission, origine et destination.
- Garantir que recherche, filtres et agrégations respectent les niveaux de connaissance.

### Critères d'acceptation
- [x] Le joueur peut retrouver par nom tout objet qu'il est autorisé à connaître.
- [x] Retour et suivant restaurent le contexte visuel précédent.
- [x] Le fil d'Ariane permet de revenir de planète à système, secteur puis galaxie.
- [x] Aucun filtre ou résultat de recherche ne révèle un objet inconnu.
- [x] La carte reste lisible avec au moins dix missions et plusieurs colonies.
- [x] Les labels prioritaires restent stables et ne clignotent pas excessivement.
- [x] Une mission sélectionnée focalise clairement son trajet, son origine et sa destination.

### Dépendances
- MVP-030
- MVP-023-C
- MVP-010-B

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-39"></a>

## MVP-031 — Implémenter sauvegarde, chargement et migration V1

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-39](https://cadylo.app/galactic/issue/ENG-39) |
| Statut | À faire |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Permettre au joueur de reprendre une partie et sécuriser l'évolution du format de données.

### Contenu
- Créer `SaveGameV1` avec seed, version, tick, découvertes, colonies, stocks, bâtiments, recherches, flottes et missions.
- Ajouter sauvegarde manuelle, chargement et autosauvegardes tournantes.
- Recréer les entités visuelles depuis la sauvegarde.
- Valider la compatibilité avec la version de génération.
- Créer le squelette des migrations futures.

### Critères d'acceptation
- [ ] Une partie peut être sauvegardée puis rechargée sans perte fonctionnelle.
- [ ] Les identifiants stables sont préservés.
- [ ] Les missions et files reprennent au bon état.
- [ ] Une sauvegarde incompatible produit une erreur explicite sans crash.

### Dépendances
- MVP-004
- MVP-005
- MVP-028
- MVP-019

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-40"></a>

## MVP-032 — Ajouter onboarding et objectifs contextuels

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-40](https://cadylo.app/galactic/issue/ENG-40) |
| Statut | À faire |
| Priorité | P2 |
| Estimation | 5 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Guider le joueur à travers la boucle MVP sans tutoriel lourd ni documentation externe.

### Contenu
- Créer une suite d'objectifs : produire, améliorer, rechercher, construire une sonde, sonder, récolter et coloniser.
- Afficher une aide contextuelle non bloquante.
- Mettre en évidence l'écran ou l'action pertinente.
- Permettre de masquer ou réafficher les conseils.
- Journaliser les étapes franchies.

### Critères d'acceptation
- [ ] Un nouveau joueur peut atteindre la première sonde sans aide orale.
- [ ] Les objectifs se valident depuis l'état réel de simulation.
- [ ] Les conseils ne bloquent pas la navigation.
- [ ] La progression du tutoriel est sauvegardée.

### Dépendances
- MVP-015
- MVP-030
- MVP-028

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-41"></a>

## MVP-033 — Définir et implémenter la condition de réussite du MVP

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-41](https://cadylo.app/galactic/issue/ENG-41) |
| Statut | À faire |
| Priorité | P2 |
| Estimation | 3 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Donner une conclusion mesurable à la version de validation sans implémenter la victoire finale du jeu complet.

### Contenu
- Définir l'objectif cible : trois colonies, huit systèmes sondés et une technologie finale atteinte.
- Afficher la progression dans un panneau dédié.
- Déclencher un écran de réussite et un résumé de partie.
- Permettre de continuer après la réussite.
- Collecter quelques métriques de durée et progression.

### Critères d'acceptation
- [ ] La réussite se déclenche uniquement lorsque tous les critères sont atteints.
- [ ] Le résumé présente temps de jeu, colonies, systèmes et technologies.
- [ ] La partie reste jouable après l'écran de réussite.
- [ ] Le seuil peut être configuré.

### Dépendances
- MVP-023
- MVP-016
- MVP-028

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-42"></a>

## MVP-034 — Ajouter les presets graphiques et le mode GPU intégré

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-42](https://cadylo.app/galactic/issue/ENG-42) |
| Statut | À faire |
| Priorité | P2 |
| Estimation | 5 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Conserver l'identité visuelle du POC tout en permettant une exécution acceptable sans carte graphique dédiée.

### Contenu
- Créer Low, Medium et High.
- Rendre configurables bloom, HDR, nébuleuses, ombres, particules, labels et densité d'astéroïdes.
- Réduire la résolution interne ou le coût des effets en Low si l'architecture le permet.
- Partager meshes et matériaux et appliquer le LOD prévu.
- Sauvegarder le preset choisi.

### Critères d'acceptation
- [ ] Le mode Low désactive les effets les plus coûteux sans casser la lisibilité.
- [ ] Le jeu reste fonctionnel sur le poste GPU intégré de référence.
- [ ] Changer de preset ne nécessite pas de redémarrer lorsque possible.
- [ ] Le preset actif est visible dans les options.

### Dépendances
- MVP-007

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-43"></a>

## MVP-035 — Intégrer diagnostics et benchmark reproductible

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-43](https://cadylo.app/galactic/issue/ENG-43) |
| Statut | À faire |
| Priorité | P2 |
| Estimation | 5 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Mesurer objectivement les performances CPU, GPU et simulation avant chaque optimisation.

### Contenu
- Ajouter FPS, frame time, entités, meshes, matériaux et diagnostics de rendu disponibles.
- Créer une scène ou séquence benchmark avec la seed MVP.
- Produire moyenne, minimum et percentile 95 du frame time.
- Tester 720p, 1080p et les trois presets.
- Documenter le protocole de comparaison.

### Critères d'acceptation
- [ ] Deux exécutions du benchmark utilisent la même caméra et les mêmes données.
- [ ] Les résultats peuvent être exportés dans un fichier lisible.
- [ ] Le coût des effets majeurs peut être isolé.
- [ ] Une régression de performance est détectable avant release.

### Dépendances
- MVP-034

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-44"></a>

## MVP-036 — Couvrir déterminisme et règles métier par des tests

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-44](https://cadylo.app/galactic/issue/ENG-44) |
| Statut | À faire |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Sécuriser les règles centrales afin que les futurs ajouts d'IA et de contenu ne cassent pas le MVP.

### Contenu
- Tester génération seedée et IDs.
- Tester économie, coûts, production, énergie et stockage.
- Tester constructions, recherches et crafts.
- Tester graphe, routes, missions, récolte et colonisation.
- Tester niveaux de connaissance et droits de faction.

### Critères d'acceptation
- [ ] Les tests sont indépendants du rendu.
- [ ] Aucun test ne dépend du framerate réel.
- [ ] Les cas d'échec critiques sont couverts.
- [ ] `cargo test` passe de façon reproductible.

### Dépendances
- MVP-003 à MVP-031 selon les modules concernés

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-45"></a>

## MVP-037 — Ajouter un smoke test de la boucle complète

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-45](https://cadylo.app/galactic/issue/ENG-45) |
| Statut | À faire |
| Priorité | P1 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Vérifier qu'une partie peut parcourir la totalité de la vertical slice sans blocage.

### Contenu
- Démarrer une partie sur la planète mère.
- Améliorer la production et rechercher la sonde.
- Construire et envoyer une sonde.
- Récolter des ressources distantes.
- Construire un vaisseau-colonie et créer une deuxième colonie.
- Sauvegarder, charger et poursuivre.

### Critères d'acceptation
- [ ] Le scénario automatisé ou semi-automatisé atteint la deuxième colonie.
- [ ] Aucun stock, vaisseau ou mission n'est dupliqué.
- [ ] Le chargement au milieu du scénario conserve l'état exact.
- [ ] Le test est documenté et exécutable avant chaque release.

### Dépendances
- MVP-031
- MVP-036

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---

<a id="eng-46"></a>

## MVP-038 — Équilibrer, polir et packager le MVP de playtest

| Métadonnée | Valeur |
|---|---|
| Issue | [ENG-46](https://cadylo.app/galactic/issue/ENG-46) |
| Statut | À faire |
| Priorité | P2 |
| Estimation | 8 points |
| Créée le | 23/07/2026 |
| Mise à jour le | 23/07/2026 |

### Objectif
Transformer la somme des fonctionnalités en une expérience courte, compréhensible et distribuable.

### Contenu
- Équilibrer la boucle pour viser une première réussite en 60 à 90 minutes.
- Réduire les temps morts et corriger les blocages économiques.
- Polir notifications, erreurs, navigation et transitions.
- Vérifier interface en 720p et 1080p.
- Créer builds de playtest, README et checklist de retours.

### Critères d'acceptation
- [ ] Un playtesteur externe atteint la deuxième colonie sans commande debug.
- [ ] La condition de réussite est atteignable dans la durée cible.
- [ ] Les commandes qualité passent sans warning bloquant.
- [ ] Un package exécutable et un guide de test sont disponibles.

### Dépendances
- MVP-032 à MVP-037

[↑ Retour au sommaire](#sommaire-de-la-roadmap)

---
