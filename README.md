# Nazuna 🎏

Nazuna is a high-performance, minimalist, and purely data-driven management tool for WireGuard. It is designed for administrators who value simplicity, idempotency, and the "Infrastructure as Code" philosophy.

Unlike traditional management tools that create a mess of directories and files, Nazuna maintains the entire system state in a single, authoritative JSON database and generates configurations dynamically.

## 🚀 Core Features

- **Unified State**: Your entire peer list and server identity live in `users.json`.
- **Intelligent Networking**: Automatic IP allocation using `ipnet`. Just define your subnet, and Nazuna handles the math.
- **Stateless Generation**: Server and client configurations are generated on-the-fly from the database.
- **Robust Error Handling**: Powered by `anyhow` with deep contextual diagnostics for every failure point.
- **Senior Rust Standards**: Clean, DRY, and idiomatic codebase.

## 📋 Prerequisites

- **Rust**: Stable toolchain (2024 edition).
- **WireGuard Tools**: The `wg` and `wg-quick` binaries must be in your `$PATH`.
- **Permissions**: System configuration updates (`update`, `start`, `stop`) usually require `sudo`.

## ⚙️ Configuration

Nazuna is configured through environment variables to remain portable and secure.

| Variable | Description | Example |
|----------|-------------|---------|
| `WG_SERVER_IP` | **Required**. The internal VPN IP and subnet mask. | `10.50.0.1/24` |
| `WG_ENDPOINT` | **Required**. The public IP/DNS and port clients connect to. | `vpn.example.com:51820` |

## 🛠️ Usage

### 1. Initialization
Generate the server's identity and the initial database.
```bash
export WG_SERVER_IP=10.50.0.1/24
export WG_ENDPOINT=vpn.example.com:51820
cargo run -- init
```

### 2. Managing Peers
Adding a peer is instant. No configuration files are written to disk yet; only the database is updated.
```bash
cargo run -- add "denis-laptop"
cargo run -- list
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
