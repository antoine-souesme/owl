# Serveurs node — plan de mise en œuvre

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ajouter au panneau de la barre des menus un onglet « Servers » qui liste les serveurs node en écoute et permet de les couper, plus une recherche par port avec avertissement pour les ports critiques.

**Architecture:** Un nouveau module Rust `ports` lit le système (`lsof`, `ps`), décide quoi montrer et quoi couper, et envoie les signaux. Deux nouvelles sous-commandes, `owl ports [<port>]` et `owl stop <pid> <port>`, l'exposent en JSON avec l'enveloppe existante. Côté Swift, `OwlBarCore` gagne un `ServersState` et un onglet dans `BarState` ; `OwlBar` dessine la barre d'onglets, la vue Servers et réutilise la fenêtre de fusion pour la confirmation.

**Tech Stack:** Rust 2021 (`serde`, `chrono`, `thiserror`, nouveau : `libc`), Swift 5.10 / SwiftUI macOS 14, Swift Testing.

**Spec:** `docs/superpowers/specs/2026-10-15-serveurs-node-design.md` — à lire en entier avant de commencer.

## Global Constraints

- Lire `CLAUDE.md` à la racine : règles d'architecture, conventions, commandes.
- Code, identifiants, noms de tests et tout texte affiché en anglais ; commentaires, documents et messages de commit en français.
- `cargo build`, `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` passent à la fin de chaque tâche Rust.
- Dès qu'un fichier de `macos/` ou de `src/cli/` change : `cd macos && swift build --build-system native` et `macos/test.sh` passent.
- `ports` ne dépend d'aucun autre module de `owl` ; `cli` dépend de `ports` ; `ports` ne dépend jamais de `app`, `ui`, `github`.
- `OwlBarCore` décide, `OwlBar` dessine : une vue ne choisit ni texte, ni couleur, ni format.
- `FORMAT_VERSION` reste à 1.
- Délai avant l'arrêt forcé : 5 secondes (50 pas de 100 ms).
- Famille node : `node`, `npm`, `npx`, `pnpm`, `yarn`, `bun`, `bunx`, `deno`, comparée au nom **système** (champ `c` de `lsof`), jamais à la commande affichée.
- Messages exacts : `Process <pid> no longer listens on port <port>.` ; `Permission denied: process <pid> belongs to <user>.` ; `No node server running` ; `Nothing listens on port <port>` ; `Port <port> is used by a process owl cannot see (system or another user).` ; `Not a port: <text>` ; titre de confirmation `Stop <command> on port <port>?` ; ligne `Critical: <reason>` ; boutons `Cancel` et `Stop` ; ligne en cours d'arrêt `Stopping…`.
- `lsof` et `ps` sont appelés par chemin absolu : `/usr/sbin/lsof`, `/bin/ps`.
- La CI tourne sous Ubuntu : aucun test ne lance `lsof`, `ps` ni n'envoie de signal.
- Travail sur la branche `feature/serveurs-node` (déjà créée, la spec y est commitée). Ne jamais ouvrir de PR vers `main`.

## Review Focus

- Un serveur Next.js se renomme `next-server (v15…)` : il doit quand même apparaître dans la liste node (test sur le relevé, tâche 2).
- Entre la liste et le clic sur Stop, le numéro de processus peut avoir été repris par un autre programme : `owl stop` ne doit rien couper et répondre `not_listening` (test tâche 3).
- La chaîne coupée ne doit jamais remonter au-delà d'un shell, d'un agent ou d'un processus d'un autre utilisateur (tests tâche 2).
- `lsof` sort en échec quand rien n'écoute : c'est une liste vide, pas une panne (test de `outcome`, tâche 3).
- Entrée dans le champ de recherche peut arriver deux fois (raccourci caché du panneau et `onSubmit`) : une seule lecture doit partir ; et une saisie hors port (`abc`, `0`, `65536`, ` 3000 `) doit être traitée proprement (tests tâche 6).

---

## Structure des fichiers

Rust :
- Create `src/ports/mod.rs` — types partagés (`Listener`, `Process`, `Visible`, `Snapshot`, `PortsError`, `Signal`, `SignalError`, trait `Host`, `Listing`), fonctions `snapshot_from`, `list`, `search`.
- Create `src/ports/parse.rs` — lecture pure des sorties de `lsof` et `ps`.
- Create `src/ports/decide.rs` — famille node, chaîne node, règle critique, commande raccourcie, construction des `Server`.
- Create `src/ports/stop.rs` — enchaînement de l'arrêt sur un `Host`.
- Create `src/ports/system.rs` — `Machine`, la seule couche qui lance un programme ou envoie un signal.
- Create `tests/fixtures/ports/listening.txt`, `directories.txt`, `processes.txt` — relevés.
- Modify `src/main.rs` — `mod ports;`.
- Modify `src/cli/mod.rs` — `Request::Ports`, `Request::Stop`, lecture des arguments, exécution, texte d'aide.
- Modify `src/cli/output.rs` — `PortsOut`, `StopOut`, `From<&PortsError>`, exemples pour l'app.
- Modify `Cargo.toml` — `libc = "0.2"`.

Swift :
- Modify `macos/Sources/OwlBarCore/Models.swift` — `ServerId`, `Critical`, `Server`, `PortsData`, `StopData`.
- Create `macos/Sources/OwlBarCore/Servers.swift` — présentation des serveurs, fenêtre d'arrêt, `ServersState`.
- Modify `macos/Sources/OwlBarCore/BarState.swift` — `PanelTab`, `tab`, `servers`, `selectTab`, `panelOpened`, aiguillage des touches.
- Modify `macos/Sources/OwlBar/PanelView.swift` — barre d'onglets, pied du panneau commun, `DialogView` générique, raccourcis.
- Modify `macos/Sources/OwlBar/ListView.swift` — le pied du panneau part dans `PanelView`.
- Create `macos/Sources/OwlBar/ServersView.swift` — onglet Servers.
- Create `macos/Tests/OwlBarCoreTests/ServersTests.swift`.
- Modify `macos/Tests/OwlBarCoreTests/DecodingTests.swift`, `BarStateTests.swift`.
- Generated `macos/Tests/OwlBarCoreTests/Fixtures/ports.json`, `stop.json`.

Docs : `DESCRIPTION.md`, `CLAUDE.md`.

---

### Task 1: Relevés et lecture des sorties de `lsof` et `ps`

**Files:**
- Create: `tests/fixtures/ports/listening.txt`, `tests/fixtures/ports/directories.txt`, `tests/fixtures/ports/processes.txt`
- Create: `src/ports/mod.rs`, `src/ports/parse.rs`
- Modify: `src/main.rs` (liste des `mod`)

**Interfaces:**
- Produces (dans `crate::ports`) :
  - `pub struct Listener { pub pid: u32, pub name: String, pub uid: u32, pub user: String, pub port: u16 }`
  - `pub struct Process { pub ppid: u32, pub uid: u32, pub elapsed: u64, pub args: String }`
  - `pub struct Visible { pub name: String, pub directory: Option<String> }`
  - `pub struct Snapshot { pub listeners: Vec<Listener>, pub visible: HashMap<u32, Visible>, pub processes: HashMap<u32, Process> }`
  - `pub fn snapshot_from(listening: &str, directories: &str, table: &str) -> Snapshot`
  - `parse::listeners(&str) -> Vec<Listener>`, `parse::visible(&str) -> HashMap<u32, Visible>`, `parse::processes(&str) -> HashMap<u32, Process>`, `parse::elapsed(&str) -> Option<u64>`

- [ ] **Step 1: Écrire les relevés**

Relevés réels du 15 octobre 2026 sur ce Mac (un `npm run dev` de Next.js dont l'agent a été coupé, d'où son parent `1`), réduits, et complétés de deux blocs au même format : un Vite lancé par un agent via `zsh`, et un Postgres de Homebrew. Les espaces en fin de ligne de `processes.txt` sont réels : les garder.

`tests/fixtures/ports/listening.txt` (sortie de `lsof -nP -iTCP -sTCP:LISTEN -F pcuLn`) :

```
p953
crapportd
u501
Lantoinesouesme
f12
n*:59595
f14
n*:59595
p1055
cControlCenter
u501
Lantoinesouesme
f10
n*:7000
f11
n*:7000
f12
n*:5000
f13
n*:5000
p2210
cpostgres
u501
Lantoinesouesme
f7
n127.0.0.1:5432
f8
n[::1]:5432
p35860
cnode
u501
Lantoinesouesme
f13
n*:3000
p41022
cnode
u501
Lantoinesouesme
f24
n127.0.0.1:5173
f25
n[::1]:5173
```

`tests/fixtures/ports/directories.txt` (sortie de `lsof -nP -d cwd -F pcn`) :

```
p953
crapportd
fcwd
n/
p1055
cControlCenter
fcwd
n/
p2210
cpostgres
fcwd
n/opt/homebrew/var/postgresql@16
p35815
cnode
fcwd
n/Users/antoinesouesme/dev/projets/gkfa/erp-menage
p35834
cnode
fcwd
n/Users/antoinesouesme/dev/projets/gkfa/erp-menage
p35860
cnode
fcwd
n/Users/antoinesouesme/dev/projets/gkfa/erp-menage
p41000
czsh
fcwd
n/Users/antoinesouesme/dev/playground/owl
p41010
cnode
fcwd
n/Users/antoinesouesme/dev/playground/owl/site
p41022
cnode
fcwd
n/Users/antoinesouesme/dev/playground/owl/site
```

`tests/fixtures/ports/processes.txt` (sortie de `ps -A -o pid=,ppid=,uid=,etime=,args=`) :

```
    1     0     0 14-02:44:24 /sbin/launchd
  953     1   501 14-02:40:02 /usr/libexec/rapportd
 1055     1   501 14-02:39:58 /System/Library/CoreServices/ControlCenter.app/Contents/MacOS/ControlCenter
 2210     1   501 3-04:12:09 /opt/homebrew/opt/postgresql@16/bin/postgres -D /opt/homebrew/var/postgresql@16
35815     1   501    15:28:13 npm run dev   
35834 35815   501    15:28:13 node /Users/antoinesouesme/dev/projets/gkfa/erp-menage/node_modules/.bin/next dev --port 3000
35860 35834   501    15:28:13 next-server (v15.5.18) 
40990 40001   501    01:02:03 claude
41000 40990   501       12:40 /bin/zsh -c npm run dev
41010 41000   501       12:39 npm run dev
41022 41010   501       12:37 node /Users/antoinesouesme/dev/playground/owl/site/node_modules/.bin/vite
```

- [ ] **Step 2: Déclarer le module**

Dans `src/main.rs`, après `mod notify;` :

```rust
// Branché sur `cli` à la tâche 4 ; d'ici là, rien ne l'appelle hors des tests.
#[allow(dead_code)]
mod ports;
```

`src/ports/mod.rs` :

```rust
//! Serveurs qui écoutent un port TCP sur ce Mac. Ce module ne dépend d'aucun
//! autre module de `owl` : `parse` lit les réponses de `lsof` et `ps`,
//! `decide` en tire ce qu'on montre et ce qu'on coupe, `stop` enchaîne
//! l'arrêt, et `system` est la seule couche qui lance un programme ou envoie
//! un signal.

mod parse;

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
```

- [ ] **Step 3: Écrire les tests qui échouent**

`src/ports/parse.rs`, d'abord avec les seuls tests et des fonctions vides :

```rust
//! Lecture des réponses de `lsof` et `ps`. Fonctions pures : le texte entre,
//! des valeurs sortent ; une ligne illisible est ignorée.

use std::collections::HashMap;

use super::{Listener, Process, Visible};

pub fn listeners(_text: &str) -> Vec<Listener> {
    Vec::new()
}

pub fn visible(_text: &str) -> HashMap<u32, Visible> {
    HashMap::new()
}

pub fn processes(_text: &str) -> HashMap<u32, Process> {
    HashMap::new()
}

pub fn elapsed(_text: &str) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTENING: &str = include_str!("../../tests/fixtures/ports/listening.txt");
    const DIRECTORIES: &str = include_str!("../../tests/fixtures/ports/directories.txt");
    const TABLE: &str = include_str!("../../tests/fixtures/ports/processes.txt");

    #[test]
    fn one_listener_per_process_and_port() {
        let found: Vec<(u32, u16)> = listeners(LISTENING)
            .iter()
            .map(|l| (l.pid, l.port))
            .collect();
        assert_eq!(
            found,
            vec![
                (953, 59595),
                (1055, 7000),
                (1055, 5000),
                (2210, 5432),
                (35860, 3000),
                (41022, 5173),
            ]
        );
    }

    #[test]
    fn a_listener_carries_its_system_name_and_owner() {
        let next = listeners(LISTENING)
            .into_iter()
            .find(|l| l.pid == 35860)
            .unwrap();
        assert_eq!(
            next,
            Listener {
                pid: 35860,
                name: "node".into(),
                uid: 501,
                user: "antoinesouesme".into(),
                port: 3000,
            }
        );
    }

    #[test]
    fn a_missing_login_falls_back_to_the_uid() {
        let found = listeners("p12\ncnode\nu502\nf3\nn*:8080\n");
        assert_eq!(found[0].user, "502");
    }

    #[test]
    fn an_address_without_a_port_is_ignored() {
        assert!(listeners("p12\ncnode\nu501\nLmoi\nf3\nn*:*\n").is_empty());
    }

    #[test]
    fn every_visible_process_has_its_name_and_directory() {
        let found = visible(DIRECTORIES);
        assert_eq!(found.len(), 9);
        assert_eq!(
            found[&41000],
            Visible {
                name: "zsh".into(),
                directory: Some("/Users/antoinesouesme/dev/playground/owl".into()),
            }
        );
        assert_eq!(found[&35815].name, "node");
    }

    #[test]
    fn the_process_table_keeps_parents_owners_and_commands() {
        let found = processes(TABLE);
        assert_eq!(found.len(), 11);
        assert_eq!(
            found[&35834],
            Process {
                ppid: 35815,
                uid: 501,
                elapsed: 55693,
                args: "node /Users/antoinesouesme/dev/projets/gkfa/erp-menage/node_modules/.bin/next dev --port 3000".into(),
            }
        );
        assert_eq!(found[&35815].args, "npm run dev");
        assert_eq!(found[&1].uid, 0);
    }

    #[test]
    fn an_unreadable_table_line_is_skipped() {
        assert!(processes("PID PPID UID ELAPSED COMMAND\n\n").is_empty());
    }

    #[test]
    fn elapsed_time_reads_every_form_of_ps() {
        assert_eq!(elapsed("00:07"), Some(7));
        assert_eq!(elapsed("12:37"), Some(757));
        assert_eq!(elapsed("15:28:13"), Some(55693));
        assert_eq!(elapsed("3-04:12:09"), Some(274329));
        assert_eq!(elapsed("14-02:44:24"), Some(1_219_464));
        assert_eq!(elapsed("abc"), None);
        assert_eq!(elapsed("1:2:3:4"), None);
    }
}
```

Ajouter `mod parse;` est déjà fait dans `mod.rs`.

- [ ] **Step 4: Vérifier qu'ils échouent**

Run: `cargo test ports::parse`
Expected: FAIL — les vecteurs et tables sont vides, `elapsed` rend `None`.

- [ ] **Step 5: Écrire la lecture**

Remplacer les quatre fonctions vides de `src/ports/parse.rs` par :

```rust
/// `lsof -nP -iTCP -sTCP:LISTEN -F pcuLn` : un champ par ligne, la première
/// lettre dit lequel. `p` ouvre un processus ; `n` donne une adresse dont le
/// port suit le dernier deux-points. Un même port vu en IPv4 et en IPv6 ne
/// compte qu'une fois.
pub fn listeners(text: &str) -> Vec<Listener> {
    let mut found: Vec<Listener> = Vec::new();
    let mut pid: Option<u32> = None;
    let mut name = String::new();
    let mut uid = 0;
    let mut user = String::new();
    for line in text.lines() {
        let Some(tag) = line.chars().next() else {
            continue;
        };
        let value = &line[tag.len_utf8()..];
        match tag {
            'p' => {
                pid = value.parse().ok();
                name.clear();
                uid = 0;
                user.clear();
            }
            'c' => name = value.to_string(),
            'u' => uid = value.parse().unwrap_or(0),
            'L' => user = value.to_string(),
            'n' => {
                let (Some(pid), Some(port)) = (pid, port_of(value)) else {
                    continue;
                };
                if !found.iter().any(|l| l.pid == pid && l.port == port) {
                    found.push(Listener {
                        pid,
                        name: name.clone(),
                        uid,
                        user: if user.is_empty() {
                            uid.to_string()
                        } else {
                            user.clone()
                        },
                        port,
                    });
                }
            }
            _ => {}
        }
    }
    found
}

fn port_of(address: &str) -> Option<u16> {
    address.rsplit(':').next()?.parse().ok()
}

/// `lsof -nP -d cwd -F pcn` : le nom système et le dossier courant de chaque
/// processus visible.
pub fn visible(text: &str) -> HashMap<u32, Visible> {
    let mut found = HashMap::new();
    let mut pid: Option<u32> = None;
    for line in text.lines() {
        let Some(tag) = line.chars().next() else {
            continue;
        };
        let value = &line[tag.len_utf8()..];
        match (tag, pid) {
            ('p', _) => pid = value.parse().ok(),
            ('c', Some(pid)) => {
                found.insert(
                    pid,
                    Visible {
                        name: value.to_string(),
                        directory: None,
                    },
                );
            }
            ('n', Some(pid)) => {
                if let Some(entry) = found.get_mut(&pid) {
                    entry.directory = Some(value.to_string());
                }
            }
            _ => {}
        }
    }
    found
}

/// `ps -A -o pid=,ppid=,uid=,etime=,args=` : quatre champs, puis la
/// commande, qui peut contenir des espaces.
pub fn processes(text: &str) -> HashMap<u32, Process> {
    let mut found = HashMap::new();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let (Some(pid), Some(ppid), Some(uid), Some(etime)) =
            (words.next(), words.next(), words.next(), words.next())
        else {
            continue;
        };
        let (Ok(pid), Ok(ppid), Ok(uid), Some(elapsed)) =
            (pid.parse::<u32>(), ppid.parse(), uid.parse(), elapsed(etime))
        else {
            continue;
        };
        let args = words.collect::<Vec<_>>().join(" ");
        found.insert(
            pid,
            Process {
                ppid,
                uid,
                elapsed,
                args,
            },
        );
    }
    found
}

/// Temps écoulé de `ps`, `[[jj-]hh:]mm:ss`, en secondes.
pub fn elapsed(text: &str) -> Option<u64> {
    let (days, clock) = match text.split_once('-') {
        Some((days, rest)) => (days.parse::<u64>().ok()?, rest),
        None => (0, text),
    };
    let parts: Vec<u64> = clock
        .split(':')
        .map(|part| part.parse().ok())
        .collect::<Option<_>>()?;
    let (hours, minutes, seconds) = match parts.as_slice() {
        [minutes, seconds] => (0, *minutes, *seconds),
        [hours, minutes, seconds] => (*hours, *minutes, *seconds),
        _ => return None,
    };
    Some(((days * 24 + hours) * 60 + minutes) * 60 + seconds)
}
```

- [ ] **Step 6: Vérifier qu'ils passent**

Run: `cargo test ports::parse`
Expected: PASS (8 tests).

Run: `cargo build && cargo clippy -- -D warnings && cargo fmt --check && cargo test`
Expected: tout passe. Si `cargo fmt --check` échoue, lancer `cargo fmt` puis relancer.

- [ ] **Step 7: Commit**

```bash
git add tests/fixtures/ports src/ports src/main.rs
git commit -m "Lecture des ports en écoute et de la table des processus"
```

---

### Task 2: Décisions — famille node, chaîne, port critique, lignes

**Files:**
- Create: `src/ports/decide.rs`
- Modify: `src/ports/mod.rs`

**Interfaces:**
- Consumes: `Snapshot`, `Listener`, `Process`, `Visible`, `recorded()` (tâche 1).
- Produces (dans `crate::ports`, réexportés par `pub use decide::{servers, Scope, Server};`) :
  - `pub enum Scope { Node, Port(u16) }`
  - `pub struct Server { pub port: u16, pub pid: u32, pub project: Option<String>, pub directory: Option<String>, pub command: String, pub started_at: DateTime<Utc>, pub user: String, pub critical: Option<String> }`
  - `pub fn servers(snapshot: &Snapshot, scope: Scope, current_uid: u32, now: DateTime<Utc>) -> Vec<Server>`
  - `decide::chain(snapshot: &Snapshot, pid: u32) -> Vec<u32>` (du plus haut parent au processus)
  - `decide::critical(listener: &Listener, current_uid: u32) -> Option<String>`
  - `decide::short_command(args: &str) -> String`

- [ ] **Step 1: Écrire les tests qui échouent**

Dans `src/ports/mod.rs`, ajouter sous `mod parse;` :

```rust
mod decide;

pub use decide::{servers, Scope, Server};
```

`src/ports/decide.rs`, avec des corps provisoires qui compilent :

```rust
//! Ce que `owl` montre et ce qu'il coupe, décidé sur une lecture du système.

use chrono::{DateTime, TimeDelta, Utc};

use super::{Listener, Snapshot};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scope {
    /// Les serveurs de la famille node.
    Node,
    /// Tout ce qui écoute ce port.
    Port(u16),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Server {
    pub port: u16,
    pub pid: u32,
    pub project: Option<String>,
    pub directory: Option<String>,
    pub command: String,
    pub started_at: DateTime<Utc>,
    pub user: String,
    pub critical: Option<String>,
}

pub fn servers(_snapshot: &Snapshot, _scope: Scope, _current_uid: u32, _now: DateTime<Utc>) -> Vec<Server> {
    let _ = TimeDelta::zero();
    Vec::new()
}

pub fn chain(_snapshot: &Snapshot, pid: u32) -> Vec<u32> {
    vec![pid]
}

pub fn critical(_listener: &Listener, _current_uid: u32) -> Option<String> {
    None
}

pub fn short_command(args: &str) -> String {
    args.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{recorded, Process, Visible};

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn date(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text).unwrap().with_timezone(&Utc)
    }

    fn listener(port: u16, uid: u32) -> Listener {
        Listener {
            pid: 10,
            name: "node".into(),
            uid,
            user: if uid == 0 { "root".into() } else { "moi".into() },
            port,
        }
    }

    #[test]
    fn the_node_list_keeps_renamed_servers_and_drops_the_rest() {
        let found = servers(&recorded(), Scope::Node, 501, now());
        assert_eq!(
            found,
            vec![
                Server {
                    port: 3000,
                    pid: 35860,
                    project: Some("erp-menage".into()),
                    directory: Some("/Users/antoinesouesme/dev/projets/gkfa/erp-menage".into()),
                    command: "npm run dev".into(),
                    started_at: date("2026-10-14T20:31:47Z"),
                    user: "antoinesouesme".into(),
                    critical: None,
                },
                Server {
                    port: 5173,
                    pid: 41022,
                    project: Some("site".into()),
                    directory: Some("/Users/antoinesouesme/dev/playground/owl/site".into()),
                    command: "npm run dev".into(),
                    started_at: date("2026-10-15T11:47:23Z"),
                    user: "antoinesouesme".into(),
                    critical: None,
                },
            ]
        );
    }

    #[test]
    fn a_port_search_shows_any_program_with_its_risk() {
        let found = servers(&recorded(), Scope::Port(5432), 501, now());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].pid, 2210);
        assert_eq!(found[0].project.as_deref(), Some("postgresql@16"));
        assert_eq!(
            found[0].command,
            "postgres -D /opt/homebrew/var/postgresql@16"
        );
        assert_eq!(found[0].critical.as_deref(), Some("PostgreSQL"));
        assert_eq!(found[0].started_at, date("2026-10-12T07:47:51Z"));
    }

    #[test]
    fn a_root_directory_names_no_project() {
        let found = servers(&recorded(), Scope::Port(7000), 501, now());
        assert_eq!(found[0].command, "ControlCenter");
        assert_eq!(found[0].directory.as_deref(), Some("/"));
        assert_eq!(found[0].project, None);
        assert_eq!(found[0].critical.as_deref(), Some("AirPlay Receiver"));
    }

    #[test]
    fn a_port_nobody_listens_gives_an_empty_list() {
        assert!(servers(&recorded(), Scope::Port(8080), 501, now()).is_empty());
    }

    #[test]
    fn the_chain_climbs_npm_and_next_up_to_launchd() {
        assert_eq!(chain(&recorded(), 35860), vec![35815, 35834, 35860]);
    }

    #[test]
    fn the_chain_stops_at_the_agents_shell() {
        assert_eq!(chain(&recorded(), 41022), vec![41010, 41022]);
    }

    #[test]
    fn a_program_outside_the_node_family_is_alone_in_its_chain() {
        assert_eq!(chain(&recorded(), 2210), vec![2210]);
    }

    #[test]
    fn the_chain_stops_at_a_parent_of_another_user() {
        let mut snapshot = recorded();
        snapshot.processes.get_mut(&35815).unwrap().uid = 0;
        assert_eq!(chain(&snapshot, 35860), vec![35834, 35860]);
    }

    #[test]
    fn the_chain_stops_at_a_parent_that_is_gone() {
        let mut snapshot = recorded();
        snapshot.processes.remove(&35815);
        snapshot.visible.remove(&35815);
        assert_eq!(chain(&snapshot, 35860), vec![35834, 35860]);
    }

    #[test]
    fn the_chain_survives_a_loop_in_the_table() {
        let mut snapshot = recorded();
        snapshot.processes.insert(
            35815,
            Process {
                ppid: 35860,
                uid: 501,
                elapsed: 1,
                args: "npm run dev".into(),
            },
        );
        snapshot.visible.insert(
            35815,
            Visible {
                name: "node".into(),
                directory: None,
            },
        );
        assert_eq!(chain(&snapshot, 35860), vec![35815, 35834, 35860]);
    }

    #[test]
    fn another_owner_comes_first_among_the_risks() {
        assert_eq!(critical(&listener(5432, 0), 501).as_deref(), Some("owned by root"));
    }

    #[test]
    fn a_port_under_1024_is_a_system_port() {
        assert_eq!(critical(&listener(80, 501), 501).as_deref(), Some("system port"));
        assert_eq!(critical(&listener(1023, 501), 501).as_deref(), Some("system port"));
        assert_eq!(critical(&listener(1024, 501), 501), None);
    }

    #[test]
    fn known_services_are_named() {
        for (port, reason) in [
            (3306, "MySQL"),
            (5000, "AirPlay Receiver"),
            (5432, "PostgreSQL"),
            (5672, "RabbitMQ"),
            (6379, "Redis"),
            (7000, "AirPlay Receiver"),
            (9200, "Elasticsearch"),
            (11211, "Memcached"),
            (27017, "MongoDB"),
        ] {
            assert_eq!(critical(&listener(port, 501), 501).as_deref(), Some(reason));
        }
        assert_eq!(critical(&listener(3000, 501), 501), None);
    }

    #[test]
    fn commands_are_shortened() {
        assert_eq!(
            short_command("node /a/node_modules/.bin/next dev --port 3000"),
            "next dev --port 3000"
        );
        assert_eq!(short_command("npm run dev"), "npm run dev");
        assert_eq!(short_command("/usr/libexec/rapportd"), "rapportd");
        assert_eq!(
            short_command("next-server (v15.1.0) CLAUDE_CODE_SSE_PORT=37229"),
            "next-server (v15.1.0)"
        );
        assert_eq!(short_command("node"), "node");
        assert_eq!(short_command("bun run dev"), "bun run dev");
    }
}
```

- [ ] **Step 2: Vérifier qu'ils échouent**

Run: `cargo test ports::decide`
Expected: FAIL sur la plupart des tests (listes vides, chaînes d'un seul élément, `None`).

- [ ] **Step 3: Écrire les décisions**

Remplacer les quatre fonctions provisoires de `decide.rs` par :

```rust
/// Noms système de la famille node.
const NODE_FAMILY: [&str; 8] = ["node", "npm", "npx", "pnpm", "yarn", "bun", "bunx", "deno"];

/// Services connus qu'on ne coupe pas sans y penser.
const KNOWN: [(u16, &str); 9] = [
    (3306, "MySQL"),
    (5000, "AirPlay Receiver"),
    (5432, "PostgreSQL"),
    (5672, "RabbitMQ"),
    (6379, "Redis"),
    (7000, "AirPlay Receiver"),
    (9200, "Elasticsearch"),
    (11211, "Memcached"),
    (27017, "MongoDB"),
];

/// Garde-fou contre une table de processus incohérente.
const MAX_CHAIN: usize = 64;

fn is_node(name: &str) -> bool {
    NODE_FAMILY.contains(&name)
}

/// Nom système : celui du port en écoute d'abord, puis celui du dossier
/// courant. Jamais la commande de `ps`, qu'un processus peut renommer.
fn name_of(snapshot: &Snapshot, pid: u32) -> Option<&str> {
    snapshot
        .listeners
        .iter()
        .find(|l| l.pid == pid)
        .map(|l| l.name.as_str())
        .or_else(|| snapshot.visible.get(&pid).map(|v| v.name.as_str()))
}

/// Les lignes à montrer, triées par port puis par processus.
pub fn servers(
    snapshot: &Snapshot,
    scope: Scope,
    current_uid: u32,
    now: DateTime<Utc>,
) -> Vec<Server> {
    let mut found: Vec<Server> = snapshot
        .listeners
        .iter()
        .filter(|l| match scope {
            Scope::Node => is_node(&l.name),
            Scope::Port(port) => l.port == port,
        })
        .map(|l| server(snapshot, l, current_uid, now))
        .collect();
    found.sort_by_key(|s| (s.port, s.pid));
    found
}

fn server(snapshot: &Snapshot, listener: &Listener, current_uid: u32, now: DateTime<Utc>) -> Server {
    let top = chain(snapshot, listener.pid)[0];
    let directory = snapshot
        .visible
        .get(&listener.pid)
        .and_then(|v| v.directory.clone());
    let project = directory.as_deref().and_then(project_of);
    let command = snapshot
        .processes
        .get(&top)
        .map(|p| short_command(&p.args))
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| listener.name.clone());
    let elapsed = snapshot
        .processes
        .get(&listener.pid)
        .map_or(0, |p| p.elapsed);
    Server {
        port: listener.port,
        pid: listener.pid,
        project,
        directory,
        command,
        started_at: now - TimeDelta::seconds(i64::try_from(elapsed).unwrap_or(0)),
        user: listener.user.clone(),
        critical: critical(listener, current_uid),
    }
}

/// Dernier élément du dossier ; la racine ne nomme aucun projet.
fn project_of(directory: &str) -> Option<String> {
    directory
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

/// La chaîne node de `pid`, du plus haut parent au processus lui-même. Elle
/// remonte tant que le parent est de la famille node et du même
/// utilisateur : elle s'arrête au shell, au terminal ou à l'agent qui a
/// lancé le serveur. Un processus hors famille est seul dans sa chaîne.
pub fn chain(snapshot: &Snapshot, pid: u32) -> Vec<u32> {
    let mut found = vec![pid];
    if !name_of(snapshot, pid).is_some_and(is_node) {
        return found;
    }
    let uid = snapshot.processes.get(&pid).map(|p| p.uid);
    let mut current = pid;
    while found.len() < MAX_CHAIN {
        let Some(parent) = snapshot.processes.get(&current).map(|p| p.ppid) else {
            break;
        };
        if parent <= 1 || found.contains(&parent) {
            break;
        }
        let same_user = snapshot.processes.get(&parent).map(|p| p.uid) == uid;
        if !same_user || !name_of(snapshot, parent).is_some_and(is_node) {
            break;
        }
        found.push(parent);
        current = parent;
    }
    found.reverse();
    found
}

/// La première raison de se méfier : un autre propriétaire, un port
/// système, un service connu.
pub fn critical(listener: &Listener, current_uid: u32) -> Option<String> {
    if listener.uid != current_uid {
        return Some(format!("owned by {}", listener.user));
    }
    if listener.port < 1024 {
        return Some("system port".to_string());
    }
    KNOWN
        .iter()
        .find(|(port, _)| *port == listener.port)
        .map(|(_, reason)| reason.to_string())
}

/// Commande lisible : le chemin de l'exécutable réduit à son nom ; derrière
/// `node`, `bun` ou `deno`, le script prend la place de l'exécutable ; les
/// réglages `NOM=valeur` collés derrière la commande d'un processus qui
/// s'est renommé disparaissent.
pub fn short_command(args: &str) -> String {
    let mut words: Vec<&str> = args
        .split_whitespace()
        .filter(|word| !is_assignment(word))
        .collect();
    if words.len() > 1 && ["node", "bun", "deno"].contains(&base(words[0])) && words[1].contains('/') {
        words.remove(0);
    }
    if let Some(first) = words.first_mut() {
        *first = base(*first);
    }
    words.join(" ")
}

fn base(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    })
}
```

Note sur `the_chain_survives_a_loop_in_the_table` : 35815 a pour parent 35860, déjà dans la chaîne ; la boucle s'arrête donc là.

- [ ] **Step 4: Vérifier qu'ils passent**

Run: `cargo test ports::decide`
Expected: PASS (14 tests).

Run: `cargo build && cargo clippy -- -D warnings && cargo fmt && cargo fmt --check && cargo test`
Expected: tout passe.

- [ ] **Step 5: Commit**

```bash
git add src/ports
git commit -m "Choix des serveurs node, de la chaîne à couper et des ports critiques"
```

---

### Task 3: Arrêt, liste, recherche et couche système

**Files:**
- Create: `src/ports/stop.rs`, `src/ports/system.rs`
- Modify: `src/ports/mod.rs`, `Cargo.toml`

**Interfaces:**
- Consumes: `Snapshot`, `snapshot_from`, `decide::{servers, chain}`, `Scope`, `Server`.
- Produces (dans `crate::ports`) :
  - `pub enum PortsError { NotListening { pid: u32, port: u16 }, Denied { pid: u32, user: String }, System(String) }` (avec `thiserror`, messages ci-dessous)
  - `pub enum Signal { Terminate, Kill }`, `pub enum SignalError { Gone, Denied }`
  - `pub trait Host { fn snapshot(&self) -> Result<Snapshot, PortsError>; fn current_uid(&self) -> u32; fn now(&self) -> DateTime<Utc>; fn port_taken(&self, port: u16) -> bool; fn signal(&self, pid: u32, signal: Signal) -> Result<(), SignalError>; fn alive(&self, pid: u32) -> bool; fn wait(&self, duration: Duration); }`
  - `pub struct Listing { pub servers: Vec<Server>, pub hidden: bool }`
  - `pub fn list(host: &impl Host) -> Result<Listing, PortsError>`
  - `pub fn search(host: &impl Host, port: u16) -> Result<Listing, PortsError>`
  - `pub struct Stopped { pub stopped: Vec<u32>, pub forced: bool }`
  - `pub fn stop(host: &impl Host, pid: u32, port: u16) -> Result<Stopped, PortsError>`
  - `pub struct Machine;` qui implémente `Host`

- [ ] **Step 1: Ajouter `libc`**

Dans `Cargo.toml`, section `[dependencies]`, après `open = "5"` :

```toml
libc = "0.2"
```

- [ ] **Step 2: Types, liste et recherche dans `mod.rs`**

Dans `src/ports/mod.rs`, remplacer le bloc des `mod`/`use` du haut par :

```rust
mod decide;
mod parse;
mod stop;
mod system;

pub use decide::{servers, Scope, Server};
pub use stop::{stop, Stopped};
pub use system::Machine;

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use thiserror::Error;
```

et ajouter à la fin du fichier, avant `#[cfg(test)] pub(crate) fn recorded` :

```rust
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
```

- [ ] **Step 3: Écrire les tests qui échouent**

`src/ports/stop.rs` avec un corps provisoire et les tests, dont le faux `Host` partagé :

```rust
//! L'arrêt d'un serveur : vérifier qu'il écoute toujours, demander
//! poliment à toute sa chaîne, puis forcer ce qui reste.

use std::time::Duration;

use super::{decide, Host, PortsError, Signal, SignalError};

/// 50 pas de 100 ms : 5 secondes avant l'arrêt forcé.
const GRACE_STEPS: u32 = 50;
const STEP: Duration = Duration::from_millis(100);

#[derive(Debug, PartialEq)]
pub struct Stopped {
    /// La chaîne visée, du plus haut parent au processus qui écoutait.
    pub stopped: Vec<u32>,
    /// L'arrêt forcé a servi.
    pub forced: bool,
}

pub fn stop(_host: &impl Host, pid: u32, _port: u16) -> Result<Stopped, PortsError> {
    let _ = (decide::chain, GRACE_STEPS, STEP, Signal::Kill, SignalError::Gone);
    Ok(Stopped { stopped: vec![pid], forced: false })
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;
    use std::collections::{HashMap, HashSet};

    use chrono::{DateTime, Utc};

    use super::*;
    use crate::ports::{recorded, Snapshot};

    /// Faux système : une lecture figée, des processus qui meurent au
    /// signal voulu, et le journal de tout ce qu'on leur a envoyé.
    pub(crate) struct FakeHost {
        pub snapshot: Snapshot,
        pub taken: HashSet<u16>,
        /// Signal qui suffit à tuer chaque processus ; absent : `Terminate`.
        pub dies_on: HashMap<u32, Signal>,
        pub denied: HashSet<u32>,
        pub alive: RefCell<HashSet<u32>>,
        pub sent: RefCell<Vec<(u32, Signal)>>,
        pub waited: RefCell<Duration>,
    }

    impl FakeHost {
        pub(crate) fn new() -> Self {
            let snapshot = recorded();
            let alive = snapshot.processes.keys().copied().collect();
            Self {
                snapshot,
                taken: HashSet::new(),
                dies_on: HashMap::new(),
                denied: HashSet::new(),
                alive: RefCell::new(alive),
                sent: RefCell::new(Vec::new()),
                waited: RefCell::new(Duration::ZERO),
            }
        }
    }

    impl Host for FakeHost {
        fn snapshot(&self) -> Result<Snapshot, PortsError> {
            Ok(self.snapshot.clone())
        }
        fn current_uid(&self) -> u32 {
            501
        }
        fn now(&self) -> DateTime<Utc> {
            DateTime::parse_from_rfc3339("2026-10-15T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        }
        fn port_taken(&self, port: u16) -> bool {
            self.taken.contains(&port)
        }
        fn signal(&self, pid: u32, signal: Signal) -> Result<(), SignalError> {
            if self.denied.contains(&pid) {
                return Err(SignalError::Denied);
            }
            if !self.alive.borrow().contains(&pid) {
                return Err(SignalError::Gone);
            }
            self.sent.borrow_mut().push((pid, signal));
            let needed = self.dies_on.get(&pid).copied().unwrap_or(Signal::Terminate);
            if signal == Signal::Kill || needed == Signal::Terminate {
                self.alive.borrow_mut().remove(&pid);
            }
            Ok(())
        }
        fn alive(&self, pid: u32) -> bool {
            self.alive.borrow().contains(&pid)
        }
        fn wait(&self, duration: Duration) {
            *self.waited.borrow_mut() += duration;
        }
    }

    #[test]
    fn a_polite_stop_reaches_the_whole_chain_from_the_top() {
        let host = FakeHost::new();
        let stopped = stop(&host, 35860, 3000).unwrap();
        assert_eq!(
            stopped,
            Stopped {
                stopped: vec![35815, 35834, 35860],
                forced: false,
            }
        );
        assert_eq!(
            *host.sent.borrow(),
            vec![
                (35815, Signal::Terminate),
                (35834, Signal::Terminate),
                (35860, Signal::Terminate),
            ]
        );
        assert_eq!(*host.waited.borrow(), Duration::ZERO);
    }

    #[test]
    fn a_stubborn_process_is_killed_after_five_seconds() {
        let mut host = FakeHost::new();
        host.dies_on.insert(35860, Signal::Kill);
        let stopped = stop(&host, 35860, 3000).unwrap();
        assert!(stopped.forced);
        assert_eq!(*host.waited.borrow(), Duration::from_secs(5));
        assert_eq!(host.sent.borrow().last(), Some(&(35860, Signal::Kill)));
        assert!(!host.alive(35860));
    }

    #[test]
    fn a_pid_that_no_longer_listens_on_that_port_is_left_alone() {
        let host = FakeHost::new();
        assert_eq!(
            stop(&host, 35860, 3001),
            Err(PortsError::NotListening { pid: 35860, port: 3001 })
        );
        assert_eq!(
            stop(&host, 99999, 3000),
            Err(PortsError::NotListening { pid: 99999, port: 3000 })
        );
        assert!(host.sent.borrow().is_empty());
    }

    #[test]
    fn a_refused_signal_names_the_owner() {
        let mut host = FakeHost::new();
        host.denied.insert(2210);
        assert_eq!(
            stop(&host, 2210, 5432),
            Err(PortsError::Denied {
                pid: 2210,
                user: "antoinesouesme".into(),
            })
        );
    }

    #[test]
    fn a_process_already_gone_counts_as_stopped() {
        let host = FakeHost::new();
        host.alive.borrow_mut().remove(&35834);
        let stopped = stop(&host, 35860, 3000).unwrap();
        assert_eq!(stopped.stopped, vec![35815, 35834, 35860]);
        assert!(!stopped.forced);
    }

    #[test]
    fn the_error_messages_are_exact() {
        assert_eq!(
            PortsError::NotListening { pid: 48211, port: 3000 }.to_string(),
            "Process 48211 no longer listens on port 3000."
        );
        assert_eq!(
            PortsError::Denied { pid: 48211, user: "root".into() }.to_string(),
            "Permission denied: process 48211 belongs to root."
        );
        assert_eq!(PortsError::System("lsof: boom".into()).to_string(), "lsof: boom");
    }

    #[test]
    fn the_list_shows_node_servers_only() {
        let host = FakeHost::new();
        let listing = crate::ports::list(&host).unwrap();
        let ports: Vec<u16> = listing.servers.iter().map(|s| s.port).collect();
        assert_eq!(ports, vec![3000, 5173]);
        assert!(!listing.hidden);
    }

    #[test]
    fn a_search_reports_a_port_held_by_an_invisible_process() {
        let mut host = FakeHost::new();
        host.taken.insert(22);
        let listing = crate::ports::search(&host, 22).unwrap();
        assert!(listing.servers.is_empty());
        assert!(listing.hidden);
        let free = crate::ports::search(&host, 8080).unwrap();
        assert!(!free.hidden);
    }

    #[test]
    fn a_visible_listener_is_never_reported_hidden() {
        let mut host = FakeHost::new();
        host.taken.insert(5432);
        let listing = crate::ports::search(&host, 5432).unwrap();
        assert_eq!(listing.servers.len(), 1);
        assert!(!listing.hidden);
    }
}
```

`src/ports/system.rs`, avec `outcome` provisoire et ses tests :

```rust
//! La seule couche qui lance `lsof` et `ps` et envoie des signaux. Elle ne
//! décide rien : elle passe le texte brut à `parse` et rapporte ce que le
//! système répond.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
use std::process::Command;
use std::time::Duration;

use chrono::{DateTime, Utc};

use super::{snapshot_from, Host, PortsError, Signal, SignalError, Snapshot};

const LSOF: &str = "/usr/sbin/lsof";
const PS: &str = "/bin/ps";

/// Ce Mac.
pub struct Machine;

/// Sortie d'un outil, ou panne. `lsof` sort en échec quand il ne trouve
/// rien : seule une sortie standard vide accompagnée d'un message d'erreur
/// est une vraie panne.
fn outcome(success: bool, stdout: &str, stderr: &str) -> Result<String, PortsError> {
    let _ = (success, stderr);
    Ok(stdout.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_found_is_an_empty_answer() {
        assert_eq!(outcome(false, "", ""), Ok(String::new()));
    }

    #[test]
    fn a_warning_beside_an_answer_is_ignored() {
        assert_eq!(
            outcome(true, "p1\n", "lsof: WARNING: can't stat()"),
            Ok("p1\n".to_string())
        );
        assert_eq!(outcome(false, "p1\n", "lsof: WARNING"), Ok("p1\n".to_string()));
    }

    #[test]
    fn a_failure_with_a_message_is_reported_verbatim() {
        assert_eq!(
            outcome(false, "", "  lsof: illegal option  \n"),
            Err(PortsError::System("lsof: illegal option".into()))
        );
    }
}
```

- [ ] **Step 4: Vérifier qu'ils échouent**

Run: `cargo test ports::`
Expected: la compilation passe (`pub struct Machine;` existe déjà, sans `impl Host`) ; `a_polite_stop_reaches_the_whole_chain_from_the_top`, `a_stubborn_process_is_killed_after_five_seconds`, `a_pid_that_no_longer_listens_on_that_port_is_left_alone`, `a_refused_signal_names_the_owner` et `a_failure_with_a_message_is_reported_verbatim` échouent.

- [ ] **Step 5: Écrire l'arrêt et la couche système**

Dans `stop.rs`, remplacer la fonction provisoire par :

```rust
pub fn stop(host: &impl Host, pid: u32, port: u16) -> Result<Stopped, PortsError> {
    let snapshot = host.snapshot()?;
    // Un numéro de processus peut avoir été repris entre la liste et le
    // clic : on ne coupe que ce qui écoute encore ce port.
    let Some(listener) = snapshot
        .listeners
        .iter()
        .find(|l| l.pid == pid && l.port == port)
    else {
        return Err(PortsError::NotListening { pid, port });
    };
    let chain = decide::chain(&snapshot, pid);
    let denied = || PortsError::Denied {
        pid,
        user: listener.user.clone(),
    };
    for &member in &chain {
        match host.signal(member, Signal::Terminate) {
            Ok(()) | Err(SignalError::Gone) => {}
            Err(SignalError::Denied) => return Err(denied()),
        }
    }
    for _ in 0..GRACE_STEPS {
        if chain.iter().all(|&member| !host.alive(member)) {
            return Ok(Stopped {
                stopped: chain,
                forced: false,
            });
        }
        host.wait(STEP);
    }
    let mut forced = false;
    for &member in &chain {
        if !host.alive(member) {
            continue;
        }
        match host.signal(member, Signal::Kill) {
            Ok(()) => forced = true,
            Err(SignalError::Gone) => {}
            Err(SignalError::Denied) => return Err(denied()),
        }
    }
    Ok(Stopped {
        stopped: chain,
        forced,
    })
}
```

Dans `system.rs`, remplacer `outcome` par la vraie version et ajouter `Machine` :

```rust
fn outcome(success: bool, stdout: &str, stderr: &str) -> Result<String, PortsError> {
    let message = stderr.trim();
    if !success && stdout.is_empty() && !message.is_empty() {
        return Err(PortsError::System(message.to_string()));
    }
    Ok(stdout.to_string())
}

fn run(program: &str, args: &[&str]) -> Result<String, PortsError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| PortsError::System(format!("{program}: {error}")))?;
    outcome(
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
    )
}

/// Envoie un signal ; `0` ne fait que vérifier que le processus existe.
fn send(pid: u32, number: i32) -> Result<(), SignalError> {
    // `0` et `1` visent un groupe de processus ou `launchd` : jamais.
    let pid = match libc::pid_t::try_from(pid) {
        Ok(pid) if pid > 1 => pid,
        _ => return Err(SignalError::Gone),
    };
    // SAFETY: `kill` ne lit ni n'écrit aucune mémoire de ce programme.
    if unsafe { libc::kill(pid, number) } == 0 {
        return Ok(());
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::EPERM) => Err(SignalError::Denied),
        _ => Err(SignalError::Gone),
    }
}

impl Host for Machine {
    fn snapshot(&self) -> Result<Snapshot, PortsError> {
        Ok(snapshot_from(
            &run(LSOF, &["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pcuLn"])?,
            &run(LSOF, &["-nP", "-d", "cwd", "-F", "pcn"])?,
            &run(PS, &["-A", "-o", "pid=,ppid=,uid=,etime=,args="])?,
        ))
    }

    fn current_uid(&self) -> u32 {
        // SAFETY: `getuid` ne peut pas échouer et ne touche à aucune mémoire.
        unsafe { libc::getuid() }
    }

    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    /// Ouvre le port sans rien y accepter, puis le referme : s'il est déjà
    /// pris, quelqu'un l'écoute, même invisible pour `lsof`.
    fn port_taken(&self, port: u16) -> bool {
        let addresses: [SocketAddr; 4] = [
            (Ipv4Addr::LOCALHOST, port).into(),
            (Ipv4Addr::UNSPECIFIED, port).into(),
            (Ipv6Addr::LOCALHOST, port).into(),
            (Ipv6Addr::UNSPECIFIED, port).into(),
        ];
        addresses.iter().any(|address| {
            matches!(
                TcpListener::bind(address),
                Err(error) if error.kind() == std::io::ErrorKind::AddrInUse
            )
        })
    }

    fn signal(&self, pid: u32, signal: Signal) -> Result<(), SignalError> {
        let number = match signal {
            Signal::Terminate => libc::SIGTERM,
            Signal::Kill => libc::SIGKILL,
        };
        send(pid, number)
    }

    fn alive(&self, pid: u32) -> bool {
        // Un refus veut dire que le processus existe.
        !matches!(send(pid, 0), Err(SignalError::Gone))
    }

    fn wait(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}
```

- [ ] **Step 6: Vérifier qu'ils passent**

Run: `cargo test ports::`
Expected: PASS (tous les tests de `ports`).

Run: `cargo build && cargo clippy -- -D warnings && cargo fmt && cargo fmt --check && cargo test`
Expected: tout passe.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/ports
git commit -m "Arrêt d'un serveur et lecture du système"
```

---

### Task 4: Sous-commandes `owl ports` et `owl stop`

**Files:**
- Modify: `src/main.rs` (retirer le `#[allow(dead_code)]` et son commentaire au-dessus de `mod ports;`)
- Modify: `src/cli/mod.rs`
- Modify: `src/cli/output.rs`
- Generated: `macos/Tests/OwlBarCoreTests/Fixtures/ports.json`, `stop.json`

**Interfaces:**
- Consumes: `ports::{list, search, stop, Machine, Listing, Stopped, PortsError, Server, servers, Scope}`, `ports::recorded()` (tests).
- Produces:
  - `Request::Ports(Option<u16>)`, `Request::Stop { pid: u32, port: u16 }`
  - JSON `owl ports` : `{"servers":[{"port","pid","project","directory","command","started_at","user","critical":{"reason"}|null}],"hidden":bool}`
  - JSON `owl stop` : `{"stopped":[u32],"forced":bool}`
  - `kind` d'erreur : `not_listening`, `permission`, `system`
  - Fichiers `Fixtures/ports.json` (liste node du relevé à `2026-10-15T12:00:00Z`) et `Fixtures/stop.json` (`stopped: [35815, 35834, 35860]`, `forced: false`)

- [ ] **Step 1: Écrire les tests qui échouent (arguments)**

Dans `src/cli/mod.rs`, module `tests`, ajouter :

```rust
    #[test]
    fn ports_takes_an_optional_port() {
        assert_eq!(parse(&args(&["ports"])), Ok(Some(Request::Ports(None))));
        assert_eq!(
            parse(&args(&["ports", "3000"])),
            Ok(Some(Request::Ports(Some(3000))))
        );
    }

    #[test]
    fn stop_takes_a_pid_and_a_port() {
        assert_eq!(
            parse(&args(&["stop", "48211", "3000"])),
            Ok(Some(Request::Stop {
                pid: 48211,
                port: 3000
            }))
        );
    }

    #[test]
    fn malformed_ports_and_stops_are_usage_errors() {
        for bad in [
            &["ports", "0"][..],
            &["ports", "65536"],
            &["ports", "abc"],
            &["ports", "-1"],
            &["ports", "3000", "extra"],
            &["stop"],
            &["stop", "48211"],
            &["stop", "0", "3000"],
            &["stop", "abc", "3000"],
            &["stop", "48211", "0"],
            &["stop", "48211", "70000"],
        ] {
            assert_eq!(parse(&args(bad)), Err(UsageError), "{bad:?}");
        }
    }
```

et dans `the_usage_message_names_every_subcommand`, ajouter :

```rust
        assert!(text.contains("owl ports [<port>]"));
        assert!(text.contains("owl stop <pid> <port>"));
```

- [ ] **Step 2: Écrire les tests qui échouent (JSON)**

Dans `src/cli/output.rs`, module `tests`, ajouter les aides et tests (près de `list_json`) :

```rust
    fn ports_json() -> Value {
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-15T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let listing = crate::ports::Listing {
            servers: crate::ports::servers(
                &crate::ports::recorded(),
                crate::ports::Scope::Node,
                501,
                now,
            ),
            hidden: false,
        };
        let (text, _) = envelope(Ok(ports_data(&listing)));
        serde_json::from_str(&text).unwrap()
    }

    fn stop_json() -> Value {
        let stopped = crate::ports::Stopped {
            stopped: vec![35815, 35834, 35860],
            forced: false,
        };
        let (text, _) = envelope(Ok(stop_data(&stopped)));
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn a_server_is_written_in_full() {
        let json = ports_json();
        assert_eq!(json["ok"], true);
        assert_eq!(json["data"]["hidden"], false);
        assert_eq!(
            json["data"]["servers"][0],
            serde_json::json!({
                "port": 3000,
                "pid": 35860,
                "project": "erp-menage",
                "directory": "/Users/antoinesouesme/dev/projets/gkfa/erp-menage",
                "command": "npm run dev",
                "started_at": "2026-10-14T20:31:47Z",
                "user": "antoinesouesme",
                "critical": null
            })
        );
    }

    #[test]
    fn a_critical_port_carries_its_reason_and_a_missing_project_is_null() {
        let now = chrono::Utc::now();
        let listing = crate::ports::Listing {
            servers: crate::ports::servers(
                &crate::ports::recorded(),
                crate::ports::Scope::Port(7000),
                501,
                now,
            ),
            hidden: false,
        };
        let (text, _) = envelope(Ok(ports_data(&listing)));
        let json: Value = serde_json::from_str(&text).unwrap();
        let server = &json["data"]["servers"][0];
        assert_eq!(server["critical"], serde_json::json!({ "reason": "AirPlay Receiver" }));
        assert_eq!(server["project"], Value::Null);
    }

    #[test]
    fn a_stop_lists_the_chain_and_whether_it_was_forced() {
        assert_eq!(
            stop_json()["data"],
            serde_json::json!({ "stopped": [35815, 35834, 35860], "forced": false })
        );
    }

    #[test]
    fn port_errors_have_their_own_kind() {
        use crate::ports::PortsError;
        for (error, kind) in [
            (PortsError::NotListening { pid: 1, port: 2 }, "not_listening"),
            (
                PortsError::Denied {
                    pid: 1,
                    user: "root".into(),
                },
                "permission",
            ),
            (PortsError::System("boom".into()), "system"),
        ] {
            let body = ErrorBody::from(&error);
            assert_eq!(body.kind, kind);
            assert_eq!(body.message, error.to_string());
        }
    }
```

Dans `the_app_fixtures_are_up_to_date`, compléter `expected` :

```rust
        let expected = [
            ("list.json", pretty(&list_json())),
            ("detail.json", pretty(&detail_json())),
            ("ports.json", pretty(&ports_json())),
            ("stop.json", pretty(&stop_json())),
        ];
```

- [ ] **Step 3: Vérifier qu'ils échouent**

Run: `cargo test cli::`
Expected: FAIL à la compilation (`Request::Ports`, `ports_data`, `stop_data` inconnus).

- [ ] **Step 4: Écrire les sous-commandes**

`src/main.rs` : retirer les deux lignes au-dessus de `mod ports;` (commentaire et `#[allow(dead_code)]`).

`src/cli/mod.rs` :

1. Imports : ajouter `use crate::ports::{self, Machine};` et `use output::{ErrorBody, MergeOut};` reste.
2. `Request` :

```rust
#[derive(Debug, PartialEq)]
pub enum Request {
    List,
    Detail(PrKey),
    Merge(PrKey, MergeMethod),
    /// Les serveurs node, ou tout ce qui écoute ce port.
    Ports(Option<u16>),
    Stop { pid: u32, port: u16 },
}
```

3. `USAGE` :

```rust
const USAGE: &str = "Usage: owl [list | detail <owner/repo> <number> | merge <owner/repo> <number> <merge|squash|rebase> | ports [<port>] | stop <pid> <port>]
  owl list
  owl detail <owner/repo> <number>
  owl merge <owner/repo> <number> <merge|squash|rebase>
  owl ports [<port>]
  owl stop <pid> <port>";
```

4. Dans `parse`, avant `_ => Err(UsageError)` :

```rust
        ["ports"] => Ok(Some(Request::Ports(None))),
        ["ports", port] => Ok(Some(Request::Ports(Some(parse_port(port)?)))),
        ["stop", pid, port] => Ok(Some(Request::Stop {
            pid: parse_pid(pid)?,
            port: parse_port(port)?,
        })),
```

5. Après `parse_method` :

```rust
/// Port de 1 à 65535.
fn parse_port(word: &str) -> Result<u16, UsageError> {
    match word.parse::<u16>() {
        Ok(port) if port > 0 => Ok(port),
        _ => Err(UsageError),
    }
}

/// Numéro de processus strictement positif.
fn parse_pid(word: &str) -> Result<u32, UsageError> {
    match word.parse::<u32>() {
        Ok(pid) if pid > 0 => Ok(pid),
        _ => Err(UsageError),
    }
}
```

6. Dans `run`, après le bras `Request::Merge` — ces deux bras n'utilisent pas `perform` : ni configuration, ni jeton, ni GitHub :

```rust
        Request::Ports(port) => output::envelope(
            match port {
                None => ports::list(&Machine),
                Some(port) => ports::search(&Machine, port),
            }
            .map(|listing| output::ports_data(&listing))
            .map_err(|error| ErrorBody::from(&error)),
        ),
        Request::Stop { pid, port } => output::envelope(
            ports::stop(&Machine, pid, port)
                .map(|stopped| output::stop_data(&stopped))
                .map_err(|error| ErrorBody::from(&error)),
        ),
```

Mettre à jour le commentaire de tête de `run` : « Toute erreur passe par l'enveloppe, réglages et jeton compris » reste vrai ; ajouter « Les ports ne demandent ni réglages ni jeton. ».

`src/cli/output.rs` :

1. Import : `use crate::ports::{Listing, PortsError, Stopped};`
2. Après `impl From<&TokenError> for ErrorBody` :

```rust
impl From<&PortsError> for ErrorBody {
    fn from(error: &PortsError) -> Self {
        let kind = match error {
            PortsError::NotListening { .. } => "not_listening",
            PortsError::Denied { .. } => "permission",
            PortsError::System(_) => "system",
        };
        Self::new(kind, error.to_string())
    }
}
```

3. Après `pub struct MergeOut {}` :

```rust
#[derive(Serialize)]
pub struct PortsOut {
    servers: Vec<ServerOut>,
    hidden: bool,
}

/// `project`, `directory` et `critical` sont écrits `null` quand ils
/// manquent : l'app lit toujours les mêmes clés.
#[derive(Serialize)]
struct ServerOut {
    port: u16,
    pid: u32,
    project: Option<String>,
    directory: Option<String>,
    command: String,
    started_at: String,
    user: String,
    critical: Option<CriticalOut>,
}

#[derive(Serialize)]
struct CriticalOut {
    reason: String,
}

#[derive(Serialize)]
pub struct StopOut {
    stopped: Vec<u32>,
    forced: bool,
}

pub fn ports_data(listing: &Listing) -> PortsOut {
    PortsOut {
        servers: listing
            .servers
            .iter()
            .map(|server| ServerOut {
                port: server.port,
                pid: server.pid,
                project: server.project.clone(),
                directory: server.directory.clone(),
                command: server.command.clone(),
                started_at: date(server.started_at),
                user: server.user.clone(),
                critical: server.critical.clone().map(|reason| CriticalOut { reason }),
            })
            .collect(),
        hidden: listing.hidden,
    }
}

pub fn stop_data(stopped: &Stopped) -> StopOut {
    StopOut {
        stopped: stopped.stopped.clone(),
        forced: stopped.forced,
    }
}
```

- [ ] **Step 5: Vérifier, régénérer les exemples de l'app**

Run: `OWL_UPDATE_FIXTURES=1 cargo test && cargo test`
Expected: PASS ; `macos/Tests/OwlBarCoreTests/Fixtures/ports.json` et `stop.json` existent. Ouvrir `ports.json` : deux serveurs, 3000 puis 5173.

Run: `cargo build && cargo clippy -- -D warnings && cargo fmt && cargo fmt --check`
Expected: tout passe, aucun avertissement de code mort.

Vérification à la main sur ce Mac :

Run: `cargo run -- ports; echo; cargo run -- ports 7000; echo; cargo run -- ports 22; echo; cargo run -- stop 1 1; echo "code $?"`
Expected: quatre documents JSON. `ports 7000` : ControlCenter, `critical` `AirPlay Receiver`. `ports 22` : `servers` vide (si la connexion à distance est active, `hidden: true`). `stop 1 1` : `ok: false`, `kind` `not_listening`, code 1.

Run: `cd macos && swift build --build-system native && ./test.sh`
Expected: PASS (l'app ne lit pas encore les nouveaux fichiers).

- [ ] **Step 6: Commit**

```bash
git add src macos/Tests/OwlBarCoreTests/Fixtures
git commit -m "Sous-commandes owl ports et owl stop"
```

---

### Task 5: Modèles et lecture côté app

**Files:**
- Modify: `macos/Sources/OwlBarCore/Models.swift`
- Modify: `macos/Tests/OwlBarCoreTests/DecodingTests.swift`

**Interfaces:**
- Consumes: `Fixtures/ports.json`, `Fixtures/stop.json`, `decodeOwl`.
- Produces :
  - `public struct ServerId: Hashable, Sendable { public let pid: Int; public let port: Int }`
  - `public struct Critical: Decodable, Equatable, Sendable { public let reason: String }`
  - `public struct Server: Decodable, Equatable, Sendable, Identifiable` avec `port: Int, pid: Int, project: String?, directory: String?, command: String, startedAt: Date, user: String, critical: Critical?`, `id: ServerId`
  - `public struct PortsData: Decodable, Equatable, Sendable { public let servers: [Server]; public let hidden: Bool }`
  - `public struct StopData: Decodable, Equatable, Sendable { public let stopped: [Int]; public let forced: Bool }`

- [ ] **Step 1: Écrire les tests qui échouent**

Dans `DecodingTests`, ajouter :

```swift
    @Test func theRecordedServersAreReadInFull() throws {
        let ports = try decodeOwl(PortsData.self, from: fixture("ports")).get()
        #expect(ports.hidden == false)
        #expect(ports.servers.map(\.port) == [3000, 5173])
        let next = ports.servers[0]
        #expect(next.id == ServerId(pid: 35860, port: 3000))
        #expect(next.project == "erp-menage")
        #expect(next.directory == "/Users/antoinesouesme/dev/projets/gkfa/erp-menage")
        #expect(next.command == "npm run dev")
        #expect(next.startedAt == ISO8601DateFormatter().date(from: "2026-10-14T20:31:47Z"))
        #expect(next.critical == nil)
    }

    @Test func aCriticalServerWithoutProjectIsRead() throws {
        let data = Data(#"{"version":1,"ok":true,"data":{"servers":[{"port":7000,"pid":1055,"project":null,"directory":"/","command":"ControlCenter","started_at":"2026-10-01T09:00:00Z","user":"moi","critical":{"reason":"AirPlay Receiver"}}],"hidden":false}}"#.utf8)
        let server = try decodeOwl(PortsData.self, from: data).get().servers[0]
        #expect(server.project == nil)
        #expect(server.critical == Critical(reason: "AirPlay Receiver"))
    }

    @Test func theRecordedStopIsRead() throws {
        let stop = try decodeOwl(StopData.self, from: fixture("stop")).get()
        #expect(stop.stopped == [35815, 35834, 35860])
        #expect(stop.forced == false)
    }
```

- [ ] **Step 2: Vérifier qu'ils échouent**

Run: `macos/test.sh`
Expected: FAIL à la compilation (`PortsData`, `ServerId`, `Critical`, `StopData` inconnus).

- [ ] **Step 3: Écrire les modèles**

À la fin de `Models.swift` :

```swift
/// Un serveur se reconnaît à son processus et à son port : un même
/// processus peut écouter plusieurs ports.
public struct ServerId: Hashable, Sendable {
    public let pid: Int
    public let port: Int

    public init(pid: Int, port: Int) {
        self.pid = pid
        self.port = port
    }
}

public struct Critical: Decodable, Equatable, Sendable {
    public let reason: String

    public init(reason: String) {
        self.reason = reason
    }
}

public struct Server: Decodable, Equatable, Sendable, Identifiable {
    public let port: Int
    public let pid: Int
    public let project: String?
    public let directory: String?
    public let command: String
    public let startedAt: Date
    public let user: String
    public let critical: Critical?

    public var id: ServerId { ServerId(pid: pid, port: port) }
}

/// Réponse de `owl ports`. `hidden` : le port cherché est pris par un
/// processus que `owl` ne voit pas.
public struct PortsData: Decodable, Equatable, Sendable {
    public let servers: [Server]
    public let hidden: Bool
}

/// Réponse de `owl stop`.
public struct StopData: Decodable, Equatable, Sendable {
    public let stopped: [Int]
    public let forced: Bool
}
```

- [ ] **Step 4: Vérifier qu'ils passent**

Run: `macos/test.sh && (cd macos && swift build --build-system native)`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add macos/Sources/OwlBarCore/Models.swift macos/Tests/OwlBarCoreTests/DecodingTests.swift
git commit -m "Lecture des serveurs dans l'app"
```

---

### Task 6: État et présentation de l'onglet Servers

**Files:**
- Create: `macos/Sources/OwlBarCore/Servers.swift`
- Create: `macos/Tests/OwlBarCoreTests/ServersTests.swift`

**Interfaces:**
- Consumes: `Server`, `ServerId`, `PortsData`, `StopData`, `OwlRunning`, `decodeOwl`, `shortAge`, `Palette`, `MergeRender`, `Segment`, `Tone`, `PanelKey`, `FakeRunner` (tests, défini dans `BarStateTests.swift`).
- Produces :
  - `public func parsePort(_ text: String) -> Int?`
  - `public let noServerMessage: String`, `public func nothingListensMessage(port: Int) -> String`, `public func hiddenMessage(port: Int) -> String`, `public func notAPortMessage(_ text: String) -> String`
  - `public struct ServerRow: Equatable, Identifiable, Sendable { id: ServerId, port: String, project: String, directory: String?, command: String, age: String, stopLabel: String, isStopping: Bool }`
  - `public func serverRows(_ servers: [Server], now: Date, stopping: Set<ServerId>) -> [ServerRow]`
  - `public struct StopDialog: Equatable, Sendable { public enum Stage { confirming, stopping, failed(String) }; public let server: Server; public var stage: Stage }`
  - `public func stopRender(_ dialog: StopDialog) -> MergeRender`
  - `public enum SearchState: Equatable, Sendable { case invalid(String); case found(port: Int, servers: [Server], hidden: Bool) }`
  - `@MainActor public final class ServersState: ObservableObject` avec `init(runner:now:)`, `@Published public var query: String`, publiés en lecture : `servers`, `loaded`, `errorMessage`, `isLoading`, `search`, `stopping`, `dialog`, `notice` ; calculés : `rows`, `placeholder`, `dialogRendering` ; méthodes `reload() async`, `submitSearch() async`, `queryChanged() async`, `clearSearch() async`, `stop(_:) async`, `confirmStop() async`, `cancelStop()`, `press(_:) async`

- [ ] **Step 1: Écrire les tests qui échouent**

`macos/Tests/OwlBarCoreTests/ServersTests.swift` :

```swift
import Foundation
import Testing
@testable import OwlBarCore

func serverObject(port: Int, pid: Int, critical: String? = nil) -> [String: Any] {
    [
        "port": port, "pid": pid, "project": "erp-menage",
        "directory": "/Users/moi/erp-menage", "command": "npm run dev",
        "started_at": "2026-10-15T10:00:00Z", "user": "moi",
        "critical": critical.map { ["reason": $0] as Any } ?? NSNull(),
    ]
}

func portsAnswer(_ servers: [[String: Any]], hidden: Bool = false) -> Result<Data, OwlFailure> {
    let object: [String: Any] = ["version": 1, "ok": true, "data": ["servers": servers, "hidden": hidden]]
    return .success(try! JSONSerialization.data(withJSONObject: object))
}

func stopAnswer() throws -> Result<Data, OwlFailure> {
    .success(try fixture("stop"))
}

@MainActor
final class ServersTests {
    let runner = FakeRunner()
    let clock = ISO8601DateFormatter().date(from: "2026-10-15T12:00:00Z")!
    var state: ServersState!

    init() {
        state = ServersState(runner: runner, now: { [clock] in clock })
    }

    @Test func theRecordedListBecomesRows() async throws {
        runner.answers["ports"] = .success(try fixture("ports"))
        await state.reload()
        #expect(state.rows == [
            ServerRow(id: ServerId(pid: 35860, port: 3000), port: "3000", project: "erp-menage",
                      directory: "/Users/antoinesouesme/dev/projets/gkfa/erp-menage",
                      command: "npm run dev", age: "15h", stopLabel: "Stop", isStopping: false),
            ServerRow(id: ServerId(pid: 41022, port: 5173), port: "5173", project: "site",
                      directory: "/Users/antoinesouesme/dev/playground/owl/site",
                      command: "npm run dev", age: "12m", stopLabel: "Stop", isStopping: false),
        ])
        #expect(state.placeholder == nil)
    }

    @Test func aMissingProjectShowsADash() {
        let rows = serverRows([try! decodeOwl(PortsData.self, from: Data(#"{"version":1,"ok":true,"data":{"servers":[{"port":7000,"pid":1,"project":null,"directory":null,"command":"ControlCenter","started_at":"2026-10-15T11:00:00Z","user":"moi","critical":null}],"hidden":false}}"#.utf8)).get().servers[0]], now: clock, stopping: [])
        #expect(rows[0].project == "—")
        #expect(rows[0].directory == nil)
    }

    @Test func anEmptyListSaysSo() async {
        runner.answers["ports"] = portsAnswer([])
        #expect(state.placeholder == nil)
        await state.reload()
        #expect(state.placeholder == "No node server running")
    }

    @Test func aFailureShowsOwlsMessage() async {
        runner.answers["ports"] = errorAnswer("system", "lsof: boom")
        await state.reload()
        #expect(state.placeholder == "lsof: boom")
    }

    @Test func aReadIsNeverDoubled() async {
        runner.answers["ports"] = portsAnswer([])
        runner.delay = 50_000_000
        async let first: Void = state.reload()
        async let second: Void = state.reload()
        _ = await (first, second)
        #expect(runner.calls.count == 1)
    }

    @Test func stoppingFromTheListIsImmediateThenReloads() async throws {
        runner.answers["ports"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        await state.reload()
        runner.answers["stop 10 3000"] = try stopAnswer()
        runner.answers["ports"] = portsAnswer([])
        await state.stop(ServerId(pid: 10, port: 3000))
        #expect(runner.calls.map { $0.joined(separator: " ") } == ["ports", "stop 10 3000", "ports"])
        #expect(state.dialog == nil)
        #expect(state.notice == nil)
        #expect(state.stopping.isEmpty)
        #expect(state.placeholder == "No node server running")
    }

    @Test func aRowShowsStoppingWhileItsStopRuns() async throws {
        runner.answers["ports"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        await state.reload()
        runner.answers["stop 10 3000"] = try stopAnswer()
        runner.delay = 50_000_000
        let task = Task { await state.stop(ServerId(pid: 10, port: 3000)) }
        try await Task.sleep(nanoseconds: 10_000_000)
        #expect(state.rows[0].stopLabel == "Stopping…")
        #expect(state.rows[0].isStopping)
        await task.value
    }

    @Test func aFailedStopFromTheListShowsTheMessageBelow() async {
        runner.answers["ports"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        await state.reload()
        runner.answers["stop 10 3000"] = errorAnswer("not_listening", "Process 10 no longer listens on port 3000.")
        await state.stop(ServerId(pid: 10, port: 3000))
        #expect(state.notice == "Process 10 no longer listens on port 3000.")
        #expect(runner.calls.last == ["ports"])
    }

    @Test func aSearchShowsWhatListensOnThatPort() async {
        runner.answers["ports 5432"] = portsAnswer([serverObject(port: 5432, pid: 22, critical: "PostgreSQL")])
        state.query = " 5432 "
        await state.submitSearch()
        #expect(state.rows.map(\.port) == ["5432"])
        #expect(runner.calls == [["ports", "5432"]])
    }

    @Test func aSearchWithNothingListeningSaysSo() async {
        runner.answers["ports 8080"] = portsAnswer([])
        state.query = "8080"
        await state.submitSearch()
        #expect(state.placeholder == "Nothing listens on port 8080")
    }

    @Test func aPortHeldByAnInvisibleProcessIsExplained() async {
        runner.answers["ports 22"] = portsAnswer([], hidden: true)
        state.query = "22"
        await state.submitSearch()
        #expect(state.placeholder == "Port 22 is used by a process owl cannot see (system or another user).")
    }

    @Test(arguments: ["abc", "0", "65536", "-1", "30 00"])
    func anInvalidPortIsRefusedWithoutCallingOwl(text: String) async {
        state.query = text
        await state.submitSearch()
        #expect(state.placeholder == "Not a port: \(text)")
        #expect(runner.calls.isEmpty)
    }

    @Test func aDoubleEnterRunsOneSearch() async {
        runner.answers["ports 3000"] = portsAnswer([])
        runner.delay = 50_000_000
        state.query = "3000"
        async let first: Void = state.submitSearch()
        async let second: Void = state.submitSearch()
        _ = await (first, second)
        #expect(runner.calls.count == 1)
    }

    @Test func clearingTheFieldReturnsToTheList() async {
        runner.answers["ports 3000"] = portsAnswer([])
        runner.answers["ports"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        state.query = "3000"
        await state.submitSearch()
        state.query = ""
        await state.queryChanged()
        #expect(state.search == nil)
        #expect(state.rows.count == 1)
    }

    @Test func escapeClearsTheSearch() async {
        runner.answers["ports 3000"] = portsAnswer([])
        runner.answers["ports"] = portsAnswer([])
        state.query = "3000"
        await state.submitSearch()
        await state.press(.escape)
        #expect(state.search == nil)
        #expect(state.query == "")
    }

    @Test func enterInTheServersTabSubmitsTheSearch() async {
        runner.answers["ports 3000"] = portsAnswer([])
        state.query = "3000"
        await state.press(.enter)
        #expect(runner.calls == [["ports", "3000"]])
    }

    @Test func stoppingASearchResultAlwaysAsksFirst() async throws {
        runner.answers["ports 3000"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        state.query = "3000"
        await state.submitSearch()
        await state.stop(ServerId(pid: 10, port: 3000))
        #expect(state.dialog?.stage == .confirming)
        #expect(!runner.calls.contains(["stop", "10", "3000"]))
        let render = try #require(state.dialogRendering)
        #expect(render.title == "Stop npm run dev on port 3000?")
        #expect(render.frameTone == nil)
        #expect(render.confirmLabel == "Stop")
        #expect(render.cancelLabel == "Cancel")
    }

    @Test func aCriticalPortIsFramedInRedWithItsReason() async throws {
        runner.answers["ports 5432"] = portsAnswer([serverObject(port: 5432, pid: 22, critical: "PostgreSQL")])
        state.query = "5432"
        await state.submitSearch()
        await state.stop(ServerId(pid: 22, port: 5432))
        let render = try #require(state.dialogRendering)
        #expect(render.frameTone == .red)
        #expect(render.lines.contains([Segment(text: "Critical: PostgreSQL", tone: .red)]))
    }

    @Test func confirmingStopsThenSearchesAgain() async throws {
        runner.answers["ports 3000"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        state.query = "3000"
        await state.submitSearch()
        await state.stop(ServerId(pid: 10, port: 3000))
        runner.answers["stop 10 3000"] = try stopAnswer()
        runner.answers["ports 3000"] = portsAnswer([])
        await state.press(.enter)
        #expect(state.dialog == nil)
        #expect(runner.calls.suffix(2).map { $0.joined(separator: " ") } == ["stop 10 3000", "ports 3000"])
        #expect(state.placeholder == "Nothing listens on port 3000")
    }

    @Test func aFailedConfirmedStopStaysInTheWindow() async {
        runner.answers["ports 3000"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        state.query = "3000"
        await state.submitSearch()
        await state.stop(ServerId(pid: 10, port: 3000))
        runner.answers["stop 10 3000"] = errorAnswer("permission", "Permission denied: process 10 belongs to root.")
        await state.confirmStop()
        #expect(state.dialog?.stage == .failed("Permission denied: process 10 belongs to root."))
        #expect(state.dialogRendering?.lines.last == [Segment(text: "Permission denied: process 10 belongs to root.", tone: .red)])
    }

    @Test func escapeOrCancelClosesTheWindowWithoutStopping() async {
        runner.answers["ports 3000"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        state.query = "3000"
        await state.submitSearch()
        await state.stop(ServerId(pid: 10, port: 3000))
        await state.press(.escape)
        #expect(state.dialog == nil)
        #expect(state.search != nil)
        #expect(runner.calls.count == 1)
    }

    @Test func whileStoppingTheWindowHasNoButtonAndCannotBeClosed() async throws {
        runner.answers["ports 3000"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        state.query = "3000"
        await state.submitSearch()
        await state.stop(ServerId(pid: 10, port: 3000))
        runner.answers["stop 10 3000"] = try stopAnswer()
        runner.delay = 50_000_000
        let task = Task { await state.confirmStop() }
        try await Task.sleep(nanoseconds: 10_000_000)
        #expect(state.dialogRendering?.confirmLabel == nil)
        #expect(state.dialogRendering?.cancelLabel == nil)
        state.cancelStop()
        #expect(state.dialog != nil)
        await task.value
    }

    @Test func portParsing() {
        #expect(parsePort("3000") == 3000)
        #expect(parsePort(" 3000 ") == 3000)
        #expect(parsePort("1") == 1)
        #expect(parsePort("65535") == 65535)
        #expect(parsePort("0") == nil)
        #expect(parsePort("65536") == nil)
        #expect(parsePort("") == nil)
        #expect(parsePort("3e3") == nil)
    }
}
```

Note : `errorAnswer` et `fixture` viennent de `BarStateTests.swift` et `DecodingTests.swift`, même cible de tests.

- [ ] **Step 2: Vérifier qu'ils échouent**

Run: `macos/test.sh`
Expected: FAIL à la compilation (`ServersState`, `ServerRow`, `parsePort`… inconnus).

- [ ] **Step 3: Écrire `Servers.swift`**

```swift
// Onglet Servers : ce qui s'affiche et ce que fait chaque geste, déjà
// décidés. L'état demande à `owl` (`ports`, `stop`) par `OwlRunning` et ne
// lance rien lui-même ; les vues ne font que dessiner.
import Foundation

public let noServerMessage = "No node server running"

public func nothingListensMessage(port: Int) -> String {
    "Nothing listens on port \(port)"
}

public func hiddenMessage(port: Int) -> String {
    "Port \(port) is used by a process owl cannot see (system or another user)."
}

public func notAPortMessage(_ text: String) -> String {
    "Not a port: \(text)"
}

/// Un port de 1 à 65535, espaces autour tolérés.
public func parsePort(_ text: String) -> Int? {
    let trimmed = text.trimmingCharacters(in: .whitespaces)
    guard !trimmed.isEmpty, trimmed.allSatisfy(\.isASCII), let port = Int(trimmed),
          (1...65535).contains(port)
    else { return nil }
    return port
}

public struct ServerRow: Equatable, Identifiable, Sendable {
    public let id: ServerId
    public let port: String
    public let project: String
    /// Chemin complet, montré au survol du projet.
    public let directory: String?
    public let command: String
    /// Durée depuis le démarrage, au format de l'âge des PR.
    public let age: String
    public let stopLabel: String
    public let isStopping: Bool
}

public func serverRows(_ servers: [Server], now: Date, stopping: Set<ServerId>) -> [ServerRow] {
    servers.map { server in
        let isStopping = stopping.contains(server.id)
        return ServerRow(
            id: server.id,
            port: String(server.port),
            project: server.project ?? "—",
            directory: server.directory,
            command: server.command,
            age: shortAge(from: server.startedAt, now: now),
            stopLabel: isStopping ? "Stopping…" : "Stop",
            isStopping: isStopping
        )
    }
}

/// Fenêtre de confirmation d'un arrêt, ouverte depuis une recherche.
public struct StopDialog: Equatable, Sendable {
    public enum Stage: Equatable, Sendable {
        case confirming
        case stopping
        case failed(String)
    }

    public let server: Server
    public var stage: Stage
}

/// Même forme que la fenêtre de fusion : la vue est commune.
public func stopRender(_ dialog: StopDialog) -> MergeRender {
    let server = dialog.server
    var lines: [[Segment]] = [
        [Segment(text: server.project ?? server.command, tone: Palette.repo)],
    ]
    if let directory = server.directory {
        lines.append([Segment(text: directory, tone: .gray)])
    }
    if let critical = server.critical {
        lines.append([Segment(text: "Critical: \(critical.reason)", tone: .red)])
    }
    if case .failed(let message) = dialog.stage {
        lines.append([Segment(text: message, tone: .red)])
    }
    let busy = dialog.stage == .stopping
    return MergeRender(
        title: "Stop \(server.command) on port \(server.port)?",
        frameTone: server.critical == nil ? nil : .red,
        lines: lines,
        confirmLabel: busy ? nil : "Stop",
        cancelLabel: busy ? nil : "Cancel"
    )
}

/// Recherche affichée à la place de la liste.
public enum SearchState: Equatable, Sendable {
    case invalid(String)
    case found(port: Int, servers: [Server], hidden: Bool)
}

@MainActor
public final class ServersState: ObservableObject {
    /// Texte du champ « Port », lié à la vue.
    @Published public var query = ""
    @Published public private(set) var servers: [Server] = []
    @Published public private(set) var loaded = false
    @Published public private(set) var errorMessage: String?
    @Published public private(set) var isLoading = false
    @Published public private(set) var search: SearchState?
    @Published public private(set) var stopping: Set<ServerId> = []
    @Published public private(set) var dialog: StopDialog?
    /// Message bref d'un arrêt raté depuis la liste.
    @Published public private(set) var notice: String?

    private let runner: OwlRunning
    private let now: () -> Date

    public init(runner: OwlRunning, now: @escaping () -> Date = Date.init) {
        self.runner = runner
        self.now = now
    }

    public var rows: [ServerRow] { serverRows(displayed, now: now(), stopping: stopping) }

    private var displayed: [Server] {
        if case .found(_, let found, _) = search { return found }
        return servers
    }

    /// Message à la place des lignes, ou `nil` quand les lignes suffisent.
    public var placeholder: String? {
        if let errorMessage { return errorMessage }
        switch search {
        case .invalid(let text):
            return notAPortMessage(text)
        case .found(let port, let found, let hidden):
            guard found.isEmpty else { return nil }
            return hidden ? hiddenMessage(port: port) : nothingListensMessage(port: port)
        case nil:
            return loaded && servers.isEmpty ? noServerMessage : nil
        }
    }

    public var dialogRendering: MergeRender? { dialog.map(stopRender) }

    /// Relit ce qui est affiché : la recherche en cours, sinon la liste.
    public func reload() async {
        switch search {
        case .found(let port, _, _): await runSearch(port)
        case .invalid: break
        case nil: await runList()
        }
    }

    /// Entrée dans le champ : un port valide lance la recherche, un champ
    /// vide revient à la liste.
    public func submitSearch() async {
        notice = nil
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty {
            await clearSearch()
            return
        }
        guard let port = parsePort(trimmed) else {
            search = .invalid(query)
            errorMessage = nil
            return
        }
        await runSearch(port)
    }

    /// Le champ a changé : vidé, il ramène la liste.
    public func queryChanged() async {
        if query.trimmingCharacters(in: .whitespaces).isEmpty, search != nil {
            await clearSearch()
        }
    }

    public func clearSearch() async {
        query = ""
        search = nil
        notice = nil
        errorMessage = nil
        await runList()
    }

    /// Bouton Stop d'une ligne. Depuis la liste, l'arrêt part aussitôt ;
    /// depuis une recherche, il passe toujours par la confirmation.
    public func stop(_ id: ServerId) async {
        notice = nil
        if case .found(_, let found, _) = search {
            guard let server = found.first(where: { $0.id == id }) else { return }
            dialog = StopDialog(server: server, stage: .confirming)
            return
        }
        guard servers.contains(where: { $0.id == id }), !stopping.contains(id) else { return }
        stopping.insert(id)
        let result = await send(id)
        stopping.remove(id)
        if case .failure(let failure) = result { notice = failure.message }
        await runList()
    }

    /// Entrée ou bouton Stop dans la fenêtre.
    public func confirmStop() async {
        guard var current = dialog, current.stage != .stopping else { return }
        current.stage = .stopping
        dialog = current
        let result = await send(current.server.id)
        guard dialog?.server.id == current.server.id else { return }
        switch result {
        case .success:
            dialog = nil
            await reload()
        case .failure(let failure):
            current.stage = .failed(failure.message)
            dialog = current
        }
    }

    /// Échap, Cancel ou clic hors de la fenêtre ; sans effet pendant
    /// l'arrêt, qui est parti.
    public func cancelStop() {
        guard let current = dialog, current.stage != .stopping else { return }
        dialog = nil
    }

    /// Touches de l'onglet : la fenêtre d'abord, puis la recherche.
    public func press(_ key: PanelKey) async {
        if dialog != nil {
            switch key {
            case .enter: await confirmStop()
            case .escape: cancelStop()
            default: break
            }
            return
        }
        switch key {
        case .enter: await submitSearch()
        case .escape: if search != nil || !query.isEmpty { await clearSearch() }
        default: break
        }
    }

    private func send(_ id: ServerId) async -> Result<StopData, OwlFailure> {
        await runner.run(["stop", String(id.pid), String(id.port)])
            .flatMap { decodeOwl(StopData.self, from: $0) }
    }

    private func runList() async {
        guard !isLoading else { return }
        isLoading = true
        defer { isLoading = false }
        switch await runner.run(["ports"]).flatMap({ decodeOwl(PortsData.self, from: $0) }) {
        case .success(let data):
            servers = data.servers
            errorMessage = nil
            loaded = true
        case .failure(let failure):
            errorMessage = failure.message
        }
    }

    private func runSearch(_ port: Int) async {
        guard !isLoading else { return }
        isLoading = true
        defer { isLoading = false }
        switch await runner.run(["ports", String(port)]).flatMap({ decodeOwl(PortsData.self, from: $0) }) {
        case .success(let data):
            search = .found(port: port, servers: data.servers, hidden: data.hidden)
            errorMessage = nil
        case .failure(let failure):
            errorMessage = failure.message
        }
    }
}
```

Points à vérifier en lisant les tests : `stoppingFromTheListIsImmediateThenReloads` exige trois appels exacts ; `anInvalidPortIsRefusedWithoutCallingOwl` attend le texte brut du champ dans le message (`"30 00"`), d'où `search = .invalid(query)` ; `MergeRender` doit avoir un initialiseur public avec ces étiquettes — s'il n'en a pas (struct publique sans `init` explicite), ajouter dans `MergeDialog.swift` :

```swift
    public init(title: String, frameTone: Tone?, lines: [[Segment]], confirmLabel: String?, cancelLabel: String?) {
        self.title = title
        self.frameTone = frameTone
        self.lines = lines
        self.confirmLabel = confirmLabel
        self.cancelLabel = cancelLabel
    }
```

(Dans le même module `OwlBarCore`, l'initialiseur membre à membre interne suffit ; ne l'ajouter que si la compilation le demande.)

- [ ] **Step 4: Vérifier qu'ils passent**

Run: `macos/test.sh && (cd macos && swift build --build-system native)`
Expected: PASS, y compris les tests existants.

- [ ] **Step 5: Commit**

```bash
git add macos/Sources/OwlBarCore macos/Tests/OwlBarCoreTests/ServersTests.swift
git commit -m "État et présentation de l'onglet Servers"
```

---

### Task 7: Onglets dans `BarState`

**Files:**
- Modify: `macos/Sources/OwlBarCore/BarState.swift`
- Modify: `macos/Tests/OwlBarCoreTests/BarStateTests.swift`

**Interfaces:**
- Consumes: `ServersState` (tâche 6).
- Le type s'appelle `PanelTab` et non `Tab` : SwiftUI a déjà un `Tab`, l'ambiguïté casserait la compilation des vues.
- Produces :
  - `public enum PanelTab: Sendable, Hashable { case pullRequests, servers }`
  - `PanelKey` gagne `tab1`, `tab2`
  - `BarState` : `@Published public private(set) var tab: PanelTab = .pullRequests`, `public let servers: ServersState`, `public func selectTab(_ tab: PanelTab) async`, `public func panelOpened() async`

- [ ] **Step 1: Écrire les tests qui échouent**

Dans `BarStateTests`, ajouter :

```swift
    @Test func thePanelOpensOnPullRequests() {
        #expect(state.tab == .pullRequests)
    }

    @Test func commandTwoShowsTheServersAndReadsThem() async {
        runner.answers["ports"] = portsAnswer([])
        await state.press(.tab2)
        #expect(state.tab == .servers)
        #expect(runner.calls == [["ports"]])
        await state.press(.tab1)
        #expect(state.tab == .pullRequests)
        #expect(runner.calls.count == 1)
    }

    @Test func reopeningThePanelOnServersReadsThemAgain() async {
        runner.answers["ports"] = portsAnswer([])
        await state.panelOpened()
        #expect(runner.calls.isEmpty)
        await state.selectTab(.servers)
        await state.panelOpened()
        #expect(runner.calls == [["ports"], ["ports"]])
    }

    @Test func onTheServersTabKeysGoToTheServers() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        runner.answers["ports"] = portsAnswer([])
        await state.selectTab(.servers)
        let before = state.selectedKey
        await state.press(.down)
        await state.press(.m)
        #expect(state.selectedKey == before)
        #expect(state.mergeDialog == nil)
        state.servers.query = "abc"
        await state.press(.enter)
        #expect(state.servers.placeholder == "Not a port: abc")
    }

    @Test func tabKeysAreIgnoredWhileTheMergeWindowIsOpen() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        state.startMerge()
        try #require(state.mergeDialog != nil)
        await state.press(.tab2)
        #expect(state.tab == .pullRequests)
    }

    @Test func theCounterStillCountsPullRequestsOnly() async throws {
        runner.answers["list"] = try listAnswer()
        await state.refresh()
        let title = state.title
        runner.answers["ports"] = portsAnswer([serverObject(port: 3000, pid: 10)])
        await state.selectTab(.servers)
        #expect(state.title == title)
    }
```

(Si `startMerge()` de la première PR du relevé n'ouvre pas de fenêtre parce qu'aucune méthode n'est permise, sélectionner avant une PR avec `mergeMethods` non vide ; la première du relevé a `[.squash, .rebase]`, donc ça marche.)

- [ ] **Step 2: Vérifier qu'ils échouent**

Run: `macos/test.sh`
Expected: FAIL à la compilation (`tab`, `.tab2`, `panelOpened`, `servers` inconnus).

- [ ] **Step 3: Écrire les onglets**

Dans `BarState.swift` :

1. `PanelKey` :

```swift
public enum PanelKey: Sendable {
    case up, down, enter, escape, m, o, r
    /// ⌘1 et ⌘2 : onglets Pull requests et Servers.
    case tab1, tab2
}

public enum PanelTab: Sendable, Hashable {
    case pullRequests, servers
}
```

2. Propriétés, après `@Published public private(set) var frame = 0` :

```swift
    @Published public private(set) var tab: PanelTab = .pullRequests
    /// L'onglet Servers, avec son propre état.
    public let servers: ServersState
```

3. Dans `init`, après `self.now = now` :

```swift
        self.servers = ServersState(runner: runner, now: now)
```

4. Après `tick()` :

```swift
    /// Passe d'un onglet à l'autre. Arriver sur Servers relit les serveurs.
    public func selectTab(_ tab: PanelTab) async {
        notice = nil
        self.tab = tab
        if tab == .servers { await servers.reload() }
    }

    /// Le panneau vient de s'ouvrir : les serveurs affichés sont relus.
    public func panelOpened() async {
        if tab == .servers { await servers.reload() }
    }
```

5. Dans `press`, après le bloc `if var dialog = mergeDialog { … return }` :

```swift
        switch key {
        case .tab1:
            await selectTab(.pullRequests)
            return
        case .tab2:
            await selectTab(.servers)
            return
        default:
            break
        }
        if tab == .servers {
            await servers.press(key)
            return
        }
```

et dans le `switch key` final des pull requests, ajouter le bras `case .tab1, .tab2: break` pour qu'il reste exhaustif.

- [ ] **Step 4: Vérifier qu'ils passent**

Run: `macos/test.sh && (cd macos && swift build --build-system native)`
Expected: PASS, tous les tests existants compris.

- [ ] **Step 5: Commit**

```bash
git add macos/Sources/OwlBarCore/BarState.swift macos/Tests/OwlBarCoreTests/BarStateTests.swift
git commit -m "Onglets Pull requests et Servers dans l'état du panneau"
```

---

### Task 8: Vues — barre d'onglets, onglet Servers, fenêtre de confirmation

**Files:**
- Modify: `macos/Sources/OwlBar/PanelView.swift`
- Modify: `macos/Sources/OwlBar/ListView.swift`
- Create: `macos/Sources/OwlBar/ServersView.swift`

**Interfaces:**
- Consumes: `BarState.tab` (`PanelTab`), `selectTab`, `panelOpened`, `servers`, `ServersState` (tout), `ServerRow`, `MergeRender`, `Palette`, `Tone.color`.
- Produces: rien pour les autres tâches. Aucune décision dans ces fichiers : pas de texte calculé, pas de couleur choisie hors de `Palette`/`Tone`.

Le dessin n'a pas de test automatique ; la vérification se fait en construisant et en lançant l'app.

- [ ] **Step 1: `DialogView` générique**

Dans `PanelView.swift`, remplacer `struct MergeDialogView` par une vue qui ne connaît que le rendu et deux actions :

```swift
/// Fenêtre au centre du panneau : fusion ou arrêt d'un serveur. Les lignes,
/// leurs couleurs et les boutons viennent du rendu.
struct DialogView: View {
    let render: MergeRender
    let onConfirm: () -> Void
    let onCancel: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(render.title)
                .font(.headline)
                .foregroundStyle(render.frameTone?.color ?? .primary)
            ForEach(Array(render.lines.enumerated()), id: \.offset) { _, line in
                line.reduce(Text("")) { text, segment in
                    text + Text(segment.text).foregroundColor(segment.tone?.color ?? .primary)
                }
                .fixedSize(horizontal: false, vertical: true)
            }
            if render.confirmLabel != nil || render.cancelLabel != nil {
                HStack {
                    Spacer()
                    if let cancel = render.cancelLabel {
                        Button(cancel, action: onCancel)
                    }
                    if let confirm = render.confirmLabel {
                        Button(confirm, action: onConfirm)
                    }
                }
                .padding(.top, 6)
            } else {
                ProgressView().controlSize(.small)
            }
        }
        .font(.system(.body, design: .monospaced))
        .padding(14)
        .frame(width: 420)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10))
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(render.frameTone?.color ?? Color.secondary.opacity(0.4), lineWidth: 1)
        )
    }
}

/// Fond assombri sous une fenêtre : un clic dessus annule, comme Échap.
struct DialogOverlay: View {
    let render: MergeRender
    let onConfirm: () -> Void
    let onCancel: () -> Void

    var body: some View {
        ZStack {
            Color.black.opacity(0.35)
                .contentShape(Rectangle())
                .onTapGesture(perform: onCancel)
            DialogView(render: render, onConfirm: onConfirm, onCancel: onCancel)
                // Un clic dans la fenêtre, hors des boutons, ne doit pas
                // traverser jusqu'au fond qui l'annule.
                .contentShape(Rectangle())
                .onTapGesture {}
        }
    }
}
```

- [ ] **Step 2: `PanelView` avec onglets et pied commun**

Remplacer `struct PanelView` par :

```swift
struct PanelView: View {
    @ObservedObject var state: BarState
    @StateObject private var login = LaunchAtLoginToggle()

    var body: some View {
        VStack(spacing: 0) {
            Picker("", selection: Binding(
                get: { state.tab },
                set: { tab in Task { await state.selectTab(tab) } }
            )) {
                Text("Pull requests").tag(PanelTab.pullRequests)
                Text("Servers").tag(PanelTab.servers)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .padding([.horizontal, .top], 10)

            switch state.tab {
            case .pullRequests:
                ListView(state: state)
                if let notice = state.notice {
                    Divider()
                    Text(notice)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(10)
                }
            case .servers:
                ServersView(servers: state.servers)
            }

            Divider()

            HStack {
                Toggle("Launch at login", isOn: $login.isOn)
                    .toggleStyle(.checkbox)
                Spacer()
                Text(versionLabel(Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Button("Quit") { NSApplication.shared.terminate(nil) }
                    .keyboardShortcut("q")
            }
            .padding(10)
        }
        .overlay {
            if let render = state.mergeRendering {
                DialogOverlay(
                    render: render,
                    onConfirm: { Task { await state.confirmMerge() } },
                    // `cancelMerge` refuse déjà pendant l'envoi de la fusion.
                    onCancel: { state.cancelMerge() }
                )
            }
        }
        .background(PanelKeys(state: state))
        .onAppear { Task { await state.panelOpened() } }
        .frame(width: 480, height: 520)
    }
}
```

Dans `ListView.swift` : supprimer `@StateObject private var login = LaunchAtLoginToggle()` et le dernier bloc `Divider()` + `HStack { Toggle … Quit }` + `.padding(10)` (le pied est maintenant dans `PanelView`). Si `import AppKit`/`NSApplication` n'est plus utilisé dans `ListView.swift`, ne rien ajouter ; dans `PanelView.swift`, `NSApplication` vient de SwiftUI sur macOS (sinon ajouter `import AppKit`).

- [ ] **Step 3: Raccourcis du panneau**

Remplacer `struct PanelKeys` par :

```swift
/// Le clavier du panneau. Des boutons invisibles portent les raccourcis ;
/// `BarState.press` décide de ce que fait chaque touche. Les lettres ne
/// valent que sur l'onglet des pull requests : sur Servers, elles doivent
/// pouvoir s'écrire dans le champ de recherche.
struct PanelKeys: View {
    @ObservedObject var state: BarState

    var body: some View {
        ZStack {
            key(.upArrow, .up)
            key(.downArrow, .down)
            key(.return, .enter)
            key(.escape, .escape)
            key("1", .tab1, modifiers: .command)
            key("2", .tab2, modifiers: .command)
            if state.tab == .pullRequests {
                key("m", .m)
                key("o", .o)
                key("r", .r)
            }
        }
        .opacity(0)
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }

    private func key(_ shortcut: KeyEquivalent, _ key: PanelKey, modifiers: EventModifiers = []) -> some View {
        Button("") { Task { await state.press(key) } }
            .keyboardShortcut(shortcut, modifiers: modifiers)
    }
}
```

- [ ] **Step 4: `ServersView.swift`**

```swift
import OwlBarCore
import SwiftUI

/// Onglet Servers : champ de recherche, lignes, message bref et fenêtre de
/// confirmation. Tout ce qui s'affiche vient de `ServersState`.
struct ServersView: View {
    @ObservedObject var servers: ServersState

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                TextField("Port", text: $servers.query)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 120)
                    .onSubmit { Task { await servers.submitSearch() } }
                    .onChange(of: servers.query) { Task { await servers.queryChanged() } }
                Spacer()
                Button {
                    Task { await servers.reload() }
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .buttonStyle(.borderless)
                .disabled(servers.isLoading)
            }
            .padding(10)

            Divider()

            if let message = servers.placeholder {
                Text(message)
                    .textSelection(.enabled)
                    .multilineTextAlignment(.center)
                    .padding()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(servers.rows) { row in
                            ServerRowView(row: row) {
                                Task { await servers.stop(row.id) }
                            }
                            Divider()
                        }
                    }
                }
                .frame(maxHeight: .infinity)
            }

            if let notice = servers.notice {
                Divider()
                Text(notice)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(10)
            }
        }
        .overlay {
            if let render = servers.dialogRendering {
                DialogOverlay(
                    render: render,
                    onConfirm: { Task { await servers.confirmStop() } },
                    onCancel: { servers.cancelStop() }
                )
            }
        }
    }
}

struct ServerRowView: View {
    let row: ServerRow
    let stop: () -> Void

    var body: some View {
        HStack(alignment: .center, spacing: 10) {
            Text(row.port)
                .font(.system(.title3, design: .monospaced).bold())
                .frame(minWidth: 60, alignment: .leading)
            VStack(alignment: .leading, spacing: 2) {
                Text(row.project)
                    .foregroundStyle(Palette.repo.color)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .help(row.directory ?? "")
                HStack(spacing: 6) {
                    Text(row.command)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Text(row.age)
                        .foregroundStyle(Palette.age.color)
                        .fixedSize()
                }
                .font(.system(.callout, design: .monospaced))
            }
            Spacer(minLength: 0)
            Button(row.stopLabel, action: stop)
                .disabled(row.isStopping)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
    }
}
```

- [ ] **Step 5: Construire, tester, essayer**

Run: `cd macos && swift build --build-system native && cd .. && macos/test.sh`
Expected: PASS.

Run: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check`
Expected: PASS.

Run: `macos/build.sh`
Expected: « Installed and launched: …/Owl.app ».

Essai à la main, avec un serveur jetable :

```bash
mkdir -p /tmp/owl-essai && cd /tmp/owl-essai && \
echo '{"name":"owl-essai","scripts":{"dev":"node -e \"require(\\\"http\\\").createServer(()=>{}).listen(3999)\""}}' > package.json && \
(npm run dev >/dev/null 2>&1 &)
```

Puis, dans le panneau de la chouette :
1. ⌘2 : l'onglet Servers montre `3999 · owl-essai · npm run dev · 0m`, le survol du projet montre `/tmp/owl-essai` (ou `/private/tmp/owl-essai`).
2. Taper `r`, `m`, `o` dans le champ : les lettres s'écrivent, rien d'autre ne se passe.
3. Taper `7000` puis Entrée : ControlCenter, une seule ligne ; Stop ouvre la fenêtre, cadre rouge, « Critical: AirPlay Receiver » ; Échap la ferme **sans** couper.
4. Vider le champ : la liste revient.
5. Stop sur la ligne 3999 : « Stopping… », puis la ligne disparaît ; `lsof -nP -iTCP:3999 -sTCP:LISTEN` ne rend rien et `pgrep -f "npm run dev"` ne montre plus ce `npm`.
6. Taper `abc` puis Entrée : « Not a port: abc ».
7. ⌘1 : retour aux pull requests, `m`/`o`/`r` y marchent comme avant ; le pied (Launch at login, version, Quit) est présent sur les deux onglets.

Si l'étape 2 échoue (les lettres déclenchent un raccourci), corriger avant de continuer. Si Entrée lance deux recherches visibles, c'est la garde `isLoading` qui l'évite déjà : vérifier qu'une seule ligne de résultat apparaît.

- [ ] **Step 6: Commit**

```bash
git add macos/Sources/OwlBar
git commit -m "Onglet Servers dans le panneau de la chouette"
```

---

### Task 9: Documentation

**Files:**
- Modify: `DESCRIPTION.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: `DESCRIPTION.md`**

Après le paragraphe de la section « Dans la barre des menus », ajouter :

```markdown
Le panneau a deux onglets. « Pull requests » est l'écran décrit plus haut.
« Servers » liste les serveurs node qui écoutent un port sur le Mac — ceux
qu'un agent a lancés et oubliés, par exemple —, avec pour chacun le port, le
projet, la commande et depuis quand il tourne. Un bouton « Stop » coupe le
serveur et les processus node qui l'ont lancé (`npm run dev`, `next dev`),
jamais le terminal ni l'agent. Un champ permet de chercher n'importe quel port,
quel que soit le programme ; couper depuis la recherche demande toujours
confirmation, et la fenêtre passe au rouge pour un port critique (port
système, base de données, AirPlay…). L'app s'appuie sur `owl ports` et
`owl stop`.
```

Dans « Ce que owl ne fait pas », remplacer « Il lit et il fusionne, rien d'autre. » par « Côté GitHub, il lit et il fusionne, rien d'autre. »

- [ ] **Step 2: `CLAUDE.md`**

Dans « Règles d'architecture », ajouter après la ligne de `model` et `filter` :

```markdown
- `ports` ne dépend d'aucun autre module de `owl`, ni du réseau ni du
  terminal. Seul `ports::system` lance `lsof` et `ps` et envoie des signaux ;
  le reste est testé sur des relevés de `tests/fixtures/ports/`.
```

et remplacer la ligne de `cli` par :

```markdown
- `cli` dépend de `model`, `filter`, `config`, `token`, `github` et `ports`,
  jamais de `app` ni `ui`. Son JSON est le contrat de l'app de la barre des
  menus.
```

- [ ] **Step 3: Vérification finale**

Run: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check && (cd macos && swift build --build-system native) && macos/test.sh`
Expected: tout passe.

- [ ] **Step 4: Commit**

```bash
git add DESCRIPTION.md CLAUDE.md
git commit -m "Documentation de l'onglet Servers"
```

Ensuite : pousser la branche et ouvrir une PR vers `develop` (jamais `main`), avec en fin de description la ligne d'attribution demandée par la session.
