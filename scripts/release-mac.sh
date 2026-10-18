#!/usr/bin/env bash
# Build, sign, notarize and staple River Writer for macOS.
#
# One-time setup (run yourself; River never sees these credentials):
#   1. Developer ID Application certificate in your login keychain.
#   2. rustup target add x86_64-apple-darwin        # universal (Intel + Apple Silicon)
#   3. xcrun notarytool store-credentials river-notary \
#        --apple-id YOU@EXAMPLE.COM --team-id TEAMID   # prompts for an app-specific password
#
# Usage:  scripts/release-mac.sh [--no-notarize] [--arm64-only]
set -euo pipefail
cd "$(dirname "$0")/.."

notarize=1
target=universal-apple-darwin
for arg in "$@"; do
  case "$arg" in
    --no-notarize) notarize=0 ;;
    --arm64-only) target=aarch64-apple-darwin ;;
    *) echo "Unknown option: $arg" >&2; exit 2 ;;
  esac
done
profile="${RIVER_NOTARY_PROFILE:-river-notary}"

identity="${APPLE_SIGNING_IDENTITY:-$(security find-identity -p codesigning -v |
  sed -n 's/.*"\(Developer ID Application: [^"]*\)".*/\1/p' | head -1)}"
if [[ -z "$identity" ]]; then
  echo "No 'Developer ID Application' certificate found in the keychain." >&2
  exit 1
fi
if [[ "$target" == universal-apple-darwin ]] &&
  ! rustup target list --installed | grep -qx x86_64-apple-darwin; then
  echo "Missing Rust target: run 'rustup target add x86_64-apple-darwin' (or use --arm64-only)." >&2
  exit 1
fi

export PATH="/opt/homebrew/bin:$PATH"
echo "==> Building and signing with: $identity"
APPLE_SIGNING_IDENTITY="$identity" pnpm --dir app tauri build --target "$target" --bundles app,dmg

bundle="app/src-tauri/target/$target/release/bundle"
app="$bundle/macos/River Writer.app"
dmg="$(ls -t "$bundle"/dmg/*.dmg | head -1)"

echo "==> Verifying code signature"
codesign --verify --deep --strict --verbose=2 "$app"
codesign -dv --verbose=2 "$app" 2>&1 | grep -E 'Authority=Developer ID|flags=.*runtime|Identifier=' 

if [[ $notarize == 1 ]]; then
  echo "==> Notarizing $dmg (profile: $profile)"
  xcrun notarytool submit "$dmg" --keychain-profile "$profile" --wait
  xcrun stapler staple "$app"
  xcrun stapler staple "$dmg"
  echo "==> Gatekeeper assessment"
  spctl --assess --type execute --verbose=2 "$app"
  spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg"
fi

shasum -a 256 "$dmg" | tee "$dmg.sha256"
echo "==> Done: $dmg"
