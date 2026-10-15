use std::{ffi::CString, mem::MaybeUninit, os::unix::ffi::OsStrExt, path::Path};

/// (total, available) bytes of the volume containing `path`.
pub fn usage(path: &Path) -> (u64, u64) {
    let Ok(c) = CString::new(path.as_os_str().as_bytes()) else { return (0, 0) };
    let mut st = MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::statfs(c.as_ptr(), st.as_mut_ptr()) } != 0 {
        return (0, 0);
    }
    let st = unsafe { st.assume_init() };
    let bs = st.f_bsize as u64;
    (st.f_blocks as u64 * bs, st.f_bavail as u64 * bs)
}
