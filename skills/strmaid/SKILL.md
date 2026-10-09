---
name: strmaid
description: Native terminal rendering, syntax validation, MCP server, and structured analysis for Mermaid diagrams in streaming Markdown. Use whenever generating, inspecting, validating, or fixing Mermaid code, running the Strmaid MCP server, diagnosing terminal graphics capabilities, managing render engines, or previewing Markdown flows.
license: MIT OR Apache-2.0
compatibility: Linux, macOS, or Windows (requires strmaid binary in PATH)
metadata:
  version: "0.5.1"
  repository: https://github.com/ebedy/strmaid
---

# Strmaid Agent Skill

Ce skill fournit les procédures et directives permettant à un agent IA de piloter le moteur natif **`strmaid`** pour :
1. **Intégrer le serveur MCP natif (`strmaid mcp`)** pour le rendu vectoriel/matriciel (SVG, PNG Base64), la validation syntaxique et l'extraction de diagrammes via JSON-RPC 2.0.
2. **Diagnostiquer les capacités du terminal hôte (`strmaid doctor`)** pour auditer l'environnement, le protocole graphique optimal et la géométrie avant tout affichage.
3. **Valider et auto-corriger** la syntaxe de diagrammes Mermaid sans dépendance JavaScript ni navigateur headless (boucle de self-healing ultra-rapide < 10 ms).
4. **Piloter les moteurs de génération vectorielle (`--engine`)** en choisissant le moteur adapté (`mermaid-svg` par défaut, `merman` avec disposition ELK pour diagrammes complexes).
5. **Garantir la résilience et l'adaptation au terminal** via l'auto-orientation adaptative (`--no-auto-orient`), les gardes temporelles (`--timeout-ms`), le repli intelligent vers `AsciiBox` (`--no-fallback-asciibox`) et la neutralisation des séquences terminales non fiables (`--raw-passthrough` pour la désactiver).
6. **Auditer la conformité** de documents Markdown contenant plusieurs blocs Mermaid via une sortie structurée machine-readable JSON ou NDJSON (streaming en continu).
7. **Prévisualiser des diagrammes** directement dans la console ou dans les terminaux intégrés d'éditeurs (Neovim, VSCode, Helix) avec ajustement adaptatif au viewport et gestion de 7 thèmes visuels.
8. **Intercepter les flux interactifs en pseudo-terminal (`strmaid run <cmd...>`)** pour visualiser dynamiquement les diagrammes générés par des CLI d'agents (ex. Antigravity `agy`, Claude Code) ou des scripts.

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

`strmaid` implémente nativement un serveur **Model Context Protocol (MCP)** standardisé via standard I/O (JSON-RPC 2.0). C'est le mode d'intégration recommandé pour les agents d'IA (Antigravity `agy`, Claude Desktop, Cursor, Windsurf, Cline).

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

Le serveur fournit 3 outils spécialisés.

**Format des résultats (MCP 2025-06-18) :** les « sorties structurées » décrites ci-dessous sont renvoyées dans `result.structuredContent` ; le même JSON est sérialisé dans `result.content[0].text` pour les clients antérieurs. Aucun champ n'est recopié à la racine de `result`. Le serveur négocie `protocolVersion` : il renvoie la version demandée si elle est supportée (`2025-06-18`, `2024-11-05`), sinon `2025-06-18`. Un message sans membre `id` est une notification et ne reçoit jamais de réponse.

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
  - `width` *(integer, optionnel)* : Largeur cible en colonnes pour contraindre les dimensions (clampé automatiquement entre 20 et 1000 colonnes, défaut 80).
  - `engine` *(string, optionnel)* : Moteur de rendu (`mermaid-svg` par défaut, `merman` si le binaire est compilé avec la feature `merman`).
  - `timeout_ms` *(integer, optionnel)* : Délai maximal du rendu complet (SVG, rasterisation, PNG), borné entre 100 et 30 000 ms ; défaut 5 000 ms, également appliqué pour `0`.
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

#### Limites du serveur MCP
- Chaque message JSON-RPC est limité à 2 Mio : un message plus long est écarté sans être chargé et reçoit une erreur `-32700` (`id: null`) ; le message suivant est traité normalement.
- `source` est limitée à 1 Mio pour `strmaid_validate` et `strmaid_render` : au-delà, le résultat porte `isError: true`.
- `strmaid_validate` s'exécute sous un délai de 5 000 ms. Une expiration renvoie `isError: true` (« Validation Mermaid interrompue ») et non `valid: false` : **ne pas tenter de corriger la syntaxe dans ce cas**, simplifier ou découper le diagramme.
- Au plus 4 rendus expirés peuvent continuer à s'exécuter en arrière-plan ; au-delà, les nouveaux rendus sont refusés immédiatement (« trop de rendus abandonnés encore en cours ») jusqu'à leur terminaison. Espacer alors les requêtes lourdes.

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
    "error": null,
    "font_faces": 2369
  },
  "healthy": true
}
```
**Règle pour l'agent :**
- Vérifier `healthy == true` pour s'assurer du bon fonctionnement de la chaîne de rendu.
- Consulter `detected_protocol` pour adapter les instructions de rendu (priorité : `kitty` > `iterm2` > `halfblocks` > `asciibox`).
- Si `pipeline.font_faces == 0`, aucune police système n'est disponible : les rendus graphiques basculent automatiquement en `AsciiBox` (avertissement `Polices indisponibles`). Recommander l'installation d'un paquet de polices (`fonts-dejavu`, `fonts-noto`) ou demander directement `--graphics asciibox`.

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
Le bloc restitué après l'avertissement est neutralisé (aucune séquence de contrôle terminal) et sa clôture Markdown peut compter plus de trois backticks lorsque le contenu en contient lui-même : extraire le contenu entre les clôtures de même longueur, pas entre deux `` ``` `` fixes.

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

Le document JSON retourné correspond à la structure `JsonDocumentOutput` et détaille quantitativement l'état de chaque bloc :
```json
{
  "version": "0.5.1",
  "format": "json",
  "theme": "dark",
  "protocol": "halfblocks",
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
      "title": "Architecture Principale",
      "dimensions": { "width": 640, "height": 320 },
      "protocol": "halfblocks",
      "payload": "\u001b[38;2;...▀...",
      "raw_content": "flowchart TD\n  A --> B"
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

Un bloc dont le contenu dépasse 1 Mio n'est ni chargé ni rendu : il apparaît comme un diagramme invalide de type `ResourceLimit`, sans `raw_content`, et le reste du document est analysé normalement :
```json
{
  "type": "diagram",
  "index": 2,
  "valid": false,
  "error": {
    "message": "Diagramme Mermaid ignoré : 1377797 octets dépassent la limite de 1048576 octets",
    "kind": "ResourceLimit"
  },
  "raw_content": ""
}
```
**Règle pour l'agent :** un `kind == "ResourceLimit"` ne se corrige pas syntaxiquement : découper le diagramme en plusieurs blocs plus petits.

### B. Streaming continu ligne par ligne (NDJSON temps réel)
Idéal pour les flux de documentation volumineux ou les canaux d'agents en continu, sans bufferiser tout le document en mémoire :
```bash
cat flux.md | strmaid --format ndjson
```
Chaque bloc ou élément textuel est émis immédiatement sur `stdout` sous forme d'un objet JSON autonome sur une ligne distincte.

---

## 6. Moteurs de Rendu, Résilience & Affichage Terminal

### A. Choix du Moteur de Rendu (`--engine`)
`strmaid` découple l'interface de génération vectorielle via le trait `DiagramEngine` :
- `--engine mermaid-svg` (défaut) : Moteur natif Rust ultra-léger et rapide (< 10 ms), sans runtime JS.
- `--engine merman` : Moteur alternatif s'appuyant sur `merman` (layout ELK, standard Zed), recommandé pour les diagrammes d'architecture très denses ou imbriqués (nécessite la compilation avec `--features merman`).

```bash
# Rendu avec le moteur Merman :
strmaid --engine merman architecture.md
```

### B. Auto-Orientation Adaptative (`--no-auto-orient`)
Pour éviter que les diagrammes horizontaux (`flowchart LR`, `graph LR`, `RL`) ne soient tronqués ou compressés horizontalement sur des terminaux étroits, `strmaid` inspecte la géométrie disponible :
- Si la largeur effective est **inférieure à 120 colonnes**, l'orientation est automatiquement convertie en **`TD`** (Top-Down).
- Pour désactiver ce comportement et forcer l'orientation d'origine :
  ```bash
  strmaid --no-auto-orient diagram.md
  ```

### C. Garde Temporelle Anti-Blocage (`--timeout-ms`)
Afin d'éviter tout gel indéfini du terminal ou du pipeline de streaming face à des diagrammes pathologiques ou des boucles récursives de layout :
- Par défaut, un timeout strict de **5000 ms** est appliqué par diagramme.
- Ce délai est configurable ou désactivable (via `0`) :
  ```bash
  strmaid --timeout-ms 2000 flux.md
  ```

### D. Repli Résilient Intelligent vers AsciiBox (`--no-fallback-asciibox`)
Lorsqu'un rendu matriciel haute fidélité échoue en cours de route (ex. dépassement de la limite de mémoire de rasterisation ou contraintes mémoire extrêmes) :
- `strmaid` ne crashe jamais : il bascule automatiquement sur un repli sémantique **`AsciiBox`** (art Unicode / Box-Drawing) en émettant un avertissement explicite.
- Pour interdire ce repli et restituer le code brut en cas d'échec graphique :
  ```bash
  strmaid --no-fallback-asciibox diagram.md
  ```

### E. Entrées Hostiles : Quota par Diagramme et Octets Non UTF-8
- Un bloc Mermaid dépassant 1 Mio est drainé sans accumulation mémoire puis signalé par `⚠️  [Diagramme Mermaid ignoré : N octets dépassent la limite de 1048576 octets]` ; le texte qui suit est traité normalement (code de sortie `0`). Le mode `--block-only` refuse en revanche une entrée trop volumineuse avec le code `1`.
- Les octets non UTF-8 sont remplacés par `U+FFFD` ; un avertissement unique est émis sur `stderr` (filtre et serveur MCP), le flux continue.

### F. Neutralisation des Séquences Terminales (`--raw-passthrough`)
- Le contenu des blocs en repli, les messages d'erreur et toutes les lignes du pager sont débarrassés des séquences de contrôle (OSC, DCS, APC, PM, SOS, CSI) et des caractères de contrôle C0/C1 hors `\n` et `\t`.
- En mode filtre, le texte Markdown hors diagramme conserve uniquement ses couleurs SGR (`ESC [ … m`) : OSC 52 (presse-papiers), changement de titre et images Kitty injectées sont supprimés.
- `--raw-passthrough` rétablit le relais brut du texte Markdown (mode filtre uniquement), à réserver aux sources de confiance.

### G. Protocoles Graphiques Supportés (`-g`, `--graphics`)
- `kitty` : Rendu natif GPU pixel-perfect pour Warp, Kitty, WezTerm, Ghostty.
- `iterm2` : Protocole d'affichage d'images pour iTerm2 et terminaux compatibles macOS.
- `halfblocks` : Repli universel demi-blocs Unicode (`▀`, `▄`) TrueColor 24-bit (compatible Windows Terminal, PowerShell, CMD, et tous terminaux Linux/macOS modernes).
- Sous tmux (`TMUX`) ou GNU screen (`STY`), la détection automatique n'utilise jamais `kitty` ni `iterm2`, filtrés par le multiplexeur : elle se replie sur `halfblocks` (ou `asciibox` sans TrueColor). Forcer `-g kitty` reste possible si le passthrough tmux est configuré (`allow-passthrough on`). `strmaid doctor` affiche `TMUX` et `STY`.
- Un échec d'encodage de l'image iTerm2 déclenche le repli `AsciiBox` au lieu d'une image vide.
- `asciibox` : Repli sémantique en art Unicode / ASCII box-drawing pour terminaux contraints, sessions SSH anciennes ou logs CI monochromes.
- `raw` : Sortie brute vectorielle SVG sans conversion matricielle.

```bash
# Rendu demi-blocs TrueColor 24-bit calibré à 80 colonnes (--width accepte 10 à 1000) :
strmaid --block-only --width 80 diagram.mmd

# Repli en art Unicode box-drawing pour sessions SSH dégradées ou CI monochrome :
strmaid --block-only --width 60 --graphics asciibox diagram.mmd
```

- Dans le pager interactif (`--pager`), les diagrammes sont toujours tracés en `AsciiBox` monochrome, seul rendu représentable par `ratatui` ; utiliser `--no-pager` pour un rendu graphique.
- `strmaid` sans fichier alors que stdin est un terminal refuse d'ouvrir le pager (code `2`, « aucune entrée ») : un agent doit toujours fournir un fichier ou un flux (`cat doc.md | strmaid --no-pager`). Les erreurs d'usage de la ligne de commande retournent le code `2`, les autres erreurs le code `1`.

### H. Thèmes Visuels (`-t`, `--theme`)
Sélectionner la palette adéquate selon le contexte :
- `--theme dark` (défaut) : Contraste optimisé pour terminaux à fond sombre.
- `--theme light` : Palette adaptée aux fonds clairs ou blancs.
- `--theme neutral` : Nuances de gris et tons neutres équilibrés.
- `--theme amber` : Ambre monochrome chaud (esthétique CRT VT220).
- `--theme phosphor` : Vert monochrome phosphorescent (esthétique CRT P1).
- `--theme neon` : Cyberpunk haute saturation (cyan et magenta).
- `--theme mono` : Noir et blanc à contraste 100% absolu (écrans e-ink ou logs stricts).

---

## 7. Directives de Modélisation Mermaid Terminal-Safe pour Agents IA

Lorsqu'un agent IA conçoit un diagramme Mermaid destiné à être restitué dans un terminal ou via `strmaid`, il doit obligatoirement appliquer les règles suivantes pour garantir une lisibilité optimale et éviter tout dépassement :

1. **Plafond strict de largeur unitaire :**
   - Interdiction formelle des lignes de libellé de nœud excédant 30 caractères.
   - Utiliser systématiquement des sauts de ligne explicites `<br/>` pour compacter verticalement le texte des nœuds :
     ```mermaid
     flowchart TD
         A["Analyse initiale<br/>du flux entrant"] --> B["Génération SVG<br/>vectorielle"]
     ```
2. **Orientation verticale privilégiée :**
   - Déclarer préférentiellement `flowchart TD` ou `graph TD` pour les chaînes de travail afin de minimiser l'étalement horizontal sur les terminaux fenêtrés.
3. **Calibrage pour 80 colonnes :**
   - L'encombrement horizontal global calculé du graphe ne doit jamais dépasser 80 colonnes pour éviter tout déclenchement de repli dégradé en bloc de code brut.
4. **Sobriété et concision sémantique :**
   - Les nœuds ne doivent contenir que l'intitulé fonctionnel majeur et son composant technique. Les détails d'implémentation fins doivent figurer dans le texte Markdown environnant.
5. **Immunité contre l'opérateur de chaînage multiple (`&`) :**
   - En syntaxe Mermaid (`flowchart`), le caractère `&` entouré d'espaces est un opérateur structurel de parallélisme (`A & B --> C`). S'il est inséré au sein d'un libellé, certains parsers fragmentent le nœud en morceaux disjoints avec injection de délimiteurs de syntaxe (`D["`, `"]`).
   - **Règle absolue :** Ne jamais utiliser l'esperluette brute `&` dans les libellés de diagrammes Mermaid. Préférer systématiquement la conjonction naturelle « et », le signe « + », ou s'appuyer sur la substitution automatique par le caractère Unicode sécurisé `﹠` (U+FE60) opérée par `strmaid`.

---

## 8. Streaming Markdown et Interception Interactive PTY

### A. Pipeline de streaming direct
Pour consommer et afficher un flux Markdown en temps réel émis par un processus tiers ou un LLM dans un pipeline Unix :
```bash
commande_source | strmaid --no-pager
```

### Grammaire des blocs reconnus
- Ouverture : au moins trois `` ` `` ou `~` identiques suivis du langage `mermaid` ou `mermaidjs` (attributs `title`, `theme`, `width` optionnels) ; l'indentation est tolérée (blocs dans une liste).
- Fermeture : même caractère, longueur au moins égale à l'ouverture, aucune info-string.
- **Règle pour l'agent :** pour montrer un exemple de syntaxe Mermaid **sans** le faire rendre, l'entourer d'un bloc d'un autre langage plus long, par exemple ```` ````markdown ```` ; pour inclure une ligne ```` ``` ```` dans un diagramme, ouvrir le bloc avec quatre backticks.

### B. Intercepteur interactif en pseudo-terminal (`strmaid run`)
Pour exécuter une session interactive (ex. CLI d'un agent IA, REPL) au sein d'un pseudo-terminal (PTY Unix ou ConPTY Windows) tout en interceptant automatiquement les diagrammes Mermaid au vol pour les afficher directement en graphisme terminal :
```bash
# Exécute un CLI d'agent (ex. Antigravity agy) dans un PTY avec rendu inline des diagrammes Mermaid :
strmaid run agy

# Reprise d'une conversation spécifique avec agy :
strmaid run agy --dangerously-skip-permissions --conversation=<CONVERSATION_ID>

# Exécution de tout autre CLI tiers ou script Python :
strmaid run python3 mon_agent.py
```
L'application cible conserve toutes ses capacités TTY (raw mode, gestion des touches, couleurs ANSI) tandis que les blocs ```` ```mermaid ```` sont détectés, rendus graphiquement et réinjectés dans l'affichage terminal.

**Règles pour l'agent :**
- Le code de sortie de `strmaid run` est celui de la commande enfant (borné à 255) : l'interpréter comme tel pour décider d'un succès ou d'un échec.
- Les redimensionnements du terminal hôte sont transmis à la commande enfant.
- Les retours chariot isolés (barres de progression) et les caractères UTF-8 multi-octets sont restitués fidèlement.
- La sortie de l'enfant n'est pas filtrée (préservation des applications plein écran) : ne lancer via `run` que des commandes de confiance.
- Un bloc ```` ```mermaid ```` jamais refermé ou dépassant 1 Mio n'est plus retenu indéfiniment : il est restitué tel quel (texte brut) et l'interception reprend ; la mémoire de `strmaid run` reste constante quel que soit le volume émis par l'enfant.
