#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use super::vault::{self, VaultEntry};
use crate::lock_ext::LockExt;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use zbus::message::Header;
use zbus::object_server::SignalContext;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Type, Value};
use zbus::{fdo, interface, Connection, ObjectServer};

pub const BUS_NAME: &str = "org.freedesktop.secrets";
pub const SERVICE_PATH: &str = "/org/freedesktop/secrets";
pub const COLLECTION_PATH: &str = "/org/freedesktop/secrets/collection/login";
pub const ALIAS_PATH: &str = "/org/freedesktop/secrets/aliases/default";
const ITEM_PREFIX: &str = "/org/freedesktop/secrets/collection/login/";
const SESSION_PREFIX: &str = "/org/freedesktop/secrets/session/";
const PROMPT_PREFIX: &str = "/org/freedesktop/secrets/prompt/";
const NO_PROMPT: &str = "/";
const ITEM_LABEL: &str = "org.freedesktop.Secret.Item.Label";
const ITEM_ATTRS: &str = "org.freedesktop.Secret.Item.Attributes";

// ── pure helpers (unit-tested) ─────────────────────────────────────────────

/// Escapes an entry id into a legal D-Bus object-path element
/// (`[A-Za-z0-9_]+`). Every other byte, and `_` itself, becomes `_xx`.
pub fn escape_id(id: &str) -> String {
    let mut out = String::with_capacity(id.len() + 4);
    for b in id.bytes() {
        if b.is_ascii_alphanumeric() {
            out.push(b as char);
        } else {
            out.push_str(&format!("_{b:02x}"));
        }
    }
    if out.is_empty() {
        out.push_str("_00");
    }
    out
}

pub fn unescape_id(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'_' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else if b[i].is_ascii_alphanumeric() {
            out.push(b[i]);
            i += 1;
        } else {
            return None;
        }
    }
    String::from_utf8(out).ok()
}

pub fn item_path(id: &str) -> String {
    format!("{ITEM_PREFIX}{}", escape_id(id))
}

pub fn id_from_item_path(path: &str) -> Option<String> {
    unescape_id(path.strip_prefix(ITEM_PREFIX)?)
}

/// Content type to report for an entry (spec requires one).
pub fn content_type_of(e: &VaultEntry) -> String {
    if e.content_type.is_empty() { "text/plain".to_string() } else { e.content_type.clone() }
}

/// Parses an RFC 3339 timestamp to unix seconds (0 when unparsable).
pub fn unix_secs(ts: &str) -> u64 {
    chrono::DateTime::parse_from_rfc3339(ts).map(|d| d.timestamp().max(0) as u64).unwrap_or(0)
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Application name for a D-Bus peer: `/proc/<pid>/comm`.
pub fn process_name(pid: u32) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/comm")).map(|s| s.trim().to_string()).ok().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("pid {pid}"))
}

// ── audit log ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessRecord {
    pub time: String,
    pub client: String,
    pub action: String,
    pub item: String,
}

const LOG_CAP: usize = 100;
static ACCESS_LOG: Mutex<VecDeque<AccessRecord>> = Mutex::new(VecDeque::new());

pub fn note_access(client: &str, action: &str, item: &str) {
    let mut log = ACCESS_LOG.lock_safe();
    if log.len() >= LOG_CAP {
        log.pop_front();
    }
    log.push_back(AccessRecord { time: now_rfc3339(), client: client.to_string(), action: action.to_string(), item: item.to_string() });
}

pub fn access_log() -> Vec<AccessRecord> {
    ACCESS_LOG.lock_safe().iter().rev().cloned().collect()
}

// ── prompt broker (UI ⇄ D-Bus) ─────────────────────────────────────────────

pub type Emitter = Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>;

/// `true` = the vault was unlocked, `false` = the person dismissed the prompt.
static PENDING: OnceLock<Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>> = OnceLock::new();
fn pending() -> &'static Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>> {
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Called by the `secrets_prompt_reply` command.
///
/// * `password = Some(..)` → try to unlock (or, with `create`, create the vault).
///   Wrong password → `Err`, the prompt stays open so the dialog can retry.
/// * `password = None` → dismissed.
pub fn prompt_reply(prompt_id: &str, password: Option<String>, create: bool) -> Result<(), String> {
    if !pending().lock_safe().contains_key(prompt_id) {
        return Err("This request is no longer pending".to_string());
    }
    let outcome = match password {
        Some(pw) => {
            if create || !vault::vault_exists() {
                vault::create_vault(&pw).map_err(|e| e.to_string())?;
            } else {
                vault::unlock(&pw).map_err(|e| e.to_string())?;
            }
            true
        }
        None => false,
    };
    if let Some(tx) = pending().lock_safe().remove(prompt_id) {
        let _ = tx.send(outcome);
    }
    Ok(())
}

// ── D-Bus types ────────────────────────────────────────────────────────────

/// `(oayays)` — session, parameters, value, content type.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Secret {
    pub session: OwnedObjectPath,
    pub parameters: Vec<u8>,
    pub value: Vec<u8>,
    pub content_type: String,
}

struct Shared {
    emit: Emitter,
    sessions: Mutex<HashSet<String>>,
    registered: Mutex<HashSet<String>>,
    seq: AtomicU64,
}

impl Shared {
    fn next(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::Relaxed) + 1
    }
}

fn ctx_err<E: std::fmt::Display>(e: E) -> fdo::Error {
    fdo::Error::Failed(e.to_string())
}

fn op(path: &str) -> OwnedObjectPath {
    ObjectPath::try_from(path.to_string()).map(OwnedObjectPath::from).unwrap_or_else(|_| OwnedObjectPath::from(ObjectPath::from_static_str_unchecked("/")))
}

async fn caller(conn: &Connection, hdr: &Header<'_>) -> String {
    let Some(sender) = hdr.sender() else { return "unknown".into() };
    let Ok(proxy) = fdo::DBusProxy::new(conn).await else { return sender.to_string() };
    match proxy.get_connection_unix_process_id(sender.clone().into()).await {
        Ok(pid) => process_name(pid),
        Err(_) => sender.to_string(),
    }
}

fn require_unlocked() -> fdo::Result<()> {
    if vault::is_unlocked() {
        Ok(())
    } else {
        Err(fdo::Error::Failed("org.freedesktop.Secret.Error.IsLocked: the vault is locked".into()))
    }
}

fn require_session(shared: &Shared, session: &ObjectPath<'_>) -> fdo::Result<()> {
    if shared.sessions.lock_safe().contains(session.as_str()) {
        Ok(())
    } else {
        Err(fdo::Error::Failed("org.freedesktop.Secret.Error.NoSession: unknown session".into()))
    }
}

fn secret_for(entry: &VaultEntry, session: &ObjectPath<'_>) -> Secret {
    Secret {
        session: OwnedObjectPath::from(session.to_owned()),
        parameters: Vec::new(),
        value: entry.password.clone().into_bytes(),
        content_type: content_type_of(entry),
    }
}

/// Registers D-Bus objects for every Secret Service entry and removes the ones
/// that no longer exist (or all of them while the vault is locked).
async fn sync_items(shared: &Shared, server: &ObjectServer) {
    let wanted: Vec<VaultEntry> = if vault::is_unlocked() { vault::find_by_attributes(&BTreeMap::new()).unwrap_or_default() } else { Vec::new() };
    let wanted_paths: HashSet<String> = wanted.iter().map(|e| item_path(&e.id)).collect();
    let current: Vec<String> = shared.registered.lock_safe().iter().cloned().collect();
    for p in current {
        if !wanted_paths.contains(&p) {
            let _ = server.remove::<Item, _>(p.as_str()).await;
            shared.registered.lock_safe().remove(&p);
        }
    }
    for e in wanted {
        let p = item_path(&e.id);
        if shared.registered.lock_safe().contains(&p) {
            continue;
        }
        if server.at(p.as_str(), Item { id: e.id.clone() }).await.unwrap_or(false) {
            shared.registered.lock_safe().insert(p);
        }
    }
}

/// Turns the `CreateItem` arguments into a vault entry.
fn build_entry(properties: &HashMap<String, OwnedValue>, secret: &Secret, who: &str) -> fdo::Result<VaultEntry> {
    let label = properties.get(ITEM_LABEL).and_then(|v| String::try_from(v.try_clone().ok()?).ok()).unwrap_or_default();
    let attributes: BTreeMap<String, String> = properties
        .get(ITEM_ATTRS)
        .and_then(|v| HashMap::<String, String>::try_from(v.try_clone().ok()?).ok())
        .map(|m| m.into_iter().collect())
        .unwrap_or_default();
    let now = now_rfc3339();
    Ok(VaultEntry {
        id: uuid::Uuid::new_v4().to_string(),
        title: if label.is_empty() { "Secret".into() } else { label },
        password: String::from_utf8(secret.value.clone()).map_err(|_| fdo::Error::InvalidArgs("secret is not valid UTF-8".into()))?,
        created_at: now.clone(),
        updated_at: now,
        content_type: secret.content_type.clone(),
        attributes,
        created_by: who.to_string(),
        ..Default::default()
    })
}

// ── Service ────────────────────────────────────────────────────────────────

struct Service {
    shared: Arc<Shared>,
}

#[interface(name = "org.freedesktop.Secret.Service")]
impl Service {
    async fn open_session(&self, algorithm: String, _input: Value<'_>, #[zbus(object_server)] server: &ObjectServer) -> fdo::Result<(OwnedValue, OwnedObjectPath)> {
        if algorithm != "plain" {
            return Err(fdo::Error::NotSupported(format!("session algorithm {algorithm} is not supported; use plain")));
        }
        let path = format!("{SESSION_PREFIX}s{}", self.shared.next());
        server.at(path.as_str(), Session { shared: self.shared.clone(), path: path.clone() }).await.map_err(ctx_err)?;
        self.shared.sessions.lock_safe().insert(path.clone());
        let out = OwnedValue::try_from(Value::from("")).map_err(ctx_err)?;
        Ok((out, op(&path)))
    }

    async fn create_collection(&self, _properties: HashMap<String, OwnedValue>, alias: String) -> fdo::Result<(OwnedObjectPath, OwnedObjectPath)> {
        if alias == "default" || alias.is_empty() {
            Ok((op(COLLECTION_PATH), op(NO_PROMPT)))
        } else {
            Err(fdo::Error::NotSupported("Blue Vault has a single collection".into()))
        }
    }

    async fn search_items(
        &self,
        attributes: HashMap<String, String>,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> fdo::Result<(Vec<OwnedObjectPath>, Vec<OwnedObjectPath>)> {
        let who = caller(conn, &hdr).await;
        if !vault::is_unlocked() {
            // Nothing can be listed while locked. Nudge the person instead of
            // failing silently: the shell shows "<app> wants a secret — unlock?".
            (self.shared.emit)("secrets:unlock-hint", serde_json::json!({ "client": who }));
            return Ok((Vec::new(), Vec::new()));
        }
        sync_items(&self.shared, server).await;
        let query: BTreeMap<String, String> = attributes.into_iter().collect();
        let found = vault::find_by_attributes(&query).map_err(ctx_err)?;
        note_access(&who, "search", &format!("{} match(es)", found.len()));
        Ok((found.iter().map(|e| op(&item_path(&e.id))).collect(), Vec::new()))
    }

    async fn unlock(&self, objects: Vec<OwnedObjectPath>, #[zbus(object_server)] server: &ObjectServer) -> fdo::Result<(Vec<OwnedObjectPath>, OwnedObjectPath)> {
        if vault::is_unlocked() {
            sync_items(&self.shared, server).await;
            return Ok((objects, op(NO_PROMPT)));
        }
        let id = format!("u{}", self.shared.next());
        let path = format!("{PROMPT_PREFIX}{id}");
        server.at(path.as_str(), Prompt { shared: self.shared.clone(), id, result: objects, pending_item: None }).await.map_err(ctx_err)?;
        Ok((Vec::new(), op(&path)))
    }

    async fn lock(&self, objects: Vec<OwnedObjectPath>, #[zbus(object_server)] server: &ObjectServer) -> fdo::Result<(Vec<OwnedObjectPath>, OwnedObjectPath)> {
        vault::lock();
        sync_items(&self.shared, server).await;
        Ok((objects, op(NO_PROMPT)))
    }

    async fn get_secrets(
        &self,
        items: Vec<OwnedObjectPath>,
        session: OwnedObjectPath,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Header<'_>,
    ) -> fdo::Result<HashMap<OwnedObjectPath, Secret>> {
        require_session(&self.shared, &session)?;
        require_unlocked()?;
        let who = caller(conn, &hdr).await;
        let mut out = HashMap::new();
        for p in items {
            let Some(id) = id_from_item_path(p.as_str()) else { continue };
            if let Some(e) = vault::get_entry(&id).map_err(ctx_err)?.filter(|e| e.is_secret_service()) {
                note_access(&who, "read", &e.title);
                out.insert(p.clone(), secret_for(&e, &session));
            }
        }
        Ok(out)
    }

    async fn read_alias(&self, name: String) -> OwnedObjectPath {
        if name == "default" { op(COLLECTION_PATH) } else { op(NO_PROMPT) }
    }

    async fn set_alias(&self, _name: String, _collection: OwnedObjectPath) {}

    #[zbus(property)]
    async fn collections(&self) -> Vec<OwnedObjectPath> {
        vec![op(COLLECTION_PATH)]
    }
}

// ── Collection ─────────────────────────────────────────────────────────────

struct Collection {
    shared: Arc<Shared>,
}

#[interface(name = "org.freedesktop.Secret.Collection")]
impl Collection {
    async fn delete(&self) -> fdo::Result<OwnedObjectPath> {
        Err(fdo::Error::NotSupported("the default collection cannot be deleted".into()))
    }

    async fn search_items(&self, attributes: HashMap<String, String>, #[zbus(object_server)] server: &ObjectServer) -> fdo::Result<Vec<OwnedObjectPath>> {
        if !vault::is_unlocked() {
            return Ok(Vec::new());
        }
        sync_items(&self.shared, server).await;
        let query: BTreeMap<String, String> = attributes.into_iter().collect();
        Ok(vault::find_by_attributes(&query).map_err(ctx_err)?.iter().map(|e| op(&item_path(&e.id))).collect())
    }

    async fn create_item(
        &self,
        properties: HashMap<String, OwnedValue>,
        secret: Secret,
        replace: bool,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> fdo::Result<(OwnedObjectPath, OwnedObjectPath)> {
        require_session(&self.shared, &secret.session.as_ref())?;
        let who = caller(conn, &hdr).await;
        let entry = build_entry(&properties, &secret, &who)?;
        if !vault::is_unlocked() {
            // The client should have unlocked first; hand back an unlock prompt
            // that also carries out the create once the person unlocks.
            let id = format!("u{}", self.shared.next());
            let path = format!("{PROMPT_PREFIX}{id}");
            server
                .at(path.as_str(), Prompt { shared: self.shared.clone(), id, result: Vec::new(), pending_item: Some((entry, replace)) })
                .await
                .map_err(ctx_err)?;
            return Ok((op(NO_PROMPT), op(&path)));
        }
        let title = entry.title.clone();
        let id = vault::upsert_secret_item(entry, replace).map_err(ctx_err)?;
        note_access(&who, "store", &title);
        sync_items(&self.shared, server).await;
        Ok((op(&item_path(&id)), op(NO_PROMPT)))
    }

    #[zbus(property)]
    async fn items(&self) -> Vec<OwnedObjectPath> {
        if !vault::is_unlocked() {
            return Vec::new();
        }
        vault::find_by_attributes(&BTreeMap::new()).unwrap_or_default().iter().map(|e| op(&item_path(&e.id))).collect()
    }

    #[zbus(property)]
    async fn label(&self) -> String {
        "Blue Vault".to_string()
    }
    #[zbus(property)]
    async fn set_label(&mut self, _label: String) {}

    #[zbus(property)]
    async fn locked(&self) -> bool {
        !vault::is_unlocked()
    }
    #[zbus(property)]
    async fn created(&self) -> u64 {
        0
    }
    #[zbus(property)]
    async fn modified(&self) -> u64 {
        0
    }
}

// ── Item ───────────────────────────────────────────────────────────────────

struct Item {
    id: String,
}

impl Item {
    fn entry(&self) -> Option<VaultEntry> {
        vault::get_entry(&self.id).ok().flatten()
    }
}

#[interface(name = "org.freedesktop.Secret.Item")]
impl Item {
    async fn delete(&self, #[zbus(connection)] conn: &Connection, #[zbus(header)] hdr: Header<'_>) -> fdo::Result<OwnedObjectPath> {
        require_unlocked()?;
        let who = caller(conn, &hdr).await;
        let title = self.entry().map(|e| e.title).unwrap_or_default();
        vault::delete_cached(&self.id).map_err(ctx_err)?;
        note_access(&who, "delete", &title);
        Ok(op(NO_PROMPT))
    }

    async fn get_secret(&self, session: OwnedObjectPath, #[zbus(connection)] conn: &Connection, #[zbus(header)] hdr: Header<'_>) -> fdo::Result<(Secret,)> {
        require_unlocked()?;
        let e = self.entry().ok_or_else(|| fdo::Error::Failed("org.freedesktop.Secret.Error.NoSuchObject".into()))?;
        let who = caller(conn, &hdr).await;
        note_access(&who, "read", &e.title);
        // A single struct out-argument must be wrapped in a 1-tuple, otherwise
        // zbus flattens its fields into four separate out-arguments.
        Ok((secret_for(&e, &session),))
    }

    async fn set_secret(&self, secret: Secret, #[zbus(connection)] conn: &Connection, #[zbus(header)] hdr: Header<'_>) -> fdo::Result<()> {
        require_unlocked()?;
        let who = caller(conn, &hdr).await;
        let value = String::from_utf8(secret.value).map_err(|_| fdo::Error::InvalidArgs("secret is not valid UTF-8".into()))?;
        vault::set_secret(&self.id, &value, &secret.content_type, &now_rfc3339()).map_err(ctx_err)?;
        note_access(&who, "update", &self.entry().map(|e| e.title).unwrap_or_default());
        Ok(())
    }

    #[zbus(property)]
    async fn locked(&self) -> bool {
        !vault::is_unlocked()
    }
    #[zbus(property)]
    async fn attributes(&self) -> HashMap<String, String> {
        self.entry().map(|e| e.attributes.into_iter().collect()).unwrap_or_default()
    }
    #[zbus(property)]
    async fn set_attributes(&mut self, attrs: HashMap<String, String>) {
        let _ = vault::set_title_and_attributes(&self.id, None, Some(attrs.into_iter().collect()), &now_rfc3339());
    }
    #[zbus(property)]
    async fn label(&self) -> String {
        self.entry().map(|e| e.title).unwrap_or_default()
    }
    #[zbus(property)]
    async fn set_label(&mut self, label: String) {
        let _ = vault::set_title_and_attributes(&self.id, Some(&label), None, &now_rfc3339());
    }
    #[zbus(property)]
    async fn created(&self) -> u64 {
        self.entry().map(|e| unix_secs(&e.created_at)).unwrap_or(0)
    }
    #[zbus(property)]
    async fn modified(&self) -> u64 {
        self.entry().map(|e| unix_secs(&e.updated_at)).unwrap_or(0)
    }
}

// ── Session ────────────────────────────────────────────────────────────────

struct Session {
    shared: Arc<Shared>,
    path: String,
}

#[interface(name = "org.freedesktop.Secret.Session")]
impl Session {
    async fn close(&self, #[zbus(object_server)] server: &ObjectServer) -> fdo::Result<()> {
        self.shared.sessions.lock_safe().remove(&self.path);
        let _ = server.remove::<Session, _>(self.path.as_str()).await;
        Ok(())
    }
}

// ── Prompt ─────────────────────────────────────────────────────────────────

struct Prompt {
    shared: Arc<Shared>,
    id: String,
    /// Objects reported back by an `Unlock` prompt (`ao`).
    result: Vec<OwnedObjectPath>,
    /// A `CreateItem` that arrived while the vault was locked: performed once
    /// the prompt unlocks it, and the prompt then completes with the new item's
    /// path (`o`), which is what clients expect from a create prompt.
    pending_item: Option<(VaultEntry, bool)>,
}

#[interface(name = "org.freedesktop.Secret.Prompt")]
impl Prompt {
    async fn prompt(&self, window_id: String, #[zbus(connection)] conn: &Connection, #[zbus(header)] hdr: Header<'_>) -> fdo::Result<()> {
        let who = caller(conn, &hdr).await;
        let (tx, rx) = tokio::sync::oneshot::channel();
        pending().lock_safe().insert(self.id.clone(), tx);
        (self.shared.emit)(
            "secrets:unlock-request",
            serde_json::json!({ "promptId": self.id, "client": who, "windowId": window_id, "vaultExists": vault::vault_exists() }),
        );
        let conn = conn.clone();
        let path = format!("{PROMPT_PREFIX}{}", self.id);
        let id = self.id.clone();
        let result = self.result.clone();
        let pending_item = self.pending_item.clone();
        let had_pending = pending_item.is_some();
        let shared = self.shared.clone();
        let who_for_log = who.clone();
        tokio::spawn(async move {
            // The dialog is answered through `prompt_reply`; give up after two minutes.
            let outcome = tokio::time::timeout(std::time::Duration::from_secs(120), rx).await;
            let unlocked = matches!(outcome, Ok(Ok(true)));
            pending().lock_safe().remove(&id);
            let server_guard = conn.object_server();
            let server: &ObjectServer = &server_guard;
            // Deferred `CreateItem`: now that the vault is open, store the secret.
            let mut created_path: Option<String> = None;
            if unlocked {
                if let Some((entry, replace)) = pending_item {
                    let title = entry.title.clone();
                    if let Ok(item_id) = vault::upsert_secret_item(entry, replace) {
                        note_access(&who_for_log, "store", &title);
                        created_path = Some(item_path(&item_id));
                    }
                }
                sync_items(&shared, server).await;
            }
            if let Ok(ctx) = SignalContext::new(&conn, path.as_str()) {
                match (&created_path, unlocked) {
                    (Some(p), true) => {
                        if let Ok(op) = ObjectPath::try_from(p.as_str()) {
                            let _ = Prompt::completed(&ctx, false, Value::from(op)).await;
                        }
                    }
                    (None, true) if !had_pending => {
                        let list: Vec<ObjectPath> = result.iter().map(|p| ObjectPath::from(p.as_ref())).collect();
                        let _ = Prompt::completed(&ctx, false, Value::from(list)).await;
                    }
                    _ => {
                        let _ = Prompt::completed(&ctx, true, Value::from(ObjectPath::from_static_str_unchecked("/"))).await;
                    }
                }
            }
            (shared.emit)("secrets:prompt-done", serde_json::json!({ "promptId": id, "unlocked": unlocked }));
            // Let the client read the signal before the object disappears.
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let _ = server.remove::<Prompt, _>(path.as_str()).await;
        });
        Ok(())
    }

    async fn dismiss(&self) -> fdo::Result<()> {
        if let Some(tx) = pending().lock_safe().remove(&self.id) {
            let _ = tx.send(false);
        }
        Ok(())
    }

    #[zbus(signal)]
    async fn completed(ctx: &SignalContext<'_>, dismissed: bool, result: Value<'_>) -> zbus::Result<()>;
}

// ── entry point ────────────────────────────────────────────────────────────

/// Connects to the session bus, claims `org.freedesktop.secrets` and serves the
/// API until the connection drops. Returns `Ok(None)` (after logging) when
/// another keyring already owns the name.
pub async fn run(emit: Emitter) -> zbus::Result<Option<Connection>> {
    let shared = Arc::new(Shared { emit, sessions: Mutex::new(HashSet::new()), registered: Mutex::new(HashSet::new()), seq: AtomicU64::new(0) });
    let built = zbus::connection::Builder::session()?
        .name(BUS_NAME)?
        .serve_at(SERVICE_PATH, Service { shared: shared.clone() })?
        .serve_at(COLLECTION_PATH, Collection { shared: shared.clone() })?
        .serve_at(ALIAS_PATH, Collection { shared: shared.clone() })?
        .build()
        .await;
    match built {
        Ok(conn) => {
            tracing::info!("Secret Service: serving {BUS_NAME}");
            Ok(Some(conn))
        }
        Err(zbus::Error::NameTaken) => {
            tracing::warn!("Secret Service: {BUS_NAME} is already owned by another keyring; Blue Vault will not take it over");
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_escaping_round_trips_and_is_a_legal_object_path_element() {
        for id in ["3f2b9c1e-7a44-4d0e-9a55-aa11bb22cc33", "a_b", "ünï/cødé", "x"] {
            let esc = escape_id(id);
            assert!(esc.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'), "{esc}");
            assert_eq!(unescape_id(&esc).as_deref(), Some(id));
        }
        assert!(ObjectPath::try_from(item_path("3f2b9c1e-7a44")).is_ok());
    }

    #[test]
    fn distinct_ids_never_share_a_path() {
        assert_ne!(escape_id("a-b"), escape_id("a_2db"));
        assert_ne!(escape_id("a_b"), escape_id("a-b"));
    }

    #[test]
    fn item_paths_map_back_to_ids_and_foreign_paths_are_rejected() {
        let p = item_path("abc-123");
        assert_eq!(id_from_item_path(&p).as_deref(), Some("abc-123"));
        assert_eq!(id_from_item_path("/org/freedesktop/secrets/collection/other/abc"), None);
        assert_eq!(id_from_item_path(&format!("{ITEM_PREFIX}bad-char")), None);
    }

    #[test]
    fn timestamps_parse_or_fall_back_to_zero() {
        assert_eq!(unix_secs("2026-01-01T00:00:00Z"), 1767225600);
        assert_eq!(unix_secs("garbage"), 0);
    }

    #[test]
    fn access_log_is_capped_and_newest_first() {
        for i in 0..(LOG_CAP + 10) {
            note_access("app", "read", &format!("item{i}"));
        }
        let log = access_log();
        assert_eq!(log.len(), LOG_CAP);
        assert_eq!(log[0].item, format!("item{}", LOG_CAP + 9));
    }

    #[test]
    fn replying_to_an_unknown_prompt_is_an_error() {
        assert!(prompt_reply("nope", None, false).is_err());
    }
}
