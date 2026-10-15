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

Chaque ligne montre le dépôt, le numéro, l'âge de la dernière mise à jour, la
branche visée, le titre, l'état de la CI et l'état des relectures ; les brouillons et
les conflits y sont signalés. L'affichage reste volontairement sobre : une ligne par
PR, des pictogrammes plutôt que des phrases, et une couleur par colonne pour s'y
retrouver d'un coup d'œil.

L'interface est en anglais.

La flèche droite ouvre le détail d'une PR : description, auteur, branches, liste des
vérifications, relectures et commentaires, fichiers modifiés. La flèche gauche
revient à la liste.

La touche « m » fusionne la PR sélectionnée. `owl` propose uniquement les méthodes
de fusion réellement autorisées par le dépôt ; s'il n'y en a qu'une, il demande
seulement confirmation.

La liste se rafraîchit toute seule chaque minute, et la touche « r » force un
rafraîchissement.

Quand une PR devient fusionnable — GitHub dit que plus rien ne bloque —, `owl`
envoie une notification du système. Une par PR, et seulement au moment où elle le
devient : ce qui était déjà prêt au lancement ne déclenche rien.

## Dans la barre des menus

Une petite app macOS, `Owl.app`, garde `owl` sous la main hors du terminal.
Elle est autonome : `owl` est rangé à l'intérieur, rien n'est à installer dans
le terminal, et `macos/build.sh` la construit et l'installe d'une seule commande.
Une chouette dans la barre des menus affiche le nombre de PR prêtes à
fusionner et le nombre des autres. Un clic ouvre un panneau avec la même
liste ; un clic sur une PR ouvre sa fenêtre de fusion, et un bouton l'ouvre
sur GitHub. L'app interroge `owl` en coulisses,
avec les sous-commandes `owl list`, `owl detail` et `owl merge`, qui
répondent en JSON. Elle se lance à l'ouverture de session et prévient quand
une PR devient fusionnable.

Le panneau a deux onglets. « Pull requests » est l'écran décrit plus haut.
« Servers » liste les serveurs node qui écoutent un port sur le Mac — ceux
qu'un agent a lancés et oubliés, par exemple —, avec pour chacun le port, le
projet, la commande et depuis quand il tourne. Un bouton « Stop » coupe le
serveur et les processus node qui l'ont lancé (`npm run dev`, `next dev`),
jamais le terminal ni l'agent. Un champ permet de chercher n'importe quel port,
quel que soit le programme ; couper depuis la recherche demande toujours
confirmation, et la fenêtre passe au rouge pour un port critique (port
système, base de données, AirPlay…). L'app s'appuie sur `owl ports` et
`owl stop`.

## Ce que owl ne fait pas

`owl` ne crée pas de pull request, ne pousse pas de code, ne rédige pas de
commentaires et ne gère pas les issues. Côté GitHub, il lit et il fusionne, rien d'autre.

## Choix techniques

Rust, avec `ratatui` pour l'écran. L'authentification est empruntée à `gh`
(`gh auth token`), donc `owl` n'a aucune connexion à gérer. Les données viennent de
l'API GraphQL de GitHub : une seule requête ramène la liste entière avec tous ses
états.

## Prérequis

Rust (à installer), et `gh` installé et connecté.
