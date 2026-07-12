//! Server-side admission policy.
//!
//! Network listeners and protocol frames are intentionally outside this crate.
//! It owns only principal authentication, exact destination authorization, and
//! bounded per-principal resource admission.

use std::{collections::BTreeMap, net::SocketAddr};
use subtle::ConstantTimeEq;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PrincipalId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthenticationError {
    InvalidCredential,
    DuplicateCredential,
    DuplicatePrincipal,
    InconsistentCredentialLength,
}

pub struct PrincipalCredential {
    pub principal: PrincipalId,
    secret: Vec<u8>,
}

impl PrincipalCredential {
    pub fn new(principal: PrincipalId, secret: Vec<u8>) -> Result<Self, AuthenticationError> {
        if secret.is_empty() {
            return Err(AuthenticationError::InvalidCredential);
        }
        Ok(Self { principal, secret })
    }
}

/// Constant-time shared-secret authentication for the experimental Stage 2
/// listener. Credentials are configuration inputs and must never be logged.
pub struct Authenticator {
    credentials: BTreeMap<PrincipalId, Vec<u8>>,
    secret_length: Option<usize>,
}

impl Authenticator {
    pub fn new(
        credentials: impl IntoIterator<Item = PrincipalCredential>,
    ) -> Result<Self, AuthenticationError> {
        let mut configured: BTreeMap<PrincipalId, Vec<u8>> = BTreeMap::new();
        let mut secret_length = None;
        for credential in credentials {
            if let Some(expected) = secret_length
                && credential.secret.len() != expected
            {
                return Err(AuthenticationError::InconsistentCredentialLength);
            }
            secret_length = Some(credential.secret.len());
            if configured
                .values()
                .any(|existing| bool::from(existing.as_slice().ct_eq(credential.secret.as_slice())))
            {
                return Err(AuthenticationError::DuplicateCredential);
            }
            if configured
                .insert(credential.principal, credential.secret)
                .is_some()
            {
                return Err(AuthenticationError::DuplicatePrincipal);
            }
        }
        Ok(Self {
            credentials: configured,
            secret_length,
        })
    }

    pub fn authenticate(&self, secret: &[u8]) -> Result<PrincipalId, AuthenticationError> {
        if self.secret_length != Some(secret.len()) {
            return Err(AuthenticationError::InvalidCredential);
        }

        let mut authenticated = None;
        for (principal, configured) in &self.credentials {
            if bool::from(configured.as_slice().ct_eq(secret)) {
                authenticated = Some(*principal);
            }
        }
        authenticated.ok_or(AuthenticationError::InvalidCredential)
    }
}

/// Exact destination allowlist. An empty policy denies every target.
#[derive(Clone, Debug, Default)]
pub struct DestinationPolicy {
    allowed: BTreeMap<SocketAddr, ()>,
}

impl DestinationPolicy {
    pub fn new(allowed: impl IntoIterator<Item = SocketAddr>) -> Self {
        Self {
            allowed: allowed.into_iter().map(|target| (target, ())).collect(),
        }
    }

    pub fn allows(&self, target: SocketAddr) -> bool {
        self.allowed.contains_key(&target)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrincipalQuota {
    pub max_sessions: usize,
    pub max_flows_per_session: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionError {
    SessionQuotaExceeded,
    FlowQuotaExceeded,
    UnknownSession,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionLease(u64);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SessionUsage {
    flows: usize,
}

/// In-memory quota accounting for one server process.
#[derive(Debug)]
pub struct AdmissionControl {
    quota: PrincipalQuota,
    next_lease: u64,
    sessions: BTreeMap<(PrincipalId, SessionLease), SessionUsage>,
}

impl AdmissionControl {
    pub fn new(quota: PrincipalQuota) -> Self {
        Self {
            quota,
            next_lease: 0,
            sessions: BTreeMap::new(),
        }
    }

    pub fn open_session(&mut self, principal: PrincipalId) -> Result<SessionLease, AdmissionError> {
        if self
            .sessions
            .keys()
            .filter(|(candidate, _)| *candidate == principal)
            .count()
            >= self.quota.max_sessions
        {
            return Err(AdmissionError::SessionQuotaExceeded);
        }
        let lease = SessionLease(self.next_lease);
        self.next_lease = self
            .next_lease
            .checked_add(1)
            .expect("session lease exhausted");
        self.sessions
            .insert((principal, lease), SessionUsage::default());
        Ok(lease)
    }

    pub fn open_flow(
        &mut self,
        principal: PrincipalId,
        lease: SessionLease,
    ) -> Result<(), AdmissionError> {
        let usage = self
            .sessions
            .get_mut(&(principal, lease))
            .ok_or(AdmissionError::UnknownSession)?;
        if usage.flows >= self.quota.max_flows_per_session {
            return Err(AdmissionError::FlowQuotaExceeded);
        }
        usage.flows += 1;
        Ok(())
    }

    pub fn close_flow(
        &mut self,
        principal: PrincipalId,
        lease: SessionLease,
    ) -> Result<(), AdmissionError> {
        let usage = self
            .sessions
            .get_mut(&(principal, lease))
            .ok_or(AdmissionError::UnknownSession)?;
        if usage.flows == 0 {
            return Err(AdmissionError::UnknownSession);
        }
        usage.flows -= 1;
        Ok(())
    }

    pub fn close_session(
        &mut self,
        principal: PrincipalId,
        lease: SessionLease,
    ) -> Result<(), AdmissionError> {
        self.sessions
            .remove(&(principal, lease))
            .map(|_| ())
            .ok_or(AdmissionError::UnknownSession)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authentication_accepts_only_configured_credentials() {
        let authentication =
            Authenticator::new([
                PrincipalCredential::new(PrincipalId(7), vec![1, 2, 3]).expect("credential")
            ])
            .expect("authentication");

        assert_eq!(authentication.authenticate(&[1, 2, 3]), Ok(PrincipalId(7)));
        assert_eq!(
            authentication.authenticate(&[1, 2, 4]),
            Err(AuthenticationError::InvalidCredential)
        );
        assert_eq!(
            authentication.authenticate(&[]),
            Err(AuthenticationError::InvalidCredential)
        );
    }

    #[test]
    fn credentials_must_have_a_consistent_length() {
        let credentials = [
            PrincipalCredential::new(PrincipalId(1), vec![1]).expect("credential"),
            PrincipalCredential::new(PrincipalId(2), vec![2, 3]).expect("credential"),
        ];

        assert!(matches!(
            Authenticator::new(credentials),
            Err(AuthenticationError::InconsistentCredentialLength)
        ));
    }

    #[test]
    fn credentials_must_not_share_a_secret() {
        let credentials = [
            PrincipalCredential::new(PrincipalId(1), vec![1, 2, 3]).expect("credential"),
            PrincipalCredential::new(PrincipalId(2), vec![1, 2, 3]).expect("credential"),
        ];

        assert!(matches!(
            Authenticator::new(credentials),
            Err(AuthenticationError::DuplicateCredential)
        ));
    }

    #[test]
    fn destinations_are_denied_until_exactly_allowed() {
        let allowed = "192.0.2.10:443".parse().expect("socket address");
        let policy = DestinationPolicy::new([allowed]);

        assert!(policy.allows(allowed));
        assert!(!policy.allows("192.0.2.10:80".parse().expect("socket address")));
        assert!(!DestinationPolicy::default().allows(allowed));
    }

    #[test]
    fn quotas_are_principal_scoped_and_release_on_close() {
        let principal = PrincipalId(1);
        let mut admission = AdmissionControl::new(PrincipalQuota {
            max_sessions: 1,
            max_flows_per_session: 1,
        });
        let session = admission.open_session(principal).expect("session");
        assert_eq!(
            admission.open_session(principal),
            Err(AdmissionError::SessionQuotaExceeded)
        );
        admission.open_flow(principal, session).expect("flow");
        assert_eq!(
            admission.open_flow(principal, session),
            Err(AdmissionError::FlowQuotaExceeded)
        );
        admission
            .close_flow(principal, session)
            .expect("close flow");
        admission
            .close_session(principal, session)
            .expect("close session");
        assert!(admission.open_session(principal).is_ok());
    }
}
