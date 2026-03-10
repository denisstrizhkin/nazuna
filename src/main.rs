#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
mod config;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use config::{Config, User};
use ipnet::Ipv4Net;
use log::{error, info};
use std::{
    fs,
    io::Write as _,
    process::{Command, Stdio},
};

#[derive(Parser)]
#[command(name = "nazuna", version, about = "A minimalist, purely data-driven management tool for WireGuard 🩸", long_about = None)]
struct Cli {
    /// Path to the database file
    #[arg(short, long, default_value = "/etc/nazuna/nazuna.conf")]
    config: std::path::PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the server database and generate keys
    Init {
        #[arg(long, default_value = "127.0.0.1")]
        endpoint_ip: std::net::Ipv4Addr,
        #[arg(long, default_value_t = 51820)]
        endpoint_port: u16,
        #[arg(long)]
        client_dns: Option<std::net::Ipv4Addr>,
        #[arg(long, default_value = "10.50.0.1/24")]
        server_net: Ipv4Net,
        #[arg(long, default_value = "eth0")]
        external_interface: String,
        #[arg(long, default_value = "wg0")]
        wg_interface: String,
    },
    /// List all registered peers
    List,
    /// Add a new peer (e.g. nazuna add "denis-laptop")
    Add { name: String },
    /// Remove an existing peer
    Remove { name: String },
    /// Print the WireGuard client configuration for a specific peer
    Cat { name: String },
    /// Sync the database state with the WireGuard interface (generates config)
    Update,
    /// Start the WireGuard interface
    Start,
    /// Stop the WireGuard interface
    Stop,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        error!("{:?}", e);
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
        info!("⚠️  Database already exists at {}", path.display());
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
        info!(
            "✅ Initialized database at {} with defaults or environment parameters.",
            path.display()
        );
    }
    Ok(())
}

fn handle_list(path: &std::path::Path) -> Result<()> {
    let config = Config::open(path)?;
    info!("📋 Registered Peers:");
    info!("{:<20} | {:<15} | {:<44}", "Name", "IP", "Public Key");
    info!("{}", "-".repeat(85));
    for u in &config.users {
        info!("{:<20} | {:<15} | {:<44}", u.name, u.ip, u.pub_key);
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
    info!("✅ User '{name}' added with IP {ip}");
    Ok(())
}

fn handle_remove(path: &std::path::Path, name: &str) -> Result<()> {
    let mut config = Config::open(path)?;
    let initial_len = config.users.len();
    config.users.retain(|u| u.name != name);

    if config.users.len() < initial_len {
        config.save(path)?;
        info!("🗑️  User '{name}' removed.");
    } else {
        info!("⚠️  User '{name}' not found.");
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
    let mut conf_out = String::new();
    config.write_client_conf(&mut conf_out, user)?;
    print!("{}", conf_out);
    Ok(())
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

    let mut conf = String::new();
    config.write_wg_conf(&mut conf, true)?;

    let system_conf = format!("/etc/wireguard/{wg_if}.conf");
    fs::write(&system_conf, &conf)
        .with_context(|| format!("Failed to write config to {system_conf}. Try sudo."))?;

    let mut wg_only_conf = String::new();
    config.write_wg_conf(&mut wg_only_conf, false)?;

    let mut child = Command::new("wg")
        .args(["setconf", wg_if, "/dev/stdin"])
        .stdin(Stdio::piped())
        .spawn()
        .context("Failed to spawn 'wg setconf'")?;

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    stdin.write_all(wg_only_conf.as_bytes())?;
    drop(stdin);

    let status = child.wait()?;
    if status.success() {
        info!("🚀 System WireGuard configuration updated successfully.");
    } else {
        error!("⚠️  'wg setconf' failed. If the interface is down, this is normal.");
    }
    Ok(())
}
