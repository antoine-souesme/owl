# 05 — Erreurs et tests

## Objet

Décrit le comportement de `owl` quand quelque chose se passe mal, et la façon dont le
projet se vérifie.

## Principe pour les erreurs

Deux catégories, deux traitements. Une erreur de démarrage empêche `owl` de
fonctionner : il s'arrête avant d'ouvrir l'écran, avec un message sur la sortie
d'erreur et un code de sortie non nul. Une erreur en cours de route ne doit jamais
faire perdre ce qui est déjà affiché : la liste reste à l'écran, l'erreur s'affiche
dans la barre d'état, et le prochain rafraîchissement retente.

## Erreurs de démarrage

| Situation | Message |
|---|---|
| `gh` absent du `PATH` et aucune variable de jeton | « owl a besoin de gh. Installe-le, puis lance `gh auth login`. » |
| `gh` présent mais non connecté | « Non connecté à GitHub. Lance `gh auth login`. » |
| Jeton refusé (HTTP 401) | « Jeton refusé par GitHub. Lance `gh auth login` pour le renouveler. » |
| Droits insuffisants (HTTP 403 qui n'est pas une limite d'appels) | « Le jeton n'a pas les droits nécessaires. Vérifie la portée `repo`. » |
| Valeur de réglage invalide | « Réglages invalides dans <chemin> : <clé fautive>. » |
| Fichier de réglages mal formé | « Réglages invalides dans <chemin> : syntaxe TOML invalide. » |
| Fichier de réglages présent mais illisible | « Réglages invalides dans <chemin> : fichier illisible. » |
| Liste de filtres vide, ou dont tous les éléments sont blancs | « Aucun filtre actif : la recherche ramènerait tout GitHub. » |
| Dossier personnel introuvable | « Impossible de déterminer le dossier de configuration. » |

Les trois cas de réglages sont distincts parce qu'ils n'apprennent pas la même
chose : une valeur refusée peut nommer sa clé, un fichier syntaxiquement faux n'a
aucune clé à nommer, et un fichier présent mais illisible — droits insuffisants,
par exemple — n'est pas un fichier absent. Un fichier absent, lui, n'est pas une
erreur : les valeurs par défaut s'appliquent.

Ces messages sont écrits avant toute prise de contrôle du terminal, donc jamais
avalés par l'écran alterné.

## Erreurs en cours d'usage

Réseau injoignable, requête expirée, ou erreur applicative de GraphQL : la barre
d'état affiche une ligne courte, la liste précédente reste visible, et l'heure du
dernier rafraîchissement réussi permet de mesurer l'ancienneté de ce qui est affiché.
Le minuteur continue, donc un réseau qui revient se rattrape tout seul.

L'erreur en cours est effacée dès qu'une requête réussit.

Un échec de fusion n'est pas traité ici : il reste dans la fenêtre de fusion, décrit
en `04-fusion.md`.

## Limite d'appels

Le champ `rateLimit` est lu à chaque requête réussie. Quand le solde restant tombe à
zéro, `owl` suspend le rafraîchissement automatique jusqu'à l'heure de
réinitialisation, et la barre d'état l'annonce : « limite d'appels atteinte,
reprise à 14 h 32 ». La touche `r` est refusée pendant cette suspension, avec le même
message. `owl` ne réessaie jamais en boucle une requête refusée pour cause de limite.

Quand GitHub refuse un solde de limite primaire épuisé sans que l'en-tête
d'heure de réinitialisation soit exploitable, `owl` attend une minute avant de
reprendre. L'attente est arbitraire, mais l'interdiction de réessayer en
boucle, elle, ne l'est pas.

Une limite secondaire — le garde-fou de GitHub contre les rafales, indépendant du
compteur principal — n'empêche jamais le démarrage : elle est temporaire, et la
confondre avec un manque de droits ferait s'arrêter `owl` sur un diagnostic faux.
Elle se reconnaît à son en-tête de délai de reprise, au code 429, ou, faute des
deux, à la phrase que GitHub écrit dans le corps de la réponse. Elle se traite
comme la limite primaire : rafraîchissement suspendu, jamais de réessai en boucle,
et une minute d'attente quand aucune heure de reprise n'est donnée.

Aux réglages par défaut — une requête de liste par minute — la consommation reste
très en dessous des quotas de GitHub. La suspension est un garde-fou, pas un cas
courant.

## Terminal

Le mode brut et l'écran alterné sont restaurés par un garde de portée, doublé d'un
crochet de panique. Un plantage laisse un terminal utilisable et affiche la trace
d'erreur normalement.

Un terminal trop étroit pour la liste affiche « Élargis le terminal » plutôt qu'un
affichage tronqué au hasard.

## Tests

Le découpage en modules a été choisi pour rendre l'essentiel testable sans réseau ni
terminal. La règle est simple : tout ce qui décide se teste, seul le dessin échappe
aux tests automatiques.

### `filter` — tests unitaires purs

Une liste de filtres donne une chaîne de recherche. Voir les critères de réussite de
`02-filtres.md`.

### `model` et `github::dto` — tests sur réponses enregistrées

Des réponses GraphQL réelles sont enregistrées dans `tests/fixtures/`, obtenues une
fois via `gh api graphql` puis anonymisées. Les tests vérifient la traduction vers les
types métier : états de vérification, états de relecture, drapeaux du dépôt, nœuds
d'issue mélangés, PR sans CI, les deux formes de vérification.

Ces fichiers sont la référence du projet : ils décrivent ce que GitHub renvoie
vraiment, y compris ses formes surprenantes.

### `app` — tests d'état

`App::handle` prend un événement et renvoie des commandes. Chaque test construit un
état, envoie une suite d'événements, et vérifie l'état obtenu et les commandes
émises. Aucun terminal, aucun réseau. Les critères de réussite de
`03-affichage-et-navigation.md` et de `04-fusion.md` sont directement ces tests.

### `github` — client HTTP

Testé contre un serveur local qui rejoue des réponses : succès, tableau `errors`,
401, 403 de droits insuffisants, 403 et 429 de limite d'appels sous leurs
trois formes, corps tronqué. On vérifie que chaque cas donne la
bonne variante d'erreur.

### `ui` — pas de test automatique

Les fonctions de dessin sont vérifiées à l'œil. Elles ne contiennent aucune décision :
tout choix — quel pictogramme, quelle troncature, quel message — appartient à `app` ou
à `model`, et se teste là. Une fonction de dessin qui aurait besoin d'un test est le
signe qu'une décision a fui au mauvais endroit.

### Vérification avant de considérer le travail fini

`cargo build`, `cargo test`, `cargo clippy -- -D warnings` et `cargo fmt --check`
passent tous. Aucune de ces commandes n'est optionnelle.

## Critères de réussite

- Chaque erreur de démarrage du tableau produit son message et un code non nul, sans
  prise de contrôle du terminal.
- Une panne réseau pendant l'usage laisse la liste affichée et n'arrête pas le
  programme.
- Un solde de limite d'appels nul suspend le minuteur et refuse `r`, avec l'heure de
  reprise.
- Une panique du programme rend un terminal utilisable.
