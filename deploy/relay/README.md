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
Let's Encrypt certificate. The dashboard's access token is in
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
the platform provides (below); add `RELAY_DASHBOARD_TOKEN` as an encrypted
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
cp env.example .env    # set RELAY_DASHBOARD_TOKEN for the dashboard
mkdir -p bans && touch bans/list   # ranges to ban by hand, one per line
RELAY_DOMAIN=relay.example.org docker compose up -d
```

Or just the image, with no proxy and no TLS, for a trial:

```sh
docker run --rm -p 8080:8080 -p 8443:8443/udp \
  -e RELAY_DASHBOARD_TOKEN="$(openssl rand -base64 32)" \
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

## Toxic clients

Addresses that misbehave are banned automatically, with nothing to set up:
hitting limits again and again, forged proofs, malformed messages, probing,
dialing computers that refuse them, three wrong dashboard tokens, or
moving more than 20 GiB in an hour (`RELAY_MAX_TRANSFER_PER_HOUR`). Bans
last 10 minutes, then an hour, then a day, and show on the dashboard.
Private and loopback addresses are never banned automatically.

To ban someone by hand, without a restart, list their addresses or ranges
in a file and point the relay at it:

```sh
echo '203.0.113.0/24' >> /opt/sheprd-relay/bans/list    # takes effect within 30 s
```

`deploy/relay`'s compose file mounts the `./bans` directory read-only and
sets `RELAY_BAN_FILE=/bans/list`, and the droplet's user data creates it.
Elsewhere, mount a directory and point `RELAY_BAN_FILE` at a file in it, or
set `RELAY_BAN=203.0.113.7,2001:db8::/32` and restart. `RELAY_AUTO_BAN=off`
turns the automatic bans off.

## Listing a relay as one of Sheprd's

Sheprd's own relays are whatever `https://sheprd.org/relays.json` lists,
from `website/site/relays.json` in this repository. To add one once it
answers `https://DOMAIN/v1/info`, add an entry and deploy the site:

```json
{
  "version": 1,
  "relays": [
    { "endpoint": "https://fra1.relay.sheprd.org", "region": "fra1", "name": "Frankfurt" }
  ]
}
```

Remove an entry to retire a relay, or empty the list to take them all out
of service: Sheprd then shows "No Sheprd relays are available for now", and
computers stay reachable on their own networks and private relays. Clients
re-read the list every hour and keep the last good copy. The endpoint must
be `https`; the site test and the relay crate's tests check the file.

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
  refusals, live and read-only, when `RELAY_DASHBOARD_TOKEN` is set. Sign in
  with the token; three wrong tries block that address for 15 minutes.
- `https://DOMAIN/healthz` answers `ok` for an uptime check.

Logs hold no addresses, routes, or payloads.

## Updating

```sh
cd /opt/sheprd-relay && docker compose --env-file compose.env up -d --pull always
```

Restarting drops open sessions. Computers listen again within seconds and
clients reconnect; nothing is lost because nothing is stored.
