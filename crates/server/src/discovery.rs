//! Announces this server on the local network over mDNS (Bonjour) as
//! `_nuvora._tcp`, so the TV app lists it without anyone typing an address,
//! and finds it again by `id` after the address changes. The announcement
//! follows the machine's addresses as they change.

use std::net::SocketAddr;

use mdns_sd::{IfKind, ServiceDaemon, ServiceInfo};

const SERVICE_TYPE: &str = "_nuvora._tcp.local.";

pub struct Announcer {
    daemon: ServiceDaemon,
    id: String,
    bind: SocketAddr,
}

impl Announcer {
    /// Starts announcing, unless the server only listens on loopback (nobody
    /// else could connect) or mDNS is unavailable, which only costs discovery.
    pub fn start(id: &str, name: &str, bind: SocketAddr) -> Option<Announcer> {
        if bind.ip().is_loopback() {
            return None;
        }
        let started = ServiceDaemon::new().and_then(|daemon| {
            // TVs connect over IPv4; a link-local IPv6 address is no use to them.
            daemon.disable_interface(IfKind::IPv6)?;
            let announcer = Announcer {
                daemon,
                id: id.to_owned(),
                bind,
            };
            announcer.daemon.register(announcer.service(name)?)?;
            Ok(announcer)
        });
        match started {
            Ok(announcer) => {
                tracing::info!(service = SERVICE_TYPE, "announcing on the local network");
                Some(announcer)
            }
            Err(e) => {
                tracing::warn!(
                    "cannot announce on the local network; TVs must be given the address: {e}"
                );
                None
            }
        }
    }

    /// Announces the new name in place of the old.
    pub fn rename(&self, name: &str) {
        if let Err(e) = self.service(name).and_then(|s| self.daemon.register(s)) {
            tracing::warn!("cannot announce the new server name: {e}");
        }
    }

    /// Says goodbye, so clients drop the server now rather than when the announcement expires.
    pub fn stop(&self) {
        let _ = self.daemon.shutdown();
    }

    fn service(&self, name: &str) -> Result<ServiceInfo, mdns_sd::Error> {
        // Unique on the network whatever the server is called; clients show `name`.
        let instance = format!("nuvora-{}", &self.id[..12]);
        let host = format!("{instance}.local.");
        let version = env!("CARGO_PKG_VERSION");
        let properties = [
            ("id", self.id.as_str()),
            ("name", name),
            ("version", version),
        ];
        let info = match self.bind.ip() {
            ip if ip.is_unspecified() => ServiceInfo::new(
                SERVICE_TYPE,
                &instance,
                &host,
                (),
                self.bind.port(),
                &properties[..],
            )?
            .enable_addr_auto(),
            ip => ServiceInfo::new(
                SERVICE_TYPE,
                &instance,
                &host,
                ip,
                self.bind.port(),
                &properties[..],
            )?,
        };
        Ok(info)
    }
}
