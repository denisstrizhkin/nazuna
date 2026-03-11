use anyhow::{Context, Result, anyhow};
use std::io::{self, Write};
use std::process::{ChildStdin, Command, Stdio};

/// Core command runner that handles process spawning and error reporting.
fn run_proto<F>(bin: &str, args: &[&str], stdin_w: Option<F>) -> Result<String>
where
    F: FnOnce(&mut ChildStdin) -> io::Result<()>,
{
    let mut child = Command::new(bin)
        .args(args)
        .stdin(if stdin_w.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn '{} {}'", bin, args.join(" ")))?;
    if let Some(writer) = stdin_w {
        let mut stdin = child
            .stdin
            .take()
            .expect("stdin is always piped when stdin_w is Some");
        writer(&mut stdin).context("Failed to write to stdin")?;
    }
    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    } else {
        Err(anyhow!(
            "Command '{} {}' failed with status {}: {}",
            bin,
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Run a command without stdin.
pub fn run(bin: &str, args: &[&str]) -> Result<String> {
    run_proto(bin, args, None::<fn(&mut ChildStdin) -> io::Result<()>>)
}

/// Run a command with a specialized stdin writer.
pub fn run_with_stdin<F>(bin: &str, args: &[&str], writer: F) -> Result<String>
where
    F: FnOnce(&mut ChildStdin) -> io::Result<()>,
{
    run_proto(bin, args, Some(writer))
}

pub struct Wg<'a> {
    interface: Option<&'a str>,
}

impl<'a> Wg<'a> {
    /// Create a Wg instance for a specific interface.
    pub fn new(interface: &'a str) -> Self {
        Self {
            interface: Some(interface),
        }
    }

    /// Run 'wg genkey'.
    pub fn genkey() -> Result<String> {
        run("wg", &["genkey"])
    }

    /// Run 'wg pubkey' with the given private key via stdin.
    pub fn pubkey(priv_key: &str) -> Result<String> {
        run_with_stdin("wg", &["pubkey"], |stdin| {
            stdin.write_all(priv_key.as_bytes())?;
            Ok(())
        })
    }

    /// Run 'wg setconf' for the associated interface.
    pub fn setconf<F>(self, writer: F) -> Result<()>
    where
        F: FnOnce(&mut ChildStdin) -> io::Result<()>,
    {
        let iface = self
            .interface
            .context("WireGuard operation requires an interface")?;
        run_with_stdin("wg", &["setconf", iface, "/dev/stdin"], writer)?;
        Ok(())
    }
}

pub struct WgQuick<'a> {
    interface: &'a str,
}

impl<'a> WgQuick<'a> {
    pub fn new(interface: &'a str) -> Self {
        Self { interface }
    }

    pub fn up(self) -> Result<()> {
        run("wg-quick", &["up", self.interface]).map(|_| ())
    }

    pub fn down(self) -> Result<()> {
        run("wg-quick", &["down", self.interface]).map(|_| ())
    }

    pub fn strip(self) -> Result<String> {
        run("wg-quick", &["strip", self.interface])
    }
}
