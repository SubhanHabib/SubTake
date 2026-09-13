//! An owned, thread-confined native decoder behind a small C ABI.
use anyhow::{Context, Result, ensure};
use std::{
    ffi::{CStr, CString, c_char, c_void},
    path::Path,
    ptr::NonNull,
};
unsafe extern "C" {
    fn subtake_decoder_open(
        path: *const c_char,
        width: i32,
        height: i32,
        error: *mut c_char,
        capacity: usize,
    ) -> *mut c_void;
    fn subtake_decoder_frame(
        decoder: *mut c_void,
        seconds: f64,
        rgba: *mut u8,
        length: usize,
        error: *mut c_char,
        capacity: usize,
    ) -> i32;
    fn subtake_decoder_close(decoder: *mut c_void);
}
pub struct NativeDecoder {
    handle: NonNull<c_void>,
    width: u32,
    height: u32,
}
// The handle is exclusively owned; FFmpeg contexts are never accessed concurrently.
unsafe impl Send for NativeDecoder {}
impl NativeDecoder {
    pub fn open(path: &Path, width: u32, height: u32) -> Result<Self> {
        ensure!(
            width > 0 && height > 0 && width <= 8192 && height <= 8192,
            "Unsupported decode dimensions"
        );
        let path = CString::new(
            path.to_str()
                .context("Media path is not valid Unicode")?
                .as_bytes(),
        )
        .context("Media path contains a NUL byte")?;
        let mut error = [0 as c_char; 1024];
        // SAFETY: live NUL-terminated path/error buffers; returned context is owned below.
        let handle = unsafe {
            subtake_decoder_open(
                path.as_ptr(),
                width as i32,
                height as i32,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        let handle = NonNull::new(handle).with_context(|| {
            format!(
                "Native decoder: {}",
                unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy()
            )
        })?;
        Ok(Self {
            handle,
            width,
            height,
        })
    }
    pub fn frame(&mut self, seconds: f64) -> Result<Vec<u8>> {
        ensure!(
            seconds.is_finite() && seconds >= 0.,
            "Invalid decode timestamp"
        );
        let mut rgba = vec![0; self.width as usize * self.height as usize * 4];
        let mut error = [0 as c_char; 1024];
        // SAFETY: context is exclusively borrowed and both output buffers have the supplied sizes.
        let result = unsafe {
            subtake_decoder_frame(
                self.handle.as_ptr(),
                seconds,
                rgba.as_mut_ptr(),
                rgba.len(),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        ensure!(
            result == 0,
            "Native decoder: {}",
            unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy()
        );
        Ok(rgba)
    }
}
impl Drop for NativeDecoder {
    fn drop(&mut self) {
        unsafe {
            subtake_decoder_close(self.handle.as_ptr());
        }
    }
}
