# owl

Outil en ligne de commande interactif qui liste les pull requests de mon compte
GitHub dans le terminal et permet de les fusionner selon la règle du dépôt.

Le résumé du projet est dans `DESCRIPTION.md`. Les spécifications sont dans
`docs/specs/`, à lire dans l'ordre numéroté ; `docs/specs/README.md` en donne l'index.

## Ordre de vérité

Les specs font foi. Si le code s'en écarte, c'est le code qui a tort — sauf décision
explicite, auquel cas on met la spec à jour dans le même commit que le code.

Chaque spec se termine par ses critères de réussite. Ce sont les tests à écrire.

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

## Règles d'architecture

Elles sont détaillées dans `docs/specs/00-fondations.md`. En bref, les dépendances
entre modules sont à sens unique et strictes :

- `model` et `filter` ne dépendent ni du réseau ni du terminal.
- `github` dépend de `model` et `filter`, jamais de `app` ni `ui`.
- `app` ne fait aucun appel réseau : il reçoit des événements et renvoie des
  commandes que la boucle principale exécute.
- `ui` lit `app` et dessine. Une fonction de dessin ne modifie jamais l'état et ne
  prend aucune décision.

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
