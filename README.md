# sprue

Support services for running VMs on Oxide hardware.

Sprue takes the attestable VM identity provided by the Oxide Rack and transforms it into an OIDC
token that can be used to authenticate to servers and services that do not know about the Oxide
rack.

## Why is it needed

There are a number of services that are wanted on a production VM, like remote backups, logging, and
monitoring that are tedious and fragile to individually implement per server. Sprue provides
a number of those services or the configuration for them by exposing them as a deployable agent.
The `sprue-agent` runs on the guest VM and talks to a centralized `sprue-api` server. The agent
controls the authentication flow so that the `sprue-api` server can provide a number of services
back to the VM host.

## How it works

`sprue-agent` runs inside a guest VM and talks to the Oxide rack attestation mechanism via a
platform defined vsock. For the `sprue-agent` to retrieve an identity, it first asks the `sprue-api`
server for a challenge. The `sprue-agent` signs this challenge via the rack attestation mechanism
to prove its hardware and control plane identity. It then exchanges this signed challenge with the
`sprue-api` server to retrieve an OIDC token. The API server verifies these attestations against a 
trusted certificate chain and reference measurements that it retrieves at startup.

```mermaid
sequenceDiagram
    participant W as Workload
    participant A as sprue-agent (in guest)
    participant H as Attestation hardware
    participant S as sprue-api
    W->>A: request over /var/run/sprue.sock
    A->>H: attest (vsock)
    H-->>A: cert chain, log, signed nonce
    A->>S: register + prove identity
    S-->>A: registration accepted
    A-->>W: OIDC token / backup id
```

Registration moves through `Pending` → `Proven` → `Accepted`/`Rejected`. A server can be accepted
automatically when it satisfies a Cedar policy, or left for an operator to accept or reject by hand.

Once a registration is accepted, the agent can:

- **Mint OIDC tokens** so a workload can authenticate to other services without a long-lived secret.
  `sprue-api` publishes standard `/.well-known/openid-configuration` and `/.well-known/jwks.json`
  documents for verifying tokens.
- **Push backups** of arbitrary blob to S3-backed storage, blobs are staged locally and uploaded via
  a resumable, idempotent flow.
- **Check in** periodically, so the control plane can tell live servers from ones that have gone
  away.

## Getting started

You'll need a Rust toolchain (pinned in `rust-toolchain.toml`; `rustup` will pick it up
automatically) and a PostgreSQL database.

```console
$ cargo build --workspace
```

### Configuration

The API server reads `settings.toml` that defines its runtime configuration. See 
example.settings.toml for possible configuration.

One important setting is the policy file location. It should point to a file that defines the 
automatic server registration policy that the sprue instance intends to support. If a server does
not want to support automatic registration, then a blank policy can be used.

The Cedar *schema* those policies are written against is not configurable. It describes entities the
server itself constructs, so it is compiled into the binary from `sprue-api/sprue.cedarschema` and
versioned with the code. Policies are validated against it at startup, so a policy referring to an
attribute the server no longer produces fails immediately instead of silently denying every
registration.

Validate a config and apply migrations before the first run:

```console
$ cargo run --bin sprue-api -- --config settings.toml validate
$ cargo run --bin sprue-api -- --config settings.toml migrate
$ cargo run --bin sprue-api -- --config settings.toml run
```

`validate` checks that the settings file deserializes and, when an auto registration policy is
configured, that the policy file can be read, parses as Cedar, and validates against the bundled
schema.

## Development

### Tests

Tests need a PostgreSQL instance; each test creates and tears down its own database.

```console
$ export TEST_DATABASE=postgres://test:test@localhost
$ cargo test --all-features --workspace
```

### Regenerating the SDK

After changing any endpoint, regenerate the OpenAPI document and the generated clients. CI fails if 
these are out of date.

```console
$ cargo run --bin sprue-api -- --config settings.toml describe  # updates sprue-api-spec.json
$ cargo xtask generate                                          # updates sprue-sdk and sprue-cli
```

## Contributing

We're open to PRs that improve these services, especially if they make the repo easier for others
to use and contribute to. However, we are a small company, and the primary goal of this repo is as
an internal tool for Oxide, so we can't guarantee that PRs will be integrated.

## License

Unless otherwise noted, all components are licensed under the
[Mozilla Public License Version 2.0](LICENSE).
