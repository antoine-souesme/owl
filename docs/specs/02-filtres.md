# 02 — Filtres

## Objet

Décrit comment `owl` choisit les pull requests à afficher, et comment ajouter un
filtre sans toucher au reste du programme.

## Exigence

Le comportement par défaut est « mes pull requests ouvertes ». Mais l'architecture
doit rendre l'ajout d'un filtre trivial : un nouveau filtre ne doit modifier qu'un
seul fichier, `filter.rs`, et rien d'autre.

## Principe

L'API GitHub sait déjà filtrer, via la syntaxe de recherche. `owl` ne filtre donc pas
en mémoire : il assemble une chaîne de recherche et laisse GitHub travailler. Un
filtre est simplement un objet qui sait produire son fragment de chaîne.

```rust
enum Filter {
    AuthoredByMe,
    ReviewRequestedFromMe,
    AssignedToMe,
    Open,
    Draft(bool),
    Org(String),
    Repo(String),
    Label(String),
    Raw(String),
}

impl Filter {
    /// Fragment de requête de recherche GitHub.
    fn fragment(&self) -> String { … }
}

/// Assemble les fragments et garantit la présence de `is:pr`.
fn build_query(filters: &[Filter]) -> String { … }
```

Correspondances :

| Filtre | Fragment |
|---|---|
| `AuthoredByMe` | `author:@me` |
| `ReviewRequestedFromMe` | `review-requested:@me` |
| `AssignedToMe` | `assignee:@me` |
| `Open` | `is:open` |
| `Draft(true)` | `draft:true` |
| `Draft(false)` | `draft:false` |
| `Org(o)` | `org:o` |
| `Repo(r)` | `repo:r` |
| `Label(l)` | `label:"l"` |
| `Raw(s)` | `s` tel quel |

`build_query` ajoute toujours `is:pr` en tête, quels que soient les filtres, et
ajoute `sort:updated-desc` en queue pour que les PR récemment actives arrivent en
premier.

Exemple : les filtres par défaut donnent
`is:pr author:@me is:open sort:updated-desc`.

## Ajouter un filtre

Trois modifications, toutes dans `filter.rs` : une variante d'énumération, une ligne
dans `fragment`, une ligne dans la lecture depuis le fichier de réglages. Aucun autre
fichier n'est concerné.

`Raw` sert de soupape : elle permet d'écrire n'importe quelle expression de recherche
GitHub dans les réglages sans attendre qu'un filtre dédié existe.

## Lecture depuis les réglages

Le fichier de réglages contient les filtres sous forme de chaînes, dans la syntaxe
GitHub elle-même :

```toml
filters = ["author:@me", "is:open"]
```

Chaque chaîne est reconnue et traduite en variante de `Filter`. Une chaîne non
reconnue devient un `Filter::Raw`, ce qui la transmet à GitHub telle quelle. Une
liste `filters` vide provoque un arrêt avec un message : sans aucun filtre, la
recherche ramènerait les pull requests du monde entier.

## Ce qui n'est pas prévu

Il n'y a pas de changement de filtre depuis l'écran, ni de touche pour basculer entre
« mes PR » et « à relire ». Les filtres se règlent dans le fichier. Cette porte reste
ouverte : la structure `Filter` et `build_query` suffiront à l'ajouter plus tard, en
ne touchant que `app` et `ui`.

## Note d'implémentation

`config::Config` expose `filters: Vec<String>` : les réglages portent les filtres
sous forme de chaînes, et c'est `app` qui les traduit en variantes de `Filter`, une
seule fois, à sa construction.

`app` assemble la requête avec `build_query` et la transmet dans
`Command::Fetch { generation, query, page_size }`. Le client GitHub reçoit donc une
chaîne toute faite et n'assemble plus rien : l'ancienne jointure `github::search_query`
a disparu, et la syntaxe de recherche ne vit plus qu'ici.

Quatre précisions que le tableau des correspondances ne dit pas :

- `Filter::parse` accepte un libellé avec ou sans guillemets — `label:"bug"` comme
  `label:bug` donnent `Label("bug")`, qui se réécrit `label:"bug"`.
- Un préfixe sans valeur — `org:`, `repo:`, `label:` seuls — n'est pas reconnu et
  part en `Raw`, transmis tel quel.
- `build_query` écarte les fragments vides, et n'enlève aucun doublon : `is:pr`
  écrit dans les réglages apparaît deux fois dans la requête, ce que GitHub accepte.
- `Filter::parse` retire les espaces en début et en fin de chaîne avant toute
  reconnaissance : un `Raw` issu des réglages perd donc lui aussi ses espaces de
  bord — `"  involves:@me  "` donne `Raw("involves:@me")`, malgré le « telle
  quelle » du tableau ci-dessus.

## Critères de réussite

- Une liste de filtres donnée produit exactement la chaîne attendue, `is:pr` en tête
  et `sort:updated-desc` en queue.
- L'ordre des filtres dans les réglages ne change pas l'ensemble des PR ramenées.
- Une chaîne inconnue dans `filters` se retrouve intacte dans la requête.
- Une liste `filters` vide est refusée avec un message explicite.
