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

### Une liste de filtres composée uniquement de chaînes blanches n'est pas refusée

- **Origine** : plan `2026-09-02-filtres`, revue finale de la branche.
- **Ce qui est différé** : `filters = [""]` dans les réglages passe le garde-fou de `src/config.rs`, qui ne vérifie que la liste n'est pas vide. `Filter::parse` transforme cette chaîne en `Raw("")`, et `build_query` écarte le fragment vide qui en résulte : la requête devient `is:pr sort:updated-desc`, exactement la recherche « toute la planète GitHub » que le garde-fou est censé empêcher.
- **Pourquoi** : `src/config.rs` est explicitement hors du périmètre du plan `2026-09-02-filtres` ; corriger ce fichier n'y a pas sa place.
- **Ce qu'il faudrait faire** : dans `config.rs`, refuser aussi une liste dont tous les éléments sont blancs (`filtres.iter().all(|f| f.trim().is_empty())`), avec la même erreur `EmptyFilters`.

### La troncature des listes de la vue détail n'est pas mesurable

- **Origine** : plan `2026-09-02-modele-et-donnees`, tâche 4.
- **Ce qui est différé** : la requête de détail borne les listes — vingt relectures, vingt commentaires, cent fichiers — mais ne demande aucun `totalCount`. Impossible, donc, de savoir si une liste est tronquée, alors que `01-modele-et-donnees.md` prévoit une ligne « … et N de plus ».
- **Pourquoi** : la ligne est un élément d'affichage, et l'affichage de la vue détail appartient à `03-affichage-et-navigation.md`. Ajouter des `totalCount` maintenant serait modifier la requête de la spec 01 pour un besoin que personne ne consomme encore.
- **Ce qu'il faudrait faire** : à la spec 03, ajouter `totalCount` aux trois connexions de la requête de détail, le porter dans `PrDetail` — trois champs, ou un compte par liste — et composer la ligne dans `app`. Ou décider que la ligne disparaît, et retirer la phrase de la spec 01.

### Le texte d'une ligne de liste est composé dans le dessin

- **Origine** : plan `2026-09-02-fondations`, tâche 5 — relevé par la revue finale de la branche.
- **Ce qui est différé** : `src/ui/list.rs` assemble lui-même le texte de chaque ligne, `format!("{}#{}  {}", pr.repository, pr.number, pr.title)`. Le séparateur et l'ordre des champs sont donc décidés dans une fonction de dessin, alors que la règle d'architecture réserve ces choix à `app` ou `model`. La barre d'état, elle, a été corrigée : elle lit `app.status_line()` sans rien composer.
- **Pourquoi** : la spec 03 remplace entièrement ce bouchon d'affichage. Elle apporte le format réel d'une ligne — pictogrammes d'état, troncature selon la largeur du terminal, colonnes alignées — donc déplacer la composition actuelle vers `app` serait un travail à refaire aussitôt.
- **Ce qu'il faudrait faire** : à la spec 03, exposer le résumé d'une pull request depuis `app` ou `model`, sous la forme déjà prête à afficher que réclame `03-affichage-et-navigation.md`, et réduire `list.rs` à `ListItem::new(resume)`. Un test dans `app` couvre alors le format ; `list.rs` n'en a plus besoin.
