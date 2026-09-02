# Affichage et navigation — Plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Donner à `owl` ses deux vues et son clavier complet : une liste de pull requests avec pictogrammes d'état, sélection et troncature, une vue détail qui défile et se met en cache, et un rafraîchissement qui préserve la sélection.

**Architecture:** `app` porte la vue courante, la sélection, le cache des détails et les deux compteurs de génération ; il reçoit des événements et rend des commandes. Toute décision d'affichage — pictogramme, préfixe, troncature, message — est composée dans `app` et rendue sous forme de structures prêtes à dessiner (`ListRender`, `DetailLine`). `ui` lit ces structures, traduit un ton en couleur et dessine ; il ne compose plus aucun texte. `app.rs` devient un dossier `app/` : `mod.rs` pour l'état et les événements, `render.rs` pour la composition de l'affichage.

**Tech Stack:** Rust édition 2021, `ratatui` 0.30, `crossterm` 0.29, `chrono`, `open` 5 (déjà en dépendance). Aucune nouvelle dépendance.

**Spec:** `docs/specs/03-affichage-et-navigation.md` (contexte transverse : `docs/specs/00-fondations.md` pour les règles de dépendance et la structure des modules, `docs/specs/01-modele-et-donnees.md` pour `PrSummary` et `PrDetail`, `docs/specs/04-fusion.md` pour la touche `m`, laissée en attente ici)

Aucun lien de design n'a été fourni pour cette spec. La vérification à l'œil de la tâche 5 se fait donc dans le terminal, contre les maquettes en texte de la spec : `claude-in-chrome` est inapplicable — `owl` est une interface en mode texte, pas une page web, et il n'y a aucun compte de test à connecter.

## Conditions d'exécution

- Exécution en **subagent-driven development** : un sous-agent par tâche, revue entre chaque.
- Branche de travail : `feat/affichage-et-navigation`, créée depuis `develop` à la première étape de la tâche 1. Jamais de travail direct sur `develop`, jamais de pull request vers `main`.
- Ce fichier de plan n'est pas encore suivi par git : l'ajouter au premier commit de la branche.
- Le registre `sdd` du dépôt est vide au départ (`.superpowers/sdd/` ne contient que son `.gitignore`) : repartir de zéro, aucun état antérieur à reprendre.
- `docs/suivi/TODO.md` est **hors périmètre** : ses points d'affichage — langue de l'interface, séparateurs, couleurs, titre de la fenêtre — feront l'objet d'un plan à eux. Ne rien en tirer, ne pas le modifier, ne pas le mentionner dans le rapport.
- Aucune question à poser avant de commencer : tout ce qui est nécessaire est dans ce plan et dans la spec. Une question n'est légitime que devant un blocage réel qui empêche la suite — un outil absent, un service indisponible.
- Une décision **mise de côté** se consigne dans `docs/suivi/DETTE.md`, au format déjà en place dans ce fichier, et uniquement si elle est critique pour la suite. Pas les décisions prises, pas les idées d'amélioration, aucune sur-conception.
- À la fin : pull request vers `develop`, obligatoire, puis un rapport qui ne raconte pas le travail fait. Voir la tâche 5, étapes 9 et 10.

## Global Constraints

- Rust, édition 2021. Binaire unique nommé `owl`, aucune sous-commande.
- **Aucune nouvelle dépendance**, ni de production ni de développement.
- Dépendances à sens unique : `model` et `filter` ne connaissent ni le réseau ni le terminal ; `app` ne fait aucun appel réseau et ne dépend pas de `crossterm` ; `ui` lit `app` en lecture seule, ne modifie jamais l'état et ne décide de rien.
- **Aucune composition de texte dans `ui`.** Un `format!` qui assemble un contenu affiché dans `src/ui/` est un défaut. `ui` reçoit des chaînes prêtes et des tons, et n'ajoute que la mise en page et la couleur.
- Pictogrammes des vérifications, au caractère près : `✓` vert si tout passe, `✗` rouge si au moins une échoue, `○` jaune si en cours, `·` gris si aucune.
- Pictogrammes des relectures : `✓` vert approuvée, `✗` rouge changements demandés, `●` jaune relecture attendue, `·` gris rien à signaler.
- Préfixe `[brouillon] ` sur les brouillons, ligne grisée. Symbole `⚠ ` devant le titre si `MergeableState::Conflicting`. `MergeableState::Unknown` n'affiche rien.
- Le nom du dépôt et le numéro ne sont **jamais** tronqués. Seul le titre l'est. Si la largeur ne suffit pas pour le dépôt et le numéro, l'écran affiche un message demandant d'élargir le terminal.
- Aucun tri dans `owl` : l'ordre est celui renvoyé par GitHub.
- La sélection ne boucle pas : en haut de liste, la flèche haut ne fait rien ; en bas, la flèche bas ne fait rien.
- `refresh_interval = 0` désactive le minuteur ; seule la touche `r` rafraîchit. Ce câblage est déjà en place dans `src/main.rs` et ne doit pas régresser.
- Le mode brut et l'écran alterné sont restaurés à la sortie, panique comprise. Le garde et le crochet de panique de `src/main.rs` sont déjà en place et ne doivent pas régresser.
- Le jeton n'est jamais écrit dans un fichier, ni journalisé, ni affiché. Il ne rentre ni dans `app` ni dans `ui`.
- Le projet est en français : messages affichés, commentaires, messages de commit. Les identifiants du code restent en anglais.
- `cargo build`, `cargo test`, `cargo clippy -- -D warnings` et `cargo fmt --check` doivent passer à la fin de chaque tâche. Aucune n'est optionnelle.

## Décisions prises en écrivant ce plan

Elles sortent du texte de la spec. La tâche 5 les reporte dans `docs/specs/03-affichage-et-navigation.md`, comme l'exige l'ordre de vérité du `CLAUDE.md`.

1. **Deux compteurs de génération, pas un.** La spec montre un seul champ `generation`. Avec un compteur partagé, ouvrir un détail périmerait une requête de liste en vol : son résultat serait jeté et `loading.list` resterait bloqué à `true`. `App` porte donc `list_generation` et `detail_generation`, indépendants.
2. **`Event::ListLoaded` transporte un `ListPage`, pas un `Vec<PrSummary>`.** Le solde d'appels voyage avec les données depuis la spec 01 ; le séparer imposerait un second canal pour la même réponse.
3. **`Event::Quit` est conservé.** Il n'est pas dans la liste de la spec, mais le crochet de panique de `src/main.rs` en a besoin pour débloquer la boucle après avoir rendu le terminal.
4. **`Command::FetchDetail` porte le `PrSummary` entier, pas la seule clé.** `github::fetch_detail` prend `&PrSummary` et recopie le résumé déjà affiché dans le `PrDetail` : passer la clé seule obligerait `main` à retrouver la PR dans une liste qu'il n'a pas.
5. **`Command::OpenInBrowser { url }`.** La touche `o` est une commande comme les autres : `app` choisit l'URL, `main` appelle `open`. Sans quoi `app` ferait un effet de bord.
6. **La touche `m` est reconnue mais sans effet.** `MergeDialog` et les contrôles avant fusion appartiennent à `04-fusion.md`. La tâche 4 pose la touche, un champ `merge` absent, et une note dans les deux specs. Le blocage du `Tick` pendant la fenêtre de fusion est câblé par la spec 04, pas ici : aucun critère de réussite de la spec 03 ne le couvre.
7. **La vue détail ne renvoie pas à la ligne.** Une ligne logique vaut une ligne d'écran : `app` peut donc les compter et borner le défilement sans connaître la largeur. Les lignes trop longues sont tronquées comme celles de la liste ; `o` ouvre la PR dans le navigateur pour lire une description entière.
8. **La largeur se mesure en caractères (`chars().count()`).** Mesurer les colonnes réellement occupées demanderait `unicode-width`, une dépendance que les contraintes interdisent. Un titre en idéogrammes sera donc tronqué un peu tard ; c'est le seul cas concerné.
9. **La ligne « … et N de plus » est abandonnée.** La spec 01 laissait la spec 03 trancher. Elle réclame des `totalCount` dans la requête de détail pour afficher un compte que rien n'utilise ; la spec 03 décrit les listes du détail sans jamais mentionner de troncature. La phrase sort de la spec 01 et l'entrée correspondante sort de la dette, à la tâche 5.
10. **`app.rs` devient `app/mod.rs` + `app/render.rs`.** Le fichier dépasse déjà 540 lignes et cette spec y ajoute l'état des deux vues, le clavier complet et toute la composition d'affichage. La structure des modules de `00-fondations.md` est mise à jour à la tâche 5.
11. **La sélection est dessinée avec un `ListState` local.** `ui/list.rs` construit un `ListState` à chaque dessin depuis `app.selected` et le passe à `render_stateful_widget` : c'est ce qui fait défiler la liste quand elle dépasse la hauteur. L'état n'est pas conservé entre deux dessins, donc `ui` ne retient rien.
12. **Un brouillon en conflit affiche `[brouillon] ⚠ Titre`.** Les deux marques se cumulent dans cet ordre, le brouillon d'abord parce qu'il qualifie la PR, le conflit ensuite parce qu'il qualifie la fusion.

## Structure des fichiers

| Fichier | Responsabilité | Tâche |
|---|---|---|
| `src/app/mod.rs` | `View`, `Loading`, `Key`, `Event`, `Command`, `App`, `handle`, sélection, cache des détails, `status_line` | 1, 3, 4 |
| `src/app/render.rs` | `Tone`, `Glyph`, `ListRow`, `ListRender`, `DetailLine`, `list_render`, `detail_lines` | 2, 3 |
| `src/ui/mod.rs` | Aiguillage réel selon `app.view` | 3 |
| `src/ui/list.rs` | Dessine `ListRender` et la barre d'état, sans rien composer | 2 |
| `src/ui/detail.rs` | Dessine `Vec<DetailLine>` avec défilement | 3 |
| `src/main.rs` | Traduit les touches de `crossterm`, exécute `FetchDetail` et `OpenInBrowser` | 1, 3, 4 |
| `docs/specs/03-affichage-et-navigation.md` | Reçoit les décisions prises en cours de route | 5 |
| `docs/specs/01-modele-et-donnees.md` | Perd la ligne « … et N de plus » | 5 |
| `docs/specs/04-fusion.md` | Reçoit la note sur la touche `m` en attente | 4 |
| `docs/specs/00-fondations.md` | Structure des modules mise à jour (`app/`) | 5 |
| `docs/suivi/DETTE.md` | Perd deux entrées résolues | 2, 5 |

`src/app.rs` est supprimé au profit de `src/app/mod.rs` à la tâche 1. `owl` étant un binaire, ses modules ne sont pas accessibles depuis un test d'intégration : tous les tests de `app` sont des tests unitaires dans les fichiers du module.

---

### Task 1: État de la liste, clavier étendu et sélection

**Files:**
- Delete puis Create : `src/app.rs` devient `src/app/mod.rs` (déplacement par `git mv`, contenu ensuite modifié)
- Modify: `src/main.rs` (traduction des touches, exécution de `FetchList`)
- Modify: `src/ui/list.rs` (le champ `items` devient `prs` ; le reste du dessin est réécrit à la tâche 2)

**Interfaces:**
- Consumes: `PrSummary`, `PrKey`, `ListPage`, `RateLimit` de `model` ; `Filter`, `build_query` de `filter` ; `GithubError` de `github` ; `Config` de `config`.
- Produces:
  - `pub enum Key { Up, Down, Left, Right, Enter, Esc, Char(char), CtrlC, Other }`
  - `pub enum Event { Key(Key), Tick, Quit, ListLoaded { generation: Generation, result: Result<ListPage, GithubError> } }`
  - `pub enum Command { FetchList { generation: Generation, query: String, page_size: u16 }, Quit }`
  - `pub struct Loading { pub list: bool, pub detail: bool }`
  - `pub struct App` avec les champs publics `prs: Vec<PrSummary>`, `selected: usize`, `loading: Loading`, `error: Option<String>`, `rate_limit: Option<RateLimit>`, `should_quit: bool`, `last_refresh: Option<DateTime<Local>>`
  - `pub fn App::selected_pr(&self) -> Option<&PrSummary>`
  - `pub fn App::status_line(&self) -> String`

- [ ] **Step 1: Créer la branche de travail et déplacer le module**

```bash
git switch develop
git pull --ff-only
git switch -c feat/affichage-et-navigation
mkdir -p src/app
git mv src/app.rs src/app/mod.rs
cargo build 2>&1 | tail -5
```

Expected: la compilation passe encore. Rust résout `mod app;` vers `src/app/mod.rs` aussi bien que vers `src/app.rs` : le déplacement seul ne casse rien.

- [ ] **Step 2: Écrire les tests qui échouent**

Dans le `mod tests` de `src/app/mod.rs`, appliquer d'abord ces renommages mécaniques sur les tests existants — le nouvel état les impose :

- `Event::Data { generation, result }` devient `Event::ListLoaded { generation, result }` (partout).
- `Command::Fetch { .. }` devient `Command::FetchList { .. }` (partout, y compris dans les `match` des fonctions d'aide).
- `app.items` devient `app.prs` (partout).
- `app.loading` devient `app.loading.list` (partout).
- Dans `une_erreur_laisse_la_liste_affichee`, `assert_eq!(app.status, "Réseau injoignable.")` devient `assert_eq!(app.error.as_deref(), Some("Réseau injoignable."))`.
- Dans `un_succes_efface_l_erreur_en_cours`, les deux dernières lignes deviennent `assert!(app.error.is_none(), "error = {:?}", app.error);`.
- Dans `le_status_annonce_le_nombre_de_pull_requests`, `app.status` devient `app.status_line()` et les deux assertions deviennent `assert!(app.status_line().starts_with("2 pull requests"), "{}", app.status_line());` puis `assert!(app.status_line().starts_with("Aucune pull request"), "{}", app.status_line());`.
- Supprimer entièrement le test `un_tick_pendant_un_chargement_relance_et_jette_la_premiere_reponse` et son commentaire de doc : le comportement qu'il décrit est justement celui que cette spec remplace. Le nouveau test `un_tick_pendant_un_chargement_de_liste_ne_relance_rien` le remplace.

Puis ajouter ces tests neufs, toujours dans `mod tests` :

```rust
    /// PR d'un dépôt donné, pour distinguer deux clés dans un même test.
    fn pr_de(depot: &str, numero: u32) -> PrSummary {
        PrSummary {
            key: PrKey {
                repo: depot.to_string(),
                number: numero,
            },
            ..pr(numero)
        }
    }

    /// Application démarrée et garnie de la liste donnée.
    fn app_garnie(liste: Vec<PrSummary>) -> App {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(liste)),
        });
        app
    }

    /// Rafraîchit et livre la nouvelle liste, en respectant la génération.
    fn rafraichir(app: &mut App, liste: Vec<PrSummary>) {
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(liste)),
        });
    }

    #[test]
    fn les_fleches_deplacent_la_selection() {
        let mut app = app_garnie(vec![pr(1), pr(2), pr(3)]);
        assert_eq!(app.selected, 0);

        assert!(app.handle(Event::Key(Key::Down)).is_empty());
        assert_eq!(app.selected, 1);

        app.handle(Event::Key(Key::Down));
        assert_eq!(app.selected, 2);

        app.handle(Event::Key(Key::Up));
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn j_et_k_deplacent_la_selection_comme_les_fleches() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Char('j')));
        assert_eq!(app.selected, 1);
        app.handle(Event::Key(Key::Char('k')));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn la_selection_ne_deborde_pas_des_extremites() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);

        // En haut de liste, la flèche haut ne fait rien : pas de bouclage.
        app.handle(Event::Key(Key::Up));
        assert_eq!(app.selected, 0);

        app.handle(Event::Key(Key::Down));
        app.handle(Event::Key(Key::Down));
        assert_eq!(app.selected, 1, "la dernière ligne est un mur");
    }

    #[test]
    fn une_liste_vide_n_a_pas_de_selection() {
        let mut app = app_garnie(vec![]);
        assert!(app.selected_pr().is_none());
        app.handle(Event::Key(Key::Down));
        app.handle(Event::Key(Key::Up));
        assert_eq!(app.selected, 0, "aucune touche ne doit paniquer");
        assert!(app.selected_pr().is_none());
    }

    #[test]
    fn le_rafraichissement_suit_la_pr_selectionnee() {
        let mut app = app_garnie(vec![pr(1), pr(2), pr(3)]);
        app.handle(Event::Key(Key::Down));
        assert_eq!(app.selected_pr().map(|pr| pr.key.number), Some(2));

        // La 2 est passée en queue : la sélection la suit.
        rafraichir(&mut app, vec![pr(3), pr(1), pr(2)]);
        assert_eq!(app.selected, 2);
        assert_eq!(app.selected_pr().map(|pr| pr.key.number), Some(2));
    }

    #[test]
    fn deux_depots_de_meme_numero_ne_sont_pas_confondus() {
        let mut app = app_garnie(vec![pr_de("moi/un", 7), pr_de("moi/autre", 7)]);
        app.handle(Event::Key(Key::Down));
        assert_eq!(
            app.selected_pr().map(|pr| pr.key.repo.clone()),
            Some("moi/autre".to_string())
        );

        rafraichir(
            &mut app,
            vec![pr_de("moi/autre", 7), pr_de("moi/un", 7)],
        );
        assert_eq!(
            app.selected_pr().map(|pr| pr.key.repo.clone()),
            Some("moi/autre".to_string()),
            "la clé porte le dépôt, pas seulement le numéro"
        );
    }

    #[test]
    fn une_pr_disparue_laisse_la_selection_dans_les_bornes() {
        let mut app = app_garnie(vec![pr(1), pr(2), pr(3)]);
        app.handle(Event::Key(Key::Down));
        app.handle(Event::Key(Key::Down));
        assert_eq!(app.selected, 2);

        // La 3 a été fusionnée : la liste rétrécit.
        rafraichir(&mut app, vec![pr(1), pr(2)]);
        assert_eq!(app.selected, 1, "l'indice précédent, borné à la nouvelle taille");
        assert!(app.selected_pr().is_some());
    }

    #[test]
    fn une_liste_devenue_vide_n_a_plus_de_selection() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Down));
        rafraichir(&mut app, vec![]);
        assert_eq!(app.selected, 0);
        assert!(app.selected_pr().is_none());
    }

    #[test]
    fn un_tick_pendant_un_chargement_de_liste_ne_relance_rien() {
        let (mut app, _) = app_demarree();
        assert!(app.loading.list, "la requête de démarrage est en cours");
        assert!(
            app.handle(Event::Tick).is_empty(),
            "aucune seconde requête tant que la première n'a pas répondu"
        );
    }

    #[test]
    fn un_tick_apres_la_reponse_relance_la_liste() {
        let mut app = app_garnie(vec![pr(1)]);
        assert!(!app.loading.list);
        match &app.handle(Event::Tick)[0] {
            Command::FetchList { .. } => {}
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn ctrl_c_quitte() {
        let (mut app, _) = app_demarree();
        assert_eq!(app.handle(Event::Key(Key::CtrlC)), vec![Command::Quit]);
        assert!(app.should_quit);
    }
```

- [ ] **Step 3: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test app 2>&1 | tail -30`
Expected: échec de compilation. `no variant named ListLoaded found for enum Event`, `no variant or associated item named Down found for enum Key`, `no field selected on type App`.

- [ ] **Step 4: Écrire l'implémentation**

Dans `src/app/mod.rs`, remplacer les déclarations de `Key`, `Event`, `Command` et de `App` — jusqu'à la fin de `impl App` — par ce code. Les imports en tête du fichier deviennent :

```rust
use chrono::{DateTime, Local};

use crate::config::Config;
use crate::filter::{self, Filter};
use crate::github::GithubError;
use crate::model::{ListPage, PrKey, PrSummary, RateLimit};
```

```rust
/// Touche reçue, traduite par `main`. `app` ignore volontairement `crossterm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Esc,
    Char(char),
    /// Interruption clavier. Le mode brut l'a désarmée : c'est à `owl` de la
    /// traiter, sans quoi l'utilisateur n'aurait plus le réflexe habituel.
    CtrlC,
    /// Toute autre touche, sans effet.
    Other,
}

/// Ce qui arrive dans la file d'événements.
#[derive(Debug)]
pub enum Event {
    Key(Key),
    /// Tour de minuteur de rafraîchissement.
    Tick,
    /// Arrêt demandé par `main` : panique d'une tâche, ou clavier hors service.
    Quit,
    /// Résultat d'une requête de liste.
    ListLoaded {
        generation: Generation,
        result: Result<ListPage, GithubError>,
    },
}

/// Ce que `app` demande à `main` de faire.
#[derive(Debug, PartialEq)]
pub enum Command {
    FetchList {
        generation: Generation,
        /// Chaîne de recherche complète, assemblée par `filter::build_query`.
        query: String,
        page_size: u16,
    },
    Quit,
}

/// Requêtes en vol. Deux drapeaux distincts : une liste qui se rafraîchit ne
/// doit pas faire croire que le détail affiché est en train de charger.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Loading {
    pub list: bool,
    pub detail: bool,
}

pub struct App {
    pub prs: Vec<PrSummary>,
    /// Indice de la ligne sélectionnée. Vaut 0 sur une liste vide, où
    /// `selected_pr` ne rend alors rien.
    pub selected: usize,
    /// Clé de la ligne sélectionnée, retenue pour retrouver la sélection
    /// après un rafraîchissement qui a réordonné la liste.
    selected_key: Option<PrKey>,
    pub loading: Loading,
    /// Dernière erreur reçue, reprise telle quelle de GitHub. Effacée par la
    /// première réponse réussie.
    pub error: Option<String>,
    /// Solde d'appels rapporté par la dernière requête réussie. La suspension
    /// du rafraîchissement qu'il déclenche appartient à `05-erreurs-et-tests.md`.
    #[allow(dead_code)]
    pub rate_limit: Option<RateLimit>,
    pub should_quit: bool,
    pub last_refresh: Option<DateTime<Local>>,
    list_generation: Generation,
    /// Filtres des réglages, traduits une seule fois.
    filters: Vec<Filter>,
    config: Config,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            prs: Vec::new(),
            selected: 0,
            selected_key: None,
            loading: Loading::default(),
            error: None,
            rate_limit: None,
            should_quit: false,
            last_refresh: None,
            list_generation: 0,
            filters: config
                .filters
                .iter()
                .map(|texte| Filter::parse(texte))
                .collect(),
            config,
        }
    }

    /// Première demande, au démarrage.
    pub fn start(&mut self) -> Vec<Command> {
        vec![self.fetch_list()]
    }

    /// Pull request sélectionnée, s'il y en a une.
    pub fn selected_pr(&self) -> Option<&PrSummary> {
        self.prs.get(self.selected)
    }

    pub fn handle(&mut self, event: Event) -> Vec<Command> {
        match event {
            Event::Key(touche) => self.handle_key(touche),
            Event::Quit => {
                self.should_quit = true;
                vec![Command::Quit]
            }
            // Une requête de liste déjà en vol suffit : relancer ne ferait
            // qu'ajouter un appel et jeter la réponse précédente.
            Event::Tick => {
                if self.loading.list {
                    Vec::new()
                } else {
                    vec![self.fetch_list()]
                }
            }
            Event::ListLoaded { generation, result } => {
                // Réponse d'une demande dépassée : on la jette sans rien changer.
                if generation != self.list_generation {
                    return Vec::new();
                }
                self.loading.list = false;
                match result {
                    Ok(page) => {
                        self.apply_list(page.pull_requests);
                        self.rate_limit = page.rate_limit;
                        self.last_refresh = Some(Local::now());
                        self.error = None;
                    }
                    // Message de GitHub repris tel quel, et liste conservée.
                    Err(erreur) => self.error = Some(erreur.to_string()),
                }
                Vec::new()
            }
        }
    }

    fn handle_key(&mut self, touche: Key) -> Vec<Command> {
        match touche {
            Key::Char('q') | Key::CtrlC => {
                self.should_quit = true;
                vec![Command::Quit]
            }
            Key::Char('r') => vec![self.fetch_list()],
            Key::Up | Key::Char('k') => {
                self.select_previous();
                Vec::new()
            }
            Key::Down | Key::Char('j') => {
                self.select_next();
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Range une nouvelle liste en préservant la sélection.
    ///
    /// La clé retenue est cherchée dans la nouvelle liste. Si la PR a disparu,
    /// l'indice précédent est conservé, borné à la nouvelle taille : c'est ce
    /// qui garde le curseur au même endroit de l'écran plutôt que de le
    /// renvoyer en tête à chaque fusion.
    fn apply_list(&mut self, prs: Vec<PrSummary>) {
        let precedent = self.selected;
        self.prs = prs;
        self.selected = match &self.selected_key {
            Some(cle) => self
                .prs
                .iter()
                .position(|pr| &pr.key == cle)
                .unwrap_or(precedent),
            None => precedent,
        };
        self.selected = self.selected.min(self.prs.len().saturating_sub(1));
        self.remember_selection();
    }

    fn select_previous(&mut self) {
        // La sélection ne boucle pas : en haut de liste, rien ne se passe.
        if self.selected > 0 {
            self.selected -= 1;
            self.remember_selection();
        }
    }

    fn select_next(&mut self) {
        if self.selected + 1 < self.prs.len() {
            self.selected += 1;
            self.remember_selection();
        }
    }

    fn remember_selection(&mut self) {
        self.selected_key = self.selected_pr().map(|pr| pr.key.clone());
    }

    /// Ouvre une nouvelle génération et demande la liste.
    fn fetch_list(&mut self) -> Command {
        self.list_generation += 1;
        self.loading.list = true;
        Command::FetchList {
            generation: self.list_generation,
            query: filter::build_query(&self.filters),
            page_size: self.config.page_size,
        }
    }

    /// Barre d'état complète, prête à dessiner telle quelle.
    ///
    /// Assemblée ici, et pas dans `ui`, parce que chaque morceau est une
    /// décision : le libellé de l'heure, l'annonce d'une requête en cours,
    /// l'aide clavier.
    pub fn status_line(&self) -> String {
        let mut morceaux: Vec<String> = Vec::new();

        if self.last_refresh.is_none() && self.error.is_none() {
            // Rien n'est encore arrivé : l'attente est le message principal.
            morceaux.push(ATTENTE_INITIALE.to_string());
        } else {
            if self.last_refresh.is_some() {
                morceaux.push(self.liste_resumee());
            }
            if let Some(instant) = self.last_refresh {
                morceaux.push(format!("mis à jour à {}", instant.format("%H:%M")));
            }
            if self.loading.list || self.loading.detail {
                morceaux.push("chargement…".to_string());
            }
            if let Some(erreur) = &self.error {
                morceaux.push(erreur.clone());
            }
        }

        morceaux.push(AIDE.to_string());
        morceaux.join(" · ")
    }

    /// Résumé de la liste pour la barre d'état.
    fn liste_resumee(&self) -> String {
        match self.prs.len() {
            0 => "Aucune pull request".to_string(),
            1 => "1 pull request".to_string(),
            nombre => format!("{nombre} pull requests"),
        }
    }
}
```

L'aide clavier de la barre d'état s'allonge, la spec ayant ajouté des touches :

```rust
/// Aide clavier, en fin de barre d'état. Le texte est ici, pas dans `ui`.
const AIDE: &str = "↑↓ naviguer · → détail · m fusionner · r rafraîchir · o navigateur · q quitter";
```

Les tests de barre d'état existants comparent la chaîne entière : y répercuter la nouvelle aide. Par exemple `la_barre_d_etat_au_demarrage_n_annonce_l_attente_qu_une_fois` devient :

```rust
    #[test]
    fn la_barre_d_etat_au_demarrage_n_annonce_l_attente_qu_une_fois() {
        let (app, _) = app_demarree();
        assert_eq!(app.status_line(), format!("Chargement… · {AIDE}"));
    }
```

Faire de même dans les trois autres tests de barre d'état, en remplaçant le texte d'aide littéral par `{AIDE}` dans le `format!`.

- [ ] **Step 5: Traduire les touches dans `main.rs` et suivre le renommage**

Dans `src/main.rs`, l'import devient `use crossterm::event::{Event as TerminalEvent, KeyCode, KeyEventKind, KeyModifiers};` et la traduction des touches de `spawn_keyboard` devient :

```rust
        let traduite = match (touche.code, touche.modifiers) {
            // Ctrl+C d'abord : sans ce cas, elle passerait pour un « c ».
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => Key::CtrlC,
            (KeyCode::Char(caractere), _) => Key::Char(caractere),
            (KeyCode::Up, _) => Key::Up,
            (KeyCode::Down, _) => Key::Down,
            (KeyCode::Left, _) => Key::Left,
            (KeyCode::Right, _) => Key::Right,
            (KeyCode::Enter, _) => Key::Enter,
            (KeyCode::Esc, _) => Key::Esc,
            _ => Key::Other,
        };
```

Dans `execute_command`, `Command::Fetch { .. }` devient `Command::FetchList { .. }` et l'événement envoyé `Event::ListLoaded { generation, result: resultat }`.

Dans `src/ui/list.rs`, `app.items` devient `app.prs`. Le dessin lui-même est réécrit à la tâche 2 : ne rien changer d'autre ici.

- [ ] **Step 6: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test 2>&1 | tail -30`
Expected: tous les tests passent, dont les onze tests neufs de sélection, de bornes et de `Tick`.

- [ ] **Step 7: Lancer les quatre commandes**

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check
```
Expected: tout passe. `cargo fmt --check` muet ; s'il signale un écart, lancer `cargo fmt` et relancer.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "Ajoute la sélection, le clavier étendu et la préservation de la sélection"
```

---

### Task 2: Composition et dessin de la ligne de liste

**Files:**
- Create: `src/app/render.rs`
- Modify: `src/app/mod.rs` (déclaration `mod render;` et réexport)
- Modify: `src/ui/list.rs` (réécrit : ne compose plus aucun texte)
- Modify: `docs/suivi/DETTE.md` (l'entrée « Le texte d'une ligne de liste est composé dans le dessin » est résolue et sort du fichier)

**Interfaces:**
- Consumes: `App`, `App::prs`, `App::selected`, `App::status_line`, le champ privé `filters` (accessible depuis `app::render`, module enfant) ; `PrSummary`, `ChecksState`, `ReviewState`, `MergeableState` de `model`.
- Produces:
  - `pub enum Tone { Vert, Rouge, Jaune, Gris }`
  - `pub struct Glyph { pub symbol: char, pub tone: Tone }`
  - `pub struct ListRow { pub checks: Glyph, pub review: Glyph, pub text: String, pub dim: bool }`
  - `pub enum ListRender { Rows(Vec<ListRow>), Empty(Vec<String>), TooNarrow(String) }`
  - `pub fn App::list_render(&self, width: u16) -> ListRender`

- [ ] **Step 1: Écrire les tests qui échouent**

Créer `src/app/render.rs` avec ce seul contenu — le commentaire de module et les tests. Le code de production vient à l'étape 3.

```rust
//! Composition de l'affichage : pictogrammes, colonnes, troncature, messages.
//!
//! Tout ce qui se décide avant de dessiner est ici, et rien de ce qui est ici
//! ne touche au terminal. `ui` reçoit des chaînes prêtes et des tons, et
//! n'ajoute que la mise en page et la couleur.

#[cfg(test)]
mod tests {
    use super::*;

    use crate::app::tests::{app_garnie, pr, pr_de};
    use crate::config::Config;

    /// Largeur confortable : aucun titre n'y est tronqué.
    const LARGE: u16 = 120;

    fn lignes(app: &crate::app::App, largeur: u16) -> Vec<ListRow> {
        match app.list_render(largeur) {
            ListRender::Rows(lignes) => lignes,
            autre => panic!("rendu inattendu : {autre:?}"),
        }
    }

    #[test]
    fn une_ligne_porte_les_deux_pictogrammes_puis_le_depot_le_numero_et_le_titre() {
        let app = app_garnie(vec![pr(142)]);
        let ligne = lignes(&app, LARGE).remove(0);
        assert_eq!(ligne.checks, Glyph { symbol: '✓', tone: Tone::Vert });
        assert_eq!(ligne.review, Glyph { symbol: '✓', tone: Tone::Vert });
        assert_eq!(ligne.text, "moi/depot  #142  Titre 142");
        assert!(!ligne.dim);
    }

    #[test]
    fn chaque_etat_de_verification_a_son_pictogramme() {
        let cas = [
            (ChecksState::Success, '✓', Tone::Vert),
            (ChecksState::Failure, '✗', Tone::Rouge),
            (ChecksState::Pending, '○', Tone::Jaune),
            (ChecksState::None, '·', Tone::Gris),
        ];
        for (etat, symbole, ton) in cas {
            let app = app_garnie(vec![PrSummary { checks: etat, ..pr(1) }]);
            assert_eq!(
                lignes(&app, LARGE)[0].checks,
                Glyph { symbol: symbole, tone: ton },
                "état = {etat:?}"
            );
        }
    }

    #[test]
    fn chaque_etat_de_relecture_a_son_pictogramme() {
        let cas = [
            (ReviewState::Approved, '✓', Tone::Vert),
            (ReviewState::ChangesRequested, '✗', Tone::Rouge),
            (ReviewState::ReviewRequired, '●', Tone::Jaune),
            (ReviewState::None, '·', Tone::Gris),
        ];
        for (etat, symbole, ton) in cas {
            let app = app_garnie(vec![PrSummary { review: etat, ..pr(1) }]);
            assert_eq!(
                lignes(&app, LARGE)[0].review,
                Glyph { symbol: symbole, tone: ton },
                "état = {etat:?}"
            );
        }
    }

    #[test]
    fn un_brouillon_est_prefixe_et_grise() {
        let app = app_garnie(vec![PrSummary { is_draft: true, ..pr(150) }]);
        let ligne = lignes(&app, LARGE).remove(0);
        assert_eq!(ligne.text, "moi/depot  #150  [brouillon] Titre 150");
        assert!(ligne.dim, "la ligne d'un brouillon est grisée");
    }

    #[test]
    fn un_conflit_de_fusion_est_signale_devant_le_titre() {
        let app = app_garnie(vec![PrSummary {
            mergeable: MergeableState::Conflicting,
            ..pr(31)
        }]);
        assert_eq!(lignes(&app, LARGE)[0].text, "moi/depot  #31  ⚠ Titre 31");
    }

    #[test]
    fn un_etat_de_fusion_inconnu_n_affiche_rien() {
        let app = app_garnie(vec![PrSummary {
            mergeable: MergeableState::Unknown,
            ..pr(31)
        }]);
        assert_eq!(
            lignes(&app, LARGE)[0].text,
            "moi/depot  #31  Titre 31",
            "GitHub calcule peut-être encore : ne rien annoncer"
        );
    }

    #[test]
    fn un_brouillon_en_conflit_porte_les_deux_marques() {
        let app = app_garnie(vec![PrSummary {
            is_draft: true,
            mergeable: MergeableState::Conflicting,
            ..pr(7)
        }]);
        assert_eq!(lignes(&app, LARGE)[0].text, "moi/depot  #7  [brouillon] ⚠ Titre 7");
    }

    #[test]
    fn les_depots_et_les_numeros_sont_alignes_entre_eux() {
        let app = app_garnie(vec![pr_de("moi/depot", 7), pr_de("moi/un-depot-plus-long", 150)]);
        let lignes = lignes(&app, LARGE);
        let colonne = |ligne: &ListRow| ligne.text.find("  #").expect("colonne du numéro");
        assert_eq!(
            colonne(&lignes[0]),
            colonne(&lignes[1]),
            "les numéros commencent à la même colonne"
        );
        let titre = |ligne: &ListRow| ligne.text.find("Titre").expect("colonne du titre");
        assert_eq!(titre(&lignes[0]), titre(&lignes[1]), "les titres aussi");
    }

    #[test]
    fn le_titre_est_tronque_a_la_largeur_disponible() {
        let app = app_garnie(vec![PrSummary {
            title: "Un titre beaucoup trop long pour la fenêtre".to_string(),
            ..pr(1)
        }]);
        // 30 colonnes moins les 6 des pictogrammes, que `ui` ajoute lui-même.
        let ligne = lignes(&app, 30).remove(0);
        assert_eq!(ligne.text.chars().count(), 24);
        assert!(ligne.text.starts_with("moi/depot  #1  "), "{}", ligne.text);
        assert!(ligne.text.ends_with('…'), "{}", ligne.text);
    }

    #[test]
    fn le_depot_et_le_numero_ne_sont_jamais_tronques() {
        // Juste de quoi tenir les pictogrammes, le dépôt et le numéro.
        let app = app_garnie(vec![pr(142)]);
        let ligne = lignes(&app, 6 + 9 + 2 + 4).remove(0);
        assert_eq!(ligne.text, "moi/depot  #142", "pas de titre, mais tout le reste");
    }

    #[test]
    fn une_fenetre_trop_etroite_demande_l_elargissement() {
        let app = app_garnie(vec![pr(142)]);
        match app.list_render(10) {
            ListRender::TooNarrow(message) => {
                assert!(message.contains("Élargis"), "message = {message}")
            }
            autre => panic!("rendu inattendu : {autre:?}"),
        }
    }

    #[test]
    fn une_liste_vide_rappelle_les_filtres_actifs() {
        let app = app_garnie(vec![]);
        match app.list_render(LARGE) {
            ListRender::Empty(lignes) => {
                assert_eq!(lignes[0], "Aucune pull request");
                assert!(
                    lignes[1].contains("author:@me") && lignes[1].contains("is:open"),
                    "un filtre trop restrictif ressemble sinon à une panne : {}",
                    lignes[1]
                );
            }
            autre => panic!("rendu inattendu : {autre:?}"),
        }
    }

    #[test]
    fn une_liste_vide_avec_des_filtres_inhabituels_les_rappelle_aussi() {
        let reglages = Config {
            filters: vec!["org:acme".to_string(), "involves:@me".to_string()],
            ..Config::default()
        };
        let app = crate::app::App::new(reglages);
        match app.list_render(LARGE) {
            ListRender::Empty(lignes) => {
                assert!(lignes[1].contains("org:acme"), "{}", lignes[1]);
                assert!(lignes[1].contains("involves:@me"), "{}", lignes[1]);
            }
            autre => panic!("rendu inattendu : {autre:?}"),
        }
    }
}
```

Pour que ces tests atteignent les fonctions d'aide de `mod.rs`, rendre le module de tests de `src/app/mod.rs` visible depuis son frère : y remplacer `#[cfg(test)] mod tests {` par

```rust
#[cfg(test)]
pub(crate) mod tests {
```

et marquer `pr`, `pr_de`, `page`, `app_demarree`, `app_garnie` et `rafraichir` en `pub(crate) fn`.

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test render 2>&1 | tail -20`
Expected: échec de compilation, `cannot find type ListRow in this scope` et `no method named list_render found for struct App`.

- [ ] **Step 3: Écrire l'implémentation**

Dans `src/app/mod.rs`, sous les autres déclarations de module — en tête du fichier, après le commentaire de module :

```rust
mod render;

pub use render::{Glyph, ListRender, ListRow, Tone};
```

Puis insérer ce code dans `src/app/render.rs`, entre le commentaire de module et le bloc `mod tests` :

```rust
use crate::app::App;
use crate::filter::Filter;
use crate::model::{ChecksState, MergeableState, PrSummary, ReviewState};

/// Couleur logique d'un élément. `ui` la traduit en couleur de terminal ;
/// le sens — vert pour « ça passe » — est décidé ici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Vert,
    Rouge,
    Jaune,
    Gris,
}

/// Un pictogramme et son ton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    pub symbol: char,
    pub tone: Tone,
}

/// Une ligne de liste, prête à dessiner.
#[derive(Debug, Clone, PartialEq)]
pub struct ListRow {
    pub checks: Glyph,
    pub review: Glyph,
    /// Dépôt, numéro et titre : colonnes déjà alignées, titre déjà tronqué,
    /// marques du brouillon et du conflit déjà posées.
    pub text: String,
    /// Ligne grisée, parce que la pull request est un brouillon.
    pub dim: bool,
}

/// Ce qu'il y a à dessiner à la place de la liste.
#[derive(Debug, Clone, PartialEq)]
pub enum ListRender {
    Rows(Vec<ListRow>),
    /// Aucune pull request : le message, puis le rappel des filtres actifs.
    Empty(Vec<String>),
    /// Terminal trop étroit pour le dépôt et le numéro, qui ne se tronquent
    /// jamais : mieux vaut le dire qu'afficher une bouillie.
    TooNarrow(String),
}

/// Largeur fixe des deux colonnes de pictogrammes, séparateur compris :
/// une espace, un pictogramme, une espace, un pictogramme, deux espaces.
const PICTOGRAMMES: usize = 6;

/// Espacement entre deux colonnes de texte.
const ECART: usize = 2;

const TROP_ETROIT: &str = "Élargis le terminal : le dépôt et le numéro n'y tiennent pas.";

const LISTE_VIDE: &str = "Aucune pull request";

impl App {
    /// Compose la liste pour une largeur donnée, celle de l'intérieur du cadre.
    ///
    /// La largeur entre ici parce que la troncature en dépend, et qu'elle est
    /// une décision : `ui` ne coupe jamais un texte lui-même.
    pub fn list_render(&self, width: u16) -> ListRender {
        if self.prs.is_empty() {
            return ListRender::Empty(vec![
                LISTE_VIDE.to_string(),
                format!("Filtres actifs : {}", self.filtres_actifs()),
            ]);
        }

        let largeur = width as usize;
        let colonne_depot = self
            .prs
            .iter()
            .map(|pr| pr.key.repo.chars().count())
            .max()
            .unwrap_or(0);
        let colonne_numero = self
            .prs
            .iter()
            .map(|pr| numero(pr).chars().count())
            .max()
            .unwrap_or(0);

        let minimale = PICTOGRAMMES + colonne_depot + ECART + colonne_numero;
        if largeur < minimale {
            return ListRender::TooNarrow(TROP_ETROIT.to_string());
        }
        let titre_disponible = largeur.saturating_sub(minimale + ECART);

        ListRender::Rows(
            self.prs
                .iter()
                .map(|pr| ListRow {
                    checks: glyphe_verifications(pr.checks),
                    review: glyphe_relecture(pr.review),
                    text: ligne_texte(pr, colonne_depot, colonne_numero, titre_disponible),
                    dim: pr.is_draft,
                })
                .collect(),
        )
    }

    /// Rappel des filtres actifs, pour la liste vide.
    fn filtres_actifs(&self) -> String {
        self.filters
            .iter()
            .map(Filter::fragment)
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

fn numero(pr: &PrSummary) -> String {
    format!("#{}", pr.key.number)
}

/// Dépôt, numéro et titre, en colonnes alignées.
fn ligne_texte(
    pr: &PrSummary,
    colonne_depot: usize,
    colonne_numero: usize,
    titre_disponible: usize,
) -> String {
    let mut ligne = format!(
        "{:<colonne_depot$}  {:<colonne_numero$}",
        pr.key.repo,
        numero(pr)
    );
    if titre_disponible > 0 {
        ligne.push_str("  ");
        ligne.push_str(&tronquer(&titre_affiche(pr), titre_disponible));
    }
    // La dernière colonne ne porte pas de remplissage inutile.
    ligne.trim_end().to_string()
}

/// Titre avec ses marques : le brouillon qualifie la pull request, le conflit
/// qualifie sa fusion. Un état de fusion inconnu n'affiche rien, GitHub étant
/// peut-être encore en train de le calculer.
fn titre_affiche(pr: &PrSummary) -> String {
    let mut titre = String::new();
    if pr.is_draft {
        titre.push_str("[brouillon] ");
    }
    if pr.mergeable == MergeableState::Conflicting {
        titre.push_str("⚠ ");
    }
    titre.push_str(&pr.title);
    titre
}

/// Coupe à la largeur donnée, en marquant la coupe. La mesure se fait en
/// caractères : compter les colonnes réellement occupées demanderait une
/// dépendance de plus.
fn tronquer(texte: &str, largeur: usize) -> String {
    if texte.chars().count() <= largeur {
        return texte.to_string();
    }
    if largeur <= 1 {
        return texte.chars().take(largeur).collect();
    }
    let mut coupe: String = texte.chars().take(largeur - 1).collect();
    coupe.push('…');
    coupe
}

fn glyphe_verifications(etat: ChecksState) -> Glyph {
    match etat {
        ChecksState::Success => Glyph {
            symbol: '✓',
            tone: Tone::Vert,
        },
        ChecksState::Failure => Glyph {
            symbol: '✗',
            tone: Tone::Rouge,
        },
        ChecksState::Pending => Glyph {
            symbol: '○',
            tone: Tone::Jaune,
        },
        ChecksState::None => Glyph {
            symbol: '·',
            tone: Tone::Gris,
        },
    }
}

fn glyphe_relecture(etat: ReviewState) -> Glyph {
    match etat {
        ReviewState::Approved => Glyph {
            symbol: '✓',
            tone: Tone::Vert,
        },
        ReviewState::ChangesRequested => Glyph {
            symbol: '✗',
            tone: Tone::Rouge,
        },
        ReviewState::ReviewRequired => Glyph {
            symbol: '●',
            tone: Tone::Jaune,
        },
        ReviewState::None => Glyph {
            symbol: '·',
            tone: Tone::Gris,
        },
    }
}
```

- [ ] **Step 4: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test render 2>&1 | tail -20`
Expected: les douze tests de `render` passent.

- [ ] **Step 5: Réécrire le dessin de la liste**

Remplacer tout le contenu de `src/ui/list.rs` par :

```rust
//! Dessin de la liste des pull requests et de la barre d'état.
//!
//! Aucune décision : les pictogrammes, les colonnes, la troncature et les
//! messages sont composés par `app`. Ici, seulement la mise en page, la
//! couleur, et le curseur de sélection.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::{App, ListRender, ListRow, Tone};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let zones = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let cadre = Block::default()
        .borders(Borders::ALL)
        .title(" owl — pull requests ");
    // La largeur utile est celle de l'intérieur du cadre : c'est elle que
    // `app` doit connaître pour décider de la troncature.
    let interieur = cadre.inner(zones[0]);
    frame.render_widget(cadre, zones[0]);

    match app.list_render(interieur.width) {
        ListRender::Rows(lignes) => {
            let items: Vec<ListItem> = lignes.into_iter().map(item).collect();
            // L'état de sélection est reconstruit à chaque dessin depuis
            // `app.selected` : c'est lui qui fait défiler la liste quand elle
            // dépasse la hauteur, et `ui` ne retient rien entre deux dessins.
            let mut etat = ListState::default().with_selected(Some(app.selected));
            let liste = List::new(items)
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            frame.render_stateful_widget(liste, interieur, &mut etat);
        }
        ListRender::Empty(lignes) => {
            let texte: Vec<Line> = lignes.into_iter().map(Line::from).collect();
            frame.render_widget(Paragraph::new(texte), interieur);
        }
        ListRender::TooNarrow(message) => {
            frame.render_widget(Paragraph::new(message), interieur);
        }
    }

    frame.render_widget(Paragraph::new(app.status_line()), zones[1]);
}

/// Une ligne : deux pictogrammes colorés, à largeur fixe, puis le texte.
fn item(ligne: ListRow) -> ListItem<'static> {
    let mut style_texte = Style::default();
    if ligne.dim {
        style_texte = style_texte.add_modifier(Modifier::DIM);
    }
    ListItem::new(Line::from(vec![
        Span::raw(" "),
        Span::styled(
            ligne.checks.symbol.to_string(),
            Style::default().fg(couleur(ligne.checks.tone)),
        ),
        Span::raw(" "),
        Span::styled(
            ligne.review.symbol.to_string(),
            Style::default().fg(couleur(ligne.review.tone)),
        ),
        Span::raw("  "),
        Span::styled(ligne.text, style_texte),
    ]))
}

fn couleur(ton: Tone) -> Color {
    match ton {
        Tone::Vert => Color::Green,
        Tone::Rouge => Color::Red,
        Tone::Jaune => Color::Yellow,
        Tone::Gris => Color::DarkGray,
    }
}
```

- [ ] **Step 6: Retirer l'entrée résolue de la dette**

Dans `docs/suivi/DETTE.md`, supprimer entièrement la section « Le texte d'une ligne de liste est composé dans le dessin ». Elle est résolue : `ui/list.rs` ne compose plus rien. Le registre n'est pas un historique.

- [ ] **Step 7: Lancer les quatre commandes**

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check
```
Expected: tout passe. Si `clippy` signale un `format!` dans `src/ui/`, c'est un vrai défaut : la composition doit repartir dans `app/render.rs`.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "Compose la ligne de liste dans app et réduit le dessin à la couleur"
```

---

### Task 3: Vue détail — état, cache, composition et dessin

**Files:**
- Modify: `src/app/mod.rs` (`View`, `details`, `detail_generation`, `Event::DetailLoaded`, `Command::FetchDetail`, clavier de la vue détail, aide selon la vue)
- Modify: `src/app/render.rs` (`DetailLine`, `detail_lines`, `detail_line_count`)
- Modify: `src/ui/detail.rs` (dessin, aujourd'hui vide)
- Modify: `src/ui/mod.rs` (aiguillage réel)
- Modify: `src/main.rs` (exécute `FetchDetail`)

**Interfaces:**
- Consumes: tout ce que les tâches 1 et 2 produisent ; `PrDetail`, `CheckRun`, `Review`, `Comment`, `ChangedFile` de `model` ; `github::Client::fetch_detail(&self, summary: &PrSummary)`.
- Produces:
  - `pub enum View { List, Detail { key: PrKey, scroll: u16 } }`
  - `pub struct CachedDetail { pub detail: PrDetail, pub loaded_at: DateTime<Local> }`
  - `Event::DetailLoaded { generation: Generation, key: PrKey, result: Result<PrDetail, GithubError> }`
  - `Command::FetchDetail { generation: Generation, summary: PrSummary }`
  - `pub struct DetailLine { pub text: String, pub tone: Option<Tone> }`
  - `pub fn App::detail_lines(&self, width: u16) -> Vec<DetailLine>`
  - `pub fn App::detail_scroll(&self) -> u16`

- [ ] **Step 1: Écrire les tests d'état qui échouent**

Dans le `mod tests` de `src/app/mod.rs`, ajouter d'abord ce constructeur de détail, puis les tests.

```rust
    /// Détail d'une pull request, minimal mais complet dans sa forme.
    pub(crate) fn detail(numero: u32) -> PrDetail {
        let resume = pr(numero);
        PrDetail {
            node_id: format!("PR_{numero}"),
            body: "Première ligne.\nSeconde ligne.".to_string(),
            head_ref: "ma-branche".to_string(),
            base_ref: "develop".to_string(),
            checks: vec![CheckRun {
                name: "tests".to_string(),
                state: ChecksState::Success,
                url: None,
            }],
            reviews: vec![Review {
                author: "collegue".to_string(),
                state: ReviewState::Approved,
                body: "Ça me va.".to_string(),
                submitted_at: "2026-08-30T10:00:00Z".parse().expect("date valide"),
            }],
            comments: vec![Comment {
                author: "moi".to_string(),
                body: "Rebasé.".to_string(),
                created_at: "2026-08-30T11:00:00Z".parse().expect("date valide"),
            }],
            files: vec![ChangedFile {
                path: "src/app/mod.rs".to_string(),
                additions: 12,
                deletions: 3,
            }],
            additions: 12,
            deletions: 3,
            summary: resume,
        }
    }

    /// Ouvre le détail de la sélection et rend la génération demandée.
    fn ouvrir_detail(app: &mut App) -> Generation {
        match &app.handle(Event::Key(Key::Right))[..] {
            [Command::FetchDetail { generation, .. }] => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn la_fleche_droite_ouvre_le_detail_et_demande_les_donnees() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Down));
        let commandes = app.handle(Event::Key(Key::Right));
        assert!(matches!(app.view, View::Detail { .. }));
        match &commandes[..] {
            [Command::FetchDetail { summary, .. }] => assert_eq!(summary.key.number, 2),
            autre => panic!("commande inattendue : {autre:?}"),
        }
        assert!(app.loading.detail);
    }

    #[test]
    fn entree_ouvre_aussi_le_detail() {
        let mut app = app_garnie(vec![pr(1)]);
        app.handle(Event::Key(Key::Enter));
        assert!(matches!(app.view, View::Detail { .. }));
    }

    #[test]
    fn ouvrir_une_pr_deja_en_cache_n_emet_aucune_commande() {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = ouvrir_detail(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        app.handle(Event::Key(Key::Left));

        let commandes = app.handle(Event::Key(Key::Right));
        assert!(
            commandes.is_empty(),
            "le cache de la session évite la requête : {commandes:?}"
        );
        assert!(!app.loading.detail);
    }

    #[test]
    fn la_fleche_gauche_et_echap_reviennent_a_la_liste() {
        let mut app = app_garnie(vec![pr(1)]);
        ouvrir_detail(&mut app);
        assert!(app.handle(Event::Key(Key::Left)).is_empty());
        assert!(matches!(app.view, View::List));

        ouvrir_detail(&mut app);
        app.handle(Event::Key(Key::Esc));
        assert!(matches!(app.view, View::List));
    }

    #[test]
    fn une_liste_vide_n_ouvre_pas_de_detail() {
        let mut app = app_garnie(vec![]);
        assert!(app.handle(Event::Key(Key::Right)).is_empty());
        assert!(matches!(app.view, View::List));
    }

    #[test]
    fn r_en_vue_detail_recharge_le_detail_et_pas_la_liste() {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = ouvrir_detail(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });

        match &app.handle(Event::Key(Key::Char('r')))[..] {
            [Command::FetchDetail { generation: neuve, .. }] => {
                assert!(*neuve > generation, "une nouvelle génération s'ouvre")
            }
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn les_fleches_font_defiler_le_detail_sans_deborder() {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = ouvrir_detail(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        assert_eq!(app.detail_scroll(), 0);

        // En haut, la flèche haut ne fait rien.
        app.handle(Event::Key(Key::Up));
        assert_eq!(app.detail_scroll(), 0);

        app.handle(Event::Key(Key::Down));
        assert_eq!(app.detail_scroll(), 1);
        app.handle(Event::Key(Key::Up));
        assert_eq!(app.detail_scroll(), 0);

        // Le bas de la zone est un mur : on ne défile pas dans le vide.
        for _ in 0..500 {
            app.handle(Event::Key(Key::Down));
        }
        let dernier = app.detail_scroll() as usize;
        assert!(dernier > 0);
        assert!(
            dernier < app.detail_lines(u16::MAX).len(),
            "défilement borné au contenu"
        );
    }

    #[test]
    fn un_detail_perime_est_ignore() {
        let mut app = app_garnie(vec![pr(1)]);
        let premiere = ouvrir_detail(&mut app);
        // Rechargement : la réponse lente de la première arrive après.
        app.handle(Event::Key(Key::Char('r')));
        app.handle(Event::DetailLoaded {
            generation: premiere,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        assert!(app.details.is_empty(), "la réponse périmée ne se range pas");
        assert!(app.loading.detail, "la requête en cours reste en cours");
    }

    #[test]
    fn ouvrir_un_detail_ne_perime_pas_une_requete_de_liste_en_vol() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        let generation_liste = match &app.handle(Event::Tick)[..] {
            [Command::FetchList { generation, .. }] => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        ouvrir_detail(&mut app);

        app.handle(Event::ListLoaded {
            generation: generation_liste,
            result: Ok(page(vec![pr(1), pr(2), pr(3)])),
        });
        assert_eq!(app.prs.len(), 3, "le résultat de liste doit être accepté");
        assert!(!app.loading.list);
    }

    #[test]
    fn un_rafraichissement_de_liste_ne_vide_pas_le_cache_des_details() {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = ouvrir_detail(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        app.handle(Event::Key(Key::Left));

        rafraichir(&mut app, vec![pr(1)]);
        assert!(
            app.details.contains_key(&pr(1).key),
            "le compromis est assumé : le détail reste en cache jusqu'à r"
        );
    }

    #[test]
    fn une_erreur_de_detail_est_reprise_telle_quelle() {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = ouvrir_detail(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Err(GithubError::Transport),
        });
        assert_eq!(app.error.as_deref(), Some("Réseau injoignable."));
        assert!(!app.loading.detail);
    }
```

L'import de `crate::model` dans `mod tests` devient :

```rust
    use crate::model::{
        ChangedFile, ChecksState, Comment, ListPage, MergeableState, PrDetail, PrKey, PrSummary,
        RateLimit, RepoMergeRules, Review, ReviewState,
    };
```

Retirer de cette liste ce que le fichier importe déjà en tête pour éviter un doublon, et supprimer un import devenu inutile si `clippy` le signale.

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test app 2>&1 | tail -30`
Expected: échec de compilation, `cannot find type View in this scope`, `no variant named FetchDetail found for enum Command`, `no method named detail_scroll found for struct App`.

- [ ] **Step 3: Écrire l'état de la vue détail**

Dans `src/app/mod.rs`, ajouter `use std::collections::HashMap;` en tête et `PrDetail` à l'import de `crate::model`. Puis :

```rust
/// Vue affichée. Le défilement voyage avec la vue : revenir à la liste puis
/// rouvrir un détail le remet en haut, ce qui est le comportement attendu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    List,
    Detail { key: PrKey, scroll: u16 },
}

/// Un détail en cache, avec l'heure de son chargement : le détail peut être
/// périmé sans qu'on le sache, autant dire quand il a été lu.
#[derive(Debug, Clone)]
pub struct CachedDetail {
    pub detail: PrDetail,
    pub loaded_at: DateTime<Local>,
}
```

Ajouter les variantes :

```rust
    /// Résultat d'une requête de détail.
    DetailLoaded {
        generation: Generation,
        key: PrKey,
        result: Result<PrDetail, GithubError>,
    },
```

```rust
    FetchDetail {
        generation: Generation,
        /// Le résumé entier, et pas la seule clé : `github::fetch_detail` le
        /// recopie dans le `PrDetail` qu'il rend.
        summary: PrSummary,
    },
```

Ajouter à `App` les champs `pub view: View`, `pub details: HashMap<PrKey, CachedDetail>`, `detail_generation: Generation`, initialisés dans `new` à `View::List`, `HashMap::new()` et `0`.

Remplacer `handle_key` par :

```rust
    fn handle_key(&mut self, touche: Key) -> Vec<Command> {
        // Touches communes aux deux vues, traitées avant l'aiguillage.
        match touche {
            Key::Char('q') | Key::CtrlC => {
                self.should_quit = true;
                return vec![Command::Quit];
            }
            Key::Char('r') => return self.refresh(),
            _ => {}
        }

        match self.view {
            View::List => self.handle_key_list(touche),
            View::Detail { .. } => self.handle_key_detail(touche),
        }
    }

    fn handle_key_list(&mut self, touche: Key) -> Vec<Command> {
        match touche {
            Key::Up | Key::Char('k') => self.select_previous(),
            Key::Down | Key::Char('j') => self.select_next(),
            Key::Right | Key::Enter => return self.open_detail(),
            _ => {}
        }
        Vec::new()
    }

    fn handle_key_detail(&mut self, touche: Key) -> Vec<Command> {
        match touche {
            Key::Up | Key::Char('k') => self.scroll_detail(-1),
            Key::Down | Key::Char('j') => self.scroll_detail(1),
            Key::Left | Key::Esc => self.view = View::List,
            _ => {}
        }
        Vec::new()
    }

    /// `r` rafraîchit ce qui est affiché : la liste, ou le détail ouvert.
    /// Sur le détail, la requête part même si le cache répond déjà : c'est
    /// justement le seul moyen de le rafraîchir.
    fn refresh(&mut self) -> Vec<Command> {
        match &self.view {
            View::List => vec![self.fetch_list()],
            View::Detail { key, .. } => {
                let cle = key.clone();
                match self.prs.iter().find(|pr| pr.key == cle).cloned() {
                    Some(resume) => vec![self.fetch_detail(resume)],
                    // La PR a disparu de la liste : rien à recharger.
                    None => Vec::new(),
                }
            }
        }
    }

    /// Ouvre le détail de la sélection. Une PR déjà consultée pendant la
    /// session s'affiche depuis le cache, sans nouvelle requête.
    fn open_detail(&mut self) -> Vec<Command> {
        let Some(resume) = self.selected_pr().cloned() else {
            return Vec::new();
        };
        self.view = View::Detail {
            key: resume.key.clone(),
            scroll: 0,
        };
        if self.details.contains_key(&resume.key) {
            return Vec::new();
        }
        vec![self.fetch_detail(resume)]
    }

    fn fetch_detail(&mut self, summary: PrSummary) -> Command {
        self.detail_generation += 1;
        self.loading.detail = true;
        Command::FetchDetail {
            generation: self.detail_generation,
            summary,
        }
    }

    /// Défilement courant de la vue détail. Zéro dans la vue liste.
    pub fn detail_scroll(&self) -> u16 {
        match &self.view {
            View::Detail { scroll, .. } => *scroll,
            View::List => 0,
        }
    }

    /// Défile de `pas` lignes, borné au contenu. La hauteur de la zone n'est
    /// pas connue ici : la dernière ligne reste atteignable, et le dessin ne
    /// peut de toute façon pas défiler au-delà.
    fn scroll_detail(&mut self, pas: i32) {
        let maximum = self.detail_line_count().saturating_sub(1) as u16;
        if let View::Detail { scroll, .. } = &mut self.view {
            let vise = i64::from(*scroll) + i64::from(pas);
            *scroll = vise.clamp(0, i64::from(maximum)) as u16;
        }
    }
```

Et le traitement de l'événement, dans `handle` :

```rust
            Event::DetailLoaded {
                generation,
                key,
                result,
            } => {
                if generation != self.detail_generation {
                    return Vec::new();
                }
                self.loading.detail = false;
                match result {
                    Ok(detail) => {
                        self.details.insert(
                            key,
                            CachedDetail {
                                detail,
                                loaded_at: Local::now(),
                            },
                        );
                        self.error = None;
                    }
                    Err(erreur) => self.error = Some(erreur.to_string()),
                }
                Vec::new()
            }
```

Enfin, l'aide clavier dépend de la vue. Remplacer la constante `AIDE` par deux constantes et lire la bonne dans `status_line` :

```rust
const AIDE_LISTE: &str =
    "↑↓ naviguer · → détail · m fusionner · r rafraîchir · o navigateur · q quitter";
const AIDE_DETAIL: &str =
    "↑↓ défiler · ← liste · m fusionner · r rafraîchir · o navigateur · q quitter";
```

```rust
        morceaux.push(
            match self.view {
                View::List => AIDE_LISTE,
                View::Detail { .. } => AIDE_DETAIL,
            }
            .to_string(),
        );
```

Les tests de barre d'état de la tâche 1 comparent `{AIDE}` : y remplacer `AIDE` par `AIDE_LISTE`.

- [ ] **Step 4: Écrire les tests de composition du détail qui échouent**

Dans le `mod tests` de `src/app/render.rs`, ajouter :

```rust
    use crate::app::tests::detail;
    use crate::app::{Command, Event, Key, View};

    /// Détail ouvert sur la PR donnée, réponse livrée.
    fn app_en_detail(numero: u32) -> crate::app::App {
        let mut app = app_garnie(vec![pr(numero)]);
        let generation = match &app.handle(Event::Key(Key::Right))[..] {
            [Command::FetchDetail { generation, .. }] => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(numero).key,
            result: Ok(detail(numero)),
        });
        app
    }

    fn textes(app: &crate::app::App) -> Vec<String> {
        app.detail_lines(LARGE)
            .into_iter()
            .map(|ligne| ligne.text)
            .collect()
    }

    #[test]
    fn l_entete_est_affiche_avant_la_reponse_et_le_reste_indique_le_chargement() {
        let mut app = app_garnie(vec![pr(142)]);
        app.handle(Event::Key(Key::Right));
        assert!(matches!(app.view, View::Detail { .. }));

        let textes = textes(&app);
        assert!(
            textes[0].contains("moi/depot") && textes[0].contains("#142"),
            "l'en-tête vient de PrSummary, déjà en mémoire : {textes:?}"
        );
        assert!(textes[0].contains("Titre 142"), "{textes:?}");
        assert!(
            textes.iter().any(|ligne| ligne.contains("Chargement")),
            "{textes:?}"
        );
    }

    #[test]
    fn le_detail_donne_les_etats_en_clair() {
        let textes = textes(&app_en_detail(1)).join("\n");
        assert!(textes.contains("de ma-branche vers develop"), "{textes}");
        assert!(textes.contains("moi"), "l'auteur : {textes}");
        assert!(
            textes.contains("toutes les vérifications passent"),
            "les mêmes états que la liste, en clair : {textes}"
        );
        assert!(textes.contains("approuvée"), "{textes}");
    }

    #[test]
    fn le_detail_liste_la_description_les_verifications_les_echanges_et_les_fichiers() {
        let textes = textes(&app_en_detail(1)).join("\n");
        assert!(textes.contains("Première ligne."), "{textes}");
        assert!(textes.contains("Seconde ligne."), "{textes}");
        assert!(textes.contains("tests"), "une vérification par ligne : {textes}");
        assert!(textes.contains("collegue"), "une relecture : {textes}");
        assert!(textes.contains("Rebasé."), "un commentaire : {textes}");
        assert!(
            textes.contains("src/app/mod.rs") && textes.contains("+12") && textes.contains("-3"),
            "les fichiers et leurs compteurs : {textes}"
        );
    }

    #[test]
    fn les_relectures_et_les_commentaires_sont_dans_l_ordre_chronologique() {
        let textes = textes(&app_en_detail(1)).join("\n");
        let relecture = textes.find("collegue").expect("la relecture de 10:00");
        let commentaire = textes.find("Rebasé.").expect("le commentaire de 11:00");
        assert!(relecture < commentaire, "{textes}");
    }

    #[test]
    fn le_detail_porte_l_heure_de_son_chargement() {
        let app = app_en_detail(1);
        let heure = app
            .details
            .values()
            .next()
            .expect("un détail en cache")
            .loaded_at
            .format("%H:%M")
            .to_string();
        assert!(
            textes(&app).iter().any(|ligne| ligne.contains(&heure)),
            "le détail peut être périmé : autant dire quand il a été lu"
        );
    }

    #[test]
    fn une_ligne_de_detail_trop_longue_est_tronquee() {
        let app = app_en_detail(1);
        for ligne in app.detail_lines(40) {
            assert!(ligne.text.chars().count() <= 40, "ligne = {}", ligne.text);
        }
    }
```

- [ ] **Step 5: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test render 2>&1 | tail -20`
Expected: `no method named detail_lines found for struct App`.

- [ ] **Step 6: Écrire la composition du détail**

Dans `src/app/render.rs`, compléter l'import — `use crate::app::{App, View};` et `use crate::model::{ChecksState, MergeableState, PrDetail, PrSummary, ReviewState};` — puis ajouter :

```rust
/// Une ligne de la vue détail, prête à dessiner. `tone` absent : couleur par
/// défaut du terminal.
#[derive(Debug, Clone, PartialEq)]
pub struct DetailLine {
    pub text: String,
    pub tone: Option<Tone>,
}

impl DetailLine {
    fn simple(texte: impl Into<String>) -> Self {
        Self {
            text: texte.into(),
            tone: None,
        }
    }

    fn teintee(texte: impl Into<String>, ton: Tone) -> Self {
        Self {
            text: texte.into(),
            tone: Some(ton),
        }
    }
}

const CHARGEMENT_DETAIL: &str = "Chargement du détail…";
const SANS_DESCRIPTION: &str = "(aucune description)";

impl App {
    /// Nombre de lignes du détail. La largeur ne change que leur longueur,
    /// jamais leur nombre : le défilement peut donc se borner sans elle.
    pub(crate) fn detail_line_count(&self) -> usize {
        self.detail_lines(u16::MAX).len()
    }

    /// Compose la vue détail : une seule zone qui défile, pas un ensemble de
    /// panneaux. Tant que la requête n'a pas répondu, l'en-tête vient du
    /// résumé déjà en mémoire et le reste annonce le chargement.
    pub fn detail_lines(&self, width: u16) -> Vec<DetailLine> {
        let View::Detail { key, .. } = &self.view else {
            return Vec::new();
        };
        let Some(resume) = self.prs.iter().find(|pr| &pr.key == key) else {
            return Vec::new();
        };

        let mut lignes = vec![
            DetailLine::simple(format!(
                "{}  #{}  {}",
                resume.key.repo, resume.key.number, resume.title
            )),
            DetailLine::simple(format!("par {}", resume.author)),
        ];

        match self.details.get(key) {
            None => lignes.push(DetailLine::simple(CHARGEMENT_DETAIL)),
            Some(cache) => {
                lignes.extend(corps_du_detail(
                    &cache.detail,
                    &cache.loaded_at.format("%H:%M").to_string(),
                ));
            }
        }

        // La troncature est faite en dernier, sur toutes les lignes à la fois :
        // aucune n'a le droit de dépasser la zone.
        let largeur = width as usize;
        lignes
            .into_iter()
            .map(|ligne| DetailLine {
                text: tronquer(&ligne.text, largeur),
                tone: ligne.tone,
            })
            .collect()
    }
}

/// Corps du détail, dans l'ordre de la spec : branches, états en clair,
/// description, vérifications, échanges, fichiers.
fn corps_du_detail(detail: &PrDetail, heure: &str) -> Vec<DetailLine> {
    let mut lignes = vec![
        DetailLine::simple(format!(
            "de {} vers {}",
            detail.head_ref, detail.base_ref
        )),
        DetailLine::teintee(
            libelle_verifications(detail.summary.checks),
            glyphe_verifications(detail.summary.checks).tone,
        ),
        DetailLine::teintee(
            libelle_relecture(detail.summary.review),
            glyphe_relecture(detail.summary.review).tone,
        ),
        DetailLine::simple(libelle_fusion(detail.summary.mergeable)),
        DetailLine::simple(String::new()),
    ];

    if detail.body.trim().is_empty() {
        lignes.push(DetailLine::simple(SANS_DESCRIPTION));
    } else {
        lignes.extend(detail.body.lines().map(DetailLine::simple));
    }
    lignes.push(DetailLine::simple(String::new()));

    lignes.push(DetailLine::simple(format!(
        "Vérifications ({})",
        detail.checks.len()
    )));
    for verification in &detail.checks {
        let glyphe = glyphe_verifications(verification.state);
        lignes.push(DetailLine::teintee(
            format!("  {} {}", glyphe.symbol, verification.name),
            glyphe.tone,
        ));
    }
    lignes.push(DetailLine::simple(String::new()));

    lignes.push(DetailLine::simple("Relectures et commentaires"));
    lignes.extend(echanges(detail));
    lignes.push(DetailLine::simple(String::new()));

    lignes.push(DetailLine::simple(format!(
        "Fichiers modifiés ({}) · +{} -{}",
        detail.files.len(),
        detail.additions,
        detail.deletions
    )));
    for fichier in &detail.files {
        lignes.push(DetailLine::simple(format!(
            "  {}  +{} -{}",
            fichier.path, fichier.additions, fichier.deletions
        )));
    }

    lignes.push(DetailLine::simple(String::new()));
    lignes.push(DetailLine::teintee(
        format!("Détail chargé à {heure}"),
        Tone::Gris,
    ));
    lignes
}

/// Relectures et commentaires fondus dans un seul fil chronologique : c'est
/// l'ordre dans lequel la conversation a eu lieu.
fn echanges(detail: &PrDetail) -> Vec<DetailLine> {
    let mut fil: Vec<(chrono::DateTime<chrono::Utc>, String)> = Vec::new();
    for relecture in &detail.reviews {
        fil.push((
            relecture.submitted_at,
            format!(
                "  {} · {} · {}",
                relecture.author,
                libelle_relecture(relecture.state),
                relecture.body.replace('\n', " ")
            ),
        ));
    }
    for commentaire in &detail.comments {
        fil.push((
            commentaire.created_at,
            format!(
                "  {} · {}",
                commentaire.author,
                commentaire.body.replace('\n', " ")
            ),
        ));
    }
    fil.sort_by_key(|(instant, _)| *instant);
    fil.into_iter()
        .map(|(_, texte)| DetailLine::simple(texte))
        .collect()
}

fn libelle_verifications(etat: ChecksState) -> &'static str {
    match etat {
        ChecksState::Success => "toutes les vérifications passent",
        ChecksState::Failure => "au moins une vérification échoue",
        ChecksState::Pending => "vérifications en cours",
        ChecksState::None => "aucune vérification",
    }
}

fn libelle_relecture(etat: ReviewState) -> &'static str {
    match etat {
        ReviewState::Approved => "approuvée",
        ReviewState::ChangesRequested => "changements demandés",
        ReviewState::ReviewRequired => "relecture attendue",
        ReviewState::None => "rien à signaler",
    }
}

fn libelle_fusion(etat: MergeableState) -> &'static str {
    match etat {
        MergeableState::Mergeable => "fusion possible",
        MergeableState::Conflicting => "conflits à résoudre",
        // Une attente, pas un blocage : GitHub calcule ce champ à la demande.
        MergeableState::Unknown => "état de fusion en cours de calcul",
    }
}
```

- [ ] **Step 7: Dessiner la vue détail et brancher l'aiguillage**

Remplacer tout le contenu de `src/ui/detail.rs` par :

```rust
//! Dessin de la vue détail d'une pull request.
//!
//! Une seule zone qui défile. Aucune décision : les lignes et leurs tons sont
//! composés par `app`.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, Tone};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let zones = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let cadre = Block::default().borders(Borders::ALL).title(" owl — détail ");
    let interieur = cadre.inner(zones[0]);
    frame.render_widget(cadre, zones[0]);

    let lignes: Vec<Line> = app
        .detail_lines(interieur.width)
        .into_iter()
        .map(|ligne| {
            let style = match ligne.tone {
                Some(ton) => Style::default().fg(couleur(ton)),
                None => Style::default(),
            };
            Line::from(Span::styled(ligne.text, style))
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lignes).scroll((app.detail_scroll(), 0)),
        interieur,
    );

    frame.render_widget(Paragraph::new(app.status_line()), zones[1]);
}

fn couleur(ton: Tone) -> Color {
    match ton {
        Tone::Vert => Color::Green,
        Tone::Rouge => Color::Red,
        Tone::Jaune => Color::Yellow,
        Tone::Gris => Color::DarkGray,
    }
}
```

`couleur` est maintenant écrite deux fois, dans `list.rs` et dans `detail.rs`. La déplacer dans `src/ui/mod.rs` et l'importer depuis les deux : `pub(crate) fn couleur(ton: Tone) -> Color`, avec `use crate::ui::couleur;` dans les deux fichiers de dessin.

Remplacer le corps de `draw` dans `src/ui/mod.rs` par l'aiguillage réel :

```rust
pub fn draw(frame: &mut Frame, app: &App) {
    let zone = frame.area();
    match app.view {
        View::List => list::draw(frame, zone, app),
        View::Detail { .. } => detail::draw(frame, zone, app),
    }
}
```

avec `use crate::app::{App, Tone, View};` en tête du fichier.

- [ ] **Step 8: Exécuter `FetchDetail` dans `main.rs`**

Ajouter ce bras dans `execute_command`, à côté de `Command::FetchList` :

```rust
        Command::FetchDetail {
            generation,
            summary,
        } => {
            let envoi = envoi.clone();
            let client = client.clone();
            let cle = summary.key.clone();
            tokio::spawn(async move {
                let resultat = client.fetch_detail(&summary).await;
                let _ = envoi.send(Event::DetailLoaded {
                    generation,
                    key: cle,
                    result: resultat,
                });
            });
        }
```

- [ ] **Step 9: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test 2>&1 | tail -30`
Expected: tous les tests passent, dont les onze tests d'état du détail et les six de composition.

- [ ] **Step 10: Lancer les quatre commandes**

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check
```
Expected: tout passe.

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "Ajoute la vue détail, son cache de session et son défilement"
```

---

### Task 4: Ouverture dans le navigateur, et la touche `m` en attente

**Files:**
- Modify: `src/app/mod.rs` (`Command::OpenInBrowser`, touches `o` et `m`)
- Modify: `src/main.rs` (exécute `OpenInBrowser`)
- Modify: `docs/specs/04-fusion.md` (note d'implémentation sur la touche `m`)
- Modify: `docs/specs/03-affichage-et-navigation.md` (note d'implémentation sur la touche `m`)

**Interfaces:**
- Consumes: tout ce que les tâches 1 à 3 produisent ; `PrSummary::url` de `model`.
- Produces: `Command::OpenInBrowser { url: String }`

- [ ] **Step 1: Écrire les tests qui échouent**

Dans le `mod tests` de `src/app/mod.rs` :

```rust
    #[test]
    fn o_ouvre_la_pr_selectionnee_dans_le_navigateur() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Down));
        assert_eq!(
            app.handle(Event::Key(Key::Char('o'))),
            vec![Command::OpenInBrowser {
                url: "https://github.com/moi/depot/pull/2".to_string()
            }]
        );
    }

    #[test]
    fn o_en_vue_detail_ouvre_la_pr_affichee() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Down));
        ouvrir_detail(&mut app);
        assert_eq!(
            app.handle(Event::Key(Key::Char('o'))),
            vec![Command::OpenInBrowser {
                url: "https://github.com/moi/depot/pull/2".to_string()
            }]
        );
    }

    #[test]
    fn o_sur_une_liste_vide_ne_fait_rien() {
        let mut app = app_garnie(vec![]);
        assert!(app.handle(Event::Key(Key::Char('o'))).is_empty());
    }

    #[test]
    fn m_est_reconnue_et_reste_sans_effet_jusqu_a_la_spec_04() {
        let mut app = app_garnie(vec![pr(1)]);
        let commandes = app.handle(Event::Key(Key::Char('m')));
        assert!(commandes.is_empty(), "commandes = {commandes:?}");
        assert!(matches!(app.view, View::List), "aucun changement de vue");
        assert!(app.error.is_none(), "et aucun message d'erreur");
    }
```

- [ ] **Step 2: Lancer les tests pour vérifier qu'ils échouent**

Run: `cargo test app 2>&1 | tail -20`
Expected: `no variant or associated item named OpenInBrowser found for enum Command`.

- [ ] **Step 3: Écrire l'implémentation**

Ajouter la variante dans `Command` :

```rust
    /// Ouvre une URL dans le navigateur. `app` choisit l'URL, `main` l'ouvre :
    /// `app` ne fait aucun effet de bord lui-même.
    OpenInBrowser { url: String },
```

Ajouter le bras dans les touches communes de `handle_key`, avant l'aiguillage par vue :

```rust
            Key::Char('o') => return self.open_in_browser(),
            // La fenêtre de fusion et ses contrôles appartiennent à
            // `04-fusion.md`. La touche est reconnue ici pour ne pas tomber
            // dans un bras de navigation, et ne fait rien de plus.
            Key::Char('m') => return Vec::new(),
```

Et la méthode, à côté de `open_detail` :

```rust
    /// URL de la pull request affichée : la sélection dans la liste, la PR
    /// ouverte dans le détail.
    fn open_in_browser(&self) -> Vec<Command> {
        let resume = match &self.view {
            View::List => self.selected_pr(),
            View::Detail { key, .. } => self.prs.iter().find(|pr| &pr.key == key),
        };
        match resume {
            Some(pr) => vec![Command::OpenInBrowser {
                url: pr.url.clone(),
            }],
            None => Vec::new(),
        }
    }
```

- [ ] **Step 4: Exécuter la commande dans `main.rs`**

Ajouter ce bras dans `execute_command` :

```rust
        Command::OpenInBrowser { url } => {
            // Dans une tâche bloquante : lancer le navigateur peut prendre un
            // instant, et l'écran doit rester réactif pendant ce temps.
            // Un échec reste silencieux ; la remontée des erreurs de cette
            // nature appartient à `05-erreurs-et-tests.md`.
            tokio::task::spawn_blocking(move || {
                let _ = open::that_detached(&url);
            });
        }
```

- [ ] **Step 5: Lancer les tests pour vérifier qu'ils passent**

Run: `cargo test 2>&1 | tail -20`
Expected: tous les tests passent, dont les quatre neufs.

- [ ] **Step 6: Noter la touche `m` dans les deux specs**

Dans `docs/specs/04-fusion.md`, ajouter cette section juste avant les critères de réussite :

```markdown
## Note d'implémentation

La touche `m` est déjà reconnue par `app` depuis
`03-affichage-et-navigation.md`, où elle ne fait rien : ni `MergeDialog`, ni
contrôles avant fusion. Cette spec apporte le champ `merge` de `App`, la
fenêtre, la capture du clavier, et le fait qu'un `Tick` ne rafraîchisse pas la
liste tant que la fenêtre est ouverte.
```

Dans `docs/specs/03-affichage-et-navigation.md`, compléter la section « Note d'implémentation » par ce paragraphe :

```markdown
Le champ `merge` de `App` et le blocage du rafraîchissement pendant la fenêtre
de fusion ne sont pas apportés par cette spec : la touche `m` y est reconnue
mais sans effet, et `04-fusion.md` s'en charge.
```

- [ ] **Step 7: Lancer les quatre commandes**

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check
```
Expected: tout passe.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "Ouvre la pull request dans le navigateur et réserve la touche m"
```

---

### Task 5: Vérification, mise à jour des specs et pull request

**Files:**
- Modify: `docs/specs/03-affichage-et-navigation.md` (décisions prises en cours de route)
- Modify: `docs/specs/01-modele-et-donnees.md` (la ligne « … et N de plus » est abandonnée)
- Modify: `docs/specs/00-fondations.md` (structure des modules : `app/`)
- Modify: `docs/suivi/DETTE.md` (l'entrée sur la troncature des listes du détail est résolue)
- Un écart révélé ici se corrige dans le fichier concerné.

**Interfaces:**
- Consumes: tout ce que les tâches 1 à 4 produisent.
- Produces: rien. C'est une porte, pas un changement.

- [ ] **Step 1: Vérifier les critères de réussite de la spec, un par un**

```bash
cargo test 2>&1 | tail -40
```

Chaque critère de `03-affichage-et-navigation.md` doit pointer sur un test nommé :

| Critère de la spec | Test |
|---|---|
| Les flèches haut et bas déplacent la sélection et ne débordent pas des extrémités | `les_fleches_deplacent_la_selection`, `la_selection_ne_deborde_pas_des_extremites`, `j_et_k_deplacent_la_selection_comme_les_fleches` |
| La flèche droite passe en vue détail et émet `FetchDetail` ; la flèche gauche revient à la liste | `la_fleche_droite_ouvre_le_detail_et_demande_les_donnees`, `la_fleche_gauche_et_echap_reviennent_a_la_liste` |
| Ouvrir une PR déjà en cache n'émet aucune commande | `ouvrir_une_pr_deja_en_cache_n_emet_aucune_commande` |
| Après un rafraîchissement où la PR sélectionnée est toujours présente mais à un autre indice, la sélection la suit | `le_rafraichissement_suit_la_pr_selectionnee`, `deux_depots_de_meme_numero_ne_sont_pas_confondus` |
| Après un rafraîchissement où la PR sélectionnée a disparu, la sélection reste dans les bornes | `une_pr_disparue_laisse_la_selection_dans_les_bornes`, `une_liste_devenue_vide_n_a_plus_de_selection` |
| Un résultat de liste portant une génération périmée est ignoré | `un_resultat_perime_est_ignore`, `un_detail_perime_est_ignore` |
| Un `Tick` reçu pendant un chargement de liste n'émet pas de seconde requête | `un_tick_pendant_un_chargement_de_liste_ne_relance_rien`, `un_tick_apres_la_reponse_relance_la_liste` |

Un critère sans test est un manque : écrire le test avant de continuer.

- [ ] **Step 2: Vérifier qu'aucune décision d'affichage n'a fui dans `ui`**

```bash
grep -rn "format!\|to_string()\|push_str" src/ui/
```

Expected: aucun résultat, hors `ligne.checks.symbol.to_string()` et `ligne.review.symbol.to_string()` de `list.rs`, qui ne composent aucun contenu — ils convertissent un caractère déjà choisi par `app` en `Span`. Tout autre résultat est une décision au mauvais endroit : la déplacer dans `app/render.rs`.

- [ ] **Step 3: Vérifier à l'œil dans le terminal**

`claude-in-chrome` est inapplicable : `owl` est une interface en mode texte, pas une page web, et aucun compte de test n'est à connecter. La vérification se fait donc dans le terminal, contre les maquettes en texte de la spec.

```bash
cargo run
```

Expected, dans l'ordre :

1. La liste s'ouvre : deux colonnes de pictogrammes alignées, puis le dépôt, le numéro, le titre — la forme de la maquette de la spec, section « Vue liste ».
2. Les flèches haut et bas déplacent le surlignage, sans boucler aux extrémités.
3. Un brouillon apparaît grisé et préfixé `[brouillon]`, une PR en conflit porte `⚠` devant son titre.
4. La flèche droite ouvre le détail : l'en-tête s'affiche aussitôt, le reste annonce le chargement, puis le contenu arrive. Les flèches font défiler. La flèche gauche revient à la liste, et rouvrir la même PR est instantané.
5. Réduire la largeur de la fenêtre : le titre se tronque avec `…`, le dépôt et le numéro restent entiers. En dessous de leur largeur, le message d'élargissement remplace la liste.
6. `o` ouvre la pull request dans le navigateur.
7. `m` ne fait rien du tout : ni fenêtre, ni message.
8. `q` puis, sur une nouvelle exécution, `Ctrl+C` : les deux rendent un terminal propre.

Pour voir la liste vide, lancer avec un filtre qui ne ramène rien :

```bash
printf 'filters = ["author:@me", "label:\\"ce-libelle-n-existe-pas\\""]\n' > /private/tmp/owl-vide.toml
```

puis copier ce fichier en `~/.config/owl/config.toml` **après avoir sauvegardé l'existant**, lancer `cargo run`, vérifier le message « Aucune pull request » suivi du rappel des filtres, et restaurer le fichier d'origine. Sans jeton disponible, cette étape n'est pas réalisable : le noter et passer à la suivante.

- [ ] **Step 4: Traiter un écart révélé par l'étape 3**

Reproduire d'abord en test, dans `src/app/render.rs` pour un écart d'affichage, dans `src/app/mod.rs` pour un écart de navigation. Puis corriger. Un écart qui appartient visiblement à une autre spec — un message d'erreur à reformuler, un solde d'appels épuisé, la fenêtre de fusion — se consigne dans `docs/suivi/DETTE.md` plutôt que de se corriger ici.

- [ ] **Step 5: Reporter les décisions dans `docs/specs/03-affichage-et-navigation.md`**

L'ordre de vérité du `CLAUDE.md` l'exige : le code s'écarte du texte de la spec, donc la spec est mise à jour dans le même commit. Reprendre le bloc `App` de la section « État de l'application » pour qu'il décrive ce qui est écrit, et ajouter les décisions 1 à 12 de la section « Décisions prises en écrivant ce plan » de ce plan, formulées comme des règles et non comme un historique. Au minimum :

- `App` porte `list_generation` et `detail_generation`, non un seul `generation`, pour qu'ouvrir un détail ne périme pas une requête de liste en vol.
- `Event::ListLoaded` transporte un `ListPage` — les pull requests et le solde d'appels lu au passage.
- `Event::Quit` existe en plus de la liste donnée : le crochet de panique en a besoin.
- `Command::FetchDetail` porte le `PrSummary` entier ; `Command::OpenInBrowser { url }` sert la touche `o`.
- `details` associe une clé à un `CachedDetail { detail, loaded_at }` : l'heure de chargement est affichée en fin de vue détail.
- La vue détail ne renvoie pas à la ligne : une ligne logique vaut une ligne d'écran, ce qui permet de borner le défilement sans connaître la hauteur.
- Les largeurs se mesurent en caractères, pas en colonnes de terminal.
- Un brouillon en conflit affiche `[brouillon] ⚠ Titre`.
- `app` expose l'affichage sous forme de `ListRender` et de `Vec<DetailLine>`, avec un `Tone` par élément que `ui` traduit en couleur.
- Le message affiché quand la fenêtre est trop étroite : « Élargis le terminal : le dépôt et le numéro n'y tiennent pas. »
- L'aide clavier de la barre d'état dépend de la vue.

- [ ] **Step 6: Trancher la ligne « … et N de plus » dans la spec 01**

La spec 01 laissait la spec 03 décider. La décision est l'abandon : la spec 03 décrit les listes du détail sans jamais mentionner de troncature, et afficher un compte demanderait d'ajouter des `totalCount` à la requête que personne ne consomme.

Dans `docs/specs/01-modele-et-donnees.md`, supprimer la phrase « Quand une liste est tronquée, l'écran l'indique par une ligne « … et N de plus » » et le paragraphe de note d'implémentation qui explique qu'elle n'est pas réalisable. Ajouter à sa place une phrase disant que les listes du détail sont bornées par la requête et que le dépassement n'est pas signalé, `o` ouvrant la pull request dans le navigateur pour tout voir.

Puis, dans `docs/suivi/DETTE.md`, supprimer entièrement la section « La troncature des listes de la vue détail n'est pas mesurable » : elle est tranchée, elle sort du registre.

- [ ] **Step 7: Mettre à jour la structure des modules de la spec 00**

Dans `docs/specs/00-fondations.md`, dans le bloc « Structure des modules », remplacer la ligne `app.rs` par :

```
  app/
    mod.rs       état de l'application, réception des événements
    render.rs    composition de l'affichage : pictogrammes, colonnes, messages
```

- [ ] **Step 8: Lancer les quatre commandes une dernière fois**

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check
```
Expected: tout passe.

- [ ] **Step 9: Commit et pull request**

```bash
git add -A
git commit -m "Met les specs à jour et corrige les écarts relevés à la vérification"
git push -u origin feat/affichage-et-navigation
gh pr create --base develop --title "Affichage et navigation" --body "<corps en français>"
```

La pull request va **vers `develop`**, jamais vers `main`. Titre et corps en français ; le corps renvoie à `docs/specs/03-affichage-et-navigation.md` et liste les critères de réussite couverts. S'il n'y a rien à committer à cette étape, pousser directement.

- [ ] **Step 10: Rapporter**

Le rapport final ne raconte pas le travail fait. Il donne uniquement, s'il y en a, ce que l'humain doit poser ou installer lui-même — un jeton, un outil absent, une variable d'environnement — et le lien de la pull request. Rien sur `docs/suivi/DETTE.md`, rien sur les manipulations git, rien sur les vérifications à l'œil, rien sur `docs/suivi/TODO.md`. S'il n'y a aucune action humaine à mener, le dire en une phrase avec le lien de la pull request.
