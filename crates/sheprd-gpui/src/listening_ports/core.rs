//! Ports of this computer's herdr-core terminals, listed under the workspace
//! whose tab shows each terminal. A scan names only the terminal a listener
//! was started in; which workspace that is, only the window's tabs know, so
//! the window tells it and the ports are gathered again when either changes.

use super::{Listed, ListeningPorts, Port, scan};
use crate::usage::Host;
use std::collections::HashMap;

/// Each workspace's herdr-core ports, and what they were gathered from.
#[derive(Default)]
pub(super) struct Placed {
    /// Each terminal's workspace, as the window last told it.
    places: Vec<(String, String)>,
    /// The terminals' ports as the scan they were gathered from found them.
    scanned: HashMap<scan::Terminal, Vec<Port>>,
    workspaces: HashMap<String, Vec<Port>>,
}

impl ListeningPorts {
    /// Tells which workspace each herdr-core terminal of this computer is in,
    /// by its address such as `t7`; whether the ports listed changed.
    pub fn place_terminals(&mut self, mut places: Vec<(String, String)>) -> bool {
        places.sort_unstable();
        places.dedup_by(|a, b| a.0 == b.0);
        let empty = HashMap::new();
        let scanned = self
            .monitors
            .get(&Host::Local)
            .map_or(&empty, |monitor| &monitor.reading.scan.terminals);
        if places == self.core.places && *scanned == self.core.scanned {
            return false;
        }
        let workspaces = gather(&places, scanned);
        self.core.scanned = scanned.clone();
        self.core.places = places;
        let changed = workspaces != self.core.workspaces;
        self.core.workspaces = workspaces;
        changed
    }

    /// The ports the herdr-core terminals of `workspace` listen on, lowest
    /// first, and where they open; None while they listen on none.
    pub fn core_ports(&self, workspace: &str) -> Option<Listed<'_>> {
        let ports = self.core.workspaces.get(workspace)?;
        let origin = &self.monitors.get(&Host::Local)?.reading.origin;
        (!ports.is_empty()).then_some(Listed { ports, origin })
    }
}

/// The ports of each workspace's terminals in `places`, one entry per number.
fn gather(
    places: &[(String, String)],
    scanned: &HashMap<scan::Terminal, Vec<Port>>,
) -> HashMap<String, Vec<Port>> {
    let mut workspaces: HashMap<String, Vec<Port>> = HashMap::new();
    for (owner, ports) in scanned {
        let Ok(at) =
            places.binary_search_by(|(terminal, _)| terminal.as_str().cmp(&owner.terminal))
        else {
            continue;
        };
        let list = workspaces.entry(places[at].1.clone()).or_default();
        for port in ports {
            scan::add(list, port.clone());
        }
    }
    for list in workspaces.values_mut() {
        scan::sorted(list);
    }
    workspaces
}
