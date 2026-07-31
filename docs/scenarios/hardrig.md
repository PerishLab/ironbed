# Hardrig Linux delivery

This is a working pressure scenario, not a settled protocol.

Hardrig is the first real device under test. It delivers one authored estate
through repeated observation, one verified action at a time. Ironbed must be
able to exercise that delivery without learning Hardrig's resource vocabulary
or pretending that a process result is an estate result.

## Existing contract

The current Hardrig command surface establishes four distinct cases:

| command | mutation | process result | product meaning |
| --- | --- | --- | --- |
| `plan` | none | zero | the estate may still be ready, pending, or blocked |
| `drift` | none | zero or one | one means declared state is not fully ready |
| `apply --yes` | external | zero or two | zero follows verified convergence; two is blocked |
| `rebuild --yes` | destructive | zero or two | separately authorized and outside this rehearsal |

`apply` repeatedly observes the complete estate, performs only the first exact
action, verifies its postcondition, discards the plan, and observes again.
After an interruption, a new controller decision may submit a new attempt.
Ironbed must not silently replay the interrupted process.

## Three coordinates

One delivery exposes three different coordinates:

- the execution surface on which the Hardrig process runs;
- the remote delivery target whose state Hardrig owns;
- any artifact target produced by the work.

The opening rehearsal has a Linux x86-64 execution surface. Its substrate is a
provider fact such as host or container. The remote Ubuntu VM remains an
opaque Hardrig target, and this scenario produces no artifact target.

An execution surface therefore cannot stand in for a delivery target, and a
delivery target cannot stand in for a build triple.

## Inputs under pressure

The rehearsal needs these capabilities without yet granting them final field
names:

- an immutable Hardrig binary and authored model snapshot;
- a read-only model root;
- a private persistent state root with an explicit owner;
- structured program arguments rather than a shell program;
- bounded network reach and temporary capability references;
- a declared execution surface and provider evidence for its substrate;
- a lease that fences one submitted execution.

Secrets remain inside the consumer's private state or temporary capability
boundary. Ironbed records neither their values nor a fabricated digest of
unknown secret material.

## Evidence and result

Ironbed can report facts it observes:

- process creation and termination;
- raw exit or signal information;
- stdout and stderr bytes with independent stream offsets;
- time, cancellation, and output-limit events;
- the observable OS and architecture;
- cleanup of paths and processes it owns.

It cannot infer Hardrig's estate state from an exit code. It also cannot infer
which remote effects occurred after an interrupted process. A consumer adapter
may interpret Hardrig's domain result, while the transport-neutral receipt
retains only execution facts and explicit attestations.

A merged terminal display is derived evidence. Two operating-system pipes do
not establish a truthful total byte order across stdout and stderr.

## Mutation-free rehearsal

The first joint rehearsal ran Hardrig's real binary against an offline Linux
fixture through the runner:

1. `plan` exits zero while disclosing a blocked estate and creates no state;
2. `drift` exits one for the same observed condition and creates no state;
3. `apply --yes` exits two before mutation because credentials are absent;
4. the runner transport completes in all three cases and retains each raw
   process result.

The fixture remained byte-identical and the private state root remained empty.
Destructive `rebuild` stays outside the rehearsal. No provider, host, cluster,
DNS, data, or credential mutation belongs here.

The runner currently carries the private `ironbed.rehearsal/v0`,
`ironbed.seat/v0`, and `ironbed.frame/v0` shapes needed to execute this
pressure. They are not an `ironbed-proto` contract. In particular, raw byte
arrays, provider-supplied surface and resource facts, and process-group timeout
handling are visible pressure rather than settled answers.

## Mutation-bearing rehearsal

The next joint rehearsal uses a local Cube double and a temporary SSH daemon.
It gives the real Hardrig binary enough truthful observations to select its
first action while keeping every target disposable:

1. `apply --yes` creates and verifies the root seed in consumer-owned private
   state;
2. the Cube double fences the mandatory observation immediately after that
   action;
3. Hardrig exits two while Ironbed still completes the execution transport;
4. the seed exists with mode `0600`, and the authored model remains unchanged;
5. a new `plan` attempt observes the seed as ready and exposes the next action
   without replaying the first attempt.

The test is opt-in because it consumes a separately built Hardrig binary plus
local `sshd` and `ssh-keygen`. It performs no live provider mutation and retains
no generated secret after its temporary private-state root is removed.

This establishes a real asymmetry: a failed process can have a verified
consumer effect. The current private seat descriptor now names the state root
as a provider-granted `private` resource, separately from read-only inputs and
attempt-temporary paths. The runner retains that boundary and the process
failure, while verification of the seed remains in the Hardrig scenario
adapter. A grant identifies where an effect may persist; it does not let
Ironbed infer that effect from stdout.

The same case is also fenced by time instead of a Cube refusal. Hardrig creates
and verifies the seed, then blocks in the mandatory next observation. The
attempt deadline signals Hardrig's process group and reaps the direct process,
and the finished frame records `timeout` rather than inventing an exit code. A
separately authorized attempt then observes the seed as ready and continues
from the next action.

This earns a required bounded-execution input and a timeout terminal fact. It
does not equate a sent process-group signal with provider-seat cleanup.

## Container seat rehearsal

The mutation-bearing case also runs across two fresh Docker containers while
the provider keeps the same private-state bind:

- the Ironbed and Hardrig binaries, authored model, and CA bundle are mounted
  read-only;
- private state is the only writable bind;
- host networking is an explicit rehearsal grant;
- the container user matches the owner of private state;
- each container is removed after its attempt.

The first candidate image was Debian 12. It truthfully refused the host-built
binary because its glibc was older than the binary requirement. Ubuntu 24.04
matched the ABI but still needed an explicit CA bundle before Hardrig could
construct its HTTP client. These are image and executable-prerequisite facts,
not refinements of `linux-x86_64`.

With those grants in place, the first container creates the seed and exits
after the observation fence. A second container observes that same seed as
ready and continues from the next Hardrig action. This proves that a seat is
not private state and that destroying a container is not rollback.

Ironbed independently observes only `linux-x86_64`. The Docker scenario adapter
attests `container`, the exact image identity, mounts, network mode, and user.
It supplies surface facts through a read-only seat descriptor, separately from
the attempt on standard input. The runner compares required, provided, and
observed facts and does not claim it detected its own substrate.

The descriptor now carries four named resources: the Hardrig binary, model,
private state, and CA bundle. The attempt requests those IDs without teaching
Ironbed what Hardrig's arguments mean. The adapter binds the Hardrig input to
both its SHA-256 digest and Git source identity. The joint validation used
Hardrig commit `24f015bd2ec0328e73133578f7dc42b81dde0dc5`, binary digest
`sha256:d0b68fdf1b543b36cc96431718a7f10b33a27ff4087a4e0701d572eb16b72d8b`,
and Ubuntu image
`sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90`.
Building the same version label from Hardrig's earlier `main` did not satisfy
the scenario, demonstrating why version text alone is not source binding.

## VM seat rehearsal

The same mutation-bearing case now crosses two complete QEMU VM seats. The
reference adapter materializes each seat from:

- the exact Ubuntu Noble x86-64 cloud image;
- read-only OVMF code and fresh per-seat OVMF variables;
- a fresh QCOW2 root overlay;
- one read-only ISO containing Ironbed, the source-bound Hardrig binary, model,
  and CA bundle;
- one 128 MiB private ext4 disk retained across the two seats;
- a fresh NoCloud seed, two virtual CPUs, 2 GiB of memory, and user networking.

The rehearsal runs under TCG so it needs no KVM group or device permission.
That makes it a slow but portable ground-truth path. KVM acceleration can
replace TCG inside the adapter without changing the `linux-x86_64` and `vm`
requirements.

The network is closed by default. QEMU `restrict=on` blocks host and external
reach, while two explicit guest forwards expose only the temporary HTTPS Cube
double and SSH double. A one-day rehearsal CA signs the IP-address certificate
for the guest endpoint. The CA certificate enters the read-only medium; its
private key remains in the host-side temporary directory. The adapter verifies
that its forwarding helpers are gone after each seat before starting another.

The first guest runs `apply --yes`, creates and verifies the root seed on the
private disk, and exits two at the Cube fence. The second guest has a new root
overlay and new firmware variables but mounts the same private disk. Its
authorized `plan` observes the seed as ready, exposes the SSH action, and exits
zero. Both guests shut down, their attempt-local material is removed, the
authored model remains byte-identical, and the Cube double receives the same
seven requests as the host and container rehearsals.

This confirms that the current resource classes and guest-local path grants
survive a VM without provider-specific attempt vocabulary. It also shows that
the root image alone cannot identify the boot surface: firmware code and
per-seat firmware state remain separate provider facts. The private
`ironbed.seat/v0` frame currently carries the root image digest, while the
rehearsal adapter separately verifies its source and the firmware inputs.

The exact validation used Ubuntu build `20260725` from
[`noble/20260725`](https://cloud-images.ubuntu.com/noble/20260725/) with image
digest
`sha256:d1940f7d69d343355e183dff1e08a59852d32e7309baa7a4bad8365b11b005ac`,
OVMF `2024.02-2ubuntu0.9`, QEMU `8.2.2`, and the same Hardrig commit and binary
digest as the container rehearsal.

## Output and process-tree rehearsal

The private attempt carries one retained-byte limit shared by stdout and
stderr. Pipe readers feed an eight-slot bounded channel, so a slow evidence
consumer applies local backpressure. If the next chunk crosses the limit,
Ironbed retains only the remaining bytes, records observed-but-discarded bytes,
signals the process group, and finishes with `output_limit`.

A separate Linux fixture lets the direct shell exit zero while a background
child keeps its inherited output pipes open. The attempt still reaches its
deadline, signals the process group, returns within the bound, preserves the
direct exit code, and reports `timeout`. This keeps direct-process result,
attempt termination, and cleanup evidence distinct.

## Provider-loss rehearsal

The mutation-bearing case kills the complete provider seat after Hardrig has
verified the seed and entered its mandatory next observation. Both the Docker
and QEMU adapters exercise the same sequence:

1. Ironbed emits started and output evidence, but no finished frame survives;
2. the provider destroys the container or VM and verifies that its forwarding
   helpers are gone;
3. consumer-owned private state and the verified seed survive seat
   destruction;
4. a fresh seat and authorized `plan` attempt observe the seed as ready and
   continue from the next action;
5. the authored model remains byte-identical.

The QEMU case gives each attempt a fresh root overlay and firmware variables
while retaining the same private ext4 disk. Killing QEMU leaves that filesystem
unclean, so the adapter does not use `debugfs` to interpret it immediately.
The recovery guest mounts the disk and replays its journal first. Only after
that guest shuts down cleanly does the adapter inspect the lost attempt's
complete frames and prove that none is `finished`.

This proves the division of responsibility rather than a wire shape. A live
runner can report its direct-process and signal facts. After runner loss, only
the provider adapter can attest seat destruction, and neither authority may
claim rollback of private state or remote effects. The observed seed survived
this exercised loss, but the rehearsal does not promise durability for
arbitrary consumer writes that were never flushed.

## Pressure still needed

This scenario does not yet earn a stable wire shape. Further Linux pressure
must determine:

- which image, firmware, and source facts belong in durable provider evidence
  without conflating a root disk with a complete boot identity;
- how network grants become enumerable without embedding one provider's
  forwarding vocabulary;
- how an external-effect boundary is represented without claiming rollback;
- whether output exhaustion always terminates or may switch to a separately
  bounded artifact transfer without weakening evidence truth;
- how external cancellation enters the same process-group and provider cleanup
  boundary;
- which provider cleanup attestations are sufficient before a seat is reused,
  including helpers outside the guest process tree;
- how private-storage durability is declared and tested without confusing
  retained media with a guarantee for unflushed consumer writes;
- whether Hardrig eventually emits a structured domain result or leaves that
  interpretation in its consumer adapter.
