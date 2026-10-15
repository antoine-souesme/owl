//! Serveurs qui écoutent un port TCP sur ce Mac. Ce module ne dépend d'aucun
//! autre module de `owl` : `parse` lit les réponses de `lsof` et `ps`,
//! `decide` en tire ce qu'on montre et ce qu'on coupe, `stop` enchaîne
//! l'arrêt, et `system` est la seule couche qui lance un programme ou envoie
//! un signal.

mod decide;
mod parse;
mod stop;
mod system;

// Utilisés par `cli` à la tâche 4.
#[allow(unused_imports)]
pub use decide::{servers, Scope, Server};
#[allow(unused_imports)]
pub use stop::{stop, Stopped};
#[allow(unused_imports)]
pub use system::Machine;

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use thiserror::Error;

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

#[derive(Debug, Error, PartialEq)]
pub enum PortsError {
    #[error("Process {pid} no longer listens on port {port}.")]
    NotListening { pid: u32, port: u16 },
    #[error("Permission denied: process {pid} belongs to {user}.")]
    Denied { pid: u32, user: String },
    /// Message du système, tel quel.
    #[error("{0}")]
    System(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Signal {
    /// Arrêt propre (`SIGTERM`).
    Terminate,
    /// Arrêt forcé (`SIGKILL`).
    Kill,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SignalError {
    /// Le processus n'existe plus.
    Gone,
    /// Le système refuse.
    Denied,
}

/// Ce que `owl` demande au système. `Machine` le fait pour de vrai ; les
/// tests en donnent un faux.
pub trait Host {
    fn snapshot(&self) -> Result<Snapshot, PortsError>;
    fn current_uid(&self) -> u32;
    fn now(&self) -> DateTime<Utc>;
    /// Le port est occupé, qu'on voie ou non qui l'occupe.
    fn port_taken(&self, port: u16) -> bool;
    fn signal(&self, pid: u32, signal: Signal) -> Result<(), SignalError>;
    fn alive(&self, pid: u32) -> bool;
    fn wait(&self, duration: Duration);
}

/// Réponse de `owl ports`. `hidden` : rien de visible n'écoute, mais le
/// port est pris par un processus que `lsof` ne montre pas.
#[derive(Debug, PartialEq)]
pub struct Listing {
    pub servers: Vec<Server>,
    pub hidden: bool,
}

/// `owl ports` : les serveurs node.
pub fn list(host: &impl Host) -> Result<Listing, PortsError> {
    let snapshot = host.snapshot()?;
    Ok(Listing {
        servers: servers(&snapshot, Scope::Node, host.current_uid(), host.now()),
        hidden: false,
    })
}

/// `owl ports <port>` : tout ce qui écoute ce port.
pub fn search(host: &impl Host, port: u16) -> Result<Listing, PortsError> {
    let snapshot = host.snapshot()?;
    let found = servers(&snapshot, Scope::Port(port), host.current_uid(), host.now());
    let hidden = found.is_empty() && host.port_taken(port);
    Ok(Listing {
        servers: found,
        hidden,
    })
}

#[cfg(test)]
pub(crate) fn recorded() -> Snapshot {
    snapshot_from(
        include_str!("../../tests/fixtures/ports/listening.txt"),
        include_str!("../../tests/fixtures/ports/directories.txt"),
        include_str!("../../tests/fixtures/ports/processes.txt"),
    )
}
