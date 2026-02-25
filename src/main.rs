use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
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
            .with_context(|| format!("Failed to read database file {}", DATA_PATH))?;
        serde_json::from_str(&data)
            .with_context(|| format!("Failed to parse database JSON from {}", DATA_PATH))
    }

    fn save(&self) -> Result<()> {
        let data =
            serde_json::to_string_pretty(self).context("Failed to serialize database to JSON")?;
        fs::write(DATA_PATH, data)
            .with_context(|| format!("Failed to write database file to {}", DATA_PATH))
    }

    fn find_available_ip(&self, net: &Ipv4Net) -> Result<Ipv4Addr> {
        let server_ip = net.addr();
        net.hosts()
            .find(|ip| *ip != server_ip && !self.users.iter().any(|u| u.ip == *ip))
            .ok_or_else(|| anyhow!("No available IP addresses in subnet {}", net))
    }
}

struct WgEnv {
    endpoint: String,
    server_net: Ipv4Net,
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
        Commands::Add { name } => handle_add(name),
        Commands::Remove { name } => handle_remove(name),
        Commands::Cat { name } => handle_cat(name),
        Commands::Update => handle_update(),
        Commands::Start => handle_start(),
        Commands::Stop => handle_stop(),
    }
}

fn handle_init() -> Result<()> {
    if !std::path::Path::new(DATA_PATH).exists() {
        let priv_key = run_wg(&["genkey"], None)?;
        let pub_key = run_wg(&["pubkey"], Some(&priv_key))?;
        let config = Config {
            users: vec![],
            server_priv_key: priv_key,
            server_pub_key: pub_key,
        };
        config.save()?;
        println!("Initialized empty database at {DATA_PATH}");
    } else {
        println!("Database already exists at {DATA_PATH}");
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

fn handle_add(name: String) -> Result<()> {
    let mut config = Config::load()?;
    if config.users.iter().any(|u| u.name == name) {
        return Err(anyhow!("User '{}' already exists.", name));
    }

    let env = WgEnv::from_env()?;
    let ip = config.find_available_ip(&env.server_net)?;

    let priv_key = run_wg(&["genkey"], None)?;
    let pub_key = run_wg(&["pubkey"], Some(&priv_key))?;

    config.users.push(User {
        name: name.clone(),
        ip,
        priv_key,
        pub_key,
    });

    config.save()?;
    println!("User '{}' added with IP {}", name, ip);
    Ok(())
}

fn handle_remove(name: String) -> Result<()> {
    let mut config = Config::load()?;
    let initial_len = config.users.len();
    config.users.retain(|u| u.name != name);

    if config.users.len() < initial_len {
        config.save()?;
        println!("User '{}' removed.", name);
    } else {
        println!("User '{}' not found.", name);
    }
    Ok(())
}

fn handle_cat(name: String) -> Result<()> {
    let config = Config::load()?;
    let user = config
        .users
        .iter()
        .find(|u| u.name == name)
        .ok_or_else(|| anyhow!("User '{}' not found.", name))?;

    let env = WgEnv::from_env()?;

    println!(
        "[Interface]
Address = {}/{}
PrivateKey = {}
DNS = 1.1.1.1

[Peer]
PublicKey = {}
Endpoint = {}
AllowedIPs = 0.0.0.0/0
PersistentKeepalive = 25
",
        user.ip,
        env.server_net.prefix_len(),
        user.priv_key,
        config.server_pub_key,
        env.endpoint
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
        let mut stdin = child.stdin.take().unwrap();
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
        .with_context(|| format!("Failed to execute 'wg-quick {} {}'", cmd, INTERFACE))?;

    if !status.success() {
        return Err(anyhow!("wg-quick {} reported failure: {}", cmd, status));
    }
    Ok(())
}

fn sync_wireguard() -> Result<()> {
    let config = Config::load()?;
    let env = WgEnv::from_env()?;

    let mut conf = format!(
        "[Interface]
Address = {}
SaveConfig = false
ListenPort = 51820
PrivateKey = {}
",
        env.server_net, config.server_priv_key
    );

    for u in &config.users {
        conf.push_str(&format!(
            "\n[Peer]
# Name: {}
PublicKey = {}
AllowedIPs = {}/32
",
            u.name, u.pub_key, u.ip
        ));
    }

    fs::write(LOCAL_CONF, &conf).with_context(|| format!("Failed to write {}", LOCAL_CONF))?;
    println!("Generated {LOCAL_CONF}");

    let system_conf = format!("/etc/wireguard/{}.conf", INTERFACE);
    match fs::copy(LOCAL_CONF, &system_conf) {
        Ok(_) => {
            let status = Command::new("wg")
                .args(["setconf", INTERFACE, &system_conf])
                .status()
                .with_context(|| "Failed to exec wg setconf".to_string())?;

            if status.success() {
                println!("System WireGuard configuration updated successfully.");
            } else {
                eprintln!("'wg setconf' failed. If the interface is down, this is normal.");
            }
        }
        Err(e) => {
            return Err(anyhow!(
                "Failed to copy to {}: {}. Try sudo.",
                system_conf,
                e
            ));
        }
    }
    Ok(())
}
