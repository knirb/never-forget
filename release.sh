#!/bin/bash
set -e

APP_NAME="NeverForget"
BUNDLE_DIR="target/${APP_NAME}.app"

VERSION=$(grep '^version' Cargo.toml | head -n1 | sed -E 's/version *= *"([^"]+)".*/\1/')
if [ -z "$VERSION" ]; then
  echo "Could not parse version from Cargo.toml" >&2
  exit 1
fi

ZIP_NAME="${APP_NAME}-${VERSION}.zip"
ZIP_PATH="target/${ZIP_NAME}"

./bundle.sh

echo "Zipping bundle..."
rm -f "$ZIP_PATH"
# ditto preserves macOS metadata + signature; plain `zip` can mangle bundles.
ditto -c -k --sequesterRsrc --keepParent "$BUNDLE_DIR" "$ZIP_PATH"

SHA=$(shasum -a 256 "$ZIP_PATH" | awk '{print $1}')

echo ""
echo "Release artifact: $ZIP_PATH"
echo "Version:          $VERSION"
echo "sha256:           $SHA"
echo ""
echo "Next steps:"
echo "  1. gh release create v${VERSION} ${ZIP_PATH} --title \"v${VERSION}\" --notes \"...\""
echo "  2. Update Casks/neverforget.rb in your tap with version ${VERSION} and the sha256 above."
