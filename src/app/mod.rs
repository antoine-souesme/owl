//! État de l'application et traitement des événements.
//!
//! Ne fait aucun appel réseau et ne touche pas au terminal : `handle` reçoit un
//! événement, met l'état à jour, et renvoie des commandes que `main` exécute.
//! Toutes les décisions d'affichage — quel message, quel nombre — sont prises
//! ici, jamais dans `ui`.

mod render;

pub use render::{
    ListRender, ListRow, MergeRender, Tone, DETAIL_TITLE, LIST_TITLE, SELECTION_MARKER,
};

use render::truncate;

use std::collections::HashMap;

use chrono::{DateTime, Duration, Local};

use crate::config::Config;
use crate::filter::{self, Filter};
use crate::github::GithubError;
use crate::model::{ListPage, MergeMethod, MergeableState, PrDetail, PrKey, PrSummary, RateLimit};

/// Numéro de génération d'une demande réseau. Un résultat dont la génération
/// est périmée est ignoré, ce qui évite qu'une réponse lente écrase une
/// réponse plus récente.
pub type Generation = u64;

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
// `DetailLoaded` porte un `PrDetail` complet, nettement plus gros que les
// autres variantes : la spec le nomme ainsi, et boîter cette variante pour
// satisfaire clippy compliquerait chaque appelant sans bénéfice réel.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Event {
    Key(Key),
    /// Tour de minuteur de rafraîchissement.
    Tick,
    /// Le terminal a changé de taille. Rien à décider : la boucle redessine
    /// après chaque événement, et c'est ce redessin qui remet l'écran à la
    /// bonne dimension.
    Resize,
    /// Arrêt demandé par `main` : panique d'une tâche, ou clavier hors service.
    Quit,
    /// Résultat d'une requête de liste.
    ListLoaded {
        generation: Generation,
        result: Result<ListPage, GithubError>,
    },
    /// Résultat d'une requête de détail.
    DetailLoaded {
        generation: Generation,
        key: PrKey,
        result: Result<PrDetail, GithubError>,
    },
    /// Résultat d'une fusion. Aucune génération : une seule fusion peut être
    /// en vol, la fenêtre bloquant le clavier pendant l'appel.
    MergeFinished {
        key: PrKey,
        result: Result<(), GithubError>,
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
    FetchDetail {
        generation: Generation,
        /// Le résumé entier, et pas la seule clé : `github::fetch_detail` le
        /// recopie dans le `PrDetail` qu'il rend.
        summary: PrSummary,
    },
    /// Ouvre une URL dans le navigateur. `app` choisit l'URL, `main` l'ouvre :
    /// `app` ne fait aucun effet de bord lui-même.
    OpenInBrowser {
        url: String,
    },
    /// Fusionne une pull request. `node_id` est l'identifiant GraphQL quand
    /// le détail est en cache ; sinon `github` le récupère lui-même avant
    /// d'envoyer la mutation.
    Merge {
        summary: PrSummary,
        node_id: Option<String>,
        method: MergeMethod,
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

/// Aide clavier, en fin de barre d'état. Le texte est ici, pas dans `ui`.
const HELP_LIST: &str = "↑↓ move · → details · m merge · r refresh · o browser · q quit";
const HELP_DETAIL: &str = "↑↓ scroll · ← list · m merge · r refresh · o browser · q quit";
const HELP_MERGE: &str = "↑↓ choose · Enter confirm · Esc cancel";

/// Message affiché tant qu'aucune réponse n'est arrivée.
const INITIAL_WAIT: &str = "Loading…";

const DENIED_DRAFT: &str = "This pull request is a draft, it must be published first.";
const DENIED_CONFLICTS: &str = "Conflicts to resolve.";
const DENIED_UNKNOWN_STATE: &str = "Merge state being computed, try again in a moment.";
const DENIED_NO_METHOD: &str = "No merge method allowed on this repository.";
const DENIED_GONE: &str = "Pull request not found.";

/// Attente imposée quand GitHub refuse pour limite d'appels sans donner
/// d'heure de reprise — le cas des limites secondaires sans `retry-after`.
/// Une minute suffit à casser la boucle de réessais, que la spec interdit.
const UNKNOWN_RESET: i64 = 60;

/// Vue affichée. Le défilement voyage avec la vue : revenir à la liste puis
/// rouvrir un détail le remet en haut, ce qui est le comportement attendu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    List,
    Detail { key: PrKey, scroll: u16 },
}

/// Où en est la fenêtre de confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeDialogState {
    Choosing,
    Submitting,
    /// Message d'erreur de GitHub, repris tel quel.
    Failed(String),
}

/// Fenêtre de confirmation de fusion. Tant qu'elle existe, elle capte le
/// clavier et suspend le rafraîchissement automatique.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeDialog {
    pub key: PrKey,
    pub title: String,
    /// Uniquement les méthodes autorisées par le dépôt, dans l'ordre
    /// écrasement, rebasage, commit de fusion.
    pub methods: Vec<MergeMethod>,
    pub selected: usize,
    pub state: MergeDialogState,
}

impl MergeDialog {
    /// Méthode sous le curseur. `None` seulement sur une liste vide, cas que
    /// les contrôles avant fusion écartent déjà.
    pub fn method(&self) -> Option<MergeMethod> {
        self.methods.get(self.selected).copied()
    }
}

/// Un détail en cache, avec l'heure de son chargement : le détail peut être
/// périmé sans qu'on le sache, autant dire quand il a été lu.
#[derive(Debug, Clone)]
pub struct CachedDetail {
    pub detail: PrDetail,
    pub loaded_at: DateTime<Local>,
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
    /// Solde d'appels rapporté par la dernière requête réussie.
    pub rate_limit: Option<RateLimit>,
    /// Heure de reprise quand la limite d'appels est atteinte. Tant qu'elle
    /// n'est pas passée, le rafraîchissement automatique est suspendu et `r`
    /// est refusée. Elle s'éteint d'elle-même : rien n'a à la remettre à zéro.
    suspended_until: Option<DateTime<Local>>,
    pub should_quit: bool,
    pub last_refresh: Option<DateTime<Local>>,
    pub view: View,
    /// Fenêtre de fusion ouverte, s'il y en a une.
    pub merge: Option<MergeDialog>,
    /// Message posé par `owl` lui-même : motif de refus de `m`, ou fusion
    /// réussie. Distinct de `error`, qui porte les messages de GitHub et se
    /// vide à la première réponse réussie — ce qui effacerait aussitôt
    /// l'annonce d'une fusion, puisqu'elle relance une requête de liste.
    pub notice: Option<String>,
    pub details: HashMap<PrKey, CachedDetail>,
    list_generation: Generation,
    detail_generation: Generation,
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
            suspended_until: None,
            should_quit: false,
            last_refresh: None,
            view: View::List,
            merge: None,
            notice: None,
            details: HashMap::new(),
            list_generation: 0,
            detail_generation: 0,
            filters: config
                .filters
                .iter()
                .map(|text| Filter::parse(text))
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
            Event::Key(key_pressed) => self.handle_key(key_pressed),
            Event::Quit => {
                self.should_quit = true;
                vec![Command::Quit]
            }
            // Le redessin suffit : l'état ne change pas avec la taille de
            // l'écran, et le message en cours n'est pas effacé — un
            // redimensionnement n'est pas un appui sur une touche.
            Event::Resize => Vec::new(),
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
                        self.note_rate_limit();
                    }
                    // Message de GitHub repris tel quel, et liste conservée.
                    Err(error) => self.note_error(error),
                }
                Vec::new()
            }
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
                    Err(error) => self.note_error(error),
                }
                self.clamp_scroll();
                Vec::new()
            }
            Event::MergeFinished { key, result } => {
                // Réponse qui ne concerne pas la fenêtre ouverte : ignorée.
                if self.merge.as_ref().map(|dialog| &dialog.key) != Some(&key) {
                    return Vec::new();
                }
                match result {
                    Ok(()) => {
                        self.merge = None;
                        self.notice = Some(format!("{} #{} merged", key.repo, key.number));
                        // La PR fusionnée quitte la liste immédiatement, sans
                        // attendre la réponse : l'index de recherche de GitHub
                        // met un instant à l'oublier, et la revoir après une
                        // fusion réussie ferait douter du résultat.
                        let rest: Vec<PrSummary> = self
                            .prs
                            .iter()
                            .filter(|pr| pr.key != key)
                            .cloned()
                            .collect();
                        self.apply_list(rest);
                        // La liste est tout de même redemandée : elle porte le
                        // solde d'appels et les mises à jour des autres PR.
                        vec![self.fetch_list()]
                    }
                    // Message de GitHub tel quel : il dit quoi faire mieux
                    // qu'un message maison.
                    Err(error) => {
                        if let Some(dialog) = self.merge.as_mut() {
                            dialog.state = MergeDialogState::Failed(error.to_string());
                        }
                        Vec::new()
                    }
                }
            }
        }
    }

    fn handle_key(&mut self, key_pressed: Key) -> Vec<Command> {
        // Ctrl-C avant tout, fenêtre ouverte comprise : le mode brut l'a
        // désarmée, et c'est à `owl` de l'honorer. Sans quoi la seule sortie
        // serait de tuer le terminal.
        if key_pressed == Key::CtrlC {
            self.should_quit = true;
            return vec![Command::Quit];
        }

        // La fenêtre de fusion capte tout le reste du clavier.
        if self.merge.is_some() {
            return self.handle_key_merge(key_pressed);
        }

        // Un message posé par `owl` ne survit pas à la touche suivante.
        self.notice = None;

        // Touches communes aux deux vues, traitées avant l'aiguillage.
        match key_pressed {
            Key::Char('q') => {
                self.should_quit = true;
                return vec![Command::Quit];
            }
            Key::Char('r') => return self.refresh(),
            Key::Char('o') => return self.open_in_browser(),
            Key::Char('m') => return self.open_merge(),
            _ => {}
        }

        match self.view {
            View::List => self.handle_key_list(key_pressed),
            View::Detail { .. } => self.handle_key_detail(key_pressed),
        }
    }

    /// Ce que la touche demande à la fenêtre.
    fn handle_key_merge(&mut self, key_pressed: Key) -> Vec<Command> {
        /// Décision prise pendant l'emprunt de la fenêtre, appliquée après.
        enum Outcome {
            Nothing,
            Close,
            Confirm,
        }

        let Some(dialog) = self.merge.as_mut() else {
            return Vec::new();
        };

        let outcome = match (&dialog.state, key_pressed) {
            (MergeDialogState::Choosing, Key::Up) => {
                // La sélection ne boucle pas, comme celle de la liste.
                dialog.selected = dialog.selected.saturating_sub(1);
                Outcome::Nothing
            }
            (MergeDialogState::Choosing, Key::Down) => {
                if dialog.selected + 1 < dialog.methods.len() {
                    dialog.selected += 1;
                }
                Outcome::Nothing
            }
            (MergeDialogState::Choosing | MergeDialogState::Failed(_), Key::Esc) => Outcome::Close,
            (MergeDialogState::Choosing | MergeDialogState::Failed(_), Key::Enter) => {
                Outcome::Confirm
            }
            // `Submitting` n'accepte rien : l'appel est parti, et fermer la
            // fenêtre laisserait croire que la fusion est annulée.
            _ => Outcome::Nothing,
        };

        match outcome {
            Outcome::Nothing => Vec::new(),
            Outcome::Close => {
                self.merge = None;
                Vec::new()
            }
            Outcome::Confirm => self.submit_merge(),
        }
    }

    /// Lance la fusion de la pull request de la fenêtre.
    ///
    /// La fenêtre ne se ferme pas : elle passe en `Submitting` et le dit.
    /// La fermer pendant l'appel donnerait l'impression que c'est fini.
    fn submit_merge(&mut self) -> Vec<Command> {
        let Some(dialog) = self.merge.as_ref() else {
            return Vec::new();
        };
        let key = dialog.key.clone();
        let Some(method) = dialog.method() else {
            return Vec::new();
        };
        let Some(summary) = self.displayed_summary(&key).cloned() else {
            // La pull request a disparu de la liste et du cache entre
            // l'ouverture de la fenêtre et la confirmation. Sans ce message,
            // `Entrée` semblerait ne rien faire.
            self.merge = None;
            self.notice = Some(DENIED_GONE.to_string());
            return Vec::new();
        };
        // Le détail en cache porte l'identifiant GraphQL. Sans lui, `github`
        // fera la requête de détail avant la mutation.
        let node_id = self
            .details
            .get(&key)
            .map(|cache| cache.detail.node_id.clone());

        if let Some(dialog) = self.merge.as_mut() {
            dialog.state = MergeDialogState::Submitting;
        }

        vec![Command::Merge {
            summary,
            node_id,
            method,
        }]
    }

    fn handle_key_list(&mut self, key_pressed: Key) -> Vec<Command> {
        match key_pressed {
            Key::Up | Key::Char('k') => self.select_previous(),
            Key::Down | Key::Char('j') => self.select_next(),
            Key::Right | Key::Enter => return self.open_detail(),
            _ => {}
        }
        Vec::new()
    }

    fn handle_key_detail(&mut self, key_pressed: Key) -> Vec<Command> {
        match key_pressed {
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
        // Suspension en cours : la touche est refusée. Aucun message n'est
        // posé ici, la barre d'état porte déjà l'annonce et son heure de
        // reprise — l'écrire deux fois sur la même ligne n'apprend rien.
        if self.suspension().is_some() {
            return Vec::new();
        }
        match &self.view {
            View::List => vec![self.fetch_list()],
            View::Detail { key, .. } => {
                let key = key.clone();
                match self.displayed_summary(&key).cloned() {
                    Some(summary) => vec![self.fetch_detail(summary)],
                    // Ni dans la liste, ni en cache : rien à recharger.
                    None => Vec::new(),
                }
            }
        }
    }

    /// Ouvre le détail de la sélection. Une PR déjà consultée pendant la
    /// session s'affiche depuis le cache, sans nouvelle requête.
    fn open_detail(&mut self) -> Vec<Command> {
        let Some(summary) = self.selected_pr().cloned() else {
            return Vec::new();
        };
        self.view = View::Detail {
            key: summary.key.clone(),
            scroll: 0,
        };
        if self.details.contains_key(&summary.key) {
            return Vec::new();
        }
        vec![self.fetch_detail(summary)]
    }

    /// Résumé de la pull request d'une clé donnée : celui de la liste, ou à
    /// défaut celui que porte le détail en cache.
    ///
    /// Le repli n'est pas un détail d'affichage : quand un rafraîchissement
    /// retire la PR alors que son détail reste ouvert, c'est la seule source
    /// du résumé, et `o` comme `r` en ont besoin autant que le dessin.
    pub(crate) fn displayed_summary(&self, key: &PrKey) -> Option<&PrSummary> {
        self.prs
            .iter()
            .find(|pr| &pr.key == key)
            .or_else(|| self.details.get(key).map(|cache| &cache.detail.summary))
    }

    /// Pull request visée par une action : la sélection dans la liste, la PR
    /// ouverte dans le détail.
    pub(crate) fn displayed_pr(&self) -> Option<&PrSummary> {
        match &self.view {
            View::List => self.selected_pr(),
            View::Detail { key, .. } => self.displayed_summary(key),
        }
    }

    /// URL de la pull request affichée : la sélection dans la liste, la PR
    /// ouverte dans le détail.
    fn open_in_browser(&self) -> Vec<Command> {
        match self.displayed_pr() {
            Some(pr) => vec![Command::OpenInBrowser {
                url: pr.url.clone(),
            }],
            None => Vec::new(),
        }
    }

    /// Ouvre la fenêtre de fusion, ou dit pourquoi elle ne s'ouvre pas.
    ///
    /// L'état des vérifications et des relectures n'est pas contrôlé ici :
    /// les protections de branche sont l'affaire de GitHub, qui les applique
    /// lui-même. Dupliquer cette logique produirait des désaccords.
    fn open_merge(&mut self) -> Vec<Command> {
        let Some(summary) = self.displayed_pr().cloned() else {
            return Vec::new();
        };
        let reason = if summary.is_draft {
            Some(DENIED_DRAFT)
        } else {
            match summary.mergeable {
                MergeableState::Conflicting => Some(DENIED_CONFLICTS),
                MergeableState::Unknown => Some(DENIED_UNKNOWN_STATE),
                MergeableState::Mergeable => None,
            }
        };
        if let Some(message) = reason {
            self.notice = Some(message.to_string());
            return Vec::new();
        }

        let methods = summary.repo_rules.allowed();
        if methods.is_empty() {
            self.notice = Some(DENIED_NO_METHOD.to_string());
            return Vec::new();
        }

        // La méthode préférée si le dépôt l'autorise, sinon la première de la
        // liste — donc l'écrasement, puis le rebasage, puis le commit de fusion.
        let selected = methods
            .iter()
            .position(|method| *method == self.config.preferred_merge_method)
            .unwrap_or(0);

        self.merge = Some(MergeDialog {
            key: summary.key.clone(),
            title: summary.title.clone(),
            methods,
            selected,
            state: MergeDialogState::Choosing,
        });
        Vec::new()
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

    /// Défile de `step` lignes, borné au contenu. La hauteur de la zone n'est
    /// pas connue ici : la dernière ligne reste atteignable, et le dessin ne
    /// peut de toute façon pas défiler au-delà.
    fn scroll_detail(&mut self, step: i32) {
        let maximum = i64::from(self.last_detail_line());
        if let View::Detail { scroll, .. } = &mut self.view {
            let target = i64::from(*scroll) + i64::from(step);
            *scroll = target.clamp(0, maximum) as u16;
        }
    }

    /// Indice de la dernière ligne du détail, saturé plutôt que tronqué : un
    /// détail de plus de 65 536 lignes ne doit pas ramener le défilement en
    /// haut de page.
    fn last_detail_line(&self) -> u16 {
        u16::try_from(self.detail_line_count().saturating_sub(1)).unwrap_or(u16::MAX)
    }

    /// Re-borne le défilement au contenu. Un détail rechargé peut être plus
    /// court que le précédent : sans cela, l'écran resterait vide jusqu'à ce
    /// que l'utilisateur remonte.
    fn clamp_scroll(&mut self) {
        let maximum = self.last_detail_line();
        if let View::Detail { scroll, .. } = &mut self.view {
            *scroll = (*scroll).min(maximum);
        }
    }

    /// Range une nouvelle liste en préservant la sélection.
    ///
    /// La clé retenue est cherchée dans la nouvelle liste. Si la PR a disparu,
    /// l'indice précédent est conservé, borné à la nouvelle taille : c'est ce
    /// qui garde le curseur au même endroit de l'écran plutôt que de le
    /// renvoyer en tête à chaque fusion.
    fn apply_list(&mut self, prs: Vec<PrSummary>) {
        let previous = self.selected;
        self.prs = prs;
        self.selected = match &self.selected_key {
            Some(key) => self
                .prs
                .iter()
                .position(|pr| &pr.key == key)
                .unwrap_or(previous),
            None => previous,
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

    /// Suspend le rafraîchissement quand le solde rapporté est épuisé.
    fn note_rate_limit(&mut self) {
        if let Some(limit) = &self.rate_limit {
            if limit.remaining == 0 {
                self.suspended_until = Some(limit.reset_at.with_timezone(&Local));
            }
        }
    }

    /// Retient une erreur de requête. Un refus pour limite d'appels ne laisse
    /// pas de message d'erreur : il suspend le rafraîchissement, et la barre
    /// d'état l'annonce avec l'heure de reprise.
    fn note_error(&mut self, error: GithubError) {
        match error {
            GithubError::RateLimited { reset_at } => {
                let resume_at = reset_at
                    .map(|time| time.with_timezone(&Local))
                    .unwrap_or_else(|| Local::now() + Duration::seconds(UNKNOWN_RESET));
                self.suspended_until = Some(resume_at);
            }
            other => self.error = Some(other.to_string()),
        }
    }

    /// Heure de reprise si la suspension court encore. Rend `None` dès que
    /// l'heure est passée : la suspension s'éteint sans que rien la lève.
    fn suspension(&self) -> Option<DateTime<Local>> {
        self.suspended_until.filter(|time| *time > Local::now())
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

    /// Barre d'état complète, prête à dessiner telle quelle, pour la largeur
    /// donnée.
    ///
    /// Assemblée ici, et pas dans `ui`, parce que chaque morceau est une
    /// décision : le libellé de l'heure, l'annonce d'une requête en cours,
    /// l'aide clavier. La largeur en est une aussi : à 80 colonnes, l'aide
    /// seule ne tient pas avec le reste, et il faut choisir plutôt que laisser
    /// la bibliothèque de dessin rogner la ligne en silence.
    ///
    /// Règle : chaque morceau porte son rang de sacrifice ; tant que la ligne
    /// dépasse, le rang le plus élevé est retiré. L'aide part la première —
    /// c'est un rappel, pas une information — puis l'heure, puis le résumé de
    /// la liste, puis l'annonce du chargement. L'erreur en cours reste, et
    /// n'est tronquée qu'en dernier recours, si la largeur ne tient même pas
    /// un morceau.
    pub fn status_line(&self, width: u16) -> String {
        // Rangs de sacrifice, du plus retenu au premier lâché.
        const ERROR: u8 = 0;
        const LOADING: u8 = 1;
        const SUMMARY: u8 = 2;
        const TIME: u8 = 3;
        const HELP: u8 = 4;

        let mut parts: Vec<(u8, String)> = Vec::new();

        if self.last_refresh.is_none()
            && self.error.is_none()
            && self.notice.is_none()
            && self.suspension().is_none()
        {
            // Rien n'est encore arrivé : l'attente est le message principal.
            parts.push((ERROR, INITIAL_WAIT.to_string()));
        } else {
            if self.last_refresh.is_some() {
                parts.push((SUMMARY, self.list_summary()));
            }
            if let Some(at) = self.last_refresh {
                parts.push((TIME, format!("updated at {}", at.format("%H:%M"))));
            }
            if self.loading.list || self.loading.detail {
                parts.push((LOADING, "loading…".to_string()));
            }
            if let Some(error) = &self.error {
                parts.push((ERROR, error.clone()));
            }
        }

        // Message de `owl` lui-même : motif de refus, ou fusion réussie. Au
        // même rang que l'erreur : il ne doit pas être sacrifié à la place.
        if let Some(message) = &self.notice {
            parts.push((ERROR, message.clone()));
        }

        // Suspension pour limite d'appels : au même rang que l'erreur, elle
        // dit pourquoi la liste ne se rafraîchit plus.
        if let Some(resume_at) = self.suspension() {
            parts.push((ERROR, suspension_message(resume_at)));
        }

        parts.push((
            HELP,
            if self.merge.is_some() {
                HELP_MERGE.to_string()
            } else {
                match self.view {
                    View::List => HELP_LIST,
                    View::Detail { .. } => HELP_DETAIL,
                }
                .to_string()
            },
        ));

        let width = width as usize;
        while parts.len() > 1 && assemble(&parts).chars().count() > width {
            let dropped = parts
                .iter()
                .enumerate()
                .max_by_key(|(_, (rank, _))| *rank)
                .map(|(index, _)| index)
                .expect("la liste n'est pas vide");
            parts.remove(dropped);
        }
        truncate(&assemble(&parts), width)
    }

    /// Résumé de la liste pour la barre d'état, jamais tronqué autrement que
    /// par le retrait d'un morceau entier.
    fn list_summary(&self) -> String {
        match self.prs.len() {
            0 => "No pull requests".to_string(),
            1 => "1 pull request".to_string(),
            count => format!("{count} pull requests"),
        }
    }
}

/// Assemble les morceaux retenus de la barre d'état, dans l'ordre d'affichage.
fn assemble(parts: &[(u8, String)]) -> String {
    parts
        .iter()
        .map(|(_, text)| text.as_str())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Annonce de suspension pour limite d'appels, avec son heure de reprise.
fn suspension_message(resume_at: DateTime<Local>) -> String {
    format!(
        "rate limit reached, resuming at {}",
        resume_at.format("%H:%M")
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    use crate::model::{
        ChangedFile, CheckRun, ChecksState, Comment, MergeMethod, MergeableState, RepoMergeRules,
        Review, ReviewState,
    };

    /// Largeur où la barre d'état tient tout entière, aide comprise.
    const ROOMY: u16 = 200;

    pub(crate) fn pr(number: u32) -> PrSummary {
        PrSummary {
            key: PrKey {
                repo: "moi/depot".to_string(),
                number: number,
            },
            title: format!("Titre {number}"),
            author: "moi".to_string(),
            url: format!("https://github.com/moi/depot/pull/{number}"),
            is_draft: false,
            checks: ChecksState::Success,
            review: ReviewState::Approved,
            mergeable: MergeableState::Mergeable,
            base_ref: "develop".to_string(),
            updated_at: "2026-08-30T09:12:44Z".parse().expect("date valide"),
            repo_rules: RepoMergeRules {
                squash: true,
                merge: false,
                rebase: true,
                delete_branch_on_merge: true,
            },
        }
    }

    /// Réponse de liste sans solde d'appels, suffisante partout où seul le
    /// contenu de la liste compte.
    pub(crate) fn page(pull_requests: Vec<PrSummary>) -> ListPage {
        ListPage {
            pull_requests,
            rate_limit: None,
        }
    }

    /// Application démarrée, première requête émise, génération courante rendue.
    pub(crate) fn app_started() -> (App, Generation) {
        let mut app = App::new(Config::default());
        let commands = app.start();
        let generation = match &commands[0] {
            Command::FetchList { generation, .. } => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        (app, generation)
    }

    /// PR d'un dépôt donné, pour distinguer deux clés dans un même test.
    pub(crate) fn pr_in(repo: &str, number: u32) -> PrSummary {
        PrSummary {
            key: PrKey {
                repo: repo.to_string(),
                number: number,
            },
            ..pr(number)
        }
    }

    /// Application démarrée et garnie de la liste donnée.
    pub(crate) fn app_with(list: Vec<PrSummary>) -> App {
        let (mut app, generation) = app_started();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(list)),
        });
        app
    }

    /// Rafraîchit et livre la nouvelle liste, en respectant la génération.
    pub(crate) fn refresh_with(app: &mut App, list: Vec<PrSummary>) {
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(list)),
        });
    }

    /// PR dont on choisit les règles du dépôt.
    pub(crate) fn pr_with_rules(number: u32, rules: RepoMergeRules) -> PrSummary {
        PrSummary {
            repo_rules: rules,
            ..pr(number)
        }
    }

    /// Les trois méthodes autorisées.
    fn all_allowed() -> RepoMergeRules {
        RepoMergeRules {
            squash: true,
            merge: true,
            rebase: true,
            delete_branch_on_merge: true,
        }
    }

    /// Application garnie d'une seule PR, `m` déjà pressée.
    fn app_with_dialog(pr: PrSummary) -> App {
        let mut app = app_with(vec![pr]);
        app.handle(Event::Key(Key::Char('m')));
        app
    }

    #[test]
    fn a_repo_allowing_only_squash_offers_only_squash() {
        let rules = RepoMergeRules {
            squash: true,
            merge: false,
            rebase: false,
            delete_branch_on_merge: true,
        };
        let app = app_with_dialog(pr_with_rules(1, rules));
        let dialog = app.merge.as_ref().expect("la fenêtre doit être ouverte");
        assert_eq!(dialog.methods, vec![MergeMethod::Squash]);
    }

    #[test]
    fn a_repo_allowing_everything_offers_everything_with_the_preferred_method() {
        // Construction en une fois : clippy refuse la réaffectation d'un
        // champ juste après `default()`.
        let settings = Config {
            preferred_merge_method: MergeMethod::Rebase,
            ..Config::default()
        };
        let mut app = App::new(settings);
        let generation = match &app.start()[0] {
            Command::FetchList { generation, .. } => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr_with_rules(1, all_allowed())])),
        });
        app.handle(Event::Key(Key::Char('m')));

        let dialog = app.merge.as_ref().expect("la fenêtre doit être ouverte");
        assert_eq!(
            dialog.methods,
            vec![MergeMethod::Squash, MergeMethod::Rebase, MergeMethod::Merge]
        );
        assert_eq!(dialog.method(), Some(MergeMethod::Rebase));
    }

    #[test]
    fn a_preferred_method_not_allowed_falls_back_to_the_first() {
        // Réglages par défaut : la méthode préférée est l'écrasement.
        let rules = RepoMergeRules {
            squash: false,
            merge: true,
            rebase: true,
            delete_branch_on_merge: true,
        };
        let app = app_with_dialog(pr_with_rules(1, rules));
        let dialog = app.merge.as_ref().expect("la fenêtre doit être ouverte");
        assert_eq!(dialog.method(), Some(MergeMethod::Rebase));
    }

    #[test]
    fn a_draft_does_not_open_the_dialog_and_says_why() {
        let draft = PrSummary {
            is_draft: true,
            ..pr(1)
        };
        let app = app_with_dialog(draft);
        assert!(app.merge.is_none());
        assert_eq!(
            app.notice.as_deref(),
            Some("This pull request is a draft, it must be published first.")
        );
    }

    #[test]
    fn a_conflict_does_not_open_the_dialog_and_says_why() {
        let conflicting = PrSummary {
            mergeable: MergeableState::Conflicting,
            ..pr(1)
        };
        let app = app_with_dialog(conflicting);
        assert!(app.merge.is_none());
        assert_eq!(app.notice.as_deref(), Some("Conflicts to resolve."));
    }

    #[test]
    fn an_unknown_merge_state_asks_to_wait() {
        let unknown = PrSummary {
            mergeable: MergeableState::Unknown,
            ..pr(1)
        };
        let app = app_with_dialog(unknown);
        assert!(app.merge.is_none());
        assert_eq!(
            app.notice.as_deref(),
            Some("Merge state being computed, try again in a moment.")
        );
    }

    #[test]
    fn a_repo_with_no_allowed_method_does_not_open_the_dialog() {
        let rules = RepoMergeRules {
            squash: false,
            merge: false,
            rebase: false,
            delete_branch_on_merge: false,
        };
        let app = app_with_dialog(pr_with_rules(1, rules));
        assert!(app.merge.is_none());
        assert_eq!(
            app.notice.as_deref(),
            Some("No merge method allowed on this repository.")
        );
    }

    #[test]
    fn esc_closes_the_dialog_without_any_call() {
        let mut app = app_with_dialog(pr_with_rules(1, all_allowed()));
        let commands = app.handle(Event::Key(Key::Esc));
        assert!(app.merge.is_none());
        assert!(commands.is_empty(), "{commands:?}");
    }

    #[test]
    fn the_arrows_change_method_without_wrapping() {
        let mut app = app_with_dialog(pr_with_rules(1, all_allowed()));
        // Départ sur l'écrasement, méthode préférée par défaut.
        app.handle(Event::Key(Key::Up));
        assert_eq!(chosen_method(&app), MergeMethod::Squash);

        app.handle(Event::Key(Key::Down));
        assert_eq!(chosen_method(&app), MergeMethod::Rebase);
        app.handle(Event::Key(Key::Down));
        assert_eq!(chosen_method(&app), MergeMethod::Merge);
        app.handle(Event::Key(Key::Down));
        assert_eq!(chosen_method(&app), MergeMethod::Merge);
    }

    /// Méthode sous le curseur de la fenêtre ouverte.
    fn chosen_method(app: &App) -> MergeMethod {
        app.merge
            .as_ref()
            .expect("la fenêtre doit être ouverte")
            .method()
            .expect("une méthode doit être sélectionnée")
    }

    #[test]
    fn the_dialog_captures_the_application_keys() {
        let mut app = app_with_dialog(pr_with_rules(1, all_allowed()));
        for key_pressed in [Key::Char('q'), Key::Char('r'), Key::Char('o'), Key::Right] {
            let commands = app.handle(Event::Key(key_pressed));
            assert!(
                commands.is_empty(),
                "{key_pressed:?} a produit {commands:?}"
            );
        }
        assert!(!app.should_quit);
        assert!(app.merge.is_some());
        assert_eq!(app.view, View::List);
    }

    #[test]
    fn ctrl_c_quits_even_with_the_dialog_open() {
        let mut app = app_with_dialog(pr_with_rules(1, all_allowed()));
        let commands = app.handle(Event::Key(Key::CtrlC));
        assert!(app.should_quit);
        assert_eq!(commands, vec![Command::Quit]);
    }

    #[test]
    fn a_tick_does_not_refresh_while_the_dialog_is_open() {
        let mut app = app_with_dialog(pr_with_rules(1, all_allowed()));
        let commands = app.handle(Event::Tick);
        assert!(commands.is_empty(), "{commands:?}");
    }

    #[test]
    fn a_resize_asks_for_nothing_and_changes_nothing() {
        let mut app = app_with(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Down));
        let avant = app.selected;

        let commands = app.handle(Event::Resize);

        assert!(commands.is_empty(), "{commands:?}");
        assert_eq!(app.selected, avant);
        assert_eq!(app.view, View::List);
    }

    #[test]
    fn a_resize_does_not_clear_the_current_message() {
        let draft = PrSummary {
            is_draft: true,
            ..pr(1)
        };
        let mut app = app_with_dialog(draft);
        assert!(app.notice.is_some());

        app.handle(Event::Resize);

        assert!(app.notice.is_some());
    }

    /// Détail minimal portant l'identifiant GraphQL demandé.
    fn detail_of(summary: PrSummary, node_id: &str) -> PrDetail {
        PrDetail {
            summary,
            node_id: node_id.to_string(),
            body: String::new(),
            head_ref: "branche".to_string(),
            checks: Vec::new(),
            reviews: Vec::new(),
            comments: Vec::new(),
            files: Vec::new(),
            additions: 0,
            deletions: 0,
        }
    }

    /// Ouvre la fenêtre sur la PR donnée, confirme, et rend la commande émise.
    fn confirm(app: &mut App) -> Command {
        let mut commands = app.handle(Event::Key(Key::Enter));
        assert_eq!(commands.len(), 1, "{commands:?}");
        commands.remove(0)
    }

    #[test]
    fn confirming_moves_the_dialog_to_submitting_and_asks_for_the_merge() {
        let mut app = app_with(vec![pr_with_rules(142, all_allowed())]);
        app.handle(Event::Key(Key::Char('m')));
        let command = confirm(&mut app);

        match command {
            Command::Merge {
                summary,
                node_id,
                method,
            } => {
                assert_eq!(summary.key.number, 142);
                // Le détail n'a jamais été ouvert : l'identifiant est inconnu.
                assert_eq!(node_id, None);
                assert_eq!(method, MergeMethod::Squash);
            }
            other => panic!("commande inattendue : {other:?}"),
        }
        assert_eq!(
            app.merge.as_ref().map(|dialog| &dialog.state),
            Some(&MergeDialogState::Submitting)
        );
    }

    #[test]
    fn a_cached_detail_provides_the_graphql_id() {
        let summary = pr_with_rules(142, all_allowed());
        let mut app = app_with(vec![summary.clone()]);
        // Ouvre le détail, ce qui déclenche la requête, puis livre la réponse.
        let generation = match &app.handle(Event::Key(Key::Right))[0] {
            Command::FetchDetail { generation, .. } => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        app.handle(Event::DetailLoaded {
            generation,
            key: summary.key.clone(),
            result: Ok(detail_of(summary.clone(), "PR_identifiant")),
        });

        app.handle(Event::Key(Key::Char('m')));
        match confirm(&mut app) {
            Command::Merge { node_id, .. } => {
                assert_eq!(node_id.as_deref(), Some("PR_identifiant"));
            }
            other => panic!("commande inattendue : {other:?}"),
        }
    }

    #[test]
    fn confirming_a_gone_pull_request_closes_the_dialog_with_a_message() {
        let mut app = app_with(vec![pr_with_rules(142, all_allowed())]);
        app.handle(Event::Key(Key::Char('m')));
        // Une réponse de liste déjà en vol au moment de l'ouverture retire la
        // pull request visée.
        app.handle(Event::ListLoaded {
            generation: app.list_generation,
            result: Ok(page(Vec::new())),
        });

        let commands = app.handle(Event::Key(Key::Enter));

        assert!(commands.is_empty(), "{commands:?}");
        assert!(app.merge.is_none());
        assert_eq!(app.notice.as_deref(), Some("Pull request not found."));
    }

    #[test]
    fn a_successful_merge_closes_the_dialog_and_refreshes_the_list() {
        let summary = pr_with_rules(142, all_allowed());
        let key = summary.key.clone();
        let mut app = app_with(vec![summary]);
        app.handle(Event::Key(Key::Char('m')));
        confirm(&mut app);

        let commands = app.handle(Event::MergeFinished {
            key: key,
            result: Ok(()),
        });

        assert!(app.merge.is_none());
        assert_eq!(app.notice.as_deref(), Some("moi/depot #142 merged"));
        assert!(
            matches!(commands.as_slice(), [Command::FetchList { .. }]),
            "{commands:?}"
        );
        assert!(
            app.prs.is_empty(),
            "la PR fusionnée quitte la liste sans attendre la réponse : {:?}",
            app.prs
        );
    }

    #[test]
    fn a_successful_merge_leaves_the_other_pull_requests_and_the_selection_in_place() {
        let mut app = app_with(vec![
            pr_with_rules(1, all_allowed()),
            pr_with_rules(2, all_allowed()),
            pr_with_rules(3, all_allowed()),
        ]);
        app.handle(Event::Key(Key::Down));
        app.handle(Event::Key(Key::Char('m')));
        confirm(&mut app);
        app.handle(Event::MergeFinished {
            key: pr(2).key,
            result: Ok(()),
        });

        assert_eq!(
            app.prs.iter().map(|pr| pr.key.number).collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(
            app.selected, 1,
            "la sélection reste à la même place à l'écran"
        );
    }

    #[test]
    fn a_failed_merge_leaves_the_dialog_with_the_github_message() {
        let summary = pr_with_rules(142, all_allowed());
        let key = summary.key.clone();
        let mut app = app_with(vec![summary]);
        app.handle(Event::Key(Key::Char('m')));
        confirm(&mut app);

        let commands = app.handle(Event::MergeFinished {
            key: key,
            result: Err(GithubError::Api("Base branch was modified.".to_string())),
        });

        assert!(commands.is_empty(), "{commands:?}");
        assert_eq!(
            app.merge.as_ref().map(|dialog| dialog.state.clone()),
            Some(MergeDialogState::Failed(
                "Base branch was modified.".to_string()
            ))
        );
        // La PR reste dans la liste.
        assert_eq!(app.prs.len(), 1);
    }

    #[test]
    fn a_merge_finished_for_another_pr_is_ignored() {
        let summary = pr_with_rules(142, all_allowed());
        let other = PrKey {
            repo: summary.key.repo.clone(),
            number: 7,
        };
        let mut app = app_with(vec![summary]);
        app.handle(Event::Key(Key::Char('m')));
        confirm(&mut app);
        let dialog_before = app.merge.clone();

        let commands = app.handle(Event::MergeFinished {
            key: other,
            result: Ok(()),
        });

        assert!(commands.is_empty(), "{commands:?}");
        assert_eq!(
            app.merge, dialog_before,
            "la fenêtre reste inchangée : la réponse ne la concerne pas"
        );
    }

    #[test]
    fn enter_after_a_failure_retries_with_the_same_method() {
        let summary = pr_with_rules(142, all_allowed());
        let key = summary.key.clone();
        let mut app = app_with(vec![summary]);
        app.handle(Event::Key(Key::Char('m')));
        // Descendre d'un cran : le rebasage.
        app.handle(Event::Key(Key::Down));
        confirm(&mut app);
        app.handle(Event::MergeFinished {
            key: key,
            result: Err(GithubError::Api("Base branch was modified.".to_string())),
        });

        match confirm(&mut app) {
            Command::Merge { method, .. } => assert_eq!(method, MergeMethod::Rebase),
            other => panic!("commande inattendue : {other:?}"),
        }
    }

    #[test]
    fn esc_after_a_failure_closes_the_dialog() {
        let summary = pr_with_rules(142, all_allowed());
        let key = summary.key.clone();
        let mut app = app_with(vec![summary]);
        app.handle(Event::Key(Key::Char('m')));
        confirm(&mut app);
        app.handle(Event::MergeFinished {
            key: key,
            result: Err(GithubError::Api("Base branch was modified.".to_string())),
        });

        let commands = app.handle(Event::Key(Key::Esc));
        assert!(app.merge.is_none());
        assert!(commands.is_empty(), "{commands:?}");
    }

    #[test]
    fn no_key_acts_while_the_call_is_in_flight() {
        let mut app = app_with(vec![pr_with_rules(142, all_allowed())]);
        app.handle(Event::Key(Key::Char('m')));
        confirm(&mut app);

        for key_pressed in [Key::Esc, Key::Enter, Key::Up, Key::Down, Key::Char('q')] {
            let commands = app.handle(Event::Key(key_pressed));
            assert!(
                commands.is_empty(),
                "{key_pressed:?} a produit {commands:?}"
            );
        }
        assert_eq!(
            app.merge.as_ref().map(|dialog| &dialog.state),
            Some(&MergeDialogState::Submitting)
        );
    }

    #[test]
    fn a_refusal_reason_clears_on_the_next_key() {
        let draft = PrSummary {
            is_draft: true,
            ..pr(1)
        };
        let mut app = app_with_dialog(draft);
        assert!(app.notice.is_some());
        app.handle(Event::Key(Key::Down));
        assert!(app.notice.is_none());
    }

    #[test]
    fn a_refusal_reason_shows_in_the_status_bar() {
        let draft = PrSummary {
            is_draft: true,
            ..pr(1)
        };
        let app = app_with_dialog(draft);
        assert!(
            app.status_line(ROOMY)
                .contains("This pull request is a draft, it must be published first."),
            "{}",
            app.status_line(ROOMY)
        );
    }

    #[test]
    fn m_in_the_detail_view_targets_that_pr() {
        let mut app = app_with(vec![pr_in("moi/a", 1), pr_in("moi/b", 2)]);
        app.handle(Event::Key(Key::Down));
        app.handle(Event::Key(Key::Right));
        app.handle(Event::Key(Key::Char('m')));
        let dialog = app.merge.as_ref().expect("la fenêtre doit être ouverte");
        assert_eq!(dialog.key.repo, "moi/b");
        assert_eq!(dialog.key.number, 2);
    }

    #[test]
    fn startup_emits_a_single_request() {
        let mut app = App::new(Config::default());
        let commands = app.start();
        assert_eq!(
            commands,
            vec![Command::FetchList {
                generation: 1,
                query: "is:pr author:@me is:open sort:updated-desc".to_string(),
                page_size: 50,
            }]
        );
        assert!(app.loading.list);
    }

    #[test]
    fn q_asks_to_quit() {
        let (mut app, _) = app_started();
        let commands = app.handle(Event::Key(Key::Char('q')));
        assert_eq!(commands, vec![Command::Quit]);
        assert!(app.should_quit);
    }

    #[test]
    fn r_starts_a_request_with_a_newer_generation() {
        let (mut app, first_one) = app_started();
        let commands = app.handle(Event::Key(Key::Char('r')));
        match &commands[0] {
            Command::FetchList { generation, .. } => assert!(*generation > first_one),
            other => panic!("commande inattendue : {other:?}"),
        }
        assert!(app.loading.list);
    }

    #[test]
    fn the_timer_starts_a_new_request() {
        let (mut app, first_one) = app_started();
        app.handle(Event::ListLoaded {
            generation: first_one,
            result: Ok(page(vec![])),
        });
        let commands = app.handle(Event::Tick);
        match &commands[0] {
            Command::FetchList { generation, .. } => assert!(*generation > first_one),
            other => panic!("commande inattendue : {other:?}"),
        }
    }

    #[test]
    fn an_unknown_key_does_nothing() {
        let (mut app, _) = app_started();
        let commands = app.handle(Event::Key(Key::Other));
        assert!(commands.is_empty());
        assert!(!app.should_quit);
    }

    #[test]
    fn an_up_to_date_result_replaces_the_list() {
        let (mut app, generation) = app_started();
        let commands = app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1), pr(2)])),
        });
        assert!(commands.is_empty());
        assert_eq!(app.prs, vec![pr(1), pr(2)]);
        assert!(!app.loading.list);
        assert!(app.last_refresh.is_some());
    }

    #[test]
    fn a_stale_result_is_ignored() {
        let (mut app, generation) = app_started();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        // Une nouvelle requête part, puis la réponse lente de l'ancienne arrive.
        app.handle(Event::Key(Key::Char('r')));
        let commands = app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(99)])),
        });
        assert!(commands.is_empty());
        assert_eq!(
            app.prs,
            vec![pr(1)],
            "la réponse lente ne doit rien écraser"
        );
        assert!(app.loading.list, "la requête en cours reste en cours");
    }

    #[test]
    fn an_error_leaves_the_list_on_screen() {
        let (mut app, generation) = app_started();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Err(GithubError::Transport),
        });
        assert_eq!(app.prs, vec![pr(1)], "la liste précédente reste visible");
        assert_eq!(app.error.as_deref(), Some("Network unreachable."));
        assert!(!app.loading.list);
    }

    #[test]
    fn a_success_clears_the_current_error() {
        let (mut app, generation) = app_started();
        app.handle(Event::ListLoaded {
            generation,
            result: Err(GithubError::Transport),
        });
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        assert!(app.error.is_none(), "error = {:?}", app.error);
    }

    #[test]
    fn the_status_bar_announces_the_number_of_pull_requests() {
        let (mut app, generation) = app_started();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1), pr(2)])),
        });
        assert!(
            app.status_line(ROOMY).starts_with("2 pull requests"),
            "{}",
            app.status_line(ROOMY)
        );

        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![])),
        });
        assert!(
            app.status_line(ROOMY).starts_with("No pull requests"),
            "{}",
            app.status_line(ROOMY)
        );
    }

    #[test]
    fn q_during_a_load_quits_anyway() {
        let (mut app, _) = app_started();
        assert!(app.loading.list, "une requête est bien en cours");
        let commands = app.handle(Event::Key(Key::Char('q')));
        assert_eq!(commands, vec![Command::Quit]);
        assert!(app.should_quit);
    }

    #[test]
    fn a_quit_event_stops_the_loop() {
        let (mut app, _) = app_started();
        let commands = app.handle(Event::Quit);
        assert_eq!(commands, vec![Command::Quit]);
        assert!(app.should_quit);
    }

    #[test]
    fn the_status_bar_at_startup_announces_the_wait_only_once() {
        let (app, _) = app_started();
        assert_eq!(app.status_line(ROOMY), format!("Loading… · {HELP_LIST}"));
    }

    #[test]
    fn the_status_bar_after_a_response_gives_the_time_and_the_help() {
        let (mut app, generation) = app_started();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1), pr(2)])),
        });
        let time = app.last_refresh.unwrap().format("%H:%M").to_string();
        assert_eq!(
            app.status_line(ROOMY),
            format!("2 pull requests · updated at {time} · {HELP_LIST}")
        );
    }

    #[test]
    fn the_status_bar_announces_a_refresh_in_progress() {
        let (mut app, generation) = app_started();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        app.handle(Event::Key(Key::Char('r')));
        let time = app.last_refresh.unwrap().format("%H:%M").to_string();
        assert_eq!(
            app.status_line(ROOMY),
            format!("1 pull request · updated at {time} · loading… · {HELP_LIST}")
        );
    }

    #[test]
    fn the_status_bar_repeats_the_error_verbatim() {
        let (mut app, generation) = app_started();
        app.handle(Event::ListLoaded {
            generation,
            result: Err(GithubError::Transport),
        });
        assert_eq!(
            app.status_line(ROOMY),
            format!("Network unreachable. · {HELP_LIST}"),
            "aucune heure : aucun rafraîchissement n'a encore réussi"
        );
    }

    #[test]
    fn the_status_bar_never_exceeds_the_given_width() {
        let mut app = app_with(vec![pr(1), pr(2)]);
        // 80 colonnes : la largeur d'un terminal standard, où l'aide seule ne
        // tient déjà pas avec le résumé et l'heure.
        for width in [1, 12, 40, 80, 117, ROOMY] {
            let bar = app.status_line(width);
            assert!(
                bar.chars().count() <= width as usize,
                "largeur {width} : {bar}"
            );
        }
        app.handle(Event::Key(Key::Right));
        for width in [1, 40, 80, ROOMY] {
            let bar = app.status_line(width);
            assert!(
                bar.chars().count() <= width as usize,
                "en vue détail, largeur {width} : {bar}"
            );
        }
    }

    #[test]
    fn a_narrow_status_bar_drops_the_help_before_the_rest() {
        let app = app_with(vec![pr(1), pr(2)]);
        let time = app.last_refresh.unwrap().format("%H:%M").to_string();

        assert_eq!(
            app.status_line(ROOMY),
            format!("2 pull requests · updated at {time} · {HELP_LIST}"),
            "au large, tout tient"
        );

        let narrow = app.status_line(80);
        assert!(
            !narrow.contains("move"),
            "l'aide est un rappel : elle part la première ({narrow})"
        );
        assert_eq!(
            narrow,
            format!("2 pull requests · updated at {time}"),
            "le résumé et l'heure restent entiers"
        );
    }

    #[test]
    fn a_very_narrow_status_bar_keeps_the_error() {
        let (mut app, generation) = app_started();
        app.handle(Event::ListLoaded {
            generation,
            result: Err(GithubError::Transport),
        });
        assert_eq!(
            app.status_line(30),
            "Network unreachable.",
            "l'erreur est ce qu'on garde en dernier"
        );
    }

    #[test]
    fn the_keyboard_help_follows_the_displayed_view() {
        let mut app = app_with(vec![pr(1)]);
        assert!(
            app.status_line(ROOMY).ends_with(HELP_LIST),
            "{}",
            app.status_line(ROOMY)
        );

        app.handle(Event::Key(Key::Right));
        let bar = app.status_line(ROOMY);
        assert!(bar.ends_with(HELP_DETAIL), "{bar}");
        assert!(
            bar.contains("← list") && !bar.contains("→ details"),
            "une touche sans effet dans la vue n'est pas rappelée : {bar}"
        );
    }

    #[test]
    fn the_merge_dialog_help_replaces_the_list_help() {
        let mut app = app_with(vec![pr_with_rules(142, all_allowed())]);
        app.handle(Event::Key(Key::Char('m')));
        let bar = app.status_line(ROOMY);

        assert!(bar.contains(HELP_MERGE), "{bar}");
        assert!(!bar.contains("q quitter"), "{bar}");
    }

    #[test]
    fn the_settings_filters_are_passed_to_the_query() {
        let settings = Config {
            filters: vec!["review-requested:@me".to_string()],
            page_size: 7,
            ..Config::default()
        };
        let mut app = App::new(settings);
        assert_eq!(
            app.start(),
            vec![Command::FetchList {
                generation: 1,
                query: "is:pr review-requested:@me sort:updated-desc".to_string(),
                page_size: 7,
            }]
        );
    }

    #[test]
    fn an_unknown_settings_filter_reaches_the_query_intact() {
        let settings = Config {
            filters: vec!["involves:@me -is:draft".to_string()],
            ..Config::default()
        };
        let mut app = App::new(settings);
        match &app.start()[0] {
            Command::FetchList { query, .. } => {
                assert_eq!(query, "is:pr involves:@me -is:draft sort:updated-desc")
            }
            other => panic!("commande inattendue : {other:?}"),
        }
    }

    #[test]
    fn the_order_of_the_settings_filters_does_not_change_the_terms_sent() {
        let query = |filters: Vec<&str>| {
            let settings = Config {
                filters: filters.into_iter().map(str::to_string).collect(),
                ..Config::default()
            };
            match &App::new(settings).start()[0] {
                Command::FetchList { query, .. } => query.clone(),
                other => panic!("commande inattendue : {other:?}"),
            }
        };

        let first = query(vec!["author:@me", "is:open"]);
        let second = query(vec!["is:open", "author:@me"]);
        assert!(first.starts_with("is:pr "), "{first}");
        assert!(first.ends_with(" sort:updated-desc"), "{first}");

        let mut words_first: Vec<&str> = first.split(' ').collect();
        let mut words_second: Vec<&str> = second.split(' ').collect();
        words_first.sort_unstable();
        words_second.sort_unstable();
        assert_eq!(words_first, words_second);
    }

    #[test]
    fn the_arrows_move_the_selection() {
        let mut app = app_with(vec![pr(1), pr(2), pr(3)]);
        assert_eq!(app.selected, 0);

        assert!(app.handle(Event::Key(Key::Down)).is_empty());
        assert_eq!(app.selected, 1);

        app.handle(Event::Key(Key::Down));
        assert_eq!(app.selected, 2);

        app.handle(Event::Key(Key::Up));
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn j_and_k_move_the_selection_like_the_arrows() {
        let mut app = app_with(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Char('j')));
        assert_eq!(app.selected, 1);
        app.handle(Event::Key(Key::Char('k')));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn the_selection_does_not_run_past_the_ends() {
        let mut app = app_with(vec![pr(1), pr(2)]);

        // En haut de liste, la flèche haut ne fait rien : pas de bouclage.
        app.handle(Event::Key(Key::Up));
        assert_eq!(app.selected, 0);

        app.handle(Event::Key(Key::Down));
        app.handle(Event::Key(Key::Down));
        assert_eq!(app.selected, 1, "la dernière ligne est un mur");
    }

    #[test]
    fn an_empty_list_has_no_selection() {
        let mut app = app_with(vec![]);
        assert!(app.selected_pr().is_none());
        app.handle(Event::Key(Key::Down));
        app.handle(Event::Key(Key::Up));
        assert_eq!(app.selected, 0, "aucune touche ne doit paniquer");
        assert!(app.selected_pr().is_none());
    }

    #[test]
    fn the_refresh_follows_the_selected_pr() {
        let mut app = app_with(vec![pr(1), pr(2), pr(3)]);
        app.handle(Event::Key(Key::Down));
        assert_eq!(app.selected_pr().map(|pr| pr.key.number), Some(2));

        // La 2 est passée en queue : la sélection la suit.
        refresh_with(&mut app, vec![pr(3), pr(1), pr(2)]);
        assert_eq!(app.selected, 2);
        assert_eq!(app.selected_pr().map(|pr| pr.key.number), Some(2));
    }

    #[test]
    fn two_repos_with_the_same_number_are_not_confused() {
        let mut app = app_with(vec![pr_in("moi/un", 7), pr_in("moi/autre", 7)]);
        app.handle(Event::Key(Key::Down));
        assert_eq!(
            app.selected_pr().map(|pr| pr.key.repo.clone()),
            Some("moi/autre".to_string())
        );

        refresh_with(&mut app, vec![pr_in("moi/autre", 7), pr_in("moi/un", 7)]);
        assert_eq!(
            app.selected_pr().map(|pr| pr.key.repo.clone()),
            Some("moi/autre".to_string()),
            "la clé porte le dépôt, pas seulement le numéro"
        );
    }

    #[test]
    fn a_gone_pr_leaves_the_selection_within_bounds() {
        let mut app = app_with(vec![pr(1), pr(2), pr(3)]);
        app.handle(Event::Key(Key::Down));
        app.handle(Event::Key(Key::Down));
        assert_eq!(app.selected, 2);

        // La 3 a été fusionnée : la liste rétrécit.
        refresh_with(&mut app, vec![pr(1), pr(2)]);
        assert_eq!(
            app.selected, 1,
            "l'indice précédent, borné à la nouvelle taille"
        );
        assert!(app.selected_pr().is_some());
    }

    #[test]
    fn a_list_gone_empty_has_no_selection_left() {
        let mut app = app_with(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Down));
        refresh_with(&mut app, vec![]);
        assert_eq!(app.selected, 0);
        assert!(app.selected_pr().is_none());
    }

    #[test]
    fn a_tick_during_a_list_load_starts_nothing() {
        let (mut app, _) = app_started();
        assert!(app.loading.list, "la requête de démarrage est en cours");
        assert!(
            app.handle(Event::Tick).is_empty(),
            "aucune seconde requête tant que la première n'a pas répondu"
        );
    }

    #[test]
    fn a_tick_after_the_response_reloads_the_list() {
        let mut app = app_with(vec![pr(1)]);
        assert!(!app.loading.list);
        match &app.handle(Event::Tick)[0] {
            Command::FetchList { .. } => {}
            other => panic!("commande inattendue : {other:?}"),
        }
    }

    #[test]
    fn ctrl_c_quits() {
        let (mut app, _) = app_started();
        assert_eq!(app.handle(Event::Key(Key::CtrlC)), vec![Command::Quit]);
        assert!(app.should_quit);
    }

    /// Détail d'une pull request, minimal mais complet dans sa forme.
    pub(crate) fn detail(number: u32) -> PrDetail {
        let summary = pr(number);
        PrDetail {
            node_id: format!("PR_{number}"),
            body: "Première ligne.\nSeconde ligne.".to_string(),
            head_ref: "ma-branche".to_string(),
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
            files: vec![
                ChangedFile {
                    path: "src/app/mod.rs".to_string(),
                    additions: 12,
                    deletions: 3,
                },
                // Chemin délibérément long : rend probante la troncature des
                // lignes de détail.
                ChangedFile {
                    path: "src/app/un/chemin/de/fichier/particulierement/long/pour/verifier/la/troncature.rs".to_string(),
                    additions: 1,
                    deletions: 0,
                },
            ],
            additions: 12,
            deletions: 3,
            summary: summary,
        }
    }

    /// Ouvre le détail de la sélection et rend la génération demandée.
    fn open_detail_of(app: &mut App) -> Generation {
        match &app.handle(Event::Key(Key::Right))[..] {
            [Command::FetchDetail { generation, .. }] => *generation,
            other => panic!("commande inattendue : {other:?}"),
        }
    }

    #[test]
    fn the_right_arrow_opens_the_detail_and_asks_for_the_data() {
        let mut app = app_with(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Down));
        let commands = app.handle(Event::Key(Key::Right));
        assert!(matches!(app.view, View::Detail { .. }));
        match &commands[..] {
            [Command::FetchDetail { summary, .. }] => assert_eq!(summary.key.number, 2),
            other => panic!("commande inattendue : {other:?}"),
        }
        assert!(app.loading.detail);
    }

    #[test]
    fn enter_also_opens_the_detail() {
        let mut app = app_with(vec![pr(1)]);
        app.handle(Event::Key(Key::Enter));
        assert!(matches!(app.view, View::Detail { .. }));
    }

    #[test]
    fn opening_a_pr_already_cached_emits_no_command() {
        let mut app = app_with(vec![pr(1)]);
        let generation = open_detail_of(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        app.handle(Event::Key(Key::Left));

        let commands = app.handle(Event::Key(Key::Right));
        assert!(
            commands.is_empty(),
            "le cache de la session évite la requête : {commands:?}"
        );
        assert!(!app.loading.detail);
    }

    #[test]
    fn the_left_arrow_and_esc_go_back_to_the_list() {
        let mut app = app_with(vec![pr(1)]);
        open_detail_of(&mut app);
        assert!(app.handle(Event::Key(Key::Left)).is_empty());
        assert!(matches!(app.view, View::List));

        open_detail_of(&mut app);
        app.handle(Event::Key(Key::Esc));
        assert!(matches!(app.view, View::List));
    }

    #[test]
    fn an_empty_list_opens_no_detail() {
        let mut app = app_with(vec![]);
        assert!(app.handle(Event::Key(Key::Right)).is_empty());
        assert!(matches!(app.view, View::List));
    }

    #[test]
    fn r_in_the_detail_view_reloads_the_detail_not_the_list() {
        let mut app = app_with(vec![pr(1)]);
        let generation = open_detail_of(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });

        match &app.handle(Event::Key(Key::Char('r')))[..] {
            [Command::FetchDetail {
                generation: neuve, ..
            }] => {
                assert!(*neuve > generation, "une nouvelle génération s'ouvre")
            }
            other => panic!("commande inattendue : {other:?}"),
        }
    }

    #[test]
    fn the_arrows_scroll_the_detail_without_running_past() {
        let mut app = app_with(vec![pr(1)]);
        let generation = open_detail_of(&mut app);
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
        let last = app.detail_scroll() as usize;
        assert!(last > 0);
        assert!(
            last < app.detail_lines(u16::MAX).len(),
            "défilement borné au contenu"
        );
    }

    #[test]
    fn a_stale_detail_is_ignored() {
        let mut app = app_with(vec![pr(1)]);
        let first_one = open_detail_of(&mut app);
        // Rechargement : la réponse lente de la première arrive après.
        app.handle(Event::Key(Key::Char('r')));
        app.handle(Event::DetailLoaded {
            generation: first_one,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        assert!(app.details.is_empty(), "la réponse périmée ne se range pas");
        assert!(app.loading.detail, "la requête en cours reste en cours");
    }

    #[test]
    fn opening_a_detail_does_not_stale_a_list_request_in_flight() {
        let mut app = app_with(vec![pr(1), pr(2)]);
        let list_generation_id = match &app.handle(Event::Tick)[..] {
            [Command::FetchList { generation, .. }] => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        open_detail_of(&mut app);

        app.handle(Event::ListLoaded {
            generation: list_generation_id,
            result: Ok(page(vec![pr(1), pr(2), pr(3)])),
        });
        assert_eq!(app.prs.len(), 3, "le résultat de liste doit être accepté");
        assert!(!app.loading.list);
    }

    #[test]
    fn a_list_refresh_does_not_clear_the_detail_cache() {
        let mut app = app_with(vec![pr(1)]);
        let generation = open_detail_of(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        app.handle(Event::Key(Key::Left));

        refresh_with(&mut app, vec![pr(1)]);
        assert!(
            app.details.contains_key(&pr(1).key),
            "le compromis est assumé : le détail reste en cache jusqu'à r"
        );
    }

    #[test]
    fn a_detail_error_is_repeated_verbatim() {
        let mut app = app_with(vec![pr(1)]);
        let generation = open_detail_of(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Err(GithubError::Transport),
        });
        assert_eq!(app.error.as_deref(), Some("Network unreachable."));
        assert!(!app.loading.detail);
    }

    #[test]
    fn o_opens_the_selected_pr_in_the_browser() {
        let mut app = app_with(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Down));
        assert_eq!(
            app.handle(Event::Key(Key::Char('o'))),
            vec![Command::OpenInBrowser {
                url: "https://github.com/moi/depot/pull/2".to_string()
            }]
        );
    }

    #[test]
    fn o_in_the_detail_view_opens_the_displayed_pr() {
        let mut app = app_with(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Down));
        open_detail_of(&mut app);
        assert_eq!(
            app.handle(Event::Key(Key::Char('o'))),
            vec![Command::OpenInBrowser {
                url: "https://github.com/moi/depot/pull/2".to_string()
            }]
        );
    }

    /// Détail ouvert et chargé, puis PR retirée de la liste — fusionnée,
    /// fermée, ou sortie du filtre — alors que la vue reste affichée.
    fn app_in_detail_off_list() -> App {
        let mut app = app_with(vec![pr(1)]);
        let generation = open_detail_of(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        let list_generation_id = match &app.handle(Event::Tick)[..] {
            [Command::FetchList { generation, .. }] => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        app.handle(Event::ListLoaded {
            generation: list_generation_id,
            result: Ok(page(vec![])),
        });
        assert!(app.prs.is_empty(), "la PR a bien quitté la liste");
        app
    }

    #[test]
    fn o_still_opens_a_pr_that_left_the_list() {
        let mut app = app_in_detail_off_list();
        assert_eq!(
            app.handle(Event::Key(Key::Char('o'))),
            vec![Command::OpenInBrowser {
                url: "https://github.com/moi/depot/pull/1".to_string()
            }],
            "l'URL est dans le résumé porté par le détail en cache"
        );
    }

    #[test]
    fn r_still_reloads_a_pr_that_left_the_list() {
        let mut app = app_in_detail_off_list();
        match &app.handle(Event::Key(Key::Char('r')))[..] {
            [Command::FetchDetail { summary, .. }] => assert_eq!(summary.key, pr(1).key),
            other => panic!("commande inattendue : {other:?}"),
        }
    }

    #[test]
    fn a_shorter_detail_clamps_the_scroll_again() {
        let mut app = app_with(vec![pr(1)]);
        let generation = open_detail_of(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        for _ in 0..500 {
            app.handle(Event::Key(Key::Down));
        }
        let bottom = app.detail_scroll();
        assert!(bottom > 0, "le défilement est bien descendu");

        // Rechargement : la description et les échanges ont disparu, le
        // contenu est nettement plus court que le défilement en cours.
        let generation = match &app.handle(Event::Key(Key::Char('r')))[..] {
            [Command::FetchDetail { generation, .. }] => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(PrDetail {
                body: String::new(),
                checks: Vec::new(),
                reviews: Vec::new(),
                comments: Vec::new(),
                files: Vec::new(),
                ..detail(1)
            }),
        });

        assert!(app.detail_scroll() < bottom, "le défilement a été ramené");
        assert!(
            (app.detail_scroll() as usize) < app.detail_line_count(),
            "sinon l'écran reste vide jusqu'à ce qu'on remonte : défilement \
             {} pour {} lines",
            app.detail_scroll(),
            app.detail_line_count()
        );
    }

    #[test]
    fn o_on_an_empty_list_does_nothing() {
        let mut app = app_with(vec![]);
        assert!(app.handle(Event::Key(Key::Char('o'))).is_empty());
    }

    #[test]
    fn m_is_recognised_and_stays_without_effect_until_spec_04() {
        let mut app = app_with(vec![pr(1)]);
        let commands = app.handle(Event::Key(Key::Char('m')));
        assert!(commands.is_empty(), "commandes = {commands:?}");
        assert!(matches!(app.view, View::List), "aucun changement de vue");
        assert!(app.error.is_none(), "et aucun message d'erreur");
    }

    /// Réponse de liste portant un solde d'appels, pour les tests de suspension.
    pub(crate) fn page_with_remaining(
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
    fn deliver(app: &mut App, generation: Generation, result: Result<ListPage, GithubError>) {
        app.handle(Event::ListLoaded {
            generation,
            result: result,
        });
    }

    #[test]
    fn an_exhausted_rate_limit_suspends_the_timer() {
        let (mut app, generation) = app_started();
        let resume_at = chrono::Utc::now() + chrono::Duration::minutes(30);
        deliver(
            &mut app,
            generation,
            Ok(page_with_remaining(vec![pr(1)], 0, resume_at)),
        );
        assert!(
            app.handle(Event::Tick).is_empty(),
            "le minuteur ne doit plus demander de liste"
        );
    }

    #[test]
    fn a_non_zero_rate_limit_suspends_nothing() {
        let (mut app, generation) = app_started();
        let resume_at = chrono::Utc::now() + chrono::Duration::minutes(30);
        deliver(
            &mut app,
            generation,
            Ok(page_with_remaining(vec![pr(1)], 12, resume_at)),
        );
        assert!(
            !app.handle(Event::Tick).is_empty(),
            "un solde restant ne doit rien suspendre"
        );
    }

    #[test]
    fn the_status_bar_announces_the_resume_time() {
        let (mut app, generation) = app_started();
        let resume_at = chrono::Utc::now() + chrono::Duration::minutes(30);
        deliver(
            &mut app,
            generation,
            Ok(page_with_remaining(vec![pr(1)], 0, resume_at)),
        );
        let expected = format!(
            "rate limit reached, resuming at {}",
            resume_at.with_timezone(&Local).format("%H:%M")
        );
        let line = app.status_line(ROOMY);
        assert!(line.contains(&expected), "ligne = {line}");
    }

    #[test]
    fn the_r_key_is_refused_while_suspended() {
        let (mut app, generation) = app_started();
        let resume_at = chrono::Utc::now() + chrono::Duration::minutes(30);
        deliver(
            &mut app,
            generation,
            Ok(page_with_remaining(vec![pr(1)], 0, resume_at)),
        );
        assert!(
            app.handle(Event::Key(Key::Char('r'))).is_empty(),
            "r doit être refusée pendant la suspension"
        );
        assert_eq!(app.prs.len(), 1, "la liste reste affichée");
    }

    #[test]
    fn a_passed_resume_time_hands_control_back_to_the_timer() {
        let (mut app, generation) = app_started();
        let resume_at = chrono::Utc::now() - chrono::Duration::minutes(1);
        deliver(
            &mut app,
            generation,
            Ok(page_with_remaining(vec![pr(1)], 0, resume_at)),
        );
        assert!(
            !app.handle(Event::Tick).is_empty(),
            "l'heure de reprise passée, le rafraîchissement repart"
        );
        let line = app.status_line(ROOMY);
        assert!(
            !line.contains("limite d'appels"),
            "l'annonce disparaît avec la suspension : {line}"
        );
    }

    #[test]
    fn a_rate_limit_refusal_suspends_instead_of_showing_the_error() {
        let mut app = app_with(vec![pr(1)]);
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        let resume_at = chrono::Utc::now() + chrono::Duration::minutes(15);
        deliver(
            &mut app,
            generation,
            Err(GithubError::RateLimited {
                reset_at: Some(resume_at),
            }),
        );
        assert!(app.error.is_none(), "erreur = {:?}", app.error);
        assert_eq!(app.prs.len(), 1, "la liste précédente reste visible");
        assert!(app.handle(Event::Tick).is_empty());
        let line = app.status_line(ROOMY);
        assert!(line.contains("rate limit reached"), "ligne = {line}");
    }

    #[test]
    fn a_rate_limit_refusal_without_a_time_suspends_anyway() {
        let mut app = app_with(vec![pr(1)]);
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        deliver(
            &mut app,
            generation,
            Err(GithubError::RateLimited { reset_at: None }),
        );
        assert!(
            app.handle(Event::Tick).is_empty(),
            "owl ne doit jamais réessayer en boucle une requête refusée pour limite"
        );
    }

    #[test]
    fn a_network_failure_leaves_the_list_on_screen() {
        let mut app = app_with(vec![pr(1), pr(2)]);
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Err(GithubError::Transport),
        });
        assert_eq!(app.prs.len(), 2, "la liste précédente reste visible");
        assert!(
            !app.should_quit,
            "une panne réseau n'arrête pas le programme"
        );
        assert_eq!(app.error.as_deref(), Some("Network unreachable."));
        assert!(
            app.status_line(ROOMY).contains("Network unreachable."),
            "l'erreur s'affiche dans la barre d'état"
        );
        assert!(
            app.last_refresh.is_some(),
            "l'heure du dernier succès reste, elle mesure l'ancienneté"
        );
    }

    #[test]
    fn the_next_success_clears_the_error() {
        let mut app = app_with(vec![pr(1)]);
        let failure = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        app.handle(Event::ListLoaded {
            generation: failure,
            result: Err(GithubError::Transport),
        });
        assert!(app.error.is_some());

        let success = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            other => panic!("commande inattendue : {other:?}"),
        };
        app.handle(Event::ListLoaded {
            generation: success,
            result: Ok(page(vec![pr(1)])),
        });
        assert!(app.error.is_none(), "erreur = {:?}", app.error);
    }

    #[test]
    fn a_rate_limit_refusal_on_the_detail_suspends_instead_of_showing_the_error() {
        let mut app = app_with(vec![pr(1)]);
        let generation = open_detail_of(&mut app);
        let resume_at = chrono::Utc::now() + chrono::Duration::minutes(15);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Err(GithubError::RateLimited {
                reset_at: Some(resume_at),
            }),
        });
        assert!(app.error.is_none(), "erreur = {:?}", app.error);
        assert!(app.handle(Event::Tick).is_empty());
    }
}
