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
        assert_eq!(
            outcome(false, "p1\n", "lsof: WARNING"),
            Ok("p1\n".to_string())
        );
    }

    #[test]
    fn a_failure_with_a_message_is_reported_verbatim() {
        assert_eq!(
            outcome(false, "", "  lsof: illegal option  \n"),
            Err(PortsError::System("lsof: illegal option".into()))
        );
    }
}
