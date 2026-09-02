//! État de l'application et traitement des événements.
//!
//! Ne fait aucun appel réseau et ne touche pas au terminal : `handle` reçoit un
//! événement, met l'état à jour, et renvoie des commandes que `main` exécute.
//! Toutes les décisions d'affichage — quel message, quel nombre — sont prises
//! ici, jamais dans `ui`.

mod render;

pub use render::{ListRender, ListRow, MergeRender, Tone};

use render::tronquer;

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
const AIDE_LISTE: &str =
    "↑↓ naviguer · → détail · m fusionner · r rafraîchir · o navigateur · q quitter";
const AIDE_DETAIL: &str =
    "↑↓ défiler · ← liste · m fusionner · r rafraîchir · o navigateur · q quitter";
const AIDE_FUSION: &str = "↑↓ choisir · Entrée confirmer · Échap annuler";

/// Message affiché tant qu'aucune réponse n'est arrivée.
const ATTENTE_INITIALE: &str = "Chargement…";

const REFUS_BROUILLON: &str = "Pull request en brouillon, elle doit être publiée.";
const REFUS_CONFLITS: &str = "Conflits à résoudre.";
const REFUS_ETAT_INCONNU: &str = "État de fusion en cours de calcul, réessaie dans un instant.";
const REFUS_AUCUNE_METHODE: &str = "Aucune méthode de fusion autorisée sur ce dépôt.";

/// Attente imposée quand GitHub refuse pour limite d'appels sans donner
/// d'heure de reprise — le cas des limites secondaires sans `retry-after`.
/// Une minute suffit à casser la boucle de réessais, que la spec interdit.
const REPRISE_INCONNUE: i64 = 60;

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
                    Err(erreur) => self.note_error(erreur),
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
                    Err(erreur) => self.note_error(erreur),
                }
                self.borner_le_defilement();
                Vec::new()
            }
            Event::MergeFinished { key, result } => {
                // Réponse qui ne concerne pas la fenêtre ouverte : ignorée.
                if self.merge.as_ref().map(|fenetre| &fenetre.key) != Some(&key) {
                    return Vec::new();
                }
                match result {
                    Ok(()) => {
                        self.merge = None;
                        self.notice = Some(format!("{} #{} fusionnée", key.repo, key.number));
                        // La liste est redemandée tout de suite : la PR en
                        // disparaîtra, et la sélection suit la règle du
                        // rafraîchissement.
                        vec![self.fetch_list()]
                    }
                    // Message de GitHub tel quel : il dit quoi faire mieux
                    // qu'un message maison.
                    Err(erreur) => {
                        if let Some(fenetre) = self.merge.as_mut() {
                            fenetre.state = MergeDialogState::Failed(erreur.to_string());
                        }
                        Vec::new()
                    }
                }
            }
        }
    }

    fn handle_key(&mut self, touche: Key) -> Vec<Command> {
        // Ctrl-C avant tout, fenêtre ouverte comprise : le mode brut l'a
        // désarmée, et c'est à `owl` de l'honorer. Sans quoi la seule sortie
        // serait de tuer le terminal.
        if touche == Key::CtrlC {
            self.should_quit = true;
            return vec![Command::Quit];
        }

        // La fenêtre de fusion capte tout le reste du clavier.
        if self.merge.is_some() {
            return self.handle_key_merge(touche);
        }

        // Un message posé par `owl` ne survit pas à la touche suivante.
        self.notice = None;

        // Touches communes aux deux vues, traitées avant l'aiguillage.
        match touche {
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
            View::List => self.handle_key_list(touche),
            View::Detail { .. } => self.handle_key_detail(touche),
        }
    }

    /// Ce que la touche demande à la fenêtre.
    fn handle_key_merge(&mut self, touche: Key) -> Vec<Command> {
        /// Décision prise pendant l'emprunt de la fenêtre, appliquée après.
        enum Suite {
            Rien,
            Fermer,
            Confirmer,
        }

        let Some(fenetre) = self.merge.as_mut() else {
            return Vec::new();
        };

        let suite = match (&fenetre.state, touche) {
            (MergeDialogState::Choosing, Key::Up) => {
                // La sélection ne boucle pas, comme celle de la liste.
                fenetre.selected = fenetre.selected.saturating_sub(1);
                Suite::Rien
            }
            (MergeDialogState::Choosing, Key::Down) => {
                if fenetre.selected + 1 < fenetre.methods.len() {
                    fenetre.selected += 1;
                }
                Suite::Rien
            }
            (MergeDialogState::Choosing | MergeDialogState::Failed(_), Key::Esc) => Suite::Fermer,
            (MergeDialogState::Choosing | MergeDialogState::Failed(_), Key::Enter) => {
                Suite::Confirmer
            }
            // `Submitting` n'accepte rien : l'appel est parti, et fermer la
            // fenêtre laisserait croire que la fusion est annulée.
            _ => Suite::Rien,
        };

        match suite {
            Suite::Rien => Vec::new(),
            Suite::Fermer => {
                self.merge = None;
                Vec::new()
            }
            Suite::Confirmer => self.submit_merge(),
        }
    }

    /// Lance la fusion de la pull request de la fenêtre.
    ///
    /// La fenêtre ne se ferme pas : elle passe en `Submitting` et le dit.
    /// La fermer pendant l'appel donnerait l'impression que c'est fini.
    fn submit_merge(&mut self) -> Vec<Command> {
        let Some(fenetre) = self.merge.as_ref() else {
            return Vec::new();
        };
        let cle = fenetre.key.clone();
        let Some(methode) = fenetre.method() else {
            return Vec::new();
        };
        let Some(resume) = self.resume_affiche(&cle).cloned() else {
            return Vec::new();
        };
        // Le détail en cache porte l'identifiant GraphQL. Sans lui, `github`
        // fera la requête de détail avant la mutation.
        let node_id = self
            .details
            .get(&cle)
            .map(|cache| cache.detail.node_id.clone());

        if let Some(fenetre) = self.merge.as_mut() {
            fenetre.state = MergeDialogState::Submitting;
        }

        vec![Command::Merge {
            summary: resume,
            node_id,
            method: methode,
        }]
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
        // Suspension en cours : la touche est refusée. Aucun message n'est
        // posé ici, la barre d'état porte déjà l'annonce et son heure de
        // reprise — l'écrire deux fois sur la même ligne n'apprend rien.
        if self.suspension().is_some() {
            return Vec::new();
        }
        match &self.view {
            View::List => vec![self.fetch_list()],
            View::Detail { key, .. } => {
                let cle = key.clone();
                match self.resume_affiche(&cle).cloned() {
                    Some(resume) => vec![self.fetch_detail(resume)],
                    // Ni dans la liste, ni en cache : rien à recharger.
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

    /// Résumé de la pull request d'une clé donnée : celui de la liste, ou à
    /// défaut celui que porte le détail en cache.
    ///
    /// Le repli n'est pas un détail d'affichage : quand un rafraîchissement
    /// retire la PR alors que son détail reste ouvert, c'est la seule source
    /// du résumé, et `o` comme `r` en ont besoin autant que le dessin.
    pub(crate) fn resume_affiche(&self, key: &PrKey) -> Option<&PrSummary> {
        self.prs
            .iter()
            .find(|pr| &pr.key == key)
            .or_else(|| self.details.get(key).map(|cache| &cache.detail.summary))
    }

    /// Pull request visée par une action : la sélection dans la liste, la PR
    /// ouverte dans le détail.
    pub(crate) fn pr_affichee(&self) -> Option<&PrSummary> {
        match &self.view {
            View::List => self.selected_pr(),
            View::Detail { key, .. } => self.resume_affiche(key),
        }
    }

    /// URL de la pull request affichée : la sélection dans la liste, la PR
    /// ouverte dans le détail.
    fn open_in_browser(&self) -> Vec<Command> {
        match self.pr_affichee() {
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
        let Some(resume) = self.pr_affichee().cloned() else {
            return Vec::new();
        };
        let motif = if resume.is_draft {
            Some(REFUS_BROUILLON)
        } else {
            match resume.mergeable {
                MergeableState::Conflicting => Some(REFUS_CONFLITS),
                MergeableState::Unknown => Some(REFUS_ETAT_INCONNU),
                MergeableState::Mergeable => None,
            }
        };
        if let Some(message) = motif {
            self.notice = Some(message.to_string());
            return Vec::new();
        }

        let methodes = resume.repo_rules.allowed();
        if methodes.is_empty() {
            self.notice = Some(REFUS_AUCUNE_METHODE.to_string());
            return Vec::new();
        }

        // La méthode préférée si le dépôt l'autorise, sinon la première de la
        // liste — donc l'écrasement, puis le rebasage, puis le commit de fusion.
        let selected = methodes
            .iter()
            .position(|methode| *methode == self.config.preferred_merge_method)
            .unwrap_or(0);

        self.merge = Some(MergeDialog {
            key: resume.key.clone(),
            title: resume.title.clone(),
            methods: methodes,
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

    /// Défile de `pas` lignes, borné au contenu. La hauteur de la zone n'est
    /// pas connue ici : la dernière ligne reste atteignable, et le dessin ne
    /// peut de toute façon pas défiler au-delà.
    fn scroll_detail(&mut self, pas: i32) {
        let maximum = i64::from(self.derniere_ligne_du_detail());
        if let View::Detail { scroll, .. } = &mut self.view {
            let vise = i64::from(*scroll) + i64::from(pas);
            *scroll = vise.clamp(0, maximum) as u16;
        }
    }

    /// Indice de la dernière ligne du détail, saturé plutôt que tronqué : un
    /// détail de plus de 65 536 lignes ne doit pas ramener le défilement en
    /// haut de page.
    fn derniere_ligne_du_detail(&self) -> u16 {
        u16::try_from(self.detail_line_count().saturating_sub(1)).unwrap_or(u16::MAX)
    }

    /// Re-borne le défilement au contenu. Un détail rechargé peut être plus
    /// court que le précédent : sans cela, l'écran resterait vide jusqu'à ce
    /// que l'utilisateur remonte.
    fn borner_le_defilement(&mut self) {
        let maximum = self.derniere_ligne_du_detail();
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
        const ERREUR: u8 = 0;
        const CHARGEMENT: u8 = 1;
        const RESUME: u8 = 2;
        const HEURE: u8 = 3;
        const AIDE: u8 = 4;

        let mut morceaux: Vec<(u8, String)> = Vec::new();

        if self.last_refresh.is_none()
            && self.error.is_none()
            && self.notice.is_none()
            && self.suspension().is_none()
        {
            // Rien n'est encore arrivé : l'attente est le message principal.
            morceaux.push((ERREUR, ATTENTE_INITIALE.to_string()));
        } else {
            if self.last_refresh.is_some() {
                morceaux.push((RESUME, self.liste_resumee()));
            }
            if let Some(instant) = self.last_refresh {
                morceaux.push((HEURE, format!("mis à jour à {}", instant.format("%H:%M"))));
            }
            if self.loading.list || self.loading.detail {
                morceaux.push((CHARGEMENT, "chargement…".to_string()));
            }
            if let Some(erreur) = &self.error {
                morceaux.push((ERREUR, erreur.clone()));
            }
        }

        // Message de `owl` lui-même : motif de refus, ou fusion réussie. Au
        // même rang que l'erreur : il ne doit pas être sacrifié à la place.
        if let Some(message) = &self.notice {
            morceaux.push((ERREUR, message.clone()));
        }

        // Suspension pour limite d'appels : au même rang que l'erreur, elle
        // dit pourquoi la liste ne se rafraîchit plus.
        if let Some(reprise) = self.suspension() {
            morceaux.push((ERREUR, message_de_suspension(reprise)));
        }

        morceaux.push((
            AIDE,
            if self.merge.is_some() {
                AIDE_FUSION.to_string()
            } else {
                match self.view {
                    View::List => AIDE_LISTE,
                    View::Detail { .. } => AIDE_DETAIL,
                }
                .to_string()
            },
        ));

        let largeur = width as usize;
        while morceaux.len() > 1 && assembler(&morceaux).chars().count() > largeur {
            let sacrifie = morceaux
                .iter()
                .enumerate()
                .max_by_key(|(_, (rang, _))| *rang)
                .map(|(indice, _)| indice)
                .expect("la liste n'est pas vide");
            morceaux.remove(sacrifie);
        }
        tronquer(&assembler(&morceaux), largeur)
    }

    /// Résumé de la liste pour la barre d'état, jamais tronqué autrement que
    /// par le retrait d'un morceau entier.
    fn liste_resumee(&self) -> String {
        match self.prs.len() {
            0 => "Aucune pull request".to_string(),
            1 => "1 pull request".to_string(),
            nombre => format!("{nombre} pull requests"),
        }
    }
}

/// Assemble les morceaux retenus de la barre d'état, dans l'ordre d'affichage.
fn assembler(morceaux: &[(u8, String)]) -> String {
    morceaux
        .iter()
        .map(|(_, texte)| texte.as_str())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Annonce de suspension pour limite d'appels, avec son heure de reprise.
fn message_de_suspension(reprise: DateTime<Local>) -> String {
    format!(
        "limite d'appels atteinte, reprise à {}",
        reprise.format("%H h %M")
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
    const CONFORTABLE: u16 = 200;

    pub(crate) fn pr(numero: u32) -> PrSummary {
        PrSummary {
            key: PrKey {
                repo: "moi/depot".to_string(),
                number: numero,
            },
            title: format!("Titre {numero}"),
            author: "moi".to_string(),
            url: format!("https://github.com/moi/depot/pull/{numero}"),
            is_draft: false,
            checks: ChecksState::Success,
            review: ReviewState::Approved,
            mergeable: MergeableState::Mergeable,
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
    pub(crate) fn app_demarree() -> (App, Generation) {
        let mut app = App::new(Config::default());
        let commandes = app.start();
        let generation = match &commandes[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        (app, generation)
    }

    /// PR d'un dépôt donné, pour distinguer deux clés dans un même test.
    pub(crate) fn pr_de(depot: &str, numero: u32) -> PrSummary {
        PrSummary {
            key: PrKey {
                repo: depot.to_string(),
                number: numero,
            },
            ..pr(numero)
        }
    }

    /// Application démarrée et garnie de la liste donnée.
    pub(crate) fn app_garnie(liste: Vec<PrSummary>) -> App {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(liste)),
        });
        app
    }

    /// Rafraîchit et livre la nouvelle liste, en respectant la génération.
    pub(crate) fn rafraichir(app: &mut App, liste: Vec<PrSummary>) {
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(liste)),
        });
    }

    /// PR dont on choisit les règles du dépôt.
    pub(crate) fn pr_avec_regles(numero: u32, regles: RepoMergeRules) -> PrSummary {
        PrSummary {
            repo_rules: regles,
            ..pr(numero)
        }
    }

    /// Les trois méthodes autorisées.
    fn tout_autorise() -> RepoMergeRules {
        RepoMergeRules {
            squash: true,
            merge: true,
            rebase: true,
            delete_branch_on_merge: true,
        }
    }

    /// Application garnie d'une seule PR, `m` déjà pressée.
    fn app_fenetre_ouverte(pr: PrSummary) -> App {
        let mut app = app_garnie(vec![pr]);
        app.handle(Event::Key(Key::Char('m')));
        app
    }

    #[test]
    fn un_depot_qui_n_autorise_que_l_ecrasement_ne_propose_que_l_ecrasement() {
        let regles = RepoMergeRules {
            squash: true,
            merge: false,
            rebase: false,
            delete_branch_on_merge: true,
        };
        let app = app_fenetre_ouverte(pr_avec_regles(1, regles));
        let fenetre = app.merge.as_ref().expect("la fenêtre doit être ouverte");
        assert_eq!(fenetre.methods, vec![MergeMethod::Squash]);
    }

    #[test]
    fn un_depot_qui_autorise_tout_propose_tout_avec_la_methode_preferee() {
        // Construction en une fois : clippy refuse la réaffectation d'un
        // champ juste après `default()`.
        let reglages = Config {
            preferred_merge_method: MergeMethod::Rebase,
            ..Config::default()
        };
        let mut app = App::new(reglages);
        let generation = match &app.start()[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr_avec_regles(1, tout_autorise())])),
        });
        app.handle(Event::Key(Key::Char('m')));

        let fenetre = app.merge.as_ref().expect("la fenêtre doit être ouverte");
        assert_eq!(
            fenetre.methods,
            vec![MergeMethod::Squash, MergeMethod::Rebase, MergeMethod::Merge]
        );
        assert_eq!(fenetre.method(), Some(MergeMethod::Rebase));
    }

    #[test]
    fn une_methode_preferee_non_autorisee_retombe_sur_la_premiere() {
        // Réglages par défaut : la méthode préférée est l'écrasement.
        let regles = RepoMergeRules {
            squash: false,
            merge: true,
            rebase: true,
            delete_branch_on_merge: true,
        };
        let app = app_fenetre_ouverte(pr_avec_regles(1, regles));
        let fenetre = app.merge.as_ref().expect("la fenêtre doit être ouverte");
        assert_eq!(fenetre.method(), Some(MergeMethod::Rebase));
    }

    #[test]
    fn un_brouillon_n_ouvre_pas_la_fenetre_et_dit_pourquoi() {
        let brouillon = PrSummary {
            is_draft: true,
            ..pr(1)
        };
        let app = app_fenetre_ouverte(brouillon);
        assert!(app.merge.is_none());
        assert_eq!(
            app.notice.as_deref(),
            Some("Pull request en brouillon, elle doit être publiée.")
        );
    }

    #[test]
    fn un_conflit_n_ouvre_pas_la_fenetre_et_dit_pourquoi() {
        let en_conflit = PrSummary {
            mergeable: MergeableState::Conflicting,
            ..pr(1)
        };
        let app = app_fenetre_ouverte(en_conflit);
        assert!(app.merge.is_none());
        assert_eq!(app.notice.as_deref(), Some("Conflits à résoudre."));
    }

    #[test]
    fn un_etat_de_fusion_inconnu_demande_de_patienter() {
        let inconnu = PrSummary {
            mergeable: MergeableState::Unknown,
            ..pr(1)
        };
        let app = app_fenetre_ouverte(inconnu);
        assert!(app.merge.is_none());
        assert_eq!(
            app.notice.as_deref(),
            Some("État de fusion en cours de calcul, réessaie dans un instant.")
        );
    }

    #[test]
    fn un_depot_sans_methode_autorisee_n_ouvre_pas_la_fenetre() {
        let regles = RepoMergeRules {
            squash: false,
            merge: false,
            rebase: false,
            delete_branch_on_merge: false,
        };
        let app = app_fenetre_ouverte(pr_avec_regles(1, regles));
        assert!(app.merge.is_none());
        assert_eq!(
            app.notice.as_deref(),
            Some("Aucune méthode de fusion autorisée sur ce dépôt.")
        );
    }

    #[test]
    fn echap_ferme_la_fenetre_sans_aucun_appel() {
        let mut app = app_fenetre_ouverte(pr_avec_regles(1, tout_autorise()));
        let commandes = app.handle(Event::Key(Key::Esc));
        assert!(app.merge.is_none());
        assert!(commandes.is_empty(), "{commandes:?}");
    }

    #[test]
    fn les_fleches_changent_de_methode_sans_boucler() {
        let mut app = app_fenetre_ouverte(pr_avec_regles(1, tout_autorise()));
        // Départ sur l'écrasement, méthode préférée par défaut.
        app.handle(Event::Key(Key::Up));
        assert_eq!(methode_choisie(&app), MergeMethod::Squash);

        app.handle(Event::Key(Key::Down));
        assert_eq!(methode_choisie(&app), MergeMethod::Rebase);
        app.handle(Event::Key(Key::Down));
        assert_eq!(methode_choisie(&app), MergeMethod::Merge);
        app.handle(Event::Key(Key::Down));
        assert_eq!(methode_choisie(&app), MergeMethod::Merge);
    }

    /// Méthode sous le curseur de la fenêtre ouverte.
    fn methode_choisie(app: &App) -> MergeMethod {
        app.merge
            .as_ref()
            .expect("la fenêtre doit être ouverte")
            .method()
            .expect("une méthode doit être sélectionnée")
    }

    #[test]
    fn la_fenetre_capte_les_touches_de_l_application() {
        let mut app = app_fenetre_ouverte(pr_avec_regles(1, tout_autorise()));
        for touche in [Key::Char('q'), Key::Char('r'), Key::Char('o'), Key::Right] {
            let commandes = app.handle(Event::Key(touche));
            assert!(commandes.is_empty(), "{touche:?} a produit {commandes:?}");
        }
        assert!(!app.should_quit);
        assert!(app.merge.is_some());
        assert_eq!(app.view, View::List);
    }

    #[test]
    fn ctrl_c_quitte_meme_fenetre_ouverte() {
        let mut app = app_fenetre_ouverte(pr_avec_regles(1, tout_autorise()));
        let commandes = app.handle(Event::Key(Key::CtrlC));
        assert!(app.should_quit);
        assert_eq!(commandes, vec![Command::Quit]);
    }

    #[test]
    fn un_tick_ne_rafraichit_pas_tant_que_la_fenetre_est_ouverte() {
        let mut app = app_fenetre_ouverte(pr_avec_regles(1, tout_autorise()));
        let commandes = app.handle(Event::Tick);
        assert!(commandes.is_empty(), "{commandes:?}");
    }

    #[test]
    fn un_redimensionnement_ne_demande_rien_et_ne_change_rien() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        app.handle(Event::Key(Key::Down));
        let avant = app.selected;

        let commandes = app.handle(Event::Resize);

        assert!(commandes.is_empty(), "{commandes:?}");
        assert_eq!(app.selected, avant);
        assert_eq!(app.view, View::List);
    }

    #[test]
    fn un_redimensionnement_n_efface_pas_le_message_en_cours() {
        let brouillon = PrSummary {
            is_draft: true,
            ..pr(1)
        };
        let mut app = app_fenetre_ouverte(brouillon);
        assert!(app.notice.is_some());

        app.handle(Event::Resize);

        assert!(app.notice.is_some());
    }

    /// Détail minimal portant l'identifiant GraphQL demandé.
    fn detail_de(summary: PrSummary, node_id: &str) -> PrDetail {
        PrDetail {
            summary,
            node_id: node_id.to_string(),
            body: String::new(),
            head_ref: "branche".to_string(),
            base_ref: "develop".to_string(),
            checks: Vec::new(),
            reviews: Vec::new(),
            comments: Vec::new(),
            files: Vec::new(),
            additions: 0,
            deletions: 0,
        }
    }

    /// Ouvre la fenêtre sur la PR donnée, confirme, et rend la commande émise.
    fn confirmer(app: &mut App) -> Command {
        let mut commandes = app.handle(Event::Key(Key::Enter));
        assert_eq!(commandes.len(), 1, "{commandes:?}");
        commandes.remove(0)
    }

    #[test]
    fn confirmer_passe_la_fenetre_en_cours_et_demande_la_fusion() {
        let mut app = app_garnie(vec![pr_avec_regles(142, tout_autorise())]);
        app.handle(Event::Key(Key::Char('m')));
        let commande = confirmer(&mut app);

        match commande {
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
            autre => panic!("commande inattendue : {autre:?}"),
        }
        assert_eq!(
            app.merge.as_ref().map(|fenetre| &fenetre.state),
            Some(&MergeDialogState::Submitting)
        );
    }

    #[test]
    fn un_detail_en_cache_fournit_l_identifiant_graphql() {
        let resume = pr_avec_regles(142, tout_autorise());
        let mut app = app_garnie(vec![resume.clone()]);
        // Ouvre le détail, ce qui déclenche la requête, puis livre la réponse.
        let generation = match &app.handle(Event::Key(Key::Right))[0] {
            Command::FetchDetail { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::DetailLoaded {
            generation,
            key: resume.key.clone(),
            result: Ok(detail_de(resume.clone(), "PR_identifiant")),
        });

        app.handle(Event::Key(Key::Char('m')));
        match confirmer(&mut app) {
            Command::Merge { node_id, .. } => {
                assert_eq!(node_id.as_deref(), Some("PR_identifiant"));
            }
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn une_fusion_reussie_ferme_la_fenetre_et_rafraichit_la_liste() {
        let resume = pr_avec_regles(142, tout_autorise());
        let cle = resume.key.clone();
        let mut app = app_garnie(vec![resume]);
        app.handle(Event::Key(Key::Char('m')));
        confirmer(&mut app);

        let commandes = app.handle(Event::MergeFinished {
            key: cle,
            result: Ok(()),
        });

        assert!(app.merge.is_none());
        assert_eq!(app.notice.as_deref(), Some("moi/depot #142 fusionnée"));
        assert!(
            matches!(commandes.as_slice(), [Command::FetchList { .. }]),
            "{commandes:?}"
        );
    }

    #[test]
    fn une_fusion_echouee_laisse_la_fenetre_avec_le_message_de_github() {
        let resume = pr_avec_regles(142, tout_autorise());
        let cle = resume.key.clone();
        let mut app = app_garnie(vec![resume]);
        app.handle(Event::Key(Key::Char('m')));
        confirmer(&mut app);

        let commandes = app.handle(Event::MergeFinished {
            key: cle,
            result: Err(GithubError::Api("Base branch was modified.".to_string())),
        });

        assert!(commandes.is_empty(), "{commandes:?}");
        assert_eq!(
            app.merge.as_ref().map(|fenetre| fenetre.state.clone()),
            Some(MergeDialogState::Failed(
                "Base branch was modified.".to_string()
            ))
        );
        // La PR reste dans la liste.
        assert_eq!(app.prs.len(), 1);
    }

    #[test]
    fn un_merge_finished_d_une_autre_pr_est_ignore() {
        let resume = pr_avec_regles(142, tout_autorise());
        let autre = PrKey {
            repo: resume.key.repo.clone(),
            number: 7,
        };
        let mut app = app_garnie(vec![resume]);
        app.handle(Event::Key(Key::Char('m')));
        confirmer(&mut app);
        let fenetre_avant = app.merge.clone();

        let commandes = app.handle(Event::MergeFinished {
            key: autre,
            result: Ok(()),
        });

        assert!(commandes.is_empty(), "{commandes:?}");
        assert_eq!(
            app.merge, fenetre_avant,
            "la fenêtre reste inchangée : la réponse ne la concerne pas"
        );
    }

    #[test]
    fn entree_apres_un_echec_reessaie_avec_la_meme_methode() {
        let resume = pr_avec_regles(142, tout_autorise());
        let cle = resume.key.clone();
        let mut app = app_garnie(vec![resume]);
        app.handle(Event::Key(Key::Char('m')));
        // Descendre d'un cran : le rebasage.
        app.handle(Event::Key(Key::Down));
        confirmer(&mut app);
        app.handle(Event::MergeFinished {
            key: cle,
            result: Err(GithubError::Api("Base branch was modified.".to_string())),
        });

        match confirmer(&mut app) {
            Command::Merge { method, .. } => assert_eq!(method, MergeMethod::Rebase),
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn echap_apres_un_echec_ferme_la_fenetre() {
        let resume = pr_avec_regles(142, tout_autorise());
        let cle = resume.key.clone();
        let mut app = app_garnie(vec![resume]);
        app.handle(Event::Key(Key::Char('m')));
        confirmer(&mut app);
        app.handle(Event::MergeFinished {
            key: cle,
            result: Err(GithubError::Api("Base branch was modified.".to_string())),
        });

        let commandes = app.handle(Event::Key(Key::Esc));
        assert!(app.merge.is_none());
        assert!(commandes.is_empty(), "{commandes:?}");
    }

    #[test]
    fn aucune_touche_n_agit_pendant_l_appel() {
        let mut app = app_garnie(vec![pr_avec_regles(142, tout_autorise())]);
        app.handle(Event::Key(Key::Char('m')));
        confirmer(&mut app);

        for touche in [Key::Esc, Key::Enter, Key::Up, Key::Down, Key::Char('q')] {
            let commandes = app.handle(Event::Key(touche));
            assert!(commandes.is_empty(), "{touche:?} a produit {commandes:?}");
        }
        assert_eq!(
            app.merge.as_ref().map(|fenetre| &fenetre.state),
            Some(&MergeDialogState::Submitting)
        );
    }

    #[test]
    fn un_motif_de_refus_s_efface_a_la_touche_suivante() {
        let brouillon = PrSummary {
            is_draft: true,
            ..pr(1)
        };
        let mut app = app_fenetre_ouverte(brouillon);
        assert!(app.notice.is_some());
        app.handle(Event::Key(Key::Down));
        assert!(app.notice.is_none());
    }

    #[test]
    fn un_motif_de_refus_s_affiche_dans_la_barre_d_etat() {
        let brouillon = PrSummary {
            is_draft: true,
            ..pr(1)
        };
        let app = app_fenetre_ouverte(brouillon);
        assert!(
            app.status_line(CONFORTABLE)
                .contains("Pull request en brouillon, elle doit être publiée."),
            "{}",
            app.status_line(CONFORTABLE)
        );
    }

    #[test]
    fn m_sur_la_pr_ouverte_en_detail_vise_cette_pr() {
        let mut app = app_garnie(vec![pr_de("moi/a", 1), pr_de("moi/b", 2)]);
        app.handle(Event::Key(Key::Down));
        app.handle(Event::Key(Key::Right));
        app.handle(Event::Key(Key::Char('m')));
        let fenetre = app.merge.as_ref().expect("la fenêtre doit être ouverte");
        assert_eq!(fenetre.key.repo, "moi/b");
        assert_eq!(fenetre.key.number, 2);
    }

    #[test]
    fn le_demarrage_emet_une_seule_requete() {
        let mut app = App::new(Config::default());
        let commandes = app.start();
        assert_eq!(
            commandes,
            vec![Command::FetchList {
                generation: 1,
                query: "is:pr author:@me is:open sort:updated-desc".to_string(),
                page_size: 50,
            }]
        );
        assert!(app.loading.list);
    }

    #[test]
    fn q_demande_la_sortie() {
        let (mut app, _) = app_demarree();
        let commandes = app.handle(Event::Key(Key::Char('q')));
        assert_eq!(commandes, vec![Command::Quit]);
        assert!(app.should_quit);
    }

    #[test]
    fn r_relance_une_requete_avec_une_generation_plus_recente() {
        let (mut app, premiere) = app_demarree();
        let commandes = app.handle(Event::Key(Key::Char('r')));
        match &commandes[0] {
            Command::FetchList { generation, .. } => assert!(*generation > premiere),
            autre => panic!("commande inattendue : {autre:?}"),
        }
        assert!(app.loading.list);
    }

    #[test]
    fn le_minuteur_relance_une_requete() {
        let (mut app, premiere) = app_demarree();
        app.handle(Event::ListLoaded {
            generation: premiere,
            result: Ok(page(vec![])),
        });
        let commandes = app.handle(Event::Tick);
        match &commandes[0] {
            Command::FetchList { generation, .. } => assert!(*generation > premiere),
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn une_touche_inconnue_ne_fait_rien() {
        let (mut app, _) = app_demarree();
        let commandes = app.handle(Event::Key(Key::Other));
        assert!(commandes.is_empty());
        assert!(!app.should_quit);
    }

    #[test]
    fn un_resultat_a_jour_remplace_la_liste() {
        let (mut app, generation) = app_demarree();
        let commandes = app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1), pr(2)])),
        });
        assert!(commandes.is_empty());
        assert_eq!(app.prs, vec![pr(1), pr(2)]);
        assert!(!app.loading.list);
        assert!(app.last_refresh.is_some());
    }

    #[test]
    fn un_resultat_perime_est_ignore() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        // Une nouvelle requête part, puis la réponse lente de l'ancienne arrive.
        app.handle(Event::Key(Key::Char('r')));
        let commandes = app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(99)])),
        });
        assert!(commandes.is_empty());
        assert_eq!(
            app.prs,
            vec![pr(1)],
            "la réponse lente ne doit rien écraser"
        );
        assert!(app.loading.list, "la requête en cours reste en cours");
    }

    #[test]
    fn une_erreur_laisse_la_liste_affichee() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Err(GithubError::Transport),
        });
        assert_eq!(app.prs, vec![pr(1)], "la liste précédente reste visible");
        assert_eq!(app.error.as_deref(), Some("Réseau injoignable."));
        assert!(!app.loading.list);
    }

    #[test]
    fn un_succes_efface_l_erreur_en_cours() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Err(GithubError::Transport),
        });
        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        assert!(app.error.is_none(), "error = {:?}", app.error);
    }

    #[test]
    fn le_status_annonce_le_nombre_de_pull_requests() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1), pr(2)])),
        });
        assert!(
            app.status_line(CONFORTABLE).starts_with("2 pull requests"),
            "{}",
            app.status_line(CONFORTABLE)
        );

        let generation = match &app.handle(Event::Key(Key::Char('r')))[0] {
            Command::FetchList { generation, .. } => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![])),
        });
        assert!(
            app.status_line(CONFORTABLE)
                .starts_with("Aucune pull request"),
            "{}",
            app.status_line(CONFORTABLE)
        );
    }

    #[test]
    fn q_pendant_un_chargement_quitte_quand_meme() {
        let (mut app, _) = app_demarree();
        assert!(app.loading.list, "une requête est bien en cours");
        let commandes = app.handle(Event::Key(Key::Char('q')));
        assert_eq!(commandes, vec![Command::Quit]);
        assert!(app.should_quit);
    }

    #[test]
    fn un_evenement_quit_arrete_la_boucle() {
        let (mut app, _) = app_demarree();
        let commandes = app.handle(Event::Quit);
        assert_eq!(commandes, vec![Command::Quit]);
        assert!(app.should_quit);
    }

    #[test]
    fn la_barre_d_etat_au_demarrage_n_annonce_l_attente_qu_une_fois() {
        let (app, _) = app_demarree();
        assert_eq!(
            app.status_line(CONFORTABLE),
            format!("Chargement… · {AIDE_LISTE}")
        );
    }

    #[test]
    fn la_barre_d_etat_apres_une_reponse_donne_l_heure_et_l_aide() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1), pr(2)])),
        });
        let heure = app.last_refresh.unwrap().format("%H:%M").to_string();
        assert_eq!(
            app.status_line(CONFORTABLE),
            format!("2 pull requests · mis à jour à {heure} · {AIDE_LISTE}")
        );
    }

    #[test]
    fn la_barre_d_etat_annonce_le_chargement_d_un_rafraichissement() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Ok(page(vec![pr(1)])),
        });
        app.handle(Event::Key(Key::Char('r')));
        let heure = app.last_refresh.unwrap().format("%H:%M").to_string();
        assert_eq!(
            app.status_line(CONFORTABLE),
            format!("1 pull request · mis à jour à {heure} · chargement… · {AIDE_LISTE}")
        );
    }

    #[test]
    fn la_barre_d_etat_reprend_l_erreur_telle_quelle() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Err(GithubError::Transport),
        });
        assert_eq!(
            app.status_line(CONFORTABLE),
            format!("Réseau injoignable. · {AIDE_LISTE}"),
            "aucune heure : aucun rafraîchissement n'a encore réussi"
        );
    }

    #[test]
    fn la_barre_d_etat_ne_depasse_jamais_la_largeur_donnee() {
        let mut app = app_garnie(vec![pr(1), pr(2)]);
        // 80 colonnes : la largeur d'un terminal standard, où l'aide seule ne
        // tient déjà pas avec le résumé et l'heure.
        for largeur in [1, 12, 40, 80, 117, CONFORTABLE] {
            let barre = app.status_line(largeur);
            assert!(
                barre.chars().count() <= largeur as usize,
                "largeur {largeur} : {barre}"
            );
        }
        app.handle(Event::Key(Key::Right));
        for largeur in [1, 40, 80, CONFORTABLE] {
            let barre = app.status_line(largeur);
            assert!(
                barre.chars().count() <= largeur as usize,
                "en vue détail, largeur {largeur} : {barre}"
            );
        }
    }

    #[test]
    fn une_barre_d_etat_etroite_sacrifie_l_aide_avant_le_reste() {
        let app = app_garnie(vec![pr(1), pr(2)]);
        let heure = app.last_refresh.unwrap().format("%H:%M").to_string();

        assert_eq!(
            app.status_line(CONFORTABLE),
            format!("2 pull requests · mis à jour à {heure} · {AIDE_LISTE}"),
            "au large, tout tient"
        );

        let etroite = app.status_line(80);
        assert!(
            !etroite.contains("naviguer"),
            "l'aide est un rappel : elle part la première ({etroite})"
        );
        assert_eq!(
            etroite,
            format!("2 pull requests · mis à jour à {heure}"),
            "le résumé et l'heure restent entiers"
        );
    }

    #[test]
    fn une_barre_d_etat_tres_etroite_garde_l_erreur() {
        let (mut app, generation) = app_demarree();
        app.handle(Event::ListLoaded {
            generation,
            result: Err(GithubError::Transport),
        });
        assert_eq!(
            app.status_line(30),
            "Réseau injoignable.",
            "l'erreur est ce qu'on garde en dernier"
        );
    }

    #[test]
    fn l_aide_clavier_suit_la_vue_affichee() {
        let mut app = app_garnie(vec![pr(1)]);
        assert!(
            app.status_line(CONFORTABLE).ends_with(AIDE_LISTE),
            "{}",
            app.status_line(CONFORTABLE)
        );

        app.handle(Event::Key(Key::Right));
        let barre = app.status_line(CONFORTABLE);
        assert!(barre.ends_with(AIDE_DETAIL), "{barre}");
        assert!(
            barre.contains("← liste") && !barre.contains("→ détail"),
            "une touche sans effet dans la vue n'est pas rappelée : {barre}"
        );
    }

    #[test]
    fn l_aide_clavier_de_la_fenetre_de_fusion_remplace_celle_de_la_liste() {
        let mut app = app_garnie(vec![pr_avec_regles(142, tout_autorise())]);
        app.handle(Event::Key(Key::Char('m')));
        let barre = app.status_line(CONFORTABLE);

        assert!(barre.contains(AIDE_FUSION), "{barre}");
        assert!(!barre.contains("q quitter"), "{barre}");
    }

    #[test]
    fn les_filtres_des_reglages_sont_transmis_a_la_requete() {
        let reglages = Config {
            filters: vec!["review-requested:@me".to_string()],
            page_size: 7,
            ..Config::default()
        };
        let mut app = App::new(reglages);
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
    fn un_filtre_inconnu_des_reglages_traverse_la_requete_intact() {
        let reglages = Config {
            filters: vec!["involves:@me -is:draft".to_string()],
            ..Config::default()
        };
        let mut app = App::new(reglages);
        match &app.start()[0] {
            Command::FetchList { query, .. } => {
                assert_eq!(query, "is:pr involves:@me -is:draft sort:updated-desc")
            }
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn l_ordre_des_filtres_des_reglages_ne_change_pas_les_termes_ramenes() {
        let requete = |filtres: Vec<&str>| {
            let reglages = Config {
                filters: filtres.into_iter().map(str::to_string).collect(),
                ..Config::default()
            };
            match &App::new(reglages).start()[0] {
                Command::FetchList { query, .. } => query.clone(),
                autre => panic!("commande inattendue : {autre:?}"),
            }
        };

        let un = requete(vec!["author:@me", "is:open"]);
        let autre = requete(vec!["is:open", "author:@me"]);
        assert!(un.starts_with("is:pr "), "{un}");
        assert!(un.ends_with(" sort:updated-desc"), "{un}");

        let mut mots_un: Vec<&str> = un.split(' ').collect();
        let mut mots_autre: Vec<&str> = autre.split(' ').collect();
        mots_un.sort_unstable();
        mots_autre.sort_unstable();
        assert_eq!(mots_un, mots_autre);
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

        rafraichir(&mut app, vec![pr_de("moi/autre", 7), pr_de("moi/un", 7)]);
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
        assert_eq!(
            app.selected, 1,
            "l'indice précédent, borné à la nouvelle taille"
        );
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
            [Command::FetchDetail {
                generation: neuve, ..
            }] => {
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

    /// Détail ouvert et chargé, puis PR retirée de la liste — fusionnée,
    /// fermée, ou sortie du filtre — alors que la vue reste affichée.
    fn app_en_detail_hors_liste() -> App {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = ouvrir_detail(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        let generation_liste = match &app.handle(Event::Tick)[..] {
            [Command::FetchList { generation, .. }] => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
        };
        app.handle(Event::ListLoaded {
            generation: generation_liste,
            result: Ok(page(vec![])),
        });
        assert!(app.prs.is_empty(), "la PR a bien quitté la liste");
        app
    }

    #[test]
    fn o_ouvre_encore_une_pr_qui_a_quitte_la_liste() {
        let mut app = app_en_detail_hors_liste();
        assert_eq!(
            app.handle(Event::Key(Key::Char('o'))),
            vec![Command::OpenInBrowser {
                url: "https://github.com/moi/depot/pull/1".to_string()
            }],
            "l'URL est dans le résumé porté par le détail en cache"
        );
    }

    #[test]
    fn r_recharge_encore_une_pr_qui_a_quitte_la_liste() {
        let mut app = app_en_detail_hors_liste();
        match &app.handle(Event::Key(Key::Char('r')))[..] {
            [Command::FetchDetail { summary, .. }] => assert_eq!(summary.key, pr(1).key),
            autre => panic!("commande inattendue : {autre:?}"),
        }
    }

    #[test]
    fn un_detail_devenu_plus_court_reborne_le_defilement() {
        let mut app = app_garnie(vec![pr(1)]);
        let generation = ouvrir_detail(&mut app);
        app.handle(Event::DetailLoaded {
            generation,
            key: pr(1).key,
            result: Ok(detail(1)),
        });
        for _ in 0..500 {
            app.handle(Event::Key(Key::Down));
        }
        let bas = app.detail_scroll();
        assert!(bas > 0, "le défilement est bien descendu");

        // Rechargement : la description et les échanges ont disparu, le
        // contenu est nettement plus court que le défilement en cours.
        let generation = match &app.handle(Event::Key(Key::Char('r')))[..] {
            [Command::FetchDetail { generation, .. }] => *generation,
            autre => panic!("commande inattendue : {autre:?}"),
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

        assert!(app.detail_scroll() < bas, "le défilement a été ramené");
        assert!(
            (app.detail_scroll() as usize) < app.detail_line_count(),
            "sinon l'écran reste vide jusqu'à ce qu'on remonte : défilement \
             {} pour {} lignes",
            app.detail_scroll(),
            app.detail_line_count()
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
        assert!(
            ligne.contains("limite d'appels atteinte"),
            "ligne = {ligne}"
        );
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
}
