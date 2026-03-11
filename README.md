# Nazuna 🩸

Nazuna is a high-performance, minimalist, and purely data-driven management tool for WireGuard. It is designed for administrators who value simplicity, idempotency, and the "Infrastructure as Code" philosophy.

Unlike traditional management tools that create a mess of directories and files, Nazuna maintains the entire system state in a single, authoritative JSON database and generates configurations dynamically.

## 🚀 Core Features

- **Unified State**: Your entire peer list and server identity live in a single config file (default: `/etc/nazuna/nazuna.conf`).
- **Intelligent Networking**: Automatic IP allocation using `ipnet`. Just define your subnet, and Nazuna handles the math.
- **Stateless Generation**: Server and client configurations are generated on-the-fly from the database.
- **Robust Error Handling**: Powered by `anyhow` with deep contextual diagnostics for every failure point.
- **Senior Rust Standards**: Clean, DRY, and idiomatic codebase.
- **Pleasant CLI UX**: Enjoy descriptive success/error prompts decorated with emojis and nicely formatted tables.

## 📋 Prerequisites

- **Rust**: Stable toolchain (2024 edition).
- **WireGuard Tools**: The `wg` and `wg-quick` binaries must be in your `$PATH`.
- **Permissions**: System configuration updates (`update`, `start`, `stop`) usually require `sudo`.

## 📦 Installation

To install `nazuna` directly from this repository via Git, run:
```bash
cargo install --git https://github.com/denisstrizhkin/nazuna.git
```
*Note: Ensure `~/.cargo/bin` is in your `$PATH`.*

## ⚙️ Configuration (Initial Setup)

Nazuna is configured through CLI flags **during initialization**. Once initialized, these parameters are stored in the database and are no longer required.

| Flag | Environment Variable | Description | Default / Example |
|------------|-----------------------|-------------|-------------------|
| `--server-net` | `WG_SERVER_NET` | Internal VPN IP and subnet. | `10.50.0.1/24` |
| `--endpoint-ip` | `WG_ENDPOINT_IP` | Public IP for clients. | `127.0.0.1` |
| `--endpoint-port` | `WG_ENDPOINT_PORT` | Public port for clients. | `51820` |
| `--client-dns` | `WG_CLIENT_DNS` | DNS server for clients. | *None* |
| `--external-interface` | `WG_EXTERNAL_INTERFACE` | External WAN interface for NAT. | `eth0` |
| `--wg-interface` | `WG_INTERFACE` | WireGuard interface name. | `wg0` |
| `-c`, `--config` | *N/A* | Path to the JSON database. | `/etc/nazuna/nazuna.conf` |
| *N/A* | `RUST_LOG` | Logging level. | `info` |

## 🛠️ Usage

### 1. Initialization
Generate the server's identity and the initial database. Use flags, environment variables, or rely on reasonable defaults.

**Using flags (recommended):**
```bash
sudo nazuna init --server-net 10.50.0.1/24 --endpoint-ip 1.2.3.4 --endpoint-port 51820
```

**Using a custom database path:**
```bash
sudo nazuna -c ./my_vpn.json init
```

### 2. Managing Peers
Adding a peer is instant. No configuration files are written to disk yet; only the database is updated.

```bash
nazuna add denis-mac
```

**Output:**
```text
✅ User 'denis-mac' added with IP 10.50.0.2
```

To list all registered peers:

```bash
nazuna list
```

**Output:**
```text
📋 Registered Peers:
Name                 | IP              | Public Key                                  
-------------------------------------------------------------------------------------
denis-mac            | 10.50.0.2       | OvMjcbm+yt7hPXe/ZCWeDiptm3R5v6g+HJq68pYyyGE=
```

To remove a peer (example):

```bash
nazuna remove denis-mac
```

**Output:**
```text
🗑️  User 'denis-mac' removed.
```

### 3. Deploying to System
Synchronize the database state with the actual WireGuard interface (`wg0`).

```bash
sudo nazuna update
```

**Output:**
```text
🚀 System WireGuard configuration updated successfully.
```

### 4. Client Handover
Retrieve the complete client configuration for a specific user to stdout.

```bash
nazuna cat denis-mac
```

**Output:**
```text
[Interface]
PrivateKey = AGx8yPyvFEncoQcnr9mjXp8Wl0sID7XhXAMvLOcGZmw=
Address = 10.50.0.2/32
[Peer]
PublicKey = XwdBWpmrAdV+vKhgy7KCkc3Y4l/EFZVDzY6wCNL/OVU=
Endpoint = 127.0.0.1:51820
AllowedIPs = 0.0.0.0/0
PersistentKeepalive = 25
```

Save it directly to a file:

```bash
nazuna cat denis-mac > denis-mac.conf
```

### 5. Service Control
Quick wrappers for interface management.

**Start the interface:**
```bash
sudo nazuna start
```

**Stop the interface:**
```bash
sudo nazuna stop
```

## 🏗️ Technical Architecture

- **Subnet Management**: Uses CIDR parsing to ensure no IP collisions. It skips the network, broadcast, and the server's own IP during allocation.
- **Key Generation**: Directly interfaces with the `wg` binary for cryptographically secure key generation.
- **Safety**: Uses atomic-like patterns for database updates; if config generation fails, the system state remains untouched.

## 📄 License

MIT. See `LICENSE` for details.
