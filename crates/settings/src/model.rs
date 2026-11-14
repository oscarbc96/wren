use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Identifier of a chat provider (for example `fake`), as registered by the app.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ProviderId(pub String);

/// Provider-side workspace identifier. Unique only within its provider.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct WorkspaceId(pub String);

/// Globally unique workspace key, text form `<provider>/<id>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkspaceKey {
    pub provider: ProviderId,
    pub id: WorkspaceId,
}

impl WorkspaceKey {
    pub fn new(provider: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            provider: ProviderId(provider.into()),
            id: WorkspaceId(id.into()),
        }
    }
}

impl std::fmt::Display for WorkspaceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.provider.0, self.id.0)
    }
}

impl std::str::FromStr for WorkspaceKey {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.split_once('/') {
            Some((provider, id))
                if !provider.is_empty() && !id.is_empty() && !id.contains(['/', '\\']) =>
            {
                Ok(Self::new(provider, id))
            }
            _ => Err(format!("workspace key {text:?} is not <provider>/<id>")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SearchSettings {
    pub url: String,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            url: "https://duckduckgo.com/?q={query}".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct StartupSettings {
    pub restore_session: bool,
}

impl Default for StartupSettings {
    fn default() -> Self {
        Self {
            restore_session: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TabSettings {
    pub new_tab_url: String,
    pub closed_stack: usize,
    pub sleep_after_minutes: u64,
    pub memory_budget_mb: u64,
}

impl Default for TabSettings {
    fn default() -> Self {
        Self {
            new_tab_url: "about:blank".into(),
            closed_stack: 25,
            sleep_after_minutes: 30,
            memory_budget_mb: 2048,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(default)]
pub struct SitesSettings {
    pub open_in_chrome: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct AdblockSettings {
    pub enabled: bool,
}

impl Default for AdblockSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct DownloadsSettings {
    pub dir: String,
}

impl Default for DownloadsSettings {
    fn default() -> Self {
        let dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("Downloads");
        Self {
            dir: dir.to_string_lossy().into_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct HangSettings {
    pub timeout_seconds: u64,
}

impl Default for HangSettings {
    fn default() -> Self {
        Self { timeout_seconds: 5 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct DevtoolsSettings {
    /// Start Servo's remote debugging server (Firefox DevTools) at launch.
    pub enabled: bool,
    /// Port on 127.0.0.1.
    pub port: u16,
}

impl Default for DevtoolsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 6000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CdpSettings {
    /// Start the Chrome DevTools Protocol server at launch.
    pub enabled: bool,
    /// Port on 127.0.0.1.
    pub port: u16,
}

impl Default for CdpSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 9222,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct MemorySettings {
    pub js_jit: bool,
    pub js_gc_profile: String,
    pub pressure_response: bool,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            js_jit: true,
            js_gc_profile: "default".to_string(),
            pressure_response: true,
        }
    }
}

/// All user-visible settings.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Settings {
    pub search: SearchSettings,
    pub startup: StartupSettings,
    pub tabs: TabSettings,
    pub sites: SitesSettings,
    pub adblock: AdblockSettings,
    pub downloads: DownloadsSettings,
    pub hang: HangSettings,
    pub devtools: DevtoolsSettings,
    pub cdp: CdpSettings,
    pub memory: MemorySettings,
}
