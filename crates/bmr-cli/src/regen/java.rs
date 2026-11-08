//! Find a Java runtime and check it can run a given Minecraft version. We never install one.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

const INSTALL_HINT: &str =
    "install a JDK from https://adoptium.net (or your package manager), or point --java at its `java` binary";

pub struct Java {
    pub path: PathBuf,
    pub major: u32,
}

/// `flag`, else `java` on PATH.
pub fn detect(flag: Option<&Path>) -> Result<Java> {
    let path = flag.map_or_else(|| PathBuf::from("java"), Path::to_path_buf);
    let output = match Command::new(&path).arg("-version").output() {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            bail!(
                "--regen-seed runs the vanilla server, which needs Java: `{}` not found; {INSTALL_HINT}",
                path.display()
            )
        }
        Err(e) => return Err(e).with_context(|| format!("running {} -version", path.display())),
    };
    // `java -version` prints to stderr
    let text = String::from_utf8_lossy(&output.stderr);
    let major =
        parse_major(&text).with_context(|| format!("unrecognised `{} -version` output: {text}", path.display()))?;
    Ok(Java { path, major })
}

/// Feature release from `java -version` output: `"25.0.1"` → 25, `"21-ea"` → 21, the old `"1.8.0_412"` → 8.
pub fn parse_major(version_output: &str) -> Option<u32> {
    let line = version_output.lines().find(|l| l.contains("version"))?;
    let quoted = line.split('"').nth(1)?;
    let rest = quoted.strip_prefix("1.").unwrap_or(quoted);
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// `required`: the major version Mojang's metadata lists for `mc`.
pub fn ensure_at_least(java: &Java, required: u32, mc: &str) -> Result<()> {
    if java.major < required {
        bail!(
            "Minecraft {mc} needs Java {required}, but {} is Java {}; {INSTALL_HINT}",
            java.path.display(),
            java.major
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_java_version_lines() {
        let new =
            "openjdk version \"25.0.1\" 2025-10-21\nOpenJDK Runtime Environment Temurin-25.0.1+8 (build 25.0.1+8)";
        assert_eq!(parse_major(new), Some(25));
        assert_eq!(parse_major("java version \"1.8.0_412\"\nJava(TM) SE Runtime"), Some(8));
        assert_eq!(parse_major("openjdk version \"21-ea\" 2023-09-19"), Some(21));
        assert_eq!(parse_major("openjdk version \"22\" 2024-03-19"), Some(22));
        assert_eq!(parse_major("Picked up JAVA_TOOL_OPTIONS: -Xmx1g\nopenjdk version \"17.0.2\""), Some(17));
        assert_eq!(parse_major("no such thing"), None);
    }

    #[test]
    fn version_check_explains_itself() {
        let java = Java { path: "java".into(), major: 21 };
        assert!(ensure_at_least(&java, 21, "1.21.11").is_ok());
        let err = ensure_at_least(&java, 25, "26.3").unwrap_err().to_string();
        assert!(err.contains("Java 25") && err.contains("--java"), "{err}");
    }
}
