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

fn server(
    snapshot: &Snapshot,
    listener: &Listener,
    current_uid: u32,
    now: DateTime<Utc>,
) -> Server {
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
/// utilisateur, et tant qu'il n'a pas d'autre enfant : elle s'arrête au
/// shell, au terminal ou à l'agent qui a lancé le serveur. Un processus hors famille est seul dans sa chaîne.
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
        // Un lanceur (`npm run dev`, `next dev`) n'a qu'un enfant ; un parent
        // qui en a d'autres est un agent écrit en node : on ne le coupe pas.
        let has_other_children = snapshot
            .processes
            .iter()
            .any(|(&pid, p)| p.ppid == parent && pid != current);
        if has_other_children {
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
    if words.len() > 1
        && ["node", "bun", "deno"].contains(&base(words[0]))
        && words[1].contains('/')
    {
        words.remove(0);
    }
    if let Some(first) = words.first_mut() {
        *first = base(first);
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
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn listener(port: u16, uid: u32) -> Listener {
        Listener {
            pid: 10,
            name: "node".into(),
            uid,
            user: if uid == 0 {
                "root".into()
            } else {
                "moi".into()
            },
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
    fn the_chain_stops_at_a_node_agent_with_other_children() {
        let mut snapshot = recorded();
        for (pid, ppid, name, args) in [
            (50000, 40001, "node", "node /usr/local/bin/claude"),
            (50010, 50000, "node", "npx some-mcp-server"),
            (50020, 50010, "node", "node /tmp/mcp/server.js"),
            (50030, 50000, "zsh", "/bin/zsh -c ls"),
        ] {
            snapshot.processes.insert(
                pid,
                Process {
                    ppid,
                    uid: 501,
                    elapsed: 1,
                    args: args.into(),
                },
            );
            snapshot.visible.insert(
                pid,
                Visible {
                    name: name.into(),
                    directory: None,
                },
            );
        }
        assert_eq!(chain(&snapshot, 50020), vec![50010, 50020]);
    }

    #[test]
    fn another_owner_comes_first_among_the_risks() {
        assert_eq!(
            critical(&listener(5432, 0), 501).as_deref(),
            Some("owned by root")
        );
    }

    #[test]
    fn a_port_under_1024_is_a_system_port() {
        assert_eq!(
            critical(&listener(80, 501), 501).as_deref(),
            Some("system port")
        );
        assert_eq!(
            critical(&listener(1023, 501), 501).as_deref(),
            Some("system port")
        );
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
