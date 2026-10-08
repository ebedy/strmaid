# Strmaid — Streaming Markdown & Terminal Mermaid Renderer

Outil CLI unifié et multiplateforme (Windows, Linux, macOS) de parsing de flux Markdown et de rendu terminal pixel-perfect de diagrammes Mermaid. Conçu pour visualiser avec une fidélité graphique maximale les graphes générés par des agents IA ou des utilisateurs humains, aussi bien en streaming continu (pipe Unix, ConPTY, PTY) qu'en mode statique unitaire (fichiers, visualiseur d'éditeur `--block-only`, serveur MCP).

## Language

**DiagramBlock**:
Un segment de texte délimité par une sentinelle d'ouverture (````mermaid`) et une sentinelle de fermeture (````), contenant une spécification formelle de diagramme.
_Avoid_: Mermaid chunk, raw script, code snippet

**GraphicsProtocol**:
Le protocole de transport et d'affichage matriciel ou textuel supporté par le terminal (Kitty Graphics Protocol, Iterm2InlineProtocol, HalfBlocksTrueColor ou AsciiBox en repli ultime).
_Avoid_: Terminal blitter, screen driver, display format

**ExecutionMode**:
Le mode opérationnel sélectionné selon la nature de l'entrée/sortie : StreamFilter pour un pipe Unix/Windows composable, LiveStreamingPager pour un TTY interactif, ou BlockOnly pour un rendu direct d'un diagramme isolé.
_Avoid_: Run mode, interactive toggle, display switch

**RasterizedPixmap**:
Un buffer mémoire de pixels RGBA bruts produit par le moteur vectoriel resvg à partir d'un document SVG valide.
_Avoid_: Image buffer, bitmap array, pixel cache

**GracefulFallback**:
La stratégie de repli affichant le bloc en texte brut formaté avec avertissement discret lorsqu'une erreur de syntaxe Mermaid survient, sans interrompre le flux.
_Avoid_: Crash recovery, silent skip, error ignore

**ViewportGeometry**:
Les dimensions matricielles (colonnes, lignes, pixels calculés, largeur Unicode CJK) délimitant la surface disponible du terminal pour le dimensionnement proportionnel des graphismes.
_Avoid_: Screen size, window metrics

**StructuredMachineOutput**:
Format de sérialisation JSON/NDJSON pour la communication inter-processus et l'ingestion par les agents IA.
_Avoid_: Raw JSON dump, debug print

## Décisions d'Architecture Fondatrices

Les choix d'architecture historiques du projet répondent à des contraintes strictes de performance, de portabilité et de robustesse en streaming :

### 1. Moteur de Rendu Natif Pur Rust (`mermaid-svg` + `resvg`)
- **Choix :** Génération vectorielle SVG via `mermaid-svg` et rasterisation matricielle via `resvg`, le tout compilé statiquement en Rust pur.
- **Alternatives rejetées :**
  - *Runtime V8 embarqué (`deno_core` + `mermaid.min.js`)* : Rejeté pour temps de démarrage prohibitif (1,5 à 3 s vs 2 à 6 ms en Rust natif) et binaire lourd (> 50 Mo).
  - *Headless Chromium (`mermaid-cli`)* : Rejeté pour surconsommation mémoire et inadaptation totale au streaming Unix temps réel.
- **Conséquences :** Binaire statique autonome sans dépendance C++/JavaScript/Node.js, latence de compilation sub-10ms. Repli gracieux en bloc de code brut pour les syntaxes non prises en charge.

### 2. Protocoles Graphiques Terminal & Hiérarchie de Repli Dégradé
- **Choix :** Kitty Graphics Protocol comme canal principal haute fidélité, repli automatique sur demi-blocs Unicode (`▀`, `▄`) ANSI TrueColor 24-bit, et repli ultime textuel `AsciiBox` pour environnements dégradés.
- **Alternatives rejetées :**
  - *Support Sixel exclusif* : Rejeté car Warp sous Linux ne prend pas en charge Sixel.
  - *Kitty Graphics exclusif* : Rejeté en raison du risque de séquences d'échappement corrompues sur les émulateurs incompatibles.
- **Conséquences :** Détection dynamique des capacités du terminal, compatibilité universelle (Linux, macOS, Windows) et sélection manuelle possible via `--graphics`.

### 3. Pipeline de Streaming Découplé & Dual-Mode d'Exécution
- **Choix :** Machine à états à deux phases (émission immédiate du Markdown, accumulation isolée des `DiagramBlock`). Sélection automatique du mode : `ExecutionMode::LiveStreamingPager` (`ratatui`) sur TTY interactif, `ExecutionMode::StreamFilter` composable sur pipe Unix.
- **Alternatives rejetées :**
  - *Attente EOF globale* : Rejeté car cela bloque l'affichage et brise le streaming interactif des agents IA.
  - *Filtre Unix strict sans TUI* : Rejeté car il interdit l'inspection, le défilement et le scroll rétroactif sur grand document.
- **Conséquences :** Découplage strict entre la transformation du flux et la couche d'affichage.

### 4. Dimensionnement Dynamique au Viewport & Thématisation Adaptative
- **Choix :** Calcul dynamique de la géométrie du terminal (`crossterm::terminal::size()`), préservation du ratio d'aspect vectoriel, largeur bornée à 90% des colonnes utiles. Thème sombre à fond transparent par défaut.
- **Alternatives rejetées :**
  - *Dimensions SVG statiques fixes* : Rejeté car source de troncature sur petits écrans et de sous-dimensionnement sur grands affichages.
  - *Interrogation dynamique de luminosité via OSC 11* : Rejeté pour risque de latence réseau/PTY et blocages sur `stdin` combinés.
- **Conséquences :** Intégration visuelle fluide dans les terminaux sombres modernes, ajustable via les options `--theme` et `--width`.

### 5. Stratégie de Nommage `strmaid` & Feuille de Route
- **Choix :** Nommage `strmaid` (**Str**eam / **St**reaming + Mer**maid**) pour lever la collision sur Crates.io (`termaid` déjà réservé) et refléter la mission première : streaming continu Markdown + Mermaid pour pipelines d'agents IA et développeurs.
- **Conséquences :** Intégration de la sortie structurée `--format json` / NDJSON, protocoles de repli étendus (`AsciiBox`), prise en compte CJK/Unicode-width et mode d'intégration éditeur (`--block-only`).

### 6. Neutralisation des Séquences Terminales Non Fiables
- **Choix :** tout contenu non fiable réémis vers le terminal passe par `sanitize_terminal_text` (`src/domain.rs`), qui retire les séquences CSI, OSC, DCS, APC, PM, SOS et les contrôles C0/C1 hors `\n` et `\t`. Cela couvre le contenu et le message d'erreur d'un `GracefulFallback`, l'avertissement de repli `AsciiBox`, les erreurs `CliError` affichées et les lignes du pager. Le texte Markdown hors diagramme en `ExecutionMode::StreamFilter` passe par `sanitize_passthrough_text`, qui conserve uniquement les séquences de style SGR (`ESC [ … m`) ; `--raw-passthrough` rétablit le relais brut.
- **Alternatives rejetées :**
  - *Relais brut de type `cat`* : rejeté car les flux traités proviennent d'agents IA non fiables ; OSC 52 écrit le presse-papiers, OSC 0-2 réécrit le titre, APC injecte des commandes Kitty.
  - *Filtrage limité au pager* : rejeté car le mode filtre est le chemin par défaut des pipes.
- **Conséquences :** `strmaid run` reste transparent dans cette version : les applications plein écran relayées (vim, htop) dépendent des séquences CSI de positionnement. Sa neutralisation sélective (OSC, DCS, APC) fera l'objet d'une décision distincte.

### 7. Quota par Diagramme sans Interruption du Flux
- **Choix :** un `DiagramBlock` dont le contenu dépasse `ResourceLimits::max_diagram_bytes` (1 Mio) est drainé jusqu'à sa clôture sans accumulation (`StreamStateMachine`, état `SkippingOversizedDiagram`) puis signalé par `StreamItem::OversizedDiagram`, rendu en `GracefulFallback` sans reproduction du contenu (JSON : `valid: false`, `kind: "ResourceLimit"`). Les lignes non UTF-8 sont décodées avec remplacement (`LossyLines`). `strmaid run` applique le même quota à sa capture, restitue le bloc brut en cas de dépassement et lit le PTY via un canal borné.
- **Alternatives rejetées :**
  - *Erreur fatale sur dépassement* : rejeté car un seul bloc hostile ou erroné faisait perdre le reste du document, contrairement à l'exigence de streaming robuste.
  - *Restitution du contenu du bloc ignoré* : rejeté car ce contenu est par construction trop volumineux pour être réémis sans réintroduire le coût mémoire évité.
- **Conséquences :** `StreamStateMachine::process_line` ne renvoie plus d'erreur (`Option<StreamItem>`). Une ligne unique sans `\n` reste chargée en entier : la borne porte sur les blocs, pas sur la longueur d'une ligne de texte.
