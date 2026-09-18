//! File and image attachments for chat turns.
//!
//! Files picked, dropped or pasted in the composer are ingested here: the raw
//! bytes are copied into `<data_dir>/attachments/<id>` and, when the file holds
//! readable text, an extracted copy is written next to it as `<id>.txt`. The
//! chat orchestrator later inlines that text into the prompt and sends images
//! to vision models as data URLs.
//!
//! Everything stays on this computer; nothing is uploaded anywhere.

pub mod base64;
mod office;

pub use base64::{decode as base64_decode, encode as base64_encode};

use crate::database::conversations::{new_id, now};
use crate::errors::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Largest file accepted at all.
pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
/// Largest image kept as an image (bigger ones are attached as binary files).
pub const MAX_IMAGE_BYTES: u64 = 12 * 1024 * 1024;
/// Longest extracted text stored per attachment.
pub const MAX_TEXT_CHARS: usize = 400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AttachmentKind {
    /// Raster image that a vision model can look at.
    Image,
    /// Readable text that is inlined into the prompt.
    Text,
    /// Anything else: only the file's name, type and size reach the model.
    Binary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub kind: AttachmentKind,
    pub size_bytes: u64,
    /// Characters of extracted text (0 for images and unreadable files).
    #[serde(default)]
    pub text_chars: u32,
    /// True when the extracted text was cut at [`MAX_TEXT_CHARS`].
    #[serde(default)]
    pub truncated: bool,
    /// Short explanation shown in the UI and given to the model, e.g. why a
    /// file's contents could not be read.
    #[serde(default)]
    pub note: Option<String>,
    pub created_at: String,
}

impl Attachment {
    pub fn is_image(&self) -> bool {
        self.kind == AttachmentKind::Image
    }
}

pub struct AttachmentStore {
    dir: PathBuf,
    /// Attachments that have been ingested but not yet sent with a message.
    staged: Mutex<HashMap<String, Attachment>>,
}

impl AttachmentStore {
    pub fn new(dir: PathBuf) -> Self {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(error = %e, dir = %dir.display(), "cannot create attachments directory");
        }
        Self { dir, staged: Mutex::new(HashMap::new()) }
    }

    fn staged(&self) -> std::sync::MutexGuard<'_, HashMap<String, Attachment>> {
        self.staged.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Claims staged attachments for a message being sent, in the order given.
    /// Unknown ids are skipped; the files stay on disk from here on.
    pub fn claim(&self, ids: &[String]) -> Vec<Attachment> {
        let mut staged = self.staged();
        ids.iter().filter_map(|id| staged.remove(id)).collect()
    }

    /// Drops a staged attachment the user removed before sending.
    pub fn discard(&self, id: &str) {
        let known = self.staged().remove(id).is_some();
        if known {
            self.remove(id);
        }
    }

    fn raw_path(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    fn text_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.txt"))
    }

    /// Reads a file from disk and stores it as an attachment.
    pub fn ingest_path(&self, path: &Path) -> AppResult<Attachment> {
        let meta = std::fs::metadata(path).map_err(|e| AppError::Io(format!("{}: {e}", path.display())))?;
        if meta.is_dir() {
            return Err(AppError::Invalid(format!("{} is a folder", name_of(path))));
        }
        if meta.len() > MAX_FILE_BYTES {
            return Err(AppError::Invalid(format!(
                "{} is {:.1} MB; the limit is {} MB",
                name_of(path),
                meta.len() as f64 / (1024.0 * 1024.0),
                MAX_FILE_BYTES / (1024 * 1024)
            )));
        }
        let bytes = std::fs::read(path).map_err(|e| AppError::Io(format!("{}: {e}", path.display())))?;
        self.ingest_bytes(&name_of(path), None, bytes)
    }

    /// Stores bytes that arrived from the clipboard or a drop.
    pub fn ingest_bytes(&self, name: &str, mime_hint: Option<&str>, bytes: Vec<u8>) -> AppResult<Attachment> {
        let name = sanitize_name(name);
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(AppError::Invalid(format!(
                "{name} is {:.1} MB; the limit is {} MB",
                bytes.len() as f64 / (1024.0 * 1024.0),
                MAX_FILE_BYTES / (1024 * 1024)
            )));
        }
        if bytes.is_empty() {
            return Err(AppError::Invalid(format!("{name} is empty")));
        }
        let ext = extension(&name);
        let mime = mime_hint
            .map(str::to_string)
            .filter(|m| !m.trim().is_empty() && m != "application/octet-stream")
            .unwrap_or_else(|| mime_for(&ext, &bytes));

        let id = new_id();
        let size_bytes = bytes.len() as u64;
        let mut note = None;
        let mut kind = AttachmentKind::Binary;
        let mut text: Option<String> = None;

        if mime.starts_with("image/") && mime != "image/svg+xml" {
            if size_bytes > MAX_IMAGE_BYTES {
                note = Some(format!(
                    "image larger than {} MB, so it was attached as a file and not shown to the model",
                    MAX_IMAGE_BYTES / (1024 * 1024)
                ));
            } else {
                kind = AttachmentKind::Image;
            }
        } else if let Some(extracted) = office::extract(&ext, &bytes) {
            if extracted.trim().is_empty() {
                note = Some("no text could be extracted from this document".into());
            } else {
                kind = AttachmentKind::Text;
                text = Some(extracted);
            }
        } else if ext == "pdf" {
            note = Some("PDF text extraction is not available; only the file name and size are known".into());
        } else if let Some(decoded) = decode_text(&bytes) {
            kind = AttachmentKind::Text;
            text = Some(decoded);
        } else {
            note = Some("binary file; its contents could not be read as text".into());
        }

        let mut truncated = false;
        if let Some(t) = text.as_mut() {
            if t.chars().count() > MAX_TEXT_CHARS {
                *t = t.chars().take(MAX_TEXT_CHARS).collect();
                truncated = true;
            }
        }

        std::fs::write(self.raw_path(&id), &bytes).map_err(|e| AppError::Io(e.to_string()))?;
        if let Some(t) = &text {
            std::fs::write(self.text_path(&id), t).map_err(|e| AppError::Io(e.to_string()))?;
        }

        let attachment = Attachment {
            id,
            name,
            mime,
            kind,
            size_bytes,
            text_chars: text.as_ref().map(|t| t.chars().count() as u32).unwrap_or(0),
            truncated,
            note,
            created_at: now(),
        };
        self.staged().insert(attachment.id.clone(), attachment.clone());
        Ok(attachment)
    }

    /// Stores a long block of pasted or typed text as a text attachment.
    pub fn ingest_text(&self, name: &str, text: &str) -> AppResult<Attachment> {
        let name = if name.trim().is_empty() { "pasted-text.txt".to_string() } else { name.to_string() };
        // No mime hint: the name's extension still decides how it is labelled.
        self.ingest_bytes(&name, None, text.as_bytes().to_vec())
    }

    /// Extracted text of a text attachment, if it is still on disk.
    pub fn text(&self, id: &str) -> Option<String> {
        std::fs::read_to_string(self.text_path(id)).ok()
    }

    pub fn bytes(&self, id: &str) -> AppResult<Vec<u8>> {
        std::fs::read(self.raw_path(id)).map_err(|e| AppError::Io(e.to_string()))
    }

    /// `data:` URL for an image attachment, as vision models expect it.
    pub fn data_url(&self, a: &Attachment) -> AppResult<String> {
        Ok(format!("data:{};base64,{}", a.mime, base64_encode(&self.bytes(&a.id)?)))
    }

    pub fn remove(&self, id: &str) {
        let _ = std::fs::remove_file(self.raw_path(id));
        let _ = std::fs::remove_file(self.text_path(id));
    }

    /// Deletes stored files that no message references any more. Staged
    /// attachments that were never sent are removed on the next start.
    pub fn gc(&self, keep: &HashSet<String>) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return };
        let mut removed = 0usize;
        for entry in entries.flatten() {
            let file = entry.file_name();
            let file = file.to_string_lossy();
            let id = file.strip_suffix(".txt").unwrap_or(&file);
            if !keep.contains(id) {
                let _ = std::fs::remove_file(entry.path());
                removed += 1;
            }
        }
        if removed > 0 {
            tracing::info!(files = removed, "removed unused attachment files");
        }
    }
}

fn name_of(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into())
}

/// Keeps a display name only: no directories, no control characters.
fn sanitize_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base.chars().filter(|c| !c.is_control()).take(120).collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "file".into()
    } else {
        cleaned
    }
}

pub fn extension(name: &str) -> String {
    name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

/// Human-readable size used in the prompt blocks and the UI.
pub fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{bytes} B")
    }
}

fn mime_for(ext: &str, bytes: &[u8]) -> String {
    let by_ext = match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "csv" => "text/csv",
        "md" | "markdown" => "text/markdown",
        "txt" | "log" | "ini" | "cfg" | "conf" | "env" => "text/plain",
        "html" | "htm" => "text/html",
        "xml" => "text/xml",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "zip" => "application/zip",
        _ => "",
    };
    if !by_ext.is_empty() {
        return by_ext.into();
    }
    // Sniff the few formats worth recognising without an extension.
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => "image/png".into(),
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg".into(),
        [b'G', b'I', b'F', b'8', ..] => "image/gif".into(),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp".into(),
        [b'%', b'P', b'D', b'F', ..] => "application/pdf".into(),
        _ if decode_text(bytes).is_some() => "text/plain".into(),
        _ => "application/octet-stream".into(),
    }
}

/// Returns the file as text when it decodes as UTF-8 and reads like text.
fn decode_text(bytes: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(bytes).ok()?;
    let mut control = 0usize;
    for c in s.chars().take(4096) {
        if c == '\0' {
            return None;
        }
        if c.is_control() && !matches!(c, '\n' | '\r' | '\t') {
            control += 1;
        }
    }
    if control > 16 {
        return None;
    }
    Some(s.trim_start_matches('\u{feff}').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (AttachmentStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (AttachmentStore::new(dir.path().join("attachments")), dir)
    }

    #[test]
    fn text_file_is_extracted() {
        let (s, _d) = store();
        let a = s.ingest_bytes("notes.md", None, b"# Title\nhello".to_vec()).unwrap();
        assert_eq!(a.kind, AttachmentKind::Text);
        assert_eq!(a.mime, "text/markdown");
        assert_eq!(s.text(&a.id).unwrap(), "# Title\nhello");
        assert_eq!(a.text_chars, 13);
        assert!(!a.truncated);
    }

    #[test]
    fn png_is_an_image_and_has_a_data_url() {
        let (s, _d) = store();
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
        let a = s.ingest_bytes("shot.png", None, png.to_vec()).unwrap();
        assert_eq!(a.kind, AttachmentKind::Image);
        assert!(s.data_url(&a).unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn binary_keeps_metadata_only() {
        let (s, _d) = store();
        let a = s.ingest_bytes("blob.dat", None, vec![0, 1, 2, 3, 0, 9]).unwrap();
        assert_eq!(a.kind, AttachmentKind::Binary);
        assert_eq!(a.text_chars, 0);
        assert!(a.note.is_some());
        assert!(s.text(&a.id).is_none());
    }

    #[test]
    fn long_text_is_truncated() {
        let (s, _d) = store();
        let a = s.ingest_text("big.txt", &"x".repeat(MAX_TEXT_CHARS + 500)).unwrap();
        assert!(a.truncated);
        assert_eq!(a.text_chars as usize, MAX_TEXT_CHARS);
    }

    #[test]
    fn names_are_sanitized_and_gc_keeps_referenced_files() {
        let (s, _d) = store();
        let a = s.ingest_bytes("../../etc/passwd", Some("text/plain"), b"root".to_vec()).unwrap();
        assert_eq!(a.name, "passwd");
        let b = s.ingest_text("second.txt", "keep me").unwrap();
        s.gc(&HashSet::from([b.id.clone()]));
        assert!(s.bytes(&a.id).is_err());
        assert_eq!(s.text(&b.id).unwrap(), "keep me");
    }

    #[test]
    fn oversized_files_are_rejected() {
        let (s, _d) = store();
        let err = s.ingest_bytes("huge.bin", None, vec![7; MAX_FILE_BYTES as usize + 1]).unwrap_err();
        assert!(err.to_string().contains("limit"));
    }
}
