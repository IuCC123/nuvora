//! Client addresses: trusted reverse proxies (`--trusted-proxy`), the client
//! address behind them, and which addresses are local.

use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use axum::extract::ConnectInfo;
use axum::http::{Extensions, HeaderMap};

/// An address or a CIDR block: `10.0.0.5`, `10.0.0.0/8`, `fd00::/8`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpNet {
    addr: IpAddr,
    prefix: u8,
}

#[derive(Debug, thiserror::Error)]
#[error("{0:?} is not an IP address or CIDR block")]
pub struct ParseIpNetError(String);

impl FromStr for IpNet {
    type Err = ParseIpNetError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let error = || ParseIpNetError(s.to_owned());
        let (addr, prefix) = match s.trim().split_once('/') {
            Some((addr, prefix)) => (addr, Some(prefix)),
            None => (s.trim(), None),
        };
        let addr = IpAddr::from_str(addr).map_err(|_| error())?.to_canonical();
        let max = if addr.is_ipv4() { 32 } else { 128 };
        let prefix = match prefix {
            Some(p) => p.parse().ok().filter(|p| *p <= max).ok_or_else(error)?,
            None => max,
        };
        Ok(IpNet { addr, prefix })
    }
}

impl fmt::Display for IpNet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.addr, self.prefix)
    }
}

impl IpNet {
    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.addr, ip.to_canonical()) {
            (IpAddr::V4(net), IpAddr::V4(ip)) => {
                prefix_eq(&net.octets(), &ip.octets(), self.prefix)
            }
            (IpAddr::V6(net), IpAddr::V6(ip)) => {
                prefix_eq(&net.octets(), &ip.octets(), self.prefix)
            }
            _ => false,
        }
    }
}

fn prefix_eq(a: &[u8], b: &[u8], prefix: u8) -> bool {
    let (whole, rest) = (usize::from(prefix / 8), prefix % 8);
    if a[..whole] != b[..whole] {
        return false;
    }
    rest == 0 || (a[whole] ^ b[whole]) >> (8 - rest) == 0
}

/// Loopback, RFC 1918 or IPv6 unique local (fc00::/7).
pub fn is_local(ip: IpAddr) -> bool {
    match ip.to_canonical() {
        IpAddr::V4(ip) => ip.is_loopback() || ip.is_private(),
        IpAddr::V6(ip) => ip.is_loopback() || (ip.segments()[0] & 0xfe00) == 0xfc00,
    }
}

/// The TCP peer, when the server was started with connect info.
pub fn peer(extensions: &Extensions) -> Option<IpAddr> {
    extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip().to_canonical())
}

#[derive(Debug, Clone, Default)]
pub struct TrustedProxies(Vec<IpNet>);

impl TrustedProxies {
    pub fn new(nets: Vec<IpNet>) -> TrustedProxies {
        TrustedProxies(nets)
    }

    pub fn any(&self) -> bool {
        !self.0.is_empty()
    }

    pub fn trusts(&self, ip: IpAddr) -> bool {
        self.0.iter().any(|net| net.contains(ip))
    }

    /// The client's address. From an untrusted peer, the peer. From a trusted
    /// one, the right-most `X-Forwarded-For` hop that is not itself trusted:
    /// hops to its left were written by the client and prove nothing. When
    /// every hop is trusted, the left-most; an unreadable hop ends the walk.
    pub fn client(&self, peer: IpAddr, headers: &HeaderMap) -> IpAddr {
        if !self.trusts(peer) {
            return peer;
        }
        let hops: Vec<&str> = headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(','))
            .collect();
        let mut client = peer;
        for hop in hops.iter().rev() {
            let Some(ip) = parse_hop(hop) else { break };
            client = ip;
            if !self.trusts(ip) {
                break;
            }
        }
        client
    }
}

/// `1.2.3.4`, `1.2.3.4:5678`, `::1` or `[::1]:80`.
fn parse_hop(hop: &str) -> Option<IpAddr> {
    let hop = hop.trim();
    IpAddr::from_str(hop)
        .or_else(|_| SocketAddr::from_str(hop).map(|a| a.ip()))
        .ok()
        .map(|ip| ip.to_canonical())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn nets_parse_and_contain() {
        let net: IpNet = "10.0.0.0/8".parse().unwrap();
        assert!(net.contains(ip("10.200.3.4")));
        assert!(net.contains(ip("::ffff:10.0.0.1")), "IPv4-mapped");
        assert!(!net.contains(ip("11.0.0.1")));
        assert!(!net.contains(ip("::1")));
        let net: IpNet = "172.16.0.0/12".parse().unwrap();
        assert!(net.contains(ip("172.31.255.255")));
        assert!(!net.contains(ip("172.32.0.0")));
        let one: IpNet = " 192.168.1.10 ".parse().unwrap();
        assert_eq!(one.to_string(), "192.168.1.10/32");
        assert!(one.contains(ip("192.168.1.10")) && !one.contains(ip("192.168.1.11")));
        let v6: IpNet = "fd00::/8".parse().unwrap();
        assert!(v6.contains(ip("fd12::1")) && !v6.contains(ip("fe80::1")));
        let all: IpNet = "0.0.0.0/0".parse().unwrap();
        assert!(all.contains(ip("8.8.8.8")));
        for bad in [
            "",
            "abc",
            "10.0.0.0/33",
            "10.0.0.0/",
            "::/129",
            "10.0.0.0/8/8",
        ] {
            assert!(bad.parse::<IpNet>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn local_means_loopback_rfc1918_or_ula() {
        for local in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.0.9",
            "::1",
            "fd00::1",
            "::ffff:192.168.1.2",
        ] {
            assert!(is_local(ip(local)), "{local}");
        }
        for public in [
            "8.8.8.8",
            "172.32.0.1",
            "100.64.0.1",
            "2001:db8::1",
            "fe80::1",
        ] {
            assert!(!is_local(ip(public)), "{public}");
        }
    }

    #[test]
    fn the_client_is_the_right_most_untrusted_hop() {
        let proxies = TrustedProxies::new(vec!["10.0.0.0/8".parse().unwrap()]);
        let forwarded = |values: &[&'static str]| {
            let mut headers = HeaderMap::new();
            for v in values {
                headers.append("x-forwarded-for", HeaderValue::from_static(v));
            }
            headers
        };
        let proxy = ip("10.0.0.2");
        assert_eq!(
            proxies.client(proxy, &HeaderMap::new()),
            proxy,
            "no header: the peer"
        );
        assert_eq!(
            proxies.client(proxy, &forwarded(&["203.0.113.9"])),
            ip("203.0.113.9")
        );
        assert_eq!(
            proxies.client(proxy, &forwarded(&["6.6.6.6, 203.0.113.9, 10.0.0.7"])),
            ip("203.0.113.9"),
            "a spoofed left hop is ignored, a trusted inner proxy skipped"
        );
        assert_eq!(
            proxies.client(proxy, &forwarded(&["6.6.6.6", "203.0.113.9:4711"])),
            ip("203.0.113.9"),
            "repeated headers join in order; ports are dropped"
        );
        assert_eq!(
            proxies.client(proxy, &forwarded(&["10.0.0.9, 10.0.0.8"])),
            ip("10.0.0.9")
        );
        assert_eq!(
            proxies.client(proxy, &forwarded(&["203.0.113.9, junk"])),
            proxy,
            "an unreadable hop ends the walk"
        );
        assert_eq!(
            proxies.client(ip("198.51.100.1"), &forwarded(&["203.0.113.9"])),
            ip("198.51.100.1"),
            "an untrusted peer's header is ignored"
        );
    }
}
