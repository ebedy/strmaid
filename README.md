# Strmaid

<div style="text-align: center">

**Moteur haute performance de rendu natif de diagrammes Mermaid et de streaming Markdown pour terminaux modernes.**

[![Crates.io](https://img.shields.io/crates/v/strmaid.svg)](https://crates.io/crates/strmaid)
[![CI](https://github.com/ebedy/strmaid/actions/workflows/ci.yml/badge.svg)](https://github.com/ebedy/strmaid/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org)

</div>

---

**Strmaid** (`Stream` + `Mermaid`) est un outil CLI unifié, multiplateforme (Linux, macOS, Windows) et ultra-rapide écrit en pur Rust, conçu pour parser les flux Markdown en direct et afficher instantanément des diagrammes Mermaid pixel-perfect dans votre terminal (Windows Terminal, Kitty, Ghostty, iTerm2, WezTerm, Alacritty, Warp, etc.).

Spécifiquement optimisé pour les pipelines de tuyauterie (`stdin -> stdout`, PTY/ConPTY) et la consommation en direct ou statique de flux textuels émis par des agents IA.

---

## Fonctionnalités Clés

- **Moteur 100% Rust Natif (Zéro dépendance JavaScript/V8) :**
  Génération et rasterisation instantanées (< 10 ms) grâce à [`mermaid-svg`](https://crates.io/crates/mermaid-svg) et [`resvg`](https://crates.io/crates/resvg). Aucun runtime V8 (`deno_core`), Node.js ou processus headless Chromium n'est requis.
- **Protocoles Graphiques Avancés & Rendu Matriciel :** (sous tmux ou GNU screen, la détection automatique se replie sur les demi-blocs, Kitty et iTerm2 étant filtrés par le multiplexeur)
  - **Kitty Graphics Protocol :** Rendu haute fidélité natif pixel-perfect (supporté par Kitty, Ghostty, WezTerm).
  - **iTerm2 Inline Images Protocol :** Protocole graphique inline supporté notamment sous macOS (iTerm2, WezTerm).
  - **ANSI HalfBlocks TrueColor (Fallback universel) :** Repli déterministe en demi-blocs Unicode (`▀`, `▄`) avec couleurs 24-bit, compatible avec tous les émulateurs sous Windows (Windows Terminal, PowerShell, CMD), Linux et macOS.
  - **AsciiBox (Fallback textuel monochrome) :** Rendu textuel compact pour terminaux contraints ou logs CI/CD sans TrueColor.
- **Double Mode d'Exécution (Dual-Mode) :**
  - **Live Streaming Pager (Interactif TUI) :** Activé automatiquement sur un TTY interactif ou avec `--pager`. Interface plein écran basée sur [`ratatui`](https://crates.io/crates/ratatui), avec auto-scroll en direct pendant le streaming et navigation fluide au clavier (`j`/`k`, flèches, `q`). Les diagrammes y sont tracés en `AsciiBox` monochrome, seul rendu représentable par les widgets texte de `ratatui` ; utilisez `--no-pager` pour un rendu graphique Kitty, iTerm2 ou demi-blocs. Les diagrammes sont rendus en arrière-plan : la navigation reste réactive pendant un rendu long, et une erreur de lecture du flux s'affiche dans le titre sans fermer le pager. Lancé sans fichier alors que l'entrée standard est un terminal, `strmaid` refuse d'ouvrir le pager (code de sortie `2`) : fournir un fichier ou un flux via un pipe.
  - **Filtre Composable :** Activé automatiquement dans les pipes ou avec `--no-pager`. Agit comme un filtre `stdin -> stdout` classique, idéal pour composer avec `cat`, `grep`, `less -R` ou des redirections.
- **Détection CommonMark des blocs Mermaid :**
  Les blocs ```` ```mermaid ````, `~~~mermaid` et les fences longues (```` ````mermaid ````) sont reconnus, ainsi que le langage `mermaidjs`, y compris lorsqu'ils sont indentés dans une liste. Un bloc ne se ferme que sur une fence du même caractère et de longueur au moins égale, sans info-string : une ligne ```` ```js ```` interne ne le termine plus. Un ```` ```mermaid ```` placé dans un autre bloc de code (exemple de syntaxe dans un bloc ```` ````markdown ````) reste du texte. La même grammaire s'applique au filtre, au pager, à `strmaid run`, à `--block-only` et au serveur MCP.
- **Repli Gracieux (Graceful Fallback) :**
  En cas d'erreur de syntaxe Mermaid, le diagramme est affiché sous forme de bloc de texte brut formaté avec un avertissement discret, sans interrompre le flux ni faire planter l'application.
- **Flux résilient aux entrées hostiles :**
  Un bloc Mermaid dépassant 1 Mio est ignoré sans être chargé en mémoire : un avertissement le signale et la lecture du document se poursuit. Les octets non UTF-8 sont remplacés par `U+FFFD` (un avertissement unique sur `stderr`) au lieu d'interrompre le flux. Dans `strmaid run`, la capture d'un bloc non refermé est bornée de la même façon et la sortie de la commande enfant est lue avec contre-pression, à mémoire constante.
- **Neutralisation des séquences terminales :**
  Les contenus réémis (blocs en repli, messages d'erreur, lignes du pager) sont débarrassés de toute séquence de contrôle (OSC, DCS, APC, CSI) : un flux d'agent IA ne peut pas écrire dans le presse-papiers (OSC 52), renommer la fenêtre ni injecter d'image Kitty. Le texte Markdown relayé en mode filtre conserve uniquement ses couleurs SGR (voir `--raw-passthrough`).
- **Dimensionnement Dynamique & Thèmes :**
  Calcul proportionnel automatique selon les dimensions matricielles du terminal (`ViewportGeometry`) et gestion des thèmes visuels (`dark`, `light`, `neutral`, `amber`, `phosphor`, `neon`, `mono`).

---

## Installation (Linux, macOS, Windows)

### Prérequis
- Environnement Linux (Debian, Ubuntu, Arch, Fedora, etc.) ou macOS.
- Polices système installées (ex. paquets `fonts-dejavu` ou `fonts-noto` sous Linux) pour afficher les libellés des diagrammes en rendu graphique. En leur absence (conteneur minimal, binaire musl), `strmaid` bascule automatiquement sur `AsciiBox` ; `strmaid doctor` indique le nombre de polices détectées.
- Toolchain Rust 1.95 ou plus récente (`rust-version` déclarée dans `Cargo.toml`) :
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

### 1. Via Cargo depuis Crates.io (Recommandé)
```bash
cargo install strmaid
```
*(Le binaire `strmaid` est directement installé dans `~/.cargo/bin/`).*

### 2. Binaires autonomes précompilés (Sans runtime Rust)
Téléchargez l'archive correspondant à votre architecture depuis la page [**GitHub Releases v0.5.0**](https://github.com/ebedy/strmaid/releases/tag/v0.5.0) :

| Plateforme | Cible | Téléchargement |
| :--- | :--- | :--- |
| **Linux (glibc)** | `x86_64-unknown-linux-gnu` | [Archive .tar.gz](https://github.com/ebedy/strmaid/releases/download/v0.5.0/strmaid-v0.5.0-x86_64-unknown-linux-gnu.tar.gz) |
| **Linux (musl statique)** | `x86_64-unknown-linux-musl` | [Archive .tar.gz](https://github.com/ebedy/strmaid/releases/download/v0.5.0/strmaid-v0.5.0-x86_64-unknown-linux-musl.tar.gz) |
| **Linux ARM64** | `aarch64-unknown-linux-gnu` | [Archive .tar.gz](https://github.com/ebedy/strmaid/releases/download/v0.5.0/strmaid-v0.5.0-aarch64-unknown-linux-gnu.tar.gz) |
| **macOS Apple Silicon** | `aarch64-apple-darwin` | [Archive .tar.gz](https://github.com/ebedy/strmaid/releases/download/v0.5.0/strmaid-v0.5.0-aarch64-apple-darwin.tar.gz) |
| **macOS Intel** | `x86_64-apple-darwin` | [Archive .tar.gz](https://github.com/ebedy/strmaid/releases/download/v0.5.0/strmaid-v0.5.0-x86_64-apple-darwin.tar.gz) |
| **Windows x64** | `x86_64-pc-windows-msvc` | [Archive .zip](https://github.com/ebedy/strmaid/releases/download/v0.5.0/strmaid-v0.5.0-x86_64-pc-windows-msvc.zip) |

```bash
# Exemple d'installation sous Linux / macOS :
tar -xzf strmaid-v0.5.0-*.tar.gz
sudo mv strmaid /usr/local/bin/
```

### 3. Depuis les sources locales
```bash
git clone https://github.com/ebedy/strmaid.git
cd strmaid
cargo install --path .
```

### 4. Vérification & Diagnostic Système
Validez l'installation et inspectez les capacités de votre terminal hôte avec la commande dédiée :
```bash
strmaid doctor
```

### 5. Intégration pour Agents IA via Skill
Strmaid respecte la spécification ouverte [**`agentskills.io`**](https://agentskills.io) et est déployable via le registre [**`skills.sh`**](https://skills.sh) :
```bash
npx skills add ebedy/strmaid
```
Ou manuellement pour Antigravity / Claude Code :
```bash
mkdir -p ~/.agents/skills/strmaid
cp skills/strmaid/SKILL.md ~/.agents/skills/strmaid/SKILL.md
```
*(Consultez la fiche procédurale complète dans [`skills/strmaid/SKILL.md`](skills/strmaid/SKILL.md)).*

---

## Manuel d'Utilisation

### Syntaxe générale
```bash
strmaid [OPTIONS] [FILE]
```

### Options de la ligne de commande

| Option | Argument | Description |
| :--- | :--- | :--- |
| `[FILE]` | Chemin | Fichier Markdown à lire. Si omis, `strmaid` lit depuis l'entrée standard (`stdin`). |
| `-b`, `--block-only` | *Aucun* | Mode visualiseur pour éditeur : rend un unique bloc Mermaid (avec ou sans balises Markdown), sans TUI. Code de sortie `0` (valide) ou `1` (erreur). |
| `-w`, `--width` | Nombre (10–1000) | Surcharge la largeur du terminal en colonnes (déclenche la compaction progressive si < 80 cols). |
| `-p`, `--pager` | *Aucun* | Force le mode pager interactif plein écran (TUI Ratatui). |
| `--no-pager` | *Aucun* | Désactive le pager et force le mode filtre Unix composable (`stdout`). |
| `-g`, `--graphics` | `kitty` \| `iterm2` \| `halfblocks` \| `asciibox` \| `raw` | Force le protocole graphique. Auto-détecté par défaut selon l'environnement. |
| `-t`, `--theme` | `dark` \| `light` \| `neutral` \| `amber` \| `phosphor` \| `neon` \| `mono` | Définit le thème visuel pour les diagrammes (par défaut : `dark`). |
| `--engine` | `mermaid-svg` \| `merman` | Moteur de rendu Mermaid (`mermaid-svg` par défaut, `merman` disponible avec la feature `merman`). Le thème `--theme` s'applique aux deux moteurs. |
| `--no-auto-orient` | *Aucun* | Désactive l'auto-orientation préventive (`LR`/`RL` $\rightarrow$ `TD`) sur terminaux étroits (< 120 cols). |
| `--timeout-ms` | `MS` | Délai maximal d'exécution d'un rendu de diagramme en millisecondes (par défaut : `5000`, `0` pour désactiver). |
| `--no-fallback-asciibox` | *Aucun* | Désactive le repli automatique vers `AsciiBox` en cas d'échec du rendu graphique et restitue le code brut. |
| `--raw-passthrough` | *Aucun* | Mode filtre uniquement : relaie le texte Markdown hors diagramme tel quel. Par défaut, les séquences terminales actives (OSC, DCS, APC, CSI hors couleurs) sont neutralisées et seules les couleurs SGR sont conservées. |
| `--format` | `human` \| `json` \| `ndjson` | Format de sortie des flux analysés (par défaut : `human`). |
| `-h`, `--help` | *Aucun* | Affiche l'aide de la commande. |
| `-V`, `--version` | *Aucun* | Affiche la version de l'application. |

### Sous-commandes dédiées

| Sous-commande | Arguments | Description |
| :--- | :--- | :--- |
| `strmaid doctor` | `[--format human\|json\|ndjson]` | Diagnostique les capacités matérielles et logicielles du terminal hôte (TrueColor, PTY, SVG, resvg, polices système). |
| `strmaid run` | `<COMMAND...>` | Exécute une commande dans un pseudo-terminal (PTY) interactif en interceptant les diagrammes Mermaid. Retourne le code de sortie du processus enfant (borné à 255). |
| `strmaid mcp` | *Aucun* | Démarre le serveur Model Context Protocol (MCP) natif sur standard I/O (JSON-RPC 2.0). |

---

## Exemples Concrets

### 1. Lecture directe d'un fichier Markdown
```bash
strmaid examples/test_sample.md
```

### 2. Consommation d'un flux d'agent IA en streaming (Mode Tube Unix)
Pour consommer la réponse d'un agent de code en streaming non-interactif via un tube Unix :
```bash
# Exemple avec Antigravity CLI (agy) en mode print avec désactivation du pager :
agy -p "Décris l'architecture du projet avec un diagramme Mermaid" | strmaid --no-pager

# Exemple générique avec tout agent IA émettant sur stdout :
mon_agent_ia --print "Génère un flux Markdown" | strmaid --no-pager
```
> **Conseil :** L'option `--no-pager` est recommandée lors de l'enchaînement dans un pipe pour éviter l'instanciation du pager plein écran et prévenir les collisions d'affichage si l'outil source émet des messages d'avertissement sur `stderr`.

### 3. Pipeline Unix composable avec redirection
Pour envoyer le résultat vers un fichier ou un pager tiers :
```bash
cat document.md | strmaid --no-pager | less -R
```

### 4. Forcer le protocole en demi-blocs TrueColor
Idéal pour les terminaux ne supportant pas le protocole Kitty :
```bash
strmaid examples/test_sample.md --graphics halfblocks
```

### 5. Repli textuel universel en art Unicode / ASCII Box-Drawing
Pour les terminaux série, sessions SSH limitées ou logs CI sans couleur :
```bash
strmaid examples/test_sample.md --graphics asciibox
```

### 6. Utiliser un thème visuel rétro ou contrasté
```bash
# Style terminal CRT ambre (VT220) :
strmaid examples/test_sample.md --theme amber

# Style terminal CRT vert phosphorescent :
strmaid examples/test_sample.md --theme phosphor

# Palette cyberpunk / synthwave haute saturation :
strmaid examples/test_sample.md --theme neon

# Strict monochrome noir et blanc (e-ink / terminaux sans couleur) :
strmaid examples/test_sample.md --theme mono
```

### 7. Sortie structurée pour agents IA (NDJSON & JSON)
Pour consommer les métadonnées de rendu et valider la syntaxe sans latence dans un pipeline d'agent :
```bash
# Streaming ligne par ligne (NDJSON temps réel) :
cat flux.md | strmaid --format ndjson

# Rapport complet du document (JSON à EOF) :
strmaid examples/test_sample.md --format json
```

### 8. Intégration dans un éditeur (Neovim, Helix) avec largeur personnalisée
Pour prévisualiser instantanément la sélection ou le bloc sous le curseur dans une fenêtre flottante :
```bash
# Rendu immédiat d'une sélection (avec ou sans ```mermaid) contrainte à 60 colonnes :
xclip -o | strmaid --block-only --width 60

# Dans Neovim (:!strmaid -b -w 80) ou intégration comme linter Mermaid :
strmaid --block-only diagram.mmd
echo "Valide ? Statut exit code = $?"
```

### 9. Diagnostic des capacités du terminal hôte
```bash
# Rapport lisible humain :
strmaid doctor

# Rapport d'audit au format JSON machine-readable :
strmaid doctor --format json
```

### 10. Interception dynamique d'agents interactifs (PTY / ConPTY)
Pour exécuter une session interactive d'agent IA (ou tout script CLI) tout en conservant l'interactivité complète du terminal et en interceptant automatiquement les diagrammes Mermaid en direct :
```bash
# Lancement interactif d'Antigravity CLI (agy) au sein de strmaid :
strmaid run agy

# Reprise d'une conversation existante en session interactive PTY :
strmaid run agy --dangerously-skip-permissions --conversation=<hash conversation>

# Interception d'un script ou agent Python interactif :
strmaid run python3 mon_agent.py
```
Le code de sortie de la commande enfant est propagé (`strmaid run sh -c 'exit 42'` retourne `42`), les redimensionnements de la fenêtre sont transmis à la commande (vim, htop, less s'adaptent), les retours chariot isolés (barres de progression) sont préservés et les caractères UTF-8 multi-octets sont décodés sans corruption entre deux lectures du PTY.

> **Sécurité :** `strmaid run` relaie la sortie de l'enfant sans filtrage afin de préserver les applications plein écran (vim, htop). Les séquences OSC, DCS et APC émises par la commande atteignent donc le terminal hôte ; n'exécuter via `run` que des commandes de confiance.

---

## Intégration Serveur MCP (Model Context Protocol)

`strmaid` intègre nativement un serveur **Model Context Protocol (MCP)** standardisé via standard I/O (JSON-RPC 2.0). C'est le mode d'intégration recommandé pour les agents d'IA (Antigravity `agy`, Claude Desktop, Cursor, Windsurf, Cline).

### Outils exposés aux agents IA

| Outil MCP | Description | Paramètres d'entrée |
| :--- | :--- | :--- |
| `strmaid_validate` | Valide instantanément la syntaxe Mermaid (< 10 ms) et localise l'anomalie exacte. | `source` (string, requis) |
| `strmaid_render` | Génère le SVG vectoriel et le PNG matriciel encodé en Base64 avec dimensions exactes. | `source` (string), `theme`, `width`, `engine`, `timeout_ms` (optionnels) |
| `strmaid_detect` | Scanne un document Markdown et extrait tous les diagrammes Mermaid avec leurs métadonnées. | `markdown` (string, requis) |

### Configuration pour Antigravity CLI (`agy`)

Ajoutez l'entrée dans `~/.gemini/config/mcp_config.json` :
```json
{
  "mcpServers": {
    "strmaid": {
      "command": "strmaid",
      "args": ["mcp"]
    }
  }
}
```

### Configuration pour Claude Desktop

Dans `~/.config/Claude/claude_desktop_config.json` (Linux) ou `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) :
```json
{
  "mcpServers": {
    "strmaid": {
      "command": "strmaid",
      "args": ["mcp"]
    }
  }
}
```

### Configuration pour Cursor / Windsurf / Cline

Dans les paramètres MCP (`mcp.json`) :
```json
{
  "mcpServers": {
    "strmaid": {
      "command": "strmaid",
      "args": ["mcp"],
      "transport": "stdio"
    }
  }
}
```

### Conformité du protocole

- Révisions supportées : `2025-06-18` (renvoyée par défaut) et `2024-11-05` (renvoyée si le client la demande).
- Résultats d'outils : `structuredContent` contient la sortie structurée ; le même JSON figure dans `content[0].text` pour les clients antérieurs.
- Les notifications (messages sans `id`) ne reçoivent jamais de réponse, conformément à JSON-RPC 2.0.

### Limites de robustesse

- Messages JSON-RPC limités à 2 Mio (au-delà : erreur `-32700`, le serveur continue) ; `source` limitée à 1 Mio.
- Validation, génération SVG, rasterisation et encodage PNG s'exécutent sous un délai maximal : `timeout_ms` est borné entre 100 et 30 000 ms (défaut 5 000 ms, `0` ne désactive pas la garde).
- Les moteurs n'étant pas interruptibles, au plus 4 rendus expirés continuent en arrière-plan ; les rendus suivants sont refusés jusqu'à leur terminaison, ce qui borne la consommation CPU du serveur.

### Test de communication stdio

Pour vérifier l'initialisation du serveur MCP depuis votre shell :
```bash
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}\n' | strmaid mcp
```

---

## Navigation dans le Pager TUI

Lorsque le mode Pager interactif est actif :
- `↑` / `k` : Défilement d'une ligne vers le haut.
- `↓` / `j` : Défilement d'une ligne vers le bas.
- `Page Up` / `b` : Défilement d'une page entière vers le haut.
- `Page Down` / `Space` / `f` : Défilement d'une page entière vers le bas.
- `u` / `d` : Défilement d'une demi-page vers le haut / bas.
- `Home` / `g` : Saut immédiat au début du document.
- `End` / `G` : Saut immédiat à la fin du document.
- Molette de souris : Défilement vertical fluide.
- `q`, `Esc` ou `Ctrl-c` : Quitter le pager et revenir au shell.

---

## Architecture & Décisions Techniques

Le projet applique une conception orientée domaine (DDD) stricte avec une politique de zéro avertissement au compilateur (`unwrap_used = "deny"`, `panic = "deny"`).

- **Glossaire Métier & Décisions Fondatrices :** Consultez [`CONTEXT.md`](CONTEXT.md) pour les définitions canoniques (`DiagramBlock`, `GraphicsProtocol`, `ExecutionMode`, etc.) ainsi que les arbitrages d'architecture fondateurs (moteur Rust natif sub-10ms, protocoles graphiques Kitty/TrueColor/AsciiBox, dual-mode filtre/pager et dimensionnement dynamique).

---

## Feuille de Route & Backlog Fonctionnel

Suite au benchmark comparatif face à l'écosystème existant (`fasouto/termaid`, `meraid`, `mermaid-text`), les fonctionnalités suivantes sont intégrées à la roadmap de développement :

1. **Protocole de repli textuel Unicode / ASCII (`GraphicsProtocol::AsciiBox`) :** [Livré]
   Offrir un troisième niveau de repli dégradé en caractères box-drawing Unicode ou ASCII 7-bit pour les terminaux et environnements dépourvus de Kitty et de TrueColor 24-bit (sessions SSH anciennes, logs CI/CD monochromes).
2. **Sortie Structurée pour Agents IA (`--format json` / NDJSON) :** [Livré]
   Permettre l'émission de flux de métadonnées machine-readable (état de validation syntaxique, dimensions, erreurs explicites avec numéro de ligne et type d'anomalie) pour intégration transparente dans les pipelines d'agents d'IA.
3. **Thématisation rétro et fort contraste :** [Livré]
   Ajout des palettes `amber` (ambre monochrome style VT220), `phosphor` (vert phosphorescent), `neon` (cyberpunk) et `mono` (monochrome fort contraste).
4. **Calcul de largeur Unicode & CJK-Awareness :** [Livré]
   Prise en compte rigoureuse des caractères double-chasse CJK et emojis via `unicode-width` avec neutralisation des séquences ANSI pour garantir un dimensionnement matriciel sans distorsion.
5. **Intégration comme visualiseur d'éditeur (`--block-only`) :** [Livré]
   Point d'entrée dédié pour les extensions d'éditeurs (Neovim, Helix, VSCode terminal) afin d'afficher instantanément un diagramme sous le curseur avec code de sortie shell strict.
6. **Compaction progressive adaptative :** [Livré]
   Ajustement automatique du ratio et de la compacité vectorielle et matricielle lorsque le terminal présente une largeur contrainte (< 80 colonnes).

---

## Contribution & Communauté

Nous accueillons les contributions avec plaisir ! Avant de soumettre une Pull Request, veuillez consulter :
- Notre [Guide de Contribution](CONTRIBUTING.md) pour les règles de code, TDD et lints.
- Notre [Code de Conduite](CODE_OF_CONDUCT.md).
- Notre [Politique de Sécurité](SECURITY.md) pour tout signalement de vulnérabilité.

---

## Licence

Ce projet est distribué sous double licence libre :
- **Licence MIT** ([`LICENSE-MIT`](LICENSE-MIT))
- **Licence Apache 2.0** ([`LICENSE-APACHE`](LICENSE-APACHE))

Vous êtes libre de choisir la licence qui convient le mieux à vos besoins d'utilisation et d'intégration.
