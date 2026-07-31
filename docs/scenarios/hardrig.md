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
arrays, provider-supplied surface facts, and the absence of cancellation are
visible pressure rather than settled answers.

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
consumer effect. The current runner receipt truthfully retains the process
failure but does not yet know the private-state boundary, so the verification
remains in the Hardrig scenario adapter. Ironbed must earn any stronger claim
from an explicit resource grant rather than from stdout interpretation.

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

## Pressure still needed

This scenario does not yet earn a stable wire shape. Further Linux pressure
must determine:

- the smallest transport-neutral shape for provider-attested substrate, image,
  identity, network, and resource grants;
- how read-only model input and private persistent state become explicit
  provider grants without becoming Ironbed artifacts;
- how an external-effect boundary is represented without claiming rollback;
- how cancellation and output limits preserve the same observation-first
  recovery rule;
- which cleanup facts are meaningful for a reused execution surface.
