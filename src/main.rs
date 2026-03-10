#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
mod config;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use config::{Config, User};
use ipnet::Ipv4Net;
use log::{error, info};
use std::{
    fmt::Write as _,
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
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        server_net: Option<Ipv4Net>,
        #[arg(long)]
        external_interface: Option<String>,
        #[arg(long)]
        wg_interface: Option<String>,
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
    if let Err(e) = run() {
        error!("{:?}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::try_parse().with_context(|| "Unable to parse args!")?;
    let config_path = &cli.config;

    match cli.command {
        Commands::Init {
            endpoint,
            server_net,
            external_interface,
            wg_interface,
        } => handle_init(
            config_path,
            endpoint,
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
    endpoint: Option<String>,
    server_net: Option<Ipv4Net>,
    external_interface: Option<String>,
    wg_interface: Option<String>,
) -> Result<()> {
    if path.exists() {
        info!("⚠️  Database already exists at {}", path.display());
    } else {
        let endpoint = endpoint.unwrap_or_else(|| "vpn.example.com:51820".to_string());

        let server_net = server_net.unwrap_or_else(|| "10.50.0.1/24".parse().unwrap());

        let external_interface = external_interface.unwrap_or_else(|| "eth0".to_string());

        let wg_interface = wg_interface.unwrap_or_else(|| "wg0".to_string());

        let priv_key = run_wg(&["genkey"], None)?;
        let pub_key = run_wg(&["pubkey"], Some(&priv_key))?;

        let config = Config {
            users: vec![],
            server_priv_key: priv_key,
            server_pub_key: pub_key,
            endpoint,
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
    let config = Config::load(path)?;
    info!("📋 Registered Peers:");
    info!("{:<20} | {:<15} | {:<44}", "Name", "IP", "Public Key");
    info!("{}", "-".repeat(85));
    for u in &config.users {
        info!("{:<20} | {:<15} | {:<44}", u.name, u.ip, u.pub_key);
    }
    Ok(())
}

fn handle_add(path: &std::path::Path, name: &str) -> Result<()> {
    let mut config = Config::load(path)?;
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
    let mut config = Config::load(path)?;
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
    let config = Config::load(path)?;
    let user = config
        .users
        .iter()
        .find(|u| u.name == name)
        .ok_or_else(|| anyhow!("User '{name}' not found."))?;

    let prefix = config.server_net.prefix_len();
    let endpoint = &config.endpoint;
    let pub_key = &config.server_pub_key;

    info!(
        "[Interface]
Address = {}/{}
PrivateKey = {}
DNS = 1.1.1.1

[Peer]
PublicKey = {}
Endpoint = {}
AllowedIPs = {}, 0.0.0.0/0
PersistentKeepalive = 25
",
        user.ip, prefix, user.priv_key, pub_key, endpoint, config.server_net
    );
    Ok(())
}

fn handle_update(path: &std::path::Path) -> Result<()> {
    sync_wireguard(path)
}

fn handle_start(path: &std::path::Path) -> Result<()> {
    let config = Config::load(path)?;
    run_wg_quick("up", &config.wg_interface)
}

fn handle_stop(path: &std::path::Path) -> Result<()> {
    let config = Config::load(path)?;
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
    let config = Config::load(path)?;

    let server_net = &config.server_net;
    let priv_key = &config.server_priv_key;
    let ext_if = &config.external_interface;
    let wg_if = &config.wg_interface;

    let mut conf = format!(
        "[Interface]
Address = {server_net}
SaveConfig = false
ListenPort = 51820
PrivateKey = {priv_key}
PreUp = sysctl -w net.ipv4.ip_forward=1
PostUp = iptables -A FORWARD -i {wg_if} -o {wg_if} -j ACCEPT; iptables -t nat -A POSTROUTING -o {ext_if} -j MASQUERADE
PostDown = iptables -D FORWARD -i {wg_if} -o {wg_if} -j ACCEPT; iptables -t nat -D POSTROUTING -o {ext_if} -j MASQUERADE
"
    );

    for u in &config.users {
        let name = &u.name;
        let pub_key = &u.pub_key;
        let ip = &u.ip;
        write!(
            conf,
            "\n[Peer]\n# Name: {name}\nPublicKey = {pub_key}\nAllowedIPs = {ip}/32\n"
        )
        .context("Failed to build config string")?;
    }

    let tmp_path = format!("/tmp/nazuna_{}.conf", wg_if);
    fs::write(&tmp_path, &conf)
        .with_context(|| format!("Failed to write temporary config to {tmp_path}"))?;
    info!("✅ Generated temporary config at {tmp_path}");

    let system_conf = format!("/etc/wireguard/{wg_if}.conf");
    match fs::copy(&tmp_path, &system_conf) {
        Ok(_) => {
            // 'wg setconf' does not support 'Address' or 'SaveConfig'.
            // We must strip them before applying.
            let wg_only_conf: String = conf
                .lines()
                .filter(|line| {
                    let l = line.trim().to_lowercase();
                    !l.starts_with("address")
                        && !l.starts_with("saveconfig")
                        && !l.starts_with("preup")
                        && !l.starts_with("postup")
                        && !l.starts_with("postdown")
                })
                .collect::<Vec<_>>()
                .join("\n");

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
        }
        Err(e) => {
            return Err(anyhow!("Failed to copy to {system_conf}: {e}. Try sudo."));
        }
    }
    Ok(())
}
