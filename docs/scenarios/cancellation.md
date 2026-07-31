# Cancellation

Cancellation is a request to a live runner, not another spelling of runner or
seat loss. Ironbed must remain alive long enough to terminate its child process
group, drain output, reap the direct process, and emit one terminal frame.
Provider cleanup remains separate evidence.

## Local transport

On Unix, `SIGTERM` enters Ironbed's cancellation state. A host fixture waits
until the started frame is observable before signalling the runner. The
finished frame reports `cancelled`, no process exit code, direct-process reap,
and a kill sent to the child process group.

The incoming request and enforcement signal are distinct. `SIGTERM` reaches
Ironbed; Ironbed currently uses `SIGKILL` to close the child group.

## Provider transport

The private runner accepts an optional absolute `--cancel` path alongside the
provider-supplied seat descriptor. The provider must materialize and connect
the stream before runner launch. Reading one byte enters the same cancellation
state as local `SIGTERM`; the attempt does not learn the provider transport.

The Docker adapter uses `docker kill --signal TERM` because Ironbed is the
container entrypoint. After Hardrig verifies the root seed and reaches its
fence, Ironbed emits the terminal cancellation frame and exits zero. Docker
removes the container separately.

The QEMU adapter uses a host Unix socket, QEMU chardev, and named
virtio-serial port. The guest sees only
`/dev/virtio-ports/ironbed.cancel`. The adapter holds the host connection from
seat creation and writes the cancellation byte only after the same Hardrig
fence.

## Recovery

Both provider-owned cases retain consumer private state but rotate the seat.
A fresh authorized `plan` observes the root seed as ready, exposes the SSH
action, and leaves the authored model byte-identical. Container absence,
QEMU exit, network forwarding cleanup, and control-socket closure are provider
facts rather than runner claims.

The final QEMU test passed under TCG in 599.28 seconds. Its first guest emitted
`cancelled` with no process code and shut down cleanly. The second guest used a
fresh root overlay and firmware variables with the same private ext4 disk,
completed the seven-request scenario, and exited zero.

## Pressure

Two earlier QEMU attempts correctly failed because the provider connected the
host control socket only when cancellation was requested. The guest reader had
already observed EOF, so Hardrig completed at its ordinary fence and Ironbed
truthfully reported `process`. Pre-establishing and retaining the channel made
control reachability a seat prerequisite instead of a timing guess.

The runner-private path does not yet settle cancellation authority,
authentication, deduplication, replay, acknowledgement, or the eventual
Codehull transport. Those facts need control-plane pressure before entering
`ironbed-proto`.
