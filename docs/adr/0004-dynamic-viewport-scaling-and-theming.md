# Dimensionnement Dynamique au Viewport et Thématisation Adaptative

Pour garantir que les diagrammes vectoriels s'insèrent harmonieusement dans les sorties CLI sans débordement horizontal ni troncature, la rasterisation SVG adapte dynamiquement la largeur cible à la géométrie réelle du terminal (`crossterm::terminal::size()`) tout en appliquant un thème sombre avec fond transparent par défaut.

## Considered Options

- **Dimensions SVG statiques (taille fixe) :** Rejeté car les petits terminaux tronquent les graphismes et les grands écrans affichent des images sous-dimensionnées.
- **Auto-détection de luminosité via OSC 11 :** Rejeté car les séquences d'interrogation du terminal peuvent introduire des latences d'attente ou échouer sur des flux `stdin` combinés.
- **Thème sombre par défaut avec fond transparent et scaling au viewport :** Retenu comme configuration optimale pour Warp et Debian, ajustable via les flags `--theme` et `--width`.

## Consequences

Le module de rendu calcule le ratio d'aspect avant rasterisation et borne la largeur de l'image à 90% des colonnes disponibles.
