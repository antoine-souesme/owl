# owl

Outil en ligne de commande interactif qui liste les pull requests de mon compte
GitHub dans le terminal et permet de les fusionner selon la règle du dépôt.

Le résumé du projet est dans `DESCRIPTION.md`.

## Commandes

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo run              # lance la TUI
```

Les quatre premières doivent passer avant de considérer un travail terminé. Aucune
n'est optionnelle.

L'app de la barre des menus vit dans `macos/` :

```bash
cd macos && swift build --build-system native
macos/test.sh          # swift test, avec le framework de test des outils en ligne de commande
macos/build.sh         # construit owl et l'app, installe et relance ~/Applications/Owl.app
```

Les deux premières doivent passer dès qu'un fichier de `macos/` ou de
`src/cli/` change. `OWL_UPDATE_FIXTURES=1 cargo test` régénère les exemples
JSON lus par les tests de l'app. Sur ce Mac, sans Xcode, le système de
compilation par défaut de Swift échoue : d'où `--build-system native`.

## Règles d'architecture

Les dépendances entre modules sont à sens unique et strictes :

- `model` et `filter` ne dépendent ni du réseau ni du terminal.
- `github` dépend de `model` et `filter`, jamais de `app` ni `ui`.
- `app` ne fait aucun appel réseau : il reçoit des événements et renvoie des
  commandes que la boucle principale exécute.
- `ui` lit `app` et dessine. Une fonction de dessin ne modifie jamais l'état et ne
  prend aucune décision.
- `cli` dépend de `model`, `filter`, `config`, `token` et `github`, jamais de
  `app` ni `ui`. Son JSON est le contrat de l'app de la barre des menus.
- Dans `macos/`, `OwlBarCore` décide, `OwlBar` dessine. Une vue ne choisit ni
  pictogramme, ni texte, ni format.

Une décision qui apparaît dans `ui` — quel pictogramme, quel message, quelle
troncature — est au mauvais endroit : elle appartient à `app` ou à `model`.

## Tests

Tout ce qui décide se teste sans réseau et sans terminal. `filter` en tests purs,
`model` sur des réponses GraphQL enregistrées dans `tests/fixtures/`, `app` en lui
envoyant des touches et en vérifiant l'état obtenu. Seul le dessin n'a pas de test
automatique.

Les fichiers de `tests/fixtures/` décrivent ce que GitHub renvoie réellement, formes
surprenantes comprises. On les complète plutôt que de contourner un cas gênant.

## Conventions

- Le code est en anglais : identifiants, variables, noms de tests. L'interface
  aussi : tout ce que l'utilisateur lit, messages d'erreur du démarrage compris.
- Les commentaires, les documents de `docs/` et les messages de commit sont en
  français.
- Les messages d'erreur de GitHub sont affichés tels quels, sans reformulation. Ils
  disent quoi faire mieux qu'un message maison.
- `owl` ne duplique pas les règles de GitHub. Les protections de branche sont
  appliquées par GitHub ; `owl` transmet et rapporte.
- Le jeton d'authentification n'est jamais écrit dans un fichier, journalisé ni
  affiché.
- Le terminal est toujours restauré à la sortie, panique comprise.

## Hors périmètre

`owl` ne crée pas de pull request, ne pousse pas de code, ne rédige pas de
commentaires, ne gère pas les issues, n'affiche pas de diff ligne à ligne et ne
fusionne pas plusieurs PR d'un coup.
