mod accessibility;
mod adopt;
mod chats;
mod close;
mod commands;
mod context;
mod control_holder;
mod effects;
mod fallback_bells;
mod focus;
mod folders;
mod images;
mod kept_checkouts;
mod kept_ports;
mod kept_workspaces;
mod keys;
mod links;
mod menus;
mod open_terminal;
mod pane_keys;
mod paste;
mod phone;
mod requests;
mod restore;
mod saved_tabs;
mod screen;
mod selection;
mod settings;
mod shared_names;
mod shown_checkout;
mod split_shown;
mod stale;
mod tab_status;
mod wanted;
mod worker;

impl crate::SheprdWindow {
    /// Stands in for the herdr-core session the window builds each tick.
    pub(crate) fn set_core_session(
        &mut self,
        scope: crate::browser::Scope,
        session: sheprd_session::Session,
    ) {
        self.core_terminals.session = Some((scope, std::sync::Arc::new(session)));
    }
}
