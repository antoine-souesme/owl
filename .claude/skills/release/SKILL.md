---
name: release
description: Use when the user asks to release owl, publish a new version, or bump the version number of this project
---

# Release owl

Publie une nouvelle version de `owl` en passant par `main`, puis remet `develop`
à jour.

## Version

La version est fournie en argument (par exemple `0.2.0`). Si elle est absente,
la demander à l'utilisateur et ne rien faire tant qu'elle n'est pas connue.
Format attendu : `X.Y.Z`, sans préfixe `v`.

## Avant de commencer

Refuser et prévenir l'utilisateur si l'une de ces conditions est vraie :

- l'arbre de travail n'est pas propre (`git status --porcelain` renvoie du texte)
- la version demandée n'est pas supérieure à celle de `Cargo.toml`
- `cargo test`, `cargo clippy -- -D warnings` ou `cargo fmt --check` échouent

## Étapes

1. `git fetch` puis `git checkout develop && git pull`
2. `git checkout main && git pull`
3. `git merge develop --no-ff -m "Fusion de develop pour la version X.Y.Z"`
4. Remplacer la version dans `Cargo.toml`, puis `cargo build` pour mettre
   `Cargo.lock` à jour
5. `git add Cargo.toml Cargo.lock && git commit -m "Version X.Y.Z"`
6. `git push origin main`
7. `git checkout develop && git merge main --no-ff -m "Retour de la version X.Y.Z"`
8. `git push origin develop`

Si une fusion tombe en conflit, s'arrêter et prévenir l'utilisateur : ne jamais
résoudre un conflit de fusion de release tout seul.

## À la fin

Annoncer la version publiée et l'état des deux branches, en une ou deux phrases.

## Hors périmètre

Pas de tag, pas de release GitHub, pas de publication sur crates.io, pas de
pull request : uniquement les fusions et les poussées décrites ci-dessus.
