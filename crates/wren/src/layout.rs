use engine::ViewRect;

/// Rectangle (in the Servo parent view's coordinates) for the web content.
///
/// `window_size` is the full window size in points, `titlebar_inset` is the
/// height of the system title bar that sits above the GPUI content area, and
/// `chrome_h` is the height of the GPUI browser chrome (tabs + toolbar).
pub fn content_rect(window_size: (f32, f32), titlebar_inset: f32, chrome_h: f32) -> ViewRect {
    let (w, h) = window_size;
    ViewRect {
        x: 0.0,
        y: chrome_h,
        width: w,
        height: (h - titlebar_inset - chrome_h).max(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_rect_starts_below_chrome() {
        let r = content_rect((800.0, 600.0), 28.0, 72.0);
        assert_eq!(r.y, 72.0);
    }

    #[test]
    fn content_rect_fills_width() {
        let r = content_rect((800.0, 600.0), 28.0, 72.0);
        assert_eq!(r.width, 800.0);
    }
}
