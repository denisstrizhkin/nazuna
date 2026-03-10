#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use ipnet::Ipv4Net;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::{
    fmt::Write as _,
    fs,
    io::Write as _,
    net::Ipv4Addr,
    process::{Command, Stdio},
};

#[derive(Parser)]
#[command(name = "nazuna", version, about = "A minimalist, purely data-driven management tool for WireGuard 🩸", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the server database and generate keys
    Init,
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

#[derive(Serialize, Deserialize, Debug)]
struct User {
    name: String,
    ip: Ipv4Addr,
    priv_key: String,
    pub_key: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    users: Vec<User>,
    server_pub_key: String,
    server_priv_key: String,
    endpoint: String,
    server_net: Ipv4Net,
    external_interface: String,
    wg_interface: String,
}

impl Config {
    fn load() -> Result<Self> {
        if !std::path::Path::new(DATA_PATH).exists() {
            return Err(anyhow!(
                "❌ Database not found at {DATA_PATH}. Please run 'init' first."
            ));
        }
        let data = fs::read_to_string(DATA_PATH)
            .with_context(|| format!("Failed to read database file {DATA_PATH}"))?;
        serde_json::from_str(&data)
            .with_context(|| format!("Failed to parse database JSON from {DATA_PATH}"))
    }

    fn save(&self) -> Result<()> {
        let data =
            serde_json::to_string_pretty(self).context("Failed to serialize database to JSON")?;
        fs::write(DATA_PATH, data)
            .with_context(|| format!("Failed to write database file to {DATA_PATH}"))
    }

    fn find_available_ip(&self, net: Ipv4Net) -> Result<Ipv4Addr> {
        let server_ip = net.addr();
        net.hosts()
            .find(|ip| *ip != server_ip && !self.users.iter().any(|u| u.ip == *ip))
            .ok_or_else(|| anyhow!("No available IP addresses in subnet {net}"))
    }
}

const DATA_PATH: &str = "./users.json";


fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    if let Err(e) = run() {
        error!("{:?}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::try_parse().with_context(|| "Unable to parse args!")?;
    match cli.command {
        Commands::Init => handle_init(),
        Commands::List => handle_list(),
        Commands::Add { name } => handle_add(&name),
        Commands::Remove { name } => handle_remove(&name),
        Commands::Cat { name } => handle_cat(&name),
        Commands::Update => handle_update(),
        Commands::Start => handle_start(),
        Commands::Stop => handle_stop(),
    }
}

fn handle_init() -> Result<()> {
    if std::path::Path::new(DATA_PATH).exists() {
        info!("⚠️  Database already exists at {DATA_PATH}");
    } else {
        let endpoint = std::env::var("WG_ENDPOINT").context(
            "❌ WG_ENDPOINT environment variable is not set (e.g., 'your.server.com:51820')",
        )?;
        let server_net: Ipv4Net = std::env::var("WG_SERVER_IP")
            .context("❌ WG_SERVER_IP environment variable is not set (e.g., '10.50.0.1/24')")?
            .parse()
            .context("❌ Failed to parse WG_SERVER_IP as Ipv4Net")?;
        let external_interface = std::env::var("WG_INTERFACE")
            .context("❌ WG_INTERFACE environment variable is not set (e.g., 'eth0')")?;
        let wg_interface = std::env::var("WG_LOCAL_INTERFACE").unwrap_or_else(|_| "wg0".to_string());

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
        config.save()?;
        info!("✅ Initialized database at {DATA_PATH} with environment parameters.");
    }
    Ok(())
}

fn handle_list() -> Result<()> {
    let config = Config::load()?;
    info!("📋 Registered Peers:");
    info!("{:<20} | {:<15} | {:<44}", "Name", "IP", "Public Key");
    info!("{}", "-".repeat(85));
    for u in &config.users {
        info!("{:<20} | {:<15} | {:<44}", u.name, u.ip, u.pub_key);
    }
    Ok(())
}

fn handle_add(name: &str) -> Result<()> {
    let mut config = Config::load()?;
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

    config.save()?;
    info!("✅ User '{name}' added with IP {ip}");
    Ok(())
}

fn handle_remove(name: &str) -> Result<()> {
    let mut config = Config::load()?;
    let initial_len = config.users.len();
    config.users.retain(|u| u.name != name);

    if config.users.len() < initial_len {
        config.save()?;
        info!("🗑️  User '{name}' removed.");
    } else {
        info!("⚠️  User '{name}' not found.");
    }
    Ok(())
}

fn handle_cat(name: &str) -> Result<()> {
    let config = Config::load()?;
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

fn handle_update() -> Result<()> {
    sync_wireguard()
}

fn handle_start() -> Result<()> {
    let config = Config::load()?;
    run_wg_quick("up", &config.wg_interface)
}

fn handle_stop() -> Result<()> {
    let config = Config::load()?;
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

fn sync_wireguard() -> Result<()> {
    let config = Config::load()?;

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
