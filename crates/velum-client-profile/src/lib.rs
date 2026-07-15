//! Strict versioned YAML profiles for Velum clients.

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use velum_client_routing::{RoutingAction, RoutingPolicy, RoutingRule, RuleMatcher};

pub const PROFILE_VERSION: u16 = 1;
pub const MAX_PROFILE_BYTES: usize = 1024 * 1024;
pub const MAX_NODES: usize = 128;
pub const MAX_RULES: usize = 10_000;

/// A fully validated and reference-normalized v1 profile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientProfile {
    pub version: u16,
    pub profile: ProfileMetadata,
    pub nodes: Vec<NodeConfig>,
    pub traffic: TrafficConfig,
    pub routing: RoutingConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileMetadata {
    pub id: String,
    pub name: String,
    #[serde(rename = "default-node")]
    pub default_node: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeConfig {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(rename = "relay-address")]
    pub relay_address: SocketAddr,
    #[serde(rename = "server-name")]
    pub server_name: String,
    #[serde(rename = "credential-ref")]
    pub credential_ref: SecretRef,
    pub trust: TrustConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TrustConfig {
    System,
    CustomCa {
        #[serde(rename = "ca-ref")]
        ca_ref: SecretRef,
    },
}

impl TrustConfig {
    pub const fn ca_ref(&self) -> Option<&SecretRef> {
        match self {
            Self::System => None,
            Self::CustomCa { ca_ref } => Some(ca_ref),
        }
    }
}

/// Opaque reference into platform secure storage. Secret bytes are never part of a profile.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SecretRef(String);

impl SecretRef {
    pub fn parse(value: impl Into<String>) -> Result<Self, ProfileError> {
        let value = value.into();
        let path = value.strip_prefix("secret://velum/").ok_or_else(|| {
            ProfileError::validation("secret reference must use secret://velum/ namespace")
        })?;
        if path.is_empty()
            || value.len() > 512
            || path.split('/').any(|segment| {
                segment.is_empty()
                    || segment == "."
                    || segment == ".."
                    || !segment.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                    })
            })
        {
            return Err(ProfileError::validation("secret reference is invalid"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SecretRef {
    type Error = ProfileError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<SecretRef> for String {
    fn from(value: SecretRef) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrafficConfig {
    #[serde(rename = "preferred-adapter")]
    pub preferred_adapter: PreferredAdapter,
    #[serde(rename = "system-proxy")]
    pub system_proxy: SystemProxyConfig,
    pub tun: TunConfig,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreferredAdapter {
    SystemProxy,
    Tun,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SystemProxyConfig {
    pub port: u16,
    pub bypass: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TunConfig {
    pub mtu: u16,
    pub ipv4: InterfaceAddress,
    pub ipv6: InterfaceAddress,
    pub dns: Vec<IpAddr>,
}

/// TUN interface address and prefix. Unlike a routing CIDR, host bits are retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterfaceAddress {
    address: IpAddr,
    prefix_length: u8,
}

impl InterfaceAddress {
    pub fn new(address: IpAddr, prefix_length: u8) -> Result<Self, ProfileError> {
        let valid = match address {
            IpAddr::V4(_) => prefix_length <= 32,
            IpAddr::V6(_) => prefix_length <= 128,
        };
        if !valid {
            return Err(ProfileError::validation(
                "interface prefix length is out of range",
            ));
        }
        Ok(Self {
            address,
            prefix_length,
        })
    }

    pub const fn address(self) -> IpAddr {
        self.address
    }

    pub const fn prefix_length(self) -> u8 {
        self.prefix_length
    }
}

impl fmt::Display for InterfaceAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.address, self.prefix_length)
    }
}

impl FromStr for InterfaceAddress {
    type Err = ProfileError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let (address, prefix) = source
            .trim()
            .split_once('/')
            .ok_or_else(|| ProfileError::validation("interface address requires a prefix"))?;
        let address = address
            .parse()
            .map_err(|_| ProfileError::validation("interface address is invalid"))?;
        let prefix = prefix
            .parse()
            .map_err(|_| ProfileError::validation("interface prefix is invalid"))?;
        Self::new(address, prefix)
    }
}

impl Serialize for InterfaceAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for InterfaceAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingConfig {
    pub mode: RoutingMode,
    pub rules: Vec<RuleConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingMode {
    Rule,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleConfig {
    #[serde(rename = "match")]
    pub matcher: MatchConfig,
    pub action: ActionConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum MatchConfig {
    Domain { value: String },
    DomainSuffix { value: String },
    IpCidr { value: String },
    DstPort { value: String },
    Match,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ActionConfig {
    Direct,
    Reject,
    Proxy,
    Node { target: String },
}

impl ClientProfile {
    /// Parse, bound, validate, and normalize an application-managed YAML profile.
    pub fn from_yaml(source: &[u8]) -> Result<Self, ProfileError> {
        if source.len() > MAX_PROFILE_BYTES {
            return Err(ProfileError::limit("profile exceeds 1 MiB"));
        }
        let mut profile: Self = serde_yaml_ng::from_slice(source)
            .map_err(|error| ProfileError::syntax(error.to_string()))?;
        profile.validate_and_normalize()?;
        Ok(profile)
    }

    /// Emit deterministic field ordering and normalized stable node references.
    pub fn to_canonical_yaml(&self) -> Result<String, ProfileError> {
        let mut normalized = self.clone();
        normalized.validate_and_normalize()?;
        serde_yaml_ng::to_string(&normalized)
            .map_err(|error| ProfileError::syntax(error.to_string()))
    }

    /// Compile profile rules into the shared transport-independent policy.
    pub fn routing_policy(&self) -> Result<RoutingPolicy, ProfileError> {
        let mut normalized = self.clone();
        normalized.validate_and_normalize()?;
        let rules = normalized
            .routing
            .rules
            .iter()
            .map(RuleConfig::compile)
            .collect::<Result<Vec<_>, _>>()?;
        RoutingPolicy::new(rules).map_err(ProfileError::routing)
    }

    fn validate_and_normalize(&mut self) -> Result<(), ProfileError> {
        if self.version != PROFILE_VERSION {
            return Err(ProfileError::version("unsupported profile version"));
        }
        validate_identifier(&self.profile.id, "profile id")?;
        if self.profile.name.trim().is_empty() || self.profile.name.len() > 128 {
            return Err(ProfileError::validation("profile name is invalid"));
        }
        if self.nodes.is_empty() || self.nodes.len() > MAX_NODES {
            return Err(ProfileError::limit("profile must contain 1 to 128 nodes"));
        }
        if self.routing.rules.len() > MAX_RULES {
            return Err(ProfileError::limit("profile exceeds 10000 routing rules"));
        }

        let mut references = HashMap::new();
        let mut ids = HashSet::new();
        for node in &self.nodes {
            validate_identifier(&node.id, "node id")?;
            if !ids.insert(node.id.clone()) {
                return Err(ProfileError::validation("duplicate node id"));
            }
            insert_reference(&mut references, &node.id, &node.id)?;
            if let Some(alias) = &node.alias {
                validate_identifier(alias, "node alias")?;
                insert_reference(&mut references, alias, &node.id)?;
            }
            validate_server_name(&node.server_name)?;
        }

        self.profile.default_node = resolve_node(&references, &self.profile.default_node)?;
        for rule in &mut self.routing.rules {
            rule.validate()?;
            if let ActionConfig::Node { target } = &mut rule.action {
                *target = resolve_node(&references, target)?;
            }
        }
        if self
            .routing
            .rules
            .iter()
            .position(|rule| matches!(rule.matcher, MatchConfig::Match))
            .is_some_and(|index| index + 1 != self.routing.rules.len())
        {
            return Err(ProfileError::validation(
                "MATCH must be the final routing rule",
            ));
        }

        if !(576..=9000).contains(&self.traffic.tun.mtu) {
            return Err(ProfileError::validation("TUN MTU is out of range"));
        }
        if !self.traffic.tun.ipv4.address().is_ipv4() || !self.traffic.tun.ipv6.address().is_ipv6()
        {
            return Err(ProfileError::validation(
                "TUN ipv4 and ipv6 networks use the wrong address family",
            ));
        }
        if self.traffic.tun.dns.is_empty() {
            return Err(ProfileError::validation("TUN DNS list must not be empty"));
        }
        Ok(())
    }
}

impl RuleConfig {
    fn validate(&self) -> Result<(), ProfileError> {
        let _ = self.compile()?;
        Ok(())
    }

    fn compile(&self) -> Result<RoutingRule, ProfileError> {
        let matcher = match &self.matcher {
            MatchConfig::Domain { value } => {
                RuleMatcher::domain(value).map_err(ProfileError::routing)?
            }
            MatchConfig::DomainSuffix { value } => {
                RuleMatcher::domain_suffix(value).map_err(ProfileError::routing)?
            }
            MatchConfig::IpCidr { value } => {
                RuleMatcher::IpCidr(value.parse().map_err(ProfileError::routing)?)
            }
            MatchConfig::DstPort { value } => {
                RuleMatcher::DestinationPort(value.parse().map_err(ProfileError::routing)?)
            }
            MatchConfig::Match => RuleMatcher::Match,
        };
        let action = match &self.action {
            ActionConfig::Direct => RoutingAction::Direct,
            ActionConfig::Reject => RoutingAction::Reject,
            ActionConfig::Proxy => RoutingAction::Proxy,
            ActionConfig::Node { target } => RoutingAction::Node(target.clone()),
        };
        Ok(RoutingRule::new(matcher, action))
    }
}

fn insert_reference(
    references: &mut HashMap<String, String>,
    reference: &str,
    id: &str,
) -> Result<(), ProfileError> {
    if references
        .insert(reference.to_owned(), id.to_owned())
        .is_some()
    {
        return Err(ProfileError::validation("duplicate node id or alias"));
    }
    Ok(())
}

fn resolve_node(
    references: &HashMap<String, String>,
    reference: &str,
) -> Result<String, ProfileError> {
    references
        .get(reference)
        .cloned()
        .ok_or_else(|| ProfileError::validation("node reference does not exist"))
}

fn validate_identifier(value: &str, field: &str) -> Result<(), ProfileError> {
    if value.is_empty()
        || value.len() > 64
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'-' | b'_'))
        })
    {
        return Err(ProfileError::validation(format!("{field} is invalid")));
    }
    Ok(())
}

fn validate_server_name(value: &str) -> Result<(), ProfileError> {
    if value.parse::<IpAddr>().is_ok() {
        return Ok(());
    }
    RuleMatcher::domain(value)
        .map(|_| ())
        .map_err(|_| ProfileError::validation("server name is invalid"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileErrorKind {
    Syntax,
    UnsupportedVersion,
    Limit,
    Validation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileError {
    kind: ProfileErrorKind,
    message: String,
}

impl ProfileError {
    pub const fn kind(&self) -> ProfileErrorKind {
        self.kind
    }

    fn syntax(message: impl Into<String>) -> Self {
        Self::new(ProfileErrorKind::Syntax, message)
    }

    fn version(message: impl Into<String>) -> Self {
        Self::new(ProfileErrorKind::UnsupportedVersion, message)
    }

    fn limit(message: impl Into<String>) -> Self {
        Self::new(ProfileErrorKind::Limit, message)
    }

    fn validation(message: impl Into<String>) -> Self {
        Self::new(ProfileErrorKind::Validation, message)
    }

    fn routing(error: impl fmt::Display) -> Self {
        Self::validation(error.to_string())
    }

    fn new(kind: ProfileErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ProfileError {}

impl FromStr for ClientProfile {
    type Err = ProfileError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        Self::from_yaml(source.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use velum_client_routing::{RouteContext, RoutingAction};

    const VALID: &str = include_str!("../tests/fixtures/valid-profile.yaml");

    #[test]
    fn parses_and_normalizes_alias_references() {
        let profile = ClientProfile::from_yaml(VALID.as_bytes()).expect("valid profile");
        assert_eq!(profile.profile.default_node, "node-sg");
        assert!(matches!(
            &profile.routing.rules[0].action,
            ActionConfig::Node { target } if target == "node-sg"
        ));
    }

    #[test]
    fn canonical_yaml_round_trips() {
        let profile = ClientProfile::from_yaml(VALID.as_bytes()).expect("valid profile");
        let first = profile.to_canonical_yaml().expect("YAML");
        let reparsed = ClientProfile::from_yaml(first.as_bytes()).expect("canonical profile");
        let second = reparsed.to_canonical_yaml().expect("YAML");
        assert_eq!(profile, reparsed);
        assert_eq!(first, second);
        assert!(first.contains("target: node-sg"));
    }

    #[test]
    fn compiled_policy_supports_node_port_ipv6_and_first_match() {
        let profile = ClientProfile::from_yaml(VALID.as_bytes()).expect("valid profile");
        let policy = profile.routing_policy().expect("policy");
        let decision = policy.decide(RouteContext {
            domain: Some("api.example.com"),
            destination: "2001:db8::1".parse().expect("IP"),
            destination_port: 8500,
        });
        assert_eq!(decision, RoutingAction::Node("node-sg".into()));
        let decision = policy.decide(RouteContext {
            domain: None,
            destination: "2001:db8::1".parse().expect("IP"),
            destination_port: 8500,
        });
        assert_eq!(decision, RoutingAction::Reject);
    }

    #[test]
    fn rejects_unknown_fields_and_versions() {
        let unknown = VALID.replace("version: 1", "version: 1\nunknown: true");
        assert_eq!(
            ClientProfile::from_yaml(unknown.as_bytes())
                .expect_err("unknown field")
                .kind(),
            ProfileErrorKind::Syntax
        );
        let version = VALID.replace("version: 1", "version: 2");
        assert_eq!(
            ClientProfile::from_yaml(version.as_bytes())
                .expect_err("version")
                .kind(),
            ProfileErrorKind::UnsupportedVersion
        );
    }

    #[test]
    fn rejects_duplicate_and_dangling_node_references() {
        let duplicate = VALID.replace("alias: singapore", "alias: node-sg");
        assert!(ClientProfile::from_yaml(duplicate.as_bytes()).is_err());
        let dangling = VALID.replace("target: singapore", "target: missing");
        assert!(ClientProfile::from_yaml(dangling.as_bytes()).is_err());
    }

    #[test]
    fn rejects_inline_or_external_secret_locations() {
        for replacement in [
            "plaintext-password",
            "file:///tmp/credential",
            "secret://other/key",
        ] {
            let invalid = VALID.replace("secret://velum/personal/node-sg", replacement);
            assert!(ClientProfile::from_yaml(invalid.as_bytes()).is_err());
        }
    }

    #[test]
    fn enforces_input_and_collection_bounds() {
        assert_eq!(
            ClientProfile::from_yaml(&vec![b' '; MAX_PROFILE_BYTES + 1])
                .expect_err("oversize")
                .kind(),
            ProfileErrorKind::Limit
        );
        let rule = "    - match: { type: match }\n      action: { type: proxy }\n";
        let rules = rule.repeat(MAX_RULES + 1);
        let oversized_rules = VALID.replace(
            "    - match: { type: match }\n      action: { type: proxy }\n",
            &rules,
        );
        assert_eq!(
            ClientProfile::from_yaml(oversized_rules.as_bytes())
                .expect_err("too many rules")
                .kind(),
            ProfileErrorKind::Limit
        );
    }

    #[test]
    fn rejects_match_before_another_rule_and_invalid_ranges() {
        let wrong_order = VALID.replace(
            "    - match: { type: dst-port, value: 8000-8999 }",
            "    - match: { type: match }",
        );
        assert!(ClientProfile::from_yaml(wrong_order.as_bytes()).is_err());
        let bad_port = VALID.replace("8000-8999", "9000-8000");
        assert!(ClientProfile::from_yaml(bad_port.as_bytes()).is_err());
        let bad_cidr = VALID.replace("fd00:19::1/126", "fd00:19::1/129");
        assert!(ClientProfile::from_yaml(bad_cidr.as_bytes()).is_err());
    }

    #[test]
    fn enforces_node_bound() {
        let mut profile = ClientProfile::from_yaml(VALID.as_bytes()).expect("valid profile");
        profile.nodes = (0..=MAX_NODES)
            .map(|index| {
                let mut node = profile.nodes[0].clone();
                node.id = format!("node-{index}");
                node.alias = None;
                node
            })
            .collect();
        assert_eq!(
            profile
                .validate_and_normalize()
                .expect_err("too many nodes")
                .kind(),
            ProfileErrorKind::Limit
        );
    }
}
