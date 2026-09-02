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
c'est la seule traduction que `ui` fait.

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
| `q`, `Ctrl+C` | quitte | quitte |

La sélection ne boucle pas : en haut de liste, la flèche haut ne fait rien.

Quand la fenêtre de fusion est ouverte, elle capte tout le clavier ; les touches
ci-dessus sont inactives jusqu'à sa fermeture.

## Vue liste

Une ligne par pull request, sur une seule ligne de terminal, dans cet ordre :

```
 ✓ ✓  org/dépôt  #142  Corrige la lecture des réglages
 ✗ ●  org/autre  #7    Ajoute la commande de purge
 ○ ·  org/dépôt  #150  [brouillon] Essai de mise en cache
 ✓ ✓  org/tiers  #31   ⚠ Renomme le module de rendu
```

Deux colonnes de pictogrammes, à largeur fixe, avant le texte.

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
dépôt et le numéro, `owl` affiche à la place de la liste : « Élargis le terminal : le
dépôt et le numéro n'y tiennent pas. »

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

Une liste vide affiche « Aucune pull request » avec un rappel des filtres actifs —
sans quoi un filtre trop restrictif ressemble à une panne.

## Vue détail

De haut en bas : le titre avec le dépôt et le numéro ; la ligne d'auteur ; la ligne
des branches (`de <head> vers <base>`) ; les mêmes états que la liste, en clair cette
fois ; la description de la PR ; la liste des vérifications, une par ligne avec son
résultat ; les relectures et les commentaires, dans l'ordre chronologique ; les
fichiers modifiés avec leur nombre de lignes ajoutées et retirées ; enfin l'heure de
chargement du détail.

L'auteur et les branches occupent deux lignes distinctes, et non une seule : l'auteur
vient du résumé déjà en mémoire, les branches n'arrivent qu'avec la réponse de détail.
C'est le seul découpage qui permet d'afficher l'auteur avant la réponse.

Le tout est une seule zone qui défile, pas un ensemble de panneaux. C'est plus simple
à écrire et plus lisible dans un terminal étroit.

La vue détail ne renvoie pas à la ligne : une ligne logique vaut une ligne d'écran.
`app` peut donc les compter et borner le défilement sans connaître la largeur ni la
hauteur. Les lignes trop longues sont tronquées comme celles de la liste, et `o` ouvre
la pull request dans le navigateur pour lire une description entière.

Tant que la requête de détail n'a pas répondu, l'en-tête est affiché — il vient de
`PrSummary`, déjà en mémoire — et le reste indique « chargement ». Une PR déjà
consultée pendant la session s'affiche immédiatement depuis le cache, et sa requête
n'est pas relancée sauf appui sur `r`.

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

## Note d'implémentation

Le champ `merge` de `App` et le blocage du rafraîchissement pendant la fenêtre
de fusion ne sont pas apportés par cette spec : la touche `m` y est reconnue
mais sans effet, et `04-fusion.md` s'en charge.

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
