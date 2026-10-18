# Changelog

Toutes les évolutions notables de Strmaid sont consignées dans ce fichier.

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) et le projet
respecte le [versionnage sémantique](https://semver.org/lang/fr/). Avant la 1.0.0, une
rupture de compatibilité incrémente la version mineure.

## [Non publié]

### Corrections

- Moteur `merman` : les libellés des nœuds s'affichent désormais pour `flowchart`,
  `requirementDiagram`, `usecase`, `agentflow` et la plupart des nœuds de `swimlane-beta`.
  `merman` écrivait ces libellés dans des `<foreignObject>` HTML que `resvg` ne dessine pas,
  et les nœuds restaient vides. La directive `init` injectée porte `"htmlLabels": false` ;
  une directive `init` de l'utilisateur garde la priorité. `mindmap`, `kanban`, `block-beta`
  et une partie de `eventmodeling` et `userJourney` écrivent toujours des `<foreignObject>` :
  leurs libellés restent absents de l'image avec `--engine merman`.

## [0.6.0] - 2026-10-18

### Corrections

- Rendu `AsciiBox` : un bloc précédé d'un front matter YAML (`---` … `---`) n'est plus
  rejeté avec `type de diagramme non supporté en mode texte: ---`. Le front matter est
  retiré avant l'appel à `mermaid-text`.
- Rendu `AsciiBox` : la largeur cible est désormais une limite stricte. Un diagramme qui la
  dépasse après compactage bascule en `GracefulFallback` (`Limite de ressources dépassée: le
  diagramme dépasse la largeur du terminal (105 > 74 colonnes)`, source restituée) au lieu
  de déborder du terminal ; en JSON, `kind: "TooWide"`. La largeur cible est celle des
  graphiques (93 % ou 90 % du terminal) : un rendu qui tiendrait entre cette marge et la
  largeur réelle est aussi replié.
- Le langage d'une fence Mermaid est reconnu sans distinction de casse (` ```Mermaid `,
  ` ```MERMAID `, ` ```MermaidJS `), pour le filtre, le pager, `strmaid run`, `--block-only`
  et le serveur MCP.
- Un bloc Mermaid non fermé en fin de flux est restitué précédé d'un avertissement
  (`Bloc Mermaid non fermé : flux terminé avant la clôture, contenu restitué sans rendu`) ;
  en JSON/NDJSON, il devient un élément `diagram` avec `valid: false` et
  `kind: "UnterminatedBlock"` au lieu d'un élément `text`. `strmaid run` n'est pas modifié.
- Le repli d'un type de diagramme inconnu de `mermaid-svg` indique désormais la piste à suivre :
  `type de diagramme inconnu: venn-beta (non pris en charge par mermaid-svg ; essayez --engine
  merman)`, ou `recompilez avec --features merman puis utilisez --engine merman` si le binaire
  n'embarque pas `merman`. Le `kind` JSON (`UnknownDiagramType`) est inchangé.
- Mémoire bornée sur une ligne de texte très longue : une ligne sans `\n` n'est plus
  chargée en entier (16 Mio de ligne : 37 Mio de RSS auparavant, 6 Mio maintenant). Elle est
  relayée sans perte par segments de `ResourceLimits::max_line_bytes` (1 Mio) en filtre et
  en pager. Une ligne plus longue que cette limite n'ouvre ni ne ferme plus de bloc
  Mermaid ; en sortie NDJSON/JSON, elle apparaît en plusieurs éléments `text` consécutifs.

## [0.5.1] - 2026-10-10

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

[0.6.0]: https://github.com/ebedy/strmaid/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/ebedy/strmaid/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/ebedy/strmaid/compare/v0.4.0...v0.5.0
