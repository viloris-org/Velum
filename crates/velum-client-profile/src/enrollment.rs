use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use super::{ProfileError, validate_identifier, validate_server_name};

pub const ENROLLMENT_KIND: &str = "velum-enrollment";
pub const ENROLLMENT_VERSION: u16 = 1;
pub const MAX_ENROLLMENT_BYTES: usize = 16 * 1024;
const CREDENTIAL_BYTES: usize = 32;
const MAX_CERTIFICATE_BYTES: usize = 12 * 1024;

/// One-time transport object containing the secret material needed to install
/// a redacted client node profile. It must never be persisted as a profile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct EnrollmentBundle {
    pub kind: String,
    pub version: u16,
    pub node: EnrollmentNode,
    pub principal_id: u64,
    pub credential: String,
    pub trust: EnrollmentTrust,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct EnrollmentNode {
    pub id: String,
    pub name: String,
    pub relay_address: SocketAddr,
    pub server_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum EnrollmentTrust {
    System,
    CustomCa { certificate_pem: String },
}

impl EnrollmentBundle {
    pub fn new(
        node: EnrollmentNode,
        principal_id: u64,
        credential: &[u8],
        trust: EnrollmentTrust,
    ) -> Result<Self, ProfileError> {
        let mut bundle = Self {
            kind: ENROLLMENT_KIND.into(),
            version: ENROLLMENT_VERSION,
            node,
            principal_id,
            credential: encode_hex(credential),
            trust,
        };
        bundle.validate_and_normalize()?;
        Ok(bundle)
    }

    pub fn from_json(source: &[u8]) -> Result<Self, ProfileError> {
        if source.len() > MAX_ENROLLMENT_BYTES {
            return Err(ProfileError::limit("enrollment exceeds 16 KiB"));
        }
        let mut bundle: Self = serde_json::from_slice(source)
            .map_err(|error| ProfileError::syntax(error.to_string()))?;
        bundle.validate_and_normalize()?;
        Ok(bundle)
    }

    pub fn to_canonical_json(&self) -> Result<String, ProfileError> {
        let mut normalized = self.clone();
        normalized.validate_and_normalize()?;
        serde_json::to_string(&normalized).map_err(|error| ProfileError::syntax(error.to_string()))
    }

    pub fn credential_bytes(&self) -> Result<Vec<u8>, ProfileError> {
        decode_hex(&self.credential)
    }

    pub fn credential_ref(&self) -> String {
        format!(
            "secret://velum/enrollment/{}/{}",
            self.node.id, self.principal_id
        )
    }

    fn validate_and_normalize(&mut self) -> Result<(), ProfileError> {
        if self.kind != ENROLLMENT_KIND {
            return Err(ProfileError::validation("enrollment kind is invalid"));
        }
        if self.version != ENROLLMENT_VERSION {
            return Err(ProfileError::version("unsupported enrollment version"));
        }
        validate_identifier(&self.node.id, "enrollment node id")?;
        if self.node.name.trim().is_empty() || self.node.name.len() > 128 {
            return Err(ProfileError::validation("enrollment node name is invalid"));
        }
        if self.node.relay_address.ip().is_unspecified() {
            return Err(ProfileError::validation(
                "enrollment relay address must be client reachable",
            ));
        }
        validate_server_name(&self.node.server_name)?;
        let credential = decode_hex(&self.credential)?;
        if credential.len() != CREDENTIAL_BYTES {
            return Err(ProfileError::validation(
                "enrollment credential must contain exactly 32 bytes",
            ));
        }
        self.credential.make_ascii_lowercase();
        if let EnrollmentTrust::CustomCa { certificate_pem } = &self.trust
            && (certificate_pem.is_empty()
                || certificate_pem.len() > MAX_CERTIFICATE_BYTES
                || !certificate_pem.contains("-----BEGIN CERTIFICATE-----"))
        {
            return Err(ProfileError::validation(
                "enrollment CA certificate is invalid or too large",
            ));
        }
        Ok(())
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex(value: &str) -> Result<Vec<u8>, ProfileError> {
    if value.is_empty() || !value.len().is_multiple_of(2) {
        return Err(ProfileError::validation(
            "enrollment credential must be hexadecimal",
        ));
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("hexadecimal input is UTF-8");
            u8::from_str_radix(pair, 16)
                .map_err(|_| ProfileError::validation("enrollment credential must be hexadecimal"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> EnrollmentBundle {
        EnrollmentBundle::new(
            EnrollmentNode {
                id: "relay-sg".into(),
                name: "Singapore relay".into(),
                relay_address: "203.0.113.10:4433".parse().expect("address"),
                server_name: "relay.example".into(),
            },
            7,
            &[0x5a; 32],
            EnrollmentTrust::System,
        )
        .expect("bundle")
    }

    #[test]
    fn canonical_enrollment_round_trips_without_schema_drift() {
        let bundle = bundle();
        let canonical = bundle.to_canonical_json().expect("JSON");
        let parsed = EnrollmentBundle::from_json(canonical.as_bytes()).expect("parse");
        assert_eq!(parsed, bundle);
        assert_eq!(parsed.credential_bytes().expect("credential"), [0x5a; 32]);
        assert_eq!(
            parsed.credential_ref(),
            "secret://velum/enrollment/relay-sg/7"
        );
    }

    #[test]
    fn rejects_unknown_fields_weak_credentials_and_unreachable_addresses() {
        let canonical = bundle().to_canonical_json().expect("JSON");
        let unknown = canonical.replacen('{', "{\"unknown\":true,", 1);
        assert!(EnrollmentBundle::from_json(unknown.as_bytes()).is_err());

        let weak = canonical.replace(&"5a".repeat(32), "5a");
        assert!(EnrollmentBundle::from_json(weak.as_bytes()).is_err());

        let unreachable = canonical.replace("203.0.113.10", "0.0.0.0");
        assert!(EnrollmentBundle::from_json(unreachable.as_bytes()).is_err());
    }

    #[test]
    fn validates_custom_ca_material_and_input_bound() {
        let custom = EnrollmentBundle::new(
            bundle().node,
            8,
            &[7; 32],
            EnrollmentTrust::CustomCa {
                certificate_pem: "-----BEGIN CERTIFICATE-----\nAA==\n-----END CERTIFICATE-----\n"
                    .into(),
            },
        );
        assert!(custom.is_ok());
        assert_eq!(
            EnrollmentBundle::from_json(&vec![b' '; MAX_ENROLLMENT_BYTES + 1])
                .expect_err("oversize")
                .kind(),
            super::super::ProfileErrorKind::Limit
        );
    }
}
