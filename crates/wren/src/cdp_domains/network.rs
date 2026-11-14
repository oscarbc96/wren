use serde_json::{Value, json};

use crate::cdp_bridge::{CallResult, method_not_found, ok};

pub(crate) fn handle(method: &str, _params: &Value) -> CallResult {
    match method {
        "Network.enable" | "Network.disable" => ok(json!({})),
        "Network.setCacheDisabled"
        | "Network.setExtraHTTPHeaders"
        | "Network.setUserAgentOverride"
        | "Network.setRequestInterception" => ok(json!({})),
        "Network.getResponseBody" => crate::cdp_bridge::err(-32000, "not available"),
        _ => method_not_found(method),
    }
}
