# Nazuna 🩸

Nazuna is a WireGuard VPN management tool for **Linux server administrators**. Its job is simple: you run one command, and a new VPN user is created and ready to connect — no manual config editing required.

## Who is this for?

Server admins who run a WireGuard VPN and want to manage users without touching config files manually. If you've ever copy-pasted keys and IP addresses by hand, Nazuna does all of that for you.

## How it works

Nazuna keeps your entire user list in a single JSON file. When you add a user, it automatically assigns them an IP, generates their keys, and produces a ready-to-send client config — all in seconds.

## Installation

From crates.io (recommended):
```bash
cargo install nazuna
```

Or from the GitHub repository:
```bash
cargo install --git https://github.com/denisstrizhkin/nazuna.git
```

> Requires Rust (stable), and `wg` / `wg-quick` in your `$PATH`.

## Quick Start

**1. Initialize the server** (once, on first setup):
```bash
sudo nazuna init --server-net 10.50.0.1/24 --endpoint-ip 1.2.3.4 --endpoint-port 51820
```

**2. Add a user:**
```bash
nazuna add alice
# ✅ User 'alice' added with IP 10.50.0.2
```

**3. Apply changes to the live interface:**
```bash
sudo nazuna update
```

**4. Get the client config to send to your user:**
```bash
nazuna cat alice
# Outputs a complete WireGuard config they can import directly
```

## Other Commands

```bash
nazuna list          # Show all users and their IPs
nazuna remove alice  # Delete a user
sudo nazuna start    # Bring up the WireGuard interface
sudo nazuna stop     # Bring it down
```

## License

MIT. See `LICENSE` for details.
