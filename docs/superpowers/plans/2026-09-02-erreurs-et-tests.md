# Erreurs et tests — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fermer les trois trous restants de `docs/specs/05-erreurs-et-tests.md` : la suspension du rafraîchissement quand la limite d'appels est atteinte, le refus de démarrer sur un jeton refusé ou des droits insuffisants, et les tests des critères de réussite encore sans couverture.

**Architecture:** La suspension est une décision, donc elle vit dans `app` : un champ d'heure de reprise, consulté par le minuteur, par la touche `r` et par la barre d'état. Le contrôle de démarrage vit dans un nouveau module `startup`, une fonction pure qui classe la première réponse de GitHub en « démarrage possible » ou « erreur fatale » ; `main` fait cette première requête avant de prendre le terminal, puis injecte son résultat dans la boucle. Aucun nouveau module ne dépend du terminal.

**Tech Stack:** Rust 2021, `chrono` (heures locales), `thiserror`, `tokio`, `ratatui`. Tests : `cargo test` seul, sans réseau ni terminal.

**Spec:** `docs/specs/05-erreurs-et-tests.md`

**Branche de travail :** `feat/erreurs-et-tests`, créée depuis `develop`. La pull request finale vise `develop`, jamais `main`.

**Périmètre de l'agent d'exécution :** les tâches 1 à 3. La tâche 4 est une vérification à l'œil dans un vrai terminal, réservée au propriétaire du projet : l'agent ne l'exécute pas et n'en rend pas compte.

**Pas de lien de design** pour ce plan : la spec ne décrit aucun écran nouveau, seulement un morceau de texte de plus dans la barre d'état existante.

## Global Constraints

- Les identifiants du code sont en anglais ; messages affichés, commentaires et commits en français, accents compris.
- Dépendances entre modules à sens unique : `model` et `filter` ignorent réseau et terminal ; `github` dépend de `model` et `filter` ; `app` ne fait aucun appel réseau ; `ui` lit `app` et dessine, sans décider.
- Toute décision d'affichage — message, pictogramme, troncature — appartient à `app` ou `model`, jamais à `ui`.
- Les messages d'erreur de GitHub sont affichés tels quels, sans reformulation.
- Le jeton n'est jamais écrit dans un fichier, journalisé ni affiché.
- Le terminal est toujours restauré à la sortie, panique comprise.
- Avant de considérer une tâche finie : `cargo build`, `cargo test`, `cargo clippy -- -D warnings` et `cargo fmt --check` passent tous. Aucune n'est optionnelle.
- Si le code doit s'écarter de la spec, la spec est mise à jour dans le même commit.

## État des lieux (déjà fait, ne pas refaire)

Vérifié dans le code avant d'écrire ce plan :

- Les messages de démarrage du tableau de la spec existent : `TokenError::GhMissing`, `TokenError::GhNotAuthenticated` dans `src/token.rs` ; `ConfigError::{Syntax, Unreadable, InvalidKey, EmptyFilters, NoHomeDirectory}` dans `src/config.rs`. `main` les écrit sur la sortie d'erreur avant toute prise de contrôle du terminal et rend `ExitCode::FAILURE`.
- `src/github/mod.rs` classe déjà 401, 403 sans limite, 403 avec solde épuisé, `retry-after`, tableau `errors`, corps tronqué, transport, et chaque cas a son test contre un serveur local.
- `tests/startup.rs` couvre `gh` absent, valeur de réglage invalide, syntaxe TOML cassée, filtres vides — message exact, code non nul, sortie standard vierge.
- `ListRender::TooNarrow` et le message « Élargis le terminal… » existent dans `src/app/render.rs`, avec leur test.
- `TerminalGuard` et le crochet de panique existent dans `src/main.rs`.
- `App::error` est déjà effacée par une réponse réussie (`src/app/mod.rs`, branches `ListLoaded` et `DetailLoaded`).

## File Structure

- `src/app/mod.rs` — modifié : champ `suspended_until`, méthode `suspension`, prise en compte du solde et du refus de limite, refus du `Tick` et de `r`, morceau de barre d'état. Fichier déjà volumineux mais organisé par responsabilité ; la suspension est de l'état d'application, elle y a sa place.
- `src/startup.rs` — créé : classement de la première réponse de GitHub. Un seul type, une seule fonction, tests unitaires purs.
- `src/main.rs` — modifié : première requête avant la prise du terminal, remontée de l'erreur fatale, injection du premier résultat dans la boucle.
- `src/config.rs` — modifié : un test du message de `NoHomeDirectory`.
- `docs/specs/05-erreurs-et-tests.md` — modifié : une phrase sur la reprise quand GitHub ne donne pas d'heure.

---

### Task 1 : Suspension du rafraîchissement sur limite d'appels

**Files:**
- Modify: `src/app/mod.rs`
- Modify: `docs/specs/05-erreurs-et-tests.md`
- Test: `src/app/mod.rs` (module `tests` en fin de fichier)

**Interfaces:**
- Consomme : `crate::model::RateLimit { remaining: u32, reset_at: DateTime<Utc> }`, `crate::github::GithubError::RateLimited { reset_at: Option<DateTime<Utc>> }`, `App::status_line(&self, width: u16) -> String`, les aides de test `app_demarree()`, `app_garnie(Vec<PrSummary>)`, `page(Vec<PrSummary>)`, `pr(u32)`.
- Produit : champ privé `App::suspended_until: Option<DateTime<Local>>`, méthode privée `App::suspension(&self) -> Option<DateTime<Local>>`, fonction libre privée `message_de_suspension(reprise: DateTime<Local>) -> String`, aide de test `pub(crate) fn page_avec_solde(pull_requests: Vec<PrSummary>, remaining: u32, reset_at: DateTime<Utc>) -> ListPage`.

- [ ] **Step 1 : Écrire les tests qui échouent**

À ajouter à la fin du module `tests` de `src/app/mod.rs`. Le module de tests n'importe ni `Utc` ni `Duration` : les chemins complets sont écrits ci-dessous, les garder tels quels.

```rust
    /// Réponse de liste portant un solde d'appels, pour les tests de suspension.
    pub(crate) fn page_avec_solde(
        pull_requests: Vec<PrSummary>,
        remaining: u32,
        reset_at: chrono::DateTime<chrono::Utc>,
    ) -> ListPage {
        ListPage {
            pull_requests,
            rate_limit: Some(RateLimit {
                remaining,
                reset_at,
            }),
        }
    }

    /// Livre une réponse de liste donnée en respectant la génération en vol.
    fn livrer(app: &mut App, generation: Generation, resultat: Result<ListPage, GithubError>) {
        app.handle(Event::ListLoaded {
            generation,
            result: resultat,
        });
    }

    #[test]
    fn un_solde_epuise_suspend_le_minuteur() {
        let (mut app, generation) = app_demarree();
        let reprise = chrono::Utc::now() + chrono::Duration::minutes(30);
        livrer(
            &mut app,
            generation,
            Ok(page_avec_solde(vec![pr(1)], 0, reprise)),
        );
        assert!(
            app.handle(Event::Tick).is_empty(),
            "le minuteur ne doit plus demander de liste"
        );
    }

    #[test]
    fn un_solde_non_nul_ne_suspend_rien() {
        let (mut app, generation) = app_demarree();
        let reprise = chrono::Utc::now() + chrono::Duration::minutes(30);
        livrer(
            &mut app,
            generation,
            Ok(page_avec_solde(vec![pr(1)], 12, reprise)),
        );
        assert!(
            !app.handle(Event::Tick).is_empty(),
            "un solde restant ne doit rien suspendre"
        );
    }

    #[test]
    fn la_barre_d_etat_annonce_l_heure_de_reprise() {
        let (mut app, generation) = app_demarree();
        let reprise = chrono::Utc::now() + chrono::Duration::minutes(30);
        livrer(
            &mut app,
            generation,
            Ok(page_avec_solde(vec![pr(1)], 0, reprise)),
        );
        let attendu = format!(
            "limite d'appels atteinte, reprise à {}",
            reprise.with_timezone(&Local).format("%H h %M")
        );
        let ligne = app.status_line(CONFORTABLE);
        assert!(ligne.contains(&attendu), "ligne = {ligne}");
    }

    #[test]
    fn la_touche_r_est_refusee_pendant_la_suspension() {
        let (mut app, generation) = app_demarree();
        let reprise = chrono::Utc::now() + chrono::Duration::minutes(30);
        livrer(
            &mut app,
            generation,
            Ok(page_avec_solde(vec![pr(1)], 0, reprise)),
        );
        assert!(
            app.handle(Event::Key(Key::Char('r'))).is_empty(),
            "r doit être refusée pendant la suspension"
        );
        assert_eq!(app.prs.len(), 1, "la liste reste affichée");
    }

    #[test]
    fn la_reprise_passee_rend_la_main_au_minuteur() {
        let (mut app, generation) = app_demarree();
        let reprise = chrono::Utc::now() - chrono::Duration::minutes(1);
        livrer(
            &mut app,
            generation,
            Ok(page_avec_solde(vec![pr(1)], 0, reprise)),
        );
        assert!(
            !app.handle(Event::Tick).is_empty(),
            "l'heure de reprise passée, le rafraîchissement repart"
        );
        let ligne = app.status_line(CONFORTABLE);
        assert!(
            !ligne.contains("limite d'appels"),
            "l'annonce disparaît avec la suspension : {ligne}"
        );
    }

    #[test]
    fn un_refus_pour_limite_suspend_au_lieu_d_afficher_l_erreur() {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        let reprise = chrono::Utc::now() + chrono::Duration::minutes(15);
        livrer(
            &mut app,
            generation,
            Err(GithubError::RateLimited {
                reset_at: Some(reprise),
            }),
        );
        assert!(app.error.is_none(), "erreur = {:?}", app.error);
        assert_eq!(app.prs.len(), 1, "la liste précédente reste visible");
        assert!(app.handle(Event::Tick).is_empty());
        let ligne = app.status_line(CONFORTABLE);
        assert!(ligne.contains("limite d'appels atteinte"), "ligne = {ligne}");
    }

    #[test]
    fn un_refus_pour_limite_sans_heure_suspend_quand_meme() {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        livrer(
            &mut app,
            generation,
            Err(GithubError::RateLimited { reset_at: None }),
        );
        assert!(
            app.handle(Event::Tick).is_empty(),
            "owl ne doit jamais réessayer en boucle une requête refusée pour limite"
        );
    }
```

- [ ] **Step 2 : Lancer les tests pour les voir échouer**

Run: `cargo test un_solde_epuise_suspend_le_minuteur -- --nocapture`
Expected: FAIL — `page_avec_solde` et `livrer` compilent, mais `Event::Tick` renvoie encore une commande (`assertion failed`). Les tests qui appellent `app.error` compilent déjà.

- [ ] **Step 3 : Ajouter l'état de suspension**

Dans `src/app/mod.rs`, ajouter le champ à la structure `App`, juste après `rate_limit`, et retirer le `#[allow(dead_code)]` posé sur `rate_limit` (il devient lu) :

```rust
    /// Solde d'appels rapporté par la dernière requête réussie.
    pub rate_limit: Option<RateLimit>,
    /// Heure de reprise quand la limite d'appels est atteinte. Tant qu'elle
    /// n'est pas passée, le rafraîchissement automatique est suspendu et `r`
    /// est refusée. Elle s'éteint d'elle-même : rien n'a à la remettre à zéro.
    suspended_until: Option<DateTime<Local>>,
```

Dans `App::new`, initialiser après `rate_limit: None,` :

```rust
            suspended_until: None,
```

- [ ] **Step 4 : Décider de la suspension à la réception d'une réponse**

Toujours dans `src/app/mod.rs`, remplacer les deux branches de `Event::ListLoaded` par un appel aux nouvelles méthodes :

```rust
                match result {
                    Ok(page) => {
                        self.apply_list(page.pull_requests);
                        self.rate_limit = page.rate_limit;
                        self.last_refresh = Some(Local::now());
                        self.error = None;
                        self.note_rate_limit();
                    }
                    // Message de GitHub repris tel quel, et liste conservée.
                    Err(erreur) => self.note_error(erreur),
                }
```

Dans la branche `Event::DetailLoaded`, remplacer `Err(erreur) => self.error = Some(erreur.to_string()),` par `Err(erreur) => self.note_error(erreur),` : un refus pour limite d'appels ne doit pas plus se répéter sur un détail que sur une liste.

Ajouter les deux méthodes dans le `impl App`, à côté de `fetch_list` :

```rust
    /// Suspend le rafraîchissement quand le solde rapporté est épuisé.
    fn note_rate_limit(&mut self) {
        if let Some(limite) = &self.rate_limit {
            if limite.remaining == 0 {
                self.suspended_until = Some(limite.reset_at.with_timezone(&Local));
            }
        }
    }

    /// Retient une erreur de requête. Un refus pour limite d'appels ne laisse
    /// pas de message d'erreur : il suspend le rafraîchissement, et la barre
    /// d'état l'annonce avec l'heure de reprise.
    fn note_error(&mut self, erreur: GithubError) {
        match erreur {
            GithubError::RateLimited { reset_at } => {
                let reprise = reset_at
                    .map(|heure| heure.with_timezone(&Local))
                    .unwrap_or_else(|| Local::now() + Duration::seconds(REPRISE_INCONNUE));
                self.suspended_until = Some(reprise);
            }
            autre => self.error = Some(autre.to_string()),
        }
    }

    /// Heure de reprise si la suspension court encore. Rend `None` dès que
    /// l'heure est passée : la suspension s'éteint sans que rien la lève.
    fn suspension(&self) -> Option<DateTime<Local>> {
        self.suspended_until.filter(|heure| *heure > Local::now())
    }
```

Ajouter la constante près des autres messages, en haut du fichier :

```rust
/// Attente imposée quand GitHub refuse pour limite d'appels sans donner
/// d'heure de reprise — le cas des limites secondaires sans `retry-after`.
/// Une minute suffit à casser la boucle de réessais, que la spec interdit.
const REPRISE_INCONNUE: i64 = 60;
```

Compléter les imports `chrono` en tête de fichier :

```rust
use chrono::{DateTime, Duration, Local};
```

- [ ] **Step 5 : Refuser le tour de minuteur et la touche `r`**

Dans `Event::Tick`, ajouter la suspension aux motifs de renoncement :

```rust
            // Une requête de liste déjà en vol suffit, la liste ne change pas
            // sous une fenêtre de fusion ouverte, et une limite d'appels
            // atteinte interdit de réessayer : le tour est perdu, le suivant
            // s'en chargera.
            Event::Tick => {
                if self.loading.list || self.merge.is_some() || self.suspension().is_some() {
                    Vec::new()
                } else {
                    vec![self.fetch_list()]
                }
            }
```

Au début de `fn refresh`, refuser la touche :

```rust
    fn refresh(&mut self) -> Vec<Command> {
        // Suspension en cours : la touche est refusée. Aucun message n'est
        // posé ici, la barre d'état porte déjà l'annonce et son heure de
        // reprise — l'écrire deux fois sur la même ligne n'apprend rien.
        if self.suspension().is_some() {
            return Vec::new();
        }
        match &self.view {
```

- [ ] **Step 6 : Annoncer la suspension dans la barre d'état**

Dans `App::status_line`, corriger la condition d'attente initiale — une suspension est un message, l'attente n'est plus le sujet — puis ajouter le morceau :

```rust
        if self.last_refresh.is_none()
            && self.error.is_none()
            && self.notice.is_none()
            && self.suspension().is_none()
        {
            // Rien n'est encore arrivé : l'attente est le message principal.
            morceaux.push((ERREUR, ATTENTE_INITIALE.to_string()));
        } else {
```

Puis, juste après le bloc qui pousse `self.notice` :

```rust
        // Suspension pour limite d'appels : au même rang que l'erreur, elle
        // dit pourquoi la liste ne se rafraîchit plus.
        if let Some(reprise) = self.suspension() {
            morceaux.push((ERREUR, message_de_suspension(reprise)));
        }
```

Ajouter la fonction libre à côté de `fn assembler`, en bas du fichier :

```rust
/// Annonce de suspension pour limite d'appels, avec son heure de reprise.
fn message_de_suspension(reprise: DateTime<Local>) -> String {
    format!(
        "limite d'appels atteinte, reprise à {}",
        reprise.format("%H h %M")
    )
}
```

- [ ] **Step 7 : Lancer les tests et vérifier qu'ils passent**

Run: `cargo test`
Expected: PASS, y compris les sept tests ajoutés à l'étape 1.

- [ ] **Step 8 : Passer les quatre commandes de vérification**

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check
```
Expected: aucune erreur, aucun avertissement.

- [ ] **Step 9 : Noter dans la spec le cas non spécifié**

La spec compose son message avec une heure de reprise, mais GitHub n'en donne pas toujours (limite secondaire sans `retry-after`). Le code choisit une minute d'attente ; la spec doit le dire. Dans `docs/specs/05-erreurs-et-tests.md`, section « Limite d'appels », ajouter après le premier paragraphe :

```markdown
Quand GitHub refuse sans donner d'heure de reprise — le cas des limites
secondaires sans en-tête `retry-after` —, `owl` attend une minute avant de
reprendre. L'attente est arbitraire, mais l'interdiction de réessayer en
boucle, elle, ne l'est pas.
```

- [ ] **Step 10 : Commit**

```bash
git add src/app/mod.rs docs/specs/05-erreurs-et-tests.md
git commit -m "Suspend le rafraîchissement quand la limite d'appels est atteinte"
```

---

### Task 2 : Refus de démarrer sur un jeton refusé ou des droits insuffisants

**Files:**
- Create: `src/startup.rs`
- Modify: `src/main.rs`
- Test: `src/startup.rs` (module `tests` en fin de fichier)

**Interfaces:**
- Consomme : `crate::github::GithubError`, `crate::github::Client::fetch_pull_requests(&self, query: &str, page_size: u16) -> Result<ListPage, GithubError>`, `crate::app::{App, Command, Event}`.
- Produit : `pub enum startup::FirstResponse<T> { Start(Result<T, GithubError>), Fatal(String) }` et `pub fn startup::classify<T>(result: Result<T, GithubError>) -> FirstResponse<T>`.

Pourquoi une fonction séparée : la spec range « jeton refusé » et « droits insuffisants » parmi les erreurs de démarrage, qui s'écrivent sur la sortie d'erreur avant toute prise de contrôle du terminal. Aujourd'hui la première requête part après l'entrée dans l'écran alterné, et ces deux refus finissent en petite ligne de barre d'état. Le classement est une décision, donc il se teste ; `main` ne garde que le branchement.

Signatures déjà vérifiées : `fetch_pull_requests(&self, query: &str, page_size: u16) -> Result<ListPage, GithubError>`, et `Config::page_size` est un `u16`.

- [ ] **Step 1 : Écrire le fichier de tests et le module vide**

Créer `src/startup.rs` avec ce contenu exact :

```rust
//! Classement de la première réponse de GitHub, reçue avant toute prise de
//! contrôle du terminal.
//!
//! Deux refus de GitHub sont des erreurs de démarrage et non des messages de
//! barre d'état : un jeton refusé et des droits insuffisants ne se corrigent
//! pas en attendant le prochain rafraîchissement. Tout le reste — réseau
//! injoignable, limite d'appels, réponse illisible — laisse `owl` démarrer :
//! la liste s'affichera vide, l'erreur en barre d'état, et le minuteur
//! retentera.

use crate::github::GithubError;

/// Ce que devient la première réponse de GitHub.
pub enum FirstResponse<T> {
    /// `owl` peut démarrer, avec ce résultat comme premier événement.
    Start(Result<T, GithubError>),
    /// Erreur de démarrage : message sur la sortie d'erreur, code non nul.
    Fatal(String),
}

pub fn classify<T>(result: Result<T, GithubError>) -> FirstResponse<T> {
    match result {
        Err(erreur @ (GithubError::Unauthorized | GithubError::Forbidden)) => {
            FirstResponse::Fatal(erreur.to_string())
        }
        autre => FirstResponse::Start(autre),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rend le message d'une réponse fatale, ou échoue si elle ne l'est pas.
    fn message_fatal(reponse: FirstResponse<u8>) -> String {
        match reponse {
            FirstResponse::Fatal(message) => message,
            FirstResponse::Start(_) => panic!("cette réponse devait être fatale"),
        }
    }

    /// Vrai si la réponse laisse démarrer.
    fn demarre(reponse: FirstResponse<u8>) -> bool {
        matches!(reponse, FirstResponse::Start(_))
    }

    #[test]
    fn un_jeton_refuse_empeche_le_demarrage() {
        let message = message_fatal(classify::<u8>(Err(GithubError::Unauthorized)));
        assert_eq!(
            message,
            "Jeton refusé par GitHub. Lance `gh auth login` pour le renouveler."
        );
    }

    #[test]
    fn des_droits_insuffisants_empechent_le_demarrage() {
        let message = message_fatal(classify::<u8>(Err(GithubError::Forbidden)));
        assert_eq!(
            message,
            "Le jeton n'a pas les droits nécessaires. Vérifie la portée `repo`."
        );
    }

    #[test]
    fn un_reseau_injoignable_laisse_demarrer() {
        assert!(demarre(classify::<u8>(Err(GithubError::Transport))));
    }

    #[test]
    fn une_limite_d_appels_laisse_demarrer() {
        assert!(demarre(classify::<u8>(Err(GithubError::RateLimited {
            reset_at: None
        }))));
    }

    #[test]
    fn une_reponse_illisible_laisse_demarrer() {
        assert!(demarre(classify::<u8>(Err(GithubError::Malformed))));
    }

    #[test]
    fn une_reponse_reussie_laisse_demarrer() {
        assert!(demarre(classify::<u8>(Ok(7))));
    }
}
```

- [ ] **Step 2 : Déclarer le module et lancer les tests**

Dans `src/main.rs`, ajouter `mod startup;` à la liste des modules, en gardant l'ordre alphabétique :

```rust
mod app;
mod config;
mod filter;
mod github;
mod model;
mod startup;
mod token;
mod ui;
```

Run: `cargo test startup`
Expected: PASS, six tests. `cargo clippy` peut signaler `classify` comme jamais appelée hors tests : l'étape suivante l'utilise, ne pas ajouter de `#[allow]`.

- [ ] **Step 3 : Commit du module**

```bash
git add src/startup.rs src/main.rs
git commit -m "Classe la première réponse de GitHub en erreur de démarrage ou non"
```

- [ ] **Step 4 : Faire la première requête avant de prendre le terminal**

Dans `src/main.rs`, fonction `run`, remplacer le bloc qui va de la création de l'état jusqu'au premier `terminal.draw` par :

```rust
    let intervalle = reglages.refresh_interval;
    // Le client est construit une fois pour toutes : il porte le jeton dans
    // ses en-têtes, et c'est le seul endroit du programme où le jeton reste.
    let client = Arc::new(github::Client::new(jeton.expose())?);
    let mut etat = App::new(reglages);

    // La première requête part avant l'écran : un jeton refusé ou des droits
    // insuffisants sont des erreurs de démarrage, et leur message doit sortir
    // sur la sortie d'erreur, pas finir en ligne de barre d'état.
    let premiers = premiere_requete(&mut etat, &client).await?;

    let (envoi, mut reception) = mpsc::unbounded_channel::<Event>();

    // Le résultat déjà obtenu entre dans la file avant tout le reste : la
    // boucle le traitera à son premier tour.
    for evenement in premiers {
        let _ = envoi.send(evenement);
    }

    // L'écran est pris avant de lancer les producteurs : le clavier doit lire
    // un terminal en mode brut, jamais un terminal encore en mode ligne.
    let (mut terminal, _garde) = enter_terminal(envoi.clone())?;

    // Producteur 1 : le clavier, dans une tâche bloquante dédiée.
    spawn_keyboard(envoi.clone());

    // Producteur 2 : le minuteur de rafraîchissement, si activé.
    if intervalle > 0 {
        spawn_timer(envoi.clone(), intervalle);
    }

    terminal.draw(|cadre| ui::draw(cadre, &etat))?;
```

Le producteur 3 disparaît d'ici : les requêtes suivantes sont lancées par `execute_command` depuis la boucle, comme avant. Ajouter la fonction sous `run` :

```rust
/// Exécute la demande initiale de `app` et rend les événements à injecter
/// dans la boucle. Une erreur de démarrage remonte en `Err` : `main` l'écrit
/// et s'arrête, le terminal n'ayant jamais été pris.
async fn premiere_requete(
    etat: &mut App,
    client: &Arc<github::Client>,
) -> Result<Vec<Event>> {
    let mut evenements = Vec::new();
    for commande in etat.start() {
        match commande {
            Command::FetchList {
                generation,
                query,
                page_size,
            } => {
                let resultat = client.fetch_pull_requests(&query, page_size).await;
                match startup::classify(resultat) {
                    startup::FirstResponse::Fatal(message) => {
                        return Err(anyhow::anyhow!(message))
                    }
                    startup::FirstResponse::Start(result) => {
                        evenements.push(Event::ListLoaded { generation, result })
                    }
                }
            }
            // `start` n'émet que la demande de liste. Toute autre commande
            // serait un changement de `app` non répercuté ici.
            autre => unreachable!("commande inattendue au démarrage : {autre:?}"),
        }
    }
    Ok(evenements)
}
```

`Command` doit être importé dans `main.rs` : il l'est déjà (`use crate::app::{App, Command, Event, Key};`).

- [ ] **Step 5 : Vérifier la compilation et les tests**

Run: `cargo build && cargo test`
Expected: PASS. Si `unreachable!` réclame `Debug` sur `Command`, vérifier que `Command` dérive `Debug` — c'est le cas dans `src/app/mod.rs`.

- [ ] **Step 6 : Vérifier le refus de démarrage sur le binaire**

Le module `github` ne laisse pas régler son point d'entrée depuis l'extérieur, donc ce chemin ne se teste pas de bout en bout sans ouvrir cette porte. Un jeton bidon suffit, et il sort avant toute prise du terminal — la commande rend donc la main d'elle-même :

```bash
cargo build
OWL_TOKEN=ghp_jeton_invalide ./target/debug/owl < /dev/null > /tmp/owl-sortie.txt 2> /tmp/owl-erreur.txt
echo "code de sortie : $?"
cat /tmp/owl-erreur.txt
wc -c /tmp/owl-sortie.txt
```

Attendu :
- code de sortie non nul ;
- la sortie d'erreur porte exactement le message de `GithubError::Unauthorized`, c'est-à-dire « Jeton refusé par GitHub. Lance gh auth login pour le renouveler. », les accents graves autour de la commande comprise ;
- la sortie standard est vide : aucune séquence d'échappement, l'écran alterné n'a jamais été pris.

Si la machine n'a pas de réseau, l'erreur sera `Réseau injoignable.` et le programme démarrera : dans ce cas, sauter cette étape et le noter dans le journal d'exécution. Ne pas lancer `cargo run` sans jeton invalide : la TUI prendrait le terminal et ne rendrait pas la main.

- [ ] **Step 7 : Passer les quatre commandes de vérification**

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check
```

- [ ] **Step 8 : Commit**

```bash
git add src/main.rs
git commit -m "Refuse de démarrer sur un jeton refusé ou des droits insuffisants"
```

---

### Task 3 : Tests des critères de réussite restants

**Files:**
- Modify: `src/app/mod.rs` (module `tests`)
- Modify: `src/config.rs` (module `tests`)
- Modify: `src/main.rs` (commentaire de `Command::OpenInBrowser`)

**Interfaces:**
- Consomme : `App::handle`, `App::status_line`, `app_garnie`, `page`, `pr`, `ConfigError::NoHomeDirectory`.
- Produit : rien de nouveau, uniquement des tests.

Les autres critères sont déjà couverts (voir « État des lieux »). Restent la panne réseau pendant l'usage, l'effacement de l'erreur au succès suivant, et le message du dossier personnel introuvable.

- [ ] **Step 1 : Écrire les tests de panne réseau**

À la fin du module `tests` de `src/app/mod.rs` :

```rust
    #[test]
    fn une_panne_reseau_laisse_la_liste_affichee() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Err(GithubError::Transport),
        });
        assert_eq!(app.prs.len(), 2, "la liste précédente reste visible");
        assert!(!app.should_quit, "une panne réseau n'arrête pas le programme");
        assert_eq!(app.error.as_deref(), Some("Réseau injoignable."));
        assert!(
            app.status_line(CONFORTABLE).contains("Réseau injoignable."),
            "l'erreur s'affiche dans la barre d'état"
        );
        assert!(
            app.last_refresh.is_some(),
            "l'heure du dernier succès reste, elle mesure l'ancienneté"
        );
    }

    #[test]
    fn le_prochain_succes_efface_l_erreur() {
        let mut app = app_garnie(vec![pr(1)]);
        let echec = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation: echec,
            result: Err(GithubError::Transport),
        });
        assert!(app.error.is_some());

        let succes = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation: succes,
            result: Ok(page(vec![pr(1)])),
        });
        assert!(app.error.is_none(), "erreur = {:?}", app.error);
    }
```

- [ ] **Step 2 : Écrire le test du message de dossier personnel introuvable**

Dans le module `tests` de `src/config.rs`. Provoquer un vrai dossier personnel introuvable n'est pas possible de façon portable — `directories` retombe sur la base de données des comptes quand `HOME` est absent —, donc seul le message est vérifié, comme le demande le critère « chaque erreur du tableau produit son message » :

```rust
    #[test]
    fn le_dossier_personnel_introuvable_a_son_message() {
        assert_eq!(
            ConfigError::NoHomeDirectory.to_string(),
            "Impossible de déterminer le dossier de configuration."
        );
    }
```

- [ ] **Step 3 : Lancer les tests**

Run: `cargo test`
Expected: PASS. Ces tests décrivent un comportement déjà en place ; s'ils échouent, corriger le code, pas le test.

- [ ] **Step 4 : Corriger le renvoi de commentaire du navigateur**

`src/main.rs` promet, dans `Command::OpenInBrowser`, que la remontée de l'échec d'ouverture du navigateur appartient à la spec 05. Cette spec ne le traite pas : le renvoi est faux. Remplacer le commentaire par :

```rust
        Command::OpenInBrowser { url } => {
            // Dans une tâche bloquante : lancer le navigateur peut prendre un
            // instant, et l'écran doit rester réactif pendant ce temps.
            // Un échec reste silencieux : aucune spec ne définit de message
            // pour ce cas.
```

- [ ] **Step 5 : Passer les quatre commandes de vérification**

```bash
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check
```

- [ ] **Step 6 : Commit**

```bash
git add src/app/mod.rs src/config.rs src/main.rs
git commit -m "Teste la panne réseau en cours d'usage et les messages de démarrage restants"
```

---

### Task 4 : Vérification visuelle dans le terminal

**Files:** aucun — vérification à l'œil, comme la spec le prévoit pour `ui`.

`owl` est une interface de terminal : `claude-in-chrome` ne s'y applique pas, et il n'y a pas de compte de test à connecter. La vérification se fait dans un vrai terminal, avec un jeton GitHub valide. **Ces étapes sont à faire par le propriétaire du projet**, ou par l'agent si la session dispose d'un terminal interactif et d'un jeton.

- [ ] **Step 1 : Voir l'annonce de suspension**

Le solde d'appels ne s'épuise pas à la demande. Pour voir le morceau de barre d'état, appliquer ce patch temporaire dans `src/app/mod.rs`, méthode `note_rate_limit` :

```rust
    fn note_rate_limit(&mut self) {
        // PATCH TEMPORAIRE DE VÉRIFICATION — À RETIRER
        self.suspended_until = Some(Local::now() + Duration::minutes(42));
        if let Some(limite) = &self.rate_limit {
```

Puis :

```bash
cargo run
```

À vérifier à l'écran :
- la barre d'état porte « limite d'appels atteinte, reprise à » suivi d'une heure au format `14 h 32` ;
- la liste des pull requests reste affichée normalement ;
- appuyer sur `r` ne change rien : pas de « chargement… », pas de nouvelle heure de mise à jour ;
- attendre un tour de minuteur : l'heure de mise à jour ne bouge pas ;
- réduire la largeur de la fenêtre : l'annonce reste, l'aide clavier disparaît la première.

Retirer le patch :

```bash
git checkout src/app/mod.rs
```

- [ ] **Step 2 : Voir un terminal rendu après une panique**

Appliquer ce patch temporaire dans `src/app/mod.rs`, au début de `fn refresh` :

```rust
    fn refresh(&mut self) -> Vec<Command> {
        // PATCH TEMPORAIRE DE VÉRIFICATION — À RETIRER
        panic!("panique de vérification");
```

Puis `cargo run`, laisser la liste s'afficher, appuyer sur `r`.

À vérifier :
- le programme s'arrête, la trace de panique est lisible dans le terminal normal ;
- l'invite de commande revient, le curseur est visible, les touches s'affichent quand on les tape (le mode brut est bien désarmé) ;
- `echo test` fonctionne sans avoir à taper `reset`.

Retirer le patch :

```bash
git checkout src/app/mod.rs
```

- [ ] **Step 3 : Voir le terminal trop étroit**

`cargo run`, puis réduire la largeur de la fenêtre jusqu'à une vingtaine de colonnes.

À vérifier : le message « Élargis le terminal… » remplace la liste, sans lignes tronquées au hasard ; élargir de nouveau ramène la liste.

- [ ] **Step 4 : Rien à commiter**

Vérifier qu'aucun patch temporaire n'est resté :

```bash
git status --short
```
Expected: rien à signaler.

---

## Ce que ce plan ne fait pas

- Les deux entrées existantes de `docs/suivi/DETTE.md` — fusion sur une PR disparue, filtres tous blancs — ne sont pas traitées : la seconde touche le garde-fou des filtres vides, voisin de la spec 05, mais elle relève de `02-filtres.md` et mérite sa propre décision.
- Le point d'entrée de `github::Client` reste privé : le refus de démarrage se vérifie à la main plutôt qu'en ouvrant une variable d'environnement d'endpoint que la spec ne demande pas.
