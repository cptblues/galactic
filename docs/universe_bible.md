# Bible d'univers et nomenclature V3

Cette bible fixe l'identité éditoriale du ruleset `default`. Elle sert de
référence pour tout nouveau nom visible par le joueur.

## Promesse

Le monde d'avant était à bout de souffle : guerres, famines, pollution,
surpopulation et lutte pour les dernières ressources. Les gouvernements
survivants formèrent le **Consortium** autour d'une doctrine simple : une seule
direction, un seul objectif, une seule humanité.

Dans Galactic, le joueur vient d'être promu Amiral. Sa mission est d'assurer la
survie de l'humanité, sécuriser les ressources dont elle dépend et étendre sa
présence aussi loin que nécessaire. L'humour vient du décalage entre une
administration persuadée d'agir pour le bien commun et des opérations
d'exploration, d'exploitation et de résolution orbitale de plus en plus
expéditives.

## Ton et langue

- Tous les textes d'interface et noms communs sont en français.
- Les noms d'éléments restent courts, simples et immédiatement compréhensibles.
- Les descriptions portent l'identité humoristique, militariste et
  bureaucratique de l'univers.
- Le ton évoque une administration galactique persuadée que toute action
  militaire relève du service public.
- Éviter les références directes à des licences existantes.
- Un nom évocateur ne doit jamais masquer la fonction : `Sonde — Œil` reste
  identifiable comme une sonde.

## Repères canoniques

| Élément | Nom |
|---|---|
| Faction joueur | Consortium |
| Faction neutre | Collectifs à Convaincre |
| Puissance hostile dormante | Bureau Vesper de l'Oppression |
| Système natal | Hélianthe |
| Planète mère | Nacre |
| Colonie initiale | Port-Sillage |

### Factions

- **Consortium** : coalition humaine survivante, centralisée et expansionniste.
  Son vocabulaire privilégie survie, stabilité, sécurité, ressources et
  protection.
- **Collectifs à Convaincre** : habitats autonomes que le Consortium décrit
  comme des interlocuteurs mal alignés.
- **Bureau Vesper de l'Oppression** : puissance hostile dormante. Son lexique
  futur privilégiera protocoles fermés, garnisons, batteries et contrôle
  orbital.

## Systèmes et planètes

Les 64 systèmes du preset MVP ont un nom propre stable. Les planètes gardent la
règle `nom du système + lettre orbitale`, sauf `Nacre`.

Les seize premiers systèmes restent :

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

Les anciennes séries `-2`, `-3` et `-4` sont remplacées par :

1. Solédris
2. Noctavéa
3. Thaléryne
4. Brontar
5. Lyséa
6. Oradis
7. Kéméris
8. Phanéon
9. Lunévar
10. Axoria
11. Ilvaris
12. Cendral
13. Dikéa
14. Cantéor
15. Opalys
16. Varkane
17. Aurélys
18. Crépuscor
19. Pélagis
20. Colosséa
21. Myrion
22. Zéphara
23. Sabaël
24. Mnémoris
25. Nysséa
26. Ordalis
27. Vaelune
28. Braséon
29. Noméria
30. Mélodran
31. Iridys
32. Kharéon
33. Héméria
34. Ombrelis
35. Abyssara
36. Gravéon
37. Elarque
38. Oryssia
39. Aksomar
40. Spectéon
41. Artélys
42. Agréon
43. Ylvane
44. Ferrélys
45. Équoria
46. Harméon
47. Perléa
48. Arcandor

## Infrastructures

| Identifiant stable | Nom affiché | Famille |
|---|---|---|
| `metal_mine` | Mine de métal | extraction |
| `crystal_extractor` | Extracteur de cristal | extraction |
| `fuel_refinery` | Raffinerie de carburant | transformation |
| `power_plant` | Centrale énergétique | énergie |
| `warehouse` | Entrepôt | logistique |
| `construction_center` | Centre de construction | industrie |
| `research_lab` | Laboratoire | science |
| `shipyard` | Chantier naval | industrie orbitale |

## Recherches

| Identifiant stable | Nom affiché | Déblocage |
|---|---|---|
| `spatial_detection` | Détection longue portée | détection de systèmes inconnus |
| `propulsion` | Propulsion avancée | transit interstellaire |
| `cargo_capacity` | Soutes agrandies | capacité cargo augmentée |
| `remote_extraction` | Extraction automatisée | récolte distante |
| `planetary_analysis` | Analyse planétaire | rapport planétaire complet |
| `colonization` | Colonisation avancée | fondation de colonies |

## Vaisseaux

| Identifiant stable | Nom affiché | Rôle |
|---|---|---|
| `light_probe` | Sonde — Œil | reconnaissance rapide |
| `cartographer_satellite` | Satellite — Veilleur | analyse planétaire |
| `light_cargo` | Caboteur — Relais | fret à courte portée |
| `meridian_carrier` | Porteur — Navette | fret intermédiaire |
| `atlas_cargo` | Cargo — Chargeur | fret lourd |
| `needle_interceptor` | Intercepteur — Riposte | militaire léger |
| `frigate_bulwark` | Frégate — Garde | combat de première ligne |
| `bastion_cruiser` | Croiseur — Verdict | combat lourd |
| `colony_ship` | Arche coloniale — Essor | fondation de colonie |

## Contrat technique

- Les identifiants RON en `snake_case` sont des clés de sauvegarde et ne sont
  jamais renommés pour une raison éditoriale.
- Les noms et descriptions du ruleset peuvent évoluer avec `content_version`.
- Les noms générés des systèmes et planètes participent au fingerprint de
  l'univers. Ce checkpoint incrémente donc `GENERATION_VERSION`.
- Une nouvelle mécanique conserve un nom fonctionnel dans Rust et reçoit son
  nom d'univers dans les données affichées.
