use std::path::Path;

use anyhow::{Context, Result};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::window::{
    create_image, kCGWindowImageBestResolution, kCGWindowListOptionIncludingWindow,
};
use foreign_types::ForeignType;
use objc2::rc::Retained;
use objc2_app_kit::NSView;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFURLCreateWithFileSystemPath(
        alloc: *const std::ffi::c_void,
        filePath: *const std::ffi::c_void,
        pathStyle: i32,
        isDirectory: bool,
    ) -> *const std::ffi::c_void;
    fn CFRelease(cf: *const std::ffi::c_void);
    fn CFStringCreateWithCString(
        alloc: *const std::ffi::c_void,
        cstr: *const std::ffi::c_char,
        encoding: u32,
    ) -> *const std::ffi::c_void;
}

#[link(name = "ImageIO", kind = "framework")]
unsafe extern "C" {
    fn CGImageDestinationCreateWithURL(
        url: *const std::ffi::c_void,
        typ: *const std::ffi::c_void,
        count: usize,
        options: *const std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
    fn CGImageDestinationAddImage(
        dest: *mut std::ffi::c_void,
        image: *const std::ffi::c_void,
        properties: *const std::ffi::c_void,
    );
    fn CGImageDestinationFinalize(dest: *mut std::ffi::c_void) -> bool;
}

const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;
const K_CF_URL_POSIX_PATH_STYLE: i32 = 0;
const K_CF_ALLOCATOR_DEFAULT: *const std::ffi::c_void = std::ptr::null();

pub fn window_number_from_view(view: *mut std::ffi::c_void) -> Option<i32> {
    if view.is_null() {
        return None;
    }
    let view = view as *mut NSView;
    let view = unsafe { Retained::retain(view)? };
    let window = view.window()?;
    Some(window.windowNumber() as i32)
}

pub fn capture_window_png(window_number: i32, out: &Path) -> Result<()> {
    // Primary: CGWindowListCreateImage.
    let bounds = CGRect {
        origin: CGPoint {
            x: f64::INFINITY,
            y: f64::INFINITY,
        },
        size: CGSize {
            width: 0.0,
            height: 0.0,
        },
    };
    if let Some(image) = create_image(
        bounds,
        kCGWindowListOptionIncludingWindow,
        window_number as u32,
        kCGWindowImageBestResolution,
    ) {
        let ptr = image.as_ptr() as *const std::ffi::c_void;
        if write_cgimage_png(ptr, out).is_ok() {
            return Ok(());
        }
    }
    // Fallback: screencapture.
    let status = std::process::Command::new("screencapture")
        .arg("-x")
        .arg("-o")
        .arg(format!("-l{}", window_number))
        .arg(out)
        .status()
        .context("run screencapture")?;
    if status.success() && out.exists() {
        Ok(())
    } else {
        anyhow::bail!("screencapture failed")
    }
}

fn write_cgimage_png(image: *const std::ffi::c_void, out: &Path) -> Result<()> {
    let path_str = out.to_string_lossy();
    let cpath = std::ffi::CString::new(path_str.as_bytes()).context("path cstr")?;
    let cf_path = unsafe {
        CFStringCreateWithCString(
            K_CF_ALLOCATOR_DEFAULT,
            cpath.as_ptr(),
            K_CF_STRING_ENCODING_UTF8,
        )
    };
    if cf_path.is_null() {
        anyhow::bail!("cf string");
    }
    let url = unsafe {
        CFURLCreateWithFileSystemPath(
            K_CF_ALLOCATOR_DEFAULT,
            cf_path,
            K_CF_URL_POSIX_PATH_STYLE,
            false,
        )
    };
    unsafe { CFRelease(cf_path) };
    if url.is_null() {
        anyhow::bail!("cf url");
    }

    let png_type = std::ffi::CString::new("public.png").unwrap();
    let cf_type = unsafe {
        CFStringCreateWithCString(
            K_CF_ALLOCATOR_DEFAULT,
            png_type.as_ptr(),
            K_CF_STRING_ENCODING_UTF8,
        )
    };
    let dest = unsafe { CGImageDestinationCreateWithURL(url, cf_type, 1, std::ptr::null()) };
    unsafe { CFRelease(cf_type) };
    unsafe { CFRelease(url) };
    if dest.is_null() {
        anyhow::bail!("dest null");
    }
    unsafe { CGImageDestinationAddImage(dest, image, std::ptr::null()) };
    let ok = unsafe { CGImageDestinationFinalize(dest) };
    // dest is CFType; release via CFRelease
    unsafe { CFRelease(dest) };
    if ok {
        Ok(())
    } else {
        anyhow::bail!("finalize failed")
    }
}
