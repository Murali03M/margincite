use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use url::Url;

#[derive(Debug, PartialEq, Eq)]
pub struct Blocked;

pub fn check_ip(ip: IpAddr) -> Result<(), Blocked> {
    if is_blocked(ip) {
        Err(Blocked)
    } else {
        Ok(())
    }
}

pub fn check_url(url: &Url) -> Result<(), Blocked> {
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(Blocked);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(Blocked);
    }
    if let Some(ip) = host_ip(url) {
        check_ip(ip)?;
    }
    Ok(())
}

fn host_ip(url: &Url) -> Option<IpAddr> {
    match url.host()? {
        url::Host::Ipv4(ip) => Some(IpAddr::V4(ip)),
        url::Host::Ipv6(ip) => Some(IpAddr::V6(ip)),
        url::Host::Domain(_) => None,
    }
}

fn is_blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => blocked_v4(ip),
        IpAddr::V6(ip) => blocked_v6(ip),
    }
}

fn blocked_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_link_local()
        || a == 10
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || (a == 100 && (64..=127).contains(&b))
        || a >= 240
        || (a == 198 && (b == 18 || b == 19))
        || (a == 192 && b == 0 && (c == 0 || c == 2))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
}

fn blocked_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return blocked_v4(v4);
    }
    let segments = ip.segments();
    let first = segments[0];
    let nat64 = first == 0x64 && segments[1] == 0xff9b && segments[2..6] == [0; 4];
    let v4_compatible = segments[..6] == [0; 6];
    nat64
        || v4_compatible
        || first == 0x2002
        || ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (first & 0xfe00) == 0xfc00
        || (first & 0xffc0) == 0xfe80
        || (first == 0x2001 && ip.segments()[1] == 0x0db8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_internal_v4() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "240.0.0.1",
            "255.255.255.254",
            "198.18.0.1",
            "198.19.255.255",
            "192.0.0.8",
        ] {
            assert!(check_ip(ip.parse().unwrap()).is_err(), "{ip}");
        }
    }

    #[test]
    fn refuses_internal_v6() {
        for ip in [
            "::1",
            "fc00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "64:ff9b::7f00:1",
            "64:ff9b::a9fe:a9fe",
            "::7f00:1",
            "::a9fe:a9fe",
            "2002:7f00:1::1",
            "2002:a9fe:a9fe::1",
        ] {
            assert!(check_ip(ip.parse().unwrap()).is_err(), "{ip}");
        }
    }

    #[test]
    fn allows_public() {
        assert!(check_ip("93.184.216.34".parse().unwrap()).is_ok());
        assert!(check_ip("198.20.0.1".parse().unwrap()).is_ok());
        assert!(check_ip("192.0.1.1".parse().unwrap()).is_ok());
        assert!(check_ip("2001:4860:4860::8888".parse().unwrap()).is_ok());
        assert!(check_ip("2606:2800:220:1::1".parse().unwrap()).is_ok());
    }

    #[test]
    fn refuses_bad_urls() {
        for raw in [
            "ftp://x",
            "file:///tmp/x",
            "http://user:pw@x.test",
            "http://127.0.0.1/",
        ] {
            let url = Url::parse(raw).unwrap();
            assert!(check_url(&url).is_err(), "{raw}");
        }
    }
}
