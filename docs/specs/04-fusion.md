# 04 — Fusion

## Objet

Décrit ce qui se passe entre l'appui sur `m` et la pull request fusionnée.

## Exigence

`owl` doit fusionner en respectant la règle du dépôt. Un dépôt qui n'autorise que
l'écrasement des commits ne doit jamais se voir proposer autre chose. Quand plusieurs
méthodes sont autorisées, `owl` demande laquelle.

## Source de la règle

Les trois drapeaux `squashMergeAllowed`, `mergeCommitAllowed` et
`rebaseMergeAllowed` arrivent dans la requête de liste, portés par
`PrSummary::repo_rules`. Aucune requête supplémentaire n'est nécessaire pour savoir
quoi proposer.

## Contrôles avant fusion

L'appui sur `m` ne lance rien tant que ces contrôles ne sont pas passés. Chaque refus
affiche son motif dans la barre d'état et n'ouvre pas la fenêtre :

1. La PR est en brouillon → « Pull request en brouillon, elle doit être publiée. »
2. La PR a des conflits (`MergeableState::Conflicting`) → « Conflits à résoudre. »
3. L'état de fusion est inconnu (`Unknown`) → « État de fusion en cours de calcul,
   réessaie dans un instant. » GitHub calcule ce champ à la demande ; c'est une
   attente, pas une erreur.
4. Aucune méthode n'est autorisée par le dépôt → « Aucune méthode de fusion
   autorisée sur ce dépôt. »

L'état des vérifications et des relectures n'est pas un motif de refus côté `owl`.
Les protections de branche sont la seule autorité sur ce point, et c'est GitHub qui
les applique : `owl` transmet la demande et rapporte le refus s'il y en a un. Dupliquer
cette logique produirait forcément des désaccords.

## Fenêtre de confirmation

Une fenêtre centrée, par-dessus la liste, qui capte tout le clavier.

```
┌─ Fusionner ────────────────────────────────┐
│ org/dépôt #142                             │
│ Corrige la lecture des réglages            │
│                                            │
│ Méthode :                                  │
│   > Écraser les commits (squash)           │
│     Rebaser (rebase)                       │
│                                            │
│ Entrée pour confirmer · Échap pour annuler │
└────────────────────────────────────────────┘
```

Seules les méthodes autorisées par le dépôt sont listées, dans l'ordre écrasement,
rebasage, commit de fusion — c'est `RepoMergeRules::allowed()` qui rend cet ordre,
et nulle part ailleurs qu'il est recalculé. La sélection initiale est
`preferred_merge_method` des réglages si cette méthode est autorisée, sinon la
première de la liste.

Quand une seule méthode est autorisée, la liste est remplacée par une ligne
« Méthode : écraser les commits (imposé par le dépôt) », et la fenêtre ne demande plus
que la confirmation.

`Entrée` confirme, `Échap` annule, les flèches haut et bas changent de méthode sans
boucler. Aucune autre touche n'agit — sauf `Ctrl-C`, qui quitte `owl` même fenêtre
ouverte : le mode brut l'a désarmée, et c'est à `owl` de l'honorer, sans quoi la
seule sortie serait de tuer le terminal. `q`, lui, ne quitte pas tant que la fenêtre
est ouverte. Tant qu'elle l'est, la barre d'état affiche
« ↑↓ choisir · Entrée confirmer · Échap annuler » à la place de l'aide clavier
habituelle.

```rust
struct MergeDialog {
    key: PrKey,
    title: String,
    methods: Vec<MergeMethod>,   // uniquement celles autorisées, dans l'ordre ci-dessus
    selected: usize,
    state: MergeDialogState,
}

impl MergeDialog {
    fn method(&self) -> Option<MergeMethod>;   // méthode sous le curseur
}

enum MergeDialogState { Choosing, Submitting, Failed(String) }
```

`App` porte `merge: Option<MergeDialog>` et `notice: Option<String>`. Tant que
`merge` contient une fenêtre, elle capte tout le clavier et le rafraîchissement
automatique ne touche plus à la liste. `notice` porte les motifs de refus de `m`
et l'annonce d'une fusion réussie ; il s'affiche dans la barre d'état au même rang
que l'erreur de GitHub — sinon le rafraîchissement qui suit une fusion réussie
effacerait aussitôt son annonce — et s'efface au premier appui sur une touche, une
fois la fenêtre fermée.

Libellés exacts des méthodes dans la liste : « Écraser les commits (squash) »,
« Rebaser (rebase) », « Créer un commit de fusion (merge) ». La ligne à méthode
unique et l'état `Submitting` utilisent la forme courte, sans capitale ni
parenthèse : « écraser les commits », « rebaser », « créer un commit de fusion ».

`app` expose la fenêtre sous la forme d'un `MergeRender { title, lines }` : un
titre de cadre et des lignes déjà écrites, chevron de sélection compris. Toute la
composition — le chevron, les libellés, la phrase « imposé par le dépôt », le
message d'attente — est décidée dans `app/render.rs`. `merge_render` reçoit la
largeur disponible, exactement comme `status_line(width)`, et replie lui-même
chaque ligne contre une largeur de contenu bornée — sur les limites de mots quand
c'est possible, sans jamais perdre de contenu — pour qu'un message de GitHub trop
long pour tenir sur une ligne s'affiche entier, replié, plutôt que tronqué.
`ui/merge.rs` ne calcule que la taille et le centrage des lignes déjà repliées,
efface le fond et dessine le cadre.

## Déroulement de la fusion

À la confirmation, la fenêtre passe en `Submitting` et affiche « Fusion en
cours… » sans se fermer — fermer la fenêtre pendant l'appel donnerait l'impression
que c'est fini. Aucune touche n'agit tant qu'elle est dans cet état, `Échap`
comprise.

La mutation a besoin de l'identifiant GraphQL de la PR, absent de la requête de
liste. C'est `github::merge_pull_request` qui fait l'enchaînement « détail puis
mutation » : il reçoit un `node_id: Option<String>` et, quand il vaut `None`,
récupère d'abord le détail de la PR pour en prendre l'identifiant, avant
d'enchaîner la mutation. Cet enchaînement est invisible pour l'utilisateur,
hormis un temps d'attente un peu plus long. Le détail ainsi récupéré n'entre pas
dans le cache de `app` : le rafraîchissement qui suit une fusion réussie le
rendrait aussitôt périmé.

Si la pull request visée a disparu de la liste et du cache entre l'ouverture de
la fenêtre et la confirmation — une réponse de liste déjà en vol au moment de
l'ouverture suffit —, il n'y a plus rien à fusionner : la fenêtre se ferme et la
notice affiche « Pull request introuvable. ». Rester silencieux donnerait
l'impression que `Entrée` ne fait rien.

Aucun compteur de génération ne protège cet appel : une seule fusion peut être en
vol, la fenêtre bloquant le clavier pendant l'attente. Un `MergeFinished` dont la
clé ne correspond pas à la fenêtre ouverte est simplement ignoré.

En cas de succès : la fenêtre se ferme, la notice affiche
« org/dépôt #142 fusionnée », et une requête de liste est lancée immédiatement. La PR
disparaît de la liste au rafraîchissement, et la sélection suit la règle de
`03-affichage-et-navigation.md`.

En cas d'échec : la fenêtre passe en `Failed` et affiche le message d'erreur de
GitHub tel quel, suivi de « Entrée pour réessayer · Échap pour fermer ». C'est
délibéré — « Base branch was modified » ou « At least 1 approving review is
required » disent exactement quoi faire, là où un message maison brouillerait la
cause. `Échap` ferme, `Entrée` réessaie avec la même méthode.

## Suppression de la branche

`owl` ne demande jamais la suppression de la branche. Elle suit le réglage
`deleteBranchOnMerge` du dépôt, que GitHub applique lui-même après la fusion. Aucun
code de `owl` n'est concerné ; le drapeau est lu uniquement pour pouvoir, plus tard,
en informer l'utilisateur si besoin.

## Ce qui n'est pas prévu

Pas de fusion de plusieurs PR d'un coup, pas de fusion automatique en attente des
vérifications, pas de modification du titre ou du message de commit de fusion. Une
fusion, une confirmation.

## Critères de réussite

- Un dépôt n'autorisant que l'écrasement ne propose jamais le rebasage ni le commit
  de fusion.
- Un dépôt qui en autorise trois les propose toutes, avec la méthode préférée
  présélectionnée.
- Une méthode préférée non autorisée par le dépôt ne bloque rien : la première
  méthode autorisée est présélectionnée.
- `m` sur une PR en brouillon, en conflit, ou sur un dépôt sans méthode autorisée
  n'ouvre pas la fenêtre et affiche le motif.
- `Échap` en état `Choosing` ferme la fenêtre sans aucun appel.
- Une fusion réussie déclenche un rafraîchissement de la liste.
- Une fusion échouée laisse la fenêtre ouverte avec le message de GitHub, et la PR
  reste dans la liste.
- Confirmer sur une PR disparue entre-temps ferme la fenêtre, n'émet aucun appel et
  affiche « Pull request introuvable. ».
