# Changelog

Toutes les évolutions notables de Strmaid sont consignées dans ce fichier.

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) et le projet
respecte le [versionnage sémantique](https://semver.org/lang/fr/). Avant la 1.0.0, une
rupture de compatibilité incrémente la version mineure.

## [0.5.1] - 2026-10-09

### Corrections

- Rendu `AsciiBox` (pager et repli des protocoles graphiques) :
  - les guillemets englobant un libellé de nœud de flowchart ne sont plus affichés
    (`A["texte"]` rendu `texte`), sauf pour un libellé contenant `;` ou des
    délimiteurs non équilibrés, que `mermaid-text` ne lit pas sans guillemets ;
  - une arête `A --> |texte| B`, avec une espace avant le libellé, est rendue comme
    une arête libellée et non plus comme un nœud unique ;
  - `&` est affiché tel quel au lieu de `﹠` : la neutralisation des esperluettes
    est réservée au rendu SVG.

## [0.5.0] - 2026-10-09

### Ruptures de compatibilité

- **Serveur MCP** : les sorties des outils sont exposées dans `result.structuredContent`
  (MCP 2025-06-18) au lieu d'être recopiées à la racine de `result`. Les champs `svg`,
  `png_base64`, `dimensions`, `valid`, `error`, `diagram_count` et `blocks` se lisent
  désormais dans `structuredContent` ou dans le texte JSON de `content[0]`.
- **Détection des fences** : les fences CommonMark sont reconnues (`~~~mermaid`, fences
  de 4 caractères ou plus, langage `mermaidjs`). Un bloc n'est plus fermé par une ligne
  ` ```js ` ni par une fence d'un autre caractère, et un ` ```mermaid ` contenu dans un
  autre bloc de code n'est plus interprété.
- **Texte relayé** : les séquences terminales (CSI, OSC, DCS, APC, PM, SOS) du Markdown
  relayé sont neutralisées par défaut, seules les couleurs SGR étant conservées. L'option
  `--raw-passthrough` rétablit le relais brut.
- **Codes de sortie** : les erreurs d'usage renvoient le code 2. Le pager refuse de
  s'ouvrir sans fichier lorsque `stdin` est un terminal.
- **API de la bibliothèque** :
  - `StreamStateMachine::process_line` renvoie `Option<StreamItem>` au lieu de
    `Result<Option<StreamItem>, CliError>` ; `StreamItem` gagne la variante
    `OversizedDiagram`.
  - `pty::is_mermaid_fence_start` et `pty::is_fence_end` sont remplacées par
    `domain::CodeFence`.
  - `asciibox::encode_asciibox`, `GraphicsProtocol::from_str_name` et
    `domain::strip_mermaid_fences` sont supprimées ;
    `encode_image_for_protocol` renvoie `CliError::ImageEncoding` pour `AsciiBox`.

### Ajouts

- `strmaid run` propage le redimensionnement du terminal hôte au processus enfant.
- Outil MCP `strmaid_render` : options `engine` et `timeout_ms` (bornée entre 100 et
  30 000 ms, 5 000 ms par défaut) et alias de thèmes `retro-*`.
- Négociation de `protocolVersion` MCP (2025-06-18, 2024-11-05).
- Déclaration de la version minimale de Rust : `rust-version = "1.95"`.

### Corrections

- Flux : un diagramme dépassant `max_diagram_bytes` est sauté avec un avertissement et
  la lecture continue sans accumuler le bloc ; les octets non UTF-8 sont tolérés.
- Rendu : neutralisation des séquences terminales dans les replis et les messages
  d'erreur ; remontée des échecs d'encodage iTerm2 ; chargement des polices système pour
  les libellés ; libellés contenant `&` rendus correctement ; thème appliqué au moteur
  `merman`.
- Rendu sous garde temporelle : nombre de threads de rendu abandonnés plafonné par
  `ResourceLimits::max_orphan_renders` (4 par défaut).
- Serveur MCP : messages bornés à 2 Mio, sources bornées à `max_diagram_bytes`, sémantique
  des notifications JSON-RPC respectée, rendu aligné sur le pipeline CLI.
- Pager : diagrammes rendus en `AsciiBox`, rendu effectué hors du thread d'interface,
  hauteur utile corrigée.
- `strmaid run` : code de sortie de l'enfant propagé, UTF-8 décodé à travers les
  frontières de chunks, retours chariot isolés préservés, capture des blocs Mermaid
  bornée.
- Sous tmux ou GNU screen, la détection automatique se replie sur les demi-blocs, les
  séquences Kitty et iTerm2 étant filtrées par le multiplexeur.
- Débordement de largeur supprimé dans `target_columns`.

### Performances

- Cache de rendu borné en octets (`ResourceLimits::max_cache_bytes`, 32 Mio) en plus du
  nombre d'entrées ; les replis `AsciiBox` ne sont plus mis en cache.

### Intégration continue

- La release exige le succès de fmt, clippy et des tests sur le commit tagué.
- Actions GitHub épinglées par SHA ; job de vérification de la MSRV ; jeton crates.io
  limité aux secrets.
- Preuves de charge `#[ignore]` (`cargo test --release --test stress -- --ignored`).

[0.5.1]: https://github.com/ebedy/strmaid/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/ebedy/strmaid/compare/v0.4.0...v0.5.0
