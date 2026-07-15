//! Shared, transport-independent client routing policy.

use std::{
    error::Error,
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    str::FromStr,
};

/// Route selected for a client flow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoutingAction {
    Direct,
    Reject,
    Proxy,
    Node(String),
}

/// Input known when routing a flow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteContext<'a> {
    pub domain: Option<&'a str>,
    pub destination: IpAddr,
    pub destination_port: u16,
}

/// One supported rule matcher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuleMatcher {
    Domain(String),
    DomainSuffix(String),
    IpCidr(IpCidr),
    DestinationPort(PortRange),
    Match,
}

impl RuleMatcher {
    pub fn domain(value: impl Into<String>) -> Result<Self, RoutingError> {
        Ok(Self::Domain(normalize_domain(&value.into())?))
    }

    pub fn domain_suffix(value: impl Into<String>) -> Result<Self, RoutingError> {
        Ok(Self::DomainSuffix(normalize_domain(&value.into())?))
    }

    fn matches(&self, context: RouteContext<'_>, domain: Option<&str>) -> bool {
        match self {
            Self::Domain(expected) => domain.is_some_and(|actual| actual == expected),
            Self::DomainSuffix(suffix) => domain.is_some_and(|actual| {
                actual == suffix
                    || actual
                        .strip_suffix(suffix)
                        .is_some_and(|prefix| prefix.ends_with('.'))
            }),
            Self::IpCidr(cidr) => cidr.contains(context.destination),
            Self::DestinationPort(range) => range.contains(context.destination_port),
            Self::Match => true,
        }
    }
}

/// A matcher and its selected action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutingRule {
    matcher: RuleMatcher,
    action: RoutingAction,
}

impl RoutingRule {
    pub const fn new(matcher: RuleMatcher, action: RoutingAction) -> Self {
        Self { matcher, action }
    }

    pub const fn matcher(&self) -> &RuleMatcher {
        &self.matcher
    }

    pub const fn action(&self) -> &RoutingAction {
        &self.action
    }
}

/// Immutable, first-match-wins policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutingPolicy {
    rules: Vec<RoutingRule>,
}

impl RoutingPolicy {
    pub fn new(rules: Vec<RoutingRule>) -> Result<Self, RoutingError> {
        if rules
            .iter()
            .position(|rule| matches!(rule.matcher, RuleMatcher::Match))
            .is_some_and(|index| index + 1 != rules.len())
        {
            return Err(RoutingError::new("MATCH must be the final routing rule"));
        }
        Ok(Self { rules })
    }

    pub fn proxy_all() -> Self {
        Self {
            rules: vec![RoutingRule::new(RuleMatcher::Match, RoutingAction::Proxy)],
        }
    }

    pub fn rules(&self) -> &[RoutingRule] {
        &self.rules
    }

    pub fn decide(&self, context: RouteContext<'_>) -> RoutingAction {
        let domain = context
            .domain
            .and_then(|value| normalize_domain(value).ok());
        self.rules
            .iter()
            .find(|rule| rule.matcher.matches(context, domain.as_deref()))
            .map_or(RoutingAction::Proxy, |rule| rule.action.clone())
    }
}

impl Default for RoutingPolicy {
    fn default() -> Self {
        Self::proxy_all()
    }
}

impl FromStr for RoutingRule {
    type Err = RoutingError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let fields: Vec<_> = source.split(',').map(str::trim).collect();
        if fields.iter().any(|field| field.is_empty()) {
            return Err(RoutingError::new("routing rule fields must not be empty"));
        }
        let (matcher, action) = match fields.as_slice() {
            ["DOMAIN", value, action] => (Self::domain_matcher(value)?, parse_action(action)?),
            ["DOMAIN-SUFFIX", value, action] => {
                (RuleMatcher::domain_suffix(*value)?, parse_action(action)?)
            }
            ["IP-CIDR", value, action] => {
                (RuleMatcher::IpCidr(value.parse()?), parse_action(action)?)
            }
            ["DST-PORT", value, action] => (
                RuleMatcher::DestinationPort(value.parse()?),
                parse_action(action)?,
            ),
            ["MATCH", action] => (RuleMatcher::Match, parse_action(action)?),
            _ => return Err(RoutingError::new("invalid routing rule")),
        };
        Ok(Self::new(matcher, action))
    }
}

impl RoutingRule {
    fn domain_matcher(value: &str) -> Result<RuleMatcher, RoutingError> {
        RuleMatcher::domain(value)
    }
}

impl FromStr for RoutingPolicy {
    type Err = RoutingError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let rules = source
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                let line = line.trim();
                (!line.is_empty()).then_some((index + 1, line))
            })
            .map(|(line_number, line)| {
                line.parse().map_err(|error: RoutingError| {
                    RoutingError::new(format!("invalid rule on line {line_number}: {error}"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(rules)
    }
}

fn parse_action(value: &str) -> Result<RoutingAction, RoutingError> {
    match value {
        "DIRECT" => Ok(RoutingAction::Direct),
        "PROXY" => Ok(RoutingAction::Proxy),
        "REJECT" => Ok(RoutingAction::Reject),
        _ => value
            .strip_prefix("NODE:")
            .filter(|id| !id.is_empty())
            .map(|id| RoutingAction::Node(id.to_owned()))
            .ok_or_else(|| RoutingError::new("unknown routing action")),
    }
}

/// Inclusive destination port range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortRange {
    start: u16,
    end: u16,
}

impl PortRange {
    pub fn new(start: u16, end: u16) -> Result<Self, RoutingError> {
        if start == 0 || start > end {
            return Err(RoutingError::new("destination port range is invalid"));
        }
        Ok(Self { start, end })
    }

    pub const fn start(self) -> u16 {
        self.start
    }

    pub const fn end(self) -> u16 {
        self.end
    }

    pub const fn contains(self, port: u16) -> bool {
        port >= self.start && port <= self.end
    }
}

impl fmt::Display for PortRange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start == self.end {
            write!(formatter, "{}", self.start)
        } else {
            write!(formatter, "{}-{}", self.start, self.end)
        }
    }
}

impl FromStr for PortRange {
    type Err = RoutingError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let source = source.trim();
        let (start, end) = source.split_once('-').unwrap_or((source, source));
        let start = start
            .parse()
            .map_err(|_| RoutingError::new("destination port is invalid"))?;
        let end = end
            .parse()
            .map_err(|_| RoutingError::new("destination port is invalid"))?;
        Self::new(start, end)
    }
}

/// IPv4 or IPv6 network with an address-family-specific prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IpCidr {
    network: IpAddr,
    prefix_length: u8,
}

impl IpCidr {
    pub fn new(network: IpAddr, prefix_length: u8) -> Result<Self, RoutingError> {
        let valid = match network {
            IpAddr::V4(_) => prefix_length <= 32,
            IpAddr::V6(_) => prefix_length <= 128,
        };
        if !valid {
            return Err(RoutingError::new("CIDR prefix length is out of range"));
        }
        let cidr = Self {
            network,
            prefix_length,
        };
        if cidr.mask(network) != network {
            return Err(RoutingError::new("CIDR network has host bits set"));
        }
        Ok(cidr)
    }

    pub const fn network(self) -> IpAddr {
        self.network
    }

    pub const fn prefix_length(self) -> u8 {
        self.prefix_length
    }

    pub fn contains(self, address: IpAddr) -> bool {
        matches!(
            (self.network, address),
            (IpAddr::V4(_), IpAddr::V4(_)) | (IpAddr::V6(_), IpAddr::V6(_))
        ) && self.mask(address) == self.network
    }

    fn mask(self, address: IpAddr) -> IpAddr {
        match address {
            IpAddr::V4(address) => {
                let shift = 32 - u32::from(self.prefix_length.min(32));
                let mask = u32::MAX.checked_shl(shift).unwrap_or(0);
                IpAddr::V4(Ipv4Addr::from(u32::from(address) & mask))
            }
            IpAddr::V6(address) => {
                let shift = 128 - u32::from(self.prefix_length.min(128));
                let mask = u128::MAX.checked_shl(shift).unwrap_or(0);
                IpAddr::V6(Ipv6Addr::from(u128::from(address) & mask))
            }
        }
    }
}

impl fmt::Display for IpCidr {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.network, self.prefix_length)
    }
}

impl FromStr for IpCidr {
    type Err = RoutingError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let (network, prefix) = source
            .trim()
            .split_once('/')
            .ok_or_else(|| RoutingError::new("CIDR requires an address and prefix length"))?;
        let network = network
            .parse()
            .map_err(|_| RoutingError::new("CIDR address is invalid"))?;
        let prefix = prefix
            .parse()
            .map_err(|_| RoutingError::new("CIDR prefix length is invalid"))?;
        Self::new(network, prefix)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutingError {
    message: String,
}

impl RoutingError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RoutingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for RoutingError {}

fn normalize_domain(value: &str) -> Result<String, RoutingError> {
    let value = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if value.is_empty()
        || value.len() > 253
        || value.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err(RoutingError::new("routing rule domain is invalid"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(domain: Option<&'a str>, address: &str, port: u16) -> RouteContext<'a> {
        RouteContext {
            domain,
            destination: address.parse().expect("address"),
            destination_port: port,
        }
    }

    #[test]
    fn first_matching_rule_wins() {
        let policy = RoutingPolicy::new(vec![
            RoutingRule::new(
                RuleMatcher::IpCidr("192.0.2.0/24".parse().expect("CIDR")),
                RoutingAction::Direct,
            ),
            RoutingRule::new(
                RuleMatcher::domain("api.example.com").expect("domain"),
                RoutingAction::Reject,
            ),
            RoutingRule::new(RuleMatcher::Match, RoutingAction::Proxy),
        ])
        .expect("policy");
        assert_eq!(
            policy.decide(context(Some("api.example.com"), "192.0.2.1", 443)),
            RoutingAction::Direct
        );
    }

    #[test]
    fn all_matchers_support_their_boundaries() {
        let exact = RuleMatcher::domain("API.Example.Com.").expect("domain");
        let suffix = RuleMatcher::domain_suffix("example.com").expect("suffix");
        assert!(exact.matches(
            context(Some("api.example.com"), "203.0.113.1", 1),
            Some("api.example.com")
        ));
        assert!(suffix.matches(
            context(Some("example.com"), "203.0.113.1", 1),
            Some("example.com")
        ));
        assert!(suffix.matches(
            context(Some("a.example.com"), "203.0.113.1", 1),
            Some("a.example.com")
        ));
        assert!(!suffix.matches(
            context(Some("notexample.com"), "203.0.113.1", 1),
            Some("notexample.com")
        ));
        assert!(
            RuleMatcher::DestinationPort("8000-8999".parse().expect("ports"))
                .matches(context(None, "203.0.113.1", 8999), None)
        );
    }

    #[test]
    fn cidr_handles_both_address_families() {
        let ipv4: IpCidr = "10.0.0.0/8".parse().expect("IPv4");
        let ipv6: IpCidr = "2001:db8::/32".parse().expect("IPv6");
        assert!(ipv4.contains("10.1.2.3".parse().expect("address")));
        assert!(ipv6.contains("2001:db8::1".parse().expect("address")));
        assert!(!ipv4.contains("2001:db8::1".parse().expect("address")));
        assert!("10.0.0.1/8".parse::<IpCidr>().is_err());
    }

    #[test]
    fn match_must_be_last() {
        let error = RoutingPolicy::new(vec![
            RoutingRule::new(RuleMatcher::Match, RoutingAction::Proxy),
            RoutingRule::new(
                RuleMatcher::DestinationPort("443".parse().expect("port")),
                RoutingAction::Direct,
            ),
        ])
        .expect_err("invalid ordering");
        assert!(error.to_string().contains("final"));
    }

    #[test]
    fn node_action_is_returned_without_losing_identity() {
        let policy = RoutingPolicy::new(vec![RoutingRule::new(
            RuleMatcher::Match,
            RoutingAction::Node("node-sg".into()),
        )])
        .expect("policy");
        assert_eq!(
            policy.decide(context(None, "203.0.113.1", 443)),
            RoutingAction::Node("node-sg".into())
        );
    }

    #[test]
    fn legacy_text_policy_uses_the_shared_matchers() {
        let policy: RoutingPolicy = "DST-PORT,53,DIRECT\nMATCH,NODE:node-sg"
            .parse()
            .expect("policy");
        assert_eq!(
            policy.decide(context(None, "203.0.113.1", 53)),
            RoutingAction::Direct
        );
        assert_eq!(
            policy.decide(context(None, "203.0.113.1", 443)),
            RoutingAction::Node("node-sg".into())
        );
    }
}
