//! Ports of this computer's herdr-core terminals, listed under the workspace
//! whose tab shows each terminal. A scan names only the terminal a listener
//! was started in; which workspace that is, only the window's tabs know, so
//! the window tells it and the ports are gathered again when either changes.

use super::{Daemon, Listed, ListeningPorts, Port, scan};
use crate::{browser::Scope, usage::Host};
use std::collections::HashMap;

/// A workspace by the daemon whose tabs hold it, as tabs remember it: two
/// local daemons may each have a workspace of the same ID.
pub(crate) type Place = (Scope, String);

/// Each workspace's herdr-core ports, and what they were gathered from.
#[derive(Default)]
pub(super) struct Placed {
    /// Each terminal's workspace, as the window last told it.
    places: Vec<(String, Place)>,
    /// The terminals' ports as the scan they were gathered from found them.
    scanned: HashMap<scan::Terminal, Vec<Port>>,
    workspaces: HashMap<Place, Vec<Port>>,
}

impl ListeningPorts {
    /// Tells which workspace each herdr-core terminal of this computer is in,
    /// by its address such as `t7`; whether the ports listed changed.
    pub fn place_terminals(&mut self, mut places: Vec<(String, Place)>) -> bool {
        places.sort_unstable_by(|a, b| a.0.cmp(&b.0));
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

    /// The workspace of `daemon` whose herdr-core terminal started the
    /// listening process `pid` on this computer, as the latest scan found it.
    pub(super) fn core_workspace_of(&self, daemon: &Daemon, pid: u32) -> Option<&str> {
        let owner = self
            .monitors
            .get(&Host::Local)?
            .reading
            .scan
            .terminal_pids
            .get(&pid)?;
        let places = &self.core.places;
        let at = places
            .binary_search_by(|(terminal, _)| terminal.as_str().cmp(&owner.terminal))
            .ok()?;
        let (scope, workspace) = &places[at].1;
        (Some(scope) == daemon.scope().as_ref()).then_some(workspace.as_str())
    }

    /// The ports the herdr-core terminals of `workspace` of the daemon
    /// `scope` names listen on, lowest first, and where they open; None
    /// while they listen on none.
    pub fn core_ports(&self, scope: &Scope, workspace: &str) -> Option<Listed<'_>> {
        let ports = self
            .core
            .workspaces
            .get(&(scope.clone(), workspace.to_owned()))?;
        let origin = &self.monitors.get(&Host::Local)?.reading.origin;
        (!ports.is_empty()).then_some(Listed { ports, origin })
    }
}

/// The ports of each workspace's terminals in `places`, one entry per number.
fn gather(
    places: &[(String, Place)],
    scanned: &HashMap<scan::Terminal, Vec<Port>>,
) -> HashMap<Place, Vec<Port>> {
    let mut workspaces: HashMap<Place, Vec<Port>> = HashMap::new();
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
