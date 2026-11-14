use crate::manifest::{ExtensionId, Manifest, ManifestError};
use p256::ecdsa::signature::Verifier as _;
use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256VerifyingKey};
use prost::Message;
use rsa::RsaPublicKey;
use rsa::pkcs1v15::{Signature as RsaSignature, VerifyingKey as RsaVerifyingKey};
use rsa::pkcs8::DecodePublicKey;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CrxError {
    #[error("bad signature")]
    BadSignature,
    #[error("invalid crx: {0}")]
    Invalid(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("manifest: {0}")]
    Manifest(#[from] ManifestError),
    #[error("zip: {0}")]
    Zip(String),
    #[error("network: {0}")]
    Network(String),
    #[error("missing Chrome Web Store publisher signature")]
    MissingPublisherProof,
}

#[derive(Clone, PartialEq, Message)]
struct AsymmetricKeyProof {
    #[prost(bytes = "vec", tag = "1")]
    public_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    signature: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct CrxFileHeader {
    #[prost(message, repeated, tag = "2")]
    sha256_with_rsa: Vec<AsymmetricKeyProof>,
    #[prost(message, repeated, tag = "3")]
    sha256_with_ecdsa: Vec<AsymmetricKeyProof>,
    #[prost(bytes = "vec", optional, tag = "10000")]
    signed_header_data: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
struct SignedData {
    #[prost(bytes = "vec", optional, tag = "1")]
    crx_id: Option<Vec<u8>>,
}

/// SHA-256 of the Chrome Web Store publisher key's DER SPKI (from Chromium's
/// crx_verifier.cc, BSD-3-Clause).
pub const PUBLISHER_KEY_HASH: [u8; 32] = [
    0x61, 0xf7, 0xf2, 0xa6, 0xbf, 0xcf, 0x74, 0xcd, 0x0b, 0xc1, 0xfe, 0x24, 0x97, 0xcc, 0x9b, 0x04,
    0x25, 0x4c, 0x65, 0x8f, 0x79, 0xf2, 0x14, 0x53, 0x92, 0x86, 0x7e, 0xa8, 0x36, 0x63, 0x67, 0xcf,
];

/// Which proofs a CRX must carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// Developer key only (side-loaded CRX files).
    DeveloperOnly,
    /// Developer key and the Chrome Web Store publisher key (store downloads).
    RequirePublisher,
}

pub fn verify_and_unpack(
    bytes: &[u8],
    dest: &Path,
    policy: Policy,
) -> Result<(ExtensionId, Manifest), CrxError> {
    // ensure dest absent on any failure path
    let res = verify_and_unpack_inner(bytes, dest, policy);
    if res.is_err() {
        let _ = fs::remove_dir_all(dest);
    }
    res
}

fn verify_and_unpack_inner(
    bytes: &[u8],
    dest: &Path,
    policy: Policy,
) -> Result<(ExtensionId, Manifest), CrxError> {
    if bytes.len() < 16 || &bytes[0..4] != b"Cr24" {
        return Err(CrxError::Invalid("bad magic".into()));
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if version != 3 {
        return Err(CrxError::Invalid("unsupported version".into()));
    }
    let header_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let header_end = 12 + header_len;
    if header_end > bytes.len() {
        return Err(CrxError::Invalid("truncated header".into()));
    }
    let header = CrxFileHeader::decode(&bytes[12..header_end])
        .map_err(|e| CrxError::Invalid(e.to_string()))?;
    let archive = &bytes[header_end..];
    let signed_header_data = header
        .signed_header_data
        .ok_or_else(|| CrxError::Invalid("missing signed_header_data".into()))?;
    let signed = SignedData::decode(signed_header_data.as_slice())
        .map_err(|e| CrxError::Invalid(e.to_string()))?;
    let crx_id_bytes = signed
        .crx_id
        .ok_or_else(|| CrxError::Invalid("missing crx_id".into()))?;
    if crx_id_bytes.len() != 16 {
        return Err(CrxError::Invalid("bad crx_id".into()));
    }

    // build signed message
    let mut msg = Vec::new();
    msg.extend_from_slice(b"CRX3 SignedData\x00");
    msg.extend_from_slice(&(signed_header_data.len() as u32).to_le_bytes());
    msg.extend_from_slice(&signed_header_data);
    msg.extend_from_slice(archive);

    // Chromium's rules (components/crx_file/crx_verifier.cc): every proof
    // must verify, one proof must carry the developer key whose hash is the
    // crx id, and Web Store downloads must also carry the publisher key.
    let mut found_id: Option<ExtensionId> = None;
    let mut found_publisher = false;
    let proofs = header
        .sha256_with_rsa
        .iter()
        .map(|p| (p, true))
        .chain(header.sha256_with_ecdsa.iter().map(|p| (p, false)));
    let mut any = false;
    for (proof, is_rsa) in proofs {
        any = true;
        let ok = if is_rsa {
            verify_rsa(&proof.public_key, &proof.signature, &msg)
        } else {
            verify_ecdsa(&proof.public_key, &proof.signature, &msg)
        };
        if !ok {
            return Err(CrxError::BadSignature);
        }
        let key_hash: [u8; 32] = Sha256::digest(&proof.public_key).into();
        if key_hash == PUBLISHER_KEY_HASH {
            found_publisher = true;
        }
        let id = ExtensionId::from_spki_der(&proof.public_key);
        if id_matches_crx(&id, &crx_id_bytes) {
            found_id = Some(id);
        }
    }
    if !any {
        return Err(CrxError::BadSignature);
    }
    let Some(id) = found_id else {
        return Err(CrxError::BadSignature);
    };
    if policy == Policy::RequirePublisher && !found_publisher {
        return Err(CrxError::MissingPublisherProof);
    }

    // unpack to temp then rename
    let parent = dest
        .parent()
        .ok_or_else(|| CrxError::Invalid("bad dest".into()))?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".tmp-ext-{}", id.as_str()));
    if tmp.exists() {
        let _ = fs::remove_dir_all(&tmp);
    }
    fs::create_dir_all(&tmp)?;
    unpack_zip(archive, &tmp)?;

    let manifest_path = tmp.join("manifest.json");
    let manifest_json = fs::read_to_string(&manifest_path)?;
    let manifest = Manifest::parse(&manifest_json, None)?;

    if dest.exists() {
        let _ = fs::remove_dir_all(dest);
    }
    fs::rename(&tmp, dest)?;

    Ok((id, manifest))
}

fn id_matches_crx(id: &ExtensionId, crx_id: &[u8]) -> bool {
    let expected = id_to_bytes(&id.0);
    match expected {
        Some(b) => b.as_slice() == crx_id,
        None => false,
    }
}

fn id_to_bytes(s: &str) -> Option<[u8; 16]> {
    if s.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    let bytes = s.as_bytes();
    for i in 0..16 {
        let hi = bytes[2 * i].checked_sub(b'a')?;
        let lo = bytes[2 * i + 1].checked_sub(b'a')?;
        if hi > 15 || lo > 15 {
            return None;
        }
        out[i] = (hi << 4) | lo;
    }
    Some(out)
}

fn verify_rsa(pub_der: &[u8], sig: &[u8], msg: &[u8]) -> bool {
    let Ok(pubkey) = RsaPublicKey::from_public_key_der(pub_der) else {
        return false;
    };
    let vkey = RsaVerifyingKey::<Sha256>::new(pubkey);
    let Ok(sig) = RsaSignature::try_from(sig) else {
        return false;
    };
    vkey.verify(msg, &sig).is_ok()
}

fn verify_ecdsa(pub_der: &[u8], sig: &[u8], msg: &[u8]) -> bool {
    let Ok(vkey) = P256VerifyingKey::from_public_key_der(pub_der) else {
        return false;
    };
    // try DER then fixed
    if let Ok(sig) = P256Signature::from_der(sig) {
        return vkey.verify(msg, &sig).is_ok();
    }
    if let Ok(sig) = P256Signature::from_slice(sig) {
        return vkey.verify(msg, &sig).is_ok();
    }
    false
}

fn unpack_zip(archive: &[u8], dest: &Path) -> Result<(), CrxError> {
    let reader = std::io::Cursor::new(archive);
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| CrxError::Zip(e.to_string()))?;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| CrxError::Zip(e.to_string()))?;
        let Some(name) = file.enclosed_name() else {
            return Err(CrxError::Invalid("bad zip entry".into()));
        };
        let outpath = dest.join(name);
        if file.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p)?;
            }
            let mut outfile = fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(())
}

pub async fn download_from_web_store(
    id: &ExtensionId,
    chrome_version: &str,
) -> Result<Vec<u8>, CrxError> {
    let url = web_store_url(id, chrome_version);
    let resp = reqwest::get(&url)
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| CrxError::Network(e.to_string()))?;
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| CrxError::Network(e.to_string()))?;
    Ok(bytes.to_vec())
}

pub fn web_store_url(id: &ExtensionId, chrome_version: &str) -> String {
    format!(
        "https://clients2.google.com/service/update2/crx?response=redirect&acceptformat=crx3&prodversion={}&x=id%3D{}%26uc",
        chrome_version,
        id.as_str()
    )
}
