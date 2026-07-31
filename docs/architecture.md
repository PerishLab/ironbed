# Architecture

## Image

Ironbed is an open test bed, not a box.

A changing device under test is mounted on a known-good frame, connected to
known tooling, exercised, measured, removed, and followed by a reset. The frame
does not decide why the device arrived or what its result means elsewhere.

## Independent closure

The complete product boundary is:

```text
authorized immutable attempt + lease + supplied execution seat
  -> materialize
  -> execute
  -> observe
  -> finalize
  -> scrub
  -> ordered evidence + immutable artifacts + terminal receipt + clean seat
```

Ironbed must close this path under its own reference harness without Codehull.
Codehull later supplies a real control-plane adapter at the same boundary.

## Ownership

Ironbed owns:

- the normalized executable plan and its interpretation;
- exact source materialization and verification;
- process execution, timeout, cancellation, and process-tree recovery;
- temporary capability injection and removal;
- ordered log and evidence production;
- artifact transfer and local backpressure;
- attempt-local journaling and crash recovery;
- cleanup, refusal, and reusable-seat verification;
- protocol compatibility at the execution boundary.

Ironbed does not own:

- repository, branch, change, actor, organization, or event truth;
- identity proof, grants, or capability issuance;
- deciding which attempts exist;
- global queueing, scheduling, concurrency policy, or retry decisions;
- durable Run or Runner projections;
- hosts, virtual machines, images, fleet capacity, or rotation;
- the meaning of an artifact in a downstream product.

Transport retries may repeat observation or reporting. Ironbed must never
silently repeat arbitrary execution after an indeterminate external effect.

## Provider boundary

A supplied execution seat is an enforced boundary, not just a machine handle.
Its provider establishes and attests:

- substrate, image, identity, and isolation facts;
- read-only, writable, private, and temporary resource grants;
- network reach and executable prerequisites;
- lease fencing and cleanup after the runner itself is lost.

Ironbed compares independently observable facts such as OS and architecture
with that declaration, runs inside the granted boundary, and reports only the
cleanup it can prove. While alive it owns child-process recovery; after runner
loss, the provider-owned container, VM, cgroup, or equivalent closes the seat.

The runner-private rehearsal keeps the two authorities separate even before a
transport is chosen: the attempt arrives on standard input, while the launching
provider supplies an absolute `ironbed.seat/v0` descriptor through `--seat`.
The started evidence retains required, provided, and independently observed
surface facts without presenting all three as runner observations.

Destroying a seat does not roll back consumer-owned private state or an
indeterminate remote effect.

## Day 0 topology

Two physical repositories carry three concepts:

```text
codehull -> ironbed-proto <- ironbed
```

`ironbed-proto` starts as an independently publishable crate in this repository.
It moves to its own repository only when another implementation, language,
release cadence, compatibility window, or amendment authority makes independent
ownership real.

## Construction law

Every model element must close an enumerable scenario. A useful word is not yet
a resource, and a possible field is not yet state.

Hardrig Linux estate delivery is the first real device under test. Its working
scenario lives in [`scenarios/hardrig.md`](scenarios/hardrig.md).
The scenario distinguishes the execution surface, the consumer-owned delivery
target, and any artifact target before assigning fields to any of them.

Current pressure includes:

- system, substrate, surface, image, reuse, and tenancy;
- attempt, lease, seat, capability, and external effect;
- plan, step, source, and workspace;
- evidence, log, artifact, outcome, and receipt;
- cancellation, expiry, loss, refusal, cleanup, and retry.

The crates remain anchors until executable consumer pressure earns a shape.
