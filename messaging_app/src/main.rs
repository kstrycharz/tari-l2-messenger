mod want_list;

use axum::{
    Router,
    extract::{Query, State as AxumState},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
};
use ootle_rs::{
    ToAccountAddress, TransactionRequest,
    builtin_templates::{UnsignedTransactionBuilder, faucet::IFaucet},
    key_provider::PrivateKeyProvider,
    keys::{HasViewOnlyKeySecret, OotleSecretKey},
    provider::{IndexerProvider, Provider, ProviderBuilder, WalletProvider},
    wallet::OotleWallet,
};
use serde::{Deserialize, Serialize};
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::Aead,
};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::{Sha256, Digest};
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};
use tari_crypto::{
    keys::PublicKey,
    ristretto::{RistrettoPublicKey, RistrettoSecretKey},
};
use tari_ootle_common_types::{
    Network,
    engine_types::transaction_receipt::TransactionReceipt,
};
use tari_ootle_transaction::{TransactionBuilder, args};
use tari_template_lib_types::{
    ComponentAddress, TemplateAddress,
    constants::{TARI, TARI_TOKEN},
};
use tari_utilities::{ByteArray, hex::Hex};
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

use want_list::WantList;

// ── Configuration ─────────────────────────────────────────────────────────────

const NETWORK: Network = Network::Esmeralda;
const INDEXER_URL: &str = "https://ootle-indexer-a.tari.com/";
const WASM_PATH: &str = "../messaging_template/target/wasm32-unknown-unknown/release/messaging_template.wasm";

/// Pre-deployed public Esmeralda community component.
/// Deploy the messaging_template once, paste the addresses here, and recompile.
/// These are permanent — the same contract will always serve the public test chat.
/// Leave empty to degrade gracefully (public rooms visible but posting requires own component).
const PUBLIC_COMPONENT_ADDRESS: &str = "component_940c5f7543fa545777299b4ea14406f14020b5cc9dc9d978cf24dca0eaa8a880";
const PUBLIC_TEMPLATE_ADDRESS: &str  = "template_ba8e472e8d37fc778a9a59cc915a381c80213977c6d66a19d3fd90e9dc500f41";

/// Room IDs that are served by PUBLIC_COMPONENT_ADDRESS.
/// Must match the room_ids returned by /api/public-config.
const PUBLIC_ROOM_IDS: &[&str] = &["tari-messenger-test-chat"];

/// Returns the parsed public component address, if configured.
fn public_component_addr() -> Option<ComponentAddress> {
    if PUBLIC_COMPONENT_ADDRESS.is_empty() {
        return None;
    }
    parse_component_address(PUBLIC_COMPONENT_ADDRESS).ok()
}

/// Returns true if room_id is one of the globally-shared public rooms.
fn is_public_room_id(room_id: &str) -> bool {
    PUBLIC_ROOM_IDS.contains(&room_id)
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
struct UserConfig {
    display_name: String,
    account_secret_hex: String,
    view_secret_hex: String,
    account_address: ComponentAddress,
    /// Ristretto public key hex — the user's on-chain identity
    public_key_hex: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct LocalDm {
    from_pk: String,
    to_pk: String,
    content: String,
    timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone)]
struct LocalRoomMsg {
    room_id: String,
    from_pk: String,
    content: String,
    timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone)]
struct LocalRoom {
    room_id: String,
    display_name: String,
    creator_pk: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct TxRecord {
    /// Full transaction ID (hash hex)
    id: String,
    tx_type: String,
    status: String,
    /// Fee paid in micro-TARI
    fee: u64,
    timestamp: u64,
    /// Sender public key (if applicable)
    from_pk: Option<String>,
    /// Recipient public key or room_id (if applicable)
    to_id: Option<String>,
    /// Message content snippet (first 80 chars)
    content_preview: Option<String>,
    /// On-chain component address involved
    component: Option<String>,
    /// Network (always Esmeralda for now)
    network: String,
    /// Indexer explorer link (not yet available — placeholder)
    explorer_url: Option<String>,
}

impl TxRecord {
    fn new(id: impl Into<String>, tx_type: impl Into<String>, fee: u64) -> Self {
        Self {
            id: id.into(),
            tx_type: tx_type.into(),
            status: "Committed".into(),
            fee,
            timestamp: now_secs(),
            from_pk: None,
            to_id: None,
            content_preview: None,
            component: None,
            network: "Esmeralda".into(),
            explorer_url: None,
        }
    }

    fn with_from(mut self, pk: impl Into<String>) -> Self {
        self.from_pk = Some(pk.into());
        self
    }

    fn with_to(mut self, to: impl Into<String>) -> Self {
        self.to_id = Some(to.into());
        self
    }

    fn with_content(mut self, content: &str) -> Self {
        let preview = if content.len() > 80 {
            format!("{}…", &content[..80])
        } else {
            content.to_string()
        };
        self.content_preview = Some(preview);
        self
    }

    fn with_component(mut self, addr: impl Into<String>) -> Self {
        self.component = Some(addr.into());
        self
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
enum SetupMode {
    #[default]
    NotChosen,
    Simple,
    Advanced,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
enum SetupStatus {
    #[default]
    NeedsWallet,
    NeedsTemplate,
    DeployingComponent,
    Ready,
    Error(String),
}

#[derive(Serialize, Deserialize, Default)]
struct AppState {
    /// Path to the state file — set at runtime, not persisted.
    #[serde(skip)]
    state_file: String,
    /// All wallets, keyed by public_key_hex
    #[serde(default)]
    wallets: HashMap<String, UserConfig>,
    /// Stored as "template_<hex>"
    template_address: Option<String>,
    component_address: Option<ComponentAddress>,
    #[serde(default)]
    dms: Vec<LocalDm>,
    #[serde(default)]
    room_msgs: Vec<LocalRoomMsg>,
    #[serde(default)]
    rooms: Vec<LocalRoom>,
    #[serde(default)]
    tx_history: Vec<TxRecord>,
    #[serde(default)]
    setup_status: SetupStatus,
    /// Event IDs already synced from chain (prevents duplicate messages from polling)
    #[serde(default)]
    synced_event_ids: Vec<String>,
    /// Contact nicknames — maps public_key_hex to a user-set display name
    #[serde(default)]
    contacts: HashMap<String, String>,
    /// Whether E2EE encryption is enabled for DMs (experimental feature)
    #[serde(default)]
    encryption_enabled: bool,
    /// Whether the user has chosen Simple or Advanced setup mode
    #[serde(default)]
    setup_mode: SetupMode,
}

type SharedState = Arc<RwLock<AppState>>;

// ── State persistence ─────────────────────────────────────────────────────────

fn load_state(path: &str) -> AppState {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_state(state: &AppState, path: &str) {
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(path, json);
    }
}

// ── Wallet helpers ────────────────────────────────────────────────────────────

fn make_wallet(user: &UserConfig) -> anyhow::Result<OotleWallet> {
    let acc_bytes = Vec::from_hex(&user.account_secret_hex)
        .map_err(|e| anyhow::anyhow!("Invalid account secret hex: {e}"))?;
    let view_bytes = Vec::from_hex(&user.view_secret_hex)
        .map_err(|e| anyhow::anyhow!("Invalid view secret hex: {e}"))?;
    let acc_sk = RistrettoSecretKey::from_canonical_bytes(&acc_bytes)
        .map_err(|e| anyhow::anyhow!("Invalid account key: {e}"))?;
    let view_sk = RistrettoSecretKey::from_canonical_bytes(&view_bytes)
        .map_err(|e| anyhow::anyhow!("Invalid view key: {e}"))?;
    let secret = OotleSecretKey::new(NETWORK, acc_sk, view_sk);
    Ok(OotleWallet::from(PrivateKeyProvider::new(secret)))
}

async fn make_provider(user: &UserConfig) -> anyhow::Result<IndexerProvider<OotleWallet>> {
    let wallet = make_wallet(user)?;
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect(INDEXER_URL)
        .await?;
    Ok(provider)
}

fn derive_public_key_hex(account_secret_hex: &str) -> anyhow::Result<String> {
    let acc_bytes = Vec::from_hex(account_secret_hex)
        .map_err(|e| anyhow::anyhow!("Invalid secret hex: {e}"))?;
    let acc_sk = RistrettoSecretKey::from_canonical_bytes(&acc_bytes)
        .map_err(|e| anyhow::anyhow!("Invalid key bytes: {e}"))?;
    let acc_pk = RistrettoPublicKey::from_secret_key(&acc_sk);
    Ok(acc_pk.to_hex())
}

/// Derive deterministic account+view secret keys from a passphrase.
/// Uses SHA-256 with distinct domain prefixes for each key.
/// NOTE: These keys are NOT compatible with walletd's key derivation scheme.
fn derive_keys_from_passphrase(passphrase: &str) -> anyhow::Result<(String, String)> {
    let mut acc_bytes: [u8; 32] = {
        let mut h = Sha256::new();
        h.update(b"tari_ootle_account_key_v1:");
        h.update(passphrase.as_bytes());
        h.finalize().into()
    };
    // Mask top 4 bits to guarantee value < group order (2^252)
    acc_bytes[31] &= 0x0f;

    let mut view_bytes: [u8; 32] = {
        let mut h = Sha256::new();
        h.update(b"tari_ootle_view_key_v1:");
        h.update(passphrase.as_bytes());
        h.finalize().into()
    };
    view_bytes[31] &= 0x0f;

    let acc_sk = RistrettoSecretKey::from_canonical_bytes(&acc_bytes)
        .map_err(|e| anyhow::anyhow!("Account key derivation failed: {e}"))?;
    let view_sk = RistrettoSecretKey::from_canonical_bytes(&view_bytes)
        .map_err(|e| anyhow::anyhow!("View key derivation failed: {e}"))?;

    Ok((acc_sk.to_hex(), view_sk.to_hex()))
}

/// Parse a template address string (accepts "template_<hex>" or plain "<hex>").
fn parse_template_address(s: &str) -> anyhow::Result<TemplateAddress> {
    use std::str::FromStr;
    let hex = s.strip_prefix("template_").unwrap_or(s);
    TemplateAddress::from_str(hex).map_err(|e| anyhow::anyhow!("Invalid template address: {e}"))
}

// ── Core blockchain operations ────────────────────────────────────────────────

/// Returns `(tx_id, receipt)` on success.
async fn build_and_send(
    provider: &mut IndexerProvider<OotleWallet>,
    build_fn: impl FnOnce(TransactionBuilder) -> TransactionBuilder,
    want_list: WantList,
) -> anyhow::Result<(String, TransactionReceipt)> {
    let network = provider.network();
    let base = TransactionBuilder::new(network).with_auto_fill_inputs();
    let unsigned_tx = build_fn(base).build_unsigned();
    let unsigned_tx = provider
        .resolve_input_want_list(unsigned_tx, want_list.items())
        .await?;

    let tx = TransactionRequest::default()
        .with_transaction(unsigned_tx)
        .build(provider.wallet())
        .await?;

    let pending = provider.send_transaction(tx).await?;
    let tx_id = pending.tx_id().to_string();
    println!("  Tx submitted: {tx_id}");
    let outcome = pending.watch().await?;
    println!("  Outcome: {outcome}");

    if let Some(reason) = outcome.reject_reason() {
        anyhow::bail!("Transaction rejected: {reason}");
    }
    let receipt = pending
        .get_receipt()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get receipt: {e}"))?;
    Ok((tx_id, receipt))
}

/// Generate a new wallet, fund from faucet, return `(UserConfig, faucet_tx_id)`.
async fn create_funded_wallet(display_name: &str) -> anyhow::Result<(UserConfig, String)> {
    println!("  Generating wallet for '{display_name}'...");
    let secret = OotleSecretKey::random(NETWORK);
    let acc_hex = secret.account_secret().to_hex();
    let view_hex = secret.view_only_secret().to_hex();
    let pk_hex = derive_public_key_hex(&acc_hex)?;

    let wallet = OotleWallet::from(PrivateKeyProvider::new(secret));
    let mut provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect(INDEXER_URL)
        .await?;

    let account_addr = provider.default_signer_address().to_account_address();
    println!("  Account address: {account_addr}");

    println!("  Funding '{display_name}' from faucet...");
    let unsigned_tx = IFaucet::new(&provider)
        .take_faucet_funds(10 * TARI)
        .pay_fee(500u64)
        .prepare()
        .await?;

    let tx = TransactionRequest::default()
        .with_transaction(unsigned_tx)
        .build(provider.wallet())
        .await?;

    let pending = provider.send_transaction(tx).await?;
    let faucet_tx_id = pending.tx_id().to_string();
    println!("  Faucet tx submitted: {faucet_tx_id}");
    pending.watch().await?;
    println!("  '{display_name}' funded successfully.");

    Ok((
        UserConfig {
            display_name: display_name.to_string(),
            account_secret_hex: acc_hex,
            view_secret_hex: view_hex,
            account_address: account_addr,
            public_key_hex: pk_hex,
        },
        faucet_tx_id,
    ))
}

/// Import a wallet from hex keys (no faucet funding).
/// Returns UserConfig with account_address resolved from the keys.
async fn import_wallet_hex(
    display_name: &str,
    account_secret_hex: &str,
    view_secret_hex: &str,
) -> anyhow::Result<UserConfig> {
    let pk_hex = derive_public_key_hex(account_secret_hex)?;

    // Build a temporary UserConfig so we can create a provider and get the account address
    let acc_bytes = Vec::from_hex(account_secret_hex)
        .map_err(|e| anyhow::anyhow!("Invalid account secret hex: {e}"))?;
    let view_bytes = Vec::from_hex(view_secret_hex)
        .map_err(|e| anyhow::anyhow!("Invalid view secret hex: {e}"))?;
    let acc_sk = RistrettoSecretKey::from_canonical_bytes(&acc_bytes)
        .map_err(|e| anyhow::anyhow!("Invalid account key: {e}"))?;
    let view_sk = RistrettoSecretKey::from_canonical_bytes(&view_bytes)
        .map_err(|e| anyhow::anyhow!("Invalid view key: {e}"))?;
    let secret = OotleSecretKey::new(NETWORK, acc_sk, view_sk);
    let wallet = OotleWallet::from(PrivateKeyProvider::new(secret));
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect(INDEXER_URL)
        .await?;

    let account_addr = provider.default_signer_address().to_account_address();
    Ok(UserConfig {
        display_name: display_name.to_string(),
        account_secret_hex: account_secret_hex.to_string(),
        view_secret_hex: view_secret_hex.to_string(),
        account_address: account_addr,
        public_key_hex: pk_hex,
    })
}

/// Fund an existing wallet from the testnet faucet.
/// Returns the faucet tx_id.
async fn fund_wallet_faucet(user: &UserConfig) -> anyhow::Result<String> {
    println!("  Funding '{}' from faucet...", user.display_name);
    let mut provider = make_provider(user).await?;

    let unsigned_tx = IFaucet::new(&provider)
        .take_faucet_funds(10 * TARI)
        .pay_fee(500u64)
        .prepare()
        .await?;

    let tx = TransactionRequest::default()
        .with_transaction(unsigned_tx)
        .build(provider.wallet())
        .await?;

    let pending = provider.send_transaction(tx).await?;
    let tx_id = pending.tx_id().to_string();
    pending.watch().await?;
    println!("  Funded successfully.");
    Ok(tx_id)
}

/// Deploy the messaging component using the given wallet to pay fees.
/// Returns `(component_address, tx_id)`.
async fn create_component(
    funder: &UserConfig,
    template_addr: TemplateAddress,
) -> anyhow::Result<(ComponentAddress, String)> {
    println!("Deploying messaging component...");
    let mut provider = make_provider(funder).await?;
    let account_addr = funder.account_address;

    let want_list = WantList::new().add_vault_for_resource(account_addr, TARI_TOKEN, true);

    let (tx_id, receipt) = build_and_send(
        &mut provider,
        |builder| {
            builder
                .pay_fee_from_component(account_addr, 2000u64)
                .call_function(template_addr, "new", args![])
        },
        want_list,
    )
    .await?;

    let component_addr = receipt
        .diff_summary
        .upped
        .iter()
        .find_map(|s| s.substate_id.as_component_address())
        .ok_or_else(|| anyhow::anyhow!("No component address in receipt"))?;

    println!("Component deployed: {component_addr}");
    Ok((component_addr, tx_id))
}

// ── Background tasks ──────────────────────────────────────────────────────────

/// Called on startup. Sets up status and kicks off component deployment if needed.
async fn run_initialization(shared: SharedState) {
    let (needs_deploy, funder, template_str) = {
        let s = shared.read().await;
        let no_wallets = s.wallets.is_empty();
        let no_template = s.template_address.is_none();
        let has_component = s.component_address.is_some();

        if no_wallets {
            drop(s);
            shared.write().await.setup_status = SetupStatus::NeedsWallet;
            return;
        }
        if no_template {
            drop(s);
            shared.write().await.setup_status = SetupStatus::NeedsTemplate;
            return;
        }
        if has_component {
            drop(s);
            shared.write().await.setup_status = SetupStatus::Ready;
            println!("Messaging app ready!");
            return;
        }

        // Has wallets + template but no component — deploy it
        let funder = s.wallets.values().next().cloned().unwrap();
        let template_str = s.template_address.clone().unwrap();
        (true, funder, template_str)
    };

    if needs_deploy {
        shared.write().await.setup_status = SetupStatus::DeployingComponent;
        deploy_component_with_wallet(shared, funder, template_str).await;
    }
}

async fn deploy_component_with_wallet(
    shared: SharedState,
    funder: UserConfig,
    template_str: String,
) {
    let template_addr = match parse_template_address(&template_str) {
        Ok(a) => a,
        Err(e) => {
            shared.write().await.setup_status =
                SetupStatus::Error(format!("Invalid template address: {e}"));
            return;
        }
    };

    match create_component(&funder, template_addr).await {
        Ok((component_addr, tx_id)) => {
            let mut s = shared.write().await;
            s.component_address = Some(component_addr);
            s.setup_status = SetupStatus::Ready;
            s.tx_history
                .push(TxRecord::new(tx_id, "Deploy Component", 2_000));
            save_state(&s, &s.state_file);
            println!("Messaging app ready!");
        }
        Err(e) => {
            eprintln!("Failed to deploy component: {e}");
            shared.write().await.setup_status =
                SetupStatus::Error(format!("Failed to deploy component: {e}"));
        }
    }
}

async fn publish_and_deploy(shared: SharedState, funder: UserConfig, wasm_bytes: Vec<u8>) {
    let mut provider = match make_provider(&funder).await {
        Ok(p) => p,
        Err(e) => {
            shared.write().await.setup_status =
                SetupStatus::Error(format!("Failed to connect: {e}"));
            return;
        }
    };

    let account_addr = funder.account_address;
    let want_list = WantList::new().add_vault_for_resource(account_addr, TARI_TOKEN, true);

    // Publish the template (fee ~250_000 for a typical WASM)
    let publish_result = build_and_send(
        &mut provider,
        |builder| {
            builder
                .pay_fee_from_component(account_addr, 250_000u64)
                .publish_template(wasm_bytes.try_into().expect("wasm bytes"))
        },
        want_list,
    )
    .await;

    let (publish_tx_id, receipt) = match publish_result {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Publish failed: {e}");
            shared.write().await.setup_status =
                SetupStatus::Error(format!("Publish failed: {e}"));
            return;
        }
    };

    let template_addr = match receipt
        .diff_summary
        .upped
        .iter()
        .find_map(|s| s.substate_id.as_template())
    {
        Some(a) => a,
        None => {
            shared.write().await.setup_status = SetupStatus::Error(
                "No template address found in publish receipt".to_string(),
            );
            return;
        }
    };

    println!("Template published: {template_addr}");
    let template_addr_str = template_addr.to_string();

    {
        let mut s = shared.write().await;
        s.template_address = Some(template_addr_str.clone());
        s.tx_history
            .push(TxRecord::new(publish_tx_id, "Publish Template", 250_000));
        save_state(&s, &s.state_file);
    }

    let parsed_addr = match parse_template_address(&template_addr_str) {
        Ok(a) => a,
        Err(e) => {
            shared.write().await.setup_status =
                SetupStatus::Error(format!("Bad template address: {e}"));
            return;
        }
    };

    match create_component(&funder, parsed_addr).await {
        Ok((component_addr, deploy_tx_id)) => {
            let mut s = shared.write().await;
            s.component_address = Some(component_addr);
            s.setup_status = SetupStatus::Ready;
            s.tx_history
                .push(TxRecord::new(deploy_tx_id, "Deploy Component", 2_000));
            save_state(&s, &s.state_file);
            println!("Ready!");
        }
        Err(e) => {
            eprintln!("Component creation failed: {e}");
            shared.write().await.setup_status =
                SetupStatus::Error(format!("Component creation failed: {e}"));
        }
    }
}

// ── API handlers ──────────────────────────────────────────────────────────────

async fn handle_index() -> impl IntoResponse {
    Html(include_str!("../static/index.html"))
}

async fn handle_instructions() -> impl IntoResponse {
    Html(include_str!("../static/instructions.html"))
}

// ── /api/status ───────────────────────────────────────────────────────────────

async fn handle_status(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "setup_status": s.setup_status,
        "wallet_count": s.wallets.len(),
        "template_address": s.template_address,
        "component_address": s.component_address.as_ref().map(|a| a.to_string()),
    }))
}

// ── /api/wallets ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct WalletInfo {
    display_name: String,
    public_key_hex: String,
    account_address: String,
}

impl From<&UserConfig> for WalletInfo {
    fn from(u: &UserConfig) -> Self {
        Self {
            display_name: u.display_name.clone(),
            public_key_hex: u.public_key_hex.clone(),
            account_address: u.account_address.to_string(),
        }
    }
}

async fn handle_wallets(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let wallets: Vec<WalletInfo> = s.wallets.values().map(WalletInfo::from).collect();
    Json(serde_json::json!({ "wallets": wallets }))
}

// ── POST /api/wallet/create ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateWalletRequest {
    display_name: String,
}

async fn handle_wallet_create(
    AxumState(state): AxumState<SharedState>,
    Json(body): Json<CreateWalletRequest>,
) -> impl IntoResponse {
    if body.display_name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "display_name is required" })),
        );
    }
    let display_name = body.display_name.trim().to_string();
    let shared = Arc::clone(&state);

    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(create_funded_wallet(&display_name))
    })
    .await
    .map_err(|e| anyhow::anyhow!("Task panicked: {e}"))
    .and_then(|r| r);

    match result {
        Ok((user, tx_id)) => {
            let pk = user.public_key_hex.clone();
            let addr = user.account_address.to_string();
            let name = user.display_name.clone();

            let (template_str, has_component) = {
                let s = state.read().await;
                (
                    s.template_address.clone(),
                    s.component_address.is_some(),
                )
            };

            {
                let mut s = state.write().await;
                s.wallets.insert(pk.clone(), user.clone());
                s.tx_history.push(TxRecord::new(
                    tx_id,
                    format!("Faucet → {} (10 tTARI)", name),
                    500,
                ));
                // Update status
                if s.template_address.is_none() {
                    s.setup_status = SetupStatus::NeedsTemplate;
                } else if s.component_address.is_some() {
                    s.setup_status = SetupStatus::Ready;
                }
                save_state(&s, &s.state_file);
            }

            // If we have a template but no component, deploy now
            if template_str.is_some() && !has_component {
                let shared2 = Arc::clone(&shared);
                let tmpl = template_str.unwrap();
                let funder = user.clone();
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("runtime");
                    rt.block_on(async {
                        {
                            shared2.write().await.setup_status = SetupStatus::DeployingComponent;
                        }
                        deploy_component_with_wallet(shared2, funder, tmpl).await;
                    });
                });
            }

            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "ok": true,
                    "public_key_hex": pk,
                    "account_address": addr,
                    "display_name": name,
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to create wallet: {e}") })),
        ),
    }
}

// ── POST /api/wallet/import ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct ImportWalletRequest {
    display_name: String,
    account_secret_hex: String,
    view_secret_hex: String,
}

async fn handle_wallet_import(
    AxumState(state): AxumState<SharedState>,
    Json(body): Json<ImportWalletRequest>,
) -> impl IntoResponse {
    let display_name = body.display_name.trim().to_string();
    if display_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "display_name is required" })),
        );
    }

    let acc = body.account_secret_hex.trim().to_string();
    let view = body.view_secret_hex.trim().to_string();

    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(import_wallet_hex(&display_name, &acc, &view))
    })
    .await
    .map_err(|e| anyhow::anyhow!("Task panicked: {e}"))
    .and_then(|r| r);

    finish_wallet_import(state, result).await
}

// ── POST /api/wallet/passphrase ───────────────────────────────────────────────

#[derive(Deserialize)]
struct PassphraseWalletRequest {
    display_name: String,
    passphrase: String,
}

async fn handle_wallet_passphrase(
    AxumState(state): AxumState<SharedState>,
    Json(body): Json<PassphraseWalletRequest>,
) -> impl IntoResponse {
    let display_name = body.display_name.trim().to_string();
    if display_name.is_empty() || body.passphrase.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "display_name and passphrase are required" })),
        );
    }

    let passphrase = body.passphrase.clone();
    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async {
            let (acc, view) = derive_keys_from_passphrase(&passphrase)?;
            import_wallet_hex(&display_name, &acc, &view).await
        })
    })
    .await
    .map_err(|e| anyhow::anyhow!("Task panicked: {e}"))
    .and_then(|r| r);

    finish_wallet_import(state, result).await
}

/// Shared logic for completing a wallet import (hex or passphrase).
async fn finish_wallet_import(
    state: SharedState,
    result: anyhow::Result<UserConfig>,
) -> (StatusCode, Json<serde_json::Value>) {
    match result {
        Ok(user) => {
            let pk = user.public_key_hex.clone();
            let addr = user.account_address.to_string();
            let name = user.display_name.clone();

            {
                let mut s = state.write().await;
                s.wallets.insert(pk.clone(), user);
                if s.template_address.is_none() {
                    s.setup_status = SetupStatus::NeedsTemplate;
                } else if s.component_address.is_some() {
                    s.setup_status = SetupStatus::Ready;
                }
                save_state(&s, &s.state_file);
            }

            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "ok": true,
                    "public_key_hex": pk,
                    "account_address": addr,
                    "display_name": name,
                })),
            )
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Import failed: {e}") })),
        ),
    }
}

// ── POST /api/wallet/faucet ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct FaucetRequest {
    public_key_hex: String,
}

async fn handle_wallet_faucet(
    AxumState(state): AxumState<SharedState>,
    Json(body): Json<FaucetRequest>,
) -> impl IntoResponse {
    let user = {
        let s = state.read().await;
        match s.wallets.get(&body.public_key_hex).cloned() {
            Some(u) => u,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": "Wallet not found" })),
                )
            }
        }
    };

    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(fund_wallet_faucet(&user))
    })
    .await
    .map_err(|e| anyhow::anyhow!("Task panicked: {e}"))
    .and_then(|r| r);

    match result {
        Ok(tx_id) => {
            let mut s = state.write().await;
            s.tx_history.push(TxRecord::new(tx_id, "Faucet (10 tTARI)", 500));
            save_state(&s, &s.state_file);
            (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Faucet failed: {e}") })),
        ),
    }
}

// ── POST /api/template/configure ─────────────────────────────────────────────

#[derive(Deserialize)]
struct ConfigureRequest {
    template_address: String,
}

async fn handle_configure(
    AxumState(state): AxumState<SharedState>,
    Json(body): Json<ConfigureRequest>,
) -> impl IntoResponse {
    if parse_template_address(&body.template_address).is_err() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid template address. Paste the address from the Wallet Web UI."
            })),
        );
    }

    let normalized = {
        let hex = body
            .template_address
            .strip_prefix("template_")
            .unwrap_or(&body.template_address);
        format!("template_{hex}")
    };

    let funder = {
        let mut s = state.write().await;
        s.template_address = Some(normalized.clone());
        s.setup_status = SetupStatus::DeployingComponent;
        save_state(&s, &s.state_file);
        s.wallets.values().next().cloned()
    };

    if let Some(funder) = funder {
        let shared = Arc::clone(&state);
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime");
            rt.block_on(deploy_component_with_wallet(shared, funder, normalized));
        });
    } else {
        state.write().await.setup_status = SetupStatus::NeedsWallet;
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "message": "Template configured. Creating component..."
        })),
    )
}

// ── POST /api/template/publish ────────────────────────────────────────────────

async fn handle_publish(AxumState(state): AxumState<SharedState>) -> impl IntoResponse {
    let funder = {
        let s = state.read().await;
        match s.wallets.values().next().cloned() {
            Some(w) => w,
            None => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({
                        "error": "No wallets found. Create or import a wallet first."
                    })),
                )
            }
        }
    };

    let wasm_bytes = match std::fs::read(WASM_PATH) {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!(
                        "Template WASM not found at '{}'. Run: cd messaging_template && cargo build --target wasm32-unknown-unknown --release\n\nDetails: {e}",
                        WASM_PATH
                    )
                })),
            );
        }
    };

    println!("Publishing template ({} KB)...", wasm_bytes.len() / 1024);
    {
        let mut s = state.write().await;
        s.setup_status = SetupStatus::DeployingComponent;
    }

    let shared = Arc::clone(&state);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(publish_and_deploy(shared, funder, wasm_bytes));
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "message": "Publishing template on-chain... this takes ~30-60s."
        })),
    )
}

// ── E2EE Encryption helpers (experimental) ────────────────────────────────────
// Protocol: Ristretto ECDH → HKDF-SHA256 → AES-256-GCM
// Wire format: "ENC1:<hex(12-byte nonce || ciphertext)>"
// Key derivation is symmetric: sorted(pk_a, pk_b) so both sides get same key.

const ENC_PREFIX: &str = "ENC1:";

/// Derive a shared 256-bit conversation key from our secret key and the peer's public key.
/// The IKM is the ECDH shared secret point bytes (Ristretto compressed 32 bytes).
/// The HKDF info encodes both public keys in sorted order to make it symmetric.
fn derive_dm_key(
    my_sk_hex: &str,
    my_pk_hex: &str,
    their_pk_hex: &str,
) -> anyhow::Result<[u8; 32]> {
    let sk_bytes = Vec::from_hex(my_sk_hex)
        .map_err(|e| anyhow::anyhow!("Bad sk hex: {e}"))?;
    let pk_bytes = Vec::from_hex(their_pk_hex)
        .map_err(|e| anyhow::anyhow!("Bad pk hex: {e}"))?;

    let my_sk = RistrettoSecretKey::from_canonical_bytes(&sk_bytes)
        .map_err(|e| anyhow::anyhow!("Invalid secret key: {e}"))?;
    let their_pk = RistrettoPublicKey::from_canonical_bytes(&pk_bytes)
        .map_err(|e| anyhow::anyhow!("Invalid public key: {e}"))?;

    // ECDH: shared_point = their_pk * my_sk
    let shared_point = &their_pk * &my_sk;
    let shared_bytes = shared_point.as_bytes();

    // HKDF-SHA256 with sorted public keys as info (ensures symmetry)
    let mut pks: [&str; 2] = [my_pk_hex, their_pk_hex];
    pks.sort_unstable();
    let info = format!("tari-dm-v1:{}:{}", pks[0], pks[1]);

    let hk = Hkdf::<Sha256>::new(None, shared_bytes);
    let mut okm = [0u8; 32];
    hk.expand(info.as_bytes(), &mut okm)
        .map_err(|_| anyhow::anyhow!("HKDF expand failed"))?;

    Ok(okm)
}

/// Encrypt a DM message. Returns `"ENC1:<hex>"` on success, original content on failure.
fn encrypt_dm_content(
    my_sk_hex: &str,
    my_pk_hex: &str,
    their_pk_hex: &str,
    plaintext: &str,
) -> String {
    let key_result = (|| -> anyhow::Result<String> {
        let key_bytes = derive_dm_key(my_sk_hex, my_pk_hex, their_pk_hex)?;
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| anyhow::anyhow!("AES key error: {e}"))?;

        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("Encrypt error: {e}"))?;

        let mut payload = nonce_bytes.to_vec();
        payload.extend_from_slice(&ciphertext);
        let hex_str: String = payload.iter().map(|b| format!("{:02x}", b)).collect();
        Ok(format!("{}{}", ENC_PREFIX, hex_str))
    })();

    match key_result {
        Ok(enc) => enc,
        Err(e) => {
            eprintln!("Encryption failed (sending plaintext): {e}");
            plaintext.to_string()
        }
    }
}

/// Decrypt a DM message. Returns `(plaintext, true)` if decrypted, `(original, false)` if not.
fn decrypt_dm_content(
    my_sk_hex: &str,
    my_pk_hex: &str,
    their_pk_hex: &str,
    content: &str,
) -> Option<String> {
    let hex_payload = content.strip_prefix(ENC_PREFIX)?;
    let payload = (0..hex_payload.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_payload[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .ok()?;
    if payload.len() < 12 {
        return None;
    }
    let (nonce_bytes, ciphertext) = payload.split_at(12);

    let key_bytes = derive_dm_key(my_sk_hex, my_pk_hex, their_pk_hex).ok()?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).ok()?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext_bytes = cipher.decrypt(nonce, ciphertext).ok()?;
    String::from_utf8(plaintext_bytes).ok()
}

/// Try to decrypt a DM using any stored wallet key that matches sender or recipient.
/// Returns `(display_content, was_encrypted)`.
fn try_decrypt_dm(wallets: &HashMap<String, UserConfig>, dm: &LocalDm) -> (String, bool) {
    if !dm.content.starts_with(ENC_PREFIX) {
        return (dm.content.clone(), false);
    }

    // Try using the sender's key (we are the sender)
    if let Some(sender_wallet) = wallets.get(&dm.from_pk) {
        if let Some(plain) = decrypt_dm_content(
            &sender_wallet.account_secret_hex,
            &dm.from_pk,
            &dm.to_pk,
            &dm.content,
        ) {
            return (plain, true);
        }
    }

    // Try using the recipient's key (we are the recipient)
    if let Some(recipient_wallet) = wallets.get(&dm.to_pk) {
        if let Some(plain) = decrypt_dm_content(
            &recipient_wallet.account_secret_hex,
            &dm.to_pk,
            &dm.from_pk,
            &dm.content,
        ) {
            return (plain, true);
        }
    }

    // Could not decrypt — return marker
    ("[Encrypted message — key not available]".to_string(), true)
}

// ── GET/POST /api/settings ────────────────────────────────────────────────────

async fn handle_get_settings(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "encryption_enabled": s.encryption_enabled,
        "setup_mode": format!("{:?}", s.setup_mode),
    }))
}

#[derive(Deserialize)]
struct SettingsBody {
    encryption_enabled: Option<bool>,
    setup_mode: Option<String>,
}

async fn handle_post_settings(
    AxumState(state): AxumState<SharedState>,
    Json(body): Json<SettingsBody>,
) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    if let Some(enc) = body.encryption_enabled {
        s.encryption_enabled = enc;
    }
    if let Some(mode) = &body.setup_mode {
        s.setup_mode = match mode.as_str() {
            "Simple" | "simple" => SetupMode::Simple,
            "Advanced" | "advanced" => SetupMode::Advanced,
            _ => SetupMode::NotChosen,
        };
    }
    save_state(&s, &s.state_file.clone());
    Json(serde_json::json!({
        "ok": true,
        "encryption_enabled": s.encryption_enabled,
        "setup_mode": format!("{:?}", s.setup_mode),
    }))
}

// ── GET /api/public-config ────────────────────────────────────────────────────

async fn handle_public_config() -> Json<serde_json::Value> {
    let configured = !PUBLIC_COMPONENT_ADDRESS.is_empty();
    Json(serde_json::json!({
        "configured": configured,
        "component_address": PUBLIC_COMPONENT_ADDRESS,
        "template_address": PUBLIC_TEMPLATE_ADDRESS,
        // When configured=true, all users on this app share the same on-chain room.
        // When configured=false, public rooms degrade to the user's own contract (if deployed).
        "rooms": [
            {
                "room_id": "tari-messenger-test-chat",
                "display_name": "Tari Messenger Test Chat",
                "description": "Community testing ground for the Tari Messenger app on the Esmeralda testnet. Built by a community member — not an official Tari protocol channel. All messages are stored on-chain and publicly visible to anyone with the contract address."
            }
        ]
    }))
}

// ── POST /api/dm/send ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SendDmRequest {
    from_pubkey: String,
    to_pubkey: String,
    content: String,
}

async fn handle_dm_send(
    AxumState(state): AxumState<SharedState>,
    Json(body): Json<SendDmRequest>,
) -> impl IntoResponse {
    let content = body.content.trim().to_string();
    if content.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Message cannot be empty" })),
        );
    }

    let (sender, component_addr) = {
        let s = state.read().await;
        if s.setup_status != SetupStatus::Ready {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "App not ready. Complete wallet setup and template configuration first."
                })),
            );
        }
        let sender = match s.wallets.get(&body.from_pubkey).cloned() {
            Some(u) => u,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": "Sender wallet not found" })),
                )
            }
        };
        let comp = match s.component_address {
            Some(a) => a,
            None => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({ "error": "Messaging component not deployed" })),
                )
            }
        };
        (sender, comp)
    };

    let from_pk = body.from_pubkey.clone();
    let to_pk = body.to_pubkey.clone();
    let comp_str = component_addr.to_string();

    // Encrypt the content if encryption is enabled (experimental E2EE)
    let stored_content = {
        let s = state.read().await;
        if s.encryption_enabled {
            encrypt_dm_content(
                &sender.account_secret_hex,
                &from_pk,
                &to_pk,
                &content,
            )
        } else {
            content.clone()
        }
    };

    let to_pk_for_chain = to_pk.clone();
    let content_for_chain = stored_content.clone();
    let from_pk_for_rec = from_pk.clone();
    let to_pk_for_rec = to_pk.clone();
    let content_preview = content.clone();  // always store preview in plaintext

    // Save to local state immediately — UI updates without waiting for the chain (~30-60s)
    {
        let mut s = state.write().await;
        s.dms.push(LocalDm {
            from_pk: from_pk.clone(),
            to_pk: to_pk.clone(),
            content: stored_content,
            timestamp: now_secs(),
        });
        save_state(&s, &s.state_file);
    }

    // Fire blockchain tx in background — don't block the HTTP response
    let bg_state = Arc::clone(&state);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        match rt.block_on(send_dm_on_chain(sender, component_addr, to_pk_for_chain, content_for_chain)) {
            Ok(tx_id) => {
                let mut s = rt.block_on(bg_state.write());
                s.tx_history.push(
                    TxRecord::new(tx_id, "DM Sent", 2_000)
                        .with_from(from_pk_for_rec)
                        .with_to(to_pk_for_rec)
                        .with_content(&content_preview)
                        .with_component(comp_str),
                );
                save_state(&s, &s.state_file);
            }
            Err(e) => eprintln!("DM blockchain tx failed (message saved locally): {e}"),
        }
    });

    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
}

async fn send_dm_on_chain(
    sender: UserConfig,
    component_addr: ComponentAddress,
    to_pk_hex: String,
    content: String,
) -> anyhow::Result<String> {
    let mut provider = make_provider(&sender).await?;
    let account_addr = sender.account_address;

    let want_list = WantList::new()
        .add_vault_for_resource(account_addr, TARI_TOKEN, true)
        .add_specific_substate(component_addr, true);

    let (tx_id, _receipt) = build_and_send(
        &mut provider,
        |builder| {
            builder
                .pay_fee_from_component(account_addr, 2000u64)
                .call_method(component_addr, "send_dm", args![to_pk_hex, content])
        },
        want_list,
    )
    .await?;

    Ok(tx_id)
}

// ── GET /api/dm/messages ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct DmMessagesQuery {
    user_a: String,
    user_b: String,
}

async fn handle_dm_messages(
    AxumState(state): AxumState<SharedState>,
    Query(params): Query<DmMessagesQuery>,
) -> Json<serde_json::Value> {
    let s = state.read().await;
    let messages: Vec<serde_json::Value> = s
        .dms
        .iter()
        .filter(|m| {
            (m.from_pk == params.user_a && m.to_pk == params.user_b)
                || (m.from_pk == params.user_b && m.to_pk == params.user_a)
        })
        .map(|m| {
            let (display_content, encrypted) = try_decrypt_dm(&s.wallets, m);
            serde_json::json!({
                "from_pk": m.from_pk,
                "to_pk": m.to_pk,
                "content": display_content,
                "timestamp": m.timestamp,
                "encrypted": encrypted,
            })
        })
        .collect();
    Json(serde_json::json!({ "messages": messages }))
}

// ── GET /api/dm/inbox ─────────────────────────────────────────────────────────
// Returns all unique conversation partners for a given wallet, with last message
// info so the frontend can auto-populate the inbox on wallet switch.

#[derive(Deserialize)]
struct InboxQuery {
    wallet_pk: String,
}

async fn handle_dm_inbox(
    AxumState(state): AxumState<SharedState>,
    Query(params): Query<InboxQuery>,
) -> Json<serde_json::Value> {
    let s = state.read().await;
    // Collect unique partners and track the last message for each
    let mut partners: std::collections::HashMap<String, &LocalDm> = Default::default();
    for dm in &s.dms {
        let partner = if dm.from_pk == params.wallet_pk {
            Some(&dm.to_pk)
        } else if dm.to_pk == params.wallet_pk {
            Some(&dm.from_pk)
        } else {
            None
        };
        if let Some(pk) = partner {
            let entry = partners.entry(pk.clone()).or_insert(dm);
            if dm.timestamp > entry.timestamp {
                *entry = dm;
            }
        }
    }
    let mut conversations: Vec<serde_json::Value> = partners
        .iter()
        .map(|(partner_pk, last)| {
            let (display_content, encrypted) = try_decrypt_dm(&s.wallets, last);
            serde_json::json!({
                "partner_pk": partner_pk,
                "last_content": display_content,
                "last_timestamp": last.timestamp,
                "last_from": &last.from_pk,
                "encrypted": encrypted,
            })
        })
        .collect();
    // Sort by most recent first
    conversations.sort_by(|a, b| {
        b["last_timestamp"].as_u64().unwrap_or(0)
            .cmp(&a["last_timestamp"].as_u64().unwrap_or(0))
    });
    Json(serde_json::json!({ "conversations": conversations }))
}

// ── POST /api/room/create ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateRoomRequest {
    from_pubkey: String,
    room_id: String,
    display_name: String,
}

async fn handle_room_create(
    AxumState(state): AxumState<SharedState>,
    Json(body): Json<CreateRoomRequest>,
) -> impl IntoResponse {
    let room_id = body.room_id.trim().to_string();
    let display_name = body.display_name.trim().to_string();

    if room_id.is_empty() || display_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "room_id and display_name are required" })),
        );
    }

    // Check for duplicate room_id locally
    {
        let s = state.read().await;
        if s.rooms.iter().any(|r| r.room_id == room_id) {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({ "error": "Room ID already exists" })),
            );
        }
    }

    let (sender, component_addr) = {
        let s = state.read().await;
        if s.setup_status != SetupStatus::Ready {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "App not ready" })),
            );
        }
        let sender = match s.wallets.get(&body.from_pubkey).cloned() {
            Some(u) => u,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": "Sender wallet not found" })),
                )
            }
        };
        let comp = s.component_address.unwrap();
        (sender, comp)
    };

    let creator_pk = sender.public_key_hex.clone();
    let room_id2 = room_id.clone();
    let display_name2 = display_name.clone();

    let blockchain_result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(create_room_on_chain(
            sender,
            component_addr,
            room_id2,
            display_name2,
        ))
    })
    .await
    .map_err(|e| anyhow::anyhow!("Task panicked: {e}"))
    .and_then(|r| r);

    match blockchain_result {
        Ok(tx_id) => {
            let mut s = state.write().await;
            s.rooms.push(LocalRoom {
                room_id: room_id.clone(),
                display_name: display_name.clone(),
                creator_pk,
            });
            s.tx_history
                .push(TxRecord::new(tx_id, format!("Create Room #{room_id}"), 2_000));
            save_state(&s, &s.state_file);
            (StatusCode::CREATED, Json(serde_json::json!({ "ok": true, "room_id": room_id })))
        }
        Err(e) => {
            eprintln!("Failed to create room: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Transaction failed: {e}") })),
            )
        }
    }
}

async fn create_room_on_chain(
    sender: UserConfig,
    component_addr: ComponentAddress,
    room_id: String,
    display_name: String,
) -> anyhow::Result<String> {
    let mut provider = make_provider(&sender).await?;
    let account_addr = sender.account_address;

    let want_list = WantList::new()
        .add_vault_for_resource(account_addr, TARI_TOKEN, true)
        .add_specific_substate(component_addr, true);

    let (tx_id, _receipt) = build_and_send(
        &mut provider,
        |builder| {
            builder
                .pay_fee_from_component(account_addr, 2000u64)
                .call_method(component_addr, "create_room", args![room_id, display_name])
        },
        want_list,
    )
    .await?;

    Ok(tx_id)
}

// ── POST /api/room/join ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct JoinRoomRequest {
    room_id: String,
    display_name: String,
    creator_pk: Option<String>,
}

/// Add a room to local state without an on-chain transaction (for joining existing rooms).
async fn handle_room_join(
    AxumState(state): AxumState<SharedState>,
    Json(body): Json<JoinRoomRequest>,
) -> impl IntoResponse {
    let room_id = body.room_id.trim().to_string();
    if room_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "room_id is required" })),
        );
    }

    let mut s = state.write().await;
    if s.rooms.iter().any(|r| r.room_id == room_id) {
        return (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "message": "Already in room" })),
        );
    }

    let display_name = if body.display_name.trim().is_empty() {
        format!("#{room_id}")
    } else {
        body.display_name.trim().to_string()
    };

    s.rooms.push(LocalRoom {
        room_id: room_id.clone(),
        display_name,
        creator_pk: body.creator_pk.unwrap_or_default(),
    });
    save_state(&s, &s.state_file);

    (StatusCode::OK, Json(serde_json::json!({ "ok": true, "room_id": room_id })))
}

// ── POST /api/room/post ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PostRoomRequest {
    from_pubkey: String,
    room_id: String,
    content: String,
}

async fn handle_room_post(
    AxumState(state): AxumState<SharedState>,
    Json(body): Json<PostRoomRequest>,
) -> impl IntoResponse {
    let content = body.content.trim().to_string();
    if content.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Content cannot be empty" })),
        );
    }

    // Route public rooms to the hard-coded shared contract; private rooms to the user's own.
    let pub_comp = if is_public_room_id(&body.room_id) { public_component_addr() } else { None };

    let (sender, component_addr) = {
        let s = state.read().await;
        let sender = match s.wallets.get(&body.from_pubkey).cloned() {
            Some(u) => u,
            None => return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Sender wallet not found" })),
            ),
        };
        if let Some(pub_addr) = pub_comp {
            // Public room — uses the shared contract, no own component needed.
            (sender, pub_addr)
        } else {
            // Private room — requires the user's own deployed component.
            if s.setup_status != SetupStatus::Ready {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({ "error": "Deploy a messaging component first (⚙ Settings)" })),
                );
            }
            let comp = s.component_address.unwrap();
            (sender, comp)
        }
    };

    let from_pk = body.from_pubkey.clone();
    let room_id = body.room_id.clone();
    let room_id_for_chain = room_id.clone();
    let content_for_chain = content.clone();
    let from_pk_for_rec = from_pk.clone();
    let content_preview = content.clone();
    let comp_str = component_addr.to_string();

    // Save to local state immediately
    {
        let mut s = state.write().await;
        s.room_msgs.push(LocalRoomMsg {
            room_id: room_id.clone(),
            from_pk,
            content,
            timestamp: now_secs(),
        });
        save_state(&s, &s.state_file);
    }

    // Fire blockchain tx in background
    let bg_state = Arc::clone(&state);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let label = format!("Room Msg #{room_id_for_chain}");
        match rt.block_on(post_room_on_chain(sender, component_addr, room_id_for_chain.clone(), content_for_chain)) {
            Ok(tx_id) => {
                let mut s = rt.block_on(bg_state.write());
                s.tx_history.push(
                    TxRecord::new(tx_id, label, 2_000)
                        .with_from(from_pk_for_rec)
                        .with_to(room_id_for_chain)
                        .with_content(&content_preview)
                        .with_component(comp_str),
                );
                save_state(&s, &s.state_file);
            }
            Err(e) => eprintln!("Room post blockchain tx failed (message saved locally): {e}"),
        }
    });

    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
}

async fn post_room_on_chain(
    sender: UserConfig,
    component_addr: ComponentAddress,
    room_id: String,
    content: String,
) -> anyhow::Result<String> {
    let mut provider = make_provider(&sender).await?;
    let account_addr = sender.account_address;

    let want_list = WantList::new()
        .add_vault_for_resource(account_addr, TARI_TOKEN, true)
        .add_specific_substate(component_addr, true);

    let (tx_id, _receipt) = build_and_send(
        &mut provider,
        |builder| {
            builder
                .pay_fee_from_component(account_addr, 2000u64)
                .call_method(component_addr, "post_to_room", args![room_id, content])
        },
        want_list,
    )
    .await?;

    Ok(tx_id)
}

// ── GET /api/room/messages ────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RoomMessagesQuery {
    room_id: String,
}

async fn handle_room_messages(
    AxumState(state): AxumState<SharedState>,
    Query(params): Query<RoomMessagesQuery>,
) -> Json<serde_json::Value> {
    let s = state.read().await;
    let messages: Vec<&LocalRoomMsg> = s
        .room_msgs
        .iter()
        .filter(|m| m.room_id == params.room_id)
        .collect();
    Json(serde_json::json!({ "messages": messages }))
}

// ── GET /api/rooms ────────────────────────────────────────────────────────────

async fn handle_rooms(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({ "rooms": s.rooms }))
}

// ── GET /api/contacts ─────────────────────────────────────────────────────────

async fn handle_contacts(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({ "contacts": s.contacts }))
}

// ── POST /api/contacts/set ────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SetContactRequest {
    public_key_hex: String,
    display_name: String,
}

async fn handle_set_contact(
    AxumState(state): AxumState<SharedState>,
    Json(body): Json<SetContactRequest>,
) -> impl IntoResponse {
    let pk = body.public_key_hex.trim().to_string();
    let name = body.display_name.trim().to_string();
    if pk.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "public_key_hex is required" })),
        );
    }
    let mut s = state.write().await;
    if name.is_empty() {
        s.contacts.remove(&pk);
    } else {
        s.contacts.insert(pk, name);
    }
    save_state(&s, &s.state_file);
    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
}

// ── POST /api/demo/start ──────────────────────────────────────────────────────

async fn handle_demo_start(AxumState(state): AxumState<SharedState>) -> impl IntoResponse {
    // Return existing demo wallets if they already exist
    {
        let s = state.read().await;
        let ootle = s.wallets.values().find(|w| w.display_name == "Ootle (Demo)").cloned();
        let minotari = s.wallets.values().find(|w| w.display_name == "Minotari (Demo)").cloned();
        if let (Some(a), Some(b)) = (ootle, minotari) {
            return (
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "already_exists": true,
                    "ootle_pk": a.public_key_hex,
                    "minotari_pk": b.public_key_hex,
                })),
            );
        }
    }

    // Kick off background creation and return immediately
    let shared = Arc::clone(&state);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(create_demo_wallets(shared));
    });

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "ok": true,
            "message": "Creating Ootle & Minotari demo wallets... poll /api/wallets (~60s)"
        })),
    )
}

async fn create_demo_wallets(shared: SharedState) {
    println!("Creating Ootle & Minotari demo wallets concurrently...");

    // Run both faucet requests concurrently
    let ootle_handle = tokio::task::spawn_blocking(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(create_funded_wallet("Ootle (Demo)"))
    });
    let minotari_handle = tokio::task::spawn_blocking(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(create_funded_wallet("Minotari (Demo)"))
    });

    let (ootle_res, minotari_res) = tokio::join!(ootle_handle, minotari_handle);
    let ootle_res    = ootle_res.map_err(|e| anyhow::anyhow!("{e}")).and_then(|r| r);
    let minotari_res = minotari_res.map_err(|e| anyhow::anyhow!("{e}")).and_then(|r| r);

    match (ootle_res, minotari_res) {
        (Ok((ootle, ootle_tx)), Ok((minotari, minotari_tx))) => {
            let ootle_pk    = ootle.public_key_hex.clone();
            let minotari_pk = minotari.public_key_hex.clone();
            println!("Demo wallets ready! Ootle: {ootle_pk}  Minotari: {minotari_pk}");

            let (template_str, has_component) = {
                let s = shared.read().await;
                (s.template_address.clone(), s.component_address.is_some())
            };

            {
                let mut s = shared.write().await;
                s.wallets.insert(ootle_pk.clone(), ootle.clone());
                s.wallets.insert(minotari_pk.clone(), minotari.clone());
                s.tx_history.push(
                    TxRecord::new(ootle_tx, "Faucet → Ootle Demo (10 tTARI)", 500)
                        .with_from(ootle_pk.clone()),
                );
                s.tx_history.push(
                    TxRecord::new(minotari_tx, "Faucet → Minotari Demo (10 tTARI)", 500)
                        .with_from(minotari_pk.clone()),
                );
                if s.template_address.is_none() {
                    s.setup_status = SetupStatus::NeedsTemplate;
                } else if s.component_address.is_some() {
                    s.setup_status = SetupStatus::Ready;
                }
                save_state(&s, &s.state_file);
            }

            // Deploy component if template is ready but component isn't
            if template_str.is_some() && !has_component {
                let tmpl = template_str.unwrap();
                let shared2 = Arc::clone(&shared);
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();
                    rt.block_on(async {
                        shared2.write().await.setup_status = SetupStatus::DeployingComponent;
                        deploy_component_with_wallet(shared2, ootle, tmpl).await;
                    });
                });
            }
        }
        (Err(e), _) => eprintln!("Ootle demo wallet creation failed: {e}"),
        (_, Err(e)) => eprintln!("Minotari demo wallet creation failed: {e}"),
    }
}

// ── POST /api/sync ────────────────────────────────────────────────────────────

async fn handle_force_sync(AxumState(state): AxumState<SharedState>) -> impl IntoResponse {
    let component_addr = {
        let s = state.read().await;
        s.component_address.as_ref().map(|a| a.to_string())
    };

    // Always sync the public component if configured.
    if let Some(pub_addr) = public_component_addr() {
        let pub_str = pub_addr.to_string();
        let pub_state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(e) = sync_from_chain(&pub_state, &pub_str).await {
                eprintln!("[force-sync/public] Failed: {e}");
            }
        });
    }

    // Sync user's own component if deployed.
    if let Some(addr) = component_addr {
        tokio::spawn(async move {
            if let Err(e) = sync_from_chain(&state, &addr).await {
                eprintln!("[force-sync] Failed: {e}");
            }
        });
        return (StatusCode::OK, Json(serde_json::json!({"ok": true, "message": "Sync started in background"})));
    }

    if !PUBLIC_COMPONENT_ADDRESS.is_empty() {
        return (StatusCode::OK, Json(serde_json::json!({"ok": true, "message": "Public component sync started"})));
    }

    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"No component deployed"})))
}

// ── Chain polling — read component state directly from the indexer ────────────
//
// Per the tari-ootle skill: the correct way to read on-chain state is via
// `get_substate`. The `list_events` JSON-RPC is not supported by this indexer.
//
// The MessagingService component stores ALL messages in its struct fields as
// parallel Vec<String> arrays. We query the component substate, extract those
// arrays by field name, and merge into local state. This gives us the complete
// history on every poll — new clients always backdate automatically.

/// Start background polling. Runs forever, polls every 10 seconds.
async fn poll_chain_messages(state: SharedState) {
    // Always sync the public component immediately if configured — no own component needed.
    if let Some(pub_addr) = public_component_addr() {
        let pub_str = pub_addr.to_string();
        let pub_state = Arc::clone(&state);
        tokio::spawn(async move {
            println!("[chain-poll/public] Starting — syncing public contract every 30s");
            loop {
                if let Err(e) = sync_from_chain(&pub_state, &pub_str).await {
                    eprintln!("[chain-poll/public] Sync failed: {e}");
                }
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });
    }

    // Wait until user's own component is deployed and Ready before starting private sync.
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let ready = {
            let s = state.read().await;
            s.component_address.is_some() && s.setup_status == SetupStatus::Ready
        };
        if ready { break; }
    }
    println!("[chain-poll] Starting private component — reading state every 10s");
    loop {
        let component_addr = {
            let s = state.read().await;
            s.component_address.as_ref().map(|a| a.to_string())
        };
        if let Some(addr) = component_addr {
            if let Err(e) = sync_from_chain(&state, &addr).await {
                eprintln!("[chain-poll] Sync failed: {e}");
            }
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

/// Sync all messages by reading the component state directly from the indexer.
///
/// Primary strategy: GET {INDEXER_URL}substates/{component_addr}
/// This returns the full MessagingService component state including all
/// parallel Vec<String> fields (dm_from, dm_to, dm_content, room_ids, etc.).
/// We parse those arrays and reconstruct all messages. This is more reliable
/// than event parsing because state is always consistent and doesn't depend
/// on event payload serialization format.
///
/// Fallback strategy: GET {INDEXER_URL}transactions/events (event-based)
/// Used when state parsing fails or as supplementary data.
async fn sync_from_chain(state: &SharedState, component_addr: &str) -> anyhow::Result<()> {
    // Try component state sync first (most reliable)
    if let Ok(true) = sync_from_component_state(state, component_addr).await {
        return Ok(());
    }

    // Fallback: event-based sync
    sync_from_events(state, component_addr).await
}

/// Read the component's on-chain state directly and extract all messages.
/// Returns Ok(true) if any new data was found, Ok(false) if nothing changed.
async fn sync_from_component_state(state: &SharedState, component_addr: &str) -> anyhow::Result<bool> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let resp: serde_json::Value = client
        .get(format!("{}substates/{}", INDEXER_URL, component_addr))
        .send()
        .await?
        .json()
        .await?;

    // Find arrays by field name anywhere in the JSON tree
    let dm_from    = find_string_array(&resp, "dm_from");
    let dm_to      = find_string_array(&resp, "dm_to");
    let dm_content = find_string_array(&resp, "dm_content");

    let room_ids      = find_string_array(&resp, "room_ids");
    let room_names    = find_string_array(&resp, "room_names");
    let room_creators = find_string_array(&resp, "room_creators");

    let room_msg_room    = find_string_array(&resp, "room_msg_room");
    let room_msg_from    = find_string_array(&resp, "room_msg_from");
    let room_msg_content = find_string_array(&resp, "room_msg_content");

    // If we got no data at all, state read failed — log and return false
    if dm_from.is_empty() && room_ids.is_empty() && room_msg_room.is_empty() {
        let preview = resp.to_string();
        eprintln!("[chain-poll] State read returned no fields. Response preview: {}", &preview[..preview.len().min(400)]);
        return Ok(false);
    }

    let now = now_secs();
    let mut new_dms = 0usize;
    let mut new_rooms = 0usize;
    let mut new_room_msgs = 0usize;

    let mut s = state.write().await;

    // Merge DMs from parallel arrays
    let dm_len = dm_from.len().min(dm_to.len()).min(dm_content.len());
    for i in 0..dm_len {
        let (from, to, content) = (&dm_from[i], &dm_to[i], &dm_content[i]);
        if !s.dms.iter().any(|d| &d.from_pk == from && &d.to_pk == to && &d.content == content) {
            s.dms.push(LocalDm { from_pk: from.clone(), to_pk: to.clone(), content: content.clone(), timestamp: now });
            new_dms += 1;
        }
    }

    // Merge rooms from parallel arrays
    let room_len = room_ids.len().min(room_names.len()).min(room_creators.len());
    for i in 0..room_len {
        let room_id = &room_ids[i];
        if !s.rooms.iter().any(|r| &r.room_id == room_id) {
            s.rooms.push(LocalRoom {
                room_id: room_id.clone(),
                display_name: room_names[i].clone(),
                creator_pk: room_creators[i].clone(),
            });
            new_rooms += 1;
        }
    }

    // Merge room messages from parallel arrays
    let rmsg_len = room_msg_room.len().min(room_msg_from.len()).min(room_msg_content.len());
    for i in 0..rmsg_len {
        let (room_id, from, content) = (&room_msg_room[i], &room_msg_from[i], &room_msg_content[i]);
        if !s.room_msgs.iter().any(|m| &m.room_id == room_id && &m.from_pk == from && &m.content == content) {
            s.room_msgs.push(LocalRoomMsg { room_id: room_id.clone(), from_pk: from.clone(), content: content.clone(), timestamp: now });
            new_room_msgs += 1;
        }
    }

    if new_dms + new_rooms + new_room_msgs > 0 {
        println!("[chain-poll] State sync: {new_dms} DMs, {new_room_msgs} room msgs, {new_rooms} rooms");
        save_state(&s, &s.state_file);
        Ok(true)
    } else {
        eprintln!("[chain-poll] State sync: {} DMs, {} rooms, {} room msgs on chain — nothing new",
            dm_len, room_len, rmsg_len);
        Ok(false)
    }
}

/// Find a field's string array from the component state JSON.
///
/// The indexer serializes component state as CBOR-encoded map-of-pairs:
///   `{"Map": [ [{"Text":"field_name"}, {"Array":[{"Text":"val1"},{"Text":"val2"}]}], ... ]}`
///
/// This function walks the entire tree looking for that pattern.
fn find_string_array(v: &serde_json::Value, key: &str) -> Vec<String> {
    // Check if this is a Map (array-of-pairs encoding from CBOR)
    if let Some(map_arr) = v.get("Map").and_then(|m| m.as_array()) {
        for pair in map_arr {
            if let Some(pair_arr) = pair.as_array() {
                if pair_arr.len() == 2 {
                    // pair[0] is the key: {"Text": "field_name"}
                    let field_name = pair_arr[0].get("Text").and_then(|t| t.as_str()).unwrap_or("");
                    if field_name == key {
                        // pair[1] is the value: {"Array": [{"Text": "val1"}, ...]}
                        if let Some(items) = pair_arr[1].get("Array").and_then(|a| a.as_array()) {
                            return items.iter()
                                .filter_map(|i| i.get("Text").and_then(|t| t.as_str()).map(|s| s.to_string()))
                                .collect();
                        }
                    }
                }
            }
        }
    }

    // Recurse into object values and array elements
    match v {
        serde_json::Value::Object(map) => {
            for val in map.values() {
                let found = find_string_array(val, key);
                if !found.is_empty() {
                    return found;
                }
            }
            vec![]
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                let found = find_string_array(item, key);
                if !found.is_empty() {
                    return found;
                }
            }
            vec![]
        }
        _ => vec![],
    }
}

/// Fallback event-based sync. Queries the events endpoint and tries multiple
/// JSON path variations for the payload to handle different indexer formats.
async fn sync_from_events(state: &SharedState, component_addr: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let resp: serde_json::Value = client
        .get(format!("{}transactions/events", INDEXER_URL))
        .query(&[("substate_id", component_addr), ("limit", "1000")])
        .send()
        .await?
        .json()
        .await?;

    let events = match resp["events"].as_array() {
        Some(arr) => arr.clone(),
        None => {
            eprintln!("[chain-poll] Events response missing 'events' key: {}", &resp.to_string()[..resp.to_string().len().min(300)]);
            return Ok(());
        }
    };

    if let Some(first) = events.first() {
        eprintln!("[chain-poll] FIRST EVENT (fallback): {}", serde_json::to_string(first).unwrap_or_default());
    }

    let now = now_secs();
    let mut new_dms = 0usize;
    let mut new_rooms = 0usize;
    let mut new_room_msgs = 0usize;

    let mut s = state.write().await;

    for tuple in &events {
        // The response may be [[tx_id, event_obj], ...] or [event_obj, ...]
        // Try to find the object that has a "topic" field
        let event = extract_event_obj(tuple);
        let topic = event["topic"].as_str().unwrap_or("");
        if topic.is_empty() { continue; }

        let payload = &event["payload"];

        if topic.ends_with("DmSent") {
            let from    = payload_field(payload, "from");
            let to      = payload_field(payload, "to");
            let content = payload_field(payload, "content");
            if !from.is_empty() && !to.is_empty() && !content.is_empty()
                && !s.dms.iter().any(|d| d.from_pk == from && d.to_pk == to && d.content == content)
            {
                s.dms.push(LocalDm { from_pk: from, to_pk: to, content, timestamp: now });
                new_dms += 1;
            }
        } else if topic.ends_with("RoomCreated") {
            let room_id      = payload_field(payload, "room_id");
            let display_name = payload_field(payload, "display_name");
            let creator      = payload_field(payload, "creator");
            if !room_id.is_empty() && !s.rooms.iter().any(|r| r.room_id == room_id) {
                s.rooms.push(LocalRoom {
                    room_id,
                    display_name: if display_name.is_empty() { "Unknown Room".into() } else { display_name },
                    creator_pk: creator,
                });
                new_rooms += 1;
            }
        } else if topic.ends_with("RoomMessage") {
            let room_id = payload_field(payload, "room_id");
            let from    = payload_field(payload, "from");
            let content = payload_field(payload, "content");
            if !room_id.is_empty() && !from.is_empty() && !content.is_empty()
                && !s.room_msgs.iter().any(|m| m.room_id == room_id && m.from_pk == from && m.content == content)
            {
                s.room_msgs.push(LocalRoomMsg { room_id, from_pk: from, content, timestamp: now });
                new_room_msgs += 1;
            }
        }
    }

    if new_dms + new_rooms + new_room_msgs > 0 {
        println!("[chain-poll] Event sync: {new_dms} DMs, {new_room_msgs} room msgs, {new_rooms} rooms");
        save_state(&s, &s.state_file);
    } else {
        eprintln!("[chain-poll] Event sync: {} events, nothing new", events.len());
    }

    Ok(())
}

/// Extract the event object from a tuple entry. Handles both:
/// - `[tx_id, event_obj]` — standard format
/// - `event_obj` — direct object
fn extract_event_obj(v: &serde_json::Value) -> &serde_json::Value {
    if v.get("topic").is_some() {
        return v;
    }
    if let Some(arr) = v.as_array() {
        for item in arr {
            if item.get("topic").is_some() {
                return item;
            }
        }
    }
    v
}

/// Extract a string field from an event payload.
/// Handles plain JSON objects `{"key": "val"}` and CBOR-decoded array-of-pairs `[["key", "val"], ...]`.
fn payload_field(payload: &serde_json::Value, key: &str) -> String {
    // Plain object: {"from": "...", "to": "..."}
    if let Some(obj) = payload.as_object() {
        if let Some(v) = obj.get(key) {
            return v.as_str().unwrap_or("").to_string();
        }
    }
    // Array of [key, value] pairs (alternative encoding)
    if let Some(arr) = payload.as_array() {
        for pair in arr {
            if let Some(pair_arr) = pair.as_array() {
                if pair_arr.len() == 2 && pair_arr[0].as_str() == Some(key) {
                    return pair_arr[1].as_str().unwrap_or("").to_string();
                }
            }
        }
    }
    String::new()
}

// ── Balance helpers ───────────────────────────────────────────────────────────

/// Extract vault IDs from the account component state JSON.
///
/// The indexer serializes vault IDs as CBOR Tag[132, Bytes[...]] (not as "vault_..." strings).
/// Example: {"Tag":[132,{"Bytes":[14,249,182,...]}]} → "vault_0ef9b6bc..."
fn collect_vault_ids(v: &serde_json::Value, out: &mut Vec<String>) {
    // Check for CBOR Tag[132, Bytes[...]] — the vault ID tag number is 132
    if let Some(tag_arr) = v.get("Tag").and_then(|t| t.as_array()) {
        if tag_arr.len() == 2 && tag_arr[0].as_u64() == Some(132) {
            if let Some(bytes) = tag_arr[1].get("Bytes").and_then(|b| b.as_array()) {
                let hex_str: String = bytes
                    .iter()
                    .filter_map(|b| b.as_u64())
                    .map(|b| format!("{:02x}", b))
                    .collect();
                if hex_str.len() == 64 {
                    out.push(format!("vault_{}", hex_str));
                }
                return;
            }
        }
    }
    // Also handle plain "vault_..." strings (future-proofing)
    if let Some(s) = v.as_str() {
        if s.starts_with("vault_") {
            out.push(s.to_string());
        }
        return;
    }
    match v {
        serde_json::Value::Array(arr) => {
            for item in arr { collect_vault_ids(item, out); }
        }
        serde_json::Value::Object(map) => {
            for val in map.values() { collect_vault_ids(val, out); }
        }
        _ => {}
    }
}

/// Extract the TARI balance from a vault substate response.
///
/// The indexer serializes balances as strings (e.g. "revealed_amount":"9889138"),
/// not numbers. This handles both string and numeric forms, and both Stealth and
/// Fungible resource containers.
fn extract_vault_balance(v: &serde_json::Value) -> Option<u64> {
    for key in &["revealed_amount", "amount", "balance"] {
        if let Some(val) = v.get(key) {
            if let Some(n) = val.as_u64() { return Some(n); }
            if let Some(s) = val.as_str() {
                if let Ok(n) = s.parse::<u64>() { return Some(n); }
            }
        }
    }
    match v {
        serde_json::Value::Object(map) => {
            for val in map.values() {
                if let Some(n) = extract_vault_balance(val) { return Some(n); }
            }
            None
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let Some(n) = extract_vault_balance(item) { return Some(n); }
            }
            None
        }
        _ => None,
    }
}

/// Returns true if `target` appears as a string value anywhere in the JSON tree.
fn contains_string_value(v: &serde_json::Value, target: &str) -> bool {
    match v {
        serde_json::Value::String(s) => s == target,
        serde_json::Value::Array(arr) => arr.iter().any(|i| contains_string_value(i, target)),
        serde_json::Value::Object(map) => map.values().any(|i| contains_string_value(i, target)),
        _ => false,
    }
}

async fn query_balance_micro_tari(account_addr: &str) -> Option<u64> {
    let tari_resource_str = TARI_TOKEN.to_string();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .ok()?;

    // REST: GET {INDEXER_URL}substates/{account_addr}
    let resp: serde_json::Value = client
        .get(format!("{}substates/{}", INDEXER_URL, account_addr))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    // Extract vault IDs from account substate (CBOR Tag[132, Bytes] format)
    let mut vault_ids: Vec<String> = Vec::new();
    collect_vault_ids(&resp, &mut vault_ids);
    vault_ids.dedup();

    eprintln!("[balance] Account {}: found {} vault(s): {:?}", account_addr, vault_ids.len(), vault_ids);

    // Find the TARI vault and return its balance
    for vault_id in &vault_ids {
        let vresp: serde_json::Value = match client
            .get(format!("{}substates/{}", INDEXER_URL, vault_id))
            .send()
            .await
        {
            Ok(r) => match r.json().await {
                Ok(j) => j,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        if contains_string_value(&vresp, &tari_resource_str) {
            let bal = extract_vault_balance(&vresp);
            eprintln!("[balance] Vault {}: TARI balance = {:?}", vault_id, bal);
            return bal;
        }
    }
    eprintln!("[balance] No TARI vault found for {}", account_addr);
    None
}

// ── GET /api/wallet/balance ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct BalanceQuery {
    public_key_hex: String,
}

async fn handle_wallet_balance(
    AxumState(state): AxumState<SharedState>,
    Query(params): Query<BalanceQuery>,
) -> Json<serde_json::Value> {
    let user = {
        let s = state.read().await;
        s.wallets.get(&params.public_key_hex).cloned()
    };
    let Some(user) = user else {
        return Json(serde_json::json!({"error": "Wallet not found", "balance_display": "unknown"}));
    };

    let account_addr = user.account_address.to_string();
    match tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all().build().expect("rt");
        rt.block_on(query_balance_micro_tari(&account_addr))
    }).await {
        Ok(Some(micro_tari)) => Json(serde_json::json!({
            "balance_micro_tari": micro_tari,
            "balance_display": format!("{:.4} tTARI", micro_tari as f64 / 1_000_000.0),
        })),
        _ => Json(serde_json::json!({
            "balance_micro_tari": null,
            "balance_display": "unknown",
        })),
    }
}

// ── GET /api/wallet/export-keys ───────────────────────────────────────────────

fn key_to_mnemonic(key_hex: &str) -> anyhow::Result<String> {
    use bip39::Mnemonic;
    let bytes = Vec::from_hex(key_hex)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mnemonic = Mnemonic::from_entropy(&bytes)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(mnemonic.to_string())
}

fn mnemonic_to_key_hex(words: &str) -> anyhow::Result<String> {
    use bip39::Mnemonic;
    let mnemonic: Mnemonic = words.trim().parse()
        .map_err(|e: bip39::Error| anyhow::anyhow!("{e}"))?;
    let entropy = mnemonic.to_entropy();
    Ok(entropy.to_hex())
}

#[derive(Deserialize)]
struct ExportKeysQuery {
    public_key_hex: String,
}

async fn handle_wallet_export_keys(
    AxumState(state): AxumState<SharedState>,
    Query(params): Query<ExportKeysQuery>,
) -> Json<serde_json::Value> {
    let user = {
        let s = state.read().await;
        s.wallets.get(&params.public_key_hex).cloned()
    };
    let Some(user) = user else {
        return Json(serde_json::json!({"error": "Wallet not found"}));
    };

    let account_mnemonic = key_to_mnemonic(&user.account_secret_hex).ok();
    let view_mnemonic = key_to_mnemonic(&user.view_secret_hex).ok();

    Json(serde_json::json!({
        "display_name": user.display_name,
        "account_secret_hex": user.account_secret_hex,
        "view_secret_hex": user.view_secret_hex,
        "public_key_hex": user.public_key_hex,
        "account_address": user.account_address.to_string(),
        "account_mnemonic": account_mnemonic,
        "view_mnemonic": view_mnemonic,
    }))
}

// ── POST /api/wallet/import-mnemonic ─────────────────────────────────────────

#[derive(Deserialize)]
struct ImportMnemonicRequest {
    display_name: String,
    account_words: String,
    view_words: String,
}

async fn handle_wallet_import_mnemonic(
    AxumState(state): AxumState<SharedState>,
    Json(body): Json<ImportMnemonicRequest>,
) -> impl IntoResponse {
    let display_name = body.display_name.trim().to_string();
    if display_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "display_name is required" })),
        );
    }

    let acc_hex = match mnemonic_to_key_hex(&body.account_words) {
        Ok(h) => h,
        Err(e) => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Invalid account seed words: {e}") })),
        ),
    };
    let view_hex = match mnemonic_to_key_hex(&body.view_words) {
        Ok(h) => h,
        Err(e) => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Invalid view seed words: {e}") })),
        ),
    };

    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(import_wallet_hex(&display_name, &acc_hex, &view_hex))
    })
    .await
    .map_err(|e| anyhow::anyhow!("Task panicked: {e}"))
    .and_then(|r| r);

    finish_wallet_import(state, result).await
}

// ── POST /api/component/join ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct JoinComponentRequest {
    component_address: String,
    template_address: String,
}

fn parse_component_address(s: &str) -> anyhow::Result<ComponentAddress> {
    use std::str::FromStr;
    if let Ok(a) = ComponentAddress::from_str(s) {
        return Ok(a);
    }
    let hex = s.strip_prefix("component_").unwrap_or(s);
    if let Ok(a) = ComponentAddress::from_str(hex) {
        return Ok(a);
    }
    let bytes = Vec::from_hex(hex)
        .map_err(|e| anyhow::anyhow!("Invalid hex: {e}"))?;
    let arr: [u8; 32] = bytes.try_into()
        .map_err(|_| anyhow::anyhow!("Expected 32 bytes for component address"))?;
    Ok(ComponentAddress::new(arr.into()))
}

async fn handle_component_join(
    AxumState(state): AxumState<SharedState>,
    Json(body): Json<JoinComponentRequest>,
) -> impl IntoResponse {
    let component_addr = match parse_component_address(&body.component_address) {
        Ok(a) => a,
        Err(e) => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Invalid component address: {e}") })),
        ),
    };

    let normalized_template = match parse_template_address(&body.template_address) {
        Ok(_) => {
            let hex = body.template_address.strip_prefix("template_").unwrap_or(&body.template_address);
            format!("template_{hex}")
        }
        Err(e) => return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Invalid template address: {e}") })),
        ),
    };

    {
        let mut s = state.write().await;
        s.component_address = Some(component_addr);
        s.template_address = Some(normalized_template);
        s.setup_status = SetupStatus::Ready;
        save_state(&s, &s.state_file);
    }

    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
}

// ── GET /api/tx/events?tx_id=<hash> ───────────────────────────────────────────

#[derive(Deserialize)]
struct TxEventsQuery {
    tx_id: String,
}

async fn handle_tx_events(
    AxumState(state): AxumState<SharedState>,
    Query(params): Query<TxEventsQuery>,
) -> Json<serde_json::Value> {
    // We need the component address to scope the indexer query
    let component_addr = {
        let s = state.read().await;
        match &s.component_address {
            Some(a) => a.to_string(),
            None => return Json(serde_json::json!({"events": [], "error": "no component deployed"})),
        }
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    let resp: serde_json::Value = match async {
        client
            .get(format!("{}transactions/events", INDEXER_URL))
            .query(&[("substate_id", component_addr.as_str()), ("limit", "1000")])
            .send()
            .await?
            .json::<serde_json::Value>()
            .await
    }.await {
        Ok(v) => v,
        Err(e) => return Json(serde_json::json!({"events": [], "error": e.to_string()})),
    };

    let all_events = match resp["events"].as_array() {
        Some(arr) => arr.clone(),
        None => return Json(serde_json::json!({"events": [], "error": "unexpected indexer response"})),
    };

    // Filter to events for this tx_id (tuple format: [tx_id, event_obj] or just event_obj)
    let tx_id = &params.tx_id;
    let matching: Vec<serde_json::Value> = all_events
        .into_iter()
        .filter_map(|tuple| {
            if let Some(arr) = tuple.as_array() {
                // [tx_id_str, event_obj]
                if arr.len() >= 2 && arr[0].as_str() == Some(tx_id.as_str()) {
                    return Some(arr[1].clone());
                }
            }
            // plain event_obj with no tx_id wrapper — cannot filter, skip
            None
        })
        .collect();

    Json(serde_json::json!({ "events": matching }))
}

// ── GET /api/debug ────────────────────────────────────────────────────────────

async fn handle_debug(AxumState(state): AxumState<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let total_fees: u64 = s.tx_history.iter().map(|t| t.fee).sum();
    let wallets: Vec<WalletInfo> = s.wallets.values().map(WalletInfo::from).collect();
    Json(serde_json::json!({
        "network": "Esmeralda Testnet",
        "indexer_url": INDEXER_URL,
        "total_transactions": s.tx_history.len(),
        "total_fees_micro_tari": total_fees,
        "total_fees_tari": total_fees as f64 / 1_000_000.0,
        "fee_breakdown": {
            "faucet": "500 µTARI per wallet",
            "publish_template": "~250,000 µTARI",
            "deploy_component": "2,000 µTARI",
            "send_dm": "2,000 µTARI per message",
            "create_room": "2,000 µTARI",
            "post_to_room": "2,000 µTARI per post"
        },
        "wallets": wallets,
        "template_address": s.template_address,
        "component_address": s.component_address.as_ref().map(|a| a.to_string()),
        "transactions": s.tx_history,
    }))
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse --port and --state from CLI args, with defaults.
    let args: Vec<String> = std::env::args().collect();
    let mut port: u16 = 3000;
    let mut state_file = "./messaging-state.json".to_string();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                i += 1;
                port = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(3000);
            }
            "--state" => {
                i += 1;
                if let Some(s) = args.get(i) {
                    state_file = s.clone();
                }
            }
            _ => {}
        }
        i += 1;
    }

    println!("=== Tari Messenger ===");
    println!("Network: {NETWORK:?} | Indexer: {INDEXER_URL}");
    println!("Port: {port} | State: {state_file}");

    let mut state = load_state(&state_file);
    state.state_file = state_file.clone();
    println!("State loaded. Status: {:?}", state.setup_status);

    let shared = Arc::new(RwLock::new(state));

    // Recalculate status + deploy component if needed
    let init_state = Arc::clone(&shared);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(run_initialization(init_state));
    });

    // Background chain polling — syncs on-chain messages from all clients every 10s
    tokio::spawn(poll_chain_messages(Arc::clone(&shared)));

    let app = Router::new()
        .route("/", get(handle_index))
        .route("/instructions", get(handle_instructions))
        .route("/api/status", get(handle_status))
        .route("/api/wallets", get(handle_wallets))
        .route("/api/wallet/create", post(handle_wallet_create))
        .route("/api/wallet/import", post(handle_wallet_import))
        .route("/api/wallet/passphrase", post(handle_wallet_passphrase))
        .route("/api/wallet/faucet", post(handle_wallet_faucet))
        .route("/api/template/configure", post(handle_configure))
        .route("/api/template/publish", post(handle_publish))
        .route("/api/dm/send", post(handle_dm_send))
        .route("/api/dm/messages", get(handle_dm_messages))
        .route("/api/dm/inbox", get(handle_dm_inbox))
        .route("/api/room/create", post(handle_room_create))
        .route("/api/room/join", post(handle_room_join))
        .route("/api/room/post", post(handle_room_post))
        .route("/api/room/messages", get(handle_room_messages))
        .route("/api/rooms", get(handle_rooms))
        .route("/api/contacts", get(handle_contacts))
        .route("/api/contacts/set", post(handle_set_contact))
        .route("/api/demo/start", post(handle_demo_start))
        .route("/api/wallet/balance", get(handle_wallet_balance))
        .route("/api/wallet/export-keys", get(handle_wallet_export_keys))
        .route("/api/wallet/import-mnemonic", post(handle_wallet_import_mnemonic))
        .route("/api/component/join", post(handle_component_join))
        .route("/api/sync", post(handle_force_sync))
        .route("/api/debug", get(handle_debug))
        .route("/api/settings", get(handle_get_settings).post(handle_post_settings))
        .route("/api/public-config", get(handle_public_config))
        .route("/api/tx/events", get(handle_tx_events))
        .layer(CorsLayer::permissive())
        .with_state(Arc::clone(&shared));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("\nOpen your browser at: http://localhost:{port}");
    println!("(Create or import a wallet to get started)\n");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
