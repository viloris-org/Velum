use std::{
    collections::{HashMap, VecDeque},
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::{Duration, Instant},
};

/// One fake-IP allocation retaining the observed DNS identity and real answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FakeDnsMapping {
    pub domain: String,
    pub real_address: IpAddr,
    pub fake_address: IpAddr,
    pub generation: u64,
    expires_at: Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FakeDnsError {
    ZeroCapacity,
    EmptyDomain,
    GenerationMismatch,
    AddressSpaceExhausted,
}

/// Bounded, TTL-governed fake-IP ownership for one active profile generation.
pub struct FakeDnsTable {
    generation: u64,
    capacity: usize,
    mappings: HashMap<IpAddr, FakeDnsMapping>,
    insertion_order: VecDeque<IpAddr>,
    next_ipv4: u32,
    next_ipv6: u128,
}

impl FakeDnsTable {
    pub fn new(generation: u64, capacity: usize) -> Result<Self, FakeDnsError> {
        if capacity == 0 {
            return Err(FakeDnsError::ZeroCapacity);
        }
        Ok(Self {
            generation,
            capacity,
            mappings: HashMap::new(),
            insertion_order: VecDeque::new(),
            next_ipv4: u32::from(Ipv4Addr::new(198, 18, 0, 1)),
            next_ipv6: u128::from(Ipv6Addr::new(0xfd00, 0x19, 0, 0, 0, 0, 0, 1)),
        })
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn allocate(
        &mut self,
        generation: u64,
        domain: &str,
        real_address: IpAddr,
        ttl: Duration,
        now: Instant,
    ) -> Result<IpAddr, FakeDnsError> {
        if generation != self.generation {
            return Err(FakeDnsError::GenerationMismatch);
        }
        let domain = normalize_domain(domain)?;
        self.remove_expired(now);
        if let Some(existing) = self
            .mappings
            .values_mut()
            .find(|mapping| mapping.domain == domain && mapping.real_address == real_address)
        {
            existing.expires_at = now + ttl;
            return Ok(existing.fake_address);
        }
        while self.mappings.len() >= self.capacity {
            let Some(oldest) = self.insertion_order.pop_front() else {
                break;
            };
            self.mappings.remove(&oldest);
        }
        let fake_address = self.next_address(real_address)?;
        self.insertion_order.push_back(fake_address);
        self.mappings.insert(
            fake_address,
            FakeDnsMapping {
                domain,
                real_address,
                fake_address,
                generation,
                expires_at: now + ttl,
            },
        );
        Ok(fake_address)
    }

    pub fn resolve(
        &mut self,
        generation: u64,
        fake_address: IpAddr,
        now: Instant,
    ) -> Result<Option<FakeDnsMapping>, FakeDnsError> {
        if generation != self.generation {
            return Err(FakeDnsError::GenerationMismatch);
        }
        self.remove_expired(now);
        Ok(self.mappings.get(&fake_address).cloned())
    }

    pub fn replace_generation(&mut self, generation: u64) {
        self.generation = generation;
        self.mappings.clear();
        self.insertion_order.clear();
    }

    fn remove_expired(&mut self, now: Instant) {
        self.mappings.retain(|_, mapping| mapping.expires_at > now);
        self.insertion_order
            .retain(|address| self.mappings.contains_key(address));
    }

    fn next_address(&mut self, real: IpAddr) -> Result<IpAddr, FakeDnsError> {
        for _ in 0..=self.capacity {
            let candidate = match real {
                IpAddr::V4(_) => {
                    let address = self.next_ipv4;
                    self.next_ipv4 = self
                        .next_ipv4
                        .checked_add(1)
                        .ok_or(FakeDnsError::AddressSpaceExhausted)?;
                    if self.next_ipv4 > u32::from(Ipv4Addr::new(198, 19, 255, 254)) {
                        self.next_ipv4 = u32::from(Ipv4Addr::new(198, 18, 0, 1));
                    }
                    IpAddr::V4(Ipv4Addr::from(address))
                }
                IpAddr::V6(_) => {
                    let address = self.next_ipv6;
                    self.next_ipv6 = self
                        .next_ipv6
                        .checked_add(1)
                        .ok_or(FakeDnsError::AddressSpaceExhausted)?;
                    IpAddr::V6(Ipv6Addr::from(address))
                }
            };
            if !self.mappings.contains_key(&candidate) {
                return Ok(candidate);
            }
        }
        Err(FakeDnsError::AddressSpaceExhausted)
    }
}

fn normalize_domain(domain: &str) -> Result<String, FakeDnsError> {
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty() || domain.len() > 253 {
        return Err(FakeDnsError::EmptyDomain);
    }
    Ok(domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mappings_are_dual_stack_bounded_and_generation_scoped() {
        let now = Instant::now();
        let mut table = FakeDnsTable::new(7, 2).expect("table");
        let ipv4 = table
            .allocate(
                7,
                "Example.COM.",
                "192.0.2.1".parse().expect("IPv4"),
                Duration::from_secs(30),
                now,
            )
            .expect("mapping");
        let ipv6 = table
            .allocate(
                7,
                "example.net",
                "2001:db8::1".parse().expect("IPv6"),
                Duration::from_secs(30),
                now,
            )
            .expect("mapping");
        assert!(ipv4.is_ipv4());
        assert!(ipv6.is_ipv6());
        assert_eq!(
            table
                .resolve(7, ipv4, now)
                .expect("resolve")
                .unwrap()
                .domain,
            "example.com"
        );
        table.replace_generation(8);
        assert_eq!(table.resolve(8, ipv4, now), Ok(None));
        assert_eq!(
            table.resolve(7, ipv4, now),
            Err(FakeDnsError::GenerationMismatch)
        );
    }

    #[test]
    fn expired_and_oldest_mappings_are_removed() {
        let now = Instant::now();
        let mut table = FakeDnsTable::new(1, 1).expect("table");
        let first = table
            .allocate(
                1,
                "first.example",
                "192.0.2.1".parse().expect("IP"),
                Duration::from_secs(1),
                now,
            )
            .expect("first");
        assert_eq!(
            table.resolve(1, first, now + Duration::from_secs(2)),
            Ok(None)
        );
        let second = table
            .allocate(
                1,
                "second.example",
                "192.0.2.2".parse().expect("IP"),
                Duration::from_secs(30),
                now,
            )
            .expect("second");
        assert_ne!(first, second);
    }
}
