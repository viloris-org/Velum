use std::ptr;

use velum_client_profile::{ClientProfile, ProfileErrorKind};

use crate::{
    PROFILE_ABI_VERSION, VelumByteSlice, VelumMutableByteSlice, VelumProfileStatus,
    configuration::copy_bytes,
};

/// Returns the additive native profile ABI version.
#[unsafe(no_mangle)]
pub extern "C" fn velum_client_profile_abi_version() -> u16 {
    PROFILE_ABI_VERSION
}

/// Validates and normalizes a profile, returning the required canonical size.
///
/// # Safety
///
/// `input` must be readable and `out_required` writable for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_profile_validate_v1(
    input: VelumByteSlice,
    out_required: *mut usize,
) -> VelumProfileStatus {
    if out_required.is_null() {
        return VelumProfileStatus::InvalidArgument;
    }
    let canonical = match unsafe { canonical_profile(input) } {
        Ok(canonical) => canonical,
        Err(status) => return status,
    };
    unsafe { *out_required = canonical.len() };
    VelumProfileStatus::Ok
}

/// Writes canonical, stable-ID-normalized YAML into a caller-owned buffer.
///
/// `out_written` always receives the required size after successful parsing.
///
/// # Safety
///
/// `input` must be readable. `output` and `out_written` must be writable for
/// the duration of this call when their lengths are nonzero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_profile_normalize_v1(
    input: VelumByteSlice,
    output: VelumMutableByteSlice,
    out_written: *mut usize,
) -> VelumProfileStatus {
    if out_written.is_null() {
        return VelumProfileStatus::InvalidArgument;
    }
    let canonical = match unsafe { canonical_profile(input) } {
        Ok(canonical) => canonical,
        Err(status) => return status,
    };
    unsafe { *out_written = canonical.len() };
    if output.length < canonical.len() {
        return VelumProfileStatus::BufferTooSmall;
    }
    if canonical.is_empty() {
        return VelumProfileStatus::Ok;
    }
    if output.pointer.is_null() {
        return VelumProfileStatus::InvalidArgument;
    }
    unsafe {
        ptr::copy_nonoverlapping(canonical.as_ptr(), output.pointer, canonical.len());
    }
    VelumProfileStatus::Ok
}

unsafe fn canonical_profile(input: VelumByteSlice) -> Result<Vec<u8>, VelumProfileStatus> {
    let source = unsafe { copy_bytes(input) }.map_err(|_| VelumProfileStatus::InvalidArgument)?;
    let profile = ClientProfile::from_yaml(&source).map_err(status_for_profile)?;
    profile
        .to_canonical_yaml()
        .map(String::into_bytes)
        .map_err(|_| VelumProfileStatus::Internal)
}

pub(crate) fn status_for_profile(error: velum_client_profile::ProfileError) -> VelumProfileStatus {
    match error.kind() {
        ProfileErrorKind::Syntax => VelumProfileStatus::Syntax,
        ProfileErrorKind::UnsupportedVersion => VelumProfileStatus::UnsupportedVersion,
        ProfileErrorKind::Limit => VelumProfileStatus::Limit,
        ProfileErrorKind::Validation => VelumProfileStatus::Validation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &[u8] =
        include_bytes!("../../velum-client-profile/tests/fixtures/valid-profile.yaml");

    fn input(bytes: &[u8]) -> VelumByteSlice {
        VelumByteSlice {
            pointer: bytes.as_ptr(),
            length: bytes.len(),
        }
    }

    #[test]
    fn validates_then_writes_canonical_profile() {
        let mut required = 0;
        assert_eq!(
            unsafe { velum_client_profile_validate_v1(input(PROFILE), &mut required) },
            VelumProfileStatus::Ok
        );
        let mut output = vec![0; required];
        let mut written = 0;
        assert_eq!(
            unsafe {
                velum_client_profile_normalize_v1(
                    input(PROFILE),
                    VelumMutableByteSlice {
                        pointer: output.as_mut_ptr(),
                        length: output.len(),
                    },
                    &mut written,
                )
            },
            VelumProfileStatus::Ok
        );
        assert_eq!(written, required);
        assert!(
            String::from_utf8(output)
                .expect("UTF-8")
                .contains("target: node-sg")
        );
    }

    #[test]
    fn reports_required_size_without_partial_output() {
        let mut output = [0xaa; 4];
        let mut required = 0;
        assert_eq!(
            unsafe {
                velum_client_profile_normalize_v1(
                    input(PROFILE),
                    VelumMutableByteSlice {
                        pointer: output.as_mut_ptr(),
                        length: output.len(),
                    },
                    &mut required,
                )
            },
            VelumProfileStatus::BufferTooSmall
        );
        assert!(required > output.len());
        assert_eq!(output, [0xaa; 4]);
    }
}
