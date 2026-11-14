use engine::{Engine, EngineInput, Modifiers, MouseButton};
use serde_json::{Value, json};
use tabs::Tab;

use crate::cdp_bridge::{CallResult, ok};

pub(crate) fn handle(
    engine: &mut dyn Engine,
    tab: &Tab,
    method: &str,
    params: &Value,
) -> CallResult {
    match method {
        "Input.dispatchMouseEvent" => {
            let typ = params.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let x = params.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let y = params.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let button = match params
                .get("button")
                .and_then(|v| v.as_str())
                .unwrap_or("left")
            {
                "right" => MouseButton::Right,
                "middle" => MouseButton::Middle,
                _ => MouseButton::Left,
            };
            match typ {
                "mouseMoved" => engine.input(tab.webview, EngineInput::MouseMove { x, y }),
                "mousePressed" => {
                    engine.input(tab.webview, EngineInput::MouseDown { button, x, y })
                }
                "mouseReleased" => engine.input(tab.webview, EngineInput::MouseUp { button, x, y }),
                "mouseWheel" => {
                    let dx = params.get("deltaX").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let dy = params.get("deltaY").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    engine.input(tab.webview, EngineInput::Wheel { dx, dy, x, y });
                }
                _ => {}
            }
            ok(json!({}))
        }
        "Input.dispatchKeyEvent" => {
            let typ = params.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let key = params
                .get("key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let code = params
                .get("code")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let text = params
                .get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let mods = Modifiers {
                shift: params
                    .get("modifiers")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
                    & 8
                    != 0,
                ctrl: params
                    .get("modifiers")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
                    & 2
                    != 0,
                alt: params
                    .get("modifiers")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
                    & 1
                    != 0,
                cmd: params
                    .get("modifiers")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
                    & 4
                    != 0,
            };
            match typ {
                "keyDown" | "rawKeyDown" => {
                    engine.input(
                        tab.webview,
                        EngineInput::Key {
                            down: true,
                            key: key.clone(),
                            code: code.clone(),
                            modifiers: mods,
                        },
                    );
                    if let Some(t) = text.or_else(|| {
                        if key.len() == 1 {
                            Some(key.clone())
                        } else {
                            None
                        }
                    }) {
                        // char events approximated as key? Engine has no char; try Key down of text chars
                        let _ = t;
                    }
                }
                "keyUp" => {
                    engine.input(
                        tab.webview,
                        EngineInput::Key {
                            down: false,
                            key,
                            code,
                            modifiers: mods,
                        },
                    );
                }
                "char" => {
                    let ch = text.unwrap_or(key);
                    for c in ch.chars() {
                        let s = c.to_string();
                        engine.input(
                            tab.webview,
                            EngineInput::Key {
                                down: true,
                                key: s.clone(),
                                code: String::new(),
                                modifiers: Modifiers::default(),
                            },
                        );
                        engine.input(
                            tab.webview,
                            EngineInput::Key {
                                down: false,
                                key: s,
                                code: String::new(),
                                modifiers: Modifiers::default(),
                            },
                        );
                    }
                }
                _ => {}
            }
            ok(json!({}))
        }
        "Input.insertText" => {
            let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
            for c in text.chars() {
                let s = c.to_string();
                engine.input(
                    tab.webview,
                    EngineInput::Key {
                        down: true,
                        key: s.clone(),
                        code: String::new(),
                        modifiers: Modifiers::default(),
                    },
                );
                engine.input(
                    tab.webview,
                    EngineInput::Key {
                        down: false,
                        key: s,
                        code: String::new(),
                        modifiers: Modifiers::default(),
                    },
                );
            }
            ok(json!({}))
        }
        _ => crate::cdp_bridge::method_not_found(method),
    }
}
