//! Encrypts the few secrets Margincite stores (Google refresh tokens, Bing API
//! keys). ChaCha20-Poly1305 with a fresh random nonce for every seal.

use std::io::Write;
use std::path::Path;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chacha20poly1305::aead::{Aead, Generate, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};

use crate::config::ConfigError;

pub const KEY_FILE: &str = "secret.key";
const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SecretError {
    #[error("The stored secret could not be read.")]
    Unreadable,
}

/// The key that seals and opens stored secrets. Its `Debug` output never
/// shows the key, so a config can be logged safely.
#[derive(Clone)]
pub struct Secrets {
    cipher: ChaCha20Poly1305,
}

impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secrets(..)")
    }
}

impl Secrets {
    /// Reads a key written as base64 of exactly 32 bytes.
    pub fn from_base64(key: &str) -> Result<Secrets, ConfigError> {
        let bytes = STANDARD.decode(key.trim()).map_err(|_| {
            ConfigError::new("MARGINCITE_SECRET_KEY is not valid base64 (expected 32 random bytes)")
        })?;
        let bytes: [u8; KEY_BYTES] = bytes.try_into().map_err(|_| {
            ConfigError::new("MARGINCITE_SECRET_KEY must decode to exactly 32 bytes")
        })?;
        Ok(Secrets::from_bytes(bytes))
    }

    /// The key kept in `<dir>/secret.key`, made with 32 random bytes the
    /// first time. The file is private to its owner from the moment it exists.
    /// An unreadable or malformed file is an error, never replaced: a new key
    /// would make everything already sealed unreadable.
    pub fn load_or_create(dir: &Path) -> Result<Secrets, ConfigError> {
        let path = dir.join(KEY_FILE);
        let unusable = |why: &str| {
            ConfigError::new(format!(
                "The secret key file {} {why}. Restore it from a backup. Deleting it makes a new key, \
                 and then every secret saved in the app has to be entered again.",
                path.display()
            ))
        };
        create_private_dir(dir).map_err(|_| unusable("cannot be prepared"))?;
        match write_new_key(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(unusable("cannot be created")),
        }
        let text = std::fs::read_to_string(&path).map_err(|_| unusable("cannot be read"))?;
        let bytes = STANDARD
            .decode(text.trim())
            .map_err(|_| unusable("is not valid base64"))?;
        let bytes: [u8; KEY_BYTES] = bytes
            .try_into()
            .map_err(|_| unusable("does not hold exactly 32 bytes"))?;
        Ok(Secrets::from_bytes(bytes))
    }

    fn from_bytes(bytes: [u8; KEY_BYTES]) -> Secrets {
        Secrets {
            cipher: ChaCha20Poly1305::new(&bytes.into()),
        }
    }

    /// `base64(nonce || ciphertext)`, with a random nonce each time.
    pub fn seal(&self, plain: &str) -> String {
        let nonce = Nonce::generate();
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plain.as_bytes())
            .expect("sealing in memory cannot fail");
        let mut out = nonce.to_vec();
        out.extend_from_slice(&ciphertext);
        STANDARD.encode(out)
    }

    pub fn open(&self, sealed: &str) -> Result<String, SecretError> {
        let bytes = STANDARD
            .decode(sealed.trim())
            .map_err(|_| SecretError::Unreadable)?;
        if bytes.len() <= NONCE_BYTES {
            return Err(SecretError::Unreadable);
        }
        let (nonce, ciphertext) = bytes.split_at(NONCE_BYTES);
        let nonce = Nonce::try_from(nonce).map_err(|_| SecretError::Unreadable)?;
        let plain = self
            .cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|_| SecretError::Unreadable)?;
        String::from_utf8(plain).map_err(|_| SecretError::Unreadable)
    }
}

fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)
}

/// Creates the file, failing when it exists, with 32 random bytes in it.
fn write_new_key(path: &Path) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    let key = chacha20poly1305::Key::generate();
    file.write_all(STANDARD.encode(key).as_bytes())?;
    file.write_all(b"\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> String {
        STANDARD.encode([7u8; 32])
    }

    #[test]
    fn seal_then_open_returns_the_original() {
        let secrets = Secrets::from_base64(&key()).unwrap();
        let sealed = secrets.seal("1//refresh-token");
        assert!(!sealed.contains("refresh-token"));
        assert_eq!(secrets.open(&sealed).unwrap(), "1//refresh-token");
        assert_eq!(secrets.open(&secrets.seal("")).unwrap(), "");
    }

    #[test]
    fn two_seals_of_one_value_differ() {
        let secrets = Secrets::from_base64(&key()).unwrap();
        assert_ne!(secrets.seal("same"), secrets.seal("same"));
    }

    #[test]
    fn a_changed_byte_is_refused() {
        let secrets = Secrets::from_base64(&key()).unwrap();
        let mut bytes = STANDARD.decode(secrets.seal("secret")).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert_eq!(
            secrets.open(&STANDARD.encode(&bytes)),
            Err(SecretError::Unreadable)
        );
        bytes[last] ^= 1;
        bytes[0] ^= 1;
        assert_eq!(
            secrets.open(&STANDARD.encode(&bytes)),
            Err(SecretError::Unreadable)
        );
    }

    #[test]
    fn garbage_and_short_input_are_refused() {
        let secrets = Secrets::from_base64(&key()).unwrap();
        for bad in ["", "not base64!!", &STANDARD.encode([1u8; 12])] {
            assert_eq!(secrets.open(bad), Err(SecretError::Unreadable), "{bad}");
        }
    }

    #[test]
    fn another_key_cannot_open_it() {
        let secrets = Secrets::from_base64(&key()).unwrap();
        let other = Secrets::from_base64(&STANDARD.encode([8u8; 32])).unwrap();
        assert!(other.open(&secrets.seal("secret")).is_err());
    }

    #[test]
    fn a_key_that_is_not_32_bytes_is_refused() {
        for bad in [STANDARD.encode([1u8; 16]), STANDARD.encode([1u8; 33])] {
            let err = Secrets::from_base64(&bad).unwrap_err();
            assert!(err.to_string().contains("MARGINCITE_SECRET_KEY"));
        }
        let err = Secrets::from_base64("%%%").unwrap_err();
        assert!(err.to_string().contains("MARGINCITE_SECRET_KEY"));
    }

    fn temp_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("margincite-key-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn the_key_file_is_created_once_and_reused() {
        let dir = temp_dir().join("nested");
        let first = Secrets::load_or_create(&dir).unwrap();
        let path = dir.join(KEY_FILE);
        let bytes = std::fs::read_to_string(&path).unwrap();
        assert_eq!(STANDARD.decode(bytes.trim()).unwrap().len(), 32);
        let second = Secrets::load_or_create(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), bytes);
        assert_eq!(second.open(&first.seal("same key")).unwrap(), "same key");
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }

    #[cfg(unix)]
    #[test]
    fn the_key_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir();
        Secrets::load_or_create(&dir).unwrap();
        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir.join(KEY_FILE)), 0o600);
        assert_eq!(mode(&dir), 0o700);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_malformed_key_file_stops_start_up_without_showing_its_contents() {
        for bad in ["not base64 at all!!", "c2hvcnQ=", ""] {
            let dir = temp_dir();
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(KEY_FILE), bad).unwrap();
            let err = Secrets::load_or_create(&dir).unwrap_err().to_string();
            assert!(err.contains("secret.key"), "{err}");
            assert!(bad.is_empty() || !err.contains(bad), "{err}");
            std::fs::remove_dir_all(&dir).ok();
        }
    }

    #[test]
    fn debug_output_hides_the_key() {
        let secrets = Secrets::from_base64(&key()).unwrap();
        assert_eq!(format!("{secrets:?}"), "Secrets(..)");
    }
}
