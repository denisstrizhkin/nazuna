use anyhow::{Context, Result, anyhow};
use std::io::{self, Write};
use std::process::{Command, Output, Stdio};

pub trait KeyGenerator {
    fn gen_key(&self) -> Result<String>;
    fn pub_key(&self, priv_key: &str) -> Result<String>;
}

pub struct WgKeyGenerator;

impl KeyGenerator for WgKeyGenerator {
    fn gen_key(&self) -> Result<String> {
        genkey()
    }

    fn pub_key(&self, priv_key: &str) -> Result<String> {
        pubkey(priv_key)
    }
}

fn parse_output(bin: &str, args: &[&str], output: io::Result<Output>) -> Result<String> {
    let Output {
        status,
        stdout,
        stderr,
    } = output.with_context(|| format!("Failed to spawn '{} {}'", bin, args.join(" ")))?;
    if status.success() {
        Ok(String::from_utf8(stdout)
            .context("Command output was not valid UTF-8")?
            .trim()
            .to_string())
    } else {
        Err(anyhow!(
            "Command '{} {}' failed with status {}: {}",
            bin,
            args.join(" "),
            status,
            String::from_utf8_lossy(&stderr).trim()
        ))
    }
}

pub fn genkey() -> Result<String> {
    let output = Command::new("wg").arg("genkey").output();
    parse_output("wg", &["genkey"], output)
}

pub fn pubkey(priv_key: &str) -> Result<String> {
    let mut wg = Command::new("wg")
        .arg("pubkey")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to spawn 'wg pubkey'")?;
    let mut stdin = wg.stdin.take().context("Failed to open stdin for wg")?;
    stdin
        .write_all(priv_key.as_bytes())
        .context("Failed to write private key to wg pubkey stdin")?;
    drop(stdin);
    let output = wg.wait_with_output();
    parse_output("wg", &["pubkey"], output)
}

pub fn sync(interface: &str) -> Result<String> {
    let mut wg_quick = Command::new("wg-quick")
        .arg("strip")
        .arg(interface)
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn 'wg-quick strip {interface}'"))?;
    let stdout = wg_quick
        .stdout
        .take()
        .context("Failed to open stdout for wg-quick")?;
    let output = Command::new("wg")
        .arg("syncconf")
        .arg(interface)
        .arg("/dev/stdin")
        .stdin(stdout)
        .output();
    // We should probably check wg_quick status too, but wg syncconf result is what matters most here
    // as it consumes the output. If wg-quick fails, wg syncconf will likely fail with empty input or similar.
    // However, properly we should wait for wg_quick.
    let _ = wg_quick.wait();
    parse_output("wg", &["syncconf", interface, "/dev/stdin"], output)
}

pub fn up(interface: &str) -> Result<String> {
    let output = Command::new("wg-quick").arg("up").arg(interface).output();
    parse_output("wg-quick", &["up", interface], output)
}

pub fn down(interface: &str) -> Result<String> {
    let output = Command::new("wg-quick").arg("down").arg(interface).output();
    parse_output("wg-quick", &["down", interface], output)
}
