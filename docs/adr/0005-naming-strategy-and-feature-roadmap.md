# Stratégie de Nommage Strmaid et Feuille de Route Fonctionnelle

Pour résoudre la collision d'antériorité sur Crates.io et clarifier le positionnement architectural face aux outils existants, nous actons le renommage du projet de `termaid` vers `strmaid` (**Str**eam / **St**reaming + Mer**maid**) et l'adoption d'un backlog enrichi issu du benchmark concurrentiel.

## Considered Options

- **Conserver le nom `termaid` :** Rejeté car la crate `termaid` est déjà publiée sur Crates.io (v0.1.0 par eboody) et sur PyPI (Fabio Souto), ce qui empêche toute publication officielle et entretient une confusion avec les outils de tracé Unicode/ASCII.
- **Opter pour `merflow` / `markmaid` :** Moins percutant ou déjà partiellement utilisé dans d'autres langages.
- **Adopter `strmaid` :** Retenu. Nom court, disponible à 100% sur Crates.io et GitHub, évoquant immédiatement la spécialisation première du CLI : le streaming Unix de bout en bout et les flux d'agents IA intégrant des diagrammes Mermaid.

## Consequences

- Renommage du package Cargo en `strmaid` et du binaire autonome en `strmaid`.
- Intégration de 6 fonctionnalités issues du benchmark (`fasouto/termaid`, `meraid`) dans la roadmap officielle :
  1. Protocole de repli textuel `GraphicsProtocol::AsciiBox` pour les environnements sans TrueColor ni Kitty.
  2. Format de sortie structurée `--format json` / NDJSON pour les agents IA.
  3. Thèmes étendus à fort contraste (`amber`, `phosphor`, `neon`, `mono`).
  4. Prise en compte de la largeur des caractères Unicode & CJK.
  5. Mode d'intégration éditeur (`--block-only`) pour Neovim / Helix.
  6. Compaction adaptative face aux terminaux étroits.
