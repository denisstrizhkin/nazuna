#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
mod cli;
mod cmd;
mod config;

use anyhow::{Context, Result};
use cli::{Cli, Commands};
use cmd::{KeyGenerator, WgKeyGenerator};
use config::{Config, User};
use ipnet::Ipv4Net;

fn main() {
    let cli = cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("{e:?}");
        std::process::exit(1);
    }
}

struct Nazuna<'a, T: KeyGenerator> {
    key_gen: T,
    config: Config,
    config_path: &'a std::path::Path,
}

impl<T: KeyGenerator> Nazuna<'_, T> {
    fn save_config(&self) -> Result<()> {
        self.config.save(self.config_path)
    }

    fn user_display_column_width<F>(&self, selector: F, min_width: usize) -> usize
    where
        F: Fn(&User) -> usize,
    {
        self.config
            .get_users()
            .iter()
            .map(selector)
            .max()
            .unwrap_or_default()
            .max(min_width)
    }

    fn handle_list(&self) {
        println!("📋 Registered Peers:");
        let name_len = self.user_display_column_width(|u| u.name.chars().count(), 20);
        let ip_len = self.user_display_column_width(|u| u.ip.to_string().chars().count(), 15);
        let pub_key_len = self.user_display_column_width(|u| u.pub_key.chars().count(), 44);
        println!(
            "{name:<name_len$} | {ip:<ip_len$} | {pub_key:<pub_key_len$}",
            name = "Name",
            ip = "IP",
            pub_key = "Public Key"
        );
        let total_len = name_len + ip_len + pub_key_len + 6;
        println!("{}", "-".repeat(total_len));
        for u in self.config.get_users() {
            println!(
                "{name:<name_len$} | {ip:<ip_len$} | {pub_key:<pub_key_len$}",
                name = u.name,
                ip = u.ip,
                pub_key = u.pub_key
            );
        }
    }

    fn handle_add(&mut self, name: &str) -> Result<()> {
        self.config
            .find_user(name)
            .with_context(|| format!("User '{name}' already exists."))?;
        let user = self.config.add_user(name.to_string(), &self.key_gen)?;
        self.save_config()?;
        println!("✅ User '{name}' added with IP {ip}", ip = user.ip);
        Ok(())
    }

    fn handle_remove(&mut self, name: &str) -> Result<()> {
        match self.config.remove_user(name) {
            Some(_) => {
                self.save_config()?;
                println!("🗑️  User '{name}' removed.");
            }
            None => println!("⚠️  User '{name}' not found."),
        }
        Ok(())
    }

    fn handle_cat(&self, name: &str) -> Result<()> {
        let user = self
            .config
            .find_user(name)
            .with_context(|| format!("User '{name}' not found."))?;
        self.config
            .write_client_conf(&mut std::io::stdout(), user)
            .context("Failed to write client config to stdout")?;
        Ok(())
    }

    fn handle_update(&self) -> Result<()> {
        self.sync_wireguard()
    }

    fn handle_start(&self) -> Result<()> {
        self.sync_wireguard()?;
        cmd::up(self.config.get_wg_interface())
            .context("Failed to start wg-quick interface")
            .map(|_| ())
    }

    fn handle_stop(&self) -> Result<()> {
        cmd::down(self.config.get_wg_interface())
            .context("Failed to stop wg-quick interface")
            .map(|_| ())
    }

    fn sync_wireguard(&self) -> Result<()> {
        let system_conf = format!("/etc/wireguard/{}.conf", self.config.get_wg_interface());
        let mut file = std::fs::File::create(&system_conf)
            .with_context(|| format!("Failed to create config at {system_conf}. Try sudo."))?;
        self.config
            .write_wg_conf(&mut file)
            .context("Failed to write WireGuard server config to disk")?;
        match cmd::sync(self.config.get_wg_interface()) {
            Ok(_) => println!("🚀 System WireGuard configuration updated successfully."),
            Err(e) => {
                eprintln!(
                    "⚠️  'wg syncconf' failed. If the interface is down, this is normal. Error: {e}"
                );
            }
        }
        Ok(())
    }
}

fn run(cli: Cli) -> Result<()> {
    let config_path = &cli.config;
    let key_gen = WgKeyGenerator;
    if let Commands::Init {
        endpoint_ip,
        endpoint_port,
        client_dns,
        server_net,
        external_interface,
        wg_interface,
    } = cli.command
    {
        return handle_init(
            config_path,
            endpoint_ip,
            endpoint_port,
            client_dns,
            server_net,
            external_interface,
            wg_interface,
            &key_gen,
        );
    }
    let config = Config::open(config_path)?;
    let mut nazuna = Nazuna {
        key_gen,
        config,
        config_path,
    };
    match cli.command {
        Commands::List => {
            nazuna.handle_list();
            Ok(())
        }
        Commands::Add { name } => nazuna.handle_add(&name),
        Commands::Remove { name } => nazuna.handle_remove(&name),
        Commands::Cat { name } => nazuna.handle_cat(&name),
        Commands::Update => nazuna.handle_update(),
        Commands::Start => nazuna.handle_start(),
        Commands::Stop => nazuna.handle_stop(),
        Commands::Init { .. } => Ok(()),
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_init<G: KeyGenerator>(
    path: &std::path::Path,
    endpoint_ip: std::net::Ipv4Addr,
    endpoint_port: u16,
    client_dns: Option<std::net::Ipv4Addr>,
    server_net: Ipv4Net,
    external_interface: String,
    wg_interface: String,
    key_gen: &G,
) -> Result<()> {
    if path.exists() {
        println!("⚠️  Database already exists at {}", path.display());
        return Ok(());
    }
    let config = Config::try_new(
        endpoint_ip,
        endpoint_port,
        client_dns,
        server_net,
        external_interface,
        wg_interface,
        key_gen,
    )?;
    config.save(path)?;
    println!(
        "✅ Initialized database at {} with defaults or environment parameters.",
        path.display()
    );
    Ok(())
}
