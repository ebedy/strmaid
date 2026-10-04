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

