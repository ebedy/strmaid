---
name: strmaid
description: Native terminal rendering, syntax validation, MCP server, and structured analysis for Mermaid diagrams in streaming Markdown. Use whenever generating, inspecting, validating, or fixing Mermaid code, running the Strmaid MCP server, diagnosing terminal graphics capabilities, or previewing Markdown flows.
license: MIT OR Apache-2.0
compatibility: Linux, macOS, or Windows (requires strmaid binary in PATH)
metadata:
  version: "0.1.0"
  repository: https://github.com/ebedy/strmaid
---

# Strmaid Agent Skill

Ce skill fournit les procédures et directives permettant à un agent IA de piloter le moteur natif **`strmaid`** pour :
1. **Intégrer le serveur MCP natif (`strmaid mcp`)** pour le rendu vectoriel/matriciel (SVG, PNG Base64), la validation syntaxique et l'extraction de diagrammes via JSON-RPC 2.0.
2. **Diagnostiquer les capacités du terminal hôte (`strmaid doctor`)** pour auditer l'environnement, le protocole graphique optimal et la géométrie avant tout affichage.
3. **Valider et auto-corriger** la syntaxe de diagrammes Mermaid sans dépendance JavaScript ni navigateur headless (boucle de self-healing ultra-rapide < 10 ms).
4. **Auditer la conformité** de documents Markdown contenant plusieurs blocs Mermaid via une sortie structurée machine-readable JSON ou NDJSON (streaming en continu).
5. **Prévisualiser des diagrammes** directement dans la console ou dans les terminaux intégrés d'éditeurs (Neovim, VSCode, Helix) avec ajustement adaptatif au viewport et gestion de 7 thèmes visuels.
6. **Intercepter les flux interactifs en pseudo-terminal (`strmaid run <cmd...>`)** pour visualiser dynamiquement les diagrammes générés par des CLI d'agents ou des scripts.

---

## 1. Installation du Binaire Prérequis

Avant d'exécuter `strmaid`, s'assurer que le binaire est présent dans le `$PATH` :

```bash
# Sous Linux / macOS (bash / zsh) :
command -v strmaid >/dev/null 2>&1 || cargo install --path .

# Sous Windows (PowerShell) :
if (-not (Get-Command strmaid -ErrorAction SilentlyContinue)) { cargo install --path . }
```

Si le binaire n'est pas encore installé, l'installer via Cargo :
```bash
cargo install strmaid
# ou depuis les sources du dépôt :
cargo install --path /chemin/vers/strmaid
```

---

## 2. Intégration Native Model Context Protocol (MCP Server Mode)

`strmaid` implémente nativement un serveur **Model Context Protocol (MCP)** standardisé via standard I/O (JSON-RPC 2.0). C'est le mode d'intégration recommandé pour les agents d'IA (Antigravity `agy`, Claude Desktop, Cursor, Cline).

### Configuration Client MCP

Ajouter l'entrée dans le fichier de configuration des serveurs MCP (ex. `~/.gemini/antigravity-cli/mcp_config.json` ou configuration client équivalente) :

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

### Outils MCP Exposés

Le serveur fournit 3 outils spécialisés :

#### A. `strmaid_validate`
Valide la syntaxe Mermaid sans coût de rendu graphique.
- **Paramètres :**
  - `source` *(string, requis)* : Le code source du diagramme Mermaid.
- **Sortie structurée :**
  ```json
  {
    "valid": true,
    "error": null
  }
  ```
  En cas d'anomalie :
  ```json
  {
    "valid": false,
    "error": {
      "message": "Erreur syntaxe Mermaid: ligne 3: unexpected token: '???'",
      "line": 3,
      "kind": "SyntaxError"
    }
  }
  ```

#### B. `strmaid_render`
Génère le SVG vectoriel et le raster PNG encodé en Base64 avec les dimensions matricielles exactes.
- **Paramètres :**
  - `source` *(string, requis)* : Le code source du diagramme Mermaid.
  - `theme` *(string, optionnel)* : Thème visuel (`dark`, `light`, `neutral`, `amber`, `phosphor`, `neon`, `mono` - par défaut `dark`).
  - `width` *(integer, optionnel)* : Largeur cible en colonnes pour contraindre les dimensions.
- **Sortie structurée :**
  ```json
  {
    "svg": "<svg xmlns=...",
    "png_base64": "iVBORw0KGgoAAAANSUhEUgAA...",
    "dimensions": {
      "width": 640,
      "height": 320
    }
  }
  ```

#### C. `strmaid_detect`
Analyse un document Markdown complet pour isoler et recenser tous les blocs de diagrammes Mermaid avec leurs métadonnées.
- **Paramètres :**
  - `markdown` *(string, requis)* : Flux ou contenu Markdown complet.
- **Sortie structurée :**
  ```json
  {
    "diagram_count": 2,
    "blocks": [
      {
        "content": "flowchart TD\n  A --> B",
        "title": "Architecture Flux",
        "theme": "dark",
        "width": 80
      }
    ]
  }
  ```

---

## 3. Diagnostic de l'Environnement et des Capacités Terminal (`strmaid doctor`)

Avant de déclencher un rendu matriciel haute résolution ou de choisir un protocole graphique, l'agent peut auditer l'environnement hôte pour déterminer les capacités matérielles et logicielles du terminal :

```bash
# Diagnostic synthétique pour terminal :
strmaid doctor

# Diagnostic machine-readable JSON pour l'agent IA :
strmaid doctor --format json
```

### Exploitation du rapport JSON par l'agent
```json
{
  "detected_protocol": "halfblocks",
  "terminal": {
    "columns": 80,
    "rows": 24,
    "target_columns": 74
  },
  "environment": {
    "variables": {
      "COLORTERM": "truecolor",
      "KITTY_WINDOW_ID": null,
      "TERM": "xterm-256color",
      "TERM_PROGRAM": "iTerm.app",
      "WT_SESSION": null
    }
  },
  "pipeline": {
    "sample_diagram": "flowchart LR\n    A-->B",
    "svg_generated": true,
    "rasterized": true,
    "raster_dimensions": [248, 92],
    "error": null
  },
  "healthy": true
}
```
**Règle pour l'agent :**
- Vérifier `healthy == true` pour s'assurer du bon fonctionnement de la chaîne de rendu.
- Consulter `detected_protocol` pour adapter les instructions de rendu (priorité : `kitty` > `iterm2` > `halfblocks` > `asciibox`).

---

## 4. Procédure de Validation & Auto-Correction Syntaxique (Linter Mode)

Lorsqu'un agent IA écrit, modifie ou audite un diagramme Mermaid, il doit obligatoirement valider sa syntaxe avant de le présenter à l'utilisateur.

### Commande de validation
```bash
# Pour un fichier :
strmaid --block-only diagram.mmd

# Ou directement depuis une chaîne en mémoire :
printf '%s\n' "graph TD; A --> B" | strmaid --block-only
```

### Interprétation du code de retour shell
- **Code `0` (`ExitCode::SUCCESS`)** : La syntaxe Mermaid est rigoureusement valide et le rendu a abouti.
- **Code `1` (`ExitCode::FAILURE`)** : Une anomalie de syntaxe a été détectée.

### Boucle de Self-Healing (Auto-Correction)
En cas de code de sortie `1`, `strmaid` émet un diagnostic précis indiquant le numéro de ligne fautif et la nature de l'anomalie :
```text
⚠️  [Rendu Mermaid indisponible: Erreur syntaxe Mermaid: ligne 2: unexpected text: '...']
```
**Règle pour l'agent :**
1. Capturer le numéro de ligne rapporté par l'erreur.
2. Corriger le symbole, la liaison ou le nœud incriminé.
3. Ré-exécuter `strmaid --block-only` jusqu'à obtention du code de retour `0`.

---

## 5. Procédure d'Audit de Documents Markdown (Machine-Readable JSON & NDJSON)

### A. Rapport global du document (JSON à EOF)
Pour analyser la conformité globale de l'ensemble des diagrammes d'un document Markdown :
```bash
strmaid path/to/document.md --format json
```

Le document JSON retourné détaille quantitativement l'état de chaque bloc :
```json
{
  "version": "0.1.0",
  "format": "json",
  "summary": {
    "total_items": 5,
    "total_diagrams": 2,
    "valid_diagrams": 1,
    "invalid_diagrams": 1
  },
  "items": [
    {
      "type": "diagram",
      "index": 0,
      "valid": true,
      "dimensions": { "width": 640, "height": 320 },
      "protocol": "halfblocks"
    },
    {
      "type": "diagram",
      "index": 1,
      "valid": false,
      "error": {
        "message": "unexpected token",
        "line": 3,
        "kind": "SyntaxError"
      },
      "raw_content": "graph TD\n  A --> ???"
    }
  ]
}
```
L'agent filtre sur `valid == false` pour isoler les blocs problématiques et les corriger.

### B. Streaming continu ligne par ligne (NDJSON temps réel)
Idéal pour les flux de documentation volumineux ou les canaux d'agents en continu, sans bufferiser tout le document en mémoire :
```bash
cat flux.md | strmaid --format ndjson
```
Chaque bloc ou élément textuel est émis immédiatement sur `stdout` sous forme d'un objet JSON autonome sur une ligne distincte.

---

## 6. Procédure de Rendu Visuel Terminal

Pour afficher un diagramme sous forme visuelle dans le terminal sans bloquer l'invite de commande (mode non-interactif) :

### A. Protocoles graphiques supportés (`-g`, `--graphics`)
- `kitty` : Rendu natif GPU pixel-perfect pour Warp, Kitty, WezTerm, Ghostty.
- `iterm2` : Protocole d'affichage d'images pour iTerm2 et terminaux compatibles macOS.
- `halfblocks` : Repli universel demi-blocs Unicode (`▀`, `▄`) TrueColor 24-bit (compatible Windows Terminal, PowerShell, CMD, et tous terminaux Linux/macOS modernes).
- `asciibox` : Repli en art Unicode/Braille / ASCII box-drawing pour terminaux contraints, sessions SSH anciennes ou logs CI monochromes.
- `raw` : Sortie brute non encapsulée.

Exemples :
```bash
# Rendu demi-blocs TrueColor 24-bit calibré à 80 colonnes :
strmaid --block-only --width 80 diagram.mmd

# Rendu iTerm2 explicite :
strmaid --block-only --graphics iterm2 diagram.mmd

# Repli en art Unicode/Braille pour sessions SSH dégradées ou CI monochrome :
strmaid --block-only --width 60 --graphics asciibox diagram.mmd
```

### B. Adaptation aux thèmes visuels (`-t`, `--theme`)
Sélectionner la palette adéquate selon le contexte :
- `--theme dark` (défaut) : Contraste optimisé pour terminaux à fond sombre.
- `--theme light` : Palette adaptée aux fonds clairs ou blancs.
- `--theme neutral` : Nuances de gris et tons neutres équilibrés.
- `--theme amber` : Ambre monochrome chaud (esthétique CRT VT220).
- `--theme phosphor` : Vert monochrome phosphorescent (esthétique CRT P1).
- `--theme neon` : Cyberpunk haute saturation (cyan et magenta).
- `--theme mono` : Noir et blanc à contraste 100% absolu (écrans e-ink ou logs stricts).

---

## 7. Streaming Markdown et Interception Interactive PTY

### A. Pipeline de streaming direct
Pour consommer et afficher un flux Markdown en temps réel émis par un processus tiers ou un LLM dans un pipeline Unix :
```bash
commande_source | strmaid --no-pager
```

### B. Intercepteur interactif en pseudo-terminal (`strmaid run`)
Pour exécuter une session interactive (ex. CLI d'un agent IA, REPL) au sein d'un pseudo-terminal (PTY) tout en interceptant automatiquement les diagrammes Mermaid au vol pour les afficher directement en graphisme terminal :
```bash
# Exécute un CLI tiers dans un PTY avec rendu inline des diagrammes Mermaid :
strmaid run my_ai_agent_cli
```
L'application cible conserve toutes ses capacités TTY (raw mode, gestion des touches, couleurs ANSI) tandis que les blocs ```` ```mermaid ```` sont détectés, rendus graphiquement et réinjectés dans l'affichage terminal.
