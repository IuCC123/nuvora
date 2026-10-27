//! Finds Nuvora servers on the local network: they announce themselves over
//! mDNS as `_nuvora._tcp` (the server's `discovery.rs`), with their id, name
//! and version in the TXT record.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mdns_sd::{IfKind, ServiceDaemon, ServiceEvent};
use serde::Serialize;

const SERVICE_TYPE: &str = "_nuvora._tcp.local.";

#[derive(Clone, Serialize)]
pub struct FoundServer {
    pub id: String,
    pub name: String,
    pub url: String,
}

/// What the network has announced so far, by mDNS instance name.
#[derive(Clone, Default)]
pub struct Discovered(Arc<Mutex<HashMap<String, FoundServer>>>);

impl Discovered {
    /// Browses for as long as the app runs. Without mDNS (blocked by a firewall,
    /// say) the list stays empty and people type the address instead.
    pub fn start(&self) {
        let found = self.0.clone();
        let browse = ServiceDaemon::new().and_then(|daemon| {
            daemon.disable_interface(IfKind::IPv6)?;
            let events = daemon.browse(SERVICE_TYPE)?;
            Ok((daemon, events))
        });
        let Ok((daemon, events)) = browse else { return };
        std::thread::spawn(move || {
            let _daemon = daemon;
            while let Ok(event) = events.recv() {
                match event {
                    ServiceEvent::ServiceResolved(service) => {
                        // A server on this computer also answers on loopback; other devices need its network address.
                        let addresses = service.get_addresses_v4();
                        let ip = addresses
                            .iter()
                            .filter(|ip| !ip.is_loopback())
                            .min()
                            .or(addresses.iter().min());
                        let Some(ip) = ip else { continue };
                        let Some(id) = service.get_property_val_str("id") else {
                            continue;
                        };
                        let name = service.get_property_val_str("name").unwrap_or("Nuvora");
                        let server = FoundServer {
                            id: id.to_owned(),
                            name: name.to_owned(),
                            url: format!("http://{ip}:{}", service.port),
                        };
                        found
                            .lock()
                            .unwrap()
                            .insert(service.fullname.clone(), server);
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        found.lock().unwrap().remove(&fullname);
                    }
                    _ => {}
                }
            }
        });
    }

    pub fn list(&self) -> Vec<FoundServer> {
        let mut servers: Vec<FoundServer> = self.0.lock().unwrap().values().cloned().collect();
        servers.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.url.cmp(&b.url))
        });
        servers
    }

    pub fn by_id(&self, id: &str) -> Option<FoundServer> {
        self.0
            .lock()
            .unwrap()
            .values()
            .find(|s| s.id == id)
            .cloned()
    }
}
