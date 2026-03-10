#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
mod cli;
mod config;

use anyhow::{Context, Result, anyhow};
use cli::{Cli, Commands};
use config::{Config, User};
use ipnet::Ipv4Net;
use std::{
    io::Write as _,
    process::{Command, Stdio},
};

fn main() {
    let cli = cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("{:?}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let config_path = &cli.config;

    match cli.command {
        Commands::Init {
            endpoint_ip,
            endpoint_port,
            client_dns,
            server_net,
            external_interface,
            wg_interface,
        } => handle_init(
            config_path,
            endpoint_ip,
            endpoint_port,
            client_dns,
            server_net,
            external_interface,
            wg_interface,
        ),
        Commands::List => handle_list(config_path),
        Commands::Add { name } => handle_add(config_path, &name),
        Commands::Remove { name } => handle_remove(config_path, &name),
        Commands::Cat { name } => handle_cat(config_path, &name),
        Commands::Update => handle_update(config_path),
        Commands::Start => handle_start(config_path),
        Commands::Stop => handle_stop(config_path),
    }
}

fn handle_init(
    path: &std::path::Path,
    endpoint_ip: std::net::Ipv4Addr,
    endpoint_port: u16,
    client_dns: Option<std::net::Ipv4Addr>,
    server_net: Ipv4Net,
    external_interface: String,
    wg_interface: String,
) -> Result<()> {
    if path.exists() {
        println!("⚠️  Database already exists at {}", path.display());
    } else {
        let priv_key = run_wg(&["genkey"], None)?;
        let pub_key = run_wg(&["pubkey"], Some(&priv_key))?;

        let config = Config {
            users: vec![],
            server_priv_key: priv_key,
            server_pub_key: pub_key,
            endpoint_ip,
            endpoint_port,
            client_dns,
            server_net,
            external_interface,
            wg_interface,
        };
        config.save(path)?;
        println!(
            "✅ Initialized database at {} with defaults or environment parameters.",
            path.display()
        );
    }
    Ok(())
}

fn handle_list(path: &std::path::Path) -> Result<()> {
    let config = Config::open(path)?;
    println!("📋 Registered Peers:");
    println!("{:<20} | {:<15} | {:<44}", "Name", "IP", "Public Key");
    println!("{}", "-".repeat(85));
    for u in &config.users {
        println!("{:<20} | {:<15} | {:<44}", u.name, u.ip, u.pub_key);
    }
    Ok(())
}

fn handle_add(path: &std::path::Path, name: &str) -> Result<()> {
    let mut config = Config::open(path)?;
    if config.users.iter().any(|u| u.name == name) {
        return Err(anyhow!("User '{name}' already exists."));
    }

    let ip = config.find_available_ip(config.server_net)?;

    let priv_key = run_wg(&["genkey"], None)?;
    let pub_key = run_wg(&["pubkey"], Some(&priv_key))?;

    config.users.push(User {
        name: name.to_string(),
        ip,
        priv_key,
        pub_key,
    });

    config.save(path)?;
    println!("✅ User '{name}' added with IP {ip}");
    Ok(())
}

fn handle_remove(path: &std::path::Path, name: &str) -> Result<()> {
    let mut config = Config::open(path)?;
    let initial_len = config.users.len();
    config.users.retain(|u| u.name != name);

    if config.users.len() < initial_len {
        config.save(path)?;
        println!("🗑️  User '{name}' removed.");
    } else {
        println!("⚠️  User '{name}' not found.");
    }
    Ok(())
}

fn handle_cat(path: &std::path::Path, name: &str) -> Result<()> {
    let config = Config::open(path)?;
    let user = config
        .users
        .iter()
        .find(|u| u.name == name)
        .ok_or_else(|| anyhow!("User '{name}' not found."))?;
    config.write_client_conf(&mut std::io::stdout(), user)
}

fn handle_update(path: &std::path::Path) -> Result<()> {
    sync_wireguard(path)
}

fn handle_start(path: &std::path::Path) -> Result<()> {
    let config = Config::open(path)?;
    run_wg_quick("up", &config.wg_interface)
}

fn handle_stop(path: &std::path::Path) -> Result<()> {
    let config = Config::open(path)?;
    run_wg_quick("down", &config.wg_interface)
}

fn run_wg(args: &[&str], input: Option<&str>) -> Result<String> {
    let mut child = Command::new("wg")
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn 'wg {}'", args.join(" ")))?;

    if let Some(in_str) = input {
        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        stdin.write_all(in_str.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    } else {
        Err(anyhow!(
            "wg {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn run_wg_quick(cmd: &str, interface: &str) -> Result<()> {
    let status = Command::new("wg-quick")
        .args([cmd, interface])
        .status()
        .with_context(|| format!("Failed to execute 'wg-quick {cmd} {interface}'"))?;

    if !status.success() {
        return Err(anyhow!("wg-quick {cmd} reported failure: {status}"));
    }
    Ok(())
}

fn sync_wireguard(path: &std::path::Path) -> Result<()> {
    let config = Config::open(path)?;
    let wg_if = &config.wg_interface;

    let system_conf = format!("/etc/wireguard/{wg_if}.conf");
    let mut file = std::fs::File::create(&system_conf)
        .with_context(|| format!("Failed to create config at {system_conf}. Try sudo."))?;

    config
        .write_wg_conf(&mut file, true)
        .context("Failed to write WireGuard server config to disk")?;

    let mut child = Command::new("wg")
        .args(["setconf", wg_if, "/dev/stdin"])
        .stdin(Stdio::piped())
        .spawn()
        .context("Failed to spawn 'wg setconf'")?;

    if let Some(mut stdin) = child.stdin.take() {
        config
            .write_wg_conf(&mut stdin, false)
            .context("Failed to write live WireGuard config to stdin")?;
    }

    let status = child.wait()?;
    if status.success() {
        println!("🚀 System WireGuard configuration updated successfully.");
    } else {
        eprintln!("⚠️  'wg setconf' failed. If the interface is down, this is normal.");
    }
    Ok(())
}
