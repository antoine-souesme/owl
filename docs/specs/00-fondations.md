# 00 — Fondations

## Objet

Ce document fixe la technologie, la structure du projet et les réglages. Les autres
specs s'appuient sur lui.

## Nature de l'outil

`owl` est un binaire unique, interactif. Lancé sans argument, il prend le contrôle du
terminal, affiche un écran et le rend à la sortie. Il n'y a pas de sous-commandes.

## Technologie

Rust, édition 2021.

Bibliothèques retenues :

| Rôle | Bibliothèque |
|---|---|
| Écran et composants | `ratatui` |
| Événements clavier et terminal | `crossterm` |
| Exécution asynchrone | `tokio` |
| Appels HTTP | `reqwest` (avec `rustls`, pour éviter OpenSSL) |
| Sérialisation | `serde`, `serde_json` |
| Fichier de réglages | `toml` |
| Erreurs | `thiserror` pour les erreurs du domaine, `anyhow` dans le binaire |
| Chemins de configuration | `directories` |
| Dates | `chrono` |
| Ouverture dans le navigateur | `open` |

## Structure des modules

```
src/
  main.rs        démarrage, boucle d'événements, restauration du terminal
  config.rs      lecture du fichier de réglages
  token.rs       résolution du jeton d'authentification
  github/
    mod.rs       client GraphQL
    queries.rs   requêtes et mutation
    dto.rs       types de réponse brute, mappés vers model
  model.rs       types métier
  filter.rs      filtres et construction de la requête de recherche
  app/
    mod.rs       état de l'application, réception des événements
    render.rs    composition de l'affichage : pictogrammes, colonnes, messages
  ui/
    mod.rs       aiguillage de dessin selon la vue
    list.rs      dessin de la liste
    detail.rs    dessin du détail
    merge.rs     dessin de la fenêtre de fusion
```

### Règles de dépendance

Elles sont volontairement strictes, parce qu'elles sont ce qui rend le projet
testable :

- `model` et `filter` ne dépendent de rien d'autre que de la bibliothèque standard
  et de `serde`. Ni réseau, ni terminal.
- `github` dépend de `model` et de `filter`. Il ne connaît pas `app` ni `ui`.
- `app` dépend de `model` et de `filter`. Il ne fait aucun appel réseau lui-même :
  il émet des demandes et reçoit des résultats.
- `ui` dépend de `app` en lecture seule. Une fonction de dessin ne modifie jamais
  l'état.

Toute violation de ces règles est un défaut, pas un raccourci.

## Concurrence

Le programme tourne sur un exécuteur `tokio`. Trois producteurs d'événements
alimentent une même file :

1. les touches du clavier, lues dans une tâche dédiée ;
2. les résultats des appels réseau, chacun dans sa propre tâche ;
3. un minuteur de rafraîchissement.

La boucle principale prend un événement, le passe à `app`, puis redessine. Elle ne
fait jamais d'attente bloquante : un appel réseau lent laisse l'écran vivant et
réactif.

Les demandes réseau lancées par `app` portent un numéro de génération. Un résultat
dont la génération est périmée est ignoré, ce qui évite qu'une réponse lente n'écrase
une réponse plus récente.

## Authentification

Le jeton est résolu dans cet ordre, en s'arrêtant au premier trouvé :

1. la variable d'environnement `OWL_TOKEN` ;
2. la variable d'environnement `GITHUB_TOKEN` ;
3. la sortie de `gh auth token`.

Si aucune source n'aboutit, `owl` s'arrête avant d'ouvrir l'écran, avec un message
qui indique d'exécuter `gh auth login`.

Le jeton n'est jamais écrit dans un fichier, ni journalisé, ni affiché.

## Fichier de réglages

Emplacement : `~/.config/owl/config.toml`. Le fichier est optionnel ; absent, les
valeurs par défaut s'appliquent. Une clé inconnue est ignorée sans erreur. Une valeur
invalide provoque un arrêt avec un message précisant la clé fautive.

```toml
# Filtres actifs au démarrage. Voir 02-filtres.md.
filters = ["author:@me", "is:open"]

# Intervalle de rafraîchissement automatique, en secondes.
# 0 désactive le rafraîchissement automatique.
refresh_interval = 60

# Méthode de fusion présélectionnée quand le dépôt en autorise plusieurs.
# Valeurs acceptées : "squash", "rebase", "merge".
# config::MergeMethod est une réexportation de model::MergeMethod : le type
# appartient au modèle, `github` en a besoin pour la mutation et n'a pas le
# droit de dépendre des réglages.
preferred_merge_method = "squash"

# Nombre maximal de PR ramenées par requête (1 à 100).
page_size = 50
```

## Critères de réussite des fondations

- `cargo build` produit un binaire `owl`.
- Lancé sans `gh` connecté et sans variable d'environnement, `owl` affiche un
  message clair et rend un code de sortie non nul, sans salir le terminal.
- Lancé avec un jeton valide, `owl` ouvre l'écran et le referme proprement sur
  « q », y compris en cas de panique du programme.
