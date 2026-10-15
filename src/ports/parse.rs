//! Lecture des réponses de `lsof` et `ps`. Fonctions pures : le texte entre,
//! des valeurs sortent ; une ligne illisible est ignorée.

use std::collections::HashMap;

use super::{Listener, Process, Visible};

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
        let (Ok(pid), Ok(ppid), Ok(uid), Some(elapsed)) = (
            pid.parse::<u32>(),
            ppid.parse(),
            uid.parse(),
            elapsed(etime),
        ) else {
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
