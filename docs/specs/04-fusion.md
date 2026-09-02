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

Seules les méthodes autorisées par le dépôt sont listées. La sélection initiale est
`preferred_merge_method` des réglages si cette méthode est autorisée, sinon la
première de la liste dans l'ordre écrasement, rebasage, commit de fusion.

Quand une seule méthode est autorisée, la liste est remplacée par une ligne
« Méthode : écraser les commits (imposé par le dépôt) », et la fenêtre ne demande plus
que la confirmation.

`Entrée` confirme, `Échap` annule, les flèches haut et bas changent de méthode. Aucune
autre touche n'agit.

```rust
struct MergeDialog {
    key: PrKey,
    title: String,
    methods: Vec<MergeMethod>,   // uniquement celles autorisées
    selected: usize,
    state: MergeDialogState,
}

enum MergeDialogState { Choosing, Submitting, Failed(String) }
```

## Déroulement de la fusion

À la confirmation, la fenêtre passe en `Submitting` et affiche « fusion en cours »
sans se fermer — fermer la fenêtre pendant l'appel donnerait l'impression que c'est
fini.

La mutation a besoin de l'identifiant GraphQL de la PR, absent de la requête de
liste. Si le détail de la PR est en cache, l'identifiant y est. Sinon, `owl` le
récupère d'abord par la requête de détail, puis enchaîne la mutation. Cet
enchaînement est invisible pour l'utilisateur, hormis un temps d'attente un peu plus
long.

En cas de succès : la fenêtre se ferme, la barre d'état affiche
« org/dépôt #142 fusionnée », et une requête de liste est lancée immédiatement. La PR
disparaît de la liste au rafraîchissement, et la sélection suit la règle de
`03-affichage-et-navigation.md`.

En cas d'échec : la fenêtre passe en `Failed` et affiche le message d'erreur de
GitHub tel quel. C'est délibéré — « Base branch was modified » ou « At least 1
approving review is required » disent exactement quoi faire, là où un message maison
brouillerait la cause. `Échap` ferme, `Entrée` réessaie avec la même méthode.

## Suppression de la branche

`owl` ne demande jamais la suppression de la branche. Elle suit le réglage
`deleteBranchOnMerge` du dépôt, que GitHub applique lui-même après la fusion. Aucun
code de `owl` n'est concerné ; le drapeau est lu uniquement pour pouvoir, plus tard,
en informer l'utilisateur si besoin.

## Ce qui n'est pas prévu

Pas de fusion de plusieurs PR d'un coup, pas de fusion automatique en attente des
vérifications, pas de modification du titre ou du message de commit de fusion. Une
fusion, une confirmation.

## Note d'implémentation

Les fondations lisent `preferred_merge_method` dans les réglages mais ne s'en
servent pas encore : le champ porte un `#[allow(dead_code)]` dans
`config::Config`. Cette spec l'utilise et retire l'attribut. `ui/merge.rs` est
vide jusque-là.

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
