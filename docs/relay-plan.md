# Plan: Sheprd's own relay, replacing Tailscale

Today a phone reaches a computer's `sheprd-remote-control` daemon only on the
same network or over Tailscale, and Sheprd finds SSH machines through
`tailscale status`. Users without Tailscale lose both away from home. This
plan adds `sheprd-relay`: a small service we and anyone else can run, which
joins two Sheprd peers when they have no direct path, while seeing only
ciphertext. Read `AGENTS.md` first; its rules apply throughout.

Direct paths stay first. The relay is the fallback that makes a connection
work from any network, not a detour for traffic that has a better route.

## Decisions

| Question | Decision |
| --- | --- |
| What the relay is | A blind pipe: it joins two byte streams and counts bytes. It never holds a key that opens them. |
| Transport | QUIC on UDP 443 (quinn; `NWConnection` with QUIC on iOS): one connection per peer, a stream per session, no head-of-line blocking between sessions, and connection migration when a phone moves between Wi-Fi and cellular. A few fixed-size binary headers, then raw bytes; no protobuf or other codegen. WebSockets over HTTPS on TCP 443 stay as the fallback for networks that block UDP, so it always works. |
| Who can listen | Whoever proves a P-256 key. The route is SHA-256 of the key, so nobody can take over another computer's route. There are no accounts and nothing to authorize on the relay; an operator may set one optional token to keep a relay to their own computers. |
| Who can dial | Only devices the computer names. A computer gives the relay the public dial keys of its paired devices; any other dial hears `offline`, as if the computer were not there, and never reaches it. Revoking a device ends its sessions at once. |
| End-to-end encryption | Hybrid post-quantum Noise inside the stream (below). TLS to the relay protects only metadata. |
| State | None. Memory only; a restart drops sessions and listeners reconnect. No database, no volume. |
| Many relays | Relays share nothing. A computer listens on every relay it knows, and a client dials whichever answers first. Adding a region is starting a droplet. |
| Choosing a relay | Sheprd's relays by default, or the user's own: any number of private endpoints, which can replace ours entirely (below). |
| Zero config | Every setting has a safe default. The image runs with no settings; `deploy/relay` serves it at `IP.sslip.io` with a Let's Encrypt certificate and no DNS work. |
| Operator safety | Limits on connections (total and per address), listeners, sessions (total and per route), frame size, bandwidth (per session and total), idle and total session time, and dial and listen rates. |
| Telemetry | Prometheus at `/metrics` on its own port, and a read-only dashboard behind a login. Neither carries an address, a full route, or a byte of payload. |

### Still open

These change client work only; the relay is the same either way.

1. **Scope.** This plan covers both phone to daemon and Sheprd to SSH
   machines, phased. Phone to daemon comes first.
2. **One tunnel implementation or two.** Recommended: one Rust crate,
   `sheprd-tunnel`, used by the daemon and Sheprd, and by the iOS app through
   a small UniFFI framework, so the handshake is written and tested once.
   The other option is a Swift copy checked against shared test vectors.

## Architecture

```text
phone ──QUIC──▶ relay ◀──QUIC── computer (sheprd-remote-control, listening)
   └────── Noise IK + ML-KEM-768, end to end ──────┘

Sheprd ──QUIC──▶ relay ◀──QUIC── SSH machine (sheprd-relay-agent → 127.0.0.1:22)
   └──── Noise, then SSH's own encryption inside it ───┘

Either side may use a WebSocket over HTTPS instead, when UDP is blocked.
```

### The relay (`crates/sheprd-relay`, built)

Tokio, quinn, and axum. No GPUI, and no dependency on the client crates: it
knows streams, routes, and limits, nothing else.

#### QUIC (`wire`, `quic`)

ALPN `sheprd-relay/1`, TLS 1.3. Every stream a peer opens starts with one
request; the relay answers with one status byte. Keys are SEC1 compressed
P-256 (33 bytes), signatures `r ‖ s` (64).

| Request | Bytes |
| --- | --- |
| Listen | `0x01`, key, signature, token length (1), token (0–255) |
| Dial | `0x02`, route (32), dial key, signature |

A listener then says who may dial it, on its listen stream:

| Message | Bytes |
| --- | --- |
| Dial keys | `0x20`, count (1, at most 64), keys |
| Pairing key | `0x21`, key, lifetime in seconds (2, big-endian, at most 600) |

Statuses: `0` ok, `1` offline, `2` busy, `3` timeout, `4` limited,
`5` refused (bad proof), `6` forbidden (token), `7` full, `8` malformed,
`9` too many (pairing keys).

- **Proofs.** Listeners sign `"sheprd-relay/listen/quic/v1\0"`, dialers
  `"sheprd-relay/dial/quic/v1\0" ‖ … ‖ route`, each over 32 bytes exported
  from their own connection's TLS session (RFC 5705, label
  `EXPORTER-sheprd-relay-listen`). The export differs on every connection,
  so a proof cannot be replayed or passed through another relay, at no
  extra round trip.
- **Listening.** The listen stream stays open; closing it stops listening,
  and a newer listener for the same key resets it with code 5. Until it
  sends dial keys, no one can reach it.
- **Dialing.** Only a key the listener names, or an unexpired pairing key
  (used up by its one dial), gets through. Any other key gets `offline`,
  exactly as if no one listened. After `ok`, the stream is the session:
  raw bytes both ways. A phone holds one connection and opens a stream per
  computer it reaches.
- **Accepting.** For each dial, the relay opens a stream on the listener's
  own connection, starting with `0x10` and the dialer's key, so the
  computer knows which device calls before the end-to-end handshake. No
  new connection, no new handshake.
- **Revoking.** New dial keys replace the old set, and the relay resets
  every open session of a key left out (code 7) at once. Sessions opened
  with a pairing key are left alone, since approving a device replaces the
  keys while its pairing session is still open; they last 10 minutes at
  most.
- **Trust.** The relay makes a fresh key and self-signed certificate at
  each start, so it needs no volume and shares no file with the HTTPS
  proxy. `/v1/info`, read over ordinary HTTPS, gives the UDP port and the
  certificate's SHA-256, which clients pin; a client whose handshake fails
  reads it again. The end-to-end handshake inside the stream is what
  protects content; the pin protects the listen token and metadata.
- **Limits.** Connections count against the per-address limit before the
  handshake, so a flood costs no cryptography. 64 streams per connection;
  8 MiB stream and 32 MiB connection windows, so one session can fill a
  long, fast path; a 60-second idle timeout that clients' 15-second QUIC
  keepalives stay under.

#### WebSocket fallback and HTTP

| Route | What it does |
| --- | --- |
| `GET /v1/listen` | A computer's control socket. The relay sends `{"t":"challenge","nonce","version":1}`; the computer answers `{"key","signature","token"?}` over `"sheprd-relay/listen/v1\0" ‖ Host ‖ 0x00 ‖ nonce`. Binding the Host stops one relay passing another's challenge through. The relay answers `{"t":"ready","route"}`. The computer sends `{"t":"dial_keys","keys"}` and `{"t":"pairing_key","key","seconds"}`, each answered `{"t":"status","status"}`, and hears `{"t":"incoming","session","key"}` for each dial. A newer listener with the same key replaces the older one, which is closed with 4409. |
| `GET /v1/dial/{route}` | The same challenge; the device answers `{"key","signature"}` over `"sheprd-relay/dial/v1\0" ‖ Host ‖ 0x00 ‖ nonce ‖ route`, then hears `{"t":"status","status"}` with the same statuses as QUIC. After `ok`, binary frames are the session. |
| `GET /v1/accept/{session}` | The listener's socket for one dial. The session id is 128 random bits that only the listener learns, and it can be claimed once. |
| `GET /v1/info` | `{"protocol":1,"version","listen_token":bool,"quic":{"port","pin"},"websocket":bool}`, so a client can check an endpoint, know to ask for a token, and reach QUIC, before saving it. |
| `GET /healthz` | For uptime checks. |
| `/dashboard` | The read-only dashboard, when a password is set. |

WebSocket proofs cover the relay's own name, `RELAY_PUBLIC_HOST` (set from
the domain by `deploy/relay`), which the relay checks against its
configuration, never against the request's `Host`: a client sets that
header, so a malicious relay could pass another relay's challenge to a
computer and then present the proof with its own name in `Host`. Without a
public name the fallback is off and `/v1/info` says `"websocket": false`;
QUIC, whose proofs bind the TLS session itself, needs no name.

Data is binary frames only. A text frame ends the session (4400), and a
frame over `max-frame` breaks it. A revoked key's session closes with
4403. Close codes are in `splice::code`; the messages are in `socket`.

A session may mix transports: a phone on QUIC reaches a computer that could
only open a WebSocket, and the other way round.

The relay pings idle listeners every 25 seconds, which keeps NAT mappings
open. Inside a session, keepalives are the peers' job: they are encrypted,
so the relay cannot tell a keepalive from data.

### Limits

All settings are flags or `RELAY_*` variables (`sheprd-relay --help`).

| Setting | Default | Bounds |
| --- | --- | --- |
| `max-connections` | 10,000 | Open sockets of every kind |
| `max-connections-per-ip` | 64 | Per IPv4 address or IPv6 /64 |
| `max-listeners` | 5,000 | Routes with a listener |
| `max-sessions` | 5,000 | Sessions and waiting dials |
| `max-sessions-per-route` | 32 | Per computer |
| `session-bandwidth` | 4 MiB/s | Each way, per session; 0 is unlimited |
| `total-bandwidth` | unlimited | Each way, across sessions |
| `max-frame` | 64 KiB | Largest frame |
| `idle-timeout` | 5 min | Silent session |
| `max-session-duration` | 24 h | Any session |
| `accept-timeout` | 10 s | Listener's time to accept |
| `dials-per-minute` | 120 | Per address, a quarter of it at once |
| `listens-per-minute` | 30 | Per address |

Throttles are token buckets that let one second of traffic through at once
and then charge time, so interactive sessions stay snappy while a bulk
transfer is held to its rate. Each direction has its own bucket, so a
throttled download never stalls typing.

### Telemetry

`/metrics` listens on its own port (default `127.0.0.1:9090`; `off` turns it
off), optionally behind `RELAY_METRICS_TOKEN`. Series:

- `sheprd_relay_connections{kind}`, `sheprd_relay_listeners`,
  `sheprd_relay_sessions`, `sheprd_relay_pending_dials` (gauges)
- `sheprd_relay_bytes_total{direction}`, `sheprd_relay_frames_total{direction}`
- `sheprd_relay_dials_total{outcome}`, `sheprd_relay_listens_total{outcome}`
- `sheprd_relay_refused_connections_total{limit}`
- `sheprd_relay_sessions_ended_total{reason}`,
  `sheprd_relay_session_duration_seconds` (histogram)
- `sheprd_relay_throttled_seconds_total{limit}`
- `sheprd_relay_dashboard_logins_total{outcome}`
- `sheprd_relay_build_info{version}`, `sheprd_relay_start_time_seconds`

`sheprd_relay_dials_total{outcome="unauthorized"}` counts dials with a key
the computer did not name; peers were told `offline`.

The dashboard (`/dashboard`) is on only when `RELAY_DASHBOARD_PASSWORD` (or
`--dashboard-password-file`) is set, at least 12 characters. It shows
sessions, listeners, connections, throughput over ten minutes, bytes
carried, refusals, dial and listen outcomes, and the busiest sessions by
the first eight characters of their route. It changes nothing.

- One account. The password is compared in constant time, and logins are
  limited to 10 a minute per address and 60 a minute overall.
- The session cookie is HMAC-signed with a key drawn at start, lasts
  12 hours, and is `HttpOnly; Secure; SameSite=Strict; Path=/dashboard`.
- The page has a strict CSP (no inline script or style), `frame-ancestors
  'none'`, and `no-store`. All values go in through `textContent`.

### Logs

One line at start (addresses, limits, whether the dashboard is on) and one
per failed dashboard login. No addresses, routes, sessions, or payloads.

## Choosing a relay

Anyone can run a private relay (`deploy/relay`) and point Sheprd at it.
The endpoint is the only thing a user has to type, and only on the
computer: phones learn it from pairing.

**Endpoints.** `sheprd_relay::Endpoint` parses what a user types and builds
every relay URL from it (built; clients use the crate without its `server`
feature, so they get no Tokio or axum).

- `relay.example.org`, `relay.example.org:8443`, `https://…`, and `wss://…`
  are all accepted and stored as `https://HOST[:PORT][/PREFIX]`. A path
  prefix serves a relay behind a proxy that strips it.
- Plain `http://` or `ws://` is refused except for loopback, private and
  link-local addresses, and `.local` names. Content is end-to-end encrypted
  either way, but plain HTTP would expose the listen token and metadata.
- User names, passwords, queries, and fragments are refused, so a token can
  never end up in a URL, a log, or a QR code.

**The computer's setting** (`[remote]`, with Settings > Remote control >
Relays and `sheprd remote relays` beside it):

```toml
[remote]
relays = "default"   # Sheprd's relays (the default). Or one of:
# relays = "off"                                     # LAN and Tailscale only
# relays = ["https://relay.example.org"]             # only these; ours are never contacted
# relays = ["default", "https://relay.example.org"]  # both
```

- A list replaces the defaults unless it names `"default"`, so someone
  running a private relay never touches ours, not even to look one up.
- Each endpoint may have a listen token. Tokens are secrets, so they are not
  in the config: the daemon keeps them in `STATE_DIR/relay/tokens.json`
  (0600), keyed by endpoint, set through Settings or
  `sheprd remote relays token ENDPOINT` (read from stdin).
- Adding an endpoint calls `/v1/info` first. Settings refuses a relay that
  does not answer or speaks another protocol version, and asks for the
  token when `listen_token` is true. A relay that later goes away shows as
  unreachable in Settings and `sheprd remote status`; the others keep
  working.
- Changing the list restarts only the listen sockets, not the daemon.

**The phone.** Pairing (QR, Nearby, Bonjour) carries the computer's
endpoints, and every connection refreshes them, so editing the list on the
computer reaches each phone the next time it connects by any path. Dialers
never need a token. Settings > Computers > a computer lists its relays, and
"Add relay" lets a user type an endpoint for a computer paired before it
had one.

**Sheprd to SSH machines.** `sheprd-relay-agent --relay ENDPOINT
[--token-file FILE]`, repeatable, and Devices lets the user pick which of
the computer's relays to dial.

**What is not built yet.** The `[remote] relays` setting, the token store,
Settings, the command, and the phone's side arrive with the daemon and iOS
work (phase 3). Until then no client reads a relay endpoint, so none of
these is exposed as if it worked.

## Many users, kept apart

One relay serves many people's computers and phones. It keeps them apart
without accounts, so a public relay holds nothing worth stealing, and
each layer stands on its own:

| Layer | What stops another user |
| --- | --- |
| Routes | A route is the SHA-256 of the computer's relay key. Only that key's holder can listen on it; a second listener must prove the same key, and then replaces the first. |
| Dial keys | A computer names the public keys allowed to dial it (one per paired device, per relay, at most 64). Anyone else hears `offline`, the same answer as an empty route, so a stranger who learns a route cannot even tell whether its computer is up, and cannot spend its session slots. |
| End-to-end | Inside the stream, the Noise handshake proves both devices' static keys, pinned at pairing. Even a dial key stolen from a phone does not get past the computer without that phone's end-to-end key. |
| Limits | Per address, per route (32 sessions), and per session (bandwidth), so one user's traffic cannot starve another's. |

A device uses a different dial key on each relay, so relays run by
different people cannot match up the same phone. What a relay does learn:
addresses, timing, sizes, and which pseudonymous keys reach which route.

**Nothing to authorize on the relay.** The relay has no users, accounts,
or lists to keep. Who may reach a computer is the computer's own choice,
made when a device pairs, and the relay holds that choice only while the
computer is connected. One optional token (`RELAY_LISTEN_TOKEN`, off by
default) keeps a relay to its owner's computers; dialers never need one.

### Keys drop themselves

Nothing the relay knows outlives the connection it came on, and nothing
is written to disk, so no one ever has to clean up:

| State | Dropped when |
| --- | --- |
| A computer's route and dial keys | Its connection ends, or goes silent for 60 seconds (QUIC's idle timeout; the WebSocket fallback's missed pings). On reconnecting it names its keys again. |
| A pairing key | Its one dial, or its lifetime (the QR's two minutes), whichever comes first. |
| A device's dial key | The computer stops naming it: when the device is revoked, and on its own when the device has not connected for `device_idle_days` (30 by default, `docs/remote-control-plan.md`). Its open sessions end at once. |
| Sessions | Either side leaving, 5 minutes idle, or 24 hours. |
| Rate limits | They refill, and restart empty. |
| Everything | A restart. Computers reconnect and name their keys again within seconds. |

## Pairing a new device

Pairing works the same in the room or from far away: the QR is scanned
from the computer's screen, and everything else goes through the relay
when there is no direct path. The relay can carry pairing but cannot pair
anyone: it never sees the QR.

### What the QR holds

The existing remote control QR (`docs/remote-control-plan.md`, Pairing)
keeps its direct addresses and CA fingerprint, and gains, in its `#`
fragment so no server logs it:

| Field | Size | Why |
| --- | --- | --- |
| Pairing secret `S` | 32 bytes | Everything else is derived from it; valid 2 minutes, single use |
| Route | 32 bytes | Where the computer listens |
| Relays | 1–3 endpoints | Where to dial |
| Computer static key | 32 bytes (X25519) | Who the phone is talking to |
| Bundle fingerprint | 32 bytes | SHA-256 of the computer's full identity: X25519 key, ML-KEM-768 key, CA |

The ML-KEM key (1,184 bytes) would make the QR too dense to scan off a
screen, so the QR carries its fingerprint and the key arrives inside the
pairing channel.

### Steps

1. **The computer** makes `S` and derives a **pairing dial key** from it
   (`HKDF(S, "sheprd pairing dial key")` as a P-256 scalar). It registers
   that key on each of its relays as a pairing key, valid for the QR's two
   minutes and one dial.
2. **The phone** scans the QR, derives the same pairing dial key, and dials
   the route. The relay lets that one dial through and spends the key. A
   photo of the QR taken later is useless: the key is gone, and expired.
3. **The pairing handshake** runs inside the stream: Noise `IKpsk2` with
   the computer's static key from the QR and a PSK derived from `S`. Only
   someone who saw the QR can complete it, and because the PSK is
   symmetric, a recording of it stays closed even to a future quantum
   computer.
4. **The phone sends its claim**: its device key (Secure Enclave P-256, as
   today), its own X25519 and ML-KEM-768 keys, a dial key for each relay,
   and its name. The computer sends its full identity bundle, and the phone
   checks it against the fingerprint in the QR.
5. **A person approves on both screens**, as today: both show the same
   6-digit code, derived from the handshake transcript, and Sheprd asks
   "*iPhone* wants remote control of your chats. Code 428 913. Allow / Deny".
   Nothing is granted before Allow. Deny or two minutes without an answer
   closes the session.
6. **On Allow**, the computer stores the device and adds the phone's dial
   keys to its listeners on each relay. The pairing session then closes;
   from now on the phone dials with its own key and runs the normal
   hybrid handshake (Noise IK with ML-KEM-768, under End-to-end
   encryption).
7. **Other machines.** When the computer also pairs the phone with its SSH
   and cloud machines (`docs/remote-control-plan.md`, Computers), it sends
   each one the phone's keys over its existing SSH connection, and each
   adds the phone's dial key to its own listeners.

Nearby (Bluetooth) and same-network (Bonjour) pairing keep their own
commitment-based codes; after approval they hand over the same relay
data inside their channel, so a phone paired in the room also works away
from it.

### Revoking and losing a phone

- **Revoking a device** in Sheprd removes its dial keys from every relay
  the computer listens on; a device idle past `device_idle_days` is dropped
  the same way, with no one asking. The relay ends its open sessions at once, and
  its next dial hears `offline`. The computer also forgets its end-to-end
  keys, so a relay that ignored the change would still get nowhere.
- **A lost phone** is revoked the same way, from the computer or from
  another paired device.
- **A leaked relay key** (the computer's) lets someone listen on its route,
  but no paired device would complete the end-to-end handshake with them.
  Sheprd can rotate it: new key, new route, sent to each paired device the
  next time it connects by any path.

## End-to-end encryption

The relay never sees plaintext, and a relay that is taken over can drop or
delay traffic but cannot read it, change it, or pose as either side.

**Handshake: Noise `IKpsk2`, hybrid with ML-KEM-768.**

- `Noise_IKpsk2_25519_ChaChaPoly_SHA256`. IK fits: the dialer already knows
  the computer's static key from pairing, so the first message is encrypted
  and the computer learns who is calling in one round trip.
- The dialer encapsulates to the computer's static **ML-KEM-768** key, also
  learned at pairing, and sends the ciphertext in the first message's
  payload. The shared secret is the Noise PSK, mixed in at the end of the
  second message. The session is secure if either X25519 or ML-KEM-768
  holds, the pattern Rosenpass uses to protect WireGuard and the reason
  "harvest now, decrypt later" recordings stay closed.
- The prologue binds the protocol version, the relay route, and both
  parties' fingerprints, so a stream cannot be replayed onto another route.
- Static keys are pinned at pairing, beside the existing CA fingerprint. A
  computer accepts only paired devices' keys, and the phone accepts only the
  key it paired with. An unknown key is closed after the first message,
  before any application byte.
- After the handshake: ChaCha20-Poly1305 with 64-bit counters, a rekey every
  2^20 messages or ten minutes, and records padded to buckets of 256 bytes so
  keystrokes are harder to tell apart from sizes.
- Implementations: `snow` for Noise, RustCrypto `ml-kem` for the KEM, both
  maintained and audited. On iOS 26, CryptoKit has X25519, ChaChaPoly, and
  `MLKEM768` (see the open question on one implementation).

**What it protects against.** A relay or network attacker sees addresses,
timing, and padded sizes. It cannot read prompts, output, tokens, or file
names, cannot inject or reorder records, and cannot complete a handshake
with either side.

## Clients

### Daemon (`sheprd-remote-control`)

- Unless `[remote] relays = "off"`, the daemon keeps one QUIC connection
  and listen stream open on each endpoint in its list (Choosing a relay),
  using its relay key (`STATE_DIR/relay/key.pem`, 0600). When QUIC cannot
  connect within 3 s it listens over the WebSocket fallback instead, and
  tries QUIC again every few minutes. Reconnects back off from 1 s to 60 s
  with jitter, so a relay restart does not cause a stampede.
- Each accepted stream runs the Noise responder, then serves the existing
  axum router over the decrypted stream with `hyper`'s
  `serve_connection`, so the native API, WebSocket, and bearer tokens are
  unchanged.
- `sheprd remote status --json` reports each relay: listening, last error.

### iOS app and other apps

- Pairing (QR, Nearby, Bonjour) adds the computer's route, relay list, Noise
  static key, and ML-KEM key to the computer record.
- The connection race (`docs/ios-app-plan.md`, Connectivity) gains relay
  candidates, started 750 ms after the direct ones, so a reachable direct
  address wins and the relay costs nothing on a LAN. For each relay, QUIC
  goes first and the WebSocket fallback 300 ms later, the way browsers race
  HTTP/3 against HTTP/2.
- One QUIC connection per relay carries every session to every computer
  on it, and survives moving between Wi-Fi and cellular without a new
  handshake.
- `OriginKind` gains a relay kind, so Settings and the reconnecting banner
  can say "Through relay (fra1)".
- The share extension, widgets, and the watch app use `SheprdRemoteCore`
  and get the same path.
- The PWA cannot use the relay: a browser cannot run the handshake inside a
  WebSocket and then treat it as an origin. It keeps LAN and Tailscale.

### Sheprd and SSH machines

- `sheprd-relay-agent`, a small listener on the SSH machine, accepts streams
  and forwards each, after the Noise handshake, to `127.0.0.1:22`. Each SSH
  session is one QUIC stream, so several share a connection.
- Sheprd connects with `ssh -o ProxyCommand='sheprd relay connect ROUTE'`,
  so SSH keys, host keys, and `known_hosts` work as before.
- Devices > Discover gains a "Through relay" source beside Tailscale, which
  stays supported for people who have it.

## Deployment

`crates/sheprd-relay/Dockerfile` builds a static musl binary into a
distroless `nonroot` image. `deploy/relay` holds the rest:

- `compose.yml`: the relay and Caddy, which gets the certificate. The relay
  runs read-only with every capability dropped, and `/metrics` is published
  on `127.0.0.1` only.
- `cloud-init.yml`: DigitalOcean user data for Ubuntu 24.04. Installs
  Docker, allows only 22, 80, and 443 through `ufw`, makes a dashboard
  password, and starts the relay at `IP.sslip.io` unless a domain is given.
- `create-droplet.sh REGION...`: one droplet per region with `doctl`.
- `app.yaml`: DigitalOcean App Platform, with no server: WebSockets only,
  since App Platform has no UDP. The relay listens on `PORT` and reads
  callers' addresses from `do-connecting-ip` (`RELAY_CLIENT_IP_HEADER`).

**Image.** `.github/workflows/relay-image.yml` builds amd64 and arm64 on
native runners through `scripts/relay-image.sh`, which also checks that the
image starts read-only and answers `/v1/info`. Pull requests build and
check; `main` pushes each architecture by digest, joins them under
`ghcr.io/<owner>/sheprd-relay:latest` and `:sha-<commit>`, signs with cosign
keyless (the version `release.yml` pins), and attests build provenance. CI
holds no relay secrets and deploys nothing.

### Before the first public relay

- Make the GHCR package public once, and pin the Dockerfile's base images by
  digest.
- Load-test a 1 GB droplet: listeners held, sessions, and throughput at the
  default limits, measured rather than estimated.

## Phases

1. **Relay** (this change): the service, its limits, metrics, dashboard,
   image, and DigitalOcean deployment, with tests over real sockets;
   `Endpoint` with `/v1/info`, for choosing a private relay; QUIC with the
   WebSocket fallback; and dial keys, pairing keys, and revocation, all held
   only while their computer is connected.
2. **Tunnel crate**: `sheprd-tunnel` with the handshake, records, rekeying,
   and padding; test vectors; fuzzing the record parser.
3. **Daemon and iOS**: listening and naming dial keys, the `relays`
   setting and token store, `sheprd remote relays`, the pairing flow above
   (QR fields, pairing keys, the `IKpsk2` handshake, approval), revocation,
   the connection race, status, and settings text. Live tests on a phone over cellular with Wi-Fi off.
4. **SSH machines**: `sheprd-relay-agent`, `sheprd relay connect`, Devices.
5. **Our relays**: image publishing, a few regions, a Prometheus scrape
   and alerts on refusals and bandwidth.
