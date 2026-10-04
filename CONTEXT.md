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


