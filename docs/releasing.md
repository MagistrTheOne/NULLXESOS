# Release process

## Versioning

- `0.1.0-alpha.N` after Phase 5 internal builds.
- `0.1.0-rc.N` after Phase 6 smoke passes on QEMU + 1 hardware target.
- `0.1.0` after Phase 6 smoke passes on **all 3** hardware targets in `docs/smoke-checklist.md`.

## Steps

1. Bump versions in `Cargo.toml` (workspace) and every PKGBUILD under
   `packaging/pkgbuilds/*/PKGBUILD`. Run `cargo update -p` for each crate.
2. Update `docs/release-notes/<version>.md`.
3. Tag with `git tag -s v<version> -m "NULLXES OS <version>"`.
4. Push `git push --tags`. The release workflow:
   - builds the workspace under `archlinux:latest`,
   - runs the smoke `expect` script under QEMU,
   - rebuilds every PKGBUILD with `SOURCE_DATE_EPOCH` set to the tag commit,
   - signs each `*.iso` and `*.pkg.tar.zst` with the release key (held in
     GitHub Encrypted Secrets, sourced from the offline HSM at sign time),
   - publishes to the public mirror under `repo.nullxes.os/$arch/`.
5. Run `repo-add --sign` on the production repo host to update `nullxes.db`.

## Reproducibility check

For each tagged release, an automated job rebuilds every package twice in
fresh containers and asserts byte-for-byte equality of `*.pkg.tar.zst`. A
mismatch blocks the release.

## Key handling

- The release private key never leaves a YubiKey.
- CI uses a SOPS-encrypted ephemeral signing identity that is destroyed on
  every job exit. Production signatures are produced offline against the
  YubiKey by an authorised maintainer.
