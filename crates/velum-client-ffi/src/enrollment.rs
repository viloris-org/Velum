use std::ptr;

use velum_client_profile::EnrollmentBundle;

use crate::{
    ENROLLMENT_ABI_VERSION, VelumByteSlice, VelumMutableByteSlice, VelumProfileStatus,
    configuration::copy_bytes, profile::status_for_profile,
};

#[unsafe(no_mangle)]
pub extern "C" fn velum_client_enrollment_abi_version() -> u16 {
    ENROLLMENT_ABI_VERSION
}

/// Validates an enrollment and returns the canonical JSON output size.
///
/// # Safety
///
/// `input` must be readable and `out_required` writable for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_enrollment_validate_v1(
    input: VelumByteSlice,
    out_required: *mut usize,
) -> VelumProfileStatus {
    if out_required.is_null() {
        return VelumProfileStatus::InvalidArgument;
    }
    let canonical = match unsafe { canonical_enrollment(input) } {
        Ok(canonical) => canonical,
        Err(status) => return status,
    };
    unsafe { *out_required = canonical.len() };
    VelumProfileStatus::Ok
}

/// Writes validated canonical enrollment JSON into a caller-owned buffer.
///
/// # Safety
///
/// `input` must be readable. `output` and `out_written` must be writable for
/// the duration of this call when their lengths are nonzero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn velum_client_enrollment_normalize_v1(
    input: VelumByteSlice,
    output: VelumMutableByteSlice,
    out_written: *mut usize,
) -> VelumProfileStatus {
    if out_written.is_null() {
        return VelumProfileStatus::InvalidArgument;
    }
    let canonical = match unsafe { canonical_enrollment(input) } {
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
    unsafe { ptr::copy_nonoverlapping(canonical.as_ptr(), output.pointer, canonical.len()) };
    VelumProfileStatus::Ok
}

unsafe fn canonical_enrollment(input: VelumByteSlice) -> Result<Vec<u8>, VelumProfileStatus> {
    let source = unsafe { copy_bytes(input) }.map_err(|_| VelumProfileStatus::InvalidArgument)?;
    let enrollment = EnrollmentBundle::from_json(&source).map_err(status_for_profile)?;
    enrollment
        .to_canonical_json()
        .map(String::into_bytes)
        .map_err(|_| VelumProfileStatus::Internal)
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use velum_client_profile::{EnrollmentNode, EnrollmentTrust};

    use super::*;

    fn input(bytes: &[u8]) -> VelumByteSlice {
        VelumByteSlice {
            pointer: bytes.as_ptr(),
            length: bytes.len(),
        }
    }

    fn enrollment() -> Vec<u8> {
        EnrollmentBundle::new(
            EnrollmentNode {
                id: "relay-2".into(),
                name: "Relay".into(),
                relay_address: "203.0.113.10:4433".parse::<SocketAddr>().expect("address"),
                server_name: "relay.example".into(),
            },
            2,
            &[9; 32],
            EnrollmentTrust::System,
        )
        .expect("enrollment")
        .to_canonical_json()
        .expect("JSON")
        .into_bytes()
    }

    #[test]
    fn validates_and_normalizes_enrollment() {
        let enrollment = enrollment();
        let mut required = 0;
        assert_eq!(
            unsafe { velum_client_enrollment_validate_v1(input(&enrollment), &mut required) },
            VelumProfileStatus::Ok
        );
        let mut output = vec![0; required];
        assert_eq!(
            unsafe {
                velum_client_enrollment_normalize_v1(
                    input(&enrollment),
                    VelumMutableByteSlice {
                        pointer: output.as_mut_ptr(),
                        length: output.len(),
                    },
                    &mut required,
                )
            },
            VelumProfileStatus::Ok
        );
        assert_eq!(output, enrollment);
    }

    #[test]
    fn rejects_arbitrary_qr_content() {
        let mut required = 0;
        assert_eq!(
            unsafe {
                velum_client_enrollment_validate_v1(input(b"https://example.com"), &mut required)
            },
            VelumProfileStatus::Syntax
        );
    }
}
