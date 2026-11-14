use serde_json::{Value, json};

use crate::cdp_bridge::{CallResult, method_not_found, ok};

pub(crate) fn handle(method: &str, _params: &Value) -> CallResult {
    match method {
        // Emulation
        "Emulation.setDeviceMetricsOverride"
        | "Emulation.clearDeviceMetricsOverride"
        | "Emulation.setEmulatedMedia"
        | "Emulation.setFocusEmulationEnabled"
        | "Emulation.enable"
        | "Emulation.disable"
        // CSS, Debugger, Performance, Overlay, Profiler, HeapProfiler, Security,
        // ServiceWorker, Inspector, Audits, Log
        | "CSS.enable"
        | "CSS.disable"
        | "Debugger.enable"
        | "Debugger.disable"
        | "Debugger.setPauseOnExceptions"
        | "Debugger.setAsyncCallStackDepth"
        | "Performance.enable"
        | "Performance.disable"
        | "Overlay.enable"
        | "Overlay.disable"
        | "Profiler.enable"
        | "Profiler.disable"
        | "HeapProfiler.enable"
        | "HeapProfiler.disable"
        | "Security.enable"
        | "Security.disable"
        | "Security.setIgnoreCertificateErrors"
        | "ServiceWorker.enable"
        | "ServiceWorker.disable"
        | "Inspector.enable"
        | "Inspector.disable"
        | "Audits.enable"
        | "Audits.disable"
        | "Log.enable"
        | "Log.disable"
        | "Log.clear"
        // Network setters often treated as stubs here too
        | "Network.emulateNetworkConditions" => ok(json!({})),
        _ => method_not_found(method),
    }
}
