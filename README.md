# Tari Messenger

A decentralized, on-chain messaging app built on the [Tari](https://tari.com) Layer 2 network. Every message is a blockchain transaction signed with your private key. No central server. No phone number. No account registration.

> **Community project** — built by a community developer as a demo of what's possible on Tari Ootle. Not an official Tari protocol product.

---

## How It Works

The app has two parts:

- **`messaging_template/`** — A Rust smart contract compiled to WASM and deployed on the Tari DAN (Layer 2). Stores all messages on-chain.
- **`messaging_app/`** — A local Axum web server that manages your wallets and talks to the Tari network. Serves a WhatsApp-style UI at `http://localhost:3000`.

You run everything locally. Your private keys never leave your machine.

```
Your browser
    ↕  localhost:3000
Local Axum server  (your node — holds your keys)
    ↕  HTTPS
Tari DAN validators  (decentralized — hold the on-chain state)
```

---

## Features

- **Public Chats** — Join the community "Tari Messenger Test Chat" open room, pre-loaded on launch
- **Direct messages** — Send DMs to anyone by their public key
- **End-to-end encryption** — Ristretto ECDH + AES-256-GCM for DMs (toggle in wallet panel)
- **Group rooms** — Create and join group chats (always plaintext — ECDH is peer-to-peer only)
- **Multi-wallet** — Manage multiple identities in one app
- **On-chain identity** — Your Ristretto public key is your address, no signup required
- **Contact nicknames** — Assign names to contacts by their public key
- **Demo mode** — Spin up two wallets locally to test without a second device
- **Mnemonic backup** — Export and import wallets via BIP-39 seed words
- **Deploy your own contract** — One-click WASM auto-publish for a private messaging channel

---

## Building

### Prerequisites

All platforms need the Rust toolchain and the WASM compile target:

```bash
# 1. Install Rust (all platforms)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Add the WASM target (one-time)
rustup target add wasm32-unknown-unknown
```

---

### Windows

**One-click (recommended):**
```
Double-click launch.bat
```
Builds the WASM contract if needed, then starts the app. Open `http://localhost:3000`.

**Manual:**
```bat
cd messaging_template
cargo build --target wasm32-unknown-unknown --release
cd ..\messaging_app
cargo run
```

**Dev mode — two clients on one machine:**
```
Double-click launch-dev.bat
```
Launches two independent app instances on ports 3000 and 3001 for testing two-way messaging locally.

---

### Linux

```bash
# Clone and enter the repo
git clone https://github.com/YOUR_NAME/tari-messenger.git
cd tari-messenger

# Build the WASM smart contract
cd messaging_template
cargo build --target wasm32-unknown-unknown --release
cd ..

# Start the app
cd messaging_app
cargo run
```

Open `http://localhost:3000` in your browser.

**Dev mode (two clients):**
```bash
# Terminal 1
cd messaging_app
cargo run -- --port 3000 --state messaging-state-a.json

# Terminal 2
cd messaging_app
cargo run -- --port 3001 --state messaging-state-b.json
```

**Debian/Ubuntu — if Rust isn't installed yet:**
```bash
sudo apt update && sudo apt install -y build-essential curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup target add wasm32-unknown-unknown
```

---

### macOS

```bash
# Clone and enter the repo
git clone https://github.com/YOUR_NAME/tari-messenger.git
cd tari-messenger

# Build the WASM smart contract
cd messaging_template
cargo build --target wasm32-unknown-unknown --release
cd ..

# Start the app
cd messaging_app
cargo run
```

Open `http://localhost:3000` in your browser.

**Dev mode (two clients):**
```bash
# Terminal 1
cd messaging_app
cargo run -- --port 3000 --state messaging-state-a.json

# Terminal 2
cd messaging_app
cargo run -- --port 3001 --state messaging-state-b.json
```

**If Rust isn't installed yet:**
```bash
# Install Xcode command-line tools first (if prompted)
xcode-select --install

# Then install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup target add wasm32-unknown-unknown
```

> **Apple Silicon (M1/M2/M3):** No extra steps needed. The Rust toolchain and WASM target work natively on `aarch64-apple-darwin`.

---

## First-Run Setup

On first launch, a welcome screen lets you choose:

| Path | When to use |
|------|------------|
| **Quick Start** | Join the community test chat in ~60 seconds. Auto-generates a wallet, funds from faucet, joins the public room. |
| **Advanced Setup** | Deploy your own private messaging contract. Share the address only with contacts you trust. |

### Publishing Your Own Template

1. Click **Settings** in the sidebar → **Auto-Publish WASM** — the app compiles and deploys the contract automatically (~60s)
2. Or publish manually via the Tari Wallet Web UI at `http://127.0.0.1:5100` -> Publish Template
3. Share your component address with contacts so they can join via **Settings -> Join Existing**

---

## Privacy Model

| Property | Status |
|----------|--------|
| No central server | Yes — Tari DAN validators |
| No KYC / phone number | Yes — key pair only |
| No account registration | Yes |
| Sender authenticated | Yes — Ristretto signature, cannot be spoofed |
| DM content private | Optional — E2EE via Ristretto ECDH + AES-256-GCM (toggle in wallet panel) |
| Group chat content | Always plaintext — ECDH requires exactly two parties |
| Metadata (who/when) | Always public on-chain — sender/recipient keys and timestamps visible to anyone with the contract address |
| Contract address = privacy boundary | Yes — only users with your contract address can see your messages |

### How DM Encryption Works

When E2EE is enabled, each DM is encrypted before being stored on-chain:

1. **ECDH Key Agreement** — `shared_point = recipient_pubkey x sender_secretkey` (Ristretto255)
2. **Key Derivation** — `HKDF-SHA256(shared_point, info=sorted_pubkeys)` -> 256-bit key
3. **Encryption** — AES-256-GCM with a random 12-byte nonce per message
4. **Wire format** — `ENC1:<hex(nonce || ciphertext)>`

Even though ciphertext is on a public blockchain, only the sender and recipient can decrypt it.

---

## Architecture

### Network
- **Tari Esmeralda** — public testnet
- **Indexer** — `https://ootle-indexer-a.tari.com/`
- **Native token** — tTARI (testnet, free from faucet)

### Smart Contract (`messaging_template/`)
Rust WASM template deployed to Tari Ootle. Stores messages in parallel `Vec<String>` fields:
- `dm_from`, `dm_to`, `dm_content` — direct messages
- `room_ids`, `room_names`, `room_creators` — group room definitions
- `room_msg_room`, `room_msg_from`, `room_msg_content` — room messages

### Local App (`messaging_app/`)
- Wallet key management (Ristretto keypairs, BIP-39 mnemonics)
- Submits transactions to the Tari network (~2000 uTARI fee per message)
- Polls on-chain state every ~10 seconds to sync messages from all clients
- Persists state locally in `messaging-state.json` (excluded from git — contains private keys)

---

## Project Structure

```
ootle/
├── README.md
├── PROGRESS.md                     <- Build log and dev notes
├── .gitignore
├── launch.bat                      <- One-click launcher (Windows)
├── launch-dev.bat                  <- Two-client dev mode (Windows)
├── messaging_template/             <- Rust WASM smart contract
│   ├── Cargo.toml
│   └── src/lib.rs
└── messaging_app/                  <- Local Axum web server + UI
    ├── Cargo.toml
    ├── Cargo.lock
    ├── src/
    │   ├── main.rs                 <- Server, wallet management, API handlers
    │   └── want_list.rs            <- Tari input resolution helper
    └── static/
        ├── index.html              <- WhatsApp-style frontend
        └── instructions.html       <- How-it-works guide
```

---

## Crate Versions

| Crate | Version | Purpose |
|-------|---------|---------|
| `ootle-rs` | 0.1 | Tari wallet client |
| `tari_ootle_transaction` | 0.25 | Transaction builder |
| `tari_ootle_common_types` | 0.25 | Network types |
| `tari_template_lib` | 0.20 | WASM template library |
| `tari_template_lib_types` | 0.20 | Shared types |
| `tari_crypto` | 0.22 | Ristretto cryptography |

---

## License

MIT
