mod accessibility;
mod adopt;
mod chats;
mod clear;
mod close;
mod commands;
mod context;
mod control_holder;
mod copy_mode;
mod copy_mode_relink;
mod effects;
mod fallback_bells;
mod focus;
mod folders;
mod history;
mod images;
mod kept_checkouts;
mod kept_ports;
mod kept_workspaces;
mod keys;
mod links;
mod menus;
mod moves;
mod mutes;
mod open_terminal;
mod pane_keys;
mod paste;
mod phone;
mod processes;
mod quote;
mod refresh;
mod requests;
mod resize;
mod restore;
mod saved_tabs;
mod screen;
mod selection;
mod settings;
mod shared_names;
mod shown_checkout;
mod slide;
mod split_shown;
mod stale;
mod tab_status;
mod unended;
mod wanted;
mod worker;
mod zoom_drawn;

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
