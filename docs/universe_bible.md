# Bible d'univers et nomenclature V1

Cette bible fixe l'identité éditoriale du ruleset `default`. Elle sert de
référence pour tout nouveau nom visible par le joueur.

## Promesse

Galactic est une science-fiction de frontière sobre et lisible. L'Expédition
Aster s'établit dans les Confins d'Orphée, une région isolée depuis la Rupture
des anciennes routes. Depuis Port-Sillage, le joueur reconstruit une chaîne
industrielle, cartographie des systèmes silencieux et rencontre des puissances
dont les intentions restent incertaines.

Le monde doit évoquer :

- une exploration méthodique plutôt qu'une aventure magique ;
- une technologie industrielle compréhensible ;
- des distances, des délais et une logistique qui comptent ;
- des traces d'un espace humain fragmenté, sans expliquer trop tôt tous ses
  mystères.

## Ton et langue

- Tous les textes d'interface et noms communs sont en français.
- Les noms propres sont courts, prononçables et distincts au premier regard.
- Le vocabulaire maritime est réservé au voyage, au fret et aux colonies.
- Le vocabulaire de lumière sert à la détection, aux sondes et à l'énergie.
- Les termes militaires restent fonctionnels et sobres.
- Éviter les anglicismes, les numéros de modèle gratuits et les superlatifs
  comme `ultime`, `suprême` ou `méga`.
- Un nom évocateur ne doit jamais masquer la fonction : `Sonde Luciole` reste
  identifiable comme une sonde.

## Repères canoniques

| Élément | Nom |
|---|---|
| Région | Confins d'Orphée |
| Faction joueur | Expédition Aster |
| Faction neutre | Communes des Confins |
| Puissance hostile dormante | Directoire Vesper |
| Système natal | Hélianthe |
| Planète mère | Nacre |
| Colonie initiale | Port-Sillage |

### Factions

- **Expédition Aster** : organisation scientifique et industrielle envoyée
  pour rouvrir la frontière. Son lexique privilégie navigation, veille,
  assemblage et implantation.
- **Communes des Confins** : habitats autonomes sans autorité centrale unique.
  Les futurs noms associés doivent évoquer havres, relais, comptoirs et
  communautés.
- **Directoire Vesper** : puissance structurée, distante et potentiellement
  hostile. Son lexique futur privilégiera ordre, protocoles, cohortes et
  désignations froides.

## Systèmes et planètes

Les seize premiers systèmes, qui constituent le preset Test, ont un nom propre
fixe. Les presets MVP et Stress réemploient ensuite cette liste avec un suffixe
de cycle stable :

1. Hélianthe
2. Vespera
3. Néréide
4. Talos
5. Cyrène
6. Ophira
7. Méroé
8. Eidolon
9. Sélène
10. Praxia
11. Ilyr
12. Calder
13. Thémis
14. Orphéon
15. Nacréon
16. Arkan

Règles :

- un système porte un nom propre unique ;
- une planète non baptisée reprend le nom du système suivi d'une lettre
  astronomique minuscule : `Vespera b`, `Vespera c` ;
- une planète habitée ou scénaristiquement importante peut recevoir un nom
  propre court, comme `Nacre` ;
- une colonie reçoit un nom d'implantation distinct de sa planète :
  `Port-Sillage`, `Relais-Cyrène`, `Havre-Néréide` ;
- ne pas employer `Prime`, `Major` ou `Minor` sans nécessité astronomique ou
  politique explicite.

## Infrastructures

| Identifiant stable | Nom affiché | Famille |
|---|---|---|
| `metal_mine` | Fosse sidérurgique | extraction |
| `crystal_extractor` | Extracteur cristallin | extraction |
| `fuel_refinery` | Raffinerie de volatils | transformation |
| `power_plant` | Réacteur hélionique | énergie |
| `warehouse` | Dépôt logistique | logistique |
| `construction_center` | Atelier d'assemblage | industrie |
| `research_lab` | Institut d'analyse | science |
| `shipyard` | Chantier orbital | industrie orbitale |

Un nouveau bâtiment suit la formule `fonction + précision éventuelle`. Sa
description commence par ce qu'il fait, puis précise son rôle dans la boucle de
jeu.

## Recherches

| Identifiant stable | Nom affiché | Déblocage |
|---|---|---|
| `spatial_detection` | Veille sidérale | cartographie des signaux |
| `propulsion` | Propulsion à flux | transit interstellaire |
| `cargo_capacity` | Architecture de soute | soutes modulaires |
| `remote_extraction` | Prospection autonome | extraction hors-colonie |
| `planetary_analysis` | Spectrométrie planétaire | diagnostic des mondes |
| `colonization` | Ingénierie d'implantation | fondation d'avant-postes |

Une technologie décrit une discipline ou une méthode, pas seulement son bonus.
Le libellé de déblocage décrit l'action rendue possible.

## Vaisseaux

| Identifiant stable | Nom affiché | Rôle |
|---|---|---|
| `light_probe` | Sonde Luciole | reconnaissance rapide |
| `light_cargo` | Caboteur Sillage | fret à courte portée |
| `colony_ship` | Arche Pionnière | fondation de colonie |
| `frigate_bulwark` | Frégate Rempart | combat de première ligne |

Formule recommandée :

- sonde : phénomène lumineux ou instrument d'observation ;
- cargo : vocabulaire maritime ou logistique ;
- colonisation : arche, implantation ou départ ;
- militaire : fonction tactique suivie d'un nom de classe ;
- soutien : rôle opérationnel immédiatement lisible.

Les classes futures peuvent être introduites sous la forme `Croiseur Vigie` ou
`Ravitailleur Estuaire`. Un même nom de classe ne doit pas être réutilisé pour
deux rôles.

## Contrat technique

- Les identifiants RON en `snake_case` sont des clés de sauvegarde et ne sont
  jamais renommés pour une raison éditoriale.
- Les noms et descriptions du ruleset peuvent évoluer avec
  `content_version`; ils ne modifient pas son empreinte structurelle.
- Les noms générés des systèmes et planètes participent au fingerprint de
  l'univers. Toute modification exige une nouvelle `GENERATION_VERSION` et une
  nouvelle valeur de référence.
- Une nouvelle mécanique conserve un nom fonctionnel dans Rust et reçoit son
  nom d'univers dans les données affichées.
