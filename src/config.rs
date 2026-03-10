use anyhow::{Context, Result, anyhow};
use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};
use std::{io::Write, net::Ipv4Addr, path::Path};

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
    pub server_net: Ipv4Net,
    pub client_dns: Option<Ipv4Addr>,
    pub endpoint_ip: Ipv4Addr,
    pub endpoint_port: u16,
    pub external_interface: String,
    pub wg_interface: String,
}

impl Config {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open database file {}", path.display()))?;
        serde_json::from_reader(file).context("Failed to parse database JSON")
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let file = std::fs::File::create(path)
            .with_context(|| format!("Failed to create database file {}", path.display()))?;
        serde_json::to_writer_pretty(file, self).context("Failed to serialize database to JSON")
    }

    pub fn find_available_ip(&self, net: Ipv4Net) -> Result<Ipv4Addr> {
        let server_ip = net.addr();
        net.hosts()
            .find(|ip| *ip != server_ip && !self.users.iter().any(|u| u.ip == *ip))
            .ok_or_else(|| anyhow!("No available IP addresses in subnet {net}"))
    }

    pub fn write_wg_conf<W: Write>(&self, w: &mut W, is_full: bool) -> Result<()> {
        writeln!(w, "[Interface]")?;
        if is_full {
            writeln!(w, "Address = {}", self.server_net)?;
            writeln!(w, "SaveConfig = false")?;
        }
        writeln!(w, "PrivateKey = {}", self.server_priv_key)?;
        writeln!(w, "ListenPort = {}", self.endpoint_port)?;
        if is_full {
            writeln!(w, "PreUp = sysctl -w net.ipv4.ip_forward=1")?;
            writeln!(
                w,
                "PostUp = iptables -A FORWARD -i {} -j ACCEPT; iptables -t nat -A POSTROUTING -o {} -j MASQUERADE",
                self.wg_interface, self.external_interface
            )?;
            writeln!(
                w,
                "PostDown = iptables -D FORWARD -i {} -j ACCEPT; iptables -t nat -D POSTROUTING -o {} -j MASQUERADE",
                self.wg_interface, self.external_interface
            )?;
        }
        for user in &self.users {
            writeln!(w, "\n[Peer]")?;
            writeln!(w, "# Name: {}", user.name)?;
            writeln!(w, "PublicKey = {}", user.pub_key)?;
            writeln!(w, "AllowedIPs = {}/32", user.ip)?;
        }
        Ok(())
    }

    pub fn write_client_conf<W: Write>(&self, w: &mut W, user: &User) -> Result<()> {
        writeln!(w, "[Interface]")?;
        writeln!(w, "PrivateKey = {}", user.priv_key)?;
        writeln!(w, "Address = {}/32", user.ip)?;
        if let Some(dns) = self.client_dns {
            writeln!(w, "DNS = {}", dns)?;
        }
        writeln!(w, "[Peer]")?;
        writeln!(w, "PublicKey = {}", self.server_pub_key)?;
        writeln!(w, "Endpoint = {}:{}", self.endpoint_ip, self.endpoint_port)?;
        writeln!(w, "AllowedIPs = 0.0.0.0/0")?;
        writeln!(w, "PersistentKeepalive = 25")?;
        Ok(())
    }
}
