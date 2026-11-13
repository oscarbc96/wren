use serde_json::Value;

use crate::{Layer, Settings, load::parse_value};

/// JSON Schema (draft 2020-12) of the settings file.
pub fn settings_schema() -> Value {
    serde_json::to_value(schemars::schema_for!(Settings)).expect("schema serializes")
}

/// JSON pointers of keys in `text` that the schema does not know. Unparsable
/// text yields none (the parse error is reported elsewhere).
pub fn unknown_keys(text: &str) -> Vec<String> {
    let layer = Layer {
        path: std::path::Path::new(""),
        text,
    };
    let Ok(value) = parse_value(&layer) else {
        return Vec::new();
    };
    let schema = settings_schema();
    let mut out = Vec::new();
    walk(&schema, &schema, &value, "", &mut out);
    out.sort();
    out.dedup();
    out
}

fn resolve<'a>(root: &'a Value, schema: &'a Value) -> &'a Value {
    let mut cur = schema;
    // Bounded to avoid looping on a self-referencing schema.
    for _ in 0..16 {
        let Some(r) = cur.get("$ref").and_then(Value::as_str) else {
            break;
        };
        let Some(target) = r.strip_prefix('#').and_then(|p| root.pointer(p)) else {
            break;
        };
        cur = target;
    }
    cur
}

fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn walk(root: &Value, schema: &Value, value: &Value, pointer: &str, out: &mut Vec<String>) {
    let schema = resolve(root, schema);
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(branches) = schema.get(key).and_then(Value::as_array) {
            for b in branches {
                walk(root, b, value, pointer, out);
            }
        }
    }
    match value {
        Value::Object(map) => {
            let props = schema.get("properties").and_then(Value::as_object);
            let extra = schema.get("additionalProperties");
            if props.is_none() && extra.is_none() {
                return; // not an object schema (or a branch of anyOf)
            }
            for (k, v) in map {
                let ptr = format!("{pointer}/{}", escape(k));
                if let Some(sub) = props.and_then(|p| p.get(k)) {
                    walk(root, sub, v, &ptr, out);
                } else {
                    match extra {
                        Some(Value::Bool(true)) => {}
                        Some(sub @ Value::Object(_)) => walk(root, sub, v, &ptr, out),
                        _ => out.push(ptr),
                    }
                }
            }
        }
        Value::Array(items) => {
            if let Some(sub) = schema.get("items") {
                for (i, v) in items.iter().enumerate() {
                    walk(root, sub, v, &format!("{pointer}/{i}"), out);
                }
            }
        }
        _ => {}
    }
}
