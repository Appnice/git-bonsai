#!/usr/bin/env bash
# Builds release archives for the five targets the action supports into dist/, each with a .sha256.
# Runs on this Mac (no GitHub Actions): Linux and Windows through cargo-zigbuild, macOS natively.
# Uploading is a separate, explicit step: scripts/release.sh --upload vX.Y.Z (needs gh, asks nothing).
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"

TARGETS=(x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-gnu)
rm -rf dist && mkdir -p dist
for t in "${TARGETS[@]}"; do
  case "$t" in
    *apple-darwin) cargo build --release --locked --features gif --target "$t" ;;
    *)             cargo zigbuild --release --locked --features gif --target "$t" ;;
  esac
  bin=git-bonsai; [[ "$t" == *windows* ]] && bin=git-bonsai.exe
  stage=$(mktemp -d)
  cp "target/$t/release/$bin" README.md "$stage/"
  if [[ "$t" == *windows* ]]; then
    (cd "$stage" && zip -q "$OLDPWD/dist/git-bonsai-$t.zip" "$bin" README.md)
    a="dist/git-bonsai-$t.zip"
  else
    tar czf "dist/git-bonsai-$t.tar.gz" -C "$stage" "$bin" README.md
    a="dist/git-bonsai-$t.tar.gz"
  fi
  (cd dist && shasum -a 256 "$(basename "$a")" > "$(basename "$a").sha256")
  rm -rf "$stage"
  printf '%-32s %8s bytes (binary %s)\n' "$t" "$(stat -f%z "$a")" "$(stat -f%z "target/$t/release/$bin")"
done

if [[ "${1:-}" == "--upload" ]]; then
  tag="${2:?usage: release.sh --upload vX.Y.Z}"
  gh release create "$tag" dist/* --title "$tag" --notes "Git Bonsai $tag"
fi
