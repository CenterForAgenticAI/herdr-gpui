import SwiftUI

/// Where a list's chats, terminals, and changes open when they do not push
/// onto its own stack: in the iPad's sidebar, the detail column beside it.
struct RouteOpener: Sendable {
    /// What the detail column shows, so its row reads as selected.
    let current: Route?
    let open: @MainActor @Sendable (Route) -> Void
}

extension EnvironmentValues {
    /// Set by the iPad's sidebar; `nil` pushes as on a phone.
    @Entry var routeOpener: RouteOpener?
}

/// A row that opens a route: pushed onto the stack, or, in the iPad's
/// sidebar, beside it, highlighted while it is the one shown.
struct RouteLink<Label: View>: View {
    @Environment(\.routeOpener) private var opener
    let route: Route
    let label: Label

    init(_ route: Route, @ViewBuilder label: () -> Label) {
        self.route = route
        self.label = label()
    }

    var body: some View {
        if let opener {
            Button { opener.open(route) } label: {
                HStack(spacing: 0) {
                    label
                    Spacer(minLength: 0)
                }
                .contentShape(.rect)
            }
            .tint(.primary)
            .listRowBackground(opener.current == route ? Color.accentColor.opacity(0.25) : nil)
        } else {
            NavigationLink(value: route) { label }
        }
    }
}
