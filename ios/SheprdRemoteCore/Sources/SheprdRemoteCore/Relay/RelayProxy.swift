import Foundation
import Network

/// Lets the app's own HTTP and WebSocket code reach a computer through a
/// relay unchanged: a listener on this device's loopback interface, where
/// each connection the app opens becomes one `RelaySession` to the
/// computer, bytes carried both ways. Everything that leaves the device is
/// inside the tunnel.
///
/// Another process on this device can connect to a loopback port too; it
/// then reaches the computer's TLS, where everything but its open pairing
/// and ping routes still needs the phone's bearer token. It could hold
/// sessions, so the proxy carries at most `maxSessions` at once, the
/// relay's limit per dial key, and refuses the rest.
public final class RelayProxy: @unchecked Sendable {
    public typealias Open = @Sendable () async throws -> RelaySession

    /// Sessions at once: the relay allows four per dial key.
    public static let maxSessions = 4

    private let listener: NWListener
    private let open: Open
    private let queue = DispatchQueue(label: "sheprd.relay.proxy")
    /// Sessions carried now; touched on `queue` only.
    private var carried = 0

    /// A proxy that opens a session with `open` for each connection.
    public init(open: @escaping Open) throws {
        let parameters = NWParameters.tcp
        parameters.requiredInterfaceType = .loopback
        parameters.requiredLocalEndpoint = NWEndpoint.hostPort(host: "127.0.0.1", port: .any)
        listener = try NWListener(using: parameters)
        self.open = open
    }

    /// Starts listening; the port to send the app's requests to, once
    /// ready.
    public func start() async throws -> UInt16 {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<UInt16, Error>) in
            let once = Once()
            listener.stateUpdateHandler = { [listener] state in
                switch state {
                case .ready:
                    if once.claim(), let port = listener.port?.rawValue { continuation.resume(returning: port) }
                case let .failed(error):
                    if once.claim() { continuation.resume(throwing: error) }
                default:
                    break
                }
            }
            listener.newConnectionHandler = { [weak self] connection in
                self?.serve(connection)
            }
            listener.start(queue: queue)
        }
    }

    public func stop() {
        listener.cancel()
    }

    /// Runs on `queue`, as the listener's handler does.
    private func serve(_ connection: NWConnection) {
        guard carried < Self.maxSessions else {
            connection.cancel()
            return
        }
        carried += 1
        connection.start(queue: queue)
        let open = open
        Task {
            defer { queue.async { self.carried -= 1 } }
            guard let session = try? await open() else {
                connection.cancel()
                return
            }
            await withTaskGroup(of: Void.self) { group in
                group.addTask { await Self.upstream(connection, session) }
                group.addTask { await Self.downstream(session, connection) }
                await group.next()
                group.cancelAll()
            }
            await session.close()
            connection.cancel()
        }
    }

    /// What the app writes goes to the computer.
    private static func upstream(_ connection: NWConnection, _ session: RelaySession) async {
        while let data = await receive(connection) {
            if (try? await session.send(data)) == nil { return }
        }
    }

    /// What the computer sends goes to the app.
    private static func downstream(_ session: RelaySession, _ connection: NWConnection) async {
        while let data = try? await session.receive() {
            let sent: Bool = await withCheckedContinuation { continuation in
                connection.send(content: data, completion: .contentProcessed { error in
                    continuation.resume(returning: error == nil)
                })
            }
            if !sent { return }
        }
    }

    private static func receive(_ connection: NWConnection) async -> Data? {
        await withCheckedContinuation { continuation in
            connection.receive(minimumIncompleteLength: 1, maximumLength: Tunnel.Transport.maxData) { data, _, complete, error in
                if let data, !data.isEmpty {
                    continuation.resume(returning: data)
                } else if complete || error != nil {
                    continuation.resume(returning: nil)
                } else {
                    continuation.resume(returning: Data())
                }
            }
        }
    }
}

/// Resumes a continuation once, whatever the listener reports after.
private final class Once: @unchecked Sendable {
    private var done = false
    private let lock = NSLock()

    func claim() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if done { return false }
        done = true
        return true
    }
}
