//! In-memory Emscripten adapter. No file system, process spawning, or CLI parsing.
use assfonts_ass::AssCodec;
use assfonts_core::Processor;
use assfonts_fonts::FontCatalog;
use assfonts_harfbuzz::HarfBuzz;
use std::{
    alloc::{alloc, dealloc, Layout},
    slice, str,
};

struct Engine {
    catalog: FontCatalog,
    backend: HarfBuzz,
    output: Vec<u8>,
}

impl Engine {
    fn reply(&mut self, value: serde_json::Value, ok: bool) -> i32 {
        self.output = serde_json::to_vec(&value).expect("serializable response");
        i32::from(ok)
    }
    fn error(&mut self, error: impl std::fmt::Display) -> i32 {
        self.reply(serde_json::json!({"error": error.to_string()}), false)
    }
}

#[no_mangle]
pub extern "C" fn af_alloc(len: usize) -> *mut u8 {
    let Ok(layout) = Layout::array::<u8>(len.max(1)) else {
        return std::ptr::null_mut();
    };
    unsafe { alloc(layout) }
}

/// Releases a caller-owned input buffer.
///
/// # Safety
/// `ptr` must be null or a live allocation returned by `af_alloc(len)` in this
/// instance, with the exact original length. It must not be freed twice.
#[no_mangle]
pub unsafe extern "C" fn af_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        dealloc(ptr, Layout::array::<u8>(len.max(1)).unwrap());
    }
}

#[no_mangle]
pub extern "C" fn af_engine_new() -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(Engine {
        catalog: FontCatalog::default(),
        backend: HarfBuzz::default(),
        output: Vec::new(),
    }))
    .cast()
}

/// Destroys a catalog and its last response.
///
/// # Safety
/// `ptr` must be null or a live engine from this instance, without concurrent
/// users or outstanding borrowed response views. Do not destroy it twice.
#[no_mangle]
pub unsafe extern "C" fn af_engine_destroy(ptr: *mut std::ffi::c_void) {
    if !ptr.is_null() {
        drop(Box::from_raw(ptr.cast::<Engine>()));
    }
}

/// Copies a font and its UTF-8 source label into the engine's catalog.
///
/// # Safety
/// `ptr` must identify a live engine, used exclusively during this call. Both
/// byte ranges must be non-null and readable for their specified lengths,
/// including when the length is zero. Neither range may alias engine storage.
#[no_mangle]
pub unsafe extern "C" fn af_add_font(
    ptr: *mut std::ffi::c_void,
    label: *const u8,
    label_len: usize,
    data: *const u8,
    len: usize,
) -> i32 {
    let engine = &mut *ptr.cast::<Engine>();
    let label = match str::from_utf8(slice::from_raw_parts(label, label_len)) {
        Ok(value) => value,
        Err(error) => return engine.error(error),
    };
    match engine.catalog.add(label, slice::from_raw_parts(data, len)) {
        Ok(()) => engine.reply(
            serde_json::json!({"faces": engine.catalog.face_count()}),
            true,
        ),
        Err(error) => engine.error(error),
    }
}

/// Processes UTF-8 ASS using the supplied fonts, returning 1 or an error (0).
///
/// # Safety
/// `ptr` must identify a live engine, used exclusively during this call. `data`
/// must be non-null and readable for `len` bytes, including a zero-length range.
/// The input must not alias engine storage. The previous response is invalidated.
#[no_mangle]
pub unsafe extern "C" fn af_process(
    ptr: *mut std::ffi::c_void,
    data: *const u8,
    len: usize,
) -> i32 {
    let engine = &mut *ptr.cast::<Engine>();
    let text = match str::from_utf8(slice::from_raw_parts(data, len)) {
        Ok(value) => value,
        Err(error) => return engine.error(error),
    };
    let processor = Processor {
        codec: &AssCodec,
        resolver: &engine.catalog,
        subsetter: &engine.backend,
    };
    match processor.process(text) {
        Ok(processed) => engine.reply(
            serde_json::json!({
                "subtitle": processed.subtitle, "report": processed.report,
            }),
            true,
        ),
        Err(error) => engine.error(error),
    }
}

/// Borrows the last UTF-8 JSON response; copy it before mutating the engine.
///
/// # Safety
/// `ptr` must be a live engine, without concurrent mutation. The returned range
/// has `af_result_len(ptr)` bytes and is invalidated by mutation or destruction.
#[no_mangle]
pub unsafe extern "C" fn af_result_ptr(ptr: *mut std::ffi::c_void) -> *const u8 {
    (&*ptr.cast::<Engine>()).output.as_ptr()
}

/// Returns the length of the last response.
///
/// # Safety
/// `ptr` must be a live engine, without concurrent mutation or destruction.
#[no_mangle]
pub unsafe extern "C" fn af_result_len(ptr: *mut std::ffi::c_void) -> usize {
    (&*ptr.cast::<Engine>()).output.len()
}

fn main() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_preserves_input_and_recovers_from_errors() {
        let engine = af_engine_new();
        let subtitle = include_bytes!("../../../examples/basic.ass");
        let font = include_bytes!("../../../vendor/harfbuzz/test/api/fonts/OpenSans-Regular.ttf");
        let label = b"OpenSans-Regular.ttf";
        // All pointers refer to live input arrays or this engine. Responses are
        // decoded before the next mutation; font inputs are copied by the catalog.
        unsafe {
            assert_eq!(af_process(engine, [0xff].as_ptr(), 1), 0);
            let error = str::from_utf8(slice::from_raw_parts(
                af_result_ptr(engine),
                af_result_len(engine),
            ))
            .unwrap();
            assert!(error.contains("utf-8"));
            assert_eq!(af_process(engine, subtitle.as_ptr(), subtitle.len()), 0);
            assert_eq!(
                af_add_font(engine, label.as_ptr(), label.len(), [0].as_ptr(), 1),
                0
            );
            assert_eq!(
                af_add_font(
                    engine,
                    label.as_ptr(),
                    label.len(),
                    font.as_ptr(),
                    font.len()
                ),
                1
            );
            assert_eq!(af_process(engine, subtitle.as_ptr(), subtitle.len()), 1);
            let response: serde_json::Value = serde_json::from_slice(slice::from_raw_parts(
                af_result_ptr(engine),
                af_result_len(engine),
            ))
            .unwrap();
            let output = response["subtitle"].as_str().unwrap();
            let restored = format!(
                "{}[Events]{}",
                output.split("[Fonts]").next().unwrap(),
                output.split("[Events]").nth(1).unwrap()
            );
            assert_eq!(restored.as_bytes(), subtitle);
            assert_eq!(response["report"]["fonts"].as_array().unwrap().len(), 1);
            af_engine_destroy(engine);
        }
    }

    #[test]
    fn input_buffers_support_empty_payloads() {
        for len in [0, 1, 4096] {
            let ptr = af_alloc(len);
            assert!(!ptr.is_null());
            // The allocation has at least max(len, 1) bytes and is freed once
            // with the exact length originally passed to af_alloc.
            unsafe {
                ptr.write(42);
                af_free(ptr, len);
            }
        }
    }
}
