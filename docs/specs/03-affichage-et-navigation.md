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
    details: HashMap<PrKey, PrDetail>,   // cache de la session
    loading: Loading,
    error: Option<String>,
    rate_limit: Option<RateLimit>,
    merge: Option<MergeDialog>,    // voir 04-fusion.md
    config: Config,
    generation: u64,
    should_quit: bool,
}

enum View { List, Detail { key: PrKey, scroll: u16 } }

struct Loading { list: bool, detail: bool }
```

`app` ne fait aucun appel réseau. Il reçoit des événements et renvoie, le cas
échéant, des demandes que la boucle principale exécute :

```rust
enum Event { Key(KeyEvent), Tick, ListLoaded(u64, Result<Vec<PrSummary>>), DetailLoaded(u64, PrKey, Result<PrDetail>), Merged(u64, PrKey, Result<()>) }

enum Command { FetchList, FetchDetail(PrKey), Merge(PrKey, MergeMethod), Quit }

impl App {
    fn handle(&mut self, event: Event) -> Vec<Command> { … }
}
```

Cette forme — un état, des événements entrants, des commandes sortantes — est ce qui
rend la navigation testable sans terminal ni réseau.

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
n'affiche rien, puisque GitHub est peut-être encore en train de le calculer.

Le titre est tronqué à la largeur disponible. Le nom du dépôt n'est jamais tronqué :
c'est lui qui permet de s'orienter. Si la fenêtre est trop étroite pour tenir le
dépôt et le numéro, `owl` affiche un message demandant d'élargir le terminal plutôt
qu'un affichage illisible.

L'ordre d'affichage est celui renvoyé par GitHub, donc par date de mise à jour
décroissante. Il n'y a pas de tri dans `owl`.

Une barre en bas de l'écran indique le nombre de PR, l'état du chargement, l'heure du
dernier rafraîchissement réussi, et l'erreur en cours s'il y en a une.

Une liste vide affiche « Aucune pull request » avec un rappel des filtres actifs —
sans quoi un filtre trop restrictif ressemble à une panne.

## Vue détail

De haut en bas : le titre avec le dépôt et le numéro ; une ligne d'auteur et de
branches (`de <head> vers <base>`) ; les mêmes états que la liste, en clair cette
fois ; la description de la PR ; la liste des vérifications, une par ligne avec son
résultat ; les relectures et les commentaires, dans l'ordre chronologique ; les
fichiers modifiés avec leur nombre de lignes ajoutées et retirées.

Le tout est une seule zone qui défile, pas un ensemble de panneaux. C'est plus simple
à écrire et plus lisible dans un terminal étroit.

Tant que la requête de détail n'a pas répondu, l'en-tête est affiché — il vient de
`PrSummary`, déjà en mémoire — et le reste indique « chargement ». Une PR déjà
consultée pendant la session s'affiche immédiatement depuis le cache, et sa requête
n'est pas relancée sauf appui sur `r`.

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

Les fondations laissent une seule vue et un clavier réduit : `app::Key` ne
distingue que `Char` et `Other`, `ui::draw` dessine toujours la liste, et
`ui/detail.rs` est vide. Cette spec étend `Key` aux flèches et aux touches
d'action, ajoute la vue courante à `App`, et fait de `ui::draw` un véritable
aiguillage.

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
