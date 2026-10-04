# Rapport d'Architecture IA

Voici un exemple de documentation technique généré par un agent.

## Flux de Traitement

```mermaid
graph TD
    Client[Agent CLI] -->|Markdown Stream| Parser[strmaid]
    Parser -->|AST & Slices| Engine{Type de Bloc}
    Engine -->|Texte| Terminal[Sortie Standard]
    Engine -->|Mermaid| Renderer[Moteur Natif Rust]
    Renderer -->|SVG| Resvg[Rasterizer resvg]
    Resvg -->|Pixmap RGBA| Protocol[Kitty / HalfBlocks]
    Protocol --> Terminal
```

## Diagramme de Séquence

```mermaid
sequenceDiagram
    participant User as Développeur
    participant Agent as Agent IA
    participant Tool as strmaid
    User->>Agent: Prompt avec demande de schéma
    Agent->>Tool: Stream Markdown + ```mermaid
    Tool->>User: Rendu graphique haute fidélité
```

## Cas Dégradé (Syntaxe Invalide)

```mermaid
flowchart TD
    Detection[Bloc Mermaid detecte] --> Validation{Syntaxe valide ?}
    Validation -->|Oui| Render[Rendu graphique]
    Validation -->|Non| Fallback[Afficher le bloc source]
    Fallback --> Log[Signaler l'erreur]
```

Fin du document.
