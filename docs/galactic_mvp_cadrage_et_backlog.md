# Galactic — Cadrage du MVP et plan de réalisation

**Version :** 1.0  
**Seed de référence :** fixe pendant le MVP  
**Moteur :** Rust + Bevy  
**Backlog associé :** 38 issues numérotées `MVP-001` à `MVP-038`

## 1. Résumé exécutif

Le MVP doit valider une boucle solo complète : développer une planète mère, produire des ressources, débloquer une sonde, découvrir des systèmes reliés par des routes, exploiter des ressources distantes, débloquer la colonisation et créer de nouvelles colonies.

Le combat, la diplomatie active et les factions contrôlées par l'IA ne font pas partie de la boucle jouable initiale. Le modèle métier doit néanmoins être conçu avec des propriétaires, des factions et des commandes génériques afin d'éviter une refonte future.

## 2. Terminologie retenue

- **Univers** : graphe global de systèmes reliés par des routes.
- **Système stellaire** : une étoile, des planètes, des lunes et éventuellement des ceintures ou stations.
- **Planète mère** : première planète colonisée et habitable du joueur.
- **Colonie** : implantation permanente qui possède stocks, production, bâtiments et files.
- **Vue Univers** : carte 3D globale des systèmes connus ou détectés.
- **Vue Système** : visualisation locale de l’étoile et des corps célestes.

## 3. Boucle centrale

1. Observer la planète mère et ses productions.
2. Améliorer les bâtiments de ressources et d’énergie.
3. Construire Laboratoire et Chantier spatial.
4. Rechercher les technologies nécessaires.
5. Construire une sonde et sonder un système voisin.
6. Comparer les corps révélés et leurs opportunités.
7. Envoyer un cargo vers un site d’extraction distant.
8. Accumuler les ressources et débloquer la colonisation.
9. Construire un vaisseau-colonie et fonder une implantation.
10. Utiliser plusieurs colonies pour progresser plus loin dans le graphe.

## 4. Parcours de validation du MVP

Une partie de validation se termine lorsque le joueur possède trois colonies, a sondé huit systèmes et atteint une technologie finale définie. Cette condition constitue une conclusion de playtest, pas la victoire définitive du jeu complet.

## 5. Périmètre inclus

- Seed fixe et univers de 12 à 20 systèmes.
- Une planète mère habitable et équilibrée.
- Vue Univers et vue Système 3D issues du POC.
- Brouillard d’information et exploration par sondes.
- Métal, cristal, carburant et bilan énergétique.
- Huit bâtiments principaux avec niveaux et prérequis.
- Six technologies de progression.
- Sondes, cargos et vaisseaux-colonies.
- Missions de reconnaissance, transport, récolte et colonisation.
- Deux ou trois colonies gérables.
- Temps réel avec pause et vitesses x1, x2 et x4.
- Sauvegarde et chargement.
- Factions et propriété dans le core, sans IA active.
- Presets graphiques Low, Medium et High.

## 6. Hors périmètre

- Combat et équilibrage militaire.
- Diplomatie jouable, négociations et rachat de factions.
- Boucle d’actions d’IA concurrente.
- Marché dynamique.
- Population individuelle ou city-builder détaillé.
- Progression hors ligne fondée sur l’heure réelle.
- Plusieurs galaxies astronomiques contenant chacune des centaines de systèmes.
- Multijoueur.

## 7. Univers fixe, génération et sauvegarde

La seed reconstruit la définition initiale du monde : systèmes, positions, étoiles, planètes, lunes, ressources potentielles et routes. La sauvegarde conserve les conséquences de la partie : découvertes, bâtiments, stocks, technologies, flottes, missions et colonies.

Les identifiants persistants ne doivent jamais utiliser les `Entity` Bevy. Des newtypes stables sont requis pour tous les objets métier.

## 8. Niveaux de connaissance

| Niveau | Informations disponibles |
|---|---|
| Inconnu | L’objet n’est pas visible. |
| Détecté | Position et signal approximatif, sans détails. |
| Sondé | Étoile, nombre de planètes, lunes principales, indices de ressources et habitabilité. |
| Analysé | Valeurs précises, coûts, bonus, malus et colonisabilité. |
| Colonisé | Informations locales complètes et accès à la construction. |

Le rendu peut montrer une planète sans révéler ses propriétés. Le HUD doit distinguer clairement inconnu, estimation et valeur précise.

## 9. Économie

### Ressources

- Métal : constructions lourdes et structures.
- Cristal : électronique, recherche et technologies.
- Carburant : propulsion et missions.
- Énergie : capacité locale produite et consommée, non stockée par défaut.

### Bâtiments du MVP

1. Mine de métal
2. Extracteur de cristal
3. Raffinerie de carburant
4. Centrale énergétique
5. Entrepôt
6. Centre de construction
7. Laboratoire de recherche
8. Chantier spatial

Les coûts et durées augmentent avec le niveau. Les formules et définitions doivent être pilotées par les données afin de faciliter l’équilibrage.

## 10. Recherche et crafts

Technologies initiales : Détection spatiale, Propulsion, Capacité cargo, Extraction distante, Analyse planétaire et Colonisation.

Le chantier spatial utilise une file générique de craft. Les catégories Défense et Militaire peuvent exister dans le modèle ou l’interface, mais restent inactives tant qu’aucun ennemi n’est simulé.

## 11. Flottes et missions

Les trois unités actives du MVP sont la Sonde légère, le Cargo léger et le Vaisseau-colonie. Une flotte possède un propriétaire, une localisation, une composition, une cargaison et éventuellement une mission.

La machine d’état commune est : Préparation → Transit aller → Sur place → Transit retour → Terminée, avec un état Échec contrôlé.

Missions :

- `Probe` : révèle un système.
- `Transport` : déplace des ressources entre colonies.
- `Harvest` : récupère les ressources finies d’un site distant.
- `Colonize` : fonde une nouvelle colonie.
- `Return` : retour technique lorsque nécessaire.

## 12. Colonisation

Une planète doit être analysée, suffisamment habitable, accessible par une route, compatible avec la technologie du joueur et non déjà colonisée. La mission consomme un investissement élevé : vaisseau-colonie et cargaison de fondation.

Une nouvelle colonie démarre avec des stocks limités, une faible production et un socle minimal de développement. Les colonies produisent localement ; les stocks ne sont pas magiquement globaux.

## 13. Routes et découverte progressive

La vue Univers ne montre que les systèmes connus ou détectés. Lorsqu’un système est sondé, ses routes directes deviennent visibles et les systèmes situés au bout passent au niveau Détecté. Il faut ensuite les sonder à leur tour.

Cette règle soutient le gameplay et réduit le nombre d’entités 3D instanciées, mais elle ne remplace pas les optimisations de rendu : LOD, billboards, matériaux partagés, limitation des labels et presets graphiques restent nécessaires.

## 14. Factions préparées, mais inactives

Tout objet possédable utilise un `FactionId`. Les actions importantes passent par une commande contenant la faction émettrice. Le MVP peut contenir des définitions de factions futures et des relations Unknown, Neutral, Friendly ou Hostile, mais aucune boucle IA n’est exécutée.

Ce choix permettra plus tard à une IA de produire les mêmes commandes que le joueur sans réécrire la simulation.

## 15. Temps de jeu

Le jeu fonctionne en temps réel avec pause et vitesses x1, x2 et x4. Les temps de construction, recherche et déplacement doivent être suffisamment courts pour une session de validation de 60 à 90 minutes. La progression hors ligne est exclue.

## 16. Interface principale

- Vue Univers : systèmes, routes, frontière de découverte et missions.
- Vue Système : étoile, planètes, lunes et information partielle.
- Gestion planétaire : ressources, énergie, stockage et bâtiments.
- Recherche : technologies et prérequis.
- Chantier spatial : crafts et files.
- Flottes : composition, trajet, cargaison et missions.
- Empire : aperçu agrégé et sélection de colonie.
- Objectifs : progression du parcours MVP.

## 17. Performance

La cible GPU intégré nécessite un preset Low : bloom et transparence limités, aucune ombre coûteuse dans la vue Univers, billboards ou meshes simples, peu de labels, matériaux partagés et chargement visuel limité au voisinage connu.

Les décisions de performance doivent être guidées par un benchmark reproductible et des diagnostics CPU/GPU, pas par des optimisations à l’aveugle.

## 18. Principaux risques de game design

- **Stratégie automatique** : améliorer toutes les mines sans choix intéressant. Mitigation : planètes spécialisées et compromis entre rendement, habitabilité, position et coût.
- **Temps morts** : attentes trop longues. Mitigation : vitesses, files courtes et objectifs enchaînés.
- **Microgestion multi-colonies** : trop de clics répétitifs. Mitigation : aperçu global, sélection rapide et limite de colonies.
- **Exploration cosmétique** : sondes sans décision. Mitigation : informations graduelles et opportunités réellement différentes.
- **Carte illisible** : trop de systèmes, routes et labels. Mitigation : visibilité sémantique et voisinage découvert.
- **Couplage au joueur humain** : refonte future pour les IA. Mitigation : factions, ownership et commandes génériques dès maintenant.

## 19. Définition de réussite

Le MVP est validé lorsqu’un joueur extérieur peut : comprendre sa planète, améliorer sa production, rechercher et construire une sonde, découvrir un nouveau système, exploiter un site distant, construire un vaisseau-colonie, créer une deuxième colonie, gérer plusieurs colonies, sauvegarder et reprendre sa partie, puis atteindre la condition de réussite sans commande debug.

## 20. Découpage du backlog

### Fondations

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 001 | MVP-001 — Auditer et figer le POC comme baseline du MVP | P1 | 3 |
| 002 | MVP-002 — Séparer simulation, domaine, rendu et interface | P1 | 8 |
| 003 | MVP-003 — Fixer la seed MVP et introduire des identifiants stables | P1 | 5 |
| 004 | MVP-004 — Séparer l'univers généré de l'état mutable de partie | P1 | 8 |
| 005 | MVP-005 — Implémenter le temps stratégique, la pause et les vitesses | P1 | 5 |

### Univers

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 006 | MVP-006 — Générer le graphe d'univers et les routes entre systèmes | P1 | 8 |
| 007 | MVP-007 — Adapter la vue Univers au voisinage découvert | P1 | 8 |
| 008 | MVP-008 — Définir le système de départ et la planète mère | P1 | 5 |

### Exploration

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 009 | MVP-009 — Implémenter les niveaux de connaissance des objets | P1 | 8 |
| 010 | MVP-010 — Adapter les inspecteurs aux informations partielles | P1 | 5 |

### Économie

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 011 | MVP-011 — Implémenter le registre de ressources et l'énergie | P1 | 8 |
| 012 | MVP-012 — Ajouter production planétaire et capacités de stockage | P1 | 8 |
| 013 | MVP-013 — Définir le catalogue des bâtiments du MVP | P1 | 5 |
| 014 | MVP-014 — Implémenter la file de construction et les améliorations | P1 | 8 |
| 015 | MVP-015 — Construire l'écran de gestion planétaire | P1 | 8 |

### Progression

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 016 | MVP-016 — Implémenter la recherche et l'arbre technologique minimal | P1 | 8 |
| 017 | MVP-017 — Ajouter une file générique de craft au chantier spatial | P1 | 8 |

### Factions

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 018 | MVP-018 — Généraliser la propriété avec les factions | P1 | 5 |
| 019 | MVP-019 — Introduire les commandes génériques et relations dormantes | P2 | 8 |

### Flottes

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 020 | MVP-020 — Définir les flottes, vaisseaux et capacités | P1 | 8 |
| 021 | MVP-021 — Implémenter le moteur de trajet et la machine d'état des missions | P1 | 8 |

### Reconnaissance

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 022 | MVP-022 — Ajouter la sonde et la mission de reconnaissance | P1 | 8 |
| 023 | MVP-023 — Propager la découverte aux systèmes suivants | P1 | 5 |

### Exploitation

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 024 | MVP-024 — Ajouter les sites d'extraction et missions de récolte distante | P1 | 8 |

### Colonisation

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 025 | MVP-025 — Définir analyse planétaire et règles de colonisabilité | P1 | 5 |
| 026 | MVP-026 — Implémenter le vaisseau-colonie et la mission de colonisation | P1 | 8 |
| 027 | MVP-027 — Initialiser une nouvelle colonie jouable | P1 | 8 |
| 028 | MVP-028 — Ajouter la gestion multi-colonies | P1 | 8 |

### Logistique

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 029 | MVP-029 — Ajouter les missions de transport entre colonies | P1 | 5 |
| 030 | MVP-030 — Créer le HUD des flottes et missions | P2 | 8 |

### Persistance

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 031 | MVP-031 — Implémenter sauvegarde, chargement et migration V1 | P1 | 8 |

### Expérience

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 032 | MVP-032 — Ajouter onboarding et objectifs contextuels | P2 | 5 |
| 033 | MVP-033 — Définir et implémenter la condition de réussite du MVP | P2 | 3 |

### Performance

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 034 | MVP-034 — Ajouter les presets graphiques et le mode GPU intégré | P2 | 5 |
| 035 | MVP-035 — Intégrer diagnostics et benchmark reproductible | P2 | 5 |

### Qualité

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 036 | MVP-036 — Couvrir déterminisme et règles métier par des tests | P1 | 8 |
| 037 | MVP-037 — Ajouter un smoke test de la boucle complète | P1 | 8 |

### Release

| N° | Issue | Priorité | Estimation |
|---:|---|---:|---:|
| 038 | MVP-038 — Équilibrer, polir et packager le MVP de playtest | P2 | 8 |

## 21. Ordre d’exécution recommandé

Les issues sont numérotées dans l’ordre de réalisation attendu. Une issue peut être préparée en parallèle uniquement lorsque ses dépendances explicites sont terminées ou suffisamment stabilisées.

Le jalon le plus important est la première boucle jouable complète : production → recherche → sonde → découverte → récolte → colonisation. Les éléments de polish ne doivent pas retarder la validation de cette vertical slice.

## 22. Commandes qualité avant chaque jalon

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run --release
```