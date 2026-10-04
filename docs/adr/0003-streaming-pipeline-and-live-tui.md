# Pipeline de Streaming Découplé et Live Streaming Pager

Pour maintenir l'illusion de réactivité immédiate des agents IA générant du texte en continu, le binaire opère selon une machine à états à deux phases : affichage immédiat du texte Markdown et accumulation isolée des blocs Mermaid jusqu'à réception de la sentinelle fermante. Lorsque la sortie standard est connectée à un TTY interactif, le binaire instancie un Live Streaming Pager plein écran (`ratatui`) avec auto-scroll en temps réel et navigation au clavier ; en présence d'un pipe de redirection, il agit comme un filtre Unix composable inline.

## Considered Options

- **Attente EOF globale :** Rejeté car cela brise l'expérience de streaming de l'agent en bloquant l'affichage pendant toute la génération.
- **Filtre Unix strict sans TUI :** Rejeté car il ne permet pas l'inspection, le scroll rétroactif et la navigation ergonomique dans les longs documents complexes.
- **Dual-mode dynamique (Live TUI sur TTY, Filtre Unix sur Pipe) :** Retenu pour offrir la flexibilité des outils Unix et le confort d'un dashboard terminal interactif.

## Consequences

Le code isole la logique de transformation du flux d'entrée de la couche d'affichage. Le mode filtre écrit directement sur `stdout` sans altérer l'écran, tandis que le mode Pager exploite le double buffering d'écran de Ratatui.
