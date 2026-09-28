# Agent guide

Ironbed is the independent open execution testbed for perish.code.

## Current stage

- `crates/proto` anchors the independently publishable boundary package.
- `crates/runner` executes the private v0 attempt and seat shapes, supervises
  process trees, emits ordered evidence, transfers bounded artifacts, and
  closes one terminal receipt.
- `ARCHITECTURE.md` records the current execution/provider authority split.

Private rehearsal shapes are executable pressure, not a public protocol
promise. A stable contract is earned only by another implementation, language,
release cadence, compatibility window, or amendment authority.

## Stable boundary

Ironbed begins after a control plane offers one authorized, immutable attempt
under a lease and after infrastructure supplies an execution seat. It ends
after ordered evidence, immutable artifacts, and one terminal receipt have
been reported and the seat has returned to a reusable clean state.

Ironbed owns execution semantics, local recovery, observation, cleanup, and
the protocol needed to express that boundary.

It does not own repository facts, identity, authority, global scheduling,
retry decisions, durable run truth, hosts, virtual machines, images, fleet
capacity, or rotation.

## Repository shape

- `crates/proto` — transport-neutral boundary contract; no network, storage,
  scheduler, executor, or Codehull model.
- `crates/runner` — the executable testbed.
- `ARCHITECTURE.md` — current topology, authority, and failure boundaries.
- `runseal.toml` / `.runseal/resources` — env-only repository-local profile
  material.

Day 0 keeps proto and runner in one repository and release train. A separate
proto repository is earned only by independent compatibility, consumer, or
amendment pressure.

## Release

`plumb.toml` declares the product `ironbed`, its authority, the binary
`ironbed` for three targets, and the cargo attachment `ironbed-proto` on the
perish registry. The workspace declares version `0.0.0`; `crates/runner`
carries the release identity region through `plumb::identity!("IRONBED")`,
which wharf binds after an unbound build. A release follows Plumb's lifecycle:
`plumb release --help`; wharf publishes it. A stable's changelog goes to the
Depot.

## Operating

- Never commit directly on `main`.
- Work on a topic branch and land through Concord's Issue-led delivery.
- Before landing, run `plumb doctor .`, `cargo fmt --all --check`,
  `cargo clippy --locked --workspace --all-targets -- -D warnings`,
  `cargo check --locked --workspace --all-targets --release`,
  `cargo test --locked --workspace`, and `ectropy .`.
- Use `runseal profile` to validate the repository profile and
  `runseal : <command> [args...]` only when a command needs its environment.
- Product vocabulary and laws stay here; repository mechanism remains Plumb's
  right to amend.
- Do not deploy, publish, create credentials, or mutate a live runner fleet
  without explicit operator authorization.
