# Seat cleanup and reuse

A reusable seat is a provider conclusion, not a successful process exit. This
scenario closes the boundary between one finished or lost generation and a
replacement offer on the existing `linux_x86_64` container and VM surfaces.

## Authorities

The runner may report direct-process reap, an enforcement signal it sent, and
the output it retained. Those facts remain in the terminal runner frame when
the runner survives. They do not establish that a provider boundary is clean.

The provider adapter may report only facts it independently verifies after the
runner ends or disappears:

- the exact opaque generation whose boundary closed;
- the provider boundary that no longer exists;
- the enumerable attempt-local material and helpers that were retired;
- the consumer-private medium whose disposition is retained.

The retained disposition describes medium lifecycle only. It does not assert
which bytes were flushed, interpret consumer state, or promise rollback.

## Generation

The private `ironbed.seat/v0` descriptor requires one nonempty generation of at
most 128 bytes. It is assigned by the provider, retained unchanged in the
started frame, and used by the adapter to bind cleanup to the original offer.

Generation has no ordering semantics. It is not a lease, fencing token, retry
counter, durable runner identity, or global seat ID. The current reuse test asks
only that cleanup names the old generation and that a replacement offer uses a
different value.

## Container closure

Every Docker offer uses a unique named container, one fresh writable
attempt-temporary bind, and the existing read-only inputs plus retained private
state. `TMPDIR` points at the temporary grant so the executable plan can use it
without learning a host path.

Cleanup is constructed only after Docker has removed the named container and
after dropping the provider-owned descriptor and temporary directory has made
both host paths absent. Its adapter-local fact set is:

- boundary: container;
- temporary: container root and attempt-temporary grant;
- retained: private state;
- helpers: none claimed by this adapter.

The exact source-bound Hardrig binary exercised normal completion,
cancellation, and provider loss. All three cases retired their original
generation. A fresh offer then observed the retained root seed and continued
from the next Hardrig action.

## VM closure

Every QEMU offer uses fresh root overlay, OVMF variables, NoCloud seed, serial
log, control socket, read-only seat-descriptor ISO, and guest-local temporary
path. Only the consumer-private ext4 medium crosses generations. The network
remains closed except for the declared local Cube and SSH doubles.

Cleanup is constructed only after QEMU exits or is reaped, both forwarding
helpers disappear, the control socket refuses connection, the private medium
still exists, and dropping the provider-owned seat directory makes every
attempt-local path absent. Its adapter-local fact set is:

- boundary: VM;
- temporary: root overlay, firmware variables, cloud seed, seat descriptor,
  serial log, control socket, and the guest-local temporary grant;
- helpers: network forwarding;
- retained: private state.

The two-generation recovery case passed under TCG in 875.94 seconds. It used
Ubuntu Noble build `20260725` at
`sha256:d1940f7d69d343355e183dff1e08a59852d32e7309baa7a4bad8365b11b005ac`
and Hardrig commit `24f015bd2ec0328e73133578f7dc42b81dde0dc5` at
`sha256:d0b68fdf1b543b36cc96431718a7f10b33a27ff4087a4e0701d572eb16b72d8b`.
The first generation created the root seed and was fully retired. The second
generation used fresh attempt-local material, retained the same private disk,
observed the seed as ready, and completed the seven-request scenario.

The cancellation-and-reuse case passed under the same TCG surface in 780.22
seconds. The first generation emitted a distinct `cancelled` runner frame,
then provider cleanup retired its VM boundary. A replacement generation
recovered the retained seed and completed normally.

The provider-loss-and-reuse case passed in 528.77 seconds. The lost generation
retained started and output evidence but no finished runner frame. Provider
cleanup independently retired QEMU, its helpers, and attempt-local material;
the recovery generation replayed the private filesystem journal before the
adapter inspected retained bytes.

## Not yet earned

This scenario does not settle an `ironbed-proto` cleanup message, provider
authentication, acknowledgement, replay handling, monotonic generations,
private-storage durability, or Codehull retry policy. A future control-plane
adapter must pressure those concerns without merging runner and provider
authority.
