use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::{Settings, SettingsError, WorkspaceKey};

/// One settings source. Earlier layers have lower precedence.
#[derive(Debug, Clone, Copy)]
pub struct Layer<'a> {
    pub path: &'a Path,
    pub text: &'a str,
}

const DEFAULT_TEXT: &str = include_str!("../../../assets/settings/default.json");

/// The embedded built-in defaults as a layer.
pub fn default_layer() -> Layer<'static> {
    Layer {
        path: Path::new("default.json"),
        text: DEFAULT_TEXT,
    }
}

/// Merge `layers` (lowest precedence first) over nothing and type-check the result.
/// Objects merge, everything else (including arrays) replaces.
pub fn parse_layers(layers: &[Layer<'_>]) -> Result<Settings, SettingsError> {
    let mut merged = Value::Object(Map::new());
    for layer in layers {
        let value = parse_value(layer)?;
        // Check each layer alone so a type error names the file that caused it.
        // Every section has `#[serde(default)]`, so a partial layer is valid.
        deserialize(layer.path, value.clone())?;
        merge(&mut merged, value);
    }
    let path = layers.last().map(|l| l.path).unwrap_or(Path::new(""));
    deserialize(path, merged)
}

/// Settings for one workspace: `base` layers, then the workspace layer when a
/// workspace is given.
pub fn effective_for(
    workspace_key: Option<&WorkspaceKey>,
    base: &[Layer<'_>],
    workspace: Option<Layer<'_>>,
) -> Result<Settings, SettingsError> {
    let mut layers: Vec<Layer<'_>> = base.to_vec();
    if let (Some(_), Some(ws)) = (workspace_key, workspace) {
        layers.push(ws);
    }
    parse_layers(&layers)
}

pub(crate) fn parse_value(layer: &Layer<'_>) -> Result<Value, SettingsError> {
    if layer.text.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    parse_jsonc::<Value>(layer.text).map_err(|e| {
        let (line, col, message) = split_position(&e);
        SettingsError::Parse {
            path: layer.path.to_path_buf(),
            line,
            col,
            message,
        }
    })
}

/// Parse JSONC (comments and trailing commas allowed) with `serde_json_lenient`,
/// reporting the JSON path of a failure like Zed's `parse_json_with_comments`.
pub(crate) fn parse_jsonc<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, String> {
    let mut de = serde_json_lenient::Deserializer::from_str(text);
    let value = serde_path_to_error::deserialize(&mut de).map_err(|e| e.to_string())?;
    de.end().map_err(|e| e.to_string())?;
    Ok(value)
}

/// serde_json style errors end in ` at line L column C`; split it off.
pub(crate) fn split_position(text: &str) -> (usize, usize, String) {
    if let Some(idx) = text.rfind(" at line ") {
        let tail = &text[idx + " at line ".len()..];
        if let Some((l, c)) = tail.split_once(" column ")
            && let (Ok(line), Ok(col)) = (l.trim().parse(), c.trim().parse())
        {
            return (line, col, text[..idx].to_string());
        }
    }
    (1, 1, text.to_string())
}

fn deserialize(path: &Path, value: Value) -> Result<Settings, SettingsError> {
    serde_path_to_error::deserialize(value).map_err(|e| SettingsError::Type {
        path: PathBuf::from(path),
        pointer: to_pointer(e.path()),
        message: e.inner().to_string(),
    })
}

fn to_pointer(path: &serde_path_to_error::Path) -> String {
    use serde_path_to_error::Segment;
    let mut out = String::new();
    for seg in path.iter() {
        out.push('/');
        match seg {
            Segment::Seq { index } => out.push_str(&index.to_string()),
            Segment::Map { key } | Segment::Enum { variant: key } => {
                out.push_str(&key.replace('~', "~0").replace('/', "~1"))
            }
            Segment::Unknown => out.push('?'),
        }
    }
    out
}

fn merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(&k) {
                    Some(slot) => merge(slot, v),
                    None => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (slot, v) => *slot = v,
    }
}
