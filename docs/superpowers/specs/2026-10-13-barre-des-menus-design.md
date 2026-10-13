# owl dans la barre des menus

## Objectif

Pouvoir suivre et fusionner ses pull requests depuis n'importe où sur le Mac, sans
ouvrir de terminal. Une icône vit en permanence dans la barre des menus, en haut de
l'écran : elle affiche un compteur, et un clic ouvre un panneau qui offre ce que la
TUI offre aujourd'hui — liste, détail, fusion.

L'icône ne dépend pas de la TUI : elle tourne seule, que `owl` soit lancé dans un
terminal ou non.

## Ce qui a été décidé

- Une app macOS séparée, écrite en Swift (SwiftUI), rangée dans le dépôt sous
  `macos/`.
- L'app ne parle jamais à GitHub. Elle appelle le programme `owl` déjà installé,
  avec de nouvelles sous-commandes non interactives qui répondent en JSON. Toute
  la logique GitHub, le jeton emprunté à `gh`, les filtres de la configuration et
  les messages d'erreur restent en Rust, à un seul endroit.
- La TUI ne change pas : `owl` sans argument se comporte exactement comme
  aujourd'hui.

## Partie 1 — les sous-commandes de `owl`

### Contrat

Trois sous-commandes. Chacune écrit un seul document JSON sur la sortie standard et
se termine.

| Commande | Rôle |
|---|---|
| `owl list` | La liste, avec les filtres et la taille de page de la configuration. |
| `owl detail <owner/repo> <number>` | Le détail d'une pull request. |
| `owl merge <owner/repo> <number> <merge\|squash\|rebase>` | Fusionne la pull request. |

Toute autre sous-commande, ou des arguments manquants, sont une erreur d'usage :
message en anglais sur la sortie d'erreur, code de sortie 2, rien sur la sortie
standard.

Le document a toujours la même enveloppe, avec un numéro de version du format pour
que l'app refuse proprement un `owl` trop ancien ou trop récent :

```json
{ "version": 1, "ok": true, "data": { ... } }
{ "version": 1, "ok": false, "error": { "kind": "unauthorized", "message": "Token refused by GitHub. Run `gh auth login` to renew it." } }
```

`message` est le texte exact de l'erreur, celui que la TUI affiche ; les messages
de GitHub passent tels quels. `kind` reprend la variante de l'erreur (`api`,
`unauthorized`, `forbidden`, `rate_limited`, `http`, `malformed`, `transport`,
`not_found`, `config`, `token`) pour que l'app choisisse l'état de l'icône sans
analyser le texte. `rate_limited` porte en plus `reset_at` quand GitHub le donne.

Le code de sortie vaut 0 quand `ok` est vrai, 1 sinon. Une erreur de configuration
ou de jeton survenant avant toute requête passe aussi par cette enveloppe, et non
par la sortie d'erreur : l'app n'a qu'un seul format à lire.

### Contenu de `data`

`owl list` rend :

- `pull_requests` : pour chacune, tous les champs de `PrSummary` — clé (`repo`,
  `number`), titre, auteur, URL, brouillon, état CI, état de relecture, état de
  conflit, verdict de fusion (`merge_state`), branches, date de mise à jour,
  méthodes de fusion autorisées par le dépôt, vérifications du dernier commit ;
- `preferred_merge_method` : la méthode préférée de la configuration, pour que le
  panneau la présélectionne comme le fait la TUI ;
- `refresh_interval` : l'intervalle de rafraîchissement de la configuration, en
  secondes ;
- `rate_limit` quand il est connu.

`owl detail` rend tous les champs de `PrDetail` : le résumé, la description,
les vérifications, les relectures, les commentaires, les fichiers modifiés et le
total des lignes ajoutées et retirées. L'identifiant GraphQL n'y figure pas :
`owl merge` le récupère lui-même.

`owl merge` rend `data: {}` en cas de succès.

Les valeurs d'état sont écrites en minuscules (`success`, `pending`,
`changes_requested`, `clean`…). Les dates sont au format ISO 8601 en UTC.

### Organisation du code

Un nouveau module `cli` reçoit les arguments, appelle `config`, `token` et
`github`, et écrit le JSON. Il dépend de `model`, `filter`, `config`, `token` et
`github`, jamais de `app` ni `ui`. Les structures écrites en JSON sont propres à
`cli` : `model` ne prend pas de dépendance à la forme du JSON, et le contrat reste
lisible à un seul endroit.

`main` regarde ses arguments avant toute prise du terminal : avec une
sous-commande, il passe la main à `cli` et ne touche jamais à l'écran.

`owl detail` et `owl merge` partent d'une clé (`owner/repo` et numéro) et non d'un
résumé déjà chargé : `github` gagne la capacité d'aller chercher une pull request
par sa clé seule.

Le jeton n'apparaît jamais dans le JSON, sur la sortie d'erreur ni dans les
messages.

## Partie 2 — l'app de la barre des menus

### L'icône et le compteur

L'icône est une chouette suivie de deux nombres : les pull requests prêtes, puis
toutes les autres, par exemple `🦉 2 · 5`.

« Prête » veut dire que GitHub déclare que plus rien ne bloque (`merge_state` à
`clean`). Toutes les autres pull requests de la liste comptent dans le second
nombre, brouillons compris.

Quand la dernière lecture a échoué, l'icône passe en état d'alerte (pictogramme
d'avertissement à la place des nombres) jusqu'à la prochaine lecture réussie.

### Le panneau

Un clic sur l'icône ouvre un panneau d'environ 480 points de large, sous l'icône.

**Liste.** Une ligne par pull request, comme dans la TUI : dépôt, numéro, âge de
la dernière mise à jour, branche visée, titre, pictogrammes CI et relecture,
brouillon et conflit signalés. En haut, le moment de la dernière lecture et un
bouton de rafraîchissement ; en bas, un bouton pour quitter l'app.

**Détail.** Cliquer une ligne fait glisser le panneau vers le détail : titre,
auteur, branches, description, vérifications, relectures, commentaires, fichiers
modifiés. Un bouton retour revient à la liste, un bouton « Open on GitHub » ouvre
la pull request dans le navigateur, un bouton « Merge » lance la fusion.

**Fusion.** Le bouton « Merge » ne propose que les méthodes autorisées par le
dépôt, la méthode préférée de la configuration présélectionnée. S'il n'y en a
qu'une, il demande seulement confirmation. Si le dépôt n'en autorise aucune, le
bouton est désactivé avec le message « No merge method allowed on this
repository. », comme dans la TUI. Pendant la fusion, le bouton montre une
animation ; en cas de succès, le panneau revient à la liste et la rafraîchit ; en
cas d'échec, le message de GitHub s'affiche tel quel sous le bouton.

**Erreur.** Si la dernière lecture a échoué, le panneau affiche le message
d'erreur à la place de la liste, avec le bouton de rafraîchissement.

Toute l'interface est en anglais, comme la TUI.

### Rafraîchissement

L'app lance `owl list` au démarrage, puis toutes les `refresh_interval` secondes
lues dans la configuration de `owl` (une minute par défaut), et à chaque clic sur
le bouton de rafraîchissement. Une lecture en cours n'est jamais doublée. En cas de
limite d'appels atteinte, l'app ne relance rien avant `reset_at`.

L'intervalle est transmis par `owl list` dans son `data` (`refresh_interval`) :
l'app ne lit pas le fichier de configuration elle-même.

### Notifications

Quand une pull request passe à l'état prête entre deux lectures, l'app envoie une
notification du système : une par pull request, seulement au moment du passage.
Ce qui est déjà prêt à la première lecture ne déclenche rien. C'est la même règle
que la TUI.

Si la TUI tourne en même temps, la notification arrive en double. C'est accepté
pour cette version.

### Trouver `owl` et `gh`

Une app lancée depuis le Finder ne voit pas le `PATH` du terminal. L'app cherche
`owl` dans `~/.cargo/bin`, `/opt/homebrew/bin` et `/usr/local/bin`, dans cet
ordre, et le lance avec un `PATH` qui contient ces mêmes dossiers, pour que `owl`
trouve `gh` à son tour.

Si `owl` est introuvable, l'icône passe en alerte et le panneau dit « owl not
found. Install it with `cargo install --path .` ». Si `owl` répond avec une
version de format inconnue, le panneau dit « This owl version is not supported by
the menu bar app. Update both. ».

### Lancement à l'ouverture de session

L'app s'inscrit d'elle-même comme élément d'ouverture de session au premier
lancement, et le panneau offre une case « Launch at login » pour le retirer. Elle
n'a pas d'icône dans le Dock.

### Construction et installation

Un paquet Swift sous `macos/`, sans projet Xcode. Un script `macos/build.sh`
compile l'app, assemble le paquet `Owl.app`, le signe localement et le copie dans
`~/Applications`. Aucune distribution ni signature de développeur n'est prévue.

Le script de publication du projet (compétence `release`) n'est pas modifié dans
cette version.

### Organisation du code Swift

La même règle qu'en Rust : ce qui décide est séparé de ce qui dessine.

- **Lecture du JSON** : décode l'enveloppe et les données, refuse une version
  inconnue.
- **Exécution de `owl`** : trouve le programme, le lance, rend le JSON lu. C'est le
  seul endroit qui lance un processus.
- **État** : la liste, le compteur, la pull request ouverte, la fusion en cours,
  l'erreur, la détection des passages à l'état prête. Il ne lance aucun processus
  lui-même et ne dessine rien.
- **Vues** : l'icône et le panneau. Elles lisent l'état et ne décident rien — ni
  pictogramme, ni texte, ni format d'âge, qui viennent de l'état.

## Tests

**Rust.** Les sous-commandes se testent sans réseau : la transformation des
réponses enregistrées dans `tests/fixtures/` en JSON, l'enveloppe d'erreur pour
chaque variante, la lecture des arguments et l'erreur d'usage. Un exemple de
sortie de `owl list` et de `owl detail` est enregistré dans `macos/Tests/fixtures/`
à partir de ces mêmes réponses ; un test Rust vérifie qu'il reste à jour.

**Swift.** Sur ces exemples : la lecture du JSON, le refus d'une version inconnue,
le compteur, la détection des passages à l'état prête (rien à la première
lecture, une seule notification par passage), le choix des méthodes de fusion
proposées et de la méthode présélectionnée, l'état d'erreur. Le lancement de `owl`
est remplacé par un faux qui rend un JSON donné. Seul le dessin n'a pas de test
automatique.

Les commandes du projet (`cargo build`, `cargo test`, `cargo clippy -- -D
warnings`, `cargo fmt --check`) doivent passer, ainsi que `swift build` et
`swift test` dans `macos/`.

## Hors périmètre

Pas de fusion de plusieurs pull requests d'un coup, pas de création de pull
request, pas de commentaires, pas de diff ligne à ligne : les mêmes limites que la
TUI. Pas de modification des filtres depuis le panneau. Pas de distribution de
l'app hors de ce Mac.
