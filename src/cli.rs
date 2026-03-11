use clap::{Parser, Subcommand};
use ipnet::Ipv4Net;
use std::{net::Ipv4Addr, path::PathBuf};

#[derive(Parser)]
#[command(name = "nazuna", version, about = "A minimalist, purely data-driven management tool for WireGuard 🩸", long_about = None)]
pub struct Cli {
    /// Path to the database file
    #[arg(short, long, default_value = "/etc/nazuna/nazuna.conf")]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize the server database and generate keys
    Init {
        #[arg(long, env = "WG_ENDPOINT_IP", default_value = "127.0.0.1")]
        endpoint_ip: Ipv4Addr,
        #[arg(long, env = "WG_ENDPOINT_PORT", default_value_t = 51820)]
        endpoint_port: u16,
        #[arg(long, env = "WG_CLIENT_DNS")]
        client_dns: Option<Ipv4Addr>,
        #[arg(long, env = "WG_SERVER_NET", default_value = "10.50.0.1/24")]
        server_net: Ipv4Net,
        #[arg(long, env = "WG_EXTERNAL_INTERFACE", default_value = "eth0")]
        external_interface: String,
        #[arg(long, env = "WG_INTERFACE", default_value = "wg0")]
        wg_interface: String,
    },
    /// List all registered peers
    List,
    /// Add a new peer (e.g. nazuna add "denis-laptop")
    Add { name: String },
    /// Remove an existing peer
    Remove { name: String },
    /// Print the `WireGuard` client configuration for a specific peer
    Cat { name: String },
    /// Sync the database state with the `WireGuard` interface (generates config)
    Update,
    /// Start the `WireGuard` interface
    Start,
    /// Stop the `WireGuard` interface
    Stop,
}

pub fn parse() -> Cli {
    Cli::parse()
}
