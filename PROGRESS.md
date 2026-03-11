# Tari Ootle — Messenger App — Build Progress

## ⚠️ CLAUDE CODE INSTRUCTIONS — READ BEFORE DOING ANYTHING
**Always invoke the `/tari-ootle` skill at the start of every session before writing any code.**
This loads the full Tari Ootle API reference (templates, transactions, wallet, ootle-rs patterns).
Without it you will hallucinate APIs. Type `/tari-ootle` or call `Skill("tari-ootle")` first.

## STATUS: ✅ Public Chats tab + info modal shipped (2026-03-10)

> 📋 **See `PLAN.md`** for detailed session notes, feature status table, indexer JSON format,
> and potential next steps. Update that file when continuing work.

### Features Completed This Session (2026-03-10) — Public Chats UX
- **Public Chats tab** — renamed sidebar tab from "Public" → "🌐 Public Chats"
- **Public Chats Info Modal** (`#pubinfo-modal`) — 4 tabs:
  - **About**: Community app disclaimer (not official Tari), what the Test Chat is, testnet, permanence
  - **Privacy**: What's always on-chain (to/from keys, timestamps, content by default), metadata can't be hidden, group rooms always plaintext
  - **Encryption**: Full E2EE walkthrough — ECDH key agreement, HKDF-SHA256 derivation, AES-256-GCM with nonce, wire format `ENC1:...`, limitations
  - **Your Own**: How to deploy a private contract, simple (Auto-Publish) vs advanced (custom Rust template), sharing addresses
- **Auto-shows info modal** on first Public Chats tab visit (localStorage gate: `tari_public_seen`)
- **ℹ buttons** in public list info bar and room card — click to open info modal
- **Enhanced public list** — better room card copy, disclaimer wording, "Create Your Own Contract" CTA card at bottom
- **Onboarding Step 2 updated** — mentions "not an official Tari channel", privacy heads-up, link to info modal
- **pub-header-bar updated** — "How it works ℹ" link opens info modal instead of onboarding modal
- **Backend**: `handle_public_config()` updated — single room "Tari Messenger Test Chat" with community description (removed old "Tari Testnet General" / "Tari Developer Chat" rooms)

### Previous Session (2026-03-10) — Wallet Balance fix
- **Wallet Balance fixed** — `query_balance_micro_tari` now correctly parses vault IDs from
  CBOR Tag[132,Bytes] format and handles string-encoded amounts (`"revealed_amount":"9889138"`)
- All 4 features from "Next Session" section below were already implemented; balance was the only bug
- Backups saved: `main.rs.bak`, `index.html.bak`

---

## BREAKING CHANGE — Migration Required

If you have an existing `messaging-state.json`, **delete it** before running the new version.
The state schema changed (removed Ootle/Minotari, added multi-wallet HashMap + DMs + rooms).

Also **rebuild the WASM template** and **re-publish** it — new methods were added.

```bash
cd messaging_template
cargo build --target wasm32-unknown-unknown --release
# Then re-publish via the Wallet Web UI at http://127.0.0.1:5100 or use Auto-Publish
```

---

## Project Structure

```
ootle/
├── PROGRESS.md                     ← You are here
├── messaging_template/             ← Rust WASM smart contract
│   ├── Cargo.toml
│   └── src/lib.rs                  ← DMs + group rooms (parallel Vec<String> fields)
└── messaging_app/                  ← Axum web server + UI
    ├── Cargo.toml
    ├── src/
    │   ├── main.rs                 ← Multi-wallet, DM, group chat, wallet import
    │   └── want_list.rs            ← Helper for Tari input resolution
    └── static/
        ├── index.html              ← WhatsApp-like UI (sidebar + wallet mgmt + modals)
        └── instructions.html       ← Styled how-it-works page
```

---

## Quick Start

### 0. Files in this directory
```
ootle/
├── tari_ootle_walletd-0.25.8-1e047bb-windows-x64.exe  ← Tari wallet daemon
├── launch.bat                                           ← One-click app launcher
├── PROGRESS.md                                          ← You are here
├── messaging_template/                                  ← WASM smart contract
└── messaging_app/                                       ← Axum web server + UI
```

### 1. Install prerequisites (one-time)
```bash
rustup target add wasm32-unknown-unknown
```

### 2. (Optional) Launch the Tari Wallet Daemon
The wallet daemon provides the official Tari Web UI at http://127.0.0.1:5100.
Use it to manually publish templates, check on-chain balances, and manage seed words.
```bash
# From the ootle/ directory (PowerShell or cmd):
.\tari_ootle_walletd-0.25.8-1e047bb-windows-x64.exe --network esme

# Then open http://127.0.0.1:5100 in your browser
```
> The walletd is NOT required to run the messaging app — the app has its own built-in wallet.
> Use walletd if you want to: publish templates via the Web UI, check seed words, or manage
> accounts separately from the messaging app.

#### Useful walletd subcommands:
```bash
# Get seed words for current wallet (backup!)
.\tari_ootle_walletd-0.25.8-1e047bb-windows-x64.exe --network esme seed-words

# Restore wallet from seed words
.\tari_ootle_walletd-0.25.8-1e047bb-windows-x64.exe --network esme --seed-words "word1 word2 ... word24"

# Generate a new key (outputs public key hex)
.\tari_ootle_walletd-0.25.8-1e047bb-windows-x64.exe --network esme create-account
```
> ⚠️ walletd seed words are NOT compatible with the messaging app's hex key import.
> They use different key derivation schemes. Keep them separate.

### 3. Delete old state (if migrating)
```bash
# In messaging_app/ directory:
del messaging-state.json
```

### 4. Build the messaging template
```bash
cd messaging_template
cargo build --target wasm32-unknown-unknown --release
```

### 5. Start the web server (or just double-click launch.bat)
```bash
cd messaging_app
cargo run
# Open http://localhost:3000
```

### 6. First-time setup in the browser
1. Click **Add Wallet** (sidebar banner) to create or import a wallet
2. Click **Configure** to publish the template and deploy the component
3. Start chatting — **New DM** or **New Group** buttons in the sidebar

---

## New Architecture (v2)

### What Changed

| Feature | Before | After |
|---------|--------|-------|
| Users | Hardcoded Ootle + Minotari | Any number of wallets via HashMap |
| Conversations | Single shared chat | Direct messages between any two users |
| Groups | None | Group rooms with unique room_id |
| Wallet management | Auto-created | User creates/imports in browser |
| Wallet import | N/A | Generate, import hex keys, import passphrase |
| Template | `send_message`, `get_conversation_raw` | `send_dm`, `create_room`, `post_to_room`, + more |

### Template Methods (messaging_template/src/lib.rs)

```rust
// Constructor
fn new() -> Component<Self>

// Direct Messages
fn send_dm(to: String, content: String)
fn get_dm_conversation(user_a: String, user_b: String) -> Vec<String>  // triples: [from, to, content]

// Group Rooms
fn create_room(room_id: String, display_name: String)
fn post_to_room(room_id: String, content: String)
fn get_room_messages(room_id: String) -> Vec<String>  // pairs: [from, content]
fn list_rooms() -> Vec<String>  // triples: [id, name, creator]

// Stats
fn dm_count() -> u64
fn room_message_count() -> u64
```

### API Endpoints (messaging_app/src/main.rs)

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/status` | GET | Setup status, wallet count |
| `/api/wallets` | GET | List wallets (no secrets) |
| `/api/wallet/create` | POST | Generate + faucet fund (30-60s) |
| `/api/wallet/import` | POST | Import by hex keys |
| `/api/wallet/passphrase` | POST | Import by passphrase (SHA-256 derived) |
| `/api/wallet/faucet` | POST | Fund existing wallet from faucet |
| `/api/template/configure` | POST | Set template address |
| `/api/template/publish` | POST | Auto-publish WASM + deploy component |
| `/api/dm/send` | POST | Send DM (from_pubkey, to_pubkey, content) |
| `/api/dm/messages` | GET | Get DM conversation (?user_a=...&user_b=...) |
| `/api/room/create` | POST | Create group room on-chain |
| `/api/room/join` | POST | Join room locally (no on-chain tx) |
| `/api/room/post` | POST | Post to group room |
| `/api/room/messages` | GET | Get room messages (?room_id=...) |
| `/api/rooms` | GET | List all known rooms |
| `/api/debug` | GET | Debug info: fees, tx history, wallet details |

### State File (messaging-state.json)

```json
{
  "wallets": {
    "<pubkey_hex>": {
      "display_name": "Ootle",
      "account_secret_hex": "...",
      "view_secret_hex": "...",
      "account_address": "component_...",
      "public_key_hex": "..."
    }
  },
  "template_address": "template_...",
  "component_address": "component_...",
  "dms": [{"from_pk":"...", "to_pk":"...", "content":"...", "timestamp":123}],
  "room_msgs": [{"room_id":"...", "from_pk":"...", "content":"...", "timestamp":123}],
  "rooms": [{"room_id":"...", "display_name":"...", "creator_pk":"..."}],
  "tx_history": [...],
  "setup_status": "Ready"
}
```

---

## Wallet Import Methods

### Generate (recommended for new users)
- Creates random Ristretto keypair
- Funds 10 tTARI from Esmeralda faucet
- Takes ~30-60 seconds

### Import by Hex Keys
- Paste `account_secret_hex` (64 hex chars) + `view_secret_hex` (64 hex chars)
- No faucet funding — wallet must already have tTARI
- Use `/api/wallet/faucet` endpoint or "Get Testnet Funds" in UI if needed

### Import by Passphrase
- Derives keys via SHA-256 with domain prefixes
- Same passphrase → same wallet (deterministic)
- **NOT** compatible with `walletd` seed words (different derivation scheme)
- Domain prefixes: `tari_ootle_account_key_v1:` and `tari_ootle_view_key_v1:`
- Top 4 bits masked to ensure scalar is < group order

---

## Key Technical Notes

- **Template macro limitation**: `Vec<CustomStruct>` crashes `#[template]` — must use parallel `Vec<String>`
- **IndexerProvider is `!Send`**: All blockchain ops must run in `std::thread::spawn` + `new_current_thread` runtime, or via `tokio::task::spawn_blocking`
- **Rust >= 1.90.0** required for ootle-rs
- **Fees**: Faucet=500µTARI, publish=~250,000µTARI, deploy/DM/room=2,000µTARI each

---

## Network Config

- **Network**: Esmeralda testnet
- **Indexer URL**: `https://ootle-indexer-a.tari.com/`
- **Web UI**: `http://localhost:3000`
- **Wallet Web UI**: `http://127.0.0.1:5100` (for manual template publishing)

---

## Troubleshooting

### Faucet fails
- Esmeralda faucet may be temporarily down
- Check network connectivity
- Try again in a few minutes

### "Transaction rejected"
- Fee might be too low — increase the fee constants in main.rs
- Network may be slow — wait and retry

### "Room not found" on post
- The room must be created first (on-chain)
- Wait for the `create_room` transaction to confirm (~30s)

### Messages appear in UI but were just sent
- Local cache updates immediately
- Blockchain transaction confirms in the background (~30s)
- Data shown is from local cache, not queried from chain (by design for speed)

---

---

## Multi-User / Sharing Architecture — Known Limitation

The current app is **single-server, single-component**. Each server instance deploys its own on-chain component. Two people running separate servers **cannot DM each other** because their components are different.

### The fix (not yet built): "Join existing component" flow
Add a way for a user to connect to an already-deployed component by pasting its address, instead of always deploying a new one. This would allow:
- Server operator deploys the component once
- Others visit the same hosted URL (or paste the component address) and join the same component
- DMs cross between all users on the same component

### Next feature to build
Add a `POST /api/component/join` endpoint + UI flow that:
1. Accepts a component address + template address from the user
2. Sets `component_address` in state without running a deploy transaction
3. Updates `setup_status` to `Ready`
4. Lets the user send/receive messages on the shared component

### Also noted
- User has an older deployed component (from a version that may have had minting in `new()`). Current `new()` has no minting. If re-publishing + re-deploying, delete `messaging-state.json` first.

---

---

## ✅ COMPLETED FEATURES (previously "Next Session")

All 4 features below are **fully implemented** as of 2026-03-10. See `PLAN.md` for details.
The only bug that needed fixing was wallet balance (Feature 3). Everything else was already done.

---

### FEATURE 1 — Wallet Info Box (bottom-left HUD)

**What:** A persistent info panel in the bottom-left corner of the UI showing the currently active/selected wallet's public key and account address.

**Where to add it:** `messaging_app/static/index.html`
- Add a fixed `div` in the bottom-left (above or near the sidebar footer)
- Show:
  - `display_name` (e.g. "Kyle")
  - `public_key_hex` (truncated: first 8 + "..." + last 8 chars, full key on hover/click-to-copy)
  - `account_address` (component address, same truncation + copy)
- Style: dark pill/card, subtle, WhatsApp-esque. Always visible when a wallet is selected.
- Update when the user switches wallets in the sidebar.
- The wallet selector already exists in the UI — wire the display name + keys to update the HUD whenever `currentWallet` changes in JS.

**No backend changes needed** — wallet data is already returned by `/api/wallets`.

---

### FEATURE 2 — Template / Sharing Architecture Fix

**The problem:**
If you ship this folder (without `messaging-state.json`) to someone else, they run `cargo run` and it deploys a **brand new component** on Esmeralda. Their component address ≠ yours. They cannot DM you and you cannot DM them. Each install is an island.

**The solution — two-part:**

#### Part A: "Join Existing Component" UI flow
Add a new setup option in the Configure modal / setup screen:
- **Option 1 (current):** "Publish & Deploy" — auto-publish WASM + create new component
- **Option 2 (new):** "Join existing component" — paste a component address + template address, skip deploy entirely

Backend: `POST /api/component/join`
Body: `{ "component_address": "component_...", "template_address": "template_..." }`
Logic:
1. Validate both addresses parse correctly
2. Set `state.component_address` and `state.template_address`
3. Set `state.setup_status = Ready`
4. Save state
5. Return `{ ok: true }`

No blockchain transaction needed — just update local state.

#### Part B: Display YOUR shareable component address prominently
- In the Configure / Settings screen, once `setup_status == Ready`, show the `component_address` in a big copyable box labeled **"Share this address so others can join your chat"**
- Also show `template_address` since others need both to join
- Add a "Copy" button for each

This way: you deploy once, share your component address, others paste it into their "Join" flow.

**Note on the template concern:**
The template itself (WASM bytecode) is the same for everyone — it lives on-chain at the `template_address`. The **component** is what holds the data (messages). Two installs pointing at the same component address = they share all messages. The template address never changes once published. So the sharing flow is: share component_address + template_address → others join → everyone reads/writes the same on-chain data.

---

### FEATURE 3 — Wallet Balances

**What:** Show each wallet's tTARI balance in the sidebar wallet list and/or the bottom-left HUD.

**Backend — new endpoint:** `GET /api/wallet/balance?public_key_hex=<hex>`

Logic in `main.rs`:
1. Look up the `UserConfig` for the given `public_key_hex`
2. In a `spawn_blocking` block, create a provider for that wallet
3. Call `provider.get_balance(account_addr).await` or query account vault balance
   - **API to use:** `provider.get_account_balance(account_addr)` — returns `Vec<(ResourceAddress, Amount)>`
   - Filter for `TARI_TOKEN` resource address to get tTARI balance
   - Return balance in micro-TARI; frontend divides by 1_000_000 to show as "X.XX tTARI"
4. Return `{ "balance_micro_tari": 12345678, "balance_display": "12.35 tTARI" }`

**If `get_account_balance` doesn't exist on `IndexerProvider`**, try:
- `provider.get_vault_balance(vault_id)` — but need vault_id first
- Or use the indexer REST API directly: `GET {INDEXER_URL}substates/{component_addr}` and parse the vault balance from the component state JSON
- As a fallback: just display "Balance: unknown" and log a warning — don't break the app

**Frontend:**
- After wallet is selected or created, call `/api/wallet/balance?public_key_hex=...`
- Show result in the bottom-left HUD (Feature 1) and in the sidebar next to the wallet name
- Add a "Refresh" button (↺) that re-fetches balance on demand
- Show a loading spinner while fetching (balance calls can take 2-5 seconds)

---

---

### FEATURE 4 — Seed Phrase / Key Backup & Recovery

**Background from walletd CLI (`--help`):**
The official Tari wallet daemon supports seed words natively:
- `tari_ootle_walletd seed-words` — prints current seed words for backup
- `tari_ootle_walletd --seed-words "word1 word2 ..."` — restores wallet from seed words on startup

**IMPORTANT COMPATIBILITY NOTE:**
Our app derives keys differently from walletd. They are **NOT compatible**:
- `walletd` uses its own HD derivation from a master seed → produces seed words
- Our app uses raw `RistrettoSecretKey` hex pairs (`account_secret_hex` + `view_secret_hex`)
- Our "passphrase import" (SHA-256 derived) is also NOT walletd-compatible

This means: a user cannot take their walletd seed words and import them into our app directly (and vice versa). This is a known limitation.

**What to build — two-part:**

#### Part A: Show & Export User's Own Keys (Backup)
In the wallet HUD (Feature 1) or a dedicated "Wallet Details" modal:
- Add a "Backup Keys" button
- Show the raw hex keys in a modal with a big WARNING: "Save these somewhere safe. Anyone with these keys controls your wallet."
  - `Account Secret Key:` (64 hex chars) — with copy button
  - `View Secret Key:` (64 hex chars) — with copy button
- Also show public key + account address (non-sensitive, can share freely)
- These hex keys can be re-imported via the existing "Import by Hex Keys" flow

No backend changes needed — keys are in the wallet data on the server. Add a new endpoint:
`GET /api/wallet/export-keys?public_key_hex=<hex>`
Returns `{ account_secret_hex, view_secret_hex }` — ONLY serve this over localhost, add a check that the request comes from 127.0.0.1 or add a confirm step.

#### Part B: BIP-39 Style Mnemonic for Our Keys (Optional, Nice-to-Have)
Since our keys are just 32-byte scalars, we can encode them as BIP-39 mnemonics (256 bits = 24 words) using the `bip39` crate:
- `account_secret_hex` → 24-word mnemonic
- `view_secret_hex` → 24-word mnemonic
- Import: paste 24 words → decode back to hex → import as hex keys

This gives users a more human-friendly backup format without needing walletd compatibility.

Add to `Cargo.toml`:
```toml
bip39 = "2"
```

Encoding:
```rust
use bip39::{Mnemonic, Language};
let mnemonic = Mnemonic::from_entropy(&key_bytes, Language::English)?;
let words = mnemonic.to_string();  // "word1 word2 ... word24"
```
Decoding:
```rust
let mnemonic = Mnemonic::parse_in(Language::English, words)?;
let entropy = mnemonic.to_entropy();  // back to [u8; 32]
```

**UI for recovery flow:**
Add a 4th tab to the wallet import modal (alongside Generate / Hex Keys / Passphrase):
- **"Seed Words"** tab
- Two text areas: "Account Seed Words (24 words)" + "View Seed Words (24 words)"
- Calls existing `/api/wallet/import` after decoding words → hex server-side
  - OR add `POST /api/wallet/import-mnemonic` that accepts `{ display_name, account_words, view_words }`

---

### Implementation Order

1. **Feature 1** (wallet HUD) — pure frontend, no risk, do first
2. **Feature 3** (balance endpoint) — backend + frontend
   - Research the correct `ootle-rs` balance API before writing code
   - Check `IndexerProvider` methods and `WalletProvider` trait for balance querying
3. **Feature 4A** (export keys button + modal) — small backend endpoint + frontend modal
4. **Feature 4B** (BIP-39 mnemonic import) — add `bip39 = "2"` to Cargo.toml, new import tab
5. **Feature 2** (join component flow) — backend + frontend, do last

---

### Files to Touch

| File | Changes |
|------|---------|
| `messaging_app/static/index.html` | Feature 1 HUD, Feature 2 join modal + share box, Feature 3 balance display |
| `messaging_app/src/main.rs` | Feature 2 `/api/component/join`, Feature 3 `/api/wallet/balance`, Feature 4A `/api/wallet/export-keys` |
| `messaging_app/Cargo.toml` | Add `bip39 = "2"` for Feature 4B |

No template changes needed for any of these features.

---

---

## Experimental Features

> ⚠️ **WARNING:** Nothing in this section has been implemented or tested. All code samples are design proposals only. Before shipping any of this, every encryption primitive must be tested independently:
> - Key generation and serialization round-trips
> - ECDH shared secret equality between sender and recipient
> - ML-KEM encapsulate → decapsulate producing the same shared secret
> - HKDF output determinism
> - AES-256-GCM encrypt → decrypt round-trip
> - Full end-to-end: encrypt with Ootle's keys, decrypt with Minotari's keys
> - Edge cases: empty messages, maximum size messages, wrong key decryption failing gracefully
>
> Encryption bugs are silent and catastrophic. Write tests first, ship second.

These are ideas that have NOT been implemented. They are complex, risky, or depend on APIs that may not exist yet. Approach with caution.

### E1 — End-to-End Message Encryption (E2EE) — Superseded

**Status:** Superseded by the V2 Decentralized Architecture design above.

The original E1 proposal (X25519 key pairs derived from account secret) was a patch on top of the broken V1 shared-component design. It would have encrypted content but still stored everything in one shared component.

The correct approach is the full V2 redesign: per-user inbox components + Ristretto ECDH (no new key pairs — uses existing wallet keys). See **Decentralized Architecture (V2 Design)** section above.

**Do not implement E1 as originally described.** Do V2 instead.

---

### E3 — Post-Quantum Cryptography (PQC) Encryption

**Status:** Not implemented — exploration only. Feasibility analysis below.

---

#### What and Why

The V2 design uses **Ristretto ECDH** (classical elliptic curve cryptography) for key agreement. A sufficiently powerful quantum computer running Shor's algorithm could break elliptic curve discrete logarithm — meaning a future adversary could decrypt all messages sent today if they stored them ("harvest now, decrypt later").

**ML-KEM** (Module Lattice Key Encapsulation Mechanism, NIST FIPS 203, August 2024 — formerly CRYSTALS-Kyber) is the current NIST standard for quantum-resistant key encapsulation. Pairing it with **AES-256-GCM** (already quantum-resistant — Grover's algorithm only halves effective key length, 256-bit → 128-bit equivalent) gives a fully post-quantum encryption scheme.

---

#### How ML-KEM Works (vs ECDH)

ECDH is a *key agreement* — both parties compute the same shared secret independently, nothing extra is transmitted per message.

ML-KEM is a *key encapsulation mechanism (KEM)* — the sender generates a shared secret and a ciphertext that encapsulates it for the recipient. The ciphertext must be transmitted alongside the encrypted message.

**V2 (Ristretto ECDH):**
```
Minotari has: ristretto_public_key (32 bytes, already in wallet)

Ootle sends:
  shared_secret = ECDH(alice_priv × bob_pub)       ← computed locally, nothing sent
  message       = AES-256-GCM(shared_secret, text)  ← sent on-chain
  on-chain size ≈ message_length + 28 bytes (nonce + GCM tag)
```

**E3 (ML-KEM-768 + AES-256-GCM):**
```
Minotari has: ml_kem_public_key (1184 bytes, stored in his inbox component)

Ootle sends:
  (kem_ciphertext, shared_secret) = ML-KEM.Encapsulate(bob_ml_kem_pub)
  message = AES-256-GCM(shared_secret, text)
  on-chain payload = kem_ciphertext (1088 bytes) + aes_ciphertext
  on-chain size ≈ message_length + 1116 bytes overhead per message
```

---

#### Key Size Comparison

| Algorithm | Public Key | Per-Message Overhead | Quantum Resistant |
|-----------|-----------|---------------------|-------------------|
| Ristretto ECDH | 32 bytes | 0 bytes | ❌ |
| ML-KEM-512 | 800 bytes | 768 bytes | ✅ |
| ML-KEM-768 | 1184 bytes | 1088 bytes | ✅ (recommended) |
| ML-KEM-1024 | 1568 bytes | 1568 bytes | ✅ (paranoid) |

ML-KEM-768 is the NIST recommended level — equivalent to AES-192 security. The 1088-byte overhead per message is the main cost.

---

#### Rust Crate

```toml
ml-kem = "0.3"   # RustCrypto, implements FIPS 203
aes-gcm = "0.10" # Already needed for AES-256-GCM
```

The `ml-kem` crate is maintained by RustCrypto (same team as `aes-gcm`). It is `no_std` compatible.

**WASM compatibility:** Likely works — RustCrypto crates generally compile to `wasm32-unknown-unknown`. Unverified for this specific crate. Needs a test build before committing to this approach.

---

#### Where Minotari's ML-KEM Public Key Lives

Minotari's Ristretto public key is 32 bytes — small enough to share as a hex string. His ML-KEM public key is 1184 bytes — too large to share manually.

The natural place is the inbox component itself:

```rust
pub struct UserInbox {
    messages: Vec<EncryptedMessage>,
    owner_pk: RistrettoPublicKeyBytes,        // 32 bytes — Ristretto identity key
    ml_kem_public_key: Option<Vec<u8>>,       // 1184 bytes — ML-KEM encryption key
}
```

Ootle queries Minotari's inbox component once to retrieve his ML-KEM public key, caches it locally, then uses it for all future messages. Minotari's ML-KEM private key never leaves his machine.

The ML-KEM key pair would be generated at wallet creation and stored in the local state file alongside the Ristretto keys.

---

#### Recommended: Hybrid Mode (Ristretto ECDH + ML-KEM)

NIST and major implementors (Signal PQ3, Apple iMessage PQ3, Chrome TLS) all recommend a **hybrid** approach during the transition period: combine classical ECDH with ML-KEM so you get classical security today AND quantum resistance for the future. If either algorithm is broken, the other still protects you.

```
shared_secret = HKDF(
    ristretto_ecdh_secret  ||  ml_kem_secret,
    info = "tari-messenger-v2-hybrid"
)
message = AES-256-GCM(shared_secret, plaintext)
```

On-chain payload includes both the ML-KEM ciphertext (1088 bytes) and the AES ciphertext. The ECDH shared secret is computed locally with no extra bytes.

---

#### Is It Feasible on This Application?

**Yes, technically.** The Rust crate exists, is standards-compliant, and should compile to WASM.

**The real cost is on-chain fees.** Tari charges fees proportional to transaction size. Every single DM would carry ~1116 bytes of PQC overhead on top of the actual message content. At current testnet fee rates this is manageable, but it could become expensive on mainnet depending on how fee markets develop.

**The threat model question.** This messenger has no KYC — wallets are anonymous key pairs. A quantum adversary breaking the encryption would see the message content but would still have no identity to attach it to. Whether the quantum threat justifies the storage cost overhead depends entirely on how sensitive the message content is.

**Verdict:**
| Concern | Assessment |
|---------|-----------|
| Rust crate available | ✅ `ml-kem = "0.3"` (RustCrypto) |
| WASM compatibility | ⚠️ Likely yes — needs verification |
| On-chain overhead | ⚠️ ~1116 bytes per message |
| Fee impact | ⚠️ Unknown until mainnet fee market exists |
| Quantum threat vs no-KYC | 🤔 Low priority — no identity to attach decrypted content to |
| Implementation complexity | ⚠️ Moderate — key storage, key distribution, hybrid mode |
| Recommended approach | Hybrid Ristretto ECDH + ML-KEM-768 |

**Implement after V2 is stable.** V2 with Ristretto ECDH is already a significant improvement over V1. PQC is a hardening layer for a future threat, not a current vulnerability.

---

#### Implementation Exploration: What It Would Actually Take

This section works through the full implementation so it can be built when ready.

---

##### Step 1 — Dependencies (`messaging_app/Cargo.toml`)

```toml
ml-kem  = "0.3"   # NIST FIPS 203 — RustCrypto
hkdf    = "0.12"  # Key derivation for hybrid scheme
aes-gcm = "0.10"  # Symmetric encryption
base64  = "0.22"  # Encoding for on-chain storage
rand    = "0.8"   # OsRng
```

Note: these are all `messaging_app` dependencies only. The WASM template does NOT need ml-kem — key generation and encryption happen in the local app, not in the smart contract. The template only stores the result.

---

##### Step 2 — Template Changes (`messaging_template/src/lib.rs`)

Add an `ml_kem_ek` field to `UserInbox` to store Minotari's ML-KEM public encapsulation key. Ootle reads this once before sending her first PQC message.

```rust
pub struct UserInbox {
    messages: Vec<EncryptedMessage>,
    owner_pk: RistrettoPublicKeyBytes,
    ml_kem_ek: Option<String>,  // base64-encoded ML-KEM-768 encapsulation key (1184 bytes → ~1580 chars base64)
}

impl UserInbox {
    // Called once after deploy to publish the ML-KEM public key on-chain
    pub fn set_ml_kem_key(&mut self, ek_base64: String) {
        let caller = CallerContext::transaction_signer_public_key();
        assert_eq!(caller, self.owner_pk, "Only owner can set their encryption key");
        self.ml_kem_ek = Some(ek_base64);
    }

    pub fn get_ml_kem_key(&self) -> Option<String> {
        self.ml_kem_ek.clone()
    }
}
```

One extra transaction per wallet (publish the ML-KEM key after deploying the inbox). After that, no further key updates unless the user rotates keys.

---

##### Step 3 — Key Generation (`messaging_app/src/main.rs`)

At wallet creation, generate both the Ristretto key pair (already done) AND an ML-KEM-768 key pair. Store the decapsulation key (private) locally, publish the encapsulation key (public) to the inbox component.

```rust
use ml_kem::{MlKem768, KemCore};
use ml_kem::kem::Encapsulate;
use base64::{Engine, engine::general_purpose::STANDARD as B64};

fn generate_ml_kem_keypair() -> (Vec<u8>, String) {
    // Returns (dk_bytes, ek_base64)
    // dk = decapsulation key (private, 2400 bytes) — stored in UserConfig
    // ek = encapsulation key (public, 1184 bytes)  — published to inbox component
    let (dk, ek) = MlKem768::generate(&mut rand::rngs::OsRng);
    let dk_bytes = dk.as_bytes().to_vec();
    let ek_b64   = B64.encode(ek.as_bytes());
    (dk_bytes, ek_b64)
}
```

Add to `UserConfig`:
```rust
struct UserConfig {
    // ... existing fields ...
    ml_kem_dk_hex: String,  // ML-KEM decapsulation key (private, 2400 bytes as hex)
}
```

After deploying the inbox component, publish the encapsulation key in a second transaction:
```rust
call_method(inbox_addr, "set_ml_kem_key", args![ek_base64])
```

---

##### Step 4 — Hybrid Encryption

The hybrid scheme combines Ristretto ECDH (classical) and ML-KEM (quantum-resistant). Both must be broken simultaneously to compromise the message.

```rust
use hkdf::Hkdf;
use sha2::Sha256;
use ml_kem::kem::{Encapsulate, Decapsulate};

/// Encrypt a message using Hybrid Ristretto-ECDH + ML-KEM-768 + AES-256-GCM.
/// Returns a payload string stored on-chain.
fn hybrid_encrypt(
    sender_ristretto_sk: &RistrettoSecretKey,
    recipient_ristretto_pk: &RistrettoPublicKey,
    recipient_ml_kem_ek_b64: &str,
    plaintext: &str,
) -> anyhow::Result<String> {

    // 1. Ristretto ECDH — classical shared secret (32 bytes)
    let ecdh_secret = ristretto_ecdh(sender_ristretto_sk, recipient_ristretto_pk);

    // 2. ML-KEM encapsulation — quantum-resistant shared secret (32 bytes) + ciphertext (1088 bytes)
    let ek_bytes = B64.decode(recipient_ml_kem_ek_b64)?;
    let ek = MlKem768EncapsKey::try_from(ek_bytes.as_slice())?;
    let (mlkem_ct, mlkem_secret) = ek.encapsulate(&mut OsRng).map_err(|e| anyhow::anyhow!("{e:?}"))?;

    // 3. Combine both secrets via HKDF — strong as the stronger of the two algorithms
    let mut ikm = [0u8; 64];
    ikm[..32].copy_from_slice(&ecdh_secret);
    ikm[32..].copy_from_slice(mlkem_secret.as_slice());
    let hk = Hkdf::<Sha256>::new(None, &ikm);
    let mut aes_key = [0u8; 32];
    hk.expand(b"tari-messenger-v2-pqc-hybrid", &mut aes_key)?;

    // 4. AES-256-GCM encrypt
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&aes_key));
    let nonce  = Aes256Gcm::generate_nonce(&mut OsRng);
    let aes_ct = cipher.encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("AES encrypt: {e}"))?;

    // 5. Pack into on-chain payload
    // Format: "HPQC1:<base64(mlkem_ct)>:<base64(nonce+aes_ct)>"
    let mut aes_payload = nonce.to_vec();
    aes_payload.extend_from_slice(&aes_ct);
    Ok(format!("HPQC1:{}:{}", B64.encode(mlkem_ct.as_ref()), B64.encode(aes_payload)))
}
```

---

##### Step 5 — Hybrid Decryption

```rust
fn hybrid_decrypt(
    recipient_ristretto_sk: &RistrettoSecretKey,
    sender_ristretto_pk: &RistrettoPublicKey,
    recipient_ml_kem_dk_hex: &str,
    payload: &str,
) -> Option<String> {

    // Parse payload
    let rest = payload.strip_prefix("HPQC1:")?;
    let mut parts = rest.splitn(2, ':');
    let mlkem_ct_b64 = parts.next()?;
    let aes_payload_b64 = parts.next()?;

    // 1. Ristretto ECDH — same secret Ootle computed
    let ecdh_secret = ristretto_ecdh(recipient_ristretto_sk, sender_ristretto_pk);

    // 2. ML-KEM decapsulation — recover shared secret from ciphertext
    let dk_bytes = Vec::from_hex(recipient_ml_kem_dk_hex).ok()?;
    let dk = MlKem768DecapsKey::try_from(dk_bytes.as_slice()).ok()?;
    let mlkem_ct_bytes = B64.decode(mlkem_ct_b64).ok()?;
    let mlkem_ct = MlKem768Ciphertext::try_from(mlkem_ct_bytes.as_slice()).ok()?;
    let mlkem_secret = dk.decapsulate(&mlkem_ct).ok()?;

    // 3. Re-derive AES key via HKDF
    let mut ikm = [0u8; 64];
    ikm[..32].copy_from_slice(&ecdh_secret);
    ikm[32..].copy_from_slice(mlkem_secret.as_slice());
    let hk = Hkdf::<Sha256>::new(None, &ikm);
    let mut aes_key = [0u8; 32];
    hk.expand(b"tari-messenger-v2-pqc-hybrid", &mut aes_key).ok()?;

    // 4. AES-256-GCM decrypt
    let aes_payload = B64.decode(aes_payload_b64).ok()?;
    if aes_payload.len() < 12 { return None; }
    let (nonce_bytes, aes_ct) = aes_payload.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&aes_key));
    let nonce  = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher.decrypt(nonce, aes_ct).ok()?;
    String::from_utf8(plaintext).ok()
}
```

---

##### Step 6 — Fetching Recipient's ML-KEM Key

Ootle needs Minotari's ML-KEM encapsulation key before she can send a PQC message. She reads it from his inbox component via the indexer — one HTTP GET, cached locally after the first fetch.

```rust
async fn fetch_ml_kem_ek(inbox_component_addr: &str) -> anyhow::Result<Option<String>> {
    let url = format!("{}substates/{}", INDEXER_URL, inbox_component_addr);
    let resp: serde_json::Value = reqwest::get(&url).await?.json().await?;
    // Parse ml_kem_ek field from component state JSON
    let ek = find_string_field(&resp, "ml_kem_ek");
    Ok(ek)
}
```

Add `ml_kem_ek_cache: HashMap<String, String>` to `AppState` (pk → ek_base64). On first message to a new contact, fetch and cache. Never fetches again unless user requests key refresh.

---

##### Step 7 — On-Chain Message Format

Messages need a version prefix so the app knows which decryption path to use. All formats remain backward-compatible.

| Prefix | Meaning | Encryption |
|--------|---------|-----------|
| *(none)* | Legacy plaintext (V1) | None |
| `ENC:` | V2 classical only | Ristretto ECDH + AES-256-GCM |
| `HPQC1:` | V2 hybrid PQC | Ristretto ECDH + ML-KEM-768 + AES-256-GCM |

The app tries decryption in reverse order — if `HPQC1:` prefix, use hybrid. If `ENC:` prefix, use classical. Otherwise display as-is.

---

##### Step 8 — Per-Message Overhead Summary

```
Ristretto ECDH only (V2):
  plaintext (N bytes) + nonce (12) + GCM tag (16) = N + 28 bytes on-chain

Hybrid PQC (E3):
  ML-KEM ciphertext:  1088 bytes
  base64 overhead:    ~369 bytes (base64 expands by 4/3)
  nonce + GCM tag:    28 bytes
  separators/prefix:  8 bytes
  total overhead:     ~1489 bytes + plaintext
```

A 100-character message becomes ~1600 bytes on-chain with PQC. At current Tari testnet fees (~2000 µTARI per transaction regardless of size) this is fine. Mainnet fee markets are unknown.

---

##### Build Order

1. Verify `ml-kem = "0.3"` compiles cleanly with `cargo check` (no WASM needed — app only)
2. Implement `generate_ml_kem_keypair()` and add to wallet creation
3. Add `ml_kem_ek` field to `UserInbox` template, add `set_ml_kem_key` method
4. Implement `hybrid_encrypt` and `hybrid_decrypt`
5. Add `fetch_ml_kem_ek` + `ml_kem_ek_cache` to app state
6. Wire into `handle_dm_send` (encrypt) and `handle_dm_messages` (decrypt)
7. Keep `ENC:` path as fallback for contacts without ML-KEM keys published

---

### E2 — Token Balance Display

**Status:** Not implemented — API unclear / intermittently broken

**Idea:** Show each wallet's tTARI balance in the UI (sidebar HUD or wallet dropdown).

**What was tried:** A `/api/wallet/balance` endpoint exists and queries `IndexerProvider`. However the balance always shows 0 or errors intermittently, likely due to UTXO scanning delays on Esmeralda testnet.

**Proposed backend approach:**
```rust
// In make_provider, after connecting:
let balance = provider.get_account_balance(account_addr).await?;
// Returns Amount (u64 micro-TARI)
```

**Why it's broken:**
- `IndexerProvider` may not expose a reliable balance query for stealth/confidential outputs
- The Esmeralda testnet UTXO set takes 30–60s to settle after faucet funding
- Balance may require scanning the view key outputs, which the indexer may not do automatically

**Frontend:** The `updateHud()` function already has a placeholder. Would need to call `/api/wallet/balance?wallet_pk=<pk>` and render the result.

---

## Decentralized Architecture (V2 Design)

**Status:** Not implemented — full redesign required. Document this before starting.

> ⚠️ **Encryption is an Experimental Feature**
>
> All encryption described in this section (Ristretto ECDH for DMs, shared symmetric keys for group chat) is **experimental and must be treated as such when implemented**. Encryption bugs do not crash the app — they silently produce unreadable messages or, worse, a false sense of security. This means bugs can ship undetected.
>
> Encryption will require extensive testing before it can be considered stable:
>
> - **Unit tests**: each crypto primitive in isolation (ECDH, AES-256-GCM, key derivation)
> - **Round-trip tests**: encrypt with one key set, decrypt with the other, assert equality
> - **Cross-wallet tests**: Ootle encrypts for Minotari, Minotari decrypts — and vice versa
> - **Negative tests**: wrong key must fail to decrypt, not silently return garbage
> - **Group chat tests**: require the most testing of anything here (see note below)
> - **Regression tests**: any change to key derivation or message format must not silently break old messages
>
> **Group chat encryption in particular needs serious testing.** The key distribution flow (creator sends group key to each member via encrypted DM, new member onboarding, key rotation on member removal) involves multiple transactions and multiple key operations in sequence. Any bug in this flow leaves members unable to read messages with no obvious error. Test every step of the group key lifecycle independently before combining them.
>
> **Do not ship encryption as a default-on feature.** When first implemented, gate it behind a feature flag or clearly label it experimental in the UI. Let users opt in. This gives real-world testing without silently breaking messaging for everyone.

The current app has a centralized flaw: one shared component holds all messages for all users. Whoever deployed it controls it. This section describes the correct fully decentralized design.

---

### Core Insight

Minotari's Ristretto public key hex is his complete identity. From it you can derive:
- His **inbox component address** (deterministic via `with_public_key_address(pk)`)
- His **encryption key** (same key, used for ECDH)

Minotari gives Ootle his public key hex once (any channel — text, QR, in person). That is all Ootle ever needs.

---

### V2 Template: Per-User Inbox Component

Each user deploys their own inbox component. The address is deterministic — computed from their public key, no lookup needed.

```rust
pub struct UserInbox {
    messages: Vec<EncryptedMessage>,
    owner_pk: RistrettoPublicKeyBytes,
}

pub struct EncryptedMessage {
    from_pk: String,    // sender's public key (public — needed for ECDH decryption)
    ciphertext: String, // "ENC:<base64(nonce+ciphertext)>" — only owner can decrypt
}

impl UserInbox {
    // Deploy once. Address is derived from your public key — no one else can claim it.
    pub fn deploy() -> Component<Self> {
        let pk = CallerContext::transaction_signer_public_key();
        Component::new(Self { messages: Vec::new(), owner_pk: pk })
            .with_public_key_address(pk)  // address = f(public_key), deterministic
            .with_access_rules(
                ComponentAccessRules::new()
                    .method("receive_dm", rule!(allow_all))   // anyone can send to you
                    .method("get_messages", rule!(allow_all)) // ciphertext is public anyway
                    .default(rule!(deny_all))
            )
            .create()
    }

    pub fn receive_dm(&mut self, from_pk: String, ciphertext: String) {
        assert!(ciphertext.starts_with("ENC:"), "Messages must be encrypted");
        assert!(ciphertext.len() <= 2048, "Message too large");
        self.messages.push(EncryptedMessage { from_pk, ciphertext });
    }

    pub fn get_messages(&self) -> Vec<EncryptedMessage> {
        self.messages.clone()
    }
}
```

---

### DM Encryption: Ristretto ECDH

No new keys needed. The wallet's existing Ristretto secret key handles decryption.

**Ootle sends to Minotari:**
```
shared_secret = ECDH(alice_secret_key × bob_public_key) → SHA256 → 32-byte AES key
ciphertext    = AES-256-GCM(shared_secret, plaintext)
tx            = call_method(bob_inbox_addr, "receive_dm", args![alice_pk, ciphertext])
```

**Minotari reads:**
```
shared_secret = ECDH(bob_secret_key × alice_public_key) → SHA256 → 32-byte AES key
plaintext     = AES-256-GCM-decrypt(shared_secret, ciphertext)
```

Both sides independently compute the same shared secret. Minotari's secret key never leaves his machine.

**Important:** `tari_crypto` already provides Ristretto scalar multiplication. No new crypto dependencies needed — `tari_crypto = "0.22"` + `aes-gcm = "0.10"` is sufficient.

---

### Group Chat: Shared Symmetric Key

ECDH only works between 2 parties. Groups use a randomly generated AES-256 symmetric key distributed via encrypted DMs.

**Creating a group:**
1. Ootle generates a random 32-byte group key
2. Ootle creates a group component on-chain (public message board — stores ciphertexts only)
3. For each member (Minotari, Carol...), Ootle DMs them the group key:
   - Encrypt group key with `ECDH(alice_priv, member_pub)` → send to their inbox

**Posting to a group:**
```
ciphertext = AES-256-GCM(group_key, plaintext)
tx = call_method(group_component, "post", args![from_pk, ciphertext])
```

**New member joins:**
- An existing member DMs them the group key (encrypted with their public key)
- They can then read all history and post new messages

**Someone leaves (key rotation):**
- Remaining members agree on a new random group key
- Each remaining member receives the new key via encrypted DM
- All future messages use the new key
- Past messages remain readable with the old key (forward secrecy not guaranteed — acceptable tradeoff)

**Group component structure:**
```rust
pub struct GroupRoom {
    room_id: String,
    creator_pk: String,
    member_pks: Vec<String>,           // public list of members
    messages: Vec<GroupMessage>,       // ciphertexts only
}

pub struct GroupMessage {
    from_pk: String,
    ciphertext: String,  // "ENC:<base64>" encrypted with group key
}
```

---

### Key Discovery: How Ootle Gets Minotari's Public Key

There is no way to discover a public key without Minotari sharing it. This is not a flaw — it is correct security design. Options (pick one or combine):

| Method | How | Tradeoff |
|--------|-----|----------|
| Manual share | Minotari pastes his PK hex to Ootle | Requires one out-of-band message |
| QR code | Minotari shows QR, Ootle scans | Best UX for in-person |
| On-chain username registry | Second component maps "minotari" → inbox address | Permissionless but still a shared component |
| Social graph | Carol shares Minotari's PK with Ootle | Trust the introducer |

The username registry option: a separate `UsernameRegistry` template where anyone can register a name. This is permissionless (no owner can censor it once deployed) and just a convenience layer on top of the PK-based system.

---

### Comparison: V1 vs V2

| | V1 (Current) | V2 (Decentralized) |
|---|---|---|
| Message storage | One shared component | Each user's own component |
| Privacy | All messages publicly readable | Only ciphertext on-chain |
| Identity | Public key + shared component | Public key alone |
| Deployment | One person deploys for everyone | Each user deploys their own inbox |
| Send a DM | Write to shared state | Call recipient's component directly |
| Find someone | Know the shared component address | Know their public key hex |
| Encryption | None | Ristretto ECDH + AES-256-GCM |
| Groups | Shared component, plaintext | Group component + symmetric key via DMs |

---

### Files to Rewrite for V2

| File | Change |
|------|--------|
| `messaging_template/src/lib.rs` | Replace `MessagingService` with `UserInbox` + `GroupRoom` templates |
| `messaging_app/src/main.rs` | Deploy inbox on wallet create; derive inbox address from PK; Ristretto ECDH encrypt/decrypt; group key management |
| `messaging_app/static/index.html` | Add contact by PK hex; display your own PK as shareable address; encrypt/decrypt in flow |
| `messaging_app/Cargo.toml` | Add `aes-gcm = "0.10"`, `base64 = "0.22"` |

No new crypto dependencies for key agreement — `tari_crypto` already handles Ristretto scalar multiplication.

---

---

## Test Case Scripts (Encryption Primitives)

> These are test stubs to write **before** implementing any encryption in the app.
> Write them in `messaging_app/src/main.rs` (under `#[cfg(test)]`) or a separate `tests/` file.
> All tests must pass before encryption is turned on for real users.

```rust
#[cfg(test)]
mod encryption_tests {
    use super::*;

    /// ECDH: both parties compute the same shared secret from opposite keys.
    #[test]
    fn test_ecdh_shared_secret_is_symmetric() {
        // Alice's key pair
        let alice_sk = generate_test_secret_key(b"alice-test-seed-000");
        let alice_pk = RistrettoPublicKey::from_secret_key(&alice_sk);

        // Bob's key pair
        let bob_sk = generate_test_secret_key(b"bob-test-seed-00000");
        let bob_pk = RistrettoPublicKey::from_secret_key(&bob_sk);

        // Both sides compute the shared secret
        let alice_sees = ristretto_ecdh(&alice_sk, &bob_pk);
        let bob_sees   = ristretto_ecdh(&bob_sk,   &alice_pk);

        assert_eq!(alice_sees, bob_sees, "ECDH shared secret must be symmetric");
        assert_ne!(alice_sees, [0u8; 32], "Shared secret must not be zero");
    }

    /// AES-256-GCM: encrypt then decrypt returns the original plaintext.
    #[test]
    fn test_aes_gcm_round_trip() {
        let key = [0x42u8; 32]; // arbitrary test key
        let plaintext = b"Hello, Minotari!";

        let ciphertext = aes_gcm_encrypt(&key, plaintext).expect("encrypt failed");
        let recovered  = aes_gcm_decrypt(&key, &ciphertext).expect("decrypt failed");

        assert_eq!(recovered, plaintext, "Decrypted text must match original");
    }

    /// AES-256-GCM: decryption with the wrong key must fail, not return garbage.
    #[test]
    fn test_aes_gcm_wrong_key_fails() {
        let key_a = [0x11u8; 32];
        let key_b = [0x22u8; 32]; // different key

        let ciphertext = aes_gcm_encrypt(&key_a, b"secret message").expect("encrypt failed");
        let result = aes_gcm_decrypt(&key_b, &ciphertext);

        assert!(result.is_none(), "Decryption with wrong key must return None, not garbage");
    }

    /// Empty message must survive the round trip without error.
    #[test]
    fn test_encrypt_empty_message() {
        let key = [0xAAu8; 32];
        let ciphertext = aes_gcm_encrypt(&key, b"").expect("encrypt empty failed");
        let recovered  = aes_gcm_decrypt(&key, &ciphertext).expect("decrypt empty failed");
        assert_eq!(recovered, b"", "Empty round trip must return empty");
    }

    /// Cross-wallet DM test: Ootle encrypts for Minotari, Minotari decrypts.
    #[test]
    fn test_cross_wallet_dm_encrypt_decrypt() {
        let ootle_sk    = generate_test_secret_key(b"ootle-test-seed-00");
        let ootle_pk    = RistrettoPublicKey::from_secret_key(&ootle_sk);
        let minotari_sk = generate_test_secret_key(b"minotari-test-seed");
        let minotari_pk = RistrettoPublicKey::from_secret_key(&minotari_sk);

        let plaintext = "Hey Minotari, this is a secret!";

        // Ootle encrypts
        let payload = encrypt_dm_for_recipient(&ootle_sk, &minotari_pk, plaintext)
            .expect("encrypt failed");

        // Minotari decrypts
        let decrypted = decrypt_dm(&minotari_sk, &ootle_pk, &payload)
            .expect("decrypt returned None");

        assert_eq!(decrypted, plaintext, "Decrypted text must match original");
    }

    /// Cross-wallet DM test: reverse direction — Minotari encrypts for Ootle.
    #[test]
    fn test_cross_wallet_dm_reverse_direction() {
        let ootle_sk    = generate_test_secret_key(b"ootle-test-seed-00");
        let ootle_pk    = RistrettoPublicKey::from_secret_key(&ootle_sk);
        let minotari_sk = generate_test_secret_key(b"minotari-test-seed");
        let minotari_pk = RistrettoPublicKey::from_secret_key(&minotari_sk);

        let plaintext = "Hey Ootle, reply here!";

        let payload   = encrypt_dm_for_recipient(&minotari_sk, &ootle_pk, plaintext)
            .expect("encrypt failed");
        let decrypted = decrypt_dm(&ootle_sk, &minotari_pk, &payload)
            .expect("decrypt returned None");

        assert_eq!(decrypted, plaintext);
    }

    /// Group key lifecycle: generate key → distribute via DM → members decrypt group messages.
    #[test]
    fn test_group_key_distribution_and_decrypt() {
        let creator_sk  = generate_test_secret_key(b"creator-test-seed0");
        let creator_pk  = RistrettoPublicKey::from_secret_key(&creator_sk);
        let member_sk   = generate_test_secret_key(b"member-test-seed00");
        let member_pk   = RistrettoPublicKey::from_secret_key(&member_sk);

        // Creator generates group key and distributes it via encrypted DM
        let group_key = generate_group_key(); // 32 random bytes
        let key_payload = encrypt_dm_for_recipient(&creator_sk, &member_pk,
            &hex::encode(&group_key))
            .expect("key distribution encrypt failed");

        // Member decrypts the group key
        let key_hex = decrypt_dm(&member_sk, &creator_pk, &key_payload)
            .expect("key distribution decrypt failed");
        let recovered_key = hex::decode(&key_hex).expect("invalid hex in key");
        assert_eq!(recovered_key, group_key, "Member must recover the correct group key");

        // Creator posts a message to the group
        let group_msg = "Welcome to the group!";
        let group_ciphertext = aes_gcm_encrypt(&group_key.try_into().unwrap(), group_msg.as_bytes())
            .expect("group encrypt failed");

        // Member reads the group message
        let decrypted = aes_gcm_decrypt(&recovered_key.try_into().unwrap(), &group_ciphertext)
            .expect("group decrypt failed");
        assert_eq!(decrypted, group_msg.as_bytes());
    }

    /// Key rotation: old key cannot decrypt messages encrypted with new key.
    #[test]
    fn test_group_key_rotation_invalidates_old_key() {
        let old_key = [0x11u8; 32];
        let new_key = [0x22u8; 32]; // rotated key after member removal

        let ciphertext = aes_gcm_encrypt(&new_key, b"Post-rotation message")
            .expect("encrypt failed");

        // Old key (e.g. held by removed member) must not decrypt new messages
        let result = aes_gcm_decrypt(&old_key, &ciphertext);
        assert!(result.is_none(), "Old key must not decrypt messages encrypted with new key");
    }

    /// Payload format: must start with "ENC:" prefix.
    #[test]
    fn test_encrypted_payload_has_expected_prefix() {
        let sk = generate_test_secret_key(b"format-test-seed00");
        let pk = RistrettoPublicKey::from_secret_key(&sk);
        let payload = encrypt_dm_for_recipient(&sk, &pk, "test").expect("encrypt failed");
        assert!(payload.starts_with("ENC:"), "Payload must start with ENC: prefix, got: {payload}");
    }

    /// Corrupt ciphertext must return None, not panic.
    #[test]
    fn test_corrupt_ciphertext_returns_none() {
        let sk = generate_test_secret_key(b"corrupt-test-seed0");
        let pk = RistrettoPublicKey::from_secret_key(&sk);
        let result = decrypt_dm(&sk, &pk, "ENC:not_valid_base64!!!!");
        assert!(result.is_none(), "Corrupt payload must return None");
    }
}
```

> **Note:** `generate_test_secret_key`, `ristretto_ecdh`, `aes_gcm_encrypt`, `aes_gcm_decrypt`, `encrypt_dm_for_recipient`, `decrypt_dm`, and `generate_group_key` are the functions you will implement in `main.rs` as part of the V2 encryption work. Write each test, implement the function to make it pass, then move to the next.

---

## Known Issues: Indexer Sync Limitations

### The Problem — No Pagination, No Sync-to-Tip

The current sync function (`sync_from_component_state` in `main.rs`) reads all on-chain state in a **single HTTP GET**:

```rust
let url = format!("{}substates/{}", INDEXER_URL, component_addr);
let resp: serde_json::Value = reqwest::get(&url).await?.json().await?;
// Reads dm_from, dm_to, dm_content, room_ids, etc. as full arrays from resp
```

**What this means:**
- The indexer returns the component state as a JSON blob
- There is no `?limit=` or `?offset=` parameter — it is all-or-nothing
- If the component has grown beyond the indexer's internal response limit (suspected ~1000 items per array), items beyond that index are **silently dropped** — no error, no warning
- You will not know messages are missing unless you count on-chain vs. local

**Concrete risk:**
- After 1000 DMs, messages 1001, 1002, ... never arrive in the local cache
- After 1000 room posts, same issue
- There is no way to detect this happened — the UI just shows a subset

---

### Short-Term Fix — Incremental Sync (Local Count as Cursor)

Until the indexer supports cursor-based pagination, the best available mitigation is to track how many items you already have locally and only process new ones after each sync. This does NOT solve truncation (if the indexer already dropped items) but prevents re-processing and gives a known-good index:

**In `AppState`:**
```rust
struct AppState {
    // ... existing fields ...
    last_synced_dm_count: usize,        // how many DMs we've processed from on-chain
    last_synced_room_msg_count: usize,  // how many room messages we've processed
}
```

**In `sync_from_component_state`:**
```rust
// After reading on-chain arrays:
let new_dms = on_chain_dms.get(state.last_synced_dm_count..).unwrap_or(&[]);
for dm in new_dms {
    // dedup check + push to state.dms
}
state.last_synced_dm_count = on_chain_dms.len();
```

This replaces the current O(n²) dedup scan (`if !s.dms.iter().any(...)`) with a O(n) forward-only pass. It also means sync is fast even with thousands of messages — we only process the tail.

---

### Long-Term Fix — True Cursor Pagination

The correct solution requires indexer API support. Proposed API (to request from Tari team or implement if indexer is open-source):

```
GET {INDEXER_URL}substates/{component_addr}/field/dm_from?offset=1000&limit=100
```

Or an event-log based approach: emit a `MessageSent` event from the template per message, then query events with pagination:

```
GET {INDEXER_URL}events?topic=MessagingService.MessageSent&after_block=<last_seen_block>
```

Event-based indexing is the canonical decentralized approach — it gives you an append-only log you can walk forward from any point, sync to tip, and resume from a checkpoint.

**Recommended template change (low cost, future-proof):**
```rust
pub fn send_dm(&mut self, to: String, content: String) {
    // ... existing validation ...
    self.dm_from.push(caller_pk.to_hex());
    self.dm_to.push(to);
    self.dm_content.push(content.clone());

    // Emit event — enables efficient sync-to-tip without full state reads
    emit_event("MessageSent", metadata![
        "kind"    => "dm",
        "to"      => to,
        "content" => content,   // NOTE: remove this field once E2EE is in, leave only "to"
    ]);
}
```

Querying events by topic with a block cursor would allow any client to sync from any point forward, never re-read old data, and know exactly where tip is.

---

### Impact Assessment

| Scenario | Risk Level | Notes |
|----------|-----------|-------|
| < 500 total DMs across all users | Low | Well within any likely limit |
| 500–1000 DMs | Medium | Monitor, add incremental sync |
| > 1000 DMs | High | Silent message loss likely — must fix before production |
| Group rooms with heavy traffic | High | Same limit applies to room messages |

**Priority:** Add incremental sync (short-term fix) as part of the next refactor. Add event emission to the template as a forward-compatibility measure. Push for cursor API support.

---

## Platform Independence

The app is **fully cross-platform** — Linux, macOS, and Windows are all supported. The only Windows-specific files are convenience launchers (`launch.bat`, `launch-dev.bat`), and the Tari wallet daemon binary in the repo root is a Windows `.exe`.

### For Linux / macOS

1. **Install Rust and the WASM target** (same as Windows):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup target add wasm32-unknown-unknown
   ```

2. **Build and run** (same commands):
   ```bash
   cd messaging_template && cargo build --target wasm32-unknown-unknown --release
   cd ../messaging_app && cargo run
   ```

3. **For the Tari wallet daemon** (optional — only needed to publish templates via Web UI):
   - Download the Linux/macOS binary from the [Tari releases page](https://github.com/tari-project/tari-ootle/releases)
   - Run: `./tari_ootle_walletd --network esme`

4. **Shell scripts instead of .bat** — create `launch.sh` and `launch-dev.sh` for Linux/macOS if needed:
   ```bash
   #!/bin/bash
   cd messaging_app && cargo run &
   open http://localhost:3000   # macOS; use xdg-open on Linux
   ```

### Binaries and Git

The `.gitignore` already excludes `*.exe` — the Windows wallet daemon binary is **not committed**. This is intentional:

- **Binaries do not belong in git.** They are large, not diffable, and must be fetched from the official Tari releases page for your platform.
- Each user downloads the wallet daemon for their OS from [github.com/tari-project/tari-ootle/releases](https://github.com/tari-project/tari-ootle/releases).
- The messaging app itself (`messaging_app/`) compiles from source — no binary distribution needed.

**If you want to release the app binaries**, use GitHub Releases (not the repo):
- Build: `cargo build --release` on each target platform
- Attach the binary to a GitHub Release (tag `v0.1.0` etc.)
- CI can cross-compile via `cross` or GitHub Actions matrix builds for Windows/Linux/macOS

### Launcher Scripts

| File | Platform | Purpose |
|------|----------|---------|
| `launch.bat` | Windows only | Single-click run the app |
| `launch-dev.bat` | Windows only | Two-client dev mode (Ootle + Minotari) |

To add Linux/macOS equivalents, copy the bat logic into `.sh` files with `#!/bin/bash` and `xdg-open`/`open` for browser launch. These would live alongside the bat files in the repo root.

---

## If You Run Out of Tokens
1. **First:** invoke `/tari-ootle` skill (REQUIRED before any code)
2. **Then say:** "Continue building the Tari Ootle messaging app. See PROGRESS.md for full context."
3. State: App is working end-to-end. Five features queued in "NEXT SESSION" section above.
   Build order: Feature 1 (HUD) → Feature 3 (balances) → Feature 4A (export keys) → Feature 4B (BIP-39 import) → Feature 2 (join component).
