# Adoption d'un Moteur de Rendu Mermaid Natif en Rust

Pour parser et rendre les diagrammes Mermaid en streaming sans introduire de latence perceptible, nous adoptons un moteur de rendu pur Rust (`mermaid-svg`) couplé à `resvg`, plutôt qu'un runtime V8 (`deno_core`) ou headless Chromium. Cette décision élimine l'overhead de démarrage (2 à 6 ms vs 2,5 s), supprime toute dépendance C++/JavaScript et garantit un binaire statique ultra-léger pour le streaming CLI.

## Considered Options

- **Runtime V8 embarqué (`deno_core` + `mermaid.min.js`) :** Support exhaustif de Mermaid.js officiel mais temps de cold-start prohibitif (1,5 à 3 s), binaire lourd (> 50 Mo) et compilation complexe.
- **Processus Headless Chromium (`mermaid-cli`) :** Rejeté pour surconsommation mémoire et inadaptation totale à un filtre Unix temps réel.
- **Moteur Rust Pur (`mermaid-svg` + `resvg`) :** Retenu pour sa vitesse d'exécution instantanée (< 10 ms), son absence de dépendance externe et sa robustesse mémoire.

## Consequences

Les diagrammes standards (flowcharts, sequence, class, state, er, gitGraph, etc.) sont compilés instantanément. Les syntaxes expérimentales non supportées bénéficient d'un mécanisme de repli gracieusement dégradé en bloc de code brut.
