#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
mod cli;
mod cmd;
mod config;

use anyhow::{Context, Result, anyhow};
use cli::{Cli, Commands};
use config::{Config, User};
use ipnet::Ipv4Net;

fn main() {
    let cli = cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("{e:?}");
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
        let priv_key = cmd::genkey().context("Failed to generate private key")?;
        let pub_key = cmd::pubkey(&priv_key).context("Failed to generate public key")?;
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

    let name_len = config
        .users
        .iter()
        .map(|u| u.name.chars().count())
        .max()
        .unwrap_or(0)
        .max(20);
    let ip_len = config
        .users
        .iter()
        .map(|u| u.ip.to_string().chars().count())
        .max()
        .unwrap_or(0)
        .max(15);
    let pub_key_len = config
        .users
        .iter()
        .map(|u| u.pub_key.chars().count())
        .max()
        .unwrap_or(0)
        .max(44);

    println!(
        "{0:<1$} | {2:<3$} | {4:<5$}",
        "Name", name_len, "IP", ip_len, "Public Key", pub_key_len
    );
    let total_len = name_len + ip_len + pub_key_len + 6;
    println!("{}", "-".repeat(total_len));
    for u in &config.users {
        println!(
            "{0:<1$} | {2:<3$} | {4:<5$}",
            u.name, name_len, u.ip, ip_len, u.pub_key, pub_key_len
        );
    }
    Ok(())
}

fn handle_add(path: &std::path::Path, name: &str) -> Result<()> {
    let mut config = Config::open(path)?;
    if config.users.iter().any(|u| u.name == name) {
        return Err(anyhow!("User '{name}' already exists."));
    }
    let ip = config.find_available_ip(config.server_net)?;
    let priv_key = cmd::genkey().context("Failed to generate user private key")?;
    let pub_key = cmd::pubkey(&priv_key).context("Failed to generate user public key")?;
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
    cmd::up(&config.wg_interface)
        .context("Failed to start wg-quick interface")
        .map(|_| ())
}

fn handle_stop(path: &std::path::Path) -> Result<()> {
    let config = Config::open(path)?;
    cmd::down(&config.wg_interface)
        .context("Failed to stop wg-quick interface")
        .map(|_| ())
}

fn sync_wireguard(path: &std::path::Path) -> Result<()> {
    let config = Config::open(path)?;
    let system_conf = format!("/etc/wireguard/{}.conf", config.wg_interface);
    let mut file = std::fs::File::create(&system_conf)
        .with_context(|| format!("Failed to create config at {system_conf}. Try sudo."))?;
    config
        .write_wg_conf(&mut file)
        .context("Failed to write WireGuard server config to disk")?;
    match cmd::sync(&config.wg_interface) {
        Ok(_) => println!("🚀 System WireGuard configuration updated successfully."),
        Err(e) => {
            eprintln!(
                "⚠️  'wg syncconf' failed. If the interface is down, this is normal. Error: {e}"
            );
        }
    }
    Ok(())
}
