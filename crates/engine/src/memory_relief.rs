//! macOS memory relief helpers.
//!
//! Servo 0.8 content processes use the system malloc on macOS, so we return
//! free pages with `malloc_zone_pressure_relief`. Content processes run a
//! periodic relief thread and also relieve on memory pressure events when
//! available. The browser process calls relief from minimize paths.

/// Ask the system allocator to return free pages to the OS.
#[cfg(target_os = "macos")]
pub fn relief_now() {
    unsafe extern "C" {
        fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
    }
    // Safety: zone NULL targets all zones; goal 0 asks to relieve as much as possible.
    unsafe {
        let _ = malloc_zone_pressure_relief(std::ptr::null_mut(), 0);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn relief_now() {}

/// Returns true when periodic/event relief is disabled for measurement.
pub fn relief_disabled() -> bool {
    std::env::var_os("WREN_NO_RELIEF").is_some()
}

/// Start periodic allocator relief and, on macOS, a dispatch memory pressure
/// handler that triggers relief immediately. No-op when `WREN_NO_RELIEF` is set.
pub fn start_relief_thread() {
    if relief_disabled() {
        return;
    }
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(30));
            relief_now();
        }
    });
    #[cfg(target_os = "macos")]
    {
        let _ = macos::start_pressure_relief_source();
    }
}

#[cfg(target_os = "macos")]
pub mod macos {
    use std::ffi::c_void;

    pub type DispatchSourceT = *mut c_void;
    pub type DispatchQueueT = *mut c_void;
    pub type DispatchObjectT = *mut c_void;

    #[link(name = "System", kind = "dylib")]
    unsafe extern "C" {
        pub static _dispatch_source_type_memorypressure: c_void;
        pub fn dispatch_get_global_queue(identifier: isize, flags: usize) -> DispatchQueueT;
        pub fn dispatch_source_create(
            type_: *const c_void,
            handle: usize,
            mask: usize,
            queue: DispatchQueueT,
        ) -> DispatchSourceT;
        pub fn dispatch_source_set_event_handler_f(
            source: DispatchSourceT,
            handler: unsafe extern "C" fn(*mut c_void),
        );
        pub fn dispatch_set_context(object: DispatchObjectT, context: *mut c_void);
        pub fn dispatch_source_get_data(source: DispatchSourceT) -> usize;
        pub fn dispatch_resume(object: DispatchObjectT);
        pub fn dispatch_source_cancel(object: DispatchObjectT);
        pub fn dispatch_release(object: DispatchObjectT);
    }

    pub const DISPATCH_QUEUE_PRIORITY_DEFAULT: isize = 0;
    pub const DISPATCH_MEMORYPRESSURE_WARN: usize = 0x02;
    pub const DISPATCH_MEMORYPRESSURE_CRITICAL: usize = 0x04;

    /// Installs a dispatch memory pressure handler that calls `relief_now` on
    /// warn/critical. The source is leaked intentionally to keep it alive for
    /// the process lifetime.
    pub fn start_pressure_relief_source() -> bool {
        unsafe {
            let queue = dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0);
            if queue.is_null() {
                return false;
            }
            let source = dispatch_source_create(
                &_dispatch_source_type_memorypressure as *const _,
                0,
                DISPATCH_MEMORYPRESSURE_WARN | DISPATCH_MEMORYPRESSURE_CRITICAL,
                queue,
            );
            if source.is_null() {
                return false;
            }
            dispatch_set_context(source as DispatchObjectT, source);
            dispatch_source_set_event_handler_f(source, relief_handler);
            dispatch_resume(source as DispatchObjectT);
            // Leak the source so it lives for the process; do not cancel/release.
            true
        }
    }

    unsafe extern "C" fn relief_handler(context: *mut c_void) {
        if context.is_null() {
            return;
        }
        let source = context as DispatchSourceT;
        let data = unsafe { dispatch_source_get_data(source) };
        if data & (DISPATCH_MEMORYPRESSURE_WARN | DISPATCH_MEMORYPRESSURE_CRITICAL) != 0 {
            super::relief_now();
        }
    }
}
