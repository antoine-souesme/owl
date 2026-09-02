# owl

`owl` est un petit outil en ligne de commande qui affiche les pull requests de mon
compte GitHub dans le terminal, et qui permet de les fusionner en respectant les
règles du dépôt.

## À quoi ça sert

Aujourd'hui, suivre ses pull requests demande d'ouvrir un navigateur ou
d'enchaîner des commandes `gh` dépôt par dépôt. `owl` regroupe tout dans un seul
écran : la liste des PR, leur état de vérification, leur état de relecture, et la
possibilité de fusionner sans quitter le terminal.

## Ce que fait owl

On lance `owl`, sans argument. Un écran s'affiche avec la liste des pull requests
concernées — par défaut mes PR ouvertes, tous dépôts confondus.

Chaque ligne montre le dépôt, le numéro, le titre, l'état de la CI, l'état des
relectures, et signale les brouillons et les conflits. L'affichage reste volontairement
sobre : une ligne par PR, des pictogrammes plutôt que des phrases.

La flèche droite ouvre le détail d'une PR : description, auteur, branches, liste des
vérifications, relectures et commentaires, fichiers modifiés. La flèche gauche
revient à la liste.

La touche « m » fusionne la PR sélectionnée. `owl` propose uniquement les méthodes
de fusion réellement autorisées par le dépôt ; s'il n'y en a qu'une, il demande
seulement confirmation.

La liste se rafraîchit toute seule chaque minute, et la touche « r » force un
rafraîchissement.

## Ce que owl ne fait pas

`owl` ne crée pas de pull request, ne pousse pas de code, ne rédige pas de
commentaires et ne gère pas les issues. Il lit et il fusionne, rien d'autre.

## Choix techniques

Rust, avec `ratatui` pour l'écran. L'authentification est empruntée à `gh`
(`gh auth token`), donc `owl` n'a aucune connexion à gérer. Les données viennent de
l'API GraphQL de GitHub : une seule requête ramène la liste entière avec tous ses
états.

## Prérequis

Rust (à installer), et `gh` installé et connecté.

## Documentation

Les spécifications détaillées se trouvent dans `docs/specs/`.
