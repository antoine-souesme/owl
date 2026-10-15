//! Serveurs qui écoutent un port TCP sur ce Mac. Ce module ne dépend d'aucun
//! autre module de `owl` : `parse` lit les réponses de `lsof` et `ps`,
//! `decide` en tire ce qu'on montre et ce qu'on coupe, `stop` enchaîne
//! l'arrêt, et `system` est la seule couche qui lance un programme ou envoie
//! un signal.

mod decide;
mod parse;

#[allow(unused_imports)] // Utilisés par `cli` à la tâche 4.
pub use decide::{servers, Scope, Server};

use std::collections::HashMap;

/// Un processus qui écoute un port, tel que `lsof` le décrit. `name` est le
/// nom que le système donne au processus, pas la commande affichée par `ps`.
#[derive(Debug, Clone, PartialEq)]
pub struct Listener {
    pub pid: u32,
    pub name: String,
    pub uid: u32,
    pub user: String,
    pub port: u16,
}

/// Une ligne de `ps`. `elapsed` est en secondes.
#[derive(Debug, Clone, PartialEq)]
pub struct Process {
    pub ppid: u32,
    pub uid: u32,
    pub elapsed: u64,
    pub args: String,
}

/// Nom système et dossier courant d'un processus visible par `lsof`.
#[derive(Debug, Clone, PartialEq)]
pub struct Visible {
    pub name: String,
    pub directory: Option<String>,
}

/// Tout ce qu'une lecture du système apprend, à un instant donné.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub listeners: Vec<Listener>,
    pub visible: HashMap<u32, Visible>,
    pub processes: HashMap<u32, Process>,
}

/// Assemble une lecture à partir des trois textes bruts : ports en écoute,
/// dossiers courants, table des processus.
pub fn snapshot_from(listening: &str, directories: &str, table: &str) -> Snapshot {
    Snapshot {
        listeners: parse::listeners(listening),
        visible: parse::visible(directories),
        processes: parse::processes(table),
    }
}

#[cfg(test)]
pub(crate) fn recorded() -> Snapshot {
    snapshot_from(
        include_str!("../../tests/fixtures/ports/listening.txt"),
        include_str!("../../tests/fixtures/ports/directories.txt"),
        include_str!("../../tests/fixtures/ports/processes.txt"),
    )
}
