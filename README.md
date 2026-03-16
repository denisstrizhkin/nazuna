# Nazuna 🩸

**The no-nonsense WireGuard VPN manager.**

Stop wrestling with config files, manual IP allocation, and key rotation. Nazuna automates the tedious parts of running a WireGuard server so you can focus on what matters.

## Why Nazuna?

- **Zero Config Headaches:** Add users with a single command. Keys and IPs are auto-generated.
- **JSON-Backed:** Your entire state lives in one readable JSON file. No heavy databases to manage.
- **Portable:** Single binary. Easy to backup, easy to move.
- **Instant Client Configs:** Generate QR-ready configs instantly.

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

## OpenRC Service (Gentoo/Alpine)

Nazuna includes an OpenRC init script to manage your VPN as a background service.

1. Copy the init script:
```bash
sudo cp openrc/nazuna /etc/init.d/nazuna
```

2. Start the service:
```bash
sudo rc-service nazuna start
```

3. Enable on boot:
```bash
sudo rc-update add nazuna default
```

## Systemd Service (Debian/Ubuntu/Arch)

Nazuna includes a systemd service file to manage your VPN as a background service.

1. Ensure `nazuna` is accessible in your system path (e.g. symlink to `/usr/local/bin`):
```bash
sudo ln -s ~/.cargo/bin/nazuna /usr/local/bin/nazuna
```

2. Copy the service file:
```bash
sudo cp systemd/nazuna.service /etc/systemd/system/
```

3. Reload systemd and start the service:
```bash
sudo systemctl daemon-reload
sudo systemctl start nazuna
```

4. Enable on boot:
```bash
sudo systemctl enable nazuna
```

## License

MIT. See `LICENSE` for details.
