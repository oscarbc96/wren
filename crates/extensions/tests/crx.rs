use extensions::crx::Policy;
use extensions::crx::{CrxError, verify_and_unpack};
use extensions::manifest::ExtensionId;
use p256::ecdsa::signature::Signer;
use prost::Message;
use rand::rngs::OsRng;
use rsa::RsaPrivateKey;
use rsa::pkcs1v15::SigningKey as RsaSigningKey;
use rsa::pkcs8::EncodePublicKey as _;
use rsa::signature::SignatureEncoding;
use sha2::{Digest, Sha256};
use std::io::Write;
use tempfile::TempDir;

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

fn id_from_spki(spki: &[u8]) -> (ExtensionId, [u8; 16]) {
    let hash = Sha256::digest(spki);
    let mut id_bytes = [0u8; 16];
    id_bytes.copy_from_slice(&hash[..16]);
    let mut s = String::with_capacity(32);
    for b in id_bytes {
        s.push((b'a' + (b >> 4)) as char);
        s.push((b'a' + (b & 0x0f)) as char);
    }
    (ExtensionId(s), id_bytes)
}

fn build_zip(manifest_json: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(manifest_json.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    buf
}

fn build_crx_rsa() -> (Vec<u8>, ExtensionId) {
    let mut rng = OsRng;
    let priv_key = RsaPrivateKey::new(&mut rng, 2048).expect("rsa key");
    let pub_der = priv_key.to_public_key().to_public_key_der().unwrap();
    let pub_bytes = pub_der.as_bytes().to_vec();
    let (id, crx_id) = id_from_spki(&pub_bytes);

    let manifest = r#"{"manifest_version":3,"name":"Hi","version":"1.0.0"}"#;
    let zip_bytes = build_zip(manifest);

    let signed_data = SignedData {
        crx_id: Some(crx_id.to_vec()),
    };
    let mut signed_header_data = Vec::new();
    signed_data.encode(&mut signed_header_data).unwrap();

    let mut msg = Vec::new();
    msg.extend_from_slice(b"CRX3 SignedData\x00");
    msg.extend_from_slice(&(signed_header_data.len() as u32).to_le_bytes());
    msg.extend_from_slice(&signed_header_data);
    msg.extend_from_slice(&zip_bytes);

    let signing_key = RsaSigningKey::<Sha256>::new(priv_key);
    let sig = signing_key.sign(&msg);
    let sig_bytes = sig.to_bytes().to_vec();

    let header = CrxFileHeader {
        sha256_with_rsa: vec![AsymmetricKeyProof {
            public_key: pub_bytes,
            signature: sig_bytes,
        }],
        sha256_with_ecdsa: vec![],
        signed_header_data: Some(signed_header_data),
    };
    let mut header_bytes = Vec::new();
    header.encode(&mut header_bytes).unwrap();

    let mut crx = Vec::new();
    crx.extend_from_slice(b"Cr24");
    crx.extend_from_slice(&3u32.to_le_bytes());
    crx.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
    crx.extend_from_slice(&header_bytes);
    crx.extend_from_slice(&zip_bytes);
    (crx, id)
}

#[test]
fn id_from_key_is_32_a_to_p() {
    // derive from a generated key to assert format and determinism
    let mut rng = OsRng;
    let priv_key = RsaPrivateKey::new(&mut rng, 2048).expect("rsa key");
    let pub_der = priv_key.to_public_key().to_public_key_der().unwrap();
    let pub_bytes = pub_der.as_bytes();
    let (id, _) = id_from_spki(pub_bytes);
    assert_eq!(id.0.len(), 32);
    assert!(id.0.chars().all(|c| ('a'..='p').contains(&c)));
    // verify exposed API
    let api_id = ExtensionId::from_spki_der(pub_bytes);
    assert_eq!(api_id, id);
}

#[test]
fn valid_crx_installs() {
    let (crx, expected_id) = build_crx_rsa();
    let dir = TempDir::new().unwrap();
    let dest = dir.path().join("ext");
    let (id, manifest) = verify_and_unpack(&crx, &dest, Policy::DeveloperOnly).expect("valid crx");
    assert_eq!(id, expected_id);
    assert_eq!(manifest.manifest_version, 3);
    assert_eq!(manifest.name, "Hi");
    assert_eq!(manifest.version, "1.0.0");
    assert!(dest.join("manifest.json").exists());
}

#[test]
fn tampered_crx_rejected_and_cleaned() {
    let (mut crx, _) = build_crx_rsa();
    // flip one byte in the zip payload (at the end)
    let last = crx.len() - 1;
    crx[last] ^= 0x01;
    let dir = TempDir::new().unwrap();
    let dest = dir.path().join("ext");
    let err = verify_and_unpack(&crx, &dest, Policy::DeveloperOnly).unwrap_err();
    assert!(matches!(err, CrxError::BadSignature));
    assert!(!dest.exists(), "dest dir must be absent on failure");
}

#[test]
fn web_store_url_builds_expected() {
    let id = ExtensionId("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
    let url = extensions::crx::web_store_url(&id, "142.0.0.0");
    assert_eq!(
        url,
        "https://clients2.google.com/service/update2/crx?response=redirect&acceptformat=crx3&prodversion=142.0.0.0&x=id%3Daaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa%26uc"
    );
}

#[test]
fn store_download_without_publisher_proof_rejected() {
    let (crx, _) = build_crx_rsa();
    let dir = TempDir::new().unwrap();
    let dest = dir.path().join("ext");
    let err = verify_and_unpack(&crx, &dest, Policy::RequirePublisher).unwrap_err();
    assert!(matches!(err, extensions::crx::CrxError::MissingPublisherProof));
    assert!(!dest.exists());
}
