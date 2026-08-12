# Architecture

Ironbed is an open execution testbed. A changing device under test is mounted
on a known-good frame, exercised and measured, then removed before the frame is
reused. The frame never decides why the work exists or what its result means to
another product.

## Closure

```text
authorized immutable attempt + lease + supplied execution seat
  -> materialize -> execute -> observe -> finalize -> scrub
  -> ordered evidence + immutable artifacts + terminal receipt + clean seat
```

Ironbed owns plan interpretation, exact source materialization, process
execution, timeout and cancellation, temporary capability removal, ordered log
and evidence production, bounded artifact transfer, attempt-local recovery,
cleanup, reusable-seat verification, and execution-boundary compatibility.

It does not own repository or actor truth, identity and capability issuance,
global scheduling or retries, durable Run projections, fleet capacity, host or
image lifecycle, or downstream artifact meaning. An indeterminate external
effect is never silently repeated.

## Provider boundary

A seat is an enforced provider boundary, not a machine handle. Its provider
declares and attests substrate, image, identity, isolation, resource grants,
network reach, prerequisites, lease fencing, and cleanup after runner loss.
Ironbed compares independently observable surface facts and reports only what
it can prove.

The provider passes an absolute private seat descriptor separately from the
attempt. It names one opaque generation and explicit read-only, private, and
temporary grants. The attempt refers to grants by ID. Ironbed checks that its
program and working directory are covered but does not claim to have proved the
provider's mount, identity, network, or source assertions.

Generation has no clock or ordering semantics. It binds provider cleanup to
the exact retired offer, and a replacement must use another value. Consumer
private state may survive seat destruction and is never interpreted as rollback
or durability of unflushed bytes.

An artifact target is a separate provider resource, absent from the process
grant list. Ironbed opens it only after process supervision and direct-process
reap. Host rehearsal is best effort; surrounding policy decides whether the
provider mechanism offers sufficient isolation.

## Supervision

Operating-system pipes feed a bounded channel to the evidence writer. One
retained-byte limit covers stdout and stderr together. Exhaustion terminates
the process group, retains per-stream offsets, and reports observed but
discarded bytes without inventing a total order between streams.

On Unix each child starts in a fresh process group. Timeout and runner-side
abort signal that group. A terminal frame proves direct-process reap and any
signal Ironbed sent; it does not prove provider cleanup.

Live cancellation asks Ironbed to remain alive, terminate its child group,
drain output, reap the direct process, and emit a `cancelled` terminal frame.
Unix signal and a provider-established byte stream are launch transports, not
attempt vocabulary. If the runner itself is lost, no terminal frame exists;
only the provider can attest seat destruction.

## Artifacts

An attempt requests ordered single-file artifacts by stable logical ID and one
portable fresh basename. After reap, Ironbed streams each source through an
independent byte limit, hashes it, syncs a staging file, and publishes without
replacement. Artifact frames precede the one finished frame.

Timeout, cancellation, output exhaustion, and normal exit share this bounded
artifact phase. Transfer failure is terminal and never repeats execution. A
target remains attempt-scoped until its terminal receipt. After runner loss the
provider discards the whole unacknowledged target rather than inferring
completeness from files or partial frames.

## Recovery and topology

While alive, Ironbed owns child recovery. After loss, a container, VM, cgroup,
or equivalent provider boundary closes the seat. A new authorized attempt must
observe retained private state and any indeterminate external effect before a
control plane chooses the next action.

The runner and transport-neutral proto currently share one repository and
release train:

```text
codehull -> ironbed-proto <- ironbed
```

Every public model element must close an enumerable consumer scenario. Current
private v0 shapes retain pressure from Hardrig delivery, host/container/VM seat
reuse, live cancellation, provider loss, and exact release artifacts without
promoting provider-specific details into the protocol.
