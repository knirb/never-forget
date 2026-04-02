#!/bin/bash
set -e

# Release script: creates a GitHub release from the latest git tag
# Usage:
#   git tag v0.1.0
#   ./release.sh

APP_NAME="NeverForget"

# Get version from latest git tag
VERSION=$(git describe --tags --abbrev=0 2>/dev/null | sed 's/^v//')
if [ -z "$VERSION" ]; then
    echo "Error: No git tag found. Create one with: git tag v<version>"
    exit 1
fi

TAG="v$VERSION"
ZIP_NAME="${APP_NAME}-${VERSION}.zip"

# Check the tag is pushed
if ! git ls-remote --tags origin | grep -q "refs/tags/$TAG"; then
    echo "Tag $TAG not pushed yet. Pushing..."
    git push origin "$TAG"
fi

# Check if release already exists
if gh release view "$TAG" &>/dev/null; then
    echo "Error: Release $TAG already exists on GitHub"
    exit 1
fi

# Build and bundle
echo "==> Building $APP_NAME $VERSION..."
./bundle.sh

# Create zip
echo "==> Creating zip..."
cd target
rm -f "$ZIP_NAME"
zip -r "$ZIP_NAME" "${APP_NAME}.app"
cd ..

SHA=$(shasum -a 256 "target/$ZIP_NAME" | awk '{print $1}')
echo "==> SHA-256: $SHA"

# Create GitHub release
echo "==> Creating GitHub release $TAG..."
gh release create "$TAG" "target/$ZIP_NAME" \
    --title "$TAG" \
    --notes "Release $VERSION"

echo ""
echo "==> Released! https://github.com/knirb/never-forget/releases/tag/$TAG"
echo ""
echo "Homebrew cask sha256: $SHA"
