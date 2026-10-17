//! Foreign-owned input reader. The release callback owns cleanup even on errors.
use std::{
    ffi::c_void,
    io::{self, Read},
};
pub type ReadCallback = unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> isize;
pub type ReleaseCallback = unsafe extern "C" fn(*mut c_void);
pub type ImageCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize);
pub(crate) struct ForeignReader {
    pub read: ReadCallback,
    pub release: ReleaseCallback,
    // The caller guarantees its context/callback can move to the HTTP body worker.
    pub context: usize,
}
impl Read for ForeignReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let n = unsafe {
            (self.read)(
                self.context as *mut c_void,
                buffer.as_mut_ptr(),
                buffer.len(),
            )
        };
        if n < 0 {
            return Err(io::Error::other("host_read_failed"));
        }
        if n as usize > buffer.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid_host_read",
            ));
        }
        Ok(n as usize)
    }
}
impl Drop for ForeignReader {
    fn drop(&mut self) {
        unsafe { (self.release)(self.context as *mut c_void) }
    }
}
