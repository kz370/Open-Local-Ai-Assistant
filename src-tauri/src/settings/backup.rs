//! Settings backups: the settings as JSON, encrypted with ChaCha20-Poly1305
//! so the file is unreadable (and tamper-evident) outside the app.
//!
//! The key is built into the app, so this protects a backup that is copied
//! around or opened by accident; someone who pulls the key out of the app
//! binary could still decrypt it. API keys are only included when the user
//! asks for them.

use super::Settings;
use crate::errors::{AppError, AppResult};
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use sha2::{Digest, Sha256};

/// File signature plus format version.
const MAGIC: &[u8] = b"LAIA-BK1";
const NONCE_LEN: usize = 12;
/// Hashed into the 256-bit key; changing it makes old backups unreadable.
const KEY_SEED: &[u8] = b"local-ai-assistant/settings-backup/v1/7c1e9a42f0d84b6b";

fn cipher() -> ChaCha20Poly1305 {
    let key: [u8; 32] = Sha256::digest(KEY_SEED).into();
    ChaCha20Poly1305::new(Key::from_slice(&key))
}

/// Removes every API key (current provider and remembered profiles).
pub fn strip_keys(s: &mut Settings) {
    s.ai.api_key = None;
    for p in s.ai.provider_profiles.values_mut() {
        p.api_key = None;
    }
}

pub fn encrypt(settings: &Settings) -> AppResult<Vec<u8>> {
    let json = serde_json::to_vec(settings)?;
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = cipher().encrypt(&nonce, json.as_slice()).map_err(|_| AppError::Other("could not encrypt the backup".into()))?;
    let mut out = Vec::with_capacity(MAGIC.len() + NONCE_LEN + sealed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

pub fn decrypt(bytes: &[u8]) -> AppResult<Settings> {
    let invalid = || AppError::Invalid("this is not a Local Assistant settings backup, or it was damaged".into());
    let rest = bytes.strip_prefix(MAGIC).ok_or_else(invalid)?;
    if rest.len() <= NONCE_LEN {
        return Err(invalid());
    }
    let (nonce, sealed) = rest.split_at(NONCE_LEN);
    let json = cipher().decrypt(Nonce::from_slice(nonce), sealed).map_err(|_| invalid())?;
    serde_json::from_slice(&json).map_err(|_| invalid())
}

/// Folds an imported backup into the current settings. Things tied to this
/// computer (window spot, first-run state, open conversation) stay as they
/// are, and API keys missing from the backup keep their current value.
pub fn merge_import(current: &Settings, mut imported: Settings) -> Settings {
    imported.version = current.version;
    imported.last_conversation_id = current.last_conversation_id.clone();
    let g = &current.general;
    imported.general.first_run_complete = g.first_run_complete;
    imported.general.window = g.window.clone();
    imported.general.bubble_x = g.bubble_x;
    imported.general.bubble_y = g.bubble_y;
    imported.dictation.overlay_x = current.dictation.overlay_x;
    imported.dictation.overlay_y = current.dictation.overlay_y;

    // A key for provider P lives in `ai.api_key` when P is selected and in
    // `provider_profiles[P]` otherwise; look it up the same way on both sides.
    let key_of = |s: &Settings, provider: &str| {
        if s.ai.provider == provider {
            s.ai.api_key.clone()
        } else {
            s.ai.provider_profiles.get(provider).and_then(|p| p.api_key.clone())
        }
    };
    if imported.ai.api_key.is_none() {
        imported.ai.api_key = key_of(current, &imported.ai.provider);
    }
    let providers: Vec<String> = imported.ai.provider_profiles.keys().cloned().collect();
    for p in providers {
        if imported.ai.provider_profiles[&p].api_key.is_none() {
            let key = key_of(current, &p);
            imported.ai.provider_profiles.get_mut(&p).expect("listed").api_key = key;
        }
    }
    imported.sanitize();
    imported
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_hides_the_content() {
        let mut s = Settings::default();
        s.ai.api_key = Some("sk-secret-123".into());
        s.general.assistant_name = "Backup Test".into();
        let bytes = encrypt(&s).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains("sk-secret-123") && !text.contains("Backup Test"), "backup must not be readable");
        assert_eq!(decrypt(&bytes).unwrap(), s);
    }

    #[test]
    fn rejects_other_or_tampered_files() {
        assert!(decrypt(b"{\"general\":{}}").is_err());
        let mut bytes = encrypt(&Settings::default()).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(decrypt(&bytes).is_err());
    }

    #[test]
    fn strips_every_key() {
        let mut s = Settings::default();
        s.ai.api_key = Some("a".into());
        s.ai.provider_profiles.insert("openrouter".into(), super::super::ProviderProfile { api_key: Some("b".into()), ..Default::default() });
        strip_keys(&mut s);
        assert!(s.ai.api_key.is_none() && s.ai.provider_profiles["openrouter"].api_key.is_none());
    }

    #[test]
    fn import_keeps_local_things_and_missing_keys() {
        let mut current = Settings::default();
        current.general.first_run_complete = true;
        current.general.window.x = 1234;
        current.ai.provider = "openrouter".into();
        current.ai.api_key = Some("openrouter-key".into());
        let mut imported = Settings::default();
        imported.ai.provider = "openrouter".into();
        imported.general.assistant_name = "Imported".into();
        imported.general.window.x = 5;
        let merged = merge_import(&current, imported);
        assert_eq!(merged.general.assistant_name, "Imported");
        assert_eq!(merged.general.window.x, 1234);
        assert!(merged.general.first_run_complete);
        assert_eq!(merged.ai.api_key.as_deref(), Some("openrouter-key"));
    }
}
