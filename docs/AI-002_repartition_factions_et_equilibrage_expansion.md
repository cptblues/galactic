# AI-002 — Répartition des factions et équilibrage de l'expansion

> **Type :** document de conception / support d'implémentation IA  
> **Projet :** Galactic  
> **Statut :** réflexion de conception  
> **Objectif :** répartir correctement les factions dans la galaxie, limiter les snowballs géographiques et garantir que chaque faction rencontre naturellement de la friction pendant son expansion.

---

## 1. Problème à résoudre

L'équilibrage ne doit pas reposer uniquement sur :

> « combien de systèmes possède chaque faction au départ ? »

Le vrai équilibre vient de la combinaison de quatre facteurs :

```text
position initiale
+ potentiel d'expansion
+ vitesse d'expansion
+ friction rencontrée
```

Exemple de problème à éviter :

```text
Foyer Sylve
    ↓
10 systèmes favorables et vides
    ↓
aucune faction proche
    ↓
expansion sans opposition
    ↓
zone Sylve gigantesque
```

Le but n'est pas d'empêcher une faction de devenir dominante. Le but est d'éviter qu'elle devienne dominante uniquement à cause d'une géographie initiale trop favorable.

---

## 2. Organiser la galaxie en types de zones

La galaxie peut être divisée conceptuellement en quatre types de zones, invisibles pour le joueur :

```text
CŒURS DE FACTION
      ↓
ZONES TAMPONS
      ↓
FRONTIÈRES
      ↓
ESPACE SAUVAGE
```

### 2.1 Cœur de faction

Zone relativement sûre autour d'un centre initial. Elle permet de développer l'économie, reconstruire, produire et éviter une destruction immédiate en début de partie.

### 2.2 Zone tampon

Quelques systèmes non contrôlés entre les puissances. Elle permet les premières expansions, la compétition économique et la découverte progressive des autres factions.

### 2.3 Frontière

Zone où plusieurs sphères d'expansion peuvent se rencontrer. C'est là que doivent apparaître naturellement tensions, concurrence minière, propagation Sylve, guerres et diplomatie.

### 2.4 Espace sauvage

Systèmes peu développés, difficiles ou moins attractifs. Ils gardent de la place libre et évitent un découpage immédiat de toute la galaxie.

---

## 3. Ne pas remplir toute la galaxie au départ

Pour une galaxie d'environ 64 systèmes, une base de travail possible :

| État initial | Ordre de grandeur |
|---|---:|
| Consortium | 1 système |
| Ligue des Confins | 4 à 6 systèmes de présence |
| Sylves | 6 à 10 systèmes de présence |
| Sauvage / neutre | environ 45 à 50 systèmes |

Ces valeurs servent de point de départ, pas de règle finale.

**Présence ne signifie pas nécessairement contrôle total.** Un système Sylve peut ne contenir qu'une planète contaminée ou une Floraison. Un système de la Ligue peut contenir une colonie sans contrôle complet de toutes les planètes.

---

## 4. Répartition recommandée des Sylves

### 4.1 Plusieurs foyers séparés

Éviter un seul gros empire initial. Préférer 3 à 5 foyers indépendants, chacun limité à 2 ou 3 systèmes au départ.

```text
       S S
      S S

                 S
               S S S

       S

                          S S
```

Recommandation initiale :

- 3 à 5 foyers ;
- 2 à 3 systèmes par foyer au départ ;
- un foyer proche du joueur ;
- au moins un foyer ancien, éloigné et plus dangereux.

Cela crée plusieurs fronts indépendants et évite qu'une seule victoire ou défaite décide du sort de toute la faction.

---

## 5. Limiter les zones d'expansion gratuites

Chaque foyer Sylve majeur doit rencontrer rapidement au moins une forme de friction. Celle-ci peut venir de :

- présence humaine ;
- systèmes peu compatibles ;
- planètes stériles ;
- géantes gazeuses ;
- longues distances ;
- manque de biomasse ;
- autre foyer Sylve ;
- coût de propagation croissant.

Règle possible de génération :

> Un foyer Sylve majeur doit rencontrer une friction importante dans un rayon de 2 à 4 sauts.

---

## 6. Mesure essentielle : `FreeExpansionDepth`

Créer une métrique spécifique :

```text
FreeExpansionDepth
```

Elle représente le nombre de couches de systèmes qu'une faction peut potentiellement conquérir sans rencontrer de vraie contrainte.

Exemple :

```text
Sylve A : profondeur gratuite 2 → acceptable
Sylve B : profondeur gratuite 3 → acceptable
Sylve C : profondeur gratuite 9 → carte rejetée
```

Cette métrique doit faire partie de la validation de la seed.

---

## 7. Coût croissant de propagation Sylve

La protection principale contre un snowball Sylve ne doit pas être une limite fixe.

À éviter :

```text
Les Sylves ne peuvent jamais dépasser 15 systèmes.
```

Préférer un coût qui augmente avec la taille et la distance :

```text
coût de propagation
=
coût de base
× facteur de taille du foyer
× facteur de distance
× facteur régional
```

Exemple indicatif :

```text
1er monde supplémentaire   coût 100
2e                         coût 120
5e                         coût 220
10e                        coût 450
```

Effet recherché :

```text
petit foyer → expansion rapide
foyer moyen → expansion normale
foyer très vaste → expansion lente
```

---

## 8. Deux usages concurrents de la Croissance

La Croissance Sylve ne doit pas servir uniquement à s'étendre.

### Expansion

- contaminer une nouvelle planète ;
- étendre le réseau ;
- créer un foyer secondaire.

### Consolidation

- créer des Épines ;
- créer des Carapaces ;
- produire une Floraison ;
- renforcer une Racine ;
- faire émerger un Ancien.

Cela crée deux formes de menace :

```text
GRANDE ZONE DIFFUSE
beaucoup de territoire
mais défense moyenne
```

et :

```text
PETIT CŒUR ANCIEN
territoire limité
mais très dangereux
```

---

## 9. Limite de portée biologique

Les Sylves doivent progresser par continuité territoriale :

```text
S → S → S → nouvelle cible
```

À éviter : contamination aléatoire très loin du foyer.

La propagation par front améliore la lisibilité, crée des zones de confinement et donne des frontières naturelles.

---

## 10. Soft cap territorial régional

Le coût d'expansion doit augmenter lorsqu'une faction domine déjà une région.

| Contrôle régional | Modificateur expansion |
|---|---:|
| 0–25 % | ×1.0 |
| 25–50 % | ×1.2 |
| 50–70 % | ×1.6 |
| 70–85 % | ×2.3 |
| >85 % | très coûteux |

Pour les Sylves, cela peut représenter l'entretien d'un réseau biologique de plus en plus vaste. Pour les humains, le coût peut venir de la logistique, des frontières, du transport et de la défense.

---

## 11. Expansion vers une nouvelle région

Le soft cap local ne doit pas empêcher toute progression.

```text
Secteur Tau
Contrôle Sylve : 80 %
Expansion locale : très coûteuse
```

mais :

```text
Contaminer le premier système du secteur voisin : coût raisonnable
```

Effet recherché : branches, fronts irréguliers et territoires moins compacts.

---

## 12. Victoire militaire et consolidation

Une victoire ne doit pas forcément accélérer immédiatement l'expansion.

```text
Sylves détruisent une flotte de la Ligue
        ↓
pertes Sylves
        ↓
Croissance consommée
        ↓
renforcement nécessaire
        ↓
phase de consolidation
```

Règle utile :

> **Une grosse guerre doit ralentir temporairement le vainqueur.**

Ce principe vaut aussi pour la Ligue et le Consortium : reconstruire les flottes, réparer, réapprovisionner et sécuriser les nouvelles positions.

---

## 13. Tous les systèmes ne doivent pas se valoir

Chaque faction doit avoir des préférences territoriales.

Exemple Sylve indicatif :

```text
Monde tempéré       100
Monde océanique      90
Monde aride          50
Monde glacé          35
Monde volcanique     20
Géante gazeuse        0
```

La Ligue peut avoir une grille différente.

Effet recherché : territoires discontinus, systèmes ignorés, routes stratégiques et fronts non uniformes.

---

## 14. Répartition de la Ligue des Confins

La Ligue devrait commencer plus étendue que le Consortium mais moins centralisée.

Recommandation :

- 4 à 6 systèmes initiaux ;
- répartis en 2 ou 3 clusters humains ;
- colonies moins développées individuellement ;
- plusieurs frontières exposées ;
- quelques colonies proches de zones Sylves.

```text
Cluster Confins Ouest

    L
   L L

Cluster Confins Nord

      L
     L L
```

Cela renforce son identité de réseau de colonies périphériques humaines politiquement réunies mais géographiquement dispersées.

---

## 15. Bulle d'apprentissage autour du Consortium

Le départ joueur doit être volontairement contrôlé.

### À 1 saut

Très faible danger : exploration, missions, ressources et économie.

### À 2 sauts

Premières complications : petite présence Sylve, présence neutre, ressource intéressante ou concurrence.

### À 3–4 sauts

Début de la vraie frontière : Ligue, foyers Sylves, mondes rares et risques militaires.

La géographie devient ainsi un outil de progression.

---

## 16. Ne pas équilibrer uniquement le nombre de systèmes

Deux factions n'ont pas besoin du même nombre de planètes.

Il faut plutôt calculer un potentiel stratégique :

```text
StrategicPotentialScore
```

Composantes possibles :

```text
économie
+ ressources accessibles
+ qualité des planètes
+ sécurité
+ technologie
+ flotte
+ connectivité
+ potentiel d'expansion
```

Une Ligue avec plus de colonies peut rester équilibrée si celles-ci sont dispersées, moins efficaces et plus coûteuses à défendre.

---

## 17. Budget régional

Découper conceptuellement la galaxie en secteurs de 6 à 10 systèmes.

Chaque secteur possède :

- factions présentes ;
- potentiel économique ;
- pression militaire ;
- contrôle territorial ;
- potentiel d'expansion ;
- danger Sylve.

Exemple :

```text
SECTEUR TAU

8 systèmes
Sylves : 4
Ligue  : 1
Sauvage: 3

Pression Sylve : élevée
Potentiel expansion locale Sylve : faible
```

L'équilibrage régional est préférable à une limite globale uniforme.

---

## 18. Génération en plusieurs passes

### Passe 1 — Géographie

Créer systèmes, connexions, types de planètes et ressources. Aucune faction.

### Passe 2 — Placement des seeds de factions

Créer des centres initiaux :

```text
Consortium
Ligue A
Ligue B
Sylve A
Sylve B
Sylve C
```

Contraintes : distance minimale, diversité géographique et absence de concentration excessive.

### Passe 3 — Croissance initiale

Étendre légèrement chaque seed selon le type de faction, la qualité des planètes, la distance et le potentiel stratégique.

### Passe 4 — Validation

La seed n'est acceptée que si les critères d'équilibrage passent. Sinon, la carte est régénérée avec une autre seed.

---

## 19. Critères de validation d'une carte

### Consortium

- 2 à 4 destinations intéressantes à proximité ;
- aucun danger majeur immédiatement adjacent ;
- première présence hostile accessible relativement tôt ;
- vraie frontière à quelques sauts.

### Ligue

- ne pas être totalement enfermée ;
- plusieurs directions d'expansion ;
- au moins une zone de friction potentielle ;
- potentiel économique initial non excessif.

### Sylves

- plusieurs foyers séparés ;
- aucune profondeur d'expansion gratuite excessive ;
- friction à courte ou moyenne distance ;
- au moins un foyer susceptible d'interagir avec une faction humaine.

### Galaxie globale

- majorité de systèmes sauvages ;
- au moins deux régions réellement contestables ;
- aucune faction avec accès immédiat à une part disproportionnée du potentiel économique ;
- plusieurs chemins d'expansion viables pour le joueur.

---

## 20. Métriques de validation recommandées

```text
FactionExpansionScore
FactionSafetyScore
FactionResourceScore
FactionContactScore
FactionConnectivityScore
FreeExpansionDepth
RegionalDominanceScore
StrategicPotentialScore
```

Ces métriques doivent permettre d'analyser une seed, rejeter les cartes problématiques et automatiser les tests de génération.

---

## 21. Exemple de validation automatique

```text
SEED 184293

Consortium
SafetyScore           78  OK
ExpansionScore        64  OK
ResourceScore         72  OK

Ligue
ExpansionScore        81  OK
ContactScore          55  OK

Sylve A
FreeExpansionDepth     2  OK

Sylve B
FreeExpansionDepth     3  OK

Sylve C
FreeExpansionDepth     8  FAIL

RESULTAT :
SEED REJETÉE
```

---

## 22. Valeurs de départ à tester pour 64 systèmes

### Consortium

- 1 système initial ;
- 2 à 4 bonnes destinations proches ;
- faible danger à 1 saut ;
- premières complications à 2 sauts ;
- frontière réelle à 3–4 sauts.

### Ligue

- 4 à 6 systèmes initiaux ;
- 2 clusters ;
- puissance économique supérieure au joueur au départ ;
- développement moins optimisé ;
- plusieurs frontières.

### Sylves

- 3 à 4 foyers ;
- 2 à 3 systèmes par foyer au maximum ;
- un petit foyer proche du joueur ;
- un foyer ancien plus éloigné ;
- profondeur gratuite maximale cible : environ 3 systèmes.

### Sauvage

- majorité de la galaxie.

---

## 23. Déséquilibres acceptables

Les déséquilibres temporaires sont souhaitables :

```text
Sylves prennent 3 systèmes rapidement
```

```text
Ligue perd une région
```

```text
Une faction contrôle temporairement une grande frontière
```

Ils créent de l'histoire.

Ce qui doit être évité :

```text
Une faction prend 20 systèmes
uniquement parce que sa zone initiale était vide.
```

Règle centrale :

> **Aucune faction ne doit pouvoir croître longtemps sans rencontrer une friction ou payer un coût croissant.**

---

## 24. Interaction avec le futur directeur de pression

Le directeur de pression ne doit pas corriger directement la géographie.

Il peut cependant empêcher qu'une situation dominante devienne immédiatement punitive pour le joueur.

```text
Sylves très puissants dans une région éloignée
→ aucun problème
```

```text
Sylves très puissants
+ deux attaques déjà subies récemment
→ nouvelle offensive majeure temporairement différée
```

La carte peut être asymétrique sans rendre la partie injuste.

---

## 25. Debug visuel recommandé

Ajouter un mode de debug cartographique affichant :

- secteurs ;
- centres de faction ;
- distances entre seeds ;
- potentiel économique ;
- profondeur d'expansion ;
- zones de friction ;
- score régional ;
- routes probables d'expansion.

Exemple :

```text
[Sylve A]
FreeDepth = 2
RegionalPressure = 48

[Ligue Ouest]
Expansion = 72
Safety = 55

[Consortium]
Safety = 82
Expansion = 63
```

---

## 26. Tests recommandés

### Test A — 1000 générations

Mesurer :

- profondeur d'expansion Sylve ;
- sécurité du joueur ;
- potentiel économique initial ;
- distances moyennes entre factions ;
- pourcentage de cartes rejetées.

### Test B — Simulation sans joueur

Simuler 30 à 60 minutes sans commande joueur.

Observer :

- contrôle territorial ;
- croissance Sylve ;
- expansion Ligue ;
- nombre de conflits ;
- nombre de systèmes encore sauvages.

### Test C — Snowball Sylve

Sélectionner volontairement un foyer isolé et vérifier le ralentissement par coût, la consolidation et le soft cap régional.

### Test D — Destruction de la Ligue locale

Faire perdre un cluster de la Ligue proche d'un foyer Sylve et vérifier l'absence de capture instantanée de toute la région.

### Test E — Carte du joueur

Vérifier que le joueur dispose de choix proches, que le premier contact arrive suffisamment tôt et qu'aucune seed ne produit un départ presque impossible.

---

## 27. Ordre d'implémentation conseillé

```text
MAP-AI-001
Métriques de potentiel et secteurs

MAP-AI-002
Placement des seeds de faction

MAP-AI-003
Validation automatique des seeds

MAP-AI-004
Foyers Sylves et FreeExpansionDepth

MAP-AI-005
Coût croissant et soft caps régionaux

MAP-AI-006
Simulation longue durée et équilibrage
```

---

## 28. Résumé final

La solution recommandée repose sur :

```text
placement semi-contrôlé
        +
plusieurs foyers
        +
majorité de systèmes sauvages
        +
coût d'expansion croissant
        +
friction régionale
        +
préférences de planète
        +
phase de consolidation après guerre
        +
validation automatique des seeds
```

La règle de conception à conserver est :

> **Les déséquilibres temporaires créent de l'histoire. Les déséquilibres gratuits créés uniquement par la seed doivent être empêchés.**

Pour Galactic, le meilleur objectif n'est donc pas :

> « chaque faction doit contrôler le même nombre de systèmes »

mais :

> **« chaque faction doit avoir des opportunités différentes, tout en étant forcée tôt ou tard de rencontrer des contraintes, des coûts ou d'autres puissances. »**
