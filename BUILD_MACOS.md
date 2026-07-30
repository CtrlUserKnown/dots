# Building dots on macOS

Steps to build the `dots` CLI on a macOS machine. The Rust code lives in this
repo's Cargo workspace (crates `dots` + `tui-core`); the `dots` binary is the
product. Works on both Apple Silicon (arm64) and Intel (x86_64).

Unlike ssm (whose keychain backend links Apple's `Security.framework`), `dots`
has no platform-locked dependencies — no keychain, no D-Bus, pure-Rust TLS — so
it's simpler to build across platforms. Building natively on macOS is still the
simplest path (cross-compiling to macOS from Linux would need the Apple SDK).

## 1. Prerequisites

```bash
# Apple Command Line Tools — provides clang, which `ring` (via ureq's rustls
# TLS) needs to compile.
xcode-select --install

# Rust toolchain (skip if you already have rustup)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

No Homebrew packages required. `dots`'s only networked dependency is `ureq`,
which uses **rustls** (pure-Rust TLS) — so there is no OpenSSL / system TLS
library to install. A C compiler (from the Command Line Tools) is the only
native requirement.

## 2. Build

Run from the repo root (the workspace directory):

```bash
cargo build --release
# binary: target/release/dots
```

Quick check:

```bash
./target/release/dots --version
./target/release/dots --help
```

## 3. Test

```bash
cargo test            # runs the whole workspace (dots + tui-core)
```

## 4. (Optional) Universal binary (arm64 + x86_64)

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

lipo -create -output dots-universal \
  target/aarch64-apple-darwin/release/dots \
  target/x86_64-apple-darwin/release/dots

lipo -info dots-universal   # -> "x86_64 arm64"
```

## 5. (Optional) Code signing & notarization

`dots` does **not** require signing to build or run locally — a plain
`cargo build` binary works. Signing only matters if you distribute it to other
Macs, so users don't hit a Gatekeeper "unidentified developer" warning.

- **Personal use:** no signing needed. (If you want to silence Gatekeeper on a
  file you downloaded, `xattr -d com.apple.quarantine ./dots`.)
- **Distribution:** sign with a **Developer ID Application** certificate
  (Apple Developer Program, $99/yr) and notarize:

  ```bash
  codesign --sign "Developer ID Application: <You> (<TEAMID>)" \
    --options runtime --timestamp target/release/dots

  ditto -c -k --keepParent target/release/dots dots.zip
  xcrun notarytool submit dots.zip \
    --key AuthKey.p8 --key-id <KEYID> --issuer <ISSUER> --wait
  ```

  (Bare CLIs can't be stapled; wrap in a `.pkg`/`.dmg` to staple, or rely on
  Gatekeeper's online notarization check.)

## Releasing

Publishing a release doesn't require building anything but the tag — GitHub
Actions builds every platform binary for you:

1. Bump `[workspace.package].version` in `Cargo.toml` (and let `Cargo.lock`
   pick it up), commit.
2. Tag it and push the tag: `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. `.github/workflows/release.yml` picks up the tag push, verifies it matches
   `Cargo.toml`, and builds all four targets — `linux/x86_64`,
   `linux/aarch64`, `darwin/x86_64`, `darwin/aarch64` — entirely on GitHub's
   runners (the Linux builds don't need a Linux machine, and the Intel macOS
   build doesn't need Intel hardware). Each gets packaged as
   `dots-vX.Y.Z-<os>-<arch>.tar.gz` plus a `.sha256` sidecar and uploaded to
   the GitHub Release.
4. `install.sh` and `dots update` both pick the matching asset up
   automatically — no manual step needed after the tag lands.

A tag pushed from a MacBook is enough to publish a Linux build; nothing here
needs to run on Linux.

## Notes

- `dots` checks GitHub for release updates over HTTPS (via `ureq`), so the
  `update` subcommand needs network access at runtime — but the build itself
  does not.
- Because there are no platform-specific dependencies, the release profile
  (`opt-level = "z"`, `lto`, `strip`) behaves the same as on Linux; expect a
  small, self-contained binary.

## Troubleshooting

- **`xcrun: error: unable to find utility` / linker errors** → install the
  Command Line Tools: `xcode-select --install`.
- **`ring` fails to compile** → almost always a missing C compiler; the Command
  Line Tools provide it.
- **codesign "identity not found"** → list available identities with
  `security find-identity -v -p codesigning`.
