# Dette technique

Registre des points mis de côté pendant l'exécution des plans. Un point y entre
quand il est identifié puis délibérément différé — pas quand il est corrigé.

Ce fichier n'est pas un historique : une entrée résolue en sort.

## Format

Une entrée par point, la plus récente en haut.

```markdown
### <titre court>

- **Origine** : plan ou tâche d'où le point sort.
- **Ce qui est différé** : l'état courant, décrit tel quel.
- **Pourquoi** : la raison du report.
- **Ce qu'il faudrait faire** : la piste de correction, assez précise pour être reprise sans rouvrir l'analyse.
```

---

### Une limite secondaire classée en refus de droits fait échouer le démarrage avec un faux diagnostic

- **Origine** : plan `2026-09-02-erreurs-et-tests`, revue finale de la branche.
- **Ce qui est différé** : une limite secondaire de GitHub (403 ou 429 sans `retry-after`, solde non nul) est classée en `GithubError::Forbidden`. Reçue à la première requête, elle fait maintenant refuser le démarrage avec le message « Le jeton n'a pas les droits nécessaires. Vérifie la portée `repo`. » — un diagnostic faux, et une régression par rapport au comportement précédent où le programme démarrait.
- **Pourquoi** : corriger le classement demande de trancher dans `docs/specs/05-erreurs-et-tests.md` la distinction entre droits insuffisants et limite secondaire, que le tableau des erreurs de démarrage ne fait pas ; c'est une décision de spec, hors du périmètre de ce plan.
- **Ce qu'il faudrait faire** : distinguer la limite secondaire du refus de droits dans `src/github/mod.rs` (un corps de réponse de limite secondaire porte un message reconnaissable), lui donner sa propre variante d'erreur, et décider dans la spec si elle empêche le démarrage ou non.

---

### Confirmer une fusion sur une pull request disparue ne produit rien

- **Origine** : plan `2026-09-02-fusion`, revue finale de la branche.
- **Ce qui est différé** : `App::submit_merge` cherche le résumé de la pull request visée par `resume_affiche`. Si la PR a disparu à la fois de la liste et du cache de détails entre l'ouverture de la fenêtre et la confirmation, la fonction rend une liste de commandes vide sans rien changer : la fenêtre reste en `Choosing` et `Entrée` semble ne rien faire. Le cas se produit quand une réponse de liste déjà en vol au moment de l'appui sur `m` retire la PR.
- **Pourquoi** : le correctif demande un message d'écran qu'aucune spec ne définit, et la fenêtre reste fermable par `Échap` : le défaut est silencieux, pas bloquant.
- **Ce qu'il faudrait faire** : décider du message dans `docs/specs/04-fusion.md`, puis, dans ce cas, fermer la fenêtre et poser ce message dans `notice`.

---

### Une liste de filtres composée uniquement de chaînes blanches n'est pas refusée

- **Origine** : plan `2026-09-02-filtres`, revue finale de la branche.
- **Ce qui est différé** : `filters = [""]` dans les réglages passe le garde-fou de `src/config.rs`, qui ne vérifie que la liste n'est pas vide. `Filter::parse` transforme cette chaîne en `Raw("")`, et `build_query` écarte le fragment vide qui en résulte : la requête devient `is:pr sort:updated-desc`, exactement la recherche « toute la planète GitHub » que le garde-fou est censé empêcher.
- **Pourquoi** : `src/config.rs` est explicitement hors du périmètre du plan `2026-09-02-filtres` ; corriger ce fichier n'y a pas sa place.
- **Ce qu'il faudrait faire** : dans `config.rs`, refuser aussi une liste dont tous les éléments sont blancs (`filtres.iter().all(|f| f.trim().is_empty())`), avec la même erreur `EmptyFilters`.
