# AGENTS.md — Strmaid

Instructions opérationnelles pour les agents qui modifient ce dépôt. La vérité vivante du projet est dans le code, `Cargo.toml`, `README.md`, `CONTEXT.md`, `CONTRIBUTING.md` et les ADR de `docs/adr/`.

## Mission

Strmaid est un CLI Rust qui lit du Markdown depuis un fichier ou `stdin`, détecte les `DiagramBlock` Mermaid, génère un SVG via `mermaid-svg`, le rasterise avec `resvg`, puis l'affiche dans le terminal via un `GraphicsProtocol`.

Le comportement prioritaire est le streaming robuste : une entrée invalide, tronquée ou trop lourde doit produire un `GracefulFallback` et la lecture du flux doit continuer.

## Documentation A Consulter

- `CONTEXT.md` : vocabulaire de domaine à respecter dans le code, les tests, les commits et la documentation.
- `README.md` : comportement utilisateur, options CLI et exemples.
- `CONTRIBUTING.md` : standards de contribution, TDD, lints et commits.
- `docs/adr/` : décisions architecturales. Créer un nouvel ADR pour une décision structurante ou difficile à inverser.

Pour toute question sur une bibliothèque, un framework, un SDK, une API, un CLI ou un service cloud, utiliser Context7 avant de répondre ou coder. Commencer par `resolve-library-id`, puis interroger la documentation avec `query-docs` sur un concept précis.

## Documents Générés

Par défaut, créer dans `.agents/` tout document produit par un agent, notamment les rapports, analyses, audits, synthèses, plans et notes de recherche. Respecter un autre emplacement si l'utilisateur le demande explicitement ou si le dépôt définit une destination dédiée pour ce type de document. Ne pas déplacer ni écraser un document existant sans nécessité liée à la demande.

## Architecture Actuelle

Le dépôt utilise une architecture Rust plate :

```text
src/
├── main.rs          # point d'entrée
├── lib.rs           # exports de bibliotheque
├── cli.rs           # arguments Clap
├── domain.rs        # types domaine: DiagramBlock, GraphicsProtocol, ExecutionMode, etc.
├── stream.rs        # parsing streaming Markdown / DiagramBlock
├── mermaid.rs       # generation SVG Mermaid native Rust
├── rasterizer.rs    # SVG -> RasterizedImage
├── renderer.rs      # orchestration de rendu
├── filter.rs        # ExecutionMode::StreamFilter
├── pager.rs         # ExecutionMode::LivePager
└── protocol/
    ├── mod.rs
    ├── kitty.rs
    └── halfblock.rs
```

Ne pas introduire une arborescence parallèle (`engine/`, `raster/`, `viewport/`, `ui/`) sans refactor explicite. Adapter les changements aux modules existants.

## Langage Domaine

Respecter les termes de `CONTEXT.md`. Dans le code actuel, les noms effectifs à préserver sont notamment :

- `DiagramBlock`
- `GraphicsProtocol::{Kitty, HalfBlocks, Raw}`
- `ExecutionMode::{StreamFilter, LivePager}`
- `RasterizedImage`
- `ViewportGeometry`
- `CliError`
- `ResourceLimits`

Si `CONTEXT.md` et le code divergent, garder la compatibilité avec le code existant et corriger la documentation dans le même changement quand c'est pertinent.

## Contraintes Techniques

- Rust édition 2024.
- CLI avec `clap`.
- Génération Mermaid native via `mermaid-svg`.
- Rasterisation via `resvg` / `usvg`.
- TUI via `ratatui` / `crossterm`.
- Rendu terminal via Kitty Graphics Protocol, demi-blocs ANSI TrueColor ou sortie `raw`.

Ne pas ajouter Node.js, Puppeteer, Chromium ou un runtime JavaScript dans la boucle de rendu. Si un changement impose un moteur externe, le justifier dans un ADR avant implementation.

## Modes D'Execution

- `ExecutionMode::StreamFilter` : mode composable `stdin -> stdout`, activé par pipe ou `--no-pager`.
- `ExecutionMode::LivePager` : mode TUI interactif, activé sur TTY ou via `--pager`.

Le choix effectif doit rester cohérent avec `src/cli.rs`, `src/filter.rs`, `src/pager.rs` et le README.

## Résilience

Appliquer un `GracefulFallback` lorsqu'un `DiagramBlock` ne peut pas être rendu :

1. afficher un avertissement bref et exploitable ;
2. restituer le contenu du bloc sans perdre l'information utilisateur ;
3. reprendre immédiatement le traitement du flux.

Les limites de ressources doivent passer par `ResourceLimits` ou un mécanisme équivalent déjà présent. Ne pas consommer tout `stdin` en mémoire pour une fonctionnalité de streaming.

## Standards De Code

- Aucun `.unwrap()`, `.expect()` ou `panic!()` en code de production.
- Propager les erreurs avec `Result` et les types du domaine (`CliError` ou équivalent local).
- Préférer les emprunts (`&str`, `&[u8]`) et éviter les clones de contenus lourds.
- Garder des fonctions courtes, lisibles et faiblement imbriquées.
- Ajouter ou ajuster les tests quand le comportement change.
- Modélisation Mermaid Terminal-Safe : tout bloc ou exemple Mermaid produit doit limiter ses libellés à 30 caractères par ligne via `<br/>`, privilégier l'orientation verticale (`TD`) et respecter un encombrement maximal de 80 colonnes pour garantir un affichage sans débordement ni repli dégradé sur tout émulateur TTY.

## Vérification

Avant de considérer une modification terminée, exécuter autant que possible :

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

Pour les changements qui touchent au streaming, vérifier aussi un flux ralenti :

```bash
cat examples/test_sample.md | python3 -c 'import sys, time; [print(c, end="", flush=True) or time.sleep(0.01) for c in sys.stdin.read()]' | cargo run -- --no-pager
```
