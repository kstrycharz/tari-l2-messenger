# Tari Messenger — Automated Demo Design

> **Status:** Design / specification only. No code written yet.
> **Purpose:** Replace the current minimal demo (which just creates two wallets and opens a DM) with a full guided, auto-advancing feature showcase. Designed for presentations, onboarding, and feature discovery.

---

## Overview

The demo is a **scripted, narrated, self-running walkthrough** embedded directly in the main app (`/demo` route or a dedicated `demo.html` page). It tells the story of **Ootle** and **Minotari** — two characters on opposite sides of a blockchain — and uses their conversation to demonstrate every feature of the app, live, against the real Esmeralda testnet.

The demo advances automatically (with optional manual overrides) and pauses at key moments to explain what just happened and why it matters technically.

---

## Guiding Narrative

> *Ootle and Minotari want to communicate. They don't trust email. They don't trust Signal's servers. They want their messages on a blockchain — cryptographically signed, immutable, and (optionally) encrypted so even the validators can't read them.*

Each demo scene corresponds to a real app feature. The characters drive the story; the feature explanations are shown in a side panel. By the end, the viewer has seen the entire app in action.

---

## Page Layout

```
┌──────────────────────────────────────────────────────────┐
│  DEMO HEADER  (title, progress bar, scene name, skip)    │
├───────────────────────────┬──────────────────────────────┤
│                           │                              │
│   LIVE APP PANEL          │   NARRATOR PANEL             │
│   (the actual app UI,     │   (scene title, explanation, │
│    running and reacting)  │    code block, tech detail,  │
│                           │    "What just happened?")    │
│                           │                              │
├───────────────────────────┴──────────────────────────────┤
│  [◀ Prev]  [⏸ Pause]  [▶ Next]  [↺ Restart]  [✕ Exit]  │
└──────────────────────────────────────────────────────────┘
```

- **Left:** an `<iframe>` (or direct embed) of the live app UI, with the relevant element highlighted or auto-animated by the demo controller
- **Right:** a narrator card that changes with each scene — contains the scene title, a plain-English explanation, a relevant code block (the actual on-chain WASM call), and a "What just happened?" technical note
- **Bottom bar:** navigation controls; the demo auto-advances after a configurable delay (e.g. 8 seconds); the progress bar counts down

---

## Demo Entry Points

### 1. Welcome Screen Button (replaces current "🧪 Try Demo" button)
Clicking the demo button on the welcome screen now launches the new demo mode instead of the old two-step silent setup.

### 2. Direct URL: `/demo`
A dedicated route (`GET /demo`) serves `demo.html`, which can be shared as a standalone link. This page is self-contained — it bootstraps the demo automatically.

### 3. Demo Reset
A `POST /api/demo/reset` endpoint (new — not yet implemented) clears only the demo wallets (Ootle and Minotari) from state and re-runs the bootstrap, allowing the demo to be replayed without restarting the server.

---

## Feature Pages (Scenes)

Each scene is a discrete unit: it has a setup action, a visual result in the app, and a narrator explanation. Scenes are numbered and linked; the progress bar shows position.

---

### Scene 0 — Intro
**Auto-duration:** 5 seconds

**What happens in the app:**
- The welcome screen is visible, centered, no wallets yet
- Demo title card fades in over the app

**Narrator:**
> *"Every message in this app is a real blockchain transaction on Tari's Layer 2 network. No servers, no databases, no accounts — just cryptographic keys and a WASM smart contract. Let's watch it happen live."*

**Technical note:** Nothing yet. This is the blank state.

---

### Scene 1 — Wallet Creation
**Feature:** Generate new wallets (Ootle & Minotari)
**Auto-duration:** 60–90 seconds (waits for faucet)

**What happens in the app:**
1. The "Add Wallet" modal opens automatically
2. The "Generate" tab is highlighted
3. "Ootle" is typed into the name field
4. "Generate & Fund" is clicked — the UI shows the spinner
5. After ~30s: Ootle appears in the wallet dropdown
6. The flow repeats for Minotari (the demo creates both)
7. Both wallets show in the dropdown; the wallet HUD appears

**Narrator title:** `Creating Wallets — Your Identity is a Key`

**Narrator body:**
> Each "wallet" is a fresh Ristretto elliptic-curve key pair generated on the fly. The account secret key is 32 random bytes. The public key (64-char hex) derived from it is your only identity on the Tari network — no username, no email, no password.

**Code block:**
```
OotleSecretKey::random(Network::Esmeralda)
→ account_secret_key: [32 random bytes]
→ view_secret_key:    [32 random bytes]
→ public_key: RistrettoPublicKey::from_secret_key(&sk)
```

**What just happened?**
> Two keypairs were generated locally. Neither the server nor the blockchain knows who these wallets belong to. The only link between a key and a person is the display name stored in your local state file.

---

### Scene 2 — Faucet Funding
**Feature:** Get testnet tTARI
**Auto-duration:** runs during scene 1 (faucet is part of wallet creation); shown as a separate pause after wallets appear

**What happens in the app:**
- The wallet HUD shows "Balance: —"
- "↺ Balance" is clicked automatically
- After the balance fetch: "Balance: 10.00 tTARI" appears in green for both wallets

**Narrator title:** `Free Testnet Funds — No Real Money`

**Narrator body:**
> The Esmeralda faucet is an on-chain smart contract that dispenses free test tTARI to any account that asks. The app submitted a signed transaction to the faucet contract requesting 10 tTARI. The faucet verified the request and transferred tokens to each account's vault.

**Code block:**
```
IFaucet::new(&provider)
  .take_faucet_funds(10 * TARI)  // 10,000,000 µTARI
  .pay_fee(500u64)
  .prepare().await
```

**What just happened?**
> 10 tTARI landed in Ootle's account vault on-chain. The vault ID was queried from the Esmeralda indexer REST API and the balance read from the component's CBOR-encoded state.

---

### Scene 3 — Publishing the Template
**Feature:** Auto-publish WASM smart contract
**Auto-duration:** 90–120 seconds

**What happens in the app:**
- The yellow "NeedsTemplate" banner is highlighted with a pulsing ring
- The Settings modal opens to the "Deploy New" tab
- "Auto-Publish WASM" is clicked
- The banner changes to the spinning "Deploying…" state
- After ~90s: the banner disappears; the "Ready" state is reached
- The share panel appears in Settings showing the component address

**Narrator title:** `Publishing the Smart Contract — Code on the Blockchain`

**Narrator body:**
> The messaging logic lives in a Rust program compiled to WebAssembly (WASM). Publishing uploads this ~100 KB binary to the Tari network permanently. Validators store and execute it. Every future message call runs this exact code — no one can modify it after publishing.

**Code block:**
```rust
// messaging_template/src/lib.rs (simplified)
#[template]
mod messaging {
  pub struct Messaging {
    msg_from:    Vec<String>,
    msg_to:      Vec<String>,
    msg_content: Vec<String>,
  }
  impl Messaging {
    pub fn send_dm(&mut self, to: String, content: String) {
      let from = CallerContext::transaction_signer_public_key();
      self.msg_from.push(from.to_string());
      self.msg_to.push(to);
      self.msg_content.push(content);
    }
  }
}
```

**What just happened?**
> The WASM binary was wrapped in a `publish_template` transaction instruction, signed by Ootle's key, and submitted to the indexer. Validators stored it and returned a template address. Then a second transaction called `Messaging::new()` to deploy a component (an instance of that template) with its own on-chain state.

---

### Scene 4 — Sending a Plaintext DM
**Feature:** Direct message (unencrypted)
**Auto-duration:** 20 seconds after send (waits for optimistic local render)

**What happens in the app:**
- Minotari's wallet is active (Ootle is the sender, so switch to Ootle)
- "✉ New DM" is clicked; Minotari's public key is pasted automatically
- The conversation opens
- Ootle "types" a message: "Hey Minotari! This message is going to the blockchain."
- The message appears immediately in the chat (local cache)
- A subtle "sending…" indicator appears under the message
- After ~30-60s: the debug panel's transaction list shows a new "DM Sent" entry with status Committed

**Narrator title:** `Sending a Direct Message — Every Word is a Transaction`

**Narrator body:**
> Pressing Enter triggers a POST to `/api/dm/send`. The server saves the message locally (for instant display), then builds a blockchain transaction in a background thread. The transaction calls `send_dm(to_pubkey, content)` on the deployed messaging component, signed with Ootle's private key.

**Code block:**
```
POST /api/dm/send
{
  "from_pubkey": "3a8f2c...",
  "to_pubkey":   "b7e91d...",
  "content":     "Hey Minotari! This message is going to the blockchain."
}

→ on-chain: component.send_dm("b7e91d...", "Hey Minotari!...")
→ from: CallerContext::transaction_signer_public_key() // VERIFIED by network
→ fee: 2,000 µTARI
```

**What just happened?**
> The message is permanently on the Tari Layer 2 blockchain. The sender (`from`) was set by the blockchain itself from the transaction signature — Ootle cannot lie about who sent it, and no one can forge a message from her.

---

### Scene 5 — Chain Confirmation & Sync
**Feature:** Background chain sync + debug panel
**Auto-duration:** 15 seconds

**What happens in the app:**
- The debug panel (🔍) opens automatically
- The "Transactions" tab is active
- The "DM Sent" transaction from Scene 4 now shows as `Committed` with a fee and TX ID
- The "Network" tab is clicked automatically — shows indexer URL, component address, fees spent
- The panel closes

**Narrator title:** `Chain Confirmation — Watching the Transaction Confirm`

**Narrator body:**
> The debug panel shows every on-chain transaction the app has made: type, status, fee paid, and a transaction ID you can look up on a block explorer. The "Network" tab shows the live connection to the Esmeralda testnet and the on-chain addresses of the published template and deployed component.

**What just happened?**
> The indexer returned the transaction receipt. Status changed from "pending" to "Committed". The transaction ID is permanent proof that this message exists on the blockchain.

---

### Scene 6 — Receiving a Message (Chain Sync)
**Feature:** Minotari replies; Ootle's view syncs from chain
**Auto-duration:** 60 seconds

**What happens in the app:**
- Switch to Minotari's wallet
- Open the DM with Ootle
- Minotari types: "Received loud and clear! This reply came from my key."
- The message appears in Minotari's chat
- Switch back to Ootle's wallet
- Ootle's inbox shows Minotari as a new conversation with an unread badge
- The auto-sync (every 10s) picks up Minotari's reply and renders it

**Narrator title:** `Receiving Messages — Auto-Sync from the Indexer`

**Narrator body:**
> The app polls the Esmeralda indexer every 10 seconds. It reads the messaging component's full on-chain state, compares against the locally known `synced_event_ids`, and imports any new messages. Inbox badges update automatically — no push notifications required, just polling the chain.

**What just happened?**
> The indexer returned Minotari's message from the component's state. The server decoded the CBOR-encoded state, found a new `msg_id` not yet in `synced_event_ids`, and added it to the local DM list. Ootle's inbox updated on the next poll.

---

### Scene 7 — Enabling E2EE Encryption
**Feature:** Encryption toggle
**Auto-duration:** 8 seconds

**What happens in the app:**
- Switch to Ootle's wallet HUD (bottom-left panel is visible)
- A highlight ring pulses around the "🔒 E2EE Encryption (experimental)" toggle
- The checkbox is clicked — it turns green
- The label updates to active green state
- A brief flash confirmation: "Encryption enabled"

**Narrator title:** `Enabling End-to-End Encryption — Experimental`

**Narrator body:**
> Toggling encryption calls `POST /api/settings` with `{"encryption_enabled": true}`. From this point on, all new DMs are encrypted before being stored on-chain. The encryption uses your wallet's existing Ristretto private key — no separate encryption keys needed.

**Code block:**
```
POST /api/settings
{ "encryption_enabled": true }

Encryption protocol:
  shared_point = recipient_pk × sender_sk     (ECDH)
  key = HKDF-SHA256(shared_point, info=...)   (Key derivation)
  content = AES-256-GCM.encrypt(key, nonce)   (Symmetric encrypt)
  stored as: "ENC1:<hex(nonce||ciphertext)>"
```

**What just happened?**
> The setting is persisted to `messaging-state.json`. The toggle state is synced on every app reload. The next DM sent will be encrypted. The key used is derived on-the-fly from the conversation participants' keys — no key exchange needed ahead of time.

---

### Scene 8 — Sending an Encrypted DM
**Feature:** Encrypted message (E2EE)
**Auto-duration:** 20 seconds

**What happens in the app:**
- Ootle's DM with Minotari is open
- Ootle types: "This message is encrypted. Only you can read it, Minotari."
- Message appears with a 🔒 lock badge
- The conversation preview in the sidebar also shows the lock badge

**Narrator title:** `Encrypted Message — Ciphertext on the Blockchain`

**Narrator body:**
> Before the content reaches the blockchain, the server encrypts it using Ootle's private key and Minotari's public key. The ciphertext is what gets stored on-chain. Even a validator node or blockchain explorer can only see the `ENC1:...` blob — the plaintext is never on the chain.

**Code block:**
```
On-chain stored content:
"ENC1:a3f2b89c1d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0"

Decryption (Minotari's side):
  shared_point = sender_pk × minotari_sk
  key = HKDF-SHA256(shared_point, sorted_pks)
  plaintext = AES-256-GCM.decrypt(key, nonce, ciphertext)
  → "This message is encrypted. Only you can read it, Minotari."
```

**What just happened?**
> The `ENC1:` prefix signals to the server that this is an encrypted message. When Minotari's wallet is loaded, `try_decrypt_dm()` finds Minotari's private key in `AppState.wallets`, derives the same shared ECDH secret, and decrypts. The UI shows the lock badge.

---

### Scene 9 — Verifying Encryption (The "Raw Chain" Reveal)
**Feature:** Show raw ciphertext vs decrypted plaintext
**Auto-duration:** 15 seconds

**What happens in the app:**
- A split-view panel appears in the narrator area (not the app itself)
- Left side: raw value as it exists on-chain — `ENC1:a3f2b89c...`
- Right side: decrypted plaintext — "This message is encrypted. Only you can read it, Minotari."
- An arrow between the two with the label "your wallet key → AES-256-GCM decrypt"

**Narrator title:** `What the Blockchain Actually Stores`

**Narrator body:**
> This is the exact byte string written to the component's state on-chain. A blockchain observer, validator, or indexer sees only the `ENC1:` ciphertext. Without Ootle's or Minotari's private key, there is no way to derive the shared secret and decrypt it.

**What just happened?**
> This is the core of the E2EE feature. The blockchain is a public ledger — but ciphertext on a public ledger is still private, as long as the keys are kept safe.

---

### Scene 10 — Group Room (Plaintext)
**Feature:** Create a group room + encryption warning
**Auto-duration:** 30 seconds

**What happens in the app:**
- "👥 New Group" is clicked
- Room ID "demo-room" is typed, display name "Demo Room"
- "Create Room" is clicked — transaction fires in background
- The room appears in the sidebar
- Ootle opens the room
- The orange warning bar appears: "Encryption is enabled but group chats are always plaintext"
- Ootle posts: "Group rooms are always public — ECDH only works between two parties."
- The message appears without a lock badge

**Narrator title:** `Group Rooms — Always Plaintext`

**Narrator body:**
> Group rooms cannot be encrypted with ECDH because there is no single shared secret that works for an arbitrary number of participants. ECDH is strictly a two-party protocol. Encrypting group messages would require a more complex key management scheme (e.g. group keys, sender keys). This is a known limitation.

**What just happened?**
> The app detected that the active conversation is a room (not a DM) and automatically bypassed the encryption path. The orange warning bar is shown as a reminder that this conversation is public.

---

### Scene 11 — Contact Naming
**Feature:** Assign a nickname to a contact
**Auto-duration:** 10 seconds

**What happens in the app:**
- Ootle's DM with Minotari is open
- The "✏️ Set Name" button in the chat header is highlighted
- It is clicked; a prompt fires with the current display as "b7e91d…" (Minotari's raw PK)
- "Minotari" is typed and confirmed
- The chat header updates: name shows "Minotari"
- The sidebar conversation list also updates to show "Minotari" instead of the truncated key

**Narrator title:** `Contact Names — Local Nicknames, Never On-Chain`

**Narrator body:**
> Contact names are stored only in your local state file. The blockchain never sees them — it only knows public keys. This is why the display name field in wallet creation is also local only. You can rename any contact at any time without any on-chain transaction.

**What just happened?**
> `POST /api/contacts/set` wrote `{"public_key_hex": "...", "display_name": "Minotari"}` to `AppState.contacts` and saved the state file. All name renders in the UI now use this mapping.

---

### Scene 12 — Wallet Backup & Seed Words
**Feature:** Export keys, BIP-39 mnemonic
**Auto-duration:** 12 seconds

**What happens in the app:**
- The wallet HUD is focused; "🔑 Backup" is clicked
- The backup modal opens — Ootle's full public key, account address, hex private keys, and seed words are shown
- The seed words section is highlighted with a ring: "word1 word2 word3 … word24"
- A note appears: "These 24 words are your private key encoded as English words"
- The modal closes

**Narrator title:** `Wallet Backup — Your Keys, Your Responsibility`

**Narrator body:**
> The BIP-39 seed words are your private key encoded as 24 English words from a standard 2048-word dictionary. The same words always produce the same key — you can restore a wallet on any compatible app by typing them. There are two sets: one for the account key (signs transactions) and one for the view key.

**What just happened?**
> `GET /api/wallet/export-keys` returned the raw hex private keys and the app decoded them to BIP-39 using the `bip39` crate. Nothing was sent to the network. This is entirely local.

---

### Scene 13 — Balance Refresh
**Feature:** Check tTARI balance via indexer
**Auto-duration:** 15 seconds

**What happens in the app:**
- Ootle's wallet HUD is focused
- "↺ Balance" is clicked
- A brief loading state ("Checking...")
- The balance updates: "9.99 tTARI" (slightly less than 10 due to fees spent)
- A tooltip or note in the narrator: "Started at 10 tTARI — spent ~0.01 on transactions"

**Narrator title:** `Live Balance — Reading from the Blockchain`

**Narrator body:**
> The balance is fetched directly from the Esmeralda indexer. The server reads the account component's state, finds the vault holding tTARI (identified by a CBOR `Tag[132, Bytes[...]]` node), queries that vault's substate, and extracts the `revealed_amount` string field. No wallet daemon required.

**Code block:**
```
GET https://ootle-indexer-a.tari.com/substates/{account_component_id}
→ parse CBOR: find Tag[132, Bytes[vault_id_bytes]]
GET https://ootle-indexer-a.tari.com/substates/vault_{hex}
→ parse: substate.Vault.resource_container.Stealth.revealed_amount
→ "9990000" µTARI → 9.99 tTARI
```

**What just happened?**
> The indexer REST API returned the raw CBOR-decoded JSON of the account component. The vault ID was extracted from `Tag[132, Bytes[...]]`, then the vault substate was queried separately to get the balance as a string.

---

### Scene 14 — Faucet Refill
**Feature:** Get more tTARI mid-session
**Auto-duration:** 60 seconds

**What happens in the app:**
- "💧 Faucet" is clicked in the wallet HUD
- A "Requesting…" indicator appears
- After ~30s: the balance automatically refreshes to ~19.99 tTARI

**Narrator title:** `Faucet — Refilling Testnet Tokens`

**Narrator body:**
> The faucet is itself a smart contract on the Tari network. Any account can call it to receive free tTARI. The app submits a signed transaction to the faucet component, which mints tTARI and deposits it to the requester's vault. There is no rate limit enforced in the demo — though the real faucet may apply limits.

**What just happened?**
> A `take_faucet_funds` transaction was submitted using the `IFaucet` interface from `ootle-rs`. The receipt confirmed the deposit. The balance now reflects the refill.

---

### Scene 15 — Outro & Invitation
**Feature:** End card
**Auto-duration:** Manual (stays until dismissed)

**What happens in the app:**
- The demo overlay fades out, leaving the fully set-up app visible
- Both Ootle and Minotari are active in the wallet dropdown
- The full conversation history (plaintext + encrypted) is visible
- The narrator panel shows the closing card

**Narrator title:** `That's Tari Messenger`

**Narrator body:**
> You just watched every feature run live against the real Esmeralda testnet. Both wallets are funded and active. The conversation history is preserved. You can keep chatting, add more wallets, join existing components from other users, or read the full technical guide.

**Buttons:**
- `✉ Start Chatting` — closes demo, opens Ootle's DM with Minotari
- `📖 Full Guide` — opens `/instructions`
- `↺ Replay Demo` — resets and restarts from Scene 0
- `✕ Exit` — closes demo, returns to normal app

---

## Demo State Machine

```
IDLE
  ↓ user clicks "🧪 Demo" or visits /demo
BOOTSTRAPPING
  → POST /api/demo/start (creates Ootle + Minotari wallets, funds them)
  → waits for setup_status == "Ready"
  ↓
SCENE_0 [Intro]           → auto-advance after 5s
SCENE_1 [Wallet Creation] → wait for wallets to appear in state
SCENE_2 [Faucet]          → wait for balance to be non-zero
SCENE_3 [Publish]         → wait for setup_status == "Ready"
SCENE_4 [Send DM]         → auto-advance 20s after send
SCENE_5 [Confirmation]    → auto-advance after debug panel shown
SCENE_6 [Receive]         → wait for Minotari's reply to appear
SCENE_7 [Enable Enc]      → auto-advance 8s after toggle
SCENE_8 [Enc DM]          → auto-advance 20s after send
SCENE_9 [Raw Reveal]      → auto-advance 15s
SCENE_10 [Group Room]     → auto-advance 30s
SCENE_11 [Contact Name]   → auto-advance 10s
SCENE_12 [Backup]         → auto-advance 12s
SCENE_13 [Balance]        → auto-advance 15s
SCENE_14 [Faucet]         → wait for balance refresh
SCENE_15 [Outro]          → manual dismiss
  ↓
DONE
```

Each scene transition is driven by a **condition check** (not just a timer). If the condition isn't met within a timeout, the demo shows a "waiting…" indicator rather than advancing with a broken state.

---

## Demo Controller Design

### JavaScript (`demo.js`)
A standalone script loaded only on the demo page. It does not modify `index.html`. It:
- Maintains a `DEMO_STATE` object: `{ scene, paused, autoAdvanceTimer, sceneMeta[] }`
- Calls real app API endpoints (the demo is not faked — it's the real app)
- Controls the app via API calls (not DOM manipulation of the iframe — use `postMessage` or direct API calls)
- Uses `MutationObserver` or polling to detect when conditions are met (e.g. wallet appears, balance non-zero)
- Renders the narrator panel using a `SCENES[]` array (one object per scene with title, body, code, duration)
- Provides `advance()`, `back()`, `pause()`, `restart()` methods

### Scene Condition Functions
Each scene has an optional `condition: async () => boolean` field. The controller waits up to `timeout` ms for it to resolve true before auto-advancing. Examples:

```javascript
SCENES[1].condition = async () => {
  const d = await fetch('/api/wallets').then(r=>r.json());
  return d.wallets.length >= 2;
};

SCENES[3].condition = async () => {
  const d = await fetch('/api/status').then(r=>r.json());
  return d.setup_status === 'Ready';
};

SCENES[6].condition = async () => {
  // Minotari's reply has arrived
  const d = await fetch(`/api/dm/messages?user_a=${OOTLE_PK}&user_b=${MINOTARI_PK}`).then(r=>r.json());
  return (d.messages || []).some(m => m.from_pk === MINOTARI_PK);
};
```

### Action Functions
Each scene has optional `onEnter: async () => void` and `onExit: async () => void` hooks. Examples:

```javascript
SCENES[7].onEnter = async () => {
  // Enable encryption via API
  await fetch('/api/settings', {method:'POST', body:JSON.stringify({encryption_enabled:true}),...});
};

SCENES[11].onEnter = async () => {
  // Set Minotari's contact name
  await fetch('/api/contacts/set', {method:'POST', body:JSON.stringify({public_key_hex: MINOTARI_PK, display_name:'Minotari'}),...});
};
```

### Highlight System
The demo controller uses `postMessage` to the app frame (or direct DOM if same-origin) to:
- Add a CSS class `demo-highlight` to specific elements (wallet HUD, toggle, chat header)
- Remove it when the scene exits
- A CSS animation in `demo.css` handles the pulsing ring

---

## New/Modified Backend Requirements

### `POST /api/demo/reset` (new endpoint — not yet implemented)
Resets demo state: removes wallets named "Ootle" and "Minotari", clears their DMs and rooms, resets encryption setting to false. Does NOT clear other wallets or their data. Allows demo replay without server restart.

### `POST /api/demo/start` (existing, needs expansion)
Currently creates wallets and funds them. Needs to also:
1. Set Minotari as a known contact of Ootle with display name "Minotari" (and vice versa)
2. Return both wallet public keys in the response so the demo controller can use them
3. Optionally send an initial "seed" DM from Ootle to Minotari so the conversation pre-exists

### Demo Route: `GET /demo`
A new route in `main.rs` that serves `demo.html` from the static directory. The Axum router adds:
```
.route("/demo", get(serve_demo))
```
`serve_demo` reads `static/demo.html` and returns it as HTML.

---

## File Structure (to be created)

```
messaging_app/static/
  demo.html          — demo page shell (layout, progress bar, narrator panel)
  demo.js            — demo controller (state machine, scene definitions, API calls)
  demo.css           — demo-specific styles (highlight ring, narrator card, progress bar)
```

`demo.html` embeds the app either:
- **Option A (iframe):** `<iframe src="/" id="app-frame">` — cleanest separation; uses `postMessage` for coordination
- **Option B (same page):** Include the app UI directly in `demo.html` and load `demo.js` alongside `main.js` — simpler API access, harder to isolate

**Recommended: Option A** (iframe). Keeps the demo controller and app code fully independent. The demo controller only communicates via API calls and `postMessage`.

---

## Scene Timing Reference

| Scene | Feature | Min Duration | Blocking Condition |
|-------|---------|-------------|-------------------|
| 0 | Intro | 5s | None |
| 1 | Wallet Creation | ~60s | 2 wallets in `/api/wallets` |
| 2 | Faucet Balance | ~10s | Balance > 0 for both wallets |
| 3 | Publish Template | ~90s | `setup_status == "Ready"` |
| 4 | Send DM | ~20s | Message appears in local DM list |
| 5 | Chain Confirmation | ~15s | TX in debug panel as Committed |
| 6 | Receive DM | ~60s | Minotari's reply in DM thread |
| 7 | Enable Encryption | 8s | Settings returns `encryption_enabled: true` |
| 8 | Send Encrypted DM | ~20s | Encrypted message with lock badge |
| 9 | Raw Reveal | 15s | None (narrator only) |
| 10 | Group Room | ~30s | Room created + warning bar visible |
| 11 | Contact Name | 10s | None |
| 12 | Backup | 12s | None |
| 13 | Balance | ~15s | Balance updated from indexer |
| 14 | Faucet Refill | ~60s | Balance increases |
| 15 | Outro | Manual | User dismisses |

**Total minimum run time:** ~8–10 minutes (mostly waiting for chain confirmations)

---

## Progress Bar Design

The progress bar at the top of the demo has two components:
1. **Scene progress:** dots or segments (15 total), current scene highlighted
2. **Scene timer:** a CSS `@keyframes` countdown bar that fills from right-to-left over the `minDuration` of the current scene — resets when the scene's blocking condition triggers early

Scene names appear on hover over each segment:
`[●]─[●]─[●]─[○]─[○]─[○]─[○]─[○]─[○]─[○]─[○]─[○]─[○]─[○]─[○]`
`  0    1    2    3    4   ...`

---

## Narrator Panel Component

The narrator panel renders from the current scene's metadata object:

```javascript
{
  id: 8,
  title: "Encrypted Message — Ciphertext on the Blockchain",
  body: `Before the content reaches the blockchain...`,
  code: `On-chain stored content:\n"ENC1:a3f2b89c..."`,
  techNote: `The ENC1: prefix signals to the server...`,
  tag: "🔒 E2EE",         // shown as a badge next to the title
  tagColor: "green",
  minDuration: 20000,
  condition: async () => { ... },
  onEnter: async () => { ... },
}
```

Rendering:
- Title + badge at top
- Body text in muted color with strong/code highlighting
- Code block in monospace with left border accent
- "What just happened?" collapsible section (tech note)
- Scene number out of 15 in the top-right corner

---

## Accessibility & Robustness

- If the demo runs into an error (network timeout, API failure), show an error card in the narrator panel with a "Retry this step" button — don't crash the whole demo
- If the testnet is slow, the waiting indicator shows elapsed time: "Waiting for chain… (45s)"
- All auto-advancing has a "Skip" button that bypasses the condition check and moves to the next scene immediately (with a warning if the condition wasn't met)
- The demo never modifies production wallets. It only creates wallets named "Ootle" and "Minotari", and all demo actions are via the standard API

---

## Demo Page vs. In-App Modal

Two delivery options:

### Option A — Dedicated Page (`/demo`)
- Separate URL, full-page layout
- Best for sharing as a link, presentations, or onboarding new users
- The app and narrator panel sit side by side
- Demo resets cleanly when you navigate to `/demo` fresh

### Option B — In-App Modal Overlay
- Launched from the "🧪 Demo" button on the welcome screen
- The narrator panel slides in from the right (like the debug panel does now)
- The app is directly controlled (no iframe)
- Less isolation but simpler implementation

**Recommendation: implement both.** Option B for the in-app experience (replacing the current demo button), Option A for the sharable link. The demo controller (`demo.js`) can work in either context with a `DEMO_MODE: 'page' | 'modal'` flag.

---

## Summary of Changes Required (Not Yet Coded)

| Item | Type | File(s) |
|------|------|---------|
| `demo.html` | New file | `messaging_app/static/demo.html` |
| `demo.js` | New file | `messaging_app/static/demo.js` |
| `demo.css` | New file | `messaging_app/static/demo.css` |
| `GET /demo` route | Backend | `messaging_app/src/main.rs` |
| `POST /api/demo/reset` | Backend | `messaging_app/src/main.rs` |
| `POST /api/demo/start` expansion | Backend | `messaging_app/src/main.rs` |
| Replace welcome screen demo button | Frontend | `messaging_app/static/index.html` |

None of the above has been implemented. This document is the design specification only.

---

*Demo design by Claude Code — 2026-03-10*
*See PLAN.md for implementation session notes.*
