/*
 * lib.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte, a cross-platform email client.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Tagliacarte is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Tagliacarte.  If not, see <http://www.gnu.org/licenses/>.
 */

//! C FFI for tagliacarte core. Stores, folders, and transports are identified by URI.
//! Create functions return a newly allocated URI string (free with tagliacarte_free_string).
//! All string parameters are UTF-8 NUL-terminated.

use libc::{c_char, c_int, c_void, size_t};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::ops::Range;
use std::ptr;
use std::sync::{Arc, RwLock};
use tagliacarte_core::localstorage::maildir::MaildirStore;
use tagliacarte_core::message_id::MessageId;
use tagliacarte_core::protocol::imap::ImapStore;
use tagliacarte_core::protocol::matrix::{MatrixStore, MatrixTransport};
use tagliacarte_core::protocol::pop3::Pop3Store;
use tagliacarte_core::protocol::nostr::{NostrStore, NostrTransport};
use tagliacarte_core::protocol::smtp::SmtpTransport;
use tagliacarte_core::config::{
    credentials_use_keychain, default_credentials_path, keychain_available, load_credentials,
    migrate_credentials_to_file, migrate_credentials_to_keychain, save_credential,
    set_credentials_backend,
};
use tagliacarte_core::mime::MimeParser;
use tagliacarte_core::store::{
    Address, Attachment, ConversationSummary, Envelope, Folder, FolderInfo, OpenFolderEvent,
    SendPayload, SendSession, Store, StoreError, Transport,
};
use tagliacarte_core::uri::{folder_uri, imap_store_uri, maildir_store_uri, pop3_store_uri, smtp_transport_uri};

/// Wrapper so *mut c_void can be moved into Send closures (e.g. thread::spawn). C callbacks are invoked from worker threads.
struct SendableUserData(*mut c_void);
unsafe impl Send for SendableUserData {}
unsafe impl Sync for SendableUserData {}

/// Callbacks for folder list (event-driven). Callbacks may run on a backend thread; UI must marshal to main thread.
type OnFolderFound = extern "C" fn(*const c_char, c_char, *const c_char, *mut c_void);
type OnFolderRemoved = extern "C" fn(*const c_char, *mut c_void);
type OnFolderListComplete = extern "C" fn(c_int, *mut c_void);

/// Callback for folder operation errors (create/rename/delete).
type OnFolderOpError = extern "C" fn(*const c_char, *mut c_void);

/// Async open folder: on_select_event (optional), on_folder_ready(folder_uri), on_error(message).
type OnOpenFolderSelectEvent = extern "C" fn(c_int, u32, *const c_char, *mut c_void);
type OnFolderReady = extern "C" fn(*const c_char, *mut c_void);
type OnOpenFolderError = extern "C" fn(*const c_char, *mut c_void);

#[allow(dead_code)]
struct FolderListCallbacks {
    on_folder_found: OnFolderFound,
    on_folder_removed: OnFolderRemoved,
    on_complete: OnFolderListComplete,
    user_data: *mut c_void,
}

/// Callbacks for message list (event-driven).
type OnMessageSummary = extern "C" fn(*const c_char, *const c_char, *const c_char, i64, u64, *mut c_void);
type OnMessageListComplete = extern "C" fn(c_int, *mut c_void);

#[allow(dead_code)]
struct MessageListCallbacks {
    on_message_summary: OnMessageSummary,
    on_complete: OnMessageListComplete,
    user_data: *mut c_void,
}

/// Callbacks for get message (event-driven). on_metadata, then MIME entity events (mirroring MimeHandler), then on_complete.
type OnMessageMetadata = extern "C" fn(*const c_char, *const c_char, *const c_char, *const c_char, *mut c_void);
type OnStartEntity = extern "C" fn(*mut c_void);
type OnContentType = extern "C" fn(*const c_char, *mut c_void);
type OnContentDisposition = extern "C" fn(*const c_char, *mut c_void);
type OnContentId = extern "C" fn(*const c_char, *mut c_void);
type OnEndHeaders = extern "C" fn(*mut c_void);
type OnBodyContent = extern "C" fn(*const u8, size_t, *mut c_void);
type OnEndEntity = extern "C" fn(*mut c_void);
type OnMessageComplete = extern "C" fn(c_int, *mut c_void);

#[allow(dead_code)]
struct MessageCallbacks {
    on_metadata: OnMessageMetadata,
    on_start_entity: OnStartEntity,
    on_content_type: OnContentType,
    on_content_disposition: OnContentDisposition,
    on_content_id: OnContentId,
    on_end_headers: OnEndHeaders,
    on_body_content: OnBodyContent,
    on_end_entity: OnEndEntity,
    on_complete: OnMessageComplete,
    user_data: *mut c_void,
}

/// Auth type for credential request callback. 0 = Auto (password-based SASL).
pub const TAGLIACARTE_AUTH_TYPE_AUTO: c_int = 0;

/// Credential request callback: core needs a credential. UI should show dialog, then call tagliacarte_credential_provide or tagliacarte_credential_cancel.
/// Called from the thread that is performing the connect (may be a worker thread; marshal to main thread if needed).
/// store_uri: store (or transport) URI. auth_type: TAGLIACARTE_AUTH_TYPE_*. is_plaintext: 1 if connection is plaintext (show warning). username: for pre-fill.
type CredentialRequestCallback = extern "C" fn(
    *const c_char,
    c_int,
    c_int,
    *const c_char,
    *mut c_void,
);

static CREDENTIAL_REQUEST_CALLBACK: once_cell::sync::OnceCell<std::sync::Mutex<Option<(CredentialRequestCallback, SendableUserData)>>> =
    once_cell::sync::OnceCell::new();

fn with_credential_callback<R, F: FnOnce(Option<&(CredentialRequestCallback, SendableUserData)>) -> R>(f: F) -> R {
    let m = CREDENTIAL_REQUEST_CALLBACK.get_or_init(|| std::sync::Mutex::new(None));
    let guard = m.lock().ok();
    f(guard.as_ref().and_then(|g| g.as_ref()))
}

fn invoke_credential_request(store_uri: &str, auth_type: c_int, is_plaintext: bool, username: &str) {
    with_credential_callback(|opt| {
        if let Some((cb, user_data)) = opt {
            let uri_c = CString::new(store_uri).unwrap_or_else(|_| CString::new("").unwrap());
            let user_c = CString::new(username).unwrap_or_else(|_| CString::new("").unwrap());
            (cb)(uri_c.as_ptr(), auth_type, if is_plaintext { 1 } else { 0 }, user_c.as_ptr(), user_data.0);
        }
    });
}

/// Error code returned when credential is required. UI should show password dialog and then call tagliacarte_credential_provide or tagliacarte_credential_cancel.
pub const TAGLIACARTE_NEEDS_CREDENTIAL: c_int = -2;

/// Send-safe copy of callback structs (user_data as usize) for use in worker threads.
#[derive(Clone)]
struct FolderListCallbacksSend {
    on_folder_found: OnFolderFound,
    #[allow(dead_code)]
    on_folder_removed: OnFolderRemoved,
    on_complete: OnFolderListComplete,
    user_data: usize,
}
#[derive(Clone)]
struct MessageListCallbacksSend {
    on_message_summary: OnMessageSummary,
    on_complete: OnMessageListComplete,
    user_data: usize,
}
#[derive(Clone)]
struct MessageCallbacksSend {
    on_metadata: OnMessageMetadata,
    on_start_entity: OnStartEntity,
    on_content_type: OnContentType,
    on_content_disposition: OnContentDisposition,
    on_content_id: OnContentId,
    on_end_headers: OnEndHeaders,
    on_body_content: OnBodyContent,
    on_end_entity: OnEndEntity,
    on_complete: OnMessageComplete,
    user_data: usize,
}

/// Registry of stores, folders, and transports keyed by URI. Send sessions keyed by opaque session id.
/// Hosts the shared tokio runtime for all backend I/O.
struct Registry {
    runtime: tokio::runtime::Runtime,
    stores: RwLock<HashMap<String, Arc<StoreHolder>>>,
    folders: RwLock<HashMap<String, Arc<FolderHolder>>>,
    transports: RwLock<HashMap<String, Arc<TransportHolder>>>,
    send_sessions: RwLock<HashMap<String, Box<dyn SendSession>>>,
    send_session_counter: std::sync::atomic::AtomicU64,
}

fn registry() -> &'static Registry {
    static REGISTRY: once_cell::sync::OnceCell<Registry> = once_cell::sync::OnceCell::new();
    REGISTRY.get_or_init(|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("failed to create tokio runtime");
        Registry {
            runtime,
            stores: RwLock::new(HashMap::new()),
            folders: RwLock::new(HashMap::new()),
            transports: RwLock::new(HashMap::new()),
            send_sessions: RwLock::new(HashMap::new()),
            send_session_counter: std::sync::atomic::AtomicU64::new(0),
        }
    })
}

fn ptr_to_str(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string()) }
}

/// Holder for Store + optional event-driven callbacks. Callbacks stored as Send-safe (user_data as usize) so Arc<StoreHolder> is Send+Sync.
struct StoreHolder {
    store: Box<dyn Store>,
    folder_list_callbacks: RwLock<Option<FolderListCallbacksSend>>,
}

/// Holder for Folder + optional event-driven callbacks. Callbacks stored as Send-safe so Arc<FolderHolder> is Send+Sync.
struct FolderHolder {
    folder: Box<dyn Folder>,
    message_list_callbacks: RwLock<Option<MessageListCallbacksSend>>,
    message_callbacks: RwLock<Option<MessageCallbacksSend>>,
}

/// Holder for Arc<dyn Transport> (enables start_send_worker and streaming send for SMTP).
struct TransportHolder(Arc<dyn Transport>);

fn parse_address(s: &str) -> Address {
    let s = s.trim();
    if let Some(at) = s.find('@') {
        Address {
            display_name: None,
            local_part: s[..at].to_string(),
            domain: Some(s[at + 1..].to_string()),
        }
    } else {
        Address {
            display_name: None,
            local_part: s.to_string(),
            domain: None,
        }
    }
}

fn format_address_list(addrs: &[Address]) -> String {
    addrs
        .iter()
        .map(|a| -> String {
            let addr_part = match &a.domain {
                Some(d) if !d.is_empty() => format!("{}@{}", a.local_part, d),
                _ => a.local_part.clone(),
            };
            match &a.display_name {
                Some(dn) if !dn.is_empty() => {
                    if addr_part.is_empty() {
                        dn.clone()
                    } else {
                        format!("{} <{}>", dn, addr_part)
                    }
                }
                _ => addr_part,
            }
        })
        .collect::<Vec<String>>()
        .join(", ")
}

/// Format first address for display: display-name if present, otherwise addr-spec.
fn format_address_display(addrs: &[Address]) -> String {
    addrs
        .first()
        .map(|a| {
            let addr_part = match &a.domain {
                Some(d) if !d.is_empty() => format!("{}@{}", a.local_part, d),
                _ => a.local_part.clone(),
            };
            match &a.display_name {
                Some(dn) if !dn.is_empty() => dn.clone(),
                _ => addr_part,
            }
        })
        .unwrap_or_default()
}

thread_local! {
    static LAST_ERROR: std::cell::RefCell<Option<CString>> = std::cell::RefCell::new(None);
}

fn set_last_error(err: &StoreError) {
    let msg = CString::new(err.to_string()).unwrap_or_else(|_| CString::new("(error)").unwrap());
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(msg));
}

fn clear_last_error() {
    LAST_ERROR.with(|e| *e.borrow_mut() = None);
}

/// C struct for a single conversation summary (list view row).
#[repr(C)]
pub struct TagliacarteConversationSummary {
    pub id: *mut c_char,
    pub subject: *mut c_char,
    pub from_: *mut c_char,
    pub size: u64,
}

/// Single attachment in a received message (owned; freed by tagliacarte_free_message).
#[repr(C)]
pub struct TagliacarteMessageAttachment {
    pub filename: *mut c_char,
    pub mime_type: *mut c_char,
    pub data: *mut u8,
    pub data_len: size_t,
}

/// Full message (envelope + structured body + attachments). Caller frees with tagliacarte_free_message.
#[repr(C)]
pub struct TagliacarteMessage {
    pub subject: *mut c_char,
    pub from_: *mut c_char,
    pub to: *mut c_char,
    pub date: *mut c_char,
    pub body_html: *mut c_char,
    pub body_plain: *mut c_char,
    pub attachment_count: size_t,
    pub attachments: *mut TagliacarteMessageAttachment,
}

/// Version string (static, do not free).
#[no_mangle]
pub extern "C" fn tagliacarte_version() -> *const c_char {
    b"0.1.0\0".as_ptr() as *const c_char
}

/// Last error message from a failed call. Valid until next FFI call. Do not free.
#[no_mangle]
pub extern "C" fn tagliacarte_last_error() -> *const c_char {
    LAST_ERROR.with(|e| {
        e.borrow()
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(ptr::null())
    })
}

/// Free a string returned by tagliacarte_store_maildir_new, tagliacarte_store_imap_new, tagliacarte_store_pop3_new, tagliacarte_store_nostr_new, tagliacarte_store_matrix_new, tagliacarte_store_open_folder, tagliacarte_transport_smtp_new, tagliacarte_transport_nostr_new, tagliacarte_transport_matrix_new. No-op if ptr is NULL.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}

/// Free a string list returned by tagliacarte_store_list_folders. ptr is the array (NULL-terminated).
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_free_string_list(ptr: *mut *mut c_char) {
    if ptr.is_null() {
        return;
    }
    let mut p = ptr;
    while !(*p).is_null() {
        let _ = CString::from_raw(*p);
        p = p.add(1);
    }
    let _ = Vec::from_raw_parts(ptr, (p.offset_from(ptr) as usize) + 1, (p.offset_from(ptr) as usize) + 1);
}

/// Free conversation summary array and all strings inside. count = number of elements.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_free_conversation_summary_list(
    ptr: *mut TagliacarteConversationSummary,
    count: size_t,
) {
    if ptr.is_null() || count == 0 {
        return;
    }
    let slice = std::slice::from_raw_parts_mut(ptr, count);
    for s in slice.iter_mut() {
        if !s.id.is_null() {
            let _ = CString::from_raw(s.id);
            s.id = ptr::null_mut();
        }
        if !s.subject.is_null() {
            let _ = CString::from_raw(s.subject);
            s.subject = ptr::null_mut();
        }
        if !s.from_.is_null() {
            let _ = CString::from_raw(s.from_);
            s.from_ = ptr::null_mut();
        }
    }
    let _ = Vec::from_raw_parts(ptr, count, count);
}

// ---------- Store ----------

/// Create a Maildir store. root_path: path to Maildir root (cur/new/tmp). Returns store URI (caller frees with tagliacarte_free_string), or NULL on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_maildir_new(root_path: *const c_char) -> *mut c_char {
    let path = match ptr_to_str(root_path) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("root_path is null or not valid UTF-8"));
            return ptr::null_mut();
        }
    };
    match MaildirStore::new(&path) {
        Ok(store) => {
            let uri = maildir_store_uri(&path);
            let holder = StoreHolder {
                store: Box::new(store),
                folder_list_callbacks: RwLock::new(None),
            };
            if let Ok(mut guard) = registry().stores.write() {
                guard.insert(uri.clone(), Arc::new(holder));
            }
            clear_last_error();
            CString::new(uri).unwrap().into_raw()
        }
        Err(e) => {
            set_last_error(&e);
            ptr::null_mut()
        }
    }
}

/// Create an IMAP store. user_at_host: identity (e.g. "user" or "user@domain"). host and port (e.g. "imap.example.com", 993). Uses imaps: for port 993, imap: otherwise. Returns store URI (caller frees with tagliacarte_free_string), or NULL on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_imap_new(
    user_at_host: *const c_char,
    host: *const c_char,
    port: u16,
) -> *mut c_char {
    let user = match ptr_to_str(user_at_host) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("user_at_host is null or not valid UTF-8"));
            return ptr::null_mut();
        }
    };
    let host_str = match ptr_to_str(host) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("host is null or not valid UTF-8"));
            return ptr::null_mut();
        }
    };
    let uri = imap_store_uri(&user, &host_str, port);
    let mut store = ImapStore::with_runtime_handle(host_str.clone(), port, registry().runtime.handle().clone());
    store.set_username(&user);
    if let Some(path) = default_credentials_path() {
        let uri_opt = if credentials_use_keychain() { Some(uri.as_str()) } else { None };
        if let Ok(creds) = load_credentials(&path, uri_opt) {
            if let Some(entry) = creds.get(&uri) {
                store.set_credential(
                    if entry.username.is_empty() { None } else { Some(&entry.username) },
                    &entry.password_or_token,
                );
            }
        }
    }
    let holder = StoreHolder {
        store: Box::new(store),
        folder_list_callbacks: RwLock::new(None),
    };
    if let Ok(mut guard) = registry().stores.write() {
        guard.insert(uri.clone(), Arc::new(holder));
    }
    clear_last_error();
    CString::new(uri).unwrap().into_raw()
}

/// Create a POP3 store. user_at_host: identity (e.g. "user" or "user@domain"). host and port (e.g. "pop.example.com", 995). Uses pop3s: for port 995, pop3: otherwise. Authentication is performed separately (Authenticate flow); no password here. Returns store URI (caller frees with tagliacarte_free_string), or NULL on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_pop3_new(
    user_at_host: *const c_char,
    host: *const c_char,
    port: u16,
) -> *mut c_char {
    let user = match ptr_to_str(user_at_host) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("user_at_host is null or not valid UTF-8"));
            return ptr::null_mut();
        }
    };
    let host_str = match ptr_to_str(host) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("host is null or not valid UTF-8"));
            return ptr::null_mut();
        }
    };
    let uri = pop3_store_uri(&user, &host_str, port);
    let mut store = Pop3Store::with_runtime_handle(host_str.clone(), port, registry().runtime.handle().clone());
    store.set_username(&user);
    if let Some(path) = default_credentials_path() {
        let uri_opt = if credentials_use_keychain() { Some(uri.as_str()) } else { None };
        if let Ok(creds) = load_credentials(&path, uri_opt) {
            if let Some(entry) = creds.get(&uri) {
                store.set_credential(
                    if entry.username.is_empty() { None } else { Some(&entry.username) },
                    &entry.password_or_token,
                );
            }
        }
    }
    let holder = StoreHolder {
        store: Box::new(store),
        folder_list_callbacks: RwLock::new(None),
    };
    if let Ok(mut guard) = registry().stores.write() {
        guard.insert(uri.clone(), Arc::new(holder));
    }
    clear_last_error();
    CString::new(uri).unwrap().into_raw()
}

/// Create a Nostr store. relays_comma_separated: e.g. "wss://relay.damus.io,wss://relay.nostr.info". key_path: path to secret key file, or NULL to use env. Returns store URI (caller frees with tagliacarte_free_string), or NULL on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_nostr_new(
    relays_comma_separated: *const c_char,
    key_path: *const c_char,
) -> *mut c_char {
    let relays_str = match ptr_to_str(relays_comma_separated) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("relays_comma_separated is null or not valid UTF-8"));
            return ptr::null_mut();
        }
    };
    let relays: Vec<String> = relays_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let key_path_opt = if key_path.is_null() {
        None
    } else {
        ptr_to_str(key_path).map(|s| s.to_string())
    };
    match NostrStore::new(relays, key_path_opt) {
        Ok(store) => {
            let uri = store.uri().to_string();
            let holder = StoreHolder {
                store: Box::new(store),
                folder_list_callbacks: RwLock::new(None),
            };
            if let Ok(mut guard) = registry().stores.write() {
                guard.insert(uri.clone(), Arc::new(holder));
            }
            clear_last_error();
            CString::new(uri).unwrap().into_raw()
        }
        Err(e) => {
            set_last_error(&e);
            ptr::null_mut()
        }
    }
}

/// Store kind: 0 = Email, 1 = Nostr, 2 = Matrix. Returns -1 if store_uri is NULL or not found.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_kind(store_uri: *const c_char) -> c_int {
    let uri = match ptr_to_str(store_uri) {
        Some(s) => s,
        None => return -1,
    };
    if let Ok(guard) = registry().stores.read() {
        if let Some(holder) = guard.get(&uri) {
            return holder.store.store_kind() as c_int;
        }
    }
    set_last_error(&StoreError::new("store not found"));
    -1
}

/// Set the credential request callback. When the core needs a password/token it calls this (store_uri, auth_type, is_plaintext, username, user_data). Pass NULL to clear.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_set_credential_request_callback(
    callback: Option<CredentialRequestCallback>,
    user_data: *mut c_void,
) {
    let m = CREDENTIAL_REQUEST_CALLBACK.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(mut guard) = m.lock() {
        *guard = callback.map(|cb| (cb, SendableUserData(user_data)));
    }
}

/// Provide credential for the given store (or transport) URI. Call after user submits password in the dialog. Core stores it and persists to config.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_credential_provide(store_uri: *const c_char, password: *const c_char) -> c_int {
    let uri = match ptr_to_str(store_uri) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("store_uri is null or not valid UTF-8"));
            return -1;
        }
    };
    let password_str = match ptr_to_str(password) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("password is null or not valid UTF-8"));
            return -1;
        }
    };
    let holder = match registry().stores.read().ok().and_then(|g| g.get(&uri).cloned()) {
        Some(h) => h,
        None => {
            set_last_error(&StoreError::new("store not found"));
            return -1;
        }
    };
    holder.store.set_credential(None, &password_str);
    if let Some(path) = default_credentials_path() {
        if let Err(e) = save_credential(&path, &uri, "", &password_str) {
            set_last_error(&StoreError::new(e));
            return -1;
        }
    }
    clear_last_error();
    0
}

/// User cancelled the credential dialog. No-op; next connect will request again.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_credential_cancel(_store_uri: *const c_char) {
    // No-op; next connect will request again.
}

/// Set credentials backend: 1 = system keychain, 0 = encrypted file. Call at startup after reading config.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_set_credentials_backend(use_keychain: c_int) {
    set_credentials_backend(use_keychain != 0);
}

/// Returns 1 if the system keychain is available (for showing/enabling the Security tab option), 0 otherwise.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_keychain_available() -> c_int {
    if keychain_available() {
        1
    } else {
        0
    }
}

/// Migrate credentials from the encrypted file to the system keychain. Call after switching to keychain. path: credentials file path (e.g. from default_credentials_path). Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_migrate_credentials_to_keychain(path: *const c_char) -> c_int {
    let path_str = match ptr_to_str(path) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("path is null or not valid UTF-8"));
            return -1;
        }
    };
    let path_buf = std::path::PathBuf::from(path_str);
    match migrate_credentials_to_keychain(&path_buf) {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(e) => {
            set_last_error(&StoreError::new(e));
            -1
        }
    }
}

/// Migrate credentials from the system keychain to the encrypted file for the given URIs. uris: array of uri_count NUL-terminated strings (store/transport URIs from config). Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_migrate_credentials_to_file(
    path: *const c_char,
    uri_count: size_t,
    uris: *const *const c_char,
) -> c_int {
    let path_str = match ptr_to_str(path) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("path is null or not valid UTF-8"));
            return -1;
        }
    };
    let path_buf = std::path::PathBuf::from(path_str);
    let mut uri_list: Vec<String> = Vec::new();
    if !uris.is_null() {
        for i in 0..uri_count {
            let p = uris.add(i as usize).read();
            if p.is_null() {
                continue;
            }
            if let Some(s) = ptr_to_str(p) {
                uri_list.push(s.to_string());
            }
        }
    }
    match migrate_credentials_to_file(&path_buf, &uri_list) {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(e) => {
            set_last_error(&StoreError::new(e));
            -1
        }
    }
}

/// Free a store by URI. Removes from registry. No-op if store_uri is NULL or not found.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_free(store_uri: *const c_char) {
    let uri = match ptr_to_str(store_uri) {
        Some(s) => s,
        None => return,
    };
    let _ = registry().stores.write().map(|mut g| g.remove(&uri));
}

/// List folder names. On success: *out_count = number of names, *out_names = NULL-terminated array (caller frees with tagliacarte_free_string_list). Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_list_folders(
    store_uri: *const c_char,
    out_count: *mut size_t,
    out_names: *mut *mut *mut c_char,
) -> c_int {
    let uri = match ptr_to_str(store_uri) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("store_uri is null or not valid UTF-8"));
            return -1;
        }
    };
    if out_count.is_null() || out_names.is_null() {
        set_last_error(&StoreError::new("null output pointer"));
        return -1;
    }
    let holder = match registry().stores.read().ok().and_then(|g| g.get(&uri).cloned()) {
        Some(h) => h,
        None => {
            set_last_error(&StoreError::new("store not found"));
            return -1;
        }
    };
    match holder.store.list_folders() {
        Ok(folders) => {
            let mut ptrs: Vec<*mut c_char> = folders
                .iter()
                .map(|f| CString::new(f.name.as_str()).unwrap().into_raw())
                .collect();
            ptrs.push(ptr::null_mut());
            let len = ptrs.len();
            let ptr = ptrs.as_mut_ptr();
            std::mem::forget(ptrs);
            *out_count = len - 1;
            *out_names = ptr;
            clear_last_error();
            0
        }
        Err(StoreError::NeedsCredential { username, is_plaintext }) => {
            invoke_credential_request(&uri, TAGLIACARTE_AUTH_TYPE_AUTO, is_plaintext, &username);
            set_last_error(&StoreError::NeedsCredential { username, is_plaintext });
            TAGLIACARTE_NEEDS_CREDENTIAL
        }
        Err(e) => {
            set_last_error(&e);
            -1
        }
    }
}

/// Open a folder by name. Returns folder URI (caller frees with tagliacarte_free_string), or NULL on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_open_folder(
    store_uri: *const c_char,
    name: *const c_char,
) -> *mut c_char {
    let uri = match ptr_to_str(store_uri) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("store_uri is null or not valid UTF-8"));
            return ptr::null_mut();
        }
    };
    let name_str = match ptr_to_str(name) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("name is null or not valid UTF-8"));
            return ptr::null_mut();
        }
    };
    let holder = match registry().stores.read().ok().and_then(|g| g.get(&uri).cloned()) {
        Some(h) => h,
        None => {
            set_last_error(&StoreError::new("store not found"));
            return ptr::null_mut();
        }
    };
    match holder.store.open_folder(&name_str) {
        Ok(folder) => {
            let folder_uri_str = folder_uri(&uri, &name_str);
            let h = FolderHolder {
                folder,
                message_list_callbacks: RwLock::new(None),
                message_callbacks: RwLock::new(None),
            };
            if let Ok(mut guard) = registry().folders.write() {
                guard.insert(folder_uri_str.clone(), Arc::new(h));
            }
            clear_last_error();
            CString::new(folder_uri_str).unwrap().into_raw()
        }
        Err(StoreError::NeedsCredential { username, is_plaintext }) => {
            invoke_credential_request(&uri, TAGLIACARTE_AUTH_TYPE_AUTO, is_plaintext, &username);
            set_last_error(&StoreError::NeedsCredential { username, is_plaintext });
            ptr::null_mut()
        }
        Err(e) => {
            set_last_error(&e);
            ptr::null_mut()
        }
    }
}

/// Set callbacks for folder list. Call tagliacarte_store_refresh_folders to start; callbacks may run on a backend thread (UI must marshal to main thread).
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_set_folder_list_callbacks(
    store_uri: *const c_char,
    on_folder_found: OnFolderFound,
    on_folder_removed: OnFolderRemoved,
    on_complete: OnFolderListComplete,
    user_data: *mut c_void,
) {
    let uri = match ptr_to_str(store_uri) {
        Some(s) => s,
        None => return,
    };
    if let Ok(guard) = registry().stores.read() {
        if let Some(holder) = guard.get(&uri) {
            *holder.folder_list_callbacks.write().unwrap() = Some(FolderListCallbacksSend {
                on_folder_found,
                on_folder_removed,
                on_complete,
                user_data: user_data as usize,
            });
        }
    }
}

/// Start refreshing folder list. Returns immediately; callbacks are invoked from a background thread. Do not free the store until on_complete has been called.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_refresh_folders(store_uri: *const c_char) {
    let uri = match ptr_to_str(store_uri) {
        Some(s) => s,
        None => return,
    };
    let holder = match registry().stores.read() {
        Ok(g) => match g.get(&uri) {
            Some(h) => Arc::clone(h),
            None => return,
        },
        Err(_) => return,
    };
    let callbacks = match holder.folder_list_callbacks.read() {
        Ok(g) => match g.as_ref() {
            Some(c) => c.clone(),
            None => return,
        },
        Err(_) => return,
    };
    let user = std::sync::Arc::new(SendableUserData(callbacks.user_data as *mut c_void));
    let on_folder_found = callbacks.on_folder_found;
    let on_complete_cb = callbacks.on_complete;
    struct FolderCbState(OnFolderFound, std::sync::Arc<SendableUserData>);
    unsafe impl Send for FolderCbState {}
    unsafe impl Sync for FolderCbState {}
    let folder_cb = std::sync::Arc::new(FolderCbState(on_folder_found, user.clone()));
    let folder_cb_for_complete = folder_cb.clone();
    let on_folder: Box<dyn Fn(FolderInfo) + Send + Sync> = Box::new(move |f: FolderInfo| {
        let name = CString::new(f.name.as_str()).unwrap();
        let delim_char = f.delimiter.map(|c| c as c_char).unwrap_or(0);
        let attrs_str = f.attributes.join(" ");
        let attrs_c = CString::new(attrs_str).unwrap();
        (folder_cb.0)(name.as_ptr(), delim_char, attrs_c.as_ptr(), folder_cb.1.0);
    });
    let user_complete = folder_cb_for_complete.1.clone();
    let on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send> = Box::new(move |result| {
        let code = match &result {
            Ok(()) => 0,
            Err(_) => -1,
        };
        (on_complete_cb)(code, user_complete.0);
    });
    // Call store method directly — for IMAP, this sends LIST through the channel and returns immediately.
    // For other stores, the default implementation calls list_folders() synchronously, which is OK
    // since we're on the UI thread but these stores are fast (Maildir, Nostr, Matrix).
    match holder.store.refresh_folders_streaming(on_folder, on_complete) {
        Ok(()) => {}
        Err(StoreError::NeedsCredential { username, is_plaintext }) => {
            invoke_credential_request(&uri, TAGLIACARTE_AUTH_TYPE_AUTO, is_plaintext, &username);
            set_last_error(&StoreError::NeedsCredential { username, is_plaintext });
            (callbacks.on_complete)(TAGLIACARTE_NEEDS_CREDENTIAL, folder_cb_for_complete.1.0);
        }
        Err(e) => {
            set_last_error(&e);
            (callbacks.on_complete)(-1, folder_cb_for_complete.1.0);
        }
    }
}

/// Start opening a folder by name. Returns immediately; on_select_event (if non-NULL), on_folder_ready, or on_error are invoked from a background thread.
/// On success, on_folder_ready receives folder_uri (caller must free with tagliacarte_free_string). Do not free the store until the operation completes.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_start_open_folder(
    store_uri: *const c_char,
    mailbox_name: *const c_char,
    on_select_event: Option<OnOpenFolderSelectEvent>,
    on_folder_ready: OnFolderReady,
    on_error: OnOpenFolderError,
    user_data: *mut c_void,
) {
    let uri = match ptr_to_str(store_uri) {
        Some(s) => s,
        None => return,
    };
    let name_str = match ptr_to_str(mailbox_name) {
        Some(s) => s,
        None => return,
    };
    let holder = match registry().stores.read() {
        Ok(g) => match g.get(&uri) {
            Some(h) => Arc::clone(h),
            None => return,
        },
        Err(_) => return,
    };
    let uri = uri.clone();
    let name_str = name_str.clone();
    let on_select_event = on_select_event;
    let on_folder_ready = on_folder_ready;
    let on_error = on_error;
    let user = std::sync::Arc::new(SendableUserData(user_data));
    {
        let on_event: Box<dyn Fn(OpenFolderEvent) + Send + Sync> = match on_select_event {
            Some(cb) => {
                struct CbState(OnOpenFolderSelectEvent, std::sync::Arc<SendableUserData>);
                unsafe impl Send for CbState {}
                unsafe impl Sync for CbState {}
                let state = std::sync::Arc::new(CbState(cb, user.clone()));
                Box::new(move |ev: OpenFolderEvent| {
                    let (event_type, number_value, string_value) = match &ev {
                        OpenFolderEvent::Exists(n) => (0, *n, None),
                        OpenFolderEvent::Recent(n) => (1, *n, None),
                        OpenFolderEvent::Flags(f) => {
                            let s = f.join(",");
                            (2, 0, Some(s))
                        }
                        OpenFolderEvent::UidValidity(n) => (3, *n, None),
                        OpenFolderEvent::UidNext(n) => (4, *n, None),
                        OpenFolderEvent::Other(s) => (5, 0, Some(s.clone())),
                    };
                    let cstr_holder = string_value.and_then(|s| CString::new(s).ok());
                    let ptr = cstr_holder
                        .as_ref()
                        .map(|c| c.as_ptr())
                        .unwrap_or(ptr::null());
                    (state.0)(event_type, number_value, ptr, state.1.0);
                })
            }
            None => Box::new(|_| {}),
        };
        let name_str_for_call = name_str.clone();
        let user_complete = user.clone();
        let uri_for_cb = uri.clone();
        let uri_for_sync = uri.clone();
        let on_complete: Box<dyn FnOnce(Result<Box<dyn Folder>, StoreError>) + Send> =
            Box::new(move |result| {
                match result {
                    Ok(folder) => {
                        let folder_uri_str = folder_uri(&uri, &name_str);
                        let h = FolderHolder {
                            folder,
                            message_list_callbacks: RwLock::new(None),
                            message_callbacks: RwLock::new(None),
                        };
                        if let Ok(mut guard) = registry().folders.write() {
                            guard.insert(folder_uri_str.clone(), Arc::new(h));
                        }
                        let cstr = CString::new(folder_uri_str).unwrap().into_raw();
                        (on_folder_ready)(cstr, user_complete.0);
                    }
                    Err(StoreError::NeedsCredential { username, is_plaintext }) => {
                        invoke_credential_request(&uri_for_cb, TAGLIACARTE_AUTH_TYPE_AUTO, is_plaintext, &username);
                        let msg = CString::new("credential required").unwrap();
                        (on_error)(msg.as_ptr(), user_complete.0);
                    }
                    Err(e) => {
                        let msg = CString::new(e.to_string()).unwrap_or_else(|_| CString::new("").unwrap());
                        (on_error)(msg.as_ptr(), user_complete.0);
                    }
                }
            });
        match holder.store.start_open_folder_streaming(&name_str_for_call, on_event, on_complete) {
            Ok(()) => {}
            Err(StoreError::NeedsCredential { username, is_plaintext }) => {
                invoke_credential_request(&uri_for_sync, TAGLIACARTE_AUTH_TYPE_AUTO, is_plaintext, &username);
                let msg = CString::new("credential required").unwrap();
                (on_error)(msg.as_ptr(), user.0);
            }
            Err(e) => {
                let msg = CString::new(e.to_string()).unwrap_or_else(|_| CString::new("").unwrap());
                (on_error)(msg.as_ptr(), user.0);
            }
        }
    }
}

// ---------- Folder ----------

/// Free a folder by URI. Removes from registry. No-op if folder_uri is NULL or not found.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_folder_free(folder_uri: *const c_char) {
    let uri = match ptr_to_str(folder_uri) {
        Some(s) => s,
        None => return,
    };
    let _ = registry().folders.write().map(|mut g| g.remove(&uri));
}

/// Set callbacks for message list. Call tagliacarte_folder_request_message_list to start; callbacks may run on a backend thread.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_folder_set_message_list_callbacks(
    folder_uri: *const c_char,
    on_message_summary: OnMessageSummary,
    on_complete: OnMessageListComplete,
    user_data: *mut c_void,
) {
    let uri = match ptr_to_str(folder_uri) {
        Some(s) => s,
        None => return,
    };
    if let Ok(guard) = registry().folders.read() {
        if let Some(holder) = guard.get(&uri) {
            *holder.message_list_callbacks.write().unwrap() = Some(MessageListCallbacksSend {
                on_message_summary,
                on_complete,
                user_data: user_data as usize,
            });
        }
    }
}

/// Start loading message list for range [start, end). Returns immediately; callbacks invoked from a background thread. Do not free the folder until on_complete has been called.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_folder_request_message_list(
    folder_uri: *const c_char,
    start: u64,
    end: u64,
) {
    let uri = match ptr_to_str(folder_uri) {
        Some(s) => s,
        None => return,
    };
    let holder = match registry().folders.read() {
        Ok(g) => match g.get(&uri) {
            Some(h) => Arc::clone(h),
            None => return,
        },
        Err(_) => return,
    };
    let callbacks = match holder.message_list_callbacks.read() {
        Ok(g) => match g.as_ref() {
            Some(c) => c.clone(),
            None => return,
        },
        Err(_) => return,
    };
    {
        let range = start..end;
        let user = std::sync::Arc::new(SendableUserData(callbacks.user_data as *mut c_void));
        struct MessageListCbState(OnMessageSummary, OnMessageListComplete, std::sync::Arc<SendableUserData>);
        unsafe impl Send for MessageListCbState {}
        unsafe impl Sync for MessageListCbState {}
        let cb_state = std::sync::Arc::new(MessageListCbState(
            callbacks.on_message_summary,
            callbacks.on_complete,
            user.clone(),
        ));
        let cb_state_for_summary = cb_state.clone();
        let cb_state_for_error = cb_state.clone();
        let on_summary: Box<dyn Fn(ConversationSummary) + Send + Sync> =
            Box::new(move |s: ConversationSummary| {
                let id = CString::new(s.id.as_str()).unwrap();
                let subject = s
                    .envelope
                    .subject
                    .as_ref()
                    .map(|x| CString::new(x.as_str()).unwrap())
                    .unwrap_or_else(|| CString::new("").unwrap());
                let from = CString::new(format_address_display(&s.envelope.from)).unwrap();
                let date_secs = s.envelope.date.as_ref().map(|d| d.timestamp).unwrap_or(-1);
                (cb_state_for_summary.0)(
                    id.as_ptr(),
                    subject.as_ptr(),
                    from.as_ptr(),
                    date_secs,
                    s.size,
                    cb_state_for_summary.2.0,
                );
            });
        let on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send> = Box::new(move |result| {
            let code = if result.is_ok() { 0 } else { -1 };
            (cb_state.1)(code, cb_state.2.0);
        });
        if let Err(_) = holder.folder.request_message_list_streaming(range, on_summary, on_complete) {
            (cb_state_for_error.1)(-1, cb_state_for_error.2.0);
        }
    }
}

/// Set callbacks for get message. Call tagliacarte_folder_request_message to start; callbacks may run on a background thread.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_folder_set_message_callbacks(
    folder_uri: *const c_char,
    on_metadata: OnMessageMetadata,
    on_start_entity: OnStartEntity,
    on_content_type: OnContentType,
    on_content_disposition: OnContentDisposition,
    on_content_id: OnContentId,
    on_end_headers: OnEndHeaders,
    on_body_content: OnBodyContent,
    on_end_entity: OnEndEntity,
    on_complete: OnMessageComplete,
    user_data: *mut c_void,
) {
    let uri = match ptr_to_str(folder_uri) {
        Some(s) => s,
        None => return,
    };
    if let Ok(guard) = registry().folders.read() {
        if let Some(holder) = guard.get(&uri) {
            *holder.message_callbacks.write().unwrap() = Some(MessageCallbacksSend {
                on_metadata,
                on_start_entity,
                on_content_type,
                on_content_disposition,
                on_content_id,
                on_end_headers,
                on_body_content,
                on_end_entity,
                on_complete,
                user_data: user_data as usize,
            });
        }
    }
}

/// MimeHandler that forwards events to C callbacks (used for streaming MIME parsing).
struct FfiMimeHandler {
    on_start_entity: OnStartEntity,
    on_content_type: OnContentType,
    on_content_disposition: OnContentDisposition,
    on_content_id: OnContentId,
    on_end_headers: OnEndHeaders,
    on_body_content: OnBodyContent,
    on_end_entity: OnEndEntity,
    user_data: *mut c_void,
}

unsafe impl Send for FfiMimeHandler {}

impl tagliacarte_core::mime::MimeHandler for FfiMimeHandler {
    fn start_entity(&mut self, _boundary: Option<&str>) -> Result<(), tagliacarte_core::mime::MimeParseError> {
        (self.on_start_entity)(self.user_data);
        Ok(())
    }

    fn content_type(&mut self, value: &str) -> Result<(), tagliacarte_core::mime::MimeParseError> {
        if let Ok(c) = CString::new(value) {
            (self.on_content_type)(c.as_ptr(), self.user_data);
        }
        Ok(())
    }

    fn content_disposition(&mut self, value: &str) -> Result<(), tagliacarte_core::mime::MimeParseError> {
        if let Ok(c) = CString::new(value) {
            (self.on_content_disposition)(c.as_ptr(), self.user_data);
        }
        Ok(())
    }

    fn content_id(&mut self, id: &str) -> Result<(), tagliacarte_core::mime::MimeParseError> {
        // Strip angle brackets: "<foo@bar>" → "foo@bar"
        let bare = id.trim();
        let bare = if bare.starts_with('<') && bare.ends_with('>') {
            &bare[1..bare.len() - 1]
        } else {
            bare
        };
        if let Ok(c) = CString::new(bare) {
            (self.on_content_id)(c.as_ptr(), self.user_data);
        }
        Ok(())
    }

    fn end_headers(&mut self) -> Result<(), tagliacarte_core::mime::MimeParseError> {
        (self.on_end_headers)(self.user_data);
        Ok(())
    }

    fn body_content(&mut self, data: &[u8]) -> Result<(), tagliacarte_core::mime::MimeParseError> {
        if !data.is_empty() {
            (self.on_body_content)(data.as_ptr(), data.len(), self.user_data);
        }
        Ok(())
    }

    fn end_entity(&mut self, _boundary: Option<&str>) -> Result<(), tagliacarte_core::mime::MimeParseError> {
        (self.on_end_entity)(self.user_data);
        Ok(())
    }
}

/// Start loading a message by id. Returns immediately; on_metadata for envelope, then
/// MIME entity events (start_entity, content_type, content_disposition, end_headers,
/// body_content chunks, end_entity) for each part, then on_complete.
/// Raw RFC822 bytes from the store are fed directly to MimeParser as they arrive —
/// no accumulation, no heuristics. Events fire synchronously to the UI via FfiMimeHandler.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_folder_request_message(
    folder_uri: *const c_char,
    message_id: *const c_char,
) {
    let uri = match ptr_to_str(folder_uri) {
        Some(s) => s,
        None => return,
    };
    let id_str = match ptr_to_str(message_id) {
        Some(s) => s,
        None => return,
    };
    let holder = match registry().folders.read() {
        Ok(g) => match g.get(&uri) {
            Some(h) => Arc::clone(h),
            None => return,
        },
        Err(_) => return,
    };
    let cbs = match holder.message_callbacks.read() {
        Ok(g) => match g.as_ref() {
            Some(c) => c.clone(),
            None => return,
        },
        Err(_) => return,
    };
    {
        let user_data = cbs.user_data as *mut c_void;
        let id = MessageId::new(&id_str);
        // on_metadata: emit envelope to UI
        let cbs_meta = cbs.clone();
        let on_metadata: Box<dyn Fn(Envelope) + Send + Sync> = Box::new(move |env: Envelope| {
            let subject = env.subject.as_ref()
                .map(|s: &String| CString::new(s.as_str()).unwrap())
                .unwrap_or_else(|| CString::new("").unwrap());
            let from_ = CString::new(format_address_list(&env.from)).unwrap();
            let to = CString::new(format_address_list(&env.to)).unwrap();
            let date = env.date.as_ref()
                .map(|d| d.timestamp.to_string())
                .unwrap_or_default();
            let date_c = CString::new(date).unwrap();
            let ud = cbs_meta.user_data as *mut c_void;
            (cbs_meta.on_metadata)(subject.as_ptr(), from_.as_ptr(), to.as_ptr(), date_c.as_ptr(), ud);
        });
        // Create MimeParser with FfiMimeHandler up front — no buffering.
        // Raw RFC822 bytes are fed to the parser as they arrive from the store.
        let handler = FfiMimeHandler {
            on_start_entity: cbs.on_start_entity,
            on_content_type: cbs.on_content_type,
            on_content_disposition: cbs.on_content_disposition,
            on_content_id: cbs.on_content_id,
            on_end_headers: cbs.on_end_headers,
            on_body_content: cbs.on_body_content,
            on_end_entity: cbs.on_end_entity,
            user_data,
        };
        let parser = Arc::new(std::sync::Mutex::new(Some(MimeParser::new(handler))));
        // on_content_chunk: feed directly to MimeParser; events fire synchronously to UI
        let parser_chunk = parser.clone();
        let on_content_chunk: Box<dyn Fn(&[u8]) + Send + Sync> = Box::new(move |chunk: &[u8]| {
            if let Ok(mut guard) = parser_chunk.lock() {
                if let Some(ref mut p) = *guard {
                    let _ = p.receive(chunk);
                }
            }
        });
        // on_complete: close parser to flush remaining buffered line, then signal UI
        let cbs_complete = cbs.clone();
        let parser_complete = parser.clone();
        let on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send> = Box::new(move |result| {
            let ud = cbs_complete.user_data as *mut c_void;
            if result.is_err() {
                (cbs_complete.on_complete)(-1, ud);
                return;
            }
            if let Ok(mut guard) = parser_complete.lock() {
                if let Some(mut p) = guard.take() {
                    let _ = p.close();
                }
            }
            (cbs_complete.on_complete)(0, ud);
        });
        if let Err(_) = holder.folder.request_message_streaming(&id, on_metadata, on_content_chunk, on_complete) {
            (cbs.on_complete)(-1, user_data);
        }
    }
}

/// Message count in folder. Returns 0 on error (check tagliacarte_last_error).
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_folder_message_count(folder_uri: *const c_char) -> u64 {
    let uri = match ptr_to_str(folder_uri) {
        Some(s) => s,
        None => return 0,
    };
    let holder = match registry().folders.read().ok().and_then(|g| g.get(&uri).cloned()) {
        Some(h) => h,
        None => return 0,
    };
    match holder.folder.message_count() {
        Ok(n) => {
            clear_last_error();
            n
        }
        Err(e) => {
            set_last_error(&e);
            0
        }
    }
}

/// Append raw message bytes to folder (e.g. from .eml file). Supported for Maildir. Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_folder_append_message(
    folder_uri: *const c_char,
    data: *const u8,
    data_len: size_t,
) -> c_int {
    let uri = match ptr_to_str(folder_uri) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("folder_uri is null or not valid UTF-8"));
            return -1;
        }
    };
    if data.is_null() && data_len != 0 {
        set_last_error(&StoreError::new("data is null but data_len non-zero"));
        return -1;
    }
    let holder = match registry().folders.read().ok().and_then(|g| g.get(&uri).cloned()) {
        Some(h) => h,
        None => {
            set_last_error(&StoreError::new("folder not found"));
            return -1;
        }
    };
    let slice = if data_len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }
    };
    match holder.folder.append_message(slice) {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(e) => {
            set_last_error(&e);
            -1
        }
    }
}

/// Delete a message by id. Supported for Maildir. Returns 0 on success, -1 on error.
/// Temporarily disabled (set to true when ready to test).
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_folder_delete_message(
    folder_uri: *const c_char,
    message_id: *const c_char,
) -> c_int {
    const DELETE_ENABLED: bool = false;
    if !DELETE_ENABLED {
        set_last_error(&StoreError::new("delete temporarily disabled"));
        return -1;
    }

    let uri = match ptr_to_str(folder_uri) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("folder_uri is null or not valid UTF-8"));
            return -1;
        }
    };
    let id_str = match ptr_to_str(message_id) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("message_id is null or not valid UTF-8"));
            return -1;
        }
    };
    let holder = match registry().folders.read().ok().and_then(|g| g.get(&uri).cloned()) {
        Some(h) => h,
        None => {
            set_last_error(&StoreError::new("folder not found"));
            return -1;
        }
    };
    let id = MessageId::new(&id_str);
    match holder.folder.delete_message(&id) {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(e) => {
            set_last_error(&e);
            -1
        }
    }
}

/// Get a full message by id. On success: *out_message set (caller frees with tagliacarte_free_message). Returns 0 on success, -1 on error or not found.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_folder_get_message(
    folder_uri: *const c_char,
    message_id: *const c_char,
    out_message: *mut *mut TagliacarteMessage,
) -> c_int {
    let uri = match ptr_to_str(folder_uri) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("folder_uri is null or not valid UTF-8"));
            return -1;
        }
    };
    if message_id.is_null() || out_message.is_null() {
        set_last_error(&StoreError::new("null argument"));
        return -1;
    }
    let holder = match registry().folders.read().ok().and_then(|g| g.get(&uri).cloned()) {
        Some(h) => h,
        None => {
            set_last_error(&StoreError::new("folder not found"));
            return -1;
        }
    };
    let id_str = match unsafe { CStr::from_ptr(message_id).to_str() } {
        Ok(s) => s,
        Err(_) => {
            set_last_error(&StoreError::new("message_id is not valid UTF-8"));
            return -1;
        }
    };
    let id = MessageId::new(id_str);
    match holder.folder.get_message(&id) {
        Ok(Some(msg)) => {
            let subject = msg
                .envelope
                .subject
                .as_ref()
                .map(|s| CString::new(s.as_str()).unwrap().into_raw())
                .unwrap_or(ptr::null_mut());
            let from_ = CString::new(format_address_list(&msg.envelope.from)).unwrap().into_raw();
            let to = CString::new(format_address_list(&msg.envelope.to)).unwrap().into_raw();
            let date = msg
                .envelope
                .date
                .map(|d| CString::new(d.timestamp.to_string()).unwrap().into_raw())
                .unwrap_or(ptr::null_mut());
            let body_html = msg
                .body_html
                .as_ref()
                .map(|s| CString::new(s.as_str()).unwrap().into_raw())
                .unwrap_or(ptr::null_mut());
            let body_plain = msg
                .body_plain
                .as_ref()
                .map(|s| CString::new(s.as_str()).unwrap().into_raw())
                .unwrap_or(ptr::null_mut());
            let (attachment_count, attachments) = if msg.attachments.is_empty() {
                (0usize, ptr::null_mut())
            } else {
                let mut arr: Vec<TagliacarteMessageAttachment> = msg
                    .attachments
                    .iter()
                    .map(|a| {
                        let filename = a
                            .filename
                            .as_ref()
                            .map(|s| CString::new(s.as_str()).unwrap().into_raw())
                            .unwrap_or(ptr::null_mut());
                        let mime_type = CString::new(a.mime_type.as_str()).unwrap().into_raw();
                        let data_len = a.content.len();
                        let data = if data_len == 0 {
                            ptr::null_mut()
                        } else {
                            let mut buf = a.content.clone();
                            let ptr = buf.as_mut_ptr();
                            std::mem::forget(buf);
                            ptr
                        };
                        TagliacarteMessageAttachment {
                            filename,
                            mime_type,
                            data,
                            data_len,
                        }
                    })
                    .collect();
                let count = arr.len();
                let ptr = arr.as_mut_ptr();
                std::mem::forget(arr);
                (count, ptr)
            };
            let out = Box::new(TagliacarteMessage {
                subject,
                from_,
                to,
                date,
                body_html,
                body_plain,
                attachment_count,
                attachments,
            });
            *out_message = Box::into_raw(out);
            clear_last_error();
            0
        }
        Ok(None) => {
            set_last_error(&StoreError::new("message not found"));
            -1
        }
        Err(e) => {
            set_last_error(&e);
            -1
        }
    }
}

/// Free a message returned by tagliacarte_folder_get_message. No-op if msg is NULL.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_free_message(msg: *mut TagliacarteMessage) {
    if msg.is_null() {
        return;
    }
    let m = &*msg;
    if !m.subject.is_null() {
        let _ = CString::from_raw(m.subject);
    }
    if !m.from_.is_null() {
        let _ = CString::from_raw(m.from_);
    }
    if !m.to.is_null() {
        let _ = CString::from_raw(m.to);
    }
    if !m.date.is_null() {
        let _ = CString::from_raw(m.date);
    }
    if !m.body_html.is_null() {
        let _ = CString::from_raw(m.body_html);
    }
    if !m.body_plain.is_null() {
        let _ = CString::from_raw(m.body_plain);
    }
    if !m.attachments.is_null() && m.attachment_count > 0 {
        let slice = std::slice::from_raw_parts(m.attachments, m.attachment_count);
        for att in slice {
            if !att.filename.is_null() {
                let _ = CString::from_raw(att.filename);
            }
            if !att.mime_type.is_null() {
                let _ = CString::from_raw(att.mime_type);
            }
            if !att.data.is_null() && att.data_len > 0 {
                let _ = Vec::from_raw_parts(att.data, att.data_len, att.data_len);
            }
        }
        let _ = Vec::from_raw_parts(m.attachments, m.attachment_count, m.attachment_count);
    }
    let _ = Box::from_raw(msg);
}

/// List conversation summaries in range [start, end). On success: *out_count set, *out_summaries = array (caller frees with tagliacarte_free_conversation_summary_list). Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_folder_list_conversations(
    folder_uri: *const c_char,
    start: u64,
    end: u64,
    out_count: *mut size_t,
    out_summaries: *mut *mut TagliacarteConversationSummary,
) -> c_int {
    let uri = match ptr_to_str(folder_uri) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("folder_uri is null or not valid UTF-8"));
            return -1;
        }
    };
    if out_count.is_null() || out_summaries.is_null() {
        set_last_error(&StoreError::new("null output pointer"));
        return -1;
    }
    let holder = match registry().folders.read().ok().and_then(|g| g.get(&uri).cloned()) {
        Some(h) => h,
        None => {
            set_last_error(&StoreError::new("folder not found"));
            return -1;
        }
    };
    let range = Range {
        start,
        end: end.max(start),
    };
    match holder.folder.list_conversations(range) {
        Ok(summaries) => {
            let mut out: Vec<TagliacarteConversationSummary> = summaries
                .into_iter()
                .map(|s| {
                    let id = CString::new(s.id.as_str()).unwrap().into_raw();
                    let subject = s
                        .envelope
                        .subject
                        .as_ref()
                        .map(|t| CString::new(t.as_str()).unwrap().into_raw())
                        .unwrap_or(ptr::null_mut());
                    let from_str = format_address_display(&s.envelope.from);
                    let from_ = CString::new(from_str).unwrap().into_raw();
                    TagliacarteConversationSummary {
                        id,
                        subject,
                        from_,
                        size: s.size,
                    }
                })
                .collect();
            let count = out.len();
            let ptr = out.as_mut_ptr();
            std::mem::forget(out);
            *out_count = count;
            *out_summaries = ptr;
            clear_last_error();
            0
        }
        Err(e) => {
            set_last_error(&e);
            -1
        }
    }
}

// ---------- Transport ----------

/// Transport kind: 0 = Email, 1 = Nostr, 2 = Matrix. Returns -1 if transport_uri is NULL or not found.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_transport_kind(transport_uri: *const c_char) -> c_int {
    let uri = match ptr_to_str(transport_uri) {
        Some(s) => s,
        None => return -1,
    };
    if let Ok(guard) = registry().transports.read() {
        if let Some(holder) = guard.get(&uri) {
            return holder.0.transport_kind() as c_int;
        }
    }
    set_last_error(&StoreError::new("transport not found"));
    -1
}

/// Create SMTP transport. host and port. Uses smtps: for port 465, smtp: otherwise. Returns transport URI (caller frees with tagliacarte_free_string), or NULL on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_transport_smtp_new(host: *const c_char, port: u16) -> *mut c_char {
    let host_str = match ptr_to_str(host) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("host is null or not valid UTF-8"));
            return ptr::null_mut();
        }
    };
    let uri = smtp_transport_uri(&host_str, port);
    let transport: Arc<SmtpTransport> = Arc::new(SmtpTransport::with_runtime_handle(host_str.clone(), port, registry().runtime.handle().clone()));
    transport.clone().start_send_worker();
    let holder = TransportHolder(transport as Arc<dyn Transport>);
    if let Ok(mut guard) = registry().transports.write() {
        guard.insert(uri.clone(), Arc::new(holder));
    }
    clear_last_error();
    CString::new(uri).unwrap().into_raw()
}

/// Create a Nostr transport. Same parameters as tagliacarte_store_nostr_new. Returns transport URI (caller frees with tagliacarte_free_string), or NULL on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_transport_nostr_new(
    relays_comma_separated: *const c_char,
    key_path: *const c_char,
) -> *mut c_char {
    let relays_str = match ptr_to_str(relays_comma_separated) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("relays_comma_separated is null or not valid UTF-8"));
            return ptr::null_mut();
        }
    };
    let relays: Vec<String> = relays_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let key_path_opt = if key_path.is_null() {
        None
    } else {
        ptr_to_str(key_path).map(|s| s.to_string())
    };
    match NostrTransport::new(relays, key_path_opt) {
        Ok(transport) => {
            let uri = transport.uri().to_string();
            let holder = TransportHolder(Arc::new(transport) as Arc<dyn Transport>);
            if let Ok(mut guard) = registry().transports.write() {
                guard.insert(uri.clone(), Arc::new(holder));
            }
            clear_last_error();
            CString::new(uri).unwrap().into_raw()
        }
        Err(e) => {
            set_last_error(&e);
            ptr::null_mut()
        }
    }
}

/// Create a Matrix store. homeserver (e.g. https://matrix.example.org), user_id (e.g. @user:example.org), access_token (NULL = must log in). Returns store URI (caller frees with tagliacarte_free_string), or NULL on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_matrix_new(
    homeserver: *const c_char,
    user_id: *const c_char,
    access_token: *const c_char,
) -> *mut c_char {
    let home = match ptr_to_str(homeserver) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("homeserver is null or not valid UTF-8"));
            return ptr::null_mut();
        }
    };
    let user = match ptr_to_str(user_id) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("user_id is null or not valid UTF-8"));
            return ptr::null_mut();
        }
    };
    let token_opt = if access_token.is_null() {
        None
    } else {
        ptr_to_str(access_token).map(|s| s.to_string())
    };
    match MatrixStore::new(home.clone(), user.clone(), token_opt) {
        Ok(store) => {
            let uri = store.uri().to_string();
            let holder = StoreHolder {
                store: Box::new(store),
                folder_list_callbacks: RwLock::new(None),
            };
            if let Ok(mut guard) = registry().stores.write() {
                guard.insert(uri.clone(), Arc::new(holder));
            }
            clear_last_error();
            CString::new(uri).unwrap().into_raw()
        }
        Err(e) => {
            set_last_error(&e);
            ptr::null_mut()
        }
    }
}

/// Create a Matrix transport. Same parameters as tagliacarte_store_matrix_new. Returns transport URI (caller frees with tagliacarte_free_string), or NULL on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_transport_matrix_new(
    homeserver: *const c_char,
    user_id: *const c_char,
    access_token: *const c_char,
) -> *mut c_char {
    let home = match ptr_to_str(homeserver) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("homeserver is null or not valid UTF-8"));
            return ptr::null_mut();
        }
    };
    let user = match ptr_to_str(user_id) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("user_id is null or not valid UTF-8"));
            return ptr::null_mut();
        }
    };
    let token_opt = if access_token.is_null() {
        None
    } else {
        ptr_to_str(access_token).map(|s| s.to_string())
    };
    match MatrixTransport::new(home.clone(), user.clone(), token_opt) {
        Ok(transport) => {
            let uri = transport.uri().to_string();
            let holder = TransportHolder(Arc::new(transport) as Arc<dyn Transport>);
            if let Ok(mut guard) = registry().transports.write() {
                guard.insert(uri.clone(), Arc::new(holder));
            }
            clear_last_error();
            CString::new(uri).unwrap().into_raw()
        }
        Err(e) => {
            set_last_error(&e);
            ptr::null_mut()
        }
    }
}

/// Structured send: from, to, cc, subject, body_plain, body_html, optional attachments. Backend builds wire format. Returns 0 on success, -1 on error.
#[repr(C)]
pub struct TagliacarteAttachment {
    pub filename: *const c_char,
    pub mime_type: *const c_char,
    pub data: *const u8,
    pub data_len: size_t,
}

#[no_mangle]
pub unsafe extern "C" fn tagliacarte_transport_send(
    transport_uri: *const c_char,
    from: *const c_char,
    to: *const c_char,
    cc: *const c_char,
    subject: *const c_char,
    body_plain: *const c_char,
    body_html: *const c_char,
    attachment_count: size_t,
    attachments: *const TagliacarteAttachment,
) -> c_int {
    let uri = match ptr_to_str(transport_uri) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("transport_uri is null or not valid UTF-8"));
            return -1;
        }
    };
    if from.is_null() || to.is_null() {
        set_last_error(&StoreError::new("null argument"));
        return -1;
    }
    let from_str = CStr::from_ptr(from).to_string_lossy().into_owned();
    let to_str = CStr::from_ptr(to).to_string_lossy().into_owned();
    let from_addr = parse_address(&from_str);
    let to_addrs: Vec<Address> = to_str.split(',').map(|s| parse_address(s.trim())).collect();
    let cc_addrs: Vec<Address> = if cc.is_null() {
        Vec::new()
    } else {
        CStr::from_ptr(cc)
            .to_string_lossy()
            .split(',')
            .map(|s| parse_address(s.trim()))
            .collect()
    };
    let subject_opt = if subject.is_null() {
        None
    } else {
        Some(CStr::from_ptr(subject).to_string_lossy().into_owned())
    };
    let body_plain_opt = if body_plain.is_null() {
        None
    } else {
        Some(CStr::from_ptr(body_plain).to_string_lossy().into_owned())
    };
    let body_html_opt = if body_html.is_null() {
        None
    } else {
        Some(CStr::from_ptr(body_html).to_string_lossy().into_owned())
    };
    let att_list: Vec<Attachment> = if attachment_count == 0 || attachments.is_null() {
        Vec::new()
    } else {
        let slice = std::slice::from_raw_parts(attachments, attachment_count);
        slice
            .iter()
            .map(|a| {
                let filename = if a.filename.is_null() {
                    None
                } else {
                    Some(CStr::from_ptr(a.filename).to_string_lossy().into_owned())
                };
                let mime_type = if a.mime_type.is_null() {
                    "application/octet-stream".to_string()
                } else {
                    CStr::from_ptr(a.mime_type).to_string_lossy().into_owned()
                };
                let content = if a.data.is_null() {
                    Vec::new()
                } else {
                    std::slice::from_raw_parts(a.data, a.data_len).to_vec()
                };
                Attachment {
                    filename,
                    mime_type,
                    content,
                }
            })
            .collect()
    };
    let payload = SendPayload {
        from: vec![from_addr],
        to: to_addrs,
        cc: cc_addrs,
        subject: subject_opt,
        body_plain: body_plain_opt,
        body_html: body_html_opt,
        attachments: att_list,
    };
    let holder = match registry().transports.read().ok().and_then(|g| g.get(&uri).cloned()) {
        Some(h) => h,
        None => {
            set_last_error(&StoreError::new("transport not found"));
            return -1;
        }
    };
    match holder.0.send(&payload) {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(e) => {
            set_last_error(&e);
            -1
        }
    }
}

/// Async send: returns immediately; on_progress (optional) and on_complete called from a background thread. Caller must not free the transport until on_complete has been called.
type OnSendProgress = extern "C" fn(*const c_char, *mut c_void);
type OnSendComplete = extern "C" fn(c_int, *mut c_void);

#[no_mangle]
pub unsafe extern "C" fn tagliacarte_transport_send_async(
    transport_uri: *const c_char,
    from: *const c_char,
    to: *const c_char,
    cc: *const c_char,
    subject: *const c_char,
    body_plain: *const c_char,
    body_html: *const c_char,
    attachment_count: size_t,
    attachments: *const TagliacarteAttachment,
    on_progress: Option<OnSendProgress>,
    on_complete: OnSendComplete,
    user_data: *mut c_void,
) {
    let uri = match ptr_to_str(transport_uri) {
        Some(s) => s,
        None => return,
    };
    if from.is_null() || to.is_null() {
        return;
    }
    let from_str = CStr::from_ptr(from).to_string_lossy().into_owned();
    let to_str = CStr::from_ptr(to).to_string_lossy().into_owned();
    let cc_str = if cc.is_null() {
        String::new()
    } else {
        CStr::from_ptr(cc).to_string_lossy().into_owned()
    };
    let subject_opt = if subject.is_null() {
        None
    } else {
        Some(CStr::from_ptr(subject).to_string_lossy().into_owned())
    };
    let body_plain_opt = if body_plain.is_null() {
        None
    } else {
        Some(CStr::from_ptr(body_plain).to_string_lossy().into_owned())
    };
    let body_html_opt = if body_html.is_null() {
        None
    } else {
        Some(CStr::from_ptr(body_html).to_string_lossy().into_owned())
    };
    let att_list: Vec<Attachment> = if attachment_count == 0 || attachments.is_null() {
        Vec::new()
    } else {
        let slice = std::slice::from_raw_parts(attachments, attachment_count);
        slice
            .iter()
            .map(|a| {
                let filename = if a.filename.is_null() {
                    None
                } else {
                    Some(CStr::from_ptr(a.filename).to_string_lossy().into_owned())
                };
                let mime_type = if a.mime_type.is_null() {
                    "application/octet-stream".to_string()
                } else {
                    CStr::from_ptr(a.mime_type).to_string_lossy().into_owned()
                };
                let content = if a.data.is_null() {
                    Vec::new()
                } else {
                    std::slice::from_raw_parts(a.data, a.data_len).to_vec()
                };
                Attachment {
                    filename,
                    mime_type,
                    content,
                }
            })
            .collect()
    };
    let user = Arc::new(SendableUserData(user_data));
    let complete_cb = on_complete;
    registry().runtime.spawn(async move {
        if let Some(progress_cb) = on_progress {
            let status = CString::new("sending").unwrap();
            (progress_cb)(status.as_ptr(), user.0);
        }
        let from_addr = parse_address(&from_str);
        let to_addrs: Vec<Address> = to_str.split(',').map(|s| parse_address(s.trim())).collect();
        let cc_addrs: Vec<Address> = cc_str
            .split(',')
            .map(|s| parse_address(s.trim()))
            .filter(|a| !a.local_part.is_empty())
            .collect();
        let payload = SendPayload {
            from: vec![from_addr],
            to: to_addrs,
            cc: cc_addrs,
            subject: subject_opt,
            body_plain: body_plain_opt,
            body_html: body_html_opt,
            attachments: att_list,
        };
        let result = match registry().transports.read().ok().and_then(|g| g.get(&uri).cloned()) {
            Some(holder) => match holder.0.send(&payload) {
                Ok(()) => {
                    clear_last_error();
                    0
                }
                Err(e) => {
                    set_last_error(&e);
                    -1
                }
            },
            None => {
                set_last_error(&StoreError::new("transport not found"));
                -1
            }
        };
        (complete_cb)(result, user.0);
    });
}

// ---------- Streaming send (non-blocking) ----------

/// Start a streaming send session. Returns session id (caller frees with tagliacarte_free_string), or NULL if transport not found or does not support streaming.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_transport_start_send(transport_uri: *const c_char) -> *mut c_char {
    let uri = match ptr_to_str(transport_uri) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let holder = match registry().transports.read().ok().and_then(|g| g.get(&uri).cloned()) {
        Some(h) => h,
        None => return ptr::null_mut(),
    };
    let session = match holder.0.start_send() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let ctr = registry().send_session_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let session_id = format!("send:{}:{}", uri, ctr);
    if let Ok(mut guard) = registry().send_sessions.write() {
        guard.insert(session_id.clone(), session);
    }
    clear_last_error();
    CString::new(session_id).unwrap().into_raw()
}

/// Set envelope and subject. Must be called first. Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_send_session_metadata(
    session_id: *const c_char,
    from: *const c_char,
    to: *const c_char,
    cc: *const c_char,
    subject: *const c_char,
) -> c_int {
    let id = match ptr_to_str(session_id) {
        Some(s) => s,
        None => {
            set_last_error(&StoreError::new("session_id is null"));
            return -1;
        }
    };
    if from.is_null() || to.is_null() {
        set_last_error(&StoreError::new("from and to are required"));
        return -1;
    }
    let envelope = Envelope {
        from: vec![parse_address(&CStr::from_ptr(from).to_string_lossy())],
        to: CStr::from_ptr(to)
            .to_string_lossy()
            .split(',')
            .map(|s| parse_address(s.trim()))
            .collect(),
        cc: if cc.is_null() {
            Vec::new()
        } else {
            CStr::from_ptr(cc)
                .to_string_lossy()
                .split(',')
                .map(|s| parse_address(s.trim()))
                .collect()
        },
        date: None,
        subject: None,
        message_id: None,
    };
    let subject_opt = if subject.is_null() {
        None
    } else {
        Some(CStr::from_ptr(subject).to_string_lossy().into_owned())
    };
    if let Ok(mut guard) = registry().send_sessions.write() {
        if let Some(session) = guard.get_mut(&id) {
            return match session.send_metadata(&envelope, subject_opt.as_deref()) {
                Ok(()) => {
                    clear_last_error();
                    0
                }
                Err(e) => {
                    set_last_error(&e);
                    -1
                }
            };
        }
    }
    set_last_error(&StoreError::new("send session not found"));
    -1
}

/// Append a chunk of plain-text body. Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_send_session_body_plain_chunk(
    session_id: *const c_char,
    data: *const u8,
    data_len: size_t,
) -> c_int {
    let id = match ptr_to_str(session_id) {
        Some(s) => s,
        None => return -1,
    };
    let slice = if data.is_null() || data_len == 0 {
        &[][..]
    } else {
        std::slice::from_raw_parts(data, data_len)
    };
    if let Ok(mut guard) = registry().send_sessions.write() {
        if let Some(session) = guard.get_mut(&id) {
            return match session.send_body_plain_chunk(slice) {
                Ok(()) => 0,
                Err(e) => {
                    set_last_error(&e);
                    -1
                }
            };
        }
    }
    -1
}

/// Append a chunk of HTML body. Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_send_session_body_html_chunk(
    session_id: *const c_char,
    data: *const u8,
    data_len: size_t,
) -> c_int {
    let id = match ptr_to_str(session_id) {
        Some(s) => s,
        None => return -1,
    };
    let slice = if data.is_null() || data_len == 0 {
        &[][..]
    } else {
        std::slice::from_raw_parts(data, data_len)
    };
    if let Ok(mut guard) = registry().send_sessions.write() {
        if let Some(session) = guard.get_mut(&id) {
            return match session.send_body_html_chunk(slice) {
                Ok(()) => 0,
                Err(e) => {
                    set_last_error(&e);
                    -1
                }
            };
        }
    }
    -1
}

/// Start an attachment (filename optional, mime_type required). Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_send_session_start_attachment(
    session_id: *const c_char,
    filename: *const c_char,
    mime_type: *const c_char,
) -> c_int {
    let id = match ptr_to_str(session_id) {
        Some(s) => s,
        None => return -1,
    };
    if mime_type.is_null() {
        set_last_error(&StoreError::new("mime_type is required"));
        return -1;
    }
    let filename_opt = if filename.is_null() {
        None
    } else {
        Some(CStr::from_ptr(filename).to_string_lossy().into_owned())
    };
    let mime = CStr::from_ptr(mime_type).to_string_lossy().into_owned();
    if let Ok(mut guard) = registry().send_sessions.write() {
        if let Some(session) = guard.get_mut(&id) {
            return match session.start_attachment(filename_opt.as_deref(), &mime) {
                Ok(()) => 0,
                Err(e) => {
                    set_last_error(&e);
                    -1
                }
            };
        }
    }
    -1
}

/// Append a chunk of the current attachment. Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_send_session_attachment_chunk(
    session_id: *const c_char,
    data: *const u8,
    data_len: size_t,
) -> c_int {
    let id = match ptr_to_str(session_id) {
        Some(s) => s,
        None => return -1,
    };
    let slice = if data.is_null() || data_len == 0 {
        &[][..]
    } else {
        std::slice::from_raw_parts(data, data_len)
    };
    if let Ok(mut guard) = registry().send_sessions.write() {
        if let Some(session) = guard.get_mut(&id) {
            return match session.send_attachment_chunk(slice) {
                Ok(()) => 0,
                Err(e) => {
                    set_last_error(&e);
                    -1
                }
            };
        }
    }
    -1
}

/// End the current attachment. Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_send_session_end_attachment(session_id: *const c_char) -> c_int {
    let id = match ptr_to_str(session_id) {
        Some(s) => s,
        None => return -1,
    };
    if let Ok(mut guard) = registry().send_sessions.write() {
        if let Some(session) = guard.get_mut(&id) {
            return match session.end_attachment() {
                Ok(()) => 0,
                Err(e) => {
                    set_last_error(&e);
                    -1
                }
            };
        }
    }
    -1
}

/// Finish and send. Returns immediately; on_complete(ok, user_data) is called from a background thread when done. ok: 0 = success, non-zero = error. Session is consumed; do not use session_id after this.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_send_session_end_send(
    session_id: *const c_char,
    on_complete: OnSendComplete,
    user_data: *mut c_void,
) {
    let id = match ptr_to_str(session_id) {
        Some(s) => s,
        None => return,
    };
    let session = if let Ok(mut guard) = registry().send_sessions.write() {
        guard.remove(&id)
    } else {
        None
    };
    let Some(session) = session else {
        return;
    };
    let fut = session.end_send();
    let user = std::sync::Arc::new(SendableUserData(user_data));
    registry().runtime.spawn(async move {
        let result = fut.await;
        let ok = if result.is_ok() { 0 } else { -1 };
        on_complete(ok, user.0);
    });
}

/// Discard a send session without sending. No-op if session_id not found or already ended.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_send_session_free(session_id: *const c_char) {
    let id = match ptr_to_str(session_id) {
        Some(s) => s,
        None => return,
    };
    let _ = registry().send_sessions.write().map(|mut g| g.remove(&id));
}

/// Free transport. No-op if transport is NULL.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_transport_free(transport_uri: *const c_char) {
    let uri = match ptr_to_str(transport_uri) {
        Some(s) => s,
        None => return,
    };
    let _ = registry().transports.write().map(|mut g| g.remove(&uri));
}

// ---------- Folder management ----------

/// Get the hierarchy delimiter for a store. Returns '\0' if unknown or not applicable.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_hierarchy_delimiter(store_uri: *const c_char) -> c_char {
    let uri = match ptr_to_str(store_uri) {
        Some(s) => s,
        None => return 0,
    };
    let holder = match registry().stores.read().ok().and_then(|g| g.get(&uri).cloned()) {
        Some(h) => h,
        None => return 0,
    };
    holder.store.hierarchy_delimiter().map(|c| c as c_char).unwrap_or(0)
}

/// Create a folder in the store. Returns immediately. On success, the existing on_folder_found callback fires.
/// On error, on_error(message, user_data) is called from a backend thread.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_create_folder(
    store_uri: *const c_char,
    name: *const c_char,
    on_error: OnFolderOpError,
    user_data: *mut c_void,
) {
    let uri = match ptr_to_str(store_uri) {
        Some(s) => s,
        None => return,
    };
    let name_str = match ptr_to_str(name) {
        Some(s) => s,
        None => return,
    };
    let holder = match registry().stores.read().ok().and_then(|g| g.get(&uri).cloned()) {
        Some(h) => h,
        None => return,
    };
    let user = Arc::new(SendableUserData(user_data));
    let callbacks = holder.folder_list_callbacks.read().ok().and_then(|g| g.clone());
    let user_for_cb = user.clone();
    let name_for_cb = name_str.clone();
    let on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send> = Box::new(move |result| {
        match result {
            Ok(()) => {
                // For non-IMAP stores (Maildir), fire on_folder_found from here
                if let Some(ref cbs) = callbacks {
                    let name_c = CString::new(name_for_cb.as_str()).unwrap();
                    let delim_char = 0 as c_char;
                    let attrs_c = CString::new("").unwrap();
                    (cbs.on_folder_found)(name_c.as_ptr(), delim_char, attrs_c.as_ptr(), cbs.user_data as *mut c_void);
                }
            }
            Err(e) => {
                let msg = CString::new(e.to_string()).unwrap_or_else(|_| CString::new("").unwrap());
                (on_error)(msg.as_ptr(), user_for_cb.0);
            }
        }
    });
    holder.store.create_folder(&name_str, on_complete);
}

/// Rename a folder. Returns immediately. On success, on_folder_removed + on_folder_found fire.
/// On error, on_error fires.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_rename_folder(
    store_uri: *const c_char,
    old_name: *const c_char,
    new_name: *const c_char,
    on_error: OnFolderOpError,
    user_data: *mut c_void,
) {
    let uri = match ptr_to_str(store_uri) {
        Some(s) => s,
        None => return,
    };
    let old = match ptr_to_str(old_name) {
        Some(s) => s,
        None => return,
    };
    let new = match ptr_to_str(new_name) {
        Some(s) => s,
        None => return,
    };
    let holder = match registry().stores.read().ok().and_then(|g| g.get(&uri).cloned()) {
        Some(h) => h,
        None => return,
    };
    let user = Arc::new(SendableUserData(user_data));
    let callbacks = holder.folder_list_callbacks.read().ok().and_then(|g| g.clone());
    let old_for_cb = old.clone();
    let new_for_cb = new.clone();
    let user_for_cb = user.clone();
    let on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send> = Box::new(move |result| {
        match result {
            Ok(()) => {
                if let Some(ref cbs) = callbacks {
                    let old_c = CString::new(old_for_cb.as_str()).unwrap();
                    (cbs.on_folder_removed)(old_c.as_ptr(), cbs.user_data as *mut c_void);
                    let new_c = CString::new(new_for_cb.as_str()).unwrap();
                    let attrs_c = CString::new("").unwrap();
                    (cbs.on_folder_found)(new_c.as_ptr(), 0, attrs_c.as_ptr(), cbs.user_data as *mut c_void);
                }
            }
            Err(e) => {
                let msg = CString::new(e.to_string()).unwrap_or_else(|_| CString::new("").unwrap());
                (on_error)(msg.as_ptr(), user_for_cb.0);
            }
        }
    });
    holder.store.rename_folder(&old, &new, on_complete);
}

/// Delete a folder. Returns immediately. On success, on_folder_removed fires.
/// On error, on_error fires.
#[no_mangle]
pub unsafe extern "C" fn tagliacarte_store_delete_folder(
    store_uri: *const c_char,
    name: *const c_char,
    on_error: OnFolderOpError,
    user_data: *mut c_void,
) {
    let uri = match ptr_to_str(store_uri) {
        Some(s) => s,
        None => return,
    };
    let name_str = match ptr_to_str(name) {
        Some(s) => s,
        None => return,
    };
    let holder = match registry().stores.read().ok().and_then(|g| g.get(&uri).cloned()) {
        Some(h) => h,
        None => return,
    };
    let user = Arc::new(SendableUserData(user_data));
    let callbacks = holder.folder_list_callbacks.read().ok().and_then(|g| g.clone());
    let user_for_cb = user.clone();
    let name_for_cb = name_str.clone();
    let on_complete: Box<dyn FnOnce(Result<(), StoreError>) + Send> = Box::new(move |result| {
        match result {
            Ok(()) => {
                if let Some(ref cbs) = callbacks {
                    let name_c = CString::new(name_for_cb.as_str()).unwrap();
                    (cbs.on_folder_removed)(name_c.as_ptr(), cbs.user_data as *mut c_void);
                }
            }
            Err(e) => {
                let msg = CString::new(e.to_string()).unwrap_or_else(|_| CString::new("").unwrap());
                (on_error)(msg.as_ptr(), user_for_cb.0);
            }
        }
    });
    holder.store.delete_folder(&name_str, on_complete);
}
