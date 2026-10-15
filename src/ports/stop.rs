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
            Err(PortsError::NotListening {
                pid: 35860,
                port: 3001
            })
        );
        assert_eq!(
            stop(&host, 99999, 3000),
            Err(PortsError::NotListening {
                pid: 99999,
                port: 3000
            })
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
            PortsError::NotListening {
                pid: 48211,
                port: 3000
            }
            .to_string(),
            "Process 48211 no longer listens on port 3000."
        );
        assert_eq!(
            PortsError::Denied {
                pid: 48211,
                user: "root".into()
            }
            .to_string(),
            "Permission denied: process 48211 belongs to root."
        );
        assert_eq!(
            PortsError::System("lsof: boom".into()).to_string(),
            "lsof: boom"
        );
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
