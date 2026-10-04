# Protocole Graphique Terminal et Repli Dégradé

Pour afficher des images matricielles haute fidélité dans l'environnement Debian/Warp, nous implémentons le Kitty Graphics Protocol comme canal d'affichage principal, avec un repli universel déterministe en demi-blocs Unicode (`▀`, `▄`) ANSI TrueColor 24-bit. Warp ne supportant pas Sixel sous Linux, cette stratégie offre un affichage graphique natif sous Warp tout en restant fonctionnel dans n'importe quel émulateur de terminal.

## Considered Options

- **Support Sixel uniquement :** Rejeté car Warp sous Linux ne prend pas en charge Sixel.
- **Kitty Graphics Protocol exclusif :** Rejeté car les terminaux non compatibles afficheraient des artefacts de séquences d'échappement corrompues.
- **Kitty Protocol prioritaire avec repli ANSI Half-Blocks et flag CLI `--graphics` :** Retenu pour concilier fidélité maximale sous Warp et compatibilité universelle.

## Consequences

Le binaire interroge ou détecte les variables d'environnement (`TERM`, `TERM_PROGRAM`, support Kitty), émet le protocole Kitty lorsque disponible, et bascule de manière transparente sur les demi-blocs TrueColor sinon.
