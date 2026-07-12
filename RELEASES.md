# Release Policy

Velum publishes checksum-verified releases from `v*` tags. Tags with a
prerelease suffix publish **beta releases**; `vX.Y.Z` tags publish stable
releases. Neither track creates a wire compatibility promise until the
applicable protocol and release gates close.

- Foundation through Stage 4 artifacts are beta releases and create no
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
