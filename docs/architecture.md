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

The first scenario discussion will pressure:

- attempt, lease, seat, and capability;
- plan, step, source, and workspace;
- evidence, log, artifact, outcome, and receipt;
- cancellation, expiry, loss, refusal, cleanup, and retry.

Until then the crates remain anchors rather than a speculative framework.
