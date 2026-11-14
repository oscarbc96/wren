//! Memory pressure response (macOS).
//!
//! Subscribes to `DISPATCH_SOURCE_TYPE_MEMORYPRESSURE` when `memory.pressure_response`
//! is enabled. On Warn: minimize engine memory then sleep the oldest background
//! tab that may sleep. On Critical: sleep every background tab that may sleep.
//! Tab sleep rules follow Task 2: never the active tab, never pinned, never audible.

use engine::Engine;
use tabs::TabStrip;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureLevel {
    Warn,
    Critical,
}

/// Source of memory pressure notifications.
pub trait PressureSource {
    /// Drains pending pressure levels since the last call.
    fn drain(&mut self) -> Vec<PressureLevel>;
}

/// Fake pressure source for tests.
#[derive(Default)]
pub struct FakePressureSource {
    pending: Vec<PressureLevel>,
}

impl FakePressureSource {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, level: PressureLevel) {
        self.pending.push(level);
    }
}

impl PressureSource for FakePressureSource {
    fn drain(&mut self) -> Vec<PressureLevel> {
        std::mem::take(&mut self.pending)
    }
}

/// Applies drained pressure events. Returns the number of tabs slept.
pub fn handle_pressure<E: Engine, P: PressureSource + ?Sized>(
    source: &mut P,
    engine: &mut E,
    strip: &mut TabStrip,
    enabled: bool,
) -> usize {
    if !enabled {
        let _ = source.drain();
        return 0;
    }
    let levels = source.drain();
    let mut slept = 0usize;
    for level in levels {
        match level {
            PressureLevel::Warn => {
                engine.minimize_memory();
                if let Some(id) = oldest_sleepable_background(strip) {
                    strip.sleep(engine, id);
                    slept += 1;
                }
            }
            PressureLevel::Critical => {
                while let Some(id) = oldest_sleepable_background(strip) {
                    strip.sleep(engine, id);
                    slept += 1;
                }
            }
        }
    }
    slept
}

fn oldest_sleepable_background(strip: &TabStrip) -> Option<tabs::TabId> {
    let active = strip.active();
    strip
        .tabs()
        .iter()
        .filter(|t| Some(t.id) != active)
        .filter(|t| !t.pinned && !t.audible)
        .filter(|t| t.state != tabs::TabState::Slept)
        .min_by_key(|t| t.last_active)
        .map(|t| t.id)
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{PressureLevel, PressureSource};
    use engine::memory_relief::macos::{
        self as ffi, DISPATCH_MEMORYPRESSURE_CRITICAL, DISPATCH_MEMORYPRESSURE_WARN,
        DISPATCH_QUEUE_PRIORITY_DEFAULT, DispatchObjectT, DispatchSourceT,
    };
    use std::sync::{Arc, Mutex};

    #[repr(C)]
    struct Ctx {
        shared: Arc<Mutex<Vec<PressureLevel>>>,
        source: DispatchSourceT,
    }

    pub struct DispatchPressureSource {
        source: DispatchSourceT,
        shared: Arc<Mutex<Vec<PressureLevel>>>,
        ctx: *mut Ctx,
    }

    unsafe impl Send for DispatchPressureSource {}

    impl DispatchPressureSource {
        pub fn new() -> Option<Self> {
            unsafe {
                let shared = Arc::new(Mutex::new(Vec::new()));
                let queue = ffi::dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0);
                if queue.is_null() {
                    return None;
                }
                let source = ffi::dispatch_source_create(
                    &ffi::_dispatch_source_type_memorypressure as *const _,
                    0,
                    DISPATCH_MEMORYPRESSURE_WARN | DISPATCH_MEMORYPRESSURE_CRITICAL,
                    queue,
                );
                if source.is_null() {
                    return None;
                }
                let ctx = Box::into_raw(Box::new(Ctx {
                    shared: shared.clone(),
                    source,
                }));
                ffi::dispatch_set_context(source as DispatchObjectT, ctx as *mut _);
                ffi::dispatch_source_set_event_handler_f(source, handler_flags);
                ffi::dispatch_resume(source as DispatchObjectT);
                Some(Self {
                    source,
                    shared,
                    ctx,
                })
            }
        }
    }

    unsafe extern "C" fn handler_flags(context: *mut std::ffi::c_void) {
        if context.is_null() {
            return;
        }
        let ctx = unsafe { &*(context as *const Ctx) };
        let data = unsafe { ffi::dispatch_source_get_data(ctx.source) };
        if let Ok(mut pending) = ctx.shared.lock() {
            if data & DISPATCH_MEMORYPRESSURE_CRITICAL != 0 {
                pending.push(PressureLevel::Critical);
            } else if data & DISPATCH_MEMORYPRESSURE_WARN != 0 {
                pending.push(PressureLevel::Warn);
            }
        }
    }

    impl PressureSource for DispatchPressureSource {
        fn drain(&mut self) -> Vec<PressureLevel> {
            match self.shared.lock() {
                Ok(mut p) => std::mem::take(&mut *p),
                Err(_) => Vec::new(),
            }
        }
    }

    impl Drop for DispatchPressureSource {
        fn drop(&mut self) {
            unsafe {
                ffi::dispatch_source_cancel(self.source);
                ffi::dispatch_release(self.source as DispatchObjectT);
                if !self.ctx.is_null() {
                    drop(Box::from_raw(self.ctx));
                    self.ctx = std::ptr::null_mut();
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::DispatchPressureSource;

#[cfg(not(target_os = "macos"))]
pub struct DispatchPressureSource;

#[cfg(not(target_os = "macos"))]
impl DispatchPressureSource {
    pub fn new() -> Option<Self> {
        None
    }
}

#[cfg(not(target_os = "macos"))]
impl PressureSource for DispatchPressureSource {
    fn drain(&mut self) -> Vec<PressureLevel> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::FakeEngine;
    use jiff::Timestamp;
    use tabs::TabState;
    use url::Url;

    #[test]
    fn pressure_warn_minimizes_then_sleeps() {
        let mut engine = FakeEngine::new();
        let mut strip = TabStrip::new(25);
        let u1 = Url::parse("https://a.example").unwrap();
        let u2 = Url::parse("https://b.example").unwrap();
        let u3 = Url::parse("https://c.example").unwrap();
        let t1 = strip.open(&mut engine, &u1, true);
        let t2 = strip.open(&mut engine, &u2, false);
        let t3 = strip.open(&mut engine, &u3, false);

        // Make t2 older than t3 so it is chosen first.
        if let Some(tab) = strip.tabs_mut().iter_mut().find(|t| t.id == t2) {
            tab.last_active = Timestamp::from_second(10).unwrap();
        }
        if let Some(tab) = strip.tabs_mut().iter_mut().find(|t| t.id == t3) {
            tab.last_active = Timestamp::from_second(20).unwrap();
        }

        let mut source = FakePressureSource::new();
        source.push(PressureLevel::Warn);
        let slept = handle_pressure(&mut source, &mut engine, &mut strip, true);

        assert_eq!(slept, 1);
        assert!(
            engine
                .calls()
                .iter()
                .any(|c| matches!(c, engine::fake::FakeCall::MinimizeMemory))
        );
        let state_t1 = strip
            .tabs()
            .iter()
            .find(|t| t.id == t1)
            .unwrap()
            .state
            .clone();
        let state_t2 = strip
            .tabs()
            .iter()
            .find(|t| t.id == t2)
            .unwrap()
            .state
            .clone();
        let state_t3 = strip
            .tabs()
            .iter()
            .find(|t| t.id == t3)
            .unwrap()
            .state
            .clone();
        assert_eq!(state_t1, TabState::Active);
        assert_eq!(state_t2, TabState::Slept);
        assert_eq!(state_t3, TabState::Background);
    }
}
