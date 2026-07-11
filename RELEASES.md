# Release Policy

Velum does not currently publish supported releases. Tags named `snapshot-*`
may publish checksum-verified **research snapshots** of `velum`; they are
explicitly pre-releases and create neither a support obligation nor a wire
compatibility promise.

- Foundation through Stage 4 artifacts are research snapshots and create no
  wire compatibility promise.
- Stage 5 may publish a signed protocol version 0 preview only after its
  interoperability, conformance, and security gates pass.
- Version 0 breaking changes require release notes and migration tooling.
- A production candidate requires every Stage 6 release, security,
  reliability, capacity, operations, and support gate to close.
- Published artifacts must name their source revision, toolchain, build
  procedure, compatibility status, and known limitations.

Release approval belongs to release-maintainers. The first release procedure
must include signing, provenance, reproducible-build verification, staged
rollout, and tested rollback before this policy can claim those controls exist.
