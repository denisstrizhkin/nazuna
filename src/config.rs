use anyhow::{Context, Result, anyhow};
use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::Ipv4Addr;

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub name: String,
    pub ip: Ipv4Addr,
    pub priv_key: String,
    pub pub_key: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub users: Vec<User>,
    pub server_pub_key: String,
    pub server_priv_key: String,
    pub endpoint: String,
    pub server_net: Ipv4Net,
    pub external_interface: String,
    pub wg_interface: String,
}

impl Config {
    pub fn load(path: &std::path::Path) -> Result<Self> {
        if !path.exists() {
            return Err(anyhow!(
                "❌ Database not found at {}. Please run 'init' first.",
                path.display()
            ));
        }
        let data = fs::read_to_string(path)
            .with_context(|| format!("Failed to read database file {}", path.display()))?;
        serde_json::from_str(&data)
            .with_context(|| format!("Failed to parse database JSON from {}", path.display()))
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }
        let data =
            serde_json::to_string_pretty(self).context("Failed to serialize database to JSON")?;
        fs::write(path, data)
            .with_context(|| format!("Failed to write database file to {}", path.display()))
    }

    pub fn find_available_ip(&self, net: Ipv4Net) -> Result<Ipv4Addr> {
        let server_ip = net.addr();
        net.hosts()
            .find(|ip| *ip != server_ip && !self.users.iter().any(|u| u.ip == *ip))
            .ok_or_else(|| anyhow!("No available IP addresses in subnet {net}"))
    }
}
