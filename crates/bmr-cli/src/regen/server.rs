//! Drive a headless vanilla server over stdin/stdout.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

const TAIL_LINES: usize = 20;
/// Lines that mean one of our commands was refused, or the server cannot do its job.
const FATAL: [&str; 5] = [
    "Unknown or incomplete command",
    "Incorrect argument",
    "Too many chunks",
    "FAILED TO BIND TO PORT",
    "Failed to start",
];

pub struct Launch<'a> {
    pub java: &'a Path,
    pub jar: &'a Path,
    pub heap: &'a str,
    /// Where the bundled jar unpacks its libraries, shared between runs and versions.
    pub libraries: &'a Path,
    pub dir: &'a Path,
}

pub struct Server {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    tail: VecDeque<String>,
    pub errors: Vec<String>,
}

impl Server {
    pub fn start(l: &Launch<'_>) -> Result<Self> {
        let mut child = Command::new(l.java)
            .arg(format!("-Xmx{}", l.heap))
            .arg(format!("-DbundlerRepoDir={}", l.libraries.display()))
            .arg("-jar")
            .arg(l.jar)
            .arg("--nogui")
            .current_dir(l.dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("starting {}", l.java.display()))?;
        let stdin = child.stdin.take().context("server stdin")?;
        let stdout = child.stdout.take().context("server stdout")?;
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut buf = Vec::new();
            while matches!(reader.read_until(b'\n', &mut buf), Ok(n) if n > 0) {
                if tx.send(String::from_utf8_lossy(&buf).trim_end().to_owned()).is_err() {
                    break;
                }
                buf.clear();
            }
        });
        Ok(Self { child, stdin, lines, tail: VecDeque::new(), errors: Vec::new() })
    }

    /// First line containing one of `needles` within `timeout`; returns which needle.
    pub fn wait_for<'n>(&mut self, needles: &[&'n str], timeout: Duration) -> Result<&'n str> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let line = match self.lines.recv_timeout(remaining) {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => {
                    bail!("server: no {needles:?} within {timeout:?}{}", self.tail_text())
                }
                Err(RecvTimeoutError::Disconnected) => bail!("server exited early{}", self.tail_text()),
            };
            if FATAL.iter().any(|f| line.contains(f)) {
                self.errors.push(line.clone());
            }
            let hit = needles.iter().find(|n| line.contains(**n)).copied();
            self.tail.push_back(line);
            if self.tail.len() > TAIL_LINES {
                self.tail.pop_front();
            }
            if let Some(n) = hit {
                return Ok(n);
            }
        }
    }

    pub fn send(&mut self, command: &str) -> Result<()> {
        writeln!(self.stdin, "{command}").context("writing to the server")?;
        self.stdin.flush().context("writing to the server")
    }

    pub fn query<'n>(&mut self, command: &str, needles: &[&'n str], timeout: Duration) -> Result<&'n str> {
        self.send(command)?;
        self.wait_for(needles, timeout)
    }

    pub fn stop(&mut self, timeout: Duration) -> Result<()> {
        self.send("stop")?;
        // drain to EOF so a full stdout pipe can't keep the JVM from exiting
        let deadline = Instant::now() + timeout;
        loop {
            match self.lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(_) => {}
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => bail!("server did not stop within {timeout:?}{}", self.tail_text()),
            }
        }
        let status = loop {
            if let Some(status) = self.child.try_wait().context("waiting for the server")? {
                break status;
            }
            if Instant::now() > deadline {
                bail!("server did not exit within {timeout:?}{}", self.tail_text());
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        if !status.success() {
            bail!("server exited with {status}{}", self.tail_text());
        }
        Ok(())
    }

    fn tail_text(&self) -> String {
        let mut s = String::from("; last server output:");
        for l in &self.tail {
            let _ = write!(s, "\n  {l}");
        }
        s
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // no wait(): a JVM stalled in the kernel may not die promptly, and the error must still reach the user
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
        }
    }
}

/// An unused local port. Racy, but the server fails loudly on a clash (`FATAL`) and the run can be repeated.
pub fn free_port() -> Result<u16> {
    Ok(TcpListener::bind(("127.0.0.1", 0)).context("finding a free port")?.local_addr()?.port())
}

/// java.util.Properties escaping of a value: backslash, the key/value separators, non-ASCII as \uXXXX.
fn escape_property(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' | ':' | '=' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\n"),
            c if c.is_ascii() => out.push(c),
            c => {
                for unit in c.encode_utf16(&mut [0; 2]) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
        }
    }
    out
}

/// server.properties for a one-shot generation run. A leading space in a seed would be lost, as with a
/// hand-written file.
pub fn properties(seed: &str, level_type: &str, port: u16) -> String {
    let port = port.to_string();
    let entries: [(&str, &str); 14] = [
        ("level-name", "world"),
        ("online-mode", "false"),
        ("server-ip", "127.0.0.1"),
        ("server-port", &port),
        ("spawn-protection", "0"),
        ("max-players", "1"),
        ("view-distance", "4"),
        ("simulation-distance", "4"),
        // 1.21.2+ pauses an empty server after 60 s, which also stops force-loaded chunks from generating
        ("pause-when-empty-seconds", "0"),
        // force-loading a batch can take one tick past the 60 s watchdog on slower cores
        ("max-tick-time", "-1"),
        // per-chunk durability is pointless here: a run that does not stop cleanly is discarded
        ("sync-chunk-writes", "false"),
        ("generate-structures", "true"),
        ("level-type", level_type),
        ("level-seed", seed),
    ];
    entries.iter().map(|(k, v)| format!("{k}={}\n", escape_property(v))).collect()
}

/// Absolute form, so a relative path survives the server's own working directory. A bare command name stays.
pub fn absolute(p: &Path) -> Result<PathBuf> {
    if p.components().count() == 1 && !p.exists() {
        return Ok(p.to_path_buf());
    }
    std::path::absolute(p).with_context(|| p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_escape_seeds_and_namespaced_values() {
        let p = properties("a=b:c é", "minecraft:normal", 25599);
        assert!(p.contains("level-type=minecraft\\:normal\n"), "{p}");
        assert!(p.contains("level-seed=a\\=b\\:c \\u00e9\n"), "{p}");
        assert!(p.contains("server-port=25599\n"), "{p}");
        assert!(p.contains("pause-when-empty-seconds=0\n"), "{p}");
    }

    #[test]
    fn escapes_supplementary_characters_as_surrogates() {
        assert_eq!(escape_property("😀"), "\\ud83d\\ude00");
    }

    #[cfg(unix)]
    #[test]
    fn a_server_that_never_answers_times_out_without_blocking_on_exit() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("bmr-server-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("fake-java");
        std::fs::write(&fake, "#!/bin/sh\necho 'Done ('\nexec sleep 30\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let started = Instant::now();
        let launch = Launch { java: &fake, jar: &fake, heap: "1G", libraries: &dir, dir: &dir };
        let mut server = Server::start(&launch).unwrap();
        server.wait_for(&["Done ("], Duration::from_secs(10)).unwrap();
        let err = server.query("save-all flush", &["Saved the game"], Duration::from_millis(200)).unwrap_err();
        assert!(err.to_string().contains("Saved the game"), "{err}");
        drop(server);
        assert!(started.elapsed() < Duration::from_secs(20), "{:?}", started.elapsed());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn bare_command_names_stay_bare() {
        assert_eq!(absolute(Path::new("java")).unwrap(), PathBuf::from("java"));
        assert!(absolute(Path::new("some/dir/java")).unwrap().is_absolute());
    }
}
