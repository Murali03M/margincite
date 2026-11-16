//! The crawler's only name lookup. It resolves a name once, checks every
//! address and hands back only checked ones, so the connection goes to an
//! address that was checked and a second lookup cannot change the answer.

use std::net::SocketAddr;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use super::client::Network;
use super::guard;

/// A lookup produced an address the crawler must not connect to, or nothing.
#[derive(Debug)]
pub struct AddressRefused;

impl std::fmt::Display for AddressRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the address is not allowed")
    }
}

impl std::error::Error for AddressRefused {}

#[derive(Debug, Clone, Copy)]
pub struct GuardedResolver {
    network: Network,
}

impl GuardedResolver {
    pub fn new(network: Network) -> Self {
        Self { network }
    }
}

impl Resolve for GuardedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let network = self.network;
        let host = name.as_str().to_string();
        Box::pin(async move {
            let found: Vec<SocketAddr> = tokio::net::lookup_host((host, 0)).await?.collect();
            let checked = check_all(network, found)?;
            Ok(Box::new(checked.into_iter()) as Addrs)
        })
    }
}

/// In `Public` mode one refused address refuses the whole answer, so a name
/// that mixes public and internal addresses is never used.
fn check_all(network: Network, found: Vec<SocketAddr>) -> Result<Vec<SocketAddr>, AddressRefused> {
    if network == Network::Public
        && (found.is_empty() || found.iter().any(|addr| guard::check_ip(addr.ip()).is_err()))
    {
        return Err(AddressRefused);
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(raw: &str) -> SocketAddr {
        format!("{raw}:0").parse().unwrap()
    }

    #[test]
    fn one_internal_address_refuses_the_whole_answer() {
        let mixed = vec![addr("93.184.216.34"), addr("127.0.0.1")];
        assert!(check_all(Network::Public, mixed.clone()).is_err());
        assert_eq!(
            check_all(Network::AllowPrivate, mixed.clone()).unwrap(),
            mixed
        );
    }

    #[test]
    fn an_empty_answer_is_refused_in_public_mode() {
        assert!(check_all(Network::Public, Vec::new()).is_err());
    }

    #[test]
    fn public_addresses_pass() {
        let ok = vec![addr("93.184.216.34")];
        assert_eq!(check_all(Network::Public, ok.clone()).unwrap(), ok);
    }
}
