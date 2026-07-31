# Ironbed

An open execution testbed.

Ironbed mounts one sealed attempt on a known-good execution seat, exercises it,
records what happened, and returns the seat clean for the next attempt.

The repository is at cold start. It contains the independent proto and runner
anchors plus the laws that keep their boundary honest; the first real scenarios
will decide the model.

See [`docs/architecture.md`](docs/architecture.md).
