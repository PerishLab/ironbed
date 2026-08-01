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

Every private v0 descriptor now carries one provider-assigned opaque seat
generation. The runner retains it in started evidence so later provider cleanup
can name the exact boundary it retired. A generation is neither a clock nor a
globally ordered sequence, and the runner does not authenticate or interpret
it. Reuse pressure requires only that a replacement offer not reuse the retired
generation.

The same private descriptor now names only the provider grants exercised by the
Hardrig scenario: read-only input, consumer-owned private state, and
attempt-temporary paths. An attempt refers to those grants by ID. The provider
also states the execution identity and network mode, and may bind an input to a
content digest and source identity. Ironbed checks that its program and working
directory are covered by required grants, but it does not claim to have proved
the provider's mount, identity, network, or source assertions.

Destroying a seat does not roll back consumer-owned private state or an
indeterminate remote effect.

The VM rehearsal preserves the same guest-local grants used by the host and
container cases. Its provider maps an immutable input medium, a persistent
private disk, and attempt-local root and firmware state into those paths.
Neither the attempt nor the runner learns the provider's host paths or block
device layout.

A private block device remains consumer-owned state even when its seat is
lost. Provider cleanup may close QEMU and its forwarding helpers, but it does
not make an unclean filesystem safe for provider-side interpretation. A new
seat first mounts the device and lets the consumer platform replay its journal;
only after that seat shuts down cleanly may the rehearsal inspect the retained
bytes from outside. This ordering keeps block lifecycle, filesystem recovery,
and runner evidence as three distinct authorities.

A root image digest is not the complete boot identity. The successful Linux
case also required immutable firmware code and fresh per-seat firmware
variables. Those remain provider materialization facts until another provider
earns a transport-neutral shape. Acceleration is likewise an adapter choice:
TCG and KVM may exercise the same declared `linux-x86_64` VM surface without
changing the attempt.

## Live supervision boundary

The runner uses a bounded channel between each operating-system pipe and the
evidence writer. A per-attempt retained-byte limit covers stdout and stderr
together. Crossing it terminates the private v0 attempt, preserves the retained
per-stream offsets, and records separately the bytes already observed but
discarded. It does not claim a total order between the two streams or account
for bytes the terminated process never wrote.

On Unix, each child starts in a fresh process group. Timeout, output exhaustion,
and runner-side abort signal that group rather than only the direct child. The
finished frame proves that the direct process was reaped and, when applicable,
that a group signal was sent; it does not claim that a provider seat is clean.

An external `SIGTERM` asks a live Unix runner to cancel. The runner remains
alive, terminates the child process group, drains its pipes, reaps the direct
process, and emits one finished frame with `cancelled` as the cause. The
incoming cancellation request and the kill used to enforce it are distinct
facts.

The private runner also accepts an optional provider-supplied `--cancel` path.
One byte on that pre-established stream enters the same cancellation state.
The VM adapter maps a host QEMU socket to that guest-local path with
virtio-serial. POSIX signal and byte stream are current launch transports, not
attempt vocabulary or a settled control-plane protocol.

If the runner itself is lost, no terminal runner frame exists. A provider may
destroy the container, VM, cgroup, or lease boundary and separately attest that
fact. A new authorized attempt must observe any retained private state or
indeterminate external effect before deciding what to do next.

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
The provider cleanup and reuse pressure lives in
[`scenarios/reuse.md`](scenarios/reuse.md).
The scenario distinguishes the execution surface, the consumer-owned delivery
target, and any artifact target before assigning fields to any of them.

Current pressure includes:

- system, substrate, surface, image, reuse, and tenancy;
- attempt, lease, seat, capability, and external effect;
- plan, step, source, and workspace;
- evidence, log, artifact, outcome, and receipt;
- cancellation, expiry, loss, refusal, cleanup, and retry.

The crates remain anchors until executable consumer pressure earns a shape.
