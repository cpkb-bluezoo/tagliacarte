//! Attachments for a message being composed.
//!
//! Their bytes never travel inside JSON. The page POSTs each file as a raw
//! body to `/upload`, or names a path chosen in a native dialog, and this
//! store keeps it (a temp file for an upload, the original for a path) until
//! the message is sent or the attachment removed. Files are stored under
//! generated ids, never under a name the page supplied.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use base64::Engine;
use tagliacarte_app::compose_send::OutgoingFile;
use tagliacarte_app::frb_api::FrbComposeAttachment;
use tagliacarte_core::json::{JsonWriter, writer_into_string};

/// The base64 form a message is built from is held in memory at send time, so
/// attachments are capped well below what that could not bear.
pub const MAX_ATTACHMENT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub size: u64,
}

impl Info {
    pub fn to_json(&self) -> String {
        let mut w = JsonWriter::new();
        w.write_start_object();
        for (k, v) in [("id", &self.id), ("name", &self.name), ("mimeType", &self.mime_type)] {
            w.write_key(k);
            w.write_string(v);
        }
        w.write_key("size");
        w.write_i64(self.size as i64);
        w.write_end_object();
        writer_into_string(w)
    }
}

struct Item {
    info: Info,
    path: PathBuf,
    /// Whether the file is ours to delete (an upload) or the user's (a path).
    owned: bool,
}

pub struct Uploads {
    dir: PathBuf,
    items: Mutex<HashMap<String, Item>>,
}

fn random_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    (0..2)
        .map(|_| {
            let h = std::collections::hash_map::RandomState::new().build_hasher().finish();
            format!("{h:016x}")
        })
        .collect()
}

/// A file name and nothing else: the part after the last separator, with
/// control characters removed.
pub fn clean_name(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let name: String = base.chars().filter(|c| !c.is_control()).collect();
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." { "attachment".to_owned() } else { name.to_owned() }
}

pub fn guess_mime(name: &str) -> &'static str {
    match name.rsplit('.').next().map(str::to_ascii_lowercase).as_deref() {
        Some("txt") | Some("log") => "text/plain",
        Some("html") | Some("htm") => "text/html",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("zip") => "application/zip",
        Some("json") => "application/json",
        Some("csv") => "text/csv",
        Some("vcf") => "text/vcard",
        Some("ics") => "text/calendar",
        Some("doc") => "application/msword",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    }
}

impl Uploads {
    pub fn new() -> Self {
        Self {
            dir: std::env::temp_dir().join(format!("tagliacarte-uploads-{}-{}", std::process::id(), random_id())),
            items: Mutex::new(HashMap::new()),
        }
    }

    /// Keeps `bytes` as an attachment named `name`.
    pub fn put_bytes(&self, name: &str, mime_type: &str, bytes: &[u8]) -> Result<Info, String> {
        if bytes.len() as u64 > MAX_ATTACHMENT_BYTES {
            return Err(format!("attachments are limited to {} MB", MAX_ATTACHMENT_BYTES / (1024 * 1024)));
        }
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("could not store the attachment: {e}"))?;
        let id = random_id();
        let path = self.dir.join(&id);
        std::fs::write(&path, bytes).map_err(|e| format!("could not store the attachment: {e}"))?;
        let name = clean_name(name);
        let mime_type = if mime_type.trim().is_empty() { guess_mime(&name).to_owned() } else { mime_type.trim().to_owned() };
        let info = Info { id: id.clone(), name, mime_type, size: bytes.len() as u64 };
        self.items.lock().unwrap().insert(id, Item { info: info.clone(), path, owned: true });
        Ok(info)
    }

    /// Refers to a file the user chose. It is read when the message is sent.
    pub fn add_path(&self, path: &Path) -> Result<Info, String> {
        let meta = std::fs::metadata(path).map_err(|e| format!("could not read the file: {e}"))?;
        if !meta.is_file() {
            return Err("that is not a file".to_owned());
        }
        if meta.len() > MAX_ATTACHMENT_BYTES {
            return Err(format!("attachments are limited to {} MB", MAX_ATTACHMENT_BYTES / (1024 * 1024)));
        }
        let id = random_id();
        let name = clean_name(&path.to_string_lossy());
        let info = Info { id: id.clone(), mime_type: guess_mime(&name).to_owned(), name, size: meta.len() };
        self.items.lock().unwrap().insert(id, Item { info: info.clone(), path: path.to_path_buf(), owned: false });
        Ok(info)
    }

    pub fn remove(&self, id: &str) {
        if let Some(item) = self.items.lock().unwrap().remove(id) {
            if item.owned {
                let _ = std::fs::remove_file(item.path);
            }
        }
    }

    fn read(&self, id: &str) -> Result<(Info, Vec<u8>), String> {
        let (info, path) = {
            let g = self.items.lock().unwrap();
            let item = g.get(id).ok_or_else(|| format!("no such attachment '{id}'"))?;
            (item.info.clone(), item.path.clone())
        };
        let bytes = std::fs::read(&path).map_err(|e| format!("could not read attachment '{}': {e}", info.name))?;
        Ok((info, bytes))
    }

    /// The attachment as a file to stream into the message.
    pub fn outgoing(&self, id: &str) -> Result<OutgoingFile, String> {
        let g = self.items.lock().unwrap();
        let item = g.get(id).ok_or_else(|| format!("no such attachment '{id}'"))?;
        Ok(OutgoingFile {
            filename: Some(item.info.name.clone()),
            mime_type: item.info.mime_type.clone(),
            path: item.path.clone(),
        })
    }

    /// The attachment as the send API takes it. This is where its bytes first
    /// sit in memory, and only for as long as the message is being built.
    pub fn for_message(&self, id: &str) -> Result<FrbComposeAttachment, String> {
        let (info, bytes) = self.read(id)?;
        Ok(FrbComposeAttachment {
            filename: Some(info.name),
            mime_type: info.mime_type,
            bytes_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        })
    }
}

impl Drop for Uploads {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    #[test]
    fn names_are_reduced_to_a_file_name() {
        assert_eq!(clean_name("../../etc/passwd"), "passwd");
        assert_eq!(clean_name("C:\\Users\\me\\report.pdf"), "report.pdf");
        assert_eq!(clean_name("a\nb\u{0}.txt"), "ab.txt");
        assert_eq!(clean_name(".."), "attachment");
        assert_eq!(clean_name(""), "attachment");
        assert_eq!(clean_name("  photo.jpg "), "photo.jpg");
    }

    #[test]
    fn an_upload_is_kept_under_a_generated_id_and_removed_with_its_file() {
        let u = Uploads::new();
        let info = u.put_bytes("../../evil.png", "", b"\x89PNG data").unwrap();
        assert_eq!((info.name.as_str(), info.mime_type.as_str(), info.size), ("evil.png", "image/png", 9));
        assert!(u.dir.join(&info.id).is_file(), "stored under its id, not under the supplied name");

        let a = u.for_message(&info.id).unwrap();
        assert_eq!(a.filename.as_deref(), Some("evil.png"));
        assert_eq!(a.bytes_base64, base64::engine::general_purpose::STANDARD.encode(b"\x89PNG data"));

        u.remove(&info.id);
        assert!(!u.dir.join(&info.id).exists());
        assert!(u.for_message(&info.id).is_err());
    }

    #[test]
    fn a_path_is_referenced_not_copied_and_not_deleted() {
        let dir = tempfile_dir();
        let file = dir.join("notes.txt");
        std::fs::write(&file, b"hello").unwrap();
        let u = Uploads::new();
        let info = u.add_path(&file).unwrap();
        assert_eq!((info.name.as_str(), info.mime_type.as_str(), info.size), ("notes.txt", "text/plain", 5));
        assert_eq!(u.for_message(&info.id).unwrap().bytes_base64, "aGVsbG8=");
        u.remove(&info.id);
        assert!(file.exists(), "the user's own file is never deleted");
        assert!(u.add_path(&dir).is_err(), "a directory is not an attachment");
        assert!(u.add_path(&dir.join("missing")).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn oversize_uploads_are_refused_and_the_store_cleans_up_after_itself() {
        let u = Uploads::new();
        let dir = u.dir.clone();
        u.put_bytes("a.txt", "text/plain", b"x").unwrap();
        assert!(dir.is_dir());
        let big = vec![0u8; (MAX_ATTACHMENT_BYTES + 1) as usize];
        assert!(u.put_bytes("big.bin", "", &big).unwrap_err().contains("limited"));
        drop(u);
        assert!(!dir.exists(), "temp files go when the app does");
    }

    #[test]
    fn info_is_json() {
        let v = Value::parse(&Info { id: "i".into(), name: "a \"b\".txt".into(), mime_type: "text/plain".into(), size: 3 }.to_json()).unwrap();
        assert_eq!(v.opt_str("name"), Some("a \"b\".txt"));
        assert_eq!(v.u64("size").unwrap(), 3);
    }

    fn tempfile_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("tc-up-test-{}-{}", std::process::id(), random_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
