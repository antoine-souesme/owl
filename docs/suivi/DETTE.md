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
