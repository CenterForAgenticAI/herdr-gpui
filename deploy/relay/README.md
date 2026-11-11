# Running a Sheprd relay

`sheprd-relay` joins Sheprd peers that have no direct path. It carries only
end-to-end encrypted bytes, keeps no state, and needs no settings.
`docs/relay-plan.md` has the design.

## The image

CI (`.github/workflows/relay-image.yml`) builds the image natively for amd64
and arm64, checks that each starts and answers, and on every change to `main`
publishes one multi-architecture image, signed with cosign and with build
provenance:

```text
ghcr.io/sheprd-labs/sheprd-relay:latest
ghcr.io/sheprd-labs/sheprd-relay:sha-COMMIT
```

Docker picks the right architecture, so the same name works on regular and
ARM droplets. Before the first deploy, make the package public once (GitHub >
the organization > Packages > sheprd-relay > Package settings > Change
visibility), or droplets and App Platform cannot pull it without
credentials. To pin and verify the digest the workflow's summary prints:

```sh
cosign verify ghcr.io/sheprd-labs/sheprd-relay@sha256:… \
  --certificate-identity-regexp 'https://github.com/sheprd-labs/sheprd/.github/workflows/relay-image.yml@refs/heads/main' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

`scripts/relay-image.sh amd64` builds and checks it locally, as CI does.

## DigitalOcean droplets, one command (QUIC and WebSockets)

```sh
doctl auth init
deploy/relay/create-droplet.sh nyc3 fra1 sgp1
```

Each region gets a `s-1vcpu-1gb` droplet tagged `sheprd-relay`, with only
ports 22, 80, and 443 open. A few minutes later the relay answers at
`https://IP-WITH-DASHES.sslip.io` (for example `203-0-113-7.sslip.io`), with a
Let's Encrypt certificate. The dashboard login is in
`/root/sheprd-relay-login.txt` on the droplet.

To use your own names, set `RELAY_DOMAIN_SUFFIX=relay.example.org`, then point
`nyc3.relay.example.org` and the rest at the droplets. Set `RELAY_SSH_KEYS` to
your key ids from `doctl compute ssh-key list` to reach them over SSH.

## DigitalOcean App Platform (WebSockets only)

No server to manage:

```sh
doctl apps create --spec deploy/relay/app.yaml
```

App Platform carries HTTP but not UDP, so the relay runs there without QUIC,
and clients use its WebSocket fallback: slower to connect and with no
connection migration, but it works from any network. The spec wires up what
the platform provides (below); add `RELAY_DASHBOARD_PASSWORD` as an encrypted
variable in the app's settings for the dashboard.

## Ports and names from outside

A container cannot see which ports the host publishes it on, so the relay is
told what the outside looks like. Each deploy sets these for you:

| Variable | What it says | Droplet (compose) | App Platform |
| --- | --- | --- | --- |
| `PORT` | The HTTP port to listen on, when the platform picks it; `RELAY_LISTEN` wins | not set (8080) | set by the platform |
| `RELAY_QUIC_PORT` | The UDP port clients reach QUIC on, when a mapping changes it | `443` (published to the container's 8443) | QUIC is off |
| `RELAY_PUBLIC_HOST` | The name clients use, which WebSocket proofs must cover | `RELAY_DOMAIN` | `${APP_DOMAIN}` |
| `RELAY_BEHIND_PROXY` | Take callers' addresses from `X-Forwarded-For` | `1` (Caddy) | not set |
| `RELAY_CLIENT_IP_HEADER` | Take them from this header instead | not set | `do-connecting-ip` |

Callers' real addresses matter: limits are per address, so behind a proxy
without either setting, every caller would share the proxy's.

## Any host with Docker

```sh
cd deploy/relay
cp env.example .env    # set RELAY_DASHBOARD_PASSWORD for the dashboard
RELAY_DOMAIN=relay.example.org docker compose up -d
```

Or just the image, with no proxy and no TLS, for a trial:

```sh
docker run --rm -p 8080:8080 -p 8443:8443/udp \
  -e RELAY_DASHBOARD_PASSWORD='at least 12 chars' \
  ghcr.io/sheprd-labs/sheprd-relay:latest
```

The dashboard cookie is `Secure`, so outside `localhost` sign in over HTTPS.

That trial serves QUIC only. The WebSocket fallback, for networks that block
UDP, needs the relay's public name, which proofs are checked against:
`-e RELAY_PUBLIC_HOST=relay.example.org`. The compose file sets it from
`RELAY_DOMAIN`.

## Limits

Defaults suit a 1 GB droplet. Change them in `.env`; `sheprd-relay --help`
lists every one.

| Variable | Default |
| --- | --- |
| `RELAY_MAX_CONNECTIONS` | 10000 |
| `RELAY_MAX_CONNECTIONS_PER_IP` | 64 |
| `RELAY_MAX_LISTENERS` | 5000 |
| `RELAY_MAX_SESSIONS` | 5000 |
| `RELAY_MAX_SESSIONS_PER_ROUTE` | 32 |
| `RELAY_SESSION_BANDWIDTH` | 4MiB (per second, each way) |
| `RELAY_TOTAL_BANDWIDTH` | 0 (unlimited) |
| `RELAY_IDLE_TIMEOUT` | 5m |
| `RELAY_MAX_SESSION_DURATION` | 24h |
| `RELAY_LISTEN_TOKEN` | unset: any computer may listen |

## Who may use it

There is nothing to authorize. Each computer tells the relay which of its
paired devices may dial it, and only while it is connected; everyone else
hears "offline". When a computer disconnects, its keys go with it, and the
relay keeps nothing on disk.

To keep a relay to your own computers, you may set `RELAY_LISTEN_TOKEN`
(16+ characters) and give it to them. It is off by default, and dialers
never need it.

## Using your own relay from Sheprd

Once the daemon side ships (`docs/relay-plan.md`, phase 3), add the relay in
Settings > Remote control > Relays, or in the config:

```toml
[remote]
relays = ["https://relay.example.org"]   # yours only; add "default" to keep ours too
```

Any of `relay.example.org`, `https://relay.example.org:8443`, or
`wss://example.org/relay` (behind a proxy that strips `/relay`) works. Plain
`http://` is accepted only for a relay on this computer or a private network.
Sheprd checks `https://DOMAIN/v1/info` first and asks for the listen token if
the relay requires one. Paired phones learn the relay from the computer.

## Monitoring

- `/metrics` is on port 9090, published on the droplet's `127.0.0.1` only.
  Scrape it from the droplet, through `ssh -L 9090:127.0.0.1:9090`, or over
  a private network with `RELAY_METRICS_TOKEN` set and the port published
  there.
- `https://DOMAIN/dashboard` shows sessions, listeners, throughput, and
  refusals, read-only, when `RELAY_DASHBOARD_PASSWORD` is set.
- `https://DOMAIN/healthz` answers `ok` for an uptime check.

Logs hold no addresses, routes, or payloads.

## Updating

```sh
cd /opt/sheprd-relay && docker compose --env-file compose.env up -d --pull always
```

Restarting drops open sessions. Computers listen again within seconds and
clients reconnect; nothing is lost because nothing is stored.
