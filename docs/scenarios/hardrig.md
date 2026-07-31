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

The runner currently carries the private `ironbed.rehearsal/v0` and
`ironbed.frame/v0` shapes needed to execute this pressure. They are not an
`ironbed-proto` contract. In particular, raw byte arrays, provider-declared
substrate evidence, and the absence of cancellation are visible pressure
rather than settled answers.

## Pressure still needed

This scenario does not yet earn a stable wire shape. The next pressure is one
disposable Linux target on which Hardrig performs and verifies a reversible
mutation. That run must determine:

- how a provider attests host versus container versus VM;
- how private persistent state differs from an Ironbed artifact;
- how an external-effect boundary is represented without claiming rollback;
- how a new attempt resumes by observation after indeterminate termination;
- which cleanup facts are meaningful for a reused execution surface.
