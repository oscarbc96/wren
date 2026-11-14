use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("manifest version 2 is not supported")]
    Mv2,
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid manifest: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExtensionId(pub String);

impl ExtensionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_spki_der(spki: &[u8]) -> Self {
        let hash = Sha256::digest(spki);
        let mut s = String::with_capacity(32);
        for b in &hash[..16] {
            s.push((b'a' + (b >> 4)) as char);
            s.push((b'a' + (b & 0x0f)) as char);
        }
        ExtensionId(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ContentScript {
    pub matches: Vec<String>,
    pub exclude_matches: Vec<String>,
    pub js: Vec<String>,
    pub css: Vec<String>,
    pub run_at: Option<String>,
    pub all_frames: bool,
    pub world: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Background {
    pub service_worker: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Action {
    pub default_popup: Option<String>,
    pub default_title: Option<String>,
    pub default_icon: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OptionsUi {
    pub page: String,
    pub open_in_tab: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WebAccessible {
    pub resources: Vec<String>,
    pub matches: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DnrRuleResource {
    pub id: String,
    pub enabled: bool,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Dnr {
    pub rule_resources: Vec<DnrRuleResource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Messages {
    #[serde(flatten)]
    pub map: std::collections::HashMap<String, MessageEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MessageEntry {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub manifest_version: u8,
    pub description: String,
    pub default_locale: Option<String>,
    pub permissions: Vec<String>,
    pub host_permissions: Vec<String>,
    pub content_scripts: Vec<ContentScript>,
    pub background: Option<Background>,
    pub action: Option<Action>,
    pub options_page: Option<String>,
    pub options_ui: Option<OptionsUi>,
    pub icons: BTreeMap<u32, String>,
    pub web_accessible_resources: Vec<WebAccessible>,
    pub declarative_net_request: Option<Dnr>,
    pub key: Option<String>,
}

impl Manifest {
    pub fn parse(json: &str, locale: Option<&Messages>) -> Result<Manifest, ManifestError> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        let mv = value
            .get("manifest_version")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if mv == 2 {
            return Err(ManifestError::Mv2);
        }
        let mut m: Manifest = serde_json::from_value(value)?;
        if let Some(msgs) = locale {
            m.name = subst(&m.name, msgs);
            m.description = subst(&m.description, msgs);
        }
        Ok(m)
    }
}

fn subst(s: &str, msgs: &Messages) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("__MSG_") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 6..];
        if let Some(end) = after.find("__") {
            let key = &after[..end];
            if let Some(entry) = msgs.map.get(key).or_else(|| {
                msgs.map
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(key))
                    .map(|(_, v)| v)
            }) {
                out.push_str(&entry.message);
            } else {
                out.push_str(&rest[start..start + 6 + end + 2]);
            }
            rest = &after[end + 2..];
        } else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}
