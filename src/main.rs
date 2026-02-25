#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Write as _,
    fs,
    io::Write as _,
    net::Ipv4Addr,
    process::{Command, Stdio},
};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    List,
    Add { name: String },
    Remove { name: String },
    Cat { name: String },
    Update,
    Start,
    Stop,
}

#[derive(Serialize, Deserialize, Debug)]
struct User {
    name: String,
    ip: Ipv4Addr,
    priv_key: String,
    pub_key: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct Config {
    users: Vec<User>,
    server_pub_key: String,
    server_priv_key: String,
}

impl Config {
    fn load() -> Result<Self> {
        if !std::path::Path::new(DATA_PATH).exists() {
            return Ok(Config::default());
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

struct WgEnv {
    endpoint: String,
    server_net: Ipv4Net,
    external_interface: String,
}

impl WgEnv {
    fn from_env() -> Result<Self> {
        Ok(Self {
            endpoint: std::env::var("WG_ENDPOINT").context(
                "WG_ENDPOINT environment variable is not set (e.g., 'your.server.com:51820')",
            )?,
            server_net: std::env::var("WG_SERVER_IP")
                .context("WG_SERVER_IP environment variable is not set (e.g., '10.50.0.1/24')")?
                .parse()
                .context("Failed to parse WG_SERVER_IP as Ipv4Net")?,
            external_interface: std::env::var("WG_INTERFACE")
                .context("WG_INTERFACE environment variable is not set (e.g., 'eth0')")?,
        })
    }
}

const DATA_PATH: &str = "./users.json";
const LOCAL_CONF: &str = "./server.conf";
const INTERFACE: &str = "wg0";

fn main() -> Result<()> {
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
        println!("Database already exists at {DATA_PATH}");
    } else {
        let priv_key = run_wg(&["genkey"], None)?;
        let pub_key = run_wg(&["pubkey"], Some(&priv_key))?;
        let config = Config {
            users: vec![],
            server_priv_key: priv_key,
            server_pub_key: pub_key,
        };
        config.save()?;
        println!("Initialized empty database at {DATA_PATH}");
    }
    sync_wireguard()
}

fn handle_list() -> Result<()> {
    let config = Config::load()?;
    println!("{:<12} | {:<15} | {:<44}", "Name", "IP", "Public Key");
    println!("{}", "-".repeat(75));
    for u in &config.users {
        println!("{:<12} | {:<15} | {:<44}", u.name, u.ip, u.pub_key);
    }
    Ok(())
}

fn handle_add(name: &str) -> Result<()> {
    let mut config = Config::load()?;
    if config.users.iter().any(|u| u.name == name) {
        return Err(anyhow!("User '{name}' already exists."));
    }

    let env = WgEnv::from_env()?;
    let ip = config.find_available_ip(env.server_net)?;

    let priv_key = run_wg(&["genkey"], None)?;
    let pub_key = run_wg(&["pubkey"], Some(&priv_key))?;

    config.users.push(User {
        name: name.to_string(),
        ip,
        priv_key,
        pub_key,
    });

    config.save()?;
    println!("User '{name}' added with IP {ip}");
    Ok(())
}

fn handle_remove(name: &str) -> Result<()> {
    let mut config = Config::load()?;
    let initial_len = config.users.len();
    config.users.retain(|u| u.name != name);

    if config.users.len() < initial_len {
        config.save()?;
        println!("User '{name}' removed.");
    } else {
        println!("User '{name}' not found.");
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

    let env = WgEnv::from_env()?;
    let prefix = env.server_net.prefix_len();
    let endpoint = &env.endpoint;
    let pub_key = &config.server_pub_key;

    println!(
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
        user.ip, prefix, user.priv_key, pub_key, endpoint, env.server_net
    );
    Ok(())
}

fn handle_update() -> Result<()> {
    sync_wireguard()
}

fn handle_start() -> Result<()> {
    run_wg_quick("up")
}

fn handle_stop() -> Result<()> {
    run_wg_quick("down")
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

fn run_wg_quick(cmd: &str) -> Result<()> {
    let status = Command::new("wg-quick")
        .args([cmd, INTERFACE])
        .status()
        .with_context(|| format!("Failed to execute 'wg-quick {cmd} {INTERFACE}'"))?;

    if !status.success() {
        return Err(anyhow!("wg-quick {cmd} reported failure: {status}"));
    }
    Ok(())
}

fn sync_wireguard() -> Result<()> {
    let config = Config::load()?;
    let env = WgEnv::from_env()?;

    let server_net = &env.server_net;
    let priv_key = &config.server_priv_key;
    let ext_if = &env.external_interface;

    let mut conf = format!(
        "[Interface]
Address = {server_net}
SaveConfig = false
ListenPort = 51820
PrivateKey = {priv_key}
PreUp = sysctl -w net.ipv4.ip_forward=1
PostUp = iptables -A FORWARD -i {INTERFACE} -o {INTERFACE} -j ACCEPT; iptables -t nat -A POSTROUTING -o {ext_if} -j MASQUERADE
PostDown = iptables -D FORWARD -i {INTERFACE} -o {INTERFACE} -j ACCEPT; iptables -t nat -D POSTROUTING -o {ext_if} -j MASQUERADE
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

    fs::write(LOCAL_CONF, &conf).with_context(|| format!("Failed to write {LOCAL_CONF}"))?;
    println!("Generated {LOCAL_CONF}");

    let system_conf = format!("/etc/wireguard/{INTERFACE}.conf");
    match fs::copy(LOCAL_CONF, &system_conf) {
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
                .args(["setconf", INTERFACE, "/dev/stdin"])
                .stdin(Stdio::piped())
                .spawn()
                .context("Failed to spawn 'wg setconf'")?;

            let mut stdin = child.stdin.take().expect("Failed to open stdin");
            stdin.write_all(wg_only_conf.as_bytes())?;
            drop(stdin);

            let status = child.wait()?;
            if status.success() {
                println!("System WireGuard configuration updated successfully.");
            } else {
                eprintln!("'wg setconf' failed. If the interface is down, this is normal.");
            }
        }
        Err(e) => {
            return Err(anyhow!("Failed to copy to {system_conf}: {e}. Try sudo."));
        }
    }
    Ok(())
}
