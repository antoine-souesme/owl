# Serveurs node dans la barre des menus

## Objectif

Des agents lancent des serveurs de développement (Next.js, Vite, `npm run dev`…)
sur des ports comme 3000. Une fois l'agent coupé, on ne sait plus quel serveur
tourne, pour quel projet, ni comment l'arrêter simplement.

Le panneau de la chouette gagne un deuxième onglet, « Servers », qui liste les
serveurs node en écoute et permet de les couper d'un clic. Une recherche par
numéro de port permet aussi de couper n'importe quel programme, avec un
avertissement quand le port est critique.

## Ce qui a été décidé

- Uniquement dans l'app de la barre des menus. La TUI ne change pas.
- Comme pour les pull requests, l'app ne lance aucun outil du système elle-même :
  elle appelle `owl`, qui répond en JSON. La lecture des ports, le choix de ce
  qu'on coupe et la règle « port critique » vivent en Rust, testés sans rien
  lancer.
- La liste ne montre que les serveurs node. La recherche par port montre
  n'importe quel programme.
- Couper depuis la liste se fait sans confirmation. Couper depuis la recherche
  demande toujours confirmation.
- Pas de rafraîchissement en tâche de fond pour cet onglet.
- Le compteur de la chouette ne change pas : il ne parle que des pull requests.

## Partie 1 — `owl`

### Contrat

Deux nouvelles sous-commandes, avec la même enveloppe JSON et les mêmes codes de
sortie que `owl list`. Aucune n'a besoin de la configuration, du jeton ni de
GitHub : une erreur de jeton ne les empêche jamais de répondre.

| Commande | Rôle |
|---|---|
| `owl ports` | Les serveurs node qui écoutent un port TCP. |
| `owl ports <port>` | Tout ce qui écoute ce port, quel que soit le programme. |
| `owl stop <pid> <port>` | Coupe le processus `pid` et sa chaîne node, s'il écoute toujours `port`. |

`<port>` est un entier de 1 à 65535, `<pid>` un entier strictement positif ;
sinon c'est une erreur d'usage, comme aujourd'hui. Le texte d'aide nomme les deux
nouvelles commandes.

`FORMAT_VERSION` reste à 1 : les formats existants ne changent pas, on en ajoute.

### `owl ports` et `owl ports <port>`

Les deux rendent la même forme :

```json
{ "version": 1, "ok": true, "data": { "servers": [
  {
    "port": 3000,
    "pid": 48211,
    "project": "erp-menage",
    "directory": "/Users/antoinesouesme/dev/projets/gkfa/erp-menage",
    "command": "npm run dev",
    "started_at": "2026-10-15T08:12:00Z",
    "user": "antoinesouesme",
    "critical": null
  }
], "hidden": false } }
```

- Une ligne par couple (processus, port). Un même processus qui écoute en IPv4 et
  en IPv6 ne donne qu'une ligne.
- `project` est le dernier élément de `directory`, le dossier courant du
  processus. Si ce dossier est illisible, `directory` et `project` valent `null`.
- `command` est la commande du plus haut processus de la chaîne node (voir plus
  bas), raccourcie : le chemin de l'exécutable est réduit à son nom, et
  `node /…/node_modules/.bin/next dev` devient `next dev`. Pour un programme hors
  famille node, c'est sa propre commande, raccourcie de la même façon.
- `started_at` est l'heure de démarrage du processus qui écoute.
- `critical` vaut `null`, ou un objet `{ "reason": "PostgreSQL" }`.
- La liste est triée par port, puis par `pid`.
- Rien n'écoute : `servers` est vide, ce n'est pas une erreur.
- `hidden` ne vaut `true` que pour `owl ports <port>`, quand `servers` est vide
  mais que le port est pourtant occupé : un processus d'un autre utilisateur ou
  du système, que `lsof` ne montre pas sans droits d'administrateur. `owl` le
  sait en essayant d'ouvrir le port lui-même, sans rien y accepter.

### Famille node

Un processus est de la famille node quand le nom de son exécutable est l'un de :
`node`, `npm`, `npx`, `pnpm`, `yarn`, `bun`, `bunx`, `deno`. Ce nom est celui que
le système connaît (le champ « command » de `lsof`), pas la commande affichée :
Next.js renomme son serveur `next-server (v15…)` et npm se renomme
`npm run dev`, mais le système les connaît tous deux sous le nom `node`. `owl ports` sans
argument ne garde que les processus qui écoutent et qui sont de cette famille.

La **chaîne node** d'un processus est ce processus et ses parents successifs, tant
qu'ils sont de la famille node et appartiennent au même utilisateur. Elle
s'arrête au premier parent qui n'en est pas : un shell, un terminal, un agent. Pour `npm run dev` → `next dev` → serveur, la
chaîne est ces trois processus.

### Port critique

Un port est critique, avec la première raison qui s'applique :

1. le processus n'appartient pas à l'utilisateur courant : « owned by <user> » ;
2. le port est sous 1024 : « system port » ;
3. le port est un service connu :

| Port | Raison |
|---|---|
| 3306 | MySQL |
| 5000, 7000 | AirPlay Receiver |
| 5432 | PostgreSQL |
| 5672 | RabbitMQ |
| 6379 | Redis |
| 9200 | Elasticsearch |
| 11211 | Memcached |
| 27017 | MongoDB |

Critique n'interdit rien : l'app avertit, l'utilisateur décide.

### `owl stop <pid> <port>`

1. Relit les ports. Si `pid` n'écoute plus `port`, rien n'est coupé et la
   réponse est une erreur `not_listening` : « Process 48211 no longer listens on
   port 3000. ». Cela évite de couper un autre programme qui aurait repris le même
   numéro de processus.
2. Calcule la chaîne node de `pid`. Pour un programme hors famille node, la chaîne
   est `pid` seul.
3. Envoie un arrêt propre (`SIGTERM`) à toute la chaîne, du plus haut parent au
   processus qui écoute.
4. Attend jusqu'à 5 secondes que tous aient disparu. Ceux qui restent reçoivent un
   arrêt forcé (`SIGKILL`).

Réponse : `{ "stopped": [48209, 48210, 48211], "forced": false }`. `forced` dit si
l'arrêt forcé a servi.

Erreurs, avec `kind` et un message en anglais :

| `kind` | Cas | Message |
|---|---|---|
| `not_listening` | `pid` n'écoute plus `port` | `Process 48211 no longer listens on port 3000.` |
| `permission` | le système refuse le signal | `Permission denied: process 48211 belongs to root.` |
| `system` | `lsof` ou `ps` introuvable ou en échec | le message du système, tel quel |

Un processus disparu entre deux étapes n'est pas une erreur : il est compté comme
arrêté.

### Lecture du système

`owl` lit trois choses, avec des outils présents sur tous les Mac :

- les processus qui écoutent un port TCP, avec `lsof` en mode champs (`-F`),
  sans résolution des noms ni des ports ;
- le nom système et le dossier courant de tous les processus visibles, avec
  `lsof` (descripteur `cwd`) ;
- la table des processus (numéro, parent, utilisateur, temps écoulé, commande),
  avec `ps`.

### Organisation du code

Un nouveau module `ports`, sans réseau ni terminal, en deux couches :

- **ce qui décide**, en fonctions pures : lecture des sorties de `lsof` et `ps`,
  construction des lignes, filtre de la famille node, chaîne node, règle critique,
  raccourci de la commande, lecture du temps écoulé ;
- **ce qui lance**, une fine couche qui exécute `lsof`, `ps` et envoie les
  signaux, sans aucune décision.

`cli` dépend de `ports`. `ports` ne dépend ni de `github`, ni de `app`, ni de `ui`.
`output.rs` gagne les formes `PortsOut` et `StopOut`.

### Tests

- Les sorties réelles de `lsof` et `ps` sont enregistrées dans `tests/fixtures/`
  (un serveur Next.js lancé par `npm run dev`, un Vite, un Postgres de Homebrew,
  un port tenu par root), et les fonctions pures sont testées dessus.
- Chaîne node : s'arrête au shell, s'arrête à un parent disparu, un processus
  hors famille donne une chaîne d'un seul élément.
- Règle critique : chaque raison, et l'ordre entre elles.
- Lecture des arguments des deux commandes, cas invalides compris.
- Les exemples JSON lus par les tests de l'app (`OWL_UPDATE_FIXTURES=1`) gagnent
  `ports.json` et `stop.json`.
- L'envoi réel des signaux n'a pas de test automatique, comme le dessin.

## Partie 2 — l'app de la barre des menus

### Les onglets

En haut du panneau, deux onglets : « Pull requests » (l'écran actuel, inchangé) et
« Servers ». ⌘1 et ⌘2 passent de l'un à l'autre. L'app s'ouvre sur « Pull
requests ».

### L'onglet Servers

En haut, un champ « Port » et le bouton de rechargement. Dessous, une ligne par
serveur : le port en gros caractères, le projet, la commande et la durée depuis le
démarrage (« 2h », au même format que l'âge des pull requests), et un bouton
« Stop » à droite. Survoler le projet montre `directory`. Un projet inconnu
s'affiche « — ».

Liste vide : « No node server running ». Échec de `owl` : le message à la place
de la liste, comme pour les pull requests.

### Couper depuis la liste

Un clic sur « Stop » appelle `owl stop` aussitôt. La ligne affiche
« Stopping… » et son bouton se désactive. En cas de succès, la liste se recharge.
En cas d'échec, le message s'affiche au pied du panneau, à l'endroit du message
bref existant, et la liste se recharge.

### La recherche

Taper un port et Entrée lance `owl ports <port>` et affiche, à la place de la
liste, ce qui écoute ce port, avec la même forme de ligne. Rien n'écoute :
« Nothing listens on port 5432 ». Port occupé par un processus invisible
(`hidden`) : « Port 22 is used by a process owl cannot see (system or another
user). ». Un texte qui n'est pas un port valide :
« Not a port: abc ». Échap ou un champ vidé reviennent à la liste.

Le bouton « Stop » d'un résultat de recherche ouvre toujours une fenêtre de
confirmation au centre du panneau, sur le modèle de la fenêtre de fusion :

- titre « Stop <command> on port <port>? », puis le projet ou la commande
  complète ;
- port critique : cadre et titre en rouge, et une ligne « Critical: <reason> » ;
- boutons « Cancel » et « Stop ». Entrée confirme, Échap ou un clic hors de la
  fenêtre annulent ;
- pendant l'arrêt, un indicateur d'attente ; en cas d'échec, le message dans la
  fenêtre ; en cas de succès, la fenêtre se ferme et la recherche est relancée.

### Rafraîchissement

`owl ports` est lancé à chaque passage sur l'onglet Servers, à chaque ouverture du
panneau quand cet onglet est affiché, et au bouton de rechargement. Une lecture en
cours n'est jamais doublée. Aucune lecture en tâche de fond.

### Organisation du code Swift

Même règle que pour les pull requests : `OwlBarCore` décide, `OwlBar` dessine.

- `OwlBarCore` : les modèles et la lecture de `ports` et `stop` ; l'onglet
  courant ; un état des serveurs (liste, recherche, lignes en cours d'arrêt,
  fenêtre de confirmation) ; la présentation des lignes et de la fenêtre
  (textes, durée, couleur du cadre) ; l'aiguillage des touches selon l'onglet.
- `OwlBar` : la barre d'onglets, la vue de l'onglet Servers et la fenêtre de
  confirmation, qui ne font que dessiner et transmettre les clics et les touches.

Tests de `OwlBarCore`, avec un faux `owl` : lecture des exemples JSON, lignes
présentées, liste vide, recherche (trouvée, vide, invalide), arrêt réussi et
raté depuis la liste, confirmation obligatoire depuis la recherche, cadre rouge
pour un port critique, lecture jamais doublée, ⌘1 et ⌘2.

## Documentation

- `DESCRIPTION.md` : une section sur l'onglet Servers. La phrase « Il lit et il
  fusionne, rien d'autre » ne vaut plus que pour les
  pull requests.
- `CLAUDE.md` : le module `ports` dans les règles d'architecture, et `cli` qui en
  dépend.

## Hors périmètre

Pas de serveurs autres que node dans la liste, pas de lancement ni de relance de
serveur, pas de journal du serveur, pas d'arrêt de plusieurs serveurs d'un coup,
pas de notification quand un serveur s'arrête, pas d'onglet dans la TUI.
