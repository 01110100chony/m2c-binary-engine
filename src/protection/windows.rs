use std::io;
use std::path::Path;

#[cfg(windows)]
mod platform {
    use super::*;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1, SE_FILE_OBJECT, SetNamedSecurityInfoW,
    };
    use windows_sys::Win32::Security::{
        DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, GetTokenInformation,
        PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, TOKEN_QUERY, TOKEN_USER,
        TokenUser,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        GetDriveTypeW, GetVolumeInformationW, GetVolumePathNameW,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows_sys::Win32::System::WindowsProgramming::DRIVE_FIXED;

    const PATH_BUFFER_U16: usize = 32_768;

    fn wide_null(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(Some(0)).collect()
    }

    fn terminated(buffer: &[u16]) -> &[u16] {
        let length = buffer
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(buffer.len());
        &buffer[..length]
    }

    pub(crate) fn is_supported_ntfs(path: &Path) -> bool {
        let path = wide_null(path.as_os_str());
        let mut root = vec![0_u16; PATH_BUFFER_U16];
        // SAFETY: both pointers reference writable/readable NUL-terminated UTF-16
        // buffers for the duration of the Windows calls.
        if unsafe {
            GetVolumePathNameW(
                path.as_ptr(),
                root.as_mut_ptr(),
                u32::try_from(root.len()).unwrap_or(u32::MAX),
            )
        } == 0
        {
            return false;
        }
        if unsafe { GetDriveTypeW(root.as_ptr()) } != DRIVE_FIXED {
            return false;
        }
        let mut filesystem = [0_u16; 64];
        if unsafe {
            GetVolumeInformationW(
                root.as_ptr(),
                null_mut(),
                0,
                null_mut(),
                null_mut(),
                null_mut(),
                filesystem.as_mut_ptr(),
                filesystem.len() as u32,
            )
        } == 0
        {
            return false;
        }
        terminated(&filesystem) == [b'N' as u16, b'T' as u16, b'F' as u16, b'S' as u16]
    }

    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: this handle was returned by OpenProcessToken and is owned here.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    fn current_user_sid() -> io::Result<String> {
        let mut token = null_mut();
        // SAFETY: token points to writable storage and the process pseudo-handle is valid.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = OwnedHandle(token);
        let mut byte_length = 0_u32;
        // The first call is expected to fail while reporting the necessary size.
        unsafe {
            GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut byte_length);
        }
        if byte_length == 0 {
            return Err(io::Error::last_os_error());
        }
        let words = usize::try_from(byte_length)
            .ok()
            .and_then(|bytes| bytes.checked_add(size_of::<usize>() - 1))
            .map(|bytes| bytes / size_of::<usize>())
            .ok_or_else(|| io::Error::other("token information size overflow"))?;
        let mut aligned = vec![0_usize; words];
        // SAFETY: aligned provides byte_length writable bytes with TOKEN_USER alignment.
        if unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                aligned.as_mut_ptr().cast(),
                byte_length,
                &mut byte_length,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: GetTokenInformation initialized a TOKEN_USER at this aligned address.
        let user = unsafe { &*aligned.as_ptr().cast::<TOKEN_USER>() };
        let mut sid_string = null_mut();
        // SAFETY: the SID belongs to the live token-information buffer and output is writable.
        if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid_string) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut length = 0_usize;
        // SAFETY: ConvertSidToStringSidW returns a NUL-terminated LocalAlloc string.
        unsafe {
            while *sid_string.add(length) != 0 {
                length += 1;
            }
        }
        // SAFETY: the pointer is valid for length UTF-16 units as established above.
        let sid = String::from_utf16(unsafe { std::slice::from_raw_parts(sid_string, length) })
            .map_err(|_| io::Error::other("Windows returned a non-UTF-16 SID"));
        // SAFETY: this allocation is documented to be released with LocalFree.
        unsafe {
            LocalFree(sid_string.cast());
        }
        sid
    }

    pub(crate) fn restrict_permissions(path: &Path) -> io::Result<()> {
        let sid = current_user_sid()?;
        let descriptor_text = format!("D:P(A;;FA;;;SY)(A;;FA;;;{sid})");
        let descriptor_text = wide_null(OsStr::new(&descriptor_text));
        let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
        // SAFETY: input is NUL-terminated and output storage is valid.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                descriptor_text.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut present = 0;
        let mut defaulted = 0;
        let mut dacl = null_mut();
        // SAFETY: descriptor is valid until LocalFree below and all outputs are writable.
        let dacl_result = unsafe {
            GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted)
        };
        let result = if dacl_result == 0 || present == 0 || dacl.is_null() {
            Err(io::Error::last_os_error())
        } else {
            let path = wide_null(path.as_os_str());
            // SAFETY: path is NUL-terminated and dacl belongs to the live descriptor.
            let status = unsafe {
                SetNamedSecurityInfoW(
                    path.as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    dacl,
                    null(),
                )
            };
            if status == 0 {
                Ok(())
            } else {
                Err(io::Error::from_raw_os_error(status as i32))
            }
        };
        // SAFETY: descriptor was allocated by the SDDL conversion function.
        unsafe {
            LocalFree(descriptor.cast());
        }
        result
    }
}

#[cfg(windows)]
pub(crate) use platform::{is_supported_ntfs, restrict_permissions};

#[cfg(not(windows))]
pub(crate) fn is_supported_ntfs(_path: &Path) -> bool {
    false
}

#[cfg(not(windows))]
pub(crate) fn restrict_permissions(_path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "M5 permissions require Windows/MSVC on local NTFS",
    ))
}
