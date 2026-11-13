use engine::{EngineInput, Modifiers, MouseButton, content_process_token, to_servo_input};

#[test]
fn content_process_token_parses() {
    let args: Vec<String> = ["wren", "--content-process", "abc"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(content_process_token(&args), Some("abc".to_string()));

    let args: Vec<String> = ["wren"].into_iter().map(String::from).collect();
    assert_eq!(content_process_token(&args), None);

    let args: Vec<String> = ["wren", "--content-process"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(content_process_token(&args), None);
}

/// Manual steps to verify a killed content process only crashes its tab:
/// 1. Run: `cargo run -p wren --example servo_in_gpui -- --two --exit-after 30`
/// 2. In another shell, find child content processes: `pgrep -P <pid>`
/// 3. Kill one content process: `kill -9 <child-pid>`
/// 4. Expect `CRASHED` for one webview while the other webview keeps reporting events.
#[test]
#[ignore]
fn kill_one_content_process_keeps_others() {}

#[test]
fn mouse_down_scales_to_device_pixels() {
    let ev = EngineInput::MouseDown {
        button: MouseButton::Left,
        x: 10.0,
        y: 20.0,
    };
    let out = to_servo_input(&ev, 2.0).expect("input event");
    match out {
        servo::InputEvent::MouseButton(m) => {
            assert_eq!(m.action, servo::MouseButtonAction::Down);
            assert_eq!(m.button, servo::MouseButton::Primary);
            assert_eq!(
                m.point,
                servo::WebViewPoint::Device(servo::DevicePoint::new(20.0, 40.0))
            );
        }
        other => panic!("expected mouse button, got {other:?}"),
    }
}

#[test]
fn key_maps_named_and_char() {
    let ev = EngineInput::Key {
        down: true,
        key: "Enter".to_string(),
        code: "Enter".to_string(),
        modifiers: Modifiers {
            shift: false,
            ctrl: false,
            alt: false,
            cmd: false,
        },
    };
    let out = to_servo_input(&ev, 1.0).expect("input event");
    match out {
        servo::InputEvent::Keyboard(k) => {
            assert_eq!(k.event.state, servo::KeyState::Down);
            assert_eq!(k.event.key, servo::Key::Named(servo::NamedKey::Enter));
            assert_eq!(k.event.code, servo::Code::Enter);
        }
        other => panic!("expected keyboard, got {other:?}"),
    }

    let ev = EngineInput::Key {
        down: true,
        key: "a".to_string(),
        code: "KeyA".to_string(),
        modifiers: Modifiers {
            shift: true,
            ctrl: false,
            alt: false,
            cmd: false,
        },
    };
    let out = to_servo_input(&ev, 1.0).expect("input event");
    match out {
        servo::InputEvent::Keyboard(k) => {
            assert_eq!(k.event.key, servo::Key::Character("a".to_string()));
            assert_eq!(k.event.code, servo::Code::KeyA);
            assert!(k.event.modifiers.contains(servo::Modifiers::SHIFT));
        }
        other => panic!("expected keyboard, got {other:?}"),
    }
}

#[test]
fn wheel_maps_pixels() {
    let ev = EngineInput::Wheel {
        dx: 3.0,
        dy: 4.0,
        x: 10.0,
        y: 20.0,
    };
    let out = to_servo_input(&ev, 2.0).expect("input event");
    match out {
        servo::InputEvent::Wheel(w) => {
            assert_eq!(w.delta.x, 6.0);
            assert_eq!(w.delta.y, 8.0);
            assert_eq!(w.delta.z, 0.0);
            assert_eq!(w.delta.mode, servo::WheelMode::DeltaPixel);
            assert_eq!(
                w.point,
                servo::WebViewPoint::Device(servo::DevicePoint::new(20.0, 40.0))
            );
        }
        other => panic!("expected wheel, got {other:?}"),
    }
}
