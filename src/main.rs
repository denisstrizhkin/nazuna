#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
mod cli;
mod cmd;
mod config;

use anyhow::{Context, Result, anyhow};
use cli::{Cli, Commands};
use config::{Config, User};
use ipnet::Ipv4Net;

use std::io::Write;

use crate::cmd::{Wg, WgQuick};

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
        let priv_key = Wg::genkey()?;
        let pub_key = Wg::pubkey(&priv_key)?;
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
    let priv_key = Wg::genkey()?;
    let pub_key = Wg::pubkey(&priv_key)?;
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
    config
        .write_client_conf(&mut std::io::stdout(), user)
        .context("Failed to write client config to stdout")?;
    Ok(())
}

fn handle_update(path: &std::path::Path) -> Result<()> {
    sync_wireguard(path)
}

fn handle_start(path: &std::path::Path) -> Result<()> {
    let config = Config::open(path)?;
    cmd::WgQuick::new(&config.wg_interface).up()
}

fn handle_stop(path: &std::path::Path) -> Result<()> {
    let config = Config::open(path)?;
    cmd::WgQuick::new(&config.wg_interface).down()
}

fn sync_wireguard(path: &std::path::Path) -> Result<()> {
    let config = Config::open(path)?;
    let system_conf = format!("/etc/wireguard/{}.conf", config.wg_interface);
    let mut file = std::fs::File::create(&system_conf)
        .with_context(|| format!("Failed to create config at {system_conf}. Try sudo."))?;
    config
        .write_wg_conf(&mut file)
        .context("Failed to write WireGuard server config to disk")?;
    let wg_if = &config.wg_interface;
    let wg_config = WgQuick::new(wg_if)
        .strip()
        .context("Failed to strip WireGuard config for runtime update")?;
    match Wg::new(wg_if).setconf(|stdin| stdin.write_all(wg_config.as_bytes())) {
        Ok(_) => println!("🚀 System WireGuard configuration updated successfully."),
        Err(e) => {
            eprintln!(
                "⚠️  'wg setconf' failed. If the interface is down, this is normal. Error: {e}"
            );
        }
    }
    Ok(())
}
