# Guide de Contribution à Strmaid

Merci de vous intéresser à l'amélioration de **Strmaid** ! Ce projet a vocation à être une référence open source d'excellence pour le rendu de diagrammes et le streaming Markdown en terminal.

Pour garantir la pérennité, la robustesse et la sécurité du système, nous appliquons des standards de développement stricts.

---

## 1. Principes d'Ingénierie & Standards de Code

Tout code soumis au projet doit respecter ces impératifs fondamentaux :

1. **Sûreté Trans-Langage & Rigueur Rust :**
   - **Zéro `unwrap()` ou `expect()` en production :** Toute opération faillible doit renvoyer un `Result<T, E>`.
   - **Zéro `panic!()` non justifié :** Interdiction des sentinelles ou des sorties abruptes non contrôlées.
   - **Lints Clippy bloquants :** Votre code doit passer sans aucun avertissement avec :
     ```bash
     cargo clippy --all-targets -- -D warnings
     ```
2. **Conception Dirigée par le Domaine (DDD) :**
   - Respectez scrupuleusement le glossaire unifié consigné dans [`CONTEXT.md`](CONTEXT.md).
   - Toute décision architecturale majeure ou difficilement réversible doit faire l'objet d'un nouvel ADR dans `docs/adr/`.
3. **Complexité ultra-basse & Clean Code :**
   - Fonctions courtes, linéaires et à responsabilité unique (SRP).
   - Inversion des conditions et sorties précoce (*guard clauses*) pour éviter l'imbrication (`nesting <= 2`).
   - Immutabilité par défaut et fonctions pures sans effets de bord masqués.

---

## 2. Démarrage & Environnement de Développement

### Prérequis
- Toolchain Rust (version 1.85+ ou édition 2024 recommandée).
- Terminal moderne supportant les séquences ANSI / TrueColor (Warp, Kitty, WezTerm, Alacritty).

### Commandes usuelles
```bash
# Vérification rapide de la compilation
cargo check

# Lancement des tests unitaires et d'intégration
cargo test

# Contrôle strict du linter
cargo clippy --all-targets -- -D warnings

# Formatage automatique
cargo fmt --check
```

---

## 3. Protocole de Pull Request (PR)

1. **Créer une branche thématique :**
   ```bash
   git checkout -b feat/nom-de-fonctionnalite
   # ou
   git checkout -b fix/nom-de-l-anomalie
   ```
2. **Développer selon le cycle TDD :**
   - Écrivez le test prouvant le comportement attendu ou reproduisant le bug.
   - Implémentez le strict nécessaire (*minimal diff*).
   - Validez que tous les tests passent.
3. **Messages de commit (Standard Conventional Commits) :**
   Rédigez vos messages en suivant le format `type(scope): description`.
   - Types autorisés : `feat`, `fix`, `perf`, `refactor`, `test`, `chore`, `docs`.
   - Exemples :
     - `feat(kitty): optimiser la compression base64 des blocs de pixmap`
     - `fix(parser): gérer correctement les sentinelles fermantes imbriquées`
4. **Vérification pré-soumission :**
   Avant d'ouvrir votre PR, assurez-vous que :
   ```bash
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   ```

---

## 4. Signalement de Bogues & Propositions

- Pour rapporter un bug : Utilisez le template GitHub Bug Report en fournissant un snippet Markdown de reproduction et les métadonnées de votre terminal (`TERM`, `TERM_PROGRAM`).
- Pour proposer une évolution : Utilisez le template Feature Request en expliquant le cas d'usage concret et l'impact architectural.

Merci pour votre contribution à l'écosystème open source !
