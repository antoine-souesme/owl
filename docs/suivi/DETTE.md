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

### Le texte d'une ligne de liste est composé dans le dessin

- **Origine** : plan `2026-09-02-fondations`, tâche 5 — relevé par la revue finale de la branche.
- **Ce qui est différé** : `src/ui/list.rs` assemble lui-même le texte de chaque ligne, `format!("{}#{}  {}", pr.repository, pr.number, pr.title)`. Le séparateur et l'ordre des champs sont donc décidés dans une fonction de dessin, alors que la règle d'architecture réserve ces choix à `app` ou `model`. La barre d'état, elle, a été corrigée : elle lit `app.status_line()` sans rien composer.
- **Pourquoi** : la spec 03 remplace entièrement ce bouchon d'affichage. Elle apporte le format réel d'une ligne — pictogrammes d'état, troncature selon la largeur du terminal, colonnes alignées — donc déplacer la composition actuelle vers `app` serait un travail à refaire aussitôt.
- **Ce qu'il faudrait faire** : à la spec 03, exposer le résumé d'une pull request depuis `app` ou `model`, sous la forme déjà prête à afficher que réclame `03-affichage-et-navigation.md`, et réduire `list.rs` à `ListItem::new(resume)`. Un test dans `app` couvre alors le format ; `list.rs` n'en a plus besoin.
