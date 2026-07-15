//! Best-effort public address discovery for interactive setup.

use std::{net::IpAddr, time::Duration};

const IPINFO_IP_URL: &str = "https://ipinfo.io/ip";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

pub async fn detect() -> Option<IpAddr> {
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .ok()?;
    let response = client
        .get(IPINFO_IP_URL)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let address = response.text().await.ok()?.trim().parse::<IpAddr>().ok()?;
    is_usable(address).then_some(address)
}

fn is_usable(address: IpAddr) -> bool {
    !address.is_unspecified()
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::is_usable;

    #[test]
    fn unspecified_addresses_are_not_accepted() {
        let address = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
        assert!(!is_usable(address));
        assert!(is_usable("198.51.100.42".parse().expect("public address")));
    }
}
