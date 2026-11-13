use std::path::Path;

use anyhow::{Context, Result, bail};
use pbkdf2::pbkdf2_hmac;
use sha1::Sha1;
use tempfile::NamedTempFile;

use crate::{CredStore, Credential};

pub fn decrypt_v10(blob: &[u8], key: &[u8]) -> Result<String> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
    type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
    if key.len() != 16 {
        bail!("invalid key length");
    }
    if blob.len() < 3 + 16 {
        bail!("ciphertext too short");
    }
    if &blob[..3] != b"v10" {
        bail!("not a v10 blob");
    }
    let iv = &blob[3..19];
    let ct = &blob[19..];
    let mut buf = ct.to_vec();
    let pt = Aes128CbcDec::new_from_slices(key, iv)
        .map_err(|_| anyhow::anyhow!("cipher init failed"))?
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|_| anyhow::anyhow!("decrypt failed"))?;
    String::from_utf8(pt.to_vec()).context("utf8 password")
}

pub fn derive_key(password: &[u8]) -> [u8; 16] {
    let mut key = [0u8; 16];
    pbkdf2_hmac::<Sha1>(password, b"saltysalt", 1003, &mut key);
    key
}

fn chrome_safe_storage_key() -> Result<Vec<u8>> {
    if let Ok(marker) = std::env::var("WREN_CHROME_SAFE_STORAGE_KEY") {
        if marker == "missing" {
            bail!("Chrome Safe Storage key unavailable");
        }
    }
    if let Ok(hex) = std::env::var("WREN_CHROME_SAFE_STORAGE_KEY_HEX") {
        let bytes = decode_hex(&hex).context("invalid WREN_CHROME_SAFE_STORAGE_KEY_HEX")?;
        return Ok(bytes);
    }
    #[cfg(target_os = "macos")]
    {
        use security_framework::passwords::get_generic_password;
        for account in ["Chrome", "Chromium", "Brave"] {
            if let Ok(pw) = get_generic_password("Chrome Safe Storage", account) {
                return Ok(pw);
            }
        }
        bail!("Chrome Safe Storage key not found in Keychain")
    }
    #[cfg(not(target_os = "macos"))]
    {
        bail!("Chrome Safe Storage key not available on this platform")
    }
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

pub fn import(profile_dir: &Path, store: &dyn CredStore) -> Result<usize> {
    let login_data = profile_dir.join("Login Data");
    if !login_data.exists() {
        bail!("Login Data not found at {}", login_data.display());
    }
    let raw_key = chrome_safe_storage_key().context("cannot access Chrome Safe Storage key")?;
    let key = if raw_key.len() == 16 {
        let mut k = [0u8; 16];
        k.copy_from_slice(&raw_key);
        k
    } else {
        derive_key(&raw_key)
    };

    // copy to temp file before opening (Chrome may hold locks)
    let mut tmp = NamedTempFile::new().context("create temp Login Data copy")?;
    std::io::copy(
        &mut std::fs::File::open(&login_data).context("open Login Data")?,
        &mut tmp,
    )
    .context("copy Login Data")?;
    let tmp_path = tmp.path().to_path_buf();
    let conn = rusqlite::Connection::open_with_flags(
        &tmp_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .context("open copied Login Data")?;

    let mut stmt = conn
        .prepare("SELECT origin_url, username_value, password_value FROM logins")
        .context("prepare logins query")?;

    // collect first to ensure all-or-nothing on errors
    let mut creds = Vec::new();
    let rows = stmt
        .query_map([], |row| {
            let origin_url: String = row.get(0)?;
            let username: String = row.get(1).unwrap_or_default();
            let blob: Vec<u8> = row.get(2).unwrap_or_default();
            Ok((origin_url, username, blob))
        })
        .context("query logins")?;
    for row in rows {
        let (origin_url, username, blob) = row.context("read login row")?;
        if blob.is_empty() {
            continue;
        }
        let password = if blob.starts_with(b"v10") || blob.starts_with(b"v11") {
            decrypt_v10(&blob, &key)
                .with_context(|| format!("decrypt password for {origin_url}"))?
        } else {
            // older/unencrypted entries not expected; skip
            continue;
        };
        let origin = normalize_origin(&origin_url);
        creds.push(Credential {
            origin,
            username,
            password,
        });
    }

    // all succeeded, now write
    for c in &creds {
        store.save(c).context("save imported credential")?;
    }
    Ok(creds.len())
}

fn normalize_origin(url: &str) -> String {
    if let Ok(u) = url::Url::parse(url) {
        u.origin().ascii_serialization()
    } else {
        url.to_string()
    }
}
