mod accessibility;
mod chats;
mod context;
mod effects;
mod fallback_bells;
mod focus;
mod folders;
mod kept_checkouts;
mod kept_workspaces;
mod keys;
mod links;
mod open_terminal;
mod paste;
mod phone;
mod requests;
mod saved_tabs;
mod screen;
mod selection;
mod settings;
mod shown_checkout;
mod split_shown;
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
