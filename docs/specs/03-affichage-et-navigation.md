# 03 — Affichage et navigation

## Objet

Décrit l'état de l'application, les deux vues, le clavier et le rafraîchissement.

## État de l'application

```rust
struct App {
    view: View,
    prs: Vec<PrSummary>,
    selected: usize,
    selected_key: Option<PrKey>,   // sert à retrouver la sélection après rafraîchissement
    details: HashMap<PrKey, CachedDetail>,   // cache de la session
    loading: Loading,
    error: Option<String>,
    rate_limit: Option<RateLimit>,
    last_refresh: Option<DateTime<Local>>,
    filters: Vec<Filter>,          // les filtres des réglages, traduits une seule fois
    config: Config,
    list_generation: Generation,
    detail_generation: Generation,
    should_quit: bool,
}

enum View { List, Detail { key: PrKey, scroll: u16 } }

struct Loading { list: bool, detail: bool }

struct CachedDetail { detail: PrDetail, loaded_at: DateTime<Local> }
```

Les deux compteurs de génération sont indépendants. Un compteur unique ferait
qu'ouvrir un détail périmerait une requête de liste en vol : son résultat serait
jeté et `loading.list` resterait bloqué à `true`.

Le cache retient l'heure de chargement de chaque détail, affichée en fin de vue
détail : un détail peut être périmé sans qu'on le sache, autant dire quand il a été
lu.

`app` ne fait aucun appel réseau. Il reçoit des événements et renvoie, le cas
échéant, des demandes que la boucle principale exécute :

```rust
enum Event {
    Key(Key),
    Tick,
    Resize,
    Quit,
    ListLoaded { generation: Generation, result: Result<ListPage> },
    DetailLoaded { generation: Generation, key: PrKey, result: Result<PrDetail> },
}

enum Command {
    FetchList { generation: Generation, query: String, page_size: u16 },
    FetchDetail { generation: Generation, summary: PrSummary },
    OpenInBrowser { url: String },
    Quit,
}

impl App {
    fn handle(&mut self, event: Event) -> Vec<Command> { … }
}
```

`Event::ListLoaded` transporte un `ListPage` : les pull requests et le solde
d'appels lu au passage voyagent ensemble depuis `01-modele-et-donnees.md`, les
séparer imposerait un second canal pour la même réponse.

`Event::Resize` dit que le terminal a changé de taille. `app` n'en fait rien : il
ne renvoie aucune commande, ne touche pas à l'état et n'efface pas le message en
cours — un redimensionnement n'est pas un appui sur une touche. Il existe pour que
la boucle principale redessine, puisqu'elle ne redessine qu'après un événement.
Sans lui, l'écran resterait figé à l'ancienne taille jusqu'à la touche ou au tour
de minuteur suivant.

`Event::Quit` est l'arrêt demandé par la boucle principale : le crochet de panique
en a besoin pour débloquer la boucle après avoir rendu le terminal.

`Command::FetchDetail` porte le résumé entier et pas la seule clé : `github` le
recopie dans le `PrDetail` qu'il rend, et la boucle principale n'a pas la liste où
retrouver la pull request.

`Command::OpenInBrowser` sert la touche `o` : `app` choisit l'URL, la boucle
principale l'ouvre. `app` ne fait aucun effet de bord lui-même.

Cette forme — un état, des événements entrants, des commandes sortantes — est ce qui
rend la navigation testable sans terminal ni réseau.

## Ce que `app` donne à dessiner

`ui` ne compose rien. `app` rend la liste sous la forme d'un `ListRender` — soit les
lignes prêtes à dessiner, soit le message de liste vide avec le rappel des filtres,
soit le message de terminal trop étroit — et le détail sous la forme d'un
`Vec<DetailLine>`. Chaque élément porte un `Tone` que `ui` traduit en couleur, et
c'est la seule traduction que `ui` fait. Les titres des deux cadres sont eux aussi
des constantes de `app` : un titre est un message.

Une ligne de liste est une suite de morceaux, pas une chaîne unique :

```rust
struct ListRow { checks: Glyph, review: Glyph, cells: Vec<Cell>, dim: bool }

struct Cell { text: String, tone: Option<Tone> }   // ton absent : couleur par défaut

enum Tone { Green, Red, Yellow, Gray, Cyan, Blue }
```

Le découpage en morceaux est ce qui permet de colorer chaque colonne sans que `ui`
ait à retrouver où elle commence : le remplissage est déjà posé, `ui` met les
morceaux bout à bout.

Les largeurs se mesurent en caractères, pas en colonnes de terminal : mesurer les
colonnes réellement occupées demanderait une dépendance de plus. Un titre en
idéogrammes est donc tronqué un peu tard ; c'est le seul cas concerné.

## Clavier

| Touche | Vue liste | Vue détail |
|---|---|---|
| Flèche haut, `k` | sélection précédente | défilement vers le haut |
| Flèche bas, `j` | sélection suivante | défilement vers le bas |
| Flèche droite, `Entrée` | ouvre le détail | — |
| Flèche gauche, `Échap` | — | revient à la liste |
| `m` | ouvre la fenêtre de fusion | ouvre la fenêtre de fusion |
| `r` | rafraîchit la liste | rafraîchit le détail affiché |
| `o` | ouvre la PR dans le navigateur | idem |
| `q` | quitte | quitte |
| `Ctrl+C` | quitte | quitte |

La sélection ne boucle pas : en haut de liste, la flèche haut ne fait rien.

Quand la fenêtre de fusion est ouverte, elle capte tout le clavier ; les touches
ci-dessus sont inactives jusqu'à sa fermeture, `Ctrl+C` exceptée : elle quitte
`owl` même fenêtre ouverte, `q` non. Pendant ce temps, l'aide clavier de la barre
d'état devient « ↑↓ choose · Enter confirm · Esc cancel ».

## Vue liste

Le cadre porte le titre « Owl - Monitoring pull requests ».

Une ligne par pull request, sur une seule ligne de terminal, dans cet ordre :
pictogrammes, dépôt, numéro, âge, branche cible, titre.

```
→ ✓ ✓  org/depot │ #142   3h   develop  Fix settings loading
  ✗ ●  org/other │ #7     34m  main     Add the purge command
  ○ ·  org/depot │ #150   2d   develop  [draft] Try caching
  ✓ ✓  org/third │ #31    7h   main     ⚠ Rename the render module
```

La ligne sélectionnée est marquée par une flèche à gauche, et par rien d'autre :
inverser ses couleurs ferait des tons de ses colonnes autant de fonds colorés, et la
ligne cesserait de se lire comme les autres.

Deux colonnes de pictogrammes, à largeur fixe, avant le texte.

Une barre verticale sépare le dépôt du numéro : les deux se lisent ensemble, et une
simple espace les confondrait.

L'âge est celui de la dernière mise à jour, en une poignée de caractères : `m` pour
les minutes en dessous d'une heure, `h` pour les heures en dessous d'un jour, `d`
au-delà — `34m`, `7h`, `3d`. Une date dans le futur, horloges désaccordées, donne
`0m` plutôt qu'un nombre négatif.

La branche cible est celle visée par la fusion. Elle vient avec la liste, sans
requête supplémentaire.

Chaque colonne porte son ton, et c'est là toute la couleur de la ligne :

| Colonne | Ton |
|---|---|
| Dépôt | cyan |
| Séparateur et numéro | gris |
| Âge | gris |
| Branche cible | bleu |
| Titre | couleur par défaut du terminal |

Vérifications :

| Pictogramme | Sens |
|---|---|
| `✓` vert | toutes les vérifications passent |
| `✗` rouge | au moins une vérification échoue |
| `○` jaune | vérifications en cours |
| `·` gris | aucune vérification |

Relectures :

| Pictogramme | Sens |
|---|---|
| `✓` vert | approuvée |
| `✗` rouge | changements demandés |
| `●` jaune | relecture attendue |
| `·` gris | rien à signaler |

Le préfixe `[brouillon]` marque les PR en brouillon, et la ligne est grisée. Le
symbole `⚠` devant le titre signale un conflit de fusion. Un état de fusion inconnu
n'affiche rien, puisque GitHub est peut-être encore en train de le calculer. Les deux
marques se cumulent dans cet ordre — `[brouillon] ⚠ Titre` — le brouillon d'abord
parce qu'il qualifie la pull request, le conflit ensuite parce qu'il qualifie la
fusion.

Le titre est tronqué à la largeur disponible. Le nom du dépôt n'est jamais tronqué :
c'est lui qui permet de s'orienter. Si la fenêtre est trop étroite pour tenir le
dépôt et le numéro, `owl` affiche à la place de la liste : « Widen the terminal: the
repository and the number do not fit. »

Entre les deux, les colonnes sont abandonnées entières plutôt que coupées, dans cet
ordre : le titre rétrécit d'abord, puis la branche cible disparaît, puis l'âge. Une
colonne à moitié coupée n'apprend rien ; une colonne absente se remarque.

La liste défile quand elle dépasse la hauteur de la fenêtre, en suivant la sélection.
Le défilement est une affaire de dessin : `ui` le recalcule à chaque image depuis
`selected` et ne retient rien entre deux dessins.

L'ordre d'affichage est celui renvoyé par GitHub, donc par date de mise à jour
décroissante. Il n'y a pas de tri dans `owl`.

Une barre en bas de l'écran indique le nombre de PR, l'état du chargement, l'heure du
dernier rafraîchissement réussi, et l'erreur en cours s'il y en a une. Elle se termine
par l'aide clavier de la vue affichée : une touche qui ne fait rien dans la vue
courante n'y est pas rappelée.

Cette barre ne dépasse jamais la largeur de la fenêtre, et c'est `app` qui décide
de sa coupe : quand la place manque, les morceaux sont retirés entiers, du moins
important au plus important. L'aide clavier part la première — c'est un rappel, pas
une information — puis l'heure du dernier rafraîchissement, puis le résumé de la
liste, puis l'annonce du chargement. L'erreur en cours est ce qui reste en dernier.

Une liste vide affiche « No pull requests » avec un rappel des filtres actifs — sans
quoi un filtre trop restrictif ressemble à une panne.

## Vue détail

Le cadre porte le titre « Owl - Pull request details ».

En haut, un en-tête encadré : dépôt et numéro, titre, puis auteur et âge. Tout y
vient du résumé déjà en mémoire, ce qui permet de l'afficher avant la réponse de
détail. La branche cible n'y est pas reprise : la section « Branches » la donne, avec
celle d'où part la pull request. Le cadre ne dépasse pas soixante-douze colonnes : sur un terminal
large, un cadre qui suit toute la largeur encombre plus qu'il ne sépare.

En dessous, des sections titrées, séparées par une ligne vide, dans cet ordre :

| Section | Contenu |
|---|---|
| `Branches` | `<head> -> <base>` |
| `Status` | les mêmes états que la liste, en clair, pictogramme compris — vérifications, relectures, puis état de fusion |
| `Description` | la description de la PR |
| `Checks (n)` | les vérifications, une par ligne avec son résultat |
| `Reviews and comments` | les relectures et les commentaires, dans l'ordre chronologique |
| `Files changed (n) · +a -r` | les fichiers modifiés et leurs compteurs |

Une dernière ligne, en gris, donne l'heure de chargement du détail.

Le titre d'une section est teinté et n'est pas indenté ; son contenu l'est de trois
espaces. C'est ce décalage, avec la ligne vide, qui fait lire les sections comme des
blocs plutôt que comme une suite de lignes.

Le tout est une seule zone qui défile, pas un ensemble de panneaux. C'est plus simple
à écrire et plus lisible dans un terminal étroit.

La vue détail ne renvoie pas à la ligne : une ligne logique vaut une ligne d'écran.
`app` peut donc les compter et borner le défilement sans connaître la largeur ni la
hauteur. Les lignes trop longues sont tronquées comme celles de la liste, et `o` ouvre
la pull request dans le navigateur pour lire une description entière.

Tant que la requête de détail n'a pas répondu, l'en-tête encadré est affiché — il
vient de `PrSummary`, déjà en mémoire — et le reste indique « Loading details… ». Une
PR déjà consultée pendant la session s'affiche immédiatement depuis le cache, et sa
requête n'est pas relancée sauf appui sur `r`.

Si la pull request affichée a quitté la liste, son en-tête est repris du résumé porté
par le détail en cache. Un cache qui devient inaffichable dès qu'un rafraîchissement
retire la PR ne sert à rien.

Le diff n'est pas affiché ligne à ligne : seuls les chemins et les compteurs le sont.
Afficher un diff coloré dans un terminal est un projet à lui seul, et `owl` a la
touche `o` pour ouvrir la PR dans le navigateur.

## Rafraîchissement

Un minuteur émet un `Tick` toutes les `refresh_interval` secondes, soixante par
défaut. Un `Tick` déclenche une requête de liste, sauf si une requête de liste est
déjà en cours, ou si la fenêtre de fusion est ouverte — on ne change pas la liste
sous les pieds d'une confirmation.

Un intervalle réglé à zéro désactive le minuteur ; seule la touche `r` rafraîchit.

Le rafraîchissement préserve la sélection. `app` retient `selected_key`, la cherche
dans la nouvelle liste et y replace la sélection. Si la PR a disparu, la sélection
reprend l'indice précédent, borné à la nouvelle taille de liste ; sur une liste
devenue vide, il n'y a plus de sélection.

Le rafraîchissement ne vide pas le cache des détails. Une PR mise à jour verra son
détail périmé jusqu'à un appui sur `r` dans la vue détail. C'est un compromis
assumé : il évite des requêtes inutiles, et le détail porte l'heure de son
chargement.

## Sortie du terminal

Le mode brut et l'écran alterné sont restaurés à la sortie, y compris en cas de
panique du programme et sur `Ctrl+C`. Un terminal cassé après un plantage est
considéré comme un défaut.

## Critères de réussite

Tous vérifiables sans terminal, en envoyant des événements à `App` :

- Les flèches haut et bas déplacent la sélection et ne débordent pas des extrémités.
- La flèche droite passe en vue détail et émet `FetchDetail` ; la flèche gauche
  revient à la liste.
- Ouvrir une PR déjà en cache n'émet aucune commande.
- Après un rafraîchissement où la PR sélectionnée est toujours présente mais à un
  autre indice, la sélection la suit.
- Après un rafraîchissement où la PR sélectionnée a disparu, la sélection reste dans
  les bornes de la nouvelle liste.
- Un résultat de liste portant une génération périmée est ignoré.
- Un `Tick` reçu pendant un chargement de liste n'émet pas de seconde requête.
- Un `Resize` n'émet aucune commande, ne déplace pas la sélection, ne change pas de
  vue et n'efface pas le message en cours.
- Une ligne de liste porte le dépôt, le numéro, l'âge, la branche cible et le titre,
  chacun avec son ton, et le dépôt est séparé du numéro par une barre verticale.
- L'âge se lit en minutes, en heures puis en jours selon l'ancienneté, et une date
  dans le futur donne `0m`.
- Réduite, la liste abandonne la branche cible puis l'âge sans jamais tronquer le
  dépôt ni le numéro.
- La vue détail encadre son en-tête et titre ses sections.
- Aucun texte affiché ne porte de tiret cadratin.
