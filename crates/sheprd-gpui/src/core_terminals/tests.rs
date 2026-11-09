mod accessibility;
mod chats;
mod close;
mod commands;
mod context;
mod focus;
mod folders;
mod kept_checkouts;
mod kept_workspaces;
mod keys;
mod menus;
mod open_terminal;
mod pane_keys;
mod paste;
mod phone;
mod refresh;
mod requests;
mod restore;
mod saved_tabs;
mod screen;
mod selection;
mod shown_checkout;
mod split_shown;
mod stale;
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
