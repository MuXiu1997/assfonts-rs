//! Statically linked HarfBuzz adapter; the only Rust crate containing FFI.
#![deny(unsafe_op_in_unsafe_fn)]

use assfonts_core::{Error, FontFace, Result, Subsetter};
use assfonts_fonts::verify_coverage;
use std::{
    collections::BTreeSet,
    ffi::{c_char, c_void, CStr},
    ptr::NonNull,
};

unsafe extern "C" {
    fn af_subset(
        bytes: *const c_char,
        length: u32,
        index: u32,
        unicodes: *const u32,
        count: u32,
    ) -> *mut c_void;
    fn af_blob_data(blob: *mut c_void, length: *mut u32) -> *const c_char;
    fn af_blob_destroy(blob: *mut c_void);
    fn af_version() -> *const c_char;
}

struct Blob(NonNull<c_void>);
impl Drop for Blob {
    fn drop(&mut self) {
        // SAFETY: exclusively owned output from af_subset, destroyed exactly once.
        unsafe { af_blob_destroy(self.0.as_ptr()) };
    }
}

pub fn version() -> &'static str {
    // SAFETY: HarfBuzz returns a non-null, static NUL-terminated ASCII string.
    unsafe { CStr::from_ptr(af_version()) }
        .to_str()
        .expect("HarfBuzz version is ASCII")
}

pub struct HarfBuzz {
    name: String,
}
impl Default for HarfBuzz {
    fn default() -> Self {
        Self {
            name: format!("harfbuzz/{} (static)", version()),
        }
    }
}

impl Subsetter for HarfBuzz {
    fn name(&self) -> &str {
        &self.name
    }
    fn subset(&self, face: &FontFace, characters: &BTreeSet<char>) -> Result<Vec<u8>> {
        verify_coverage(&face.data, face.index, characters)?;
        let length = u32::try_from(face.data.len())
            .map_err(|_| Error::Subset("font larger than 4 GiB".into()))?;
        let unicodes: Vec<u32> = characters.iter().map(|&c| c as u32).collect();
        let count = u32::try_from(unicodes.len())
            .map_err(|_| Error::Subset("too many codepoints".into()))?;
        // SAFETY: slices remain alive during this synchronous call; lengths are
        // checked, codepoints are sorted by BTreeSet, C++ does not retain inputs.
        let ptr = unsafe {
            af_subset(
                face.data.as_ptr().cast(),
                length,
                face.index,
                unicodes.as_ptr(),
                count,
            )
        };
        let blob = Blob(NonNull::new(ptr).ok_or_else(|| {
            Error::Subset(format!(
                "HarfBuzz rejected {} face {}",
                face.source, face.index
            ))
        })?);
        let mut output_len = 0;
        // SAFETY: blob is live, output_len is a valid mutable u32.
        let data = unsafe { af_blob_data(blob.0.as_ptr(), &mut output_len) };
        if data.is_null() || output_len == 0 {
            return Err(Error::Subset("empty output blob".into()));
        }
        // SAFETY: HarfBuzz guarantees this range until the blob is destroyed;
        // copy into Rust-owned memory while the RAII owner is still alive.
        let output =
            unsafe { std::slice::from_raw_parts(data.cast::<u8>(), output_len as usize) }.to_vec();
        verify_coverage(&output, 0, characters)
            .map_err(|e| Error::Subset(format!("output coverage: {e}")))?;
        Ok(output)
    }
}
