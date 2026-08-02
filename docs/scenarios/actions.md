# Actions exact setup

This scenario uses the shared Actions repository as the first direct artifact
consumer. It exercises the real Unix `setup-binary` exact-channel installer,
not a runner-shaped replacement for the action.

## Exact input

The local rehearsal is bound to Actions commit
`5855aacf3d75759bf06f62cd037970e8b26afb81`. Its
`setup-binary/install.sh` input has digest
`sha256:c5bb8683d243fbd9354c3d395f80e3538c43857086fb50f142882e62a8baf234`.
The test refuses a different commit or a tracked change under `setup-binary`.

Network delivery is replaced by a local `curl` fixture. The fixture preserves
the action's public shape: the exact seal selects one Unix manager, and that
manager receives the declared beta channel, exact version, install root, and
local binary directory.

## Resource closure

One attempt receives four process resources:

- the host shell;
- the exact action script;
- read-only manager, seal, and network fixtures;
- one attempt-temporary scratch root.

The action places its exact install and binary seats below `RUNNER_TEMP` and
writes the exact seal beside them. A fifth resource is an artifact target. It
is visible to Ironbed after execution but is not included in the process
resource list.

The attempt names the seal inside the scratch grant as one artifact. Ironbed
reaps the action, copies the seal under an independent artifact byte limit,
emits its byte count and SHA-256 digest, and only then emits the finished frame.
The target name must be fresh and is never overwritten.

## Executable proof

The host rehearsal runs two generations. Each generation verifies:

- the real action creates the declared exact install and binary seats;
- `GITHUB_PATH` receives that generation's binary seat;
- the manager observes the declared channel, version, and exact paths;
- the published seal is byte-identical to the downloaded exact seal;
- started evidence retains the action source commit and digest;
- deleting the complete scratch root leaves the sealed artifact present;
- the next generation uses distinct scratch and artifact paths.

Runner tests separately prove that artifact transfer still completes after the
stdout limit terminates a process, and that a preexisting target remains
unchanged while the finished frame records failure.

## Package record

The next rehearsal keeps the same Actions commit but executes its real
`package-exact/main.ts` command against Shield commit
`03cfdbf7c267d1cf815daafe399191f485543b8a`. Deno is a third input, separate
from both the command source and the package source.

The provider clones the exact Shield commit into each attempt's scratch root.
That workspace is intentionally writable: `package-exact` temporarily stamps
the package manifest while rehearsing JSR publication, then restores it. The
test verifies the complete Git workspace is clean afterward and destroys it
with the rest of the generation.

A loopback registry double returns only `404`, so the command plans a release
without publishing a package or reserving a tag. Its deterministic
`package-exact/v1` record is written under `RUNNER_TEMP`, transferred through
the existing single-file artifact boundary, and retained after scratch cleanup.
Two generations produce byte-identical records and matching artifact digests.

This package pressure earns no new Ironbed field. Registry identity, package
identity, release version, integrity, tag, URL, and planned status are consumer
record contents. Ironbed retains only generic source declarations, process and
log facts, artifact bytes and digest, and the terminal transfer summary.

## Authority limit

The host tests use unshared random paths to demonstrate the vocabulary. They do
not prove that a hostile child cannot discover or open the artifact target.
Ironbed may later implement mount, container, VM, or another isolation
mechanism and report what happened. The surrounding control plane decides
which mechanism an attempt requires and whether the provider evidence is
sufficient. Cold-start host delivery is explicitly best effort.

The target remains attempt-scoped until a finished frame acknowledges the
transfer summary. If the runner is lost, the provider discards that entire
target even when final names or artifact frames are already visible; it does
not infer completeness from either. Durable upload, remote acknowledgement,
package meaning, Actions YAML, and Forgejo event interpretation remain outside
Ironbed.

This pressure changes only the private `v0` runner model. It does not yet earn
an `ironbed-proto` artifact message or a public compatibility promise.
