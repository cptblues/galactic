# POC 0.2 Implementation Notes

## Audit

- Base Bevy 0.19 conservee.
- `GalaxyData` reste independant des entites Bevy.
- Les vues `Galaxy` et `System`, la camera orbitale et le mesh picking existants sont reutilises.
- Le POC 0.1 demarre en release et les tests existants passent avant evolution.

## Adaptations

- Systeme par defaut passe a 500 systemes.
- La couche strategique est ajoutee dans `strategic`, sans stocker d'`Entity`.
- Les territoires sont representes par halos locaux autour des systemes controles.
- La selection dense en vue galaxie utilise une projection ecran et un panneau ambigu textuel.
- Les labels sont des entites 3D masquees/affichees selon score, budget et collision approximative.

## Divergences

- L'UI reste volontairement textuelle et pilotee par raccourcis.
- Les boutons demandes par la spec sont representes par commandes clavier et textes d'action dans les panneaux.
- Les flottes sont visuelles et statiques cote donnees ; leur position est interpolee au rendu.

