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
- **Protocoles Graphiques Avancés & Rendu Matriciel :**
  - **Kitty Graphics Protocol :** Rendu haute fidélité natif pixel-perfect (supporté par Kitty, Ghostty, WezTerm).
  - **iTerm2 Inline Images Protocol :** Protocole graphique inline supporté notamment sous macOS (iTerm2, WezTerm).
  - **ANSI HalfBlocks TrueColor (Fallback universel) :** Repli déterministe en demi-blocs Unicode (`▀`, `▄`) avec couleurs 24-bit, compatible avec tous les émulateurs sous Windows (Windows Terminal, PowerShell, CMD), Linux et macOS.
  - **AsciiBox (Fallback textuel monochrome) :** Rendu textuel compact pour terminaux contraints ou logs CI/CD sans TrueColor.
- **Double Mode d'Exécution (Dual-Mode) :**
  - **Live Streaming Pager (Interactif TUI) :** Activé automatiquement sur un TTY interactif ou avec `--pager`. Interface plein écran basée sur [`ratatui`](https://crates.io/crates/ratatui), avec auto-scroll en direct pendant le streaming et navigation fluide au clavier (`j`/`k`, flèches, `q`).
  - **Filtre Composable :** Activé automatiquement dans les pipes ou avec `--no-pager`. Agit comme un filtre `stdin -> stdout` classique, idéal pour composer avec `cat`, `grep`, `less -R` ou des redirections.
- **Repli Gracieux (Graceful Fallback) :**
  En cas d'erreur de syntaxe Mermaid, le diagramme est affiché sous forme de bloc de texte brut formaté avec un avertissement discret, sans interrompre le flux ni faire planter l'application.
- **Dimensionnement Dynamique & Thèmes :**
  Calcul proportionnel automatique selon les dimensions matricielles du terminal (`ViewportGeometry`) et gestion des thèmes visuels (`dark`, `light`, `neutral`, `amber`, `phosphor`, `neon`, `mono`).

---

## Installation (Linux, macOS, Windows)

### Prérequis
- Environnement Linux (Debian, Ubuntu, Arch, Fedora, etc.) ou macOS.
- Toolchain Rust (version 1.85+ ou édition 2024 recommandée) :
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

### 1. Via Cargo depuis Crates.io (Recommandé)
```bash
cargo install strmaid
```
*(Le binaire `strmaid` est directement installé dans `~/.cargo/bin/`).*

### 2. Binaires autonomes précompilés (Sans runtime Rust)
Téléchargez l'archive correspondant à votre architecture depuis la page [**GitHub Releases v0.1.0**](https://github.com/ebedy/strmaid/releases/tag/v0.1.0) :

| Plateforme | Cible | Téléchargement |
| :--- | :--- | :--- |
| **Linux (glibc)** | `x86_64-unknown-linux-gnu` | [Archive .tar.gz](https://github.com/ebedy/strmaid/releases/download/v0.1.0/strmaid-v0.1.0-x86_64-unknown-linux-gnu.tar.gz) |
| **Linux (musl statique)** | `x86_64-unknown-linux-musl` | [Archive .tar.gz](https://github.com/ebedy/strmaid/releases/download/v0.1.0/strmaid-v0.1.0-x86_64-unknown-linux-musl.tar.gz) |
| **Linux ARM64** | `aarch64-unknown-linux-gnu` | [Archive .tar.gz](https://github.com/ebedy/strmaid/releases/download/v0.1.0/strmaid-v0.1.0-aarch64-unknown-linux-gnu.tar.gz) |
| **macOS Apple Silicon** | `aarch64-apple-darwin` | [Archive .tar.gz](https://github.com/ebedy/strmaid/releases/download/v0.1.0/strmaid-v0.1.0-aarch64-apple-darwin.tar.gz) |
| **macOS Intel** | `x86_64-apple-darwin` | [Archive .tar.gz](https://github.com/ebedy/strmaid/releases/download/v0.1.0/strmaid-v0.1.0-x86_64-apple-darwin.tar.gz) |
| **Windows x64** | `x86_64-pc-windows-msvc` | [Archive .zip](https://github.com/ebedy/strmaid/releases/download/v0.1.0/strmaid-v0.1.0-x86_64-pc-windows-msvc.zip) |

```bash
# Exemple d'installation sous Linux / macOS :
tar -xzf strmaid-v0.1.0-*.tar.gz
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
| `-w`, `--width` | Nombre | Surcharge la largeur du terminal en colonnes (déclenche la compaction progressive si < 80 cols). |
| `-p`, `--pager` | *Aucun* | Force le mode pager interactif plein écran (TUI Ratatui). |
| `--no-pager` | *Aucun* | Désactive le pager et force le mode filtre Unix composable (`stdout`). |
| `-g`, `--graphics` | `kitty` \| `halfblocks` \| `asciibox` \| `raw` | Force le protocole graphique. Auto-détecté par défaut selon l'environnement. |
| `-t`, `--theme` | `dark` \| `light` \| `neutral` \| `amber` \| `phosphor` \| `neon` \| `mono` | Définit le thème visuel pour les diagrammes (par défaut : `dark`). |
| `--format` | `human` \| `json` \| `ndjson` | Format de sortie des flux analysés (par défaut : `human`). |
| `-h`, `--help` | *Aucun* | Affiche l'aide de la commande. |
| `-V`, `--version` | *Aucun* | Affiche la version de l'application. |

### Sous-commandes dédiées

| Sous-commande | Arguments | Description |
| :--- | :--- | :--- |
| `strmaid doctor` | `[--format human\|json\|ndjson]` | Diagnostique les capacités matérielles et logicielles du terminal hôte (TrueColor, PTY, SVG, resvg). |
| `strmaid run` | `<COMMAND...>` | Exécute une commande dans un pseudo-terminal (PTY) interactif en interceptant les diagrammes Mermaid. |
| `strmaid mcp` | *Aucun* | Démarre le serveur Model Context Protocol (MCP) natif sur standard I/O (JSON-RPC 2.0). |

---

## Exemples Concrets

### 1. Lecture directe d'un fichier Markdown
```bash
strmaid examples/test_sample.md
```

### 2. Consommation d'un flux d'agent IA en streaming
```bash
mon_agent_ia --stream | strmaid
```

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

### 10. Interception dynamique de commandes externes (PTY / ConPTY)
Pour exécuter un script ou un agent CLI interactif et intercepter automatiquement ses diagrammes Mermaid en direct :
```bash
strmaid run python3 mon_agent.py
```

---

## Intégration Serveur MCP (Model Context Protocol)

`strmaid` intègre nativement un serveur **Model Context Protocol (MCP)** standardisé via standard I/O (JSON-RPC 2.0). C'est le mode d'intégration recommandé pour les agents d'IA (Antigravity `agy`, Claude Desktop, Cursor, Windsurf, Cline).

### Outils exposés aux agents IA

| Outil MCP | Description | Paramètres d'entrée |
| :--- | :--- | :--- |
| `strmaid_validate` | Valide instantanément la syntaxe Mermaid (< 10 ms) et localise l'anomalie exacte. | `source` (string, requis) |
| `strmaid_render` | Génère le SVG vectoriel et le PNG matriciel encodé en Base64 avec dimensions exactes. | `source` (string), `theme` (optionnel), `width` (optionnel) |
| `strmaid_detect` | Scanne un document Markdown et extrait tous les diagrammes Mermaid avec leurs métadonnées. | `markdown` (string, requis) |

### Configuration pour Antigravity CLI (`agy`)

Ajoutez l'entrée dans [`~/.gemini/config/mcp_config.json`](~/.gemini/config/mcp_config.json) :
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

### Test de communication stdio

Pour vérifier l'initialisation du serveur MCP depuis votre shell :
```bash
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}\n' | strmaid mcp
```

---

## Navigation dans le Pager TUI

Lorsque le mode Pager interactif est actif :
- `↑` / `k` : Défilement d'une ligne vers le haut.
- `↓` / `j` : Défilement d'une ligne vers le bas.
- `Page Up` : Défilement rapide vers le haut.
- `Page Down` : Défilement rapide vers le bas.
- `q` ou `Esc` : Quitter le pager et revenir au shell.

---

## Architecture & Décisions Techniques

Le projet applique une conception orientée domaine (DDD) stricte avec une politique de zéro avertissement au compilateur (`unwrap_used = "deny"`, `panic = "deny"`).

- **Glossaire Métier & Décisions Fondatrices :** Consultez [`CONTEXT.md`](CONTEXT.md) pour les définitions canoniques (`DiagramBlock`, `GraphicsProtocol`, `ExecutionMode`, etc.) ainsi que les arbitrages d'architecture fondateurs (moteur Rust natif sub-10ms, protocoles graphiques Kitty/TrueColor/AsciiBox, dual-mode filtre/pager et dimensionnement dynamique).

---

## Feuille de Route & Backlog Fonctionnel

Suite au benchmark comparatif face à l'écosystème existant (`fasouto/termaid`, `meraid`, `mermaid-text`), les fonctionnalités suivantes sont intégrées à la roadmap de développement (détails dans [`.agents/features_backlog_and_market_analysis.md`](.agents/features_backlog_and_market_analysis.md)) :

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
