use serde_json::json;

use crate::cdp_bridge::{CallResult, method_not_found, ok};

pub(crate) fn handle(method: &str) -> CallResult {
    match method {
        "Console.enable"
        | "Console.disable"
        | "Console.clearMessages"
        | "Log.enable"
        | "Log.disable" => ok(json!({})),
        _ => method_not_found(method),
    }
}
