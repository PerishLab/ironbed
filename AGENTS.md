# Agent guide

Ironbed is the independent open execution testbed for perish.code.

## Current stage

The repository contains its Plumb-governed cold-start skeleton:

- `crates/proto` anchors the future `ironbed-proto` package.
- `crates/runner` anchors the `ironbed` binary and exposes only help and version.
- `docs/architecture.md` holds the pre-model closure.
- No attempt, lease, seat, plan, evidence, artifact, or receipt model is settled yet.

The first real scenarios must pressure those words before implementation gives
them fields or lifecycle.

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
- `docs` — product vocabulary, laws, scenarios, and verification.
- `.runseal` / `.forgejo` — thin Plumb-shaped local and remote operation.

Day 0 keeps proto and runner in one repository and release train. A separate
proto repository is earned only by independent compatibility, consumer, or
amendment pressure.

## Operating

- Never commit directly on `main`.
- Work on a task branch and land through `runseal :land`.
- Run `runseal :guard` before landing.
- Product vocabulary and laws stay here; repository mechanism remains Plumb's
  right to amend.
- Do not deploy, publish, create credentials, or mutate a live runner fleet
  without explicit operator authorization.
