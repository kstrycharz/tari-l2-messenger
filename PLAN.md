# Tari Messenger — Session Plan & Progress

## Session Summary (2026-03-10)

### What Was Done

#### ✅ Wallet Balance — FIXED
The `query_balance_micro_tari` function was broken because:
- **`collect_vault_ids`** looked for strings starting with `vault_`, but the indexer serializes
  vault IDs as CBOR Tag[132, Bytes[...]] nodes, e.g. `{"Tag":[132,{"Bytes":[14,249,...]}]}`
- **`find_u64_in`** expected numeric values, but the indexer serializes balances as strings,
  e.g. `"revealed_amount":"9889138"` (not `9889138`)

**Fix:** Replaced both helpers with correct implementations:
- `collect_vault_ids` now parses `Tag[132, Bytes[...]]` → `vault_<hex>`
- `extract_vault_balance` handles both string and numeric amounts for `revealed_amount`/`amount`

Verified against live Esmeralda indexer:
- Account: `component_0e69766efc95e...`
- Vault found: `vault_0ef9b6bc50b994...`
- Balance confirmed: 9889138 µTARI (~9.89 tTARI)

#### ✅ Backups saved
- `messaging_app/src/main.rs.bak`
- `messaging_app/static/index.html.bak`

### Feature Status After This Session

| Feature | Backend | Frontend | Status |
|---------|---------|----------|--------|
| Wallet HUD (bottom-left info panel) | N/A (no backend needed) | ✅ Done | ✅ Working |
| Join Existing Component flow | ✅ `POST /api/component/join` | ✅ Done | ✅ Working |
| Share component address UI | N/A | ✅ Done | ✅ Working |
| Wallet Balances | ✅ Fixed | ✅ Done | ✅ Fixed this session |
| Key Export / Backup Modal | ✅ `GET /api/wallet/export-keys` | ✅ Done | ✅ Working |
| BIP-39 Seed Words | ✅ `POST /api/wallet/import-mnemonic` | ✅ Done | ✅ Working |

### All Endpoints

| Endpoint | Status |
|----------|--------|
| `GET /api/status` | ✅ |
| `GET /api/wallets` | ✅ |
| `POST /api/wallet/create` | ✅ |
| `POST /api/wallet/import` | ✅ |
| `POST /api/wallet/passphrase` | ✅ |
| `POST /api/wallet/faucet` | ✅ |
| `GET /api/wallet/balance` | ✅ Fixed |
| `GET /api/wallet/export-keys` | ✅ |
| `POST /api/wallet/import-mnemonic` | ✅ |
| `POST /api/template/configure` | ✅ |
| `POST /api/template/publish` | ✅ |
| `POST /api/component/join` | ✅ |
| `POST /api/dm/send` | ✅ |
| `GET /api/dm/messages` | ✅ |
| `GET /api/dm/inbox` | ✅ |
| `POST /api/room/create` | ✅ |
| `POST /api/room/join` | ✅ |
| `POST /api/room/post` | ✅ |
| `GET /api/room/messages` | ✅ |
| `GET /api/rooms` | ✅ |
| `GET /api/contacts` | ✅ |
| `POST /api/contacts/set` | ✅ |
| `POST /api/demo/start` | ✅ |
| `POST /api/sync` | ✅ |
| `GET /api/debug` | ✅ |
| `GET /api/settings` | ✅ (added 2026-03-10) |
| `POST /api/settings` | ✅ (added 2026-03-10) |

### Indexer REST API Notes (for balance queries)

The Esmeralda indexer exposes `GET https://ootle-indexer-a.tari.com/substates/{id}`

**Account component JSON structure:**
```json
{
  "substate": {
    "Component": {
      "body": {
        "state": {
          "Map": [
            [{"Text": "vaults"}, {"Map": [
              [{"Tag":[131,{"Bytes":[1,1,...,1]}]},  // ResourceAddress tag=131
               {"Tag":[132,{"Bytes":[0e,f9,...]}]}]  // VaultId tag=132
            ]}]
          ]
        }
      }
    }
  }
}
```

**Vault substate JSON structure:**
```json
{
  "substate": {
    "Vault": {
      "resource_container": {
        "Stealth": {
          "address": "resource_0101...0101",
          "revealed_amount": "9889138",  // <-- STRING, not number!
          "locked_amount": "0"
        }
      }
    }
  }
}
```

### Session 2 (2026-03-10) — E2EE Encryption + Guide Rewrite

**Implemented E2EE encryption as an experimental feature:**

- Added `aes-gcm = "0.10"`, `hkdf = "0.12"`, `rand = "0.8"` to Cargo.toml
- Encryption protocol: Ristretto ECDH → HKDF-SHA256 → AES-256-GCM
- Wire format: `ENC1:<hex(nonce[12]||ciphertext)>`
- `derive_dm_key(my_sk_hex, my_pk_hex, their_pk_hex)` — ECDH + HKDF, sorted PKs for symmetry
- `encrypt_dm_content(...)` — encrypt on send (if enabled)
- `decrypt_dm_content(...)` — decrypt on read
- `try_decrypt_dm(wallets, dm)` — tries both sender and recipient wallet keys
- `AppState.encryption_enabled: bool` — persisted to state file
- `GET/POST /api/settings` endpoints for toggling encryption
- `handle_dm_send` now encrypts if enabled; `handle_dm_messages` auto-decrypts; inbox also decrypts previews
- Group chats: always plaintext (ECDH only works peer-to-peer)

**Frontend updates:**
- `S.encryptionEnabled` state field
- `toggleEncryption()` + `updateEncToggleUI()` functions
- Encryption toggle checkbox in wallet HUD (bottom-left)
- Lock icon 🔒 badge on encrypted messages in `renderDmMessages`
- Orange warning bar when in a group room with encryption enabled
- New "🔒 Encryption" tab in the How it Works tutorial modal
- Updated DM tab text and On-Chain tab's "Are messages private?" section
- `/api/settings` fetched on every `reload()` poll

**Guide rewrite:**
- `messaging_app/static/instructions.html` — fully rewritten (1000+ lines)
- Sticky nav bar linking to all sections
- New "Wallets" section covering all import methods
- New "Messaging" section (DMs, rooms, contacts, sync)
- New "Encryption" section with protocol diagram, wire format, limitations
- "Message Flow" updated to include encryption step
- "Architecture" updated for multi-wallet + encryption + new endpoints
- "Troubleshooting" expanded with encrypted message decryption issue

### Session 3 (2026-03-10) — Demo Design

**Created `DEMO.md` — full design specification for the new automated feature demo.**

Key decisions documented (not yet coded):
- 15 scenes covering every feature: wallet creation, faucet, publish, DM, chain confirmation, chain sync, enable encryption, send encrypted DM, raw ciphertext reveal, group room, contact naming, backup, balance, faucet refill, outro
- Demo state machine with blocking conditions (not just timers) — waits for real chain events
- Narrator panel alongside the live app UI — explains each step in plain English + code block
- Two delivery modes: dedicated `/demo` page (sharable link) + in-app modal overlay (replaces current demo button)
- Scene `onEnter`/`onExit` hooks drive API calls; `condition` functions block advance until chain confirms
- New backend endpoints needed: `POST /api/demo/reset`, expanded `POST /api/demo/start`
- New files needed: `demo.html`, `demo.js`, `demo.css`
- Total demo runtime: ~8–10 minutes (limited by testnet confirmation times)

**See `DEMO.md` for the complete specification.**

### Session 4 (2026-03-10) — Public Chats Tab + Onboarding

**Implemented:**
- **Public Chats tab** in the sidebar nav — 💬 Chats | 🌐 Public toggle
- **Pre-loaded public rooms**: "Tari Testnet General" + "Tari Developer Chat" — auto-shown in Public tab
- **Public room auto-join**: clicking a public room locally joins it (no TX), opens it, shows a red "Public channel" header bar
- **First-launch onboarding modal** — shows once (localStorage flag `tari_welcomed`)
  - Two choices: Quick Start (🌐) or Advanced Setup (⚙)
  - Privacy info always visible: what's on-chain, E2EE DMs, group plaintext, public key identity, private contract option
  - Quick Start flow: auto-joins public rooms, switches to Public tab, opens wallet creation
  - Advanced flow: opens template modal directly
- **Sidebar actions layout fix**: changed to 2×2 grid (was overflowing — "Settings" was cut off)
- **`/api/public-config` endpoint**: returns configured public component/template addresses + hardcoded room list
- **`/api/settings`** extended: now includes `setup_mode` (Simple/Advanced) — persisted in state
- **`PUBLIC_COMPONENT_ADDRESS` / `PUBLIC_TEMPLATE_ADDRESS` constants** in main.rs — set these after deploying a community component to enable cross-instance public chat

**Architecture note:**
Public rooms currently live on the user's own component. For true cross-instance public chat, set `PUBLIC_COMPONENT_ADDRESS` to a community-maintained component deployed on Esmeralda. Until then, users must share their component address to communicate.

**Feature table update:**

| Feature | Status |
|---|---|
| Public Chats tab | ✅ Done |
| Onboarding modal (first launch) | ✅ Done |
| Privacy disclosure | ✅ Done |
| Sidebar overflow fix (Settings button) | ✅ Done |
| Public component constant | ✅ Ready (empty until deployed) |

### Potential Next Steps (Backlog)

1. **Implement the demo** — build `demo.html` / `demo.js` / `demo.css` per `DEMO.md` spec
2. **`POST /api/demo/reset`** — allow demo replay without server restart
3. **Auto-refresh balance** — poll balance automatically after transactions
4. **Balance in wallet dropdown** — show µTARI next to wallet name
5. **Contacts page** — `/contacts` view to browse all known contacts and their keys
6. **Message notifications** — browser Notification API on new DM arrival
7. **Forward secrecy** — per-message ephemeral ECDH keys for the encryption layer
8. **Group E2EE** — sender-key scheme (Signal-style) for encrypted group rooms

---

### Group Chat E2EE — Design Ideas

Group encryption is hard because ECDH is two-party only. Three viable approaches:

#### Option A — Sender-Key (Signal-style) ✅ Recommended
Each sender generates a random 32-byte **group session key** the first time they post to a room.
They encrypt that key individually for every current room member using pairwise ECDH (same as DM encryption).
Subsequent messages in the room are encrypted with the group session key (AES-256-GCM).
New members need the sender's current session key re-sent to them before they can decrypt old messages.

- Wire format: `GRPENC1:<sender_pk_hex>:<hex(encrypted_session_key_for_recipient)>:<hex(nonce||ciphertext)>`
- Backend stores session keys per (room, sender_pk) — members fetch on join
- Rotating the session key on member leave provides **break-out forward secrecy**
- Complexity: medium. Needs a new `POST /api/room/session-key` endpoint and key distribution logic.

#### Option B — Shared Room Secret (simpler, weaker)
When a room is created, generate a random 32-byte room key.
The creator encrypts it for each invitee via ECDH and stores the encrypted blobs on-chain (in the component state) or off-chain (in the app state).
All messages use the same static room key.

- Simpler to implement — no per-sender state
- Weakness: no forward secrecy, compromised key exposes all history
- Wire format: `RMENC1:<hex(nonce||ciphertext)>`
- Only works for private/invite-only rooms, not open rooms

#### Option C — Pairwise Fan-out (brute force)
Sender encrypts the message separately for every room member using pairwise ECDH (same as DMs).
Stores N encrypted copies on-chain (one per member).

- Easiest to implement — reuses existing DM encryption exactly
- Scales badly: 10 members = 10x the on-chain storage and fees
- Works fine for small rooms (≤5 members), impractical beyond that

#### What Needs to Change (for Option A)
- **Template** (`messaging_template/`) — add `store_group_key(room_id, sender_pk, encrypted_key_blob)` and `get_group_keys(room_id)` methods
- **Backend** — `POST /api/room/session-key` to distribute a new session key; auto-called on first post when encryption enabled
- **Frontend** — key negotiation before first encrypted room post; re-key button for room admins
- **Wire format** — new `GRPENC1:` prefix so old clients degrade gracefully to showing raw ciphertext

---
*Generated by Claude Code sessions 2026-03-10*
