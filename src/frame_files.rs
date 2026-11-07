//! Reclaim the fixed image slots owned by one renderer, without directory traversal.
use std::{ffi::CString,fs,io,os::{fd::AsRawFd,unix::fs::OpenOptionsExt},path::Path};

/// Only the renderer's documented slots are unlinked. Unexpected entries are
/// retained, and symlinks are unlinked as entries rather than followed.
pub fn reclaim(directory:&Path)->io::Result<()> {
    let root=match fs::OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY|libc::O_NOFOLLOW|libc::O_CLOEXEC).open(directory){
        Ok(root)=>root,Err(error) if error.kind()==io::ErrorKind::NotFound=>return Ok(()),Err(error)=>return Err(error),
    };
    for slot in [0,1] {
        for extension in ["png","bmp","jpg"] {
            for suffix in ["",".next"] {
                let name=CString::new(format!("frame-{slot}.{extension}{suffix}")).unwrap();
                if unsafe{libc::unlinkat(root.as_raw_fd(),name.as_ptr(),0)}!=0 {
                    let error=io::Error::last_os_error();
                    if error.kind()!=io::ErrorKind::NotFound{return Err(error)}
                }
            }
        }
    }
    // rmdir never traverses an entry; unknown files/directories keep this root alive.
    fs::remove_dir(directory)
}
