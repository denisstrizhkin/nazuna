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
```bash
# Using flags (recommended)
sudo cargo run -- init --server-net 10.50.0.1/24 --endpoint-ip 1.2.3.4 --endpoint-port 51820

# Using a custom database path
sudo cargo run -- -c ./my_vpn.json init
```

### 2. Managing Peers
Adding a peer is instant. No configuration files are written to disk yet; only the database is updated.
```bash
cargo run -- add "denis-laptop"
# ✅ User 'denis-laptop' added with IP 10.50.0.2

cargo run -- list
# 📋 Registered Peers:
# Name                 | IP              | Public Key                                  
# -------------------------------------------------------------------------------------
# denis-laptop         | 10.50.0.2       | yN9p9KtziHtGkZ2OIrwkfn/zZWBqMu8ObI9LavENBw8=
```

### 3. Deploying to System
Synchronize the database state with the actual WireGuard interface (`wg0`).
```bash
# This generates server.conf and syncs it to /etc/wireguard/wg0.conf
cargo run -- update
```

### 4. Client Handover
Retrieve the complete client configuration for a specific user to stdout.
```bash
cargo run -- cat "denis-laptop" > denis.conf
```

### 5. Service Control
Quick wrappers for interface management.
```bash
cargo run -- start  # wg-quick up wg0
cargo run -- stop   # wg-quick down wg0
```

## 🏗️ Technical Architecture

- **Subnet Management**: Uses CIDR parsing to ensure no IP collisions. It skips the network, broadcast, and the server's own IP during allocation.
- **Key Generation**: Directly interfaces with the `wg` binary for cryptographically secure key generation.
- **Safety**: Uses atomic-like patterns for database updates; if config generation fails, the system state remains untouched.

## 📄 License

MIT. See `LICENSE` for details.
