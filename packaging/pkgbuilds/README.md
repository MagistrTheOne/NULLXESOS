# NULLXES PKGBUILDs

Per-component PKGBUILD trees for the `nullxes` pacman repository. Each subdir
is shipped as the upstream source for `makepkg` runs.

## Build flow (per component)

```bash
cd packaging/pkgbuilds/nullxes-frame/
# Stage source: copy the workspace tarball next to the PKGBUILD.
tar --use-compress-program=zstd -caf source.tar.zst \
    --transform "s,^,nullxes-frame-0.1.0/," \
    crates/ Cargo.toml Cargo.lock packaging/
# Build.
makepkg --syncdeps --rmdeps --noconfirm
```

## Reproducibility

- `RUSTFLAGS="-C codegen-units=1 -C debuginfo=0"` set at `build()` time.
- `SOURCE_DATE_EPOCH` derived from git timestamp by the CI orchestrator.
- `cargo build --frozen --locked` ensures vendored dep set is identical.

Verify reproducibility with `diffoscope`:

```bash
makepkg -f && cp *.pkg.tar.zst /tmp/a.zst
makepkg -f && cp *.pkg.tar.zst /tmp/b.zst
diffoscope /tmp/a.zst /tmp/b.zst
```

## Repo signing

The signing key never leaves a hardware token. The CI signer container loads
it from a SOPS-encrypted secret and invokes `repo-add --sign` with a single
ephemeral signature per release tag.
