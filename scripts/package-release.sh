#!/usr/bin/env bash
# Builds a portable Linux x86-64 release of HalfSpeedBtw into dist/.
#
# Links against glibc 2.31 using cargo-zigbuild, so the plugin loads on Ubuntu 20.04,
# Debian 11, Fedora 32 and newer instead of only on the build machine's glibc.
#
# Requires: cargo-zigbuild and zig (Arch: `pacman -S zig cargo-zigbuild`), objdump, python3.
# Usage: scripts/package-release.sh
set -euo pipefail

cd "$(dirname "$0")/.."

GLIBC=2.31
TARGET=x86_64-unknown-linux-gnu
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
NAME="HalfSpeedBtw-${VERSION}-linux-x86_64"
STAGE="dist/${NAME}"

if [[ -n "$(git status --porcelain)" ]]; then
    echo "Working tree is not clean; commit or stash first so the release matches a commit." >&2
    exit 1
fi

cargo zigbuild --release -p halfspeedbtw --target "${TARGET}.${GLIBC}"
LIB="target/${TARGET}/release/libhalfspeedbtw.so"

# Refuse to ship a binary that needs a newer glibc than promised.
NEWEST=$(objdump -T "$LIB" | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -uV | tail -n 1)
if [[ "$(printf '%s\n%s\n' "$NEWEST" "$GLIBC" | sort -V | tail -n 1)" != "$GLIBC" ]]; then
    echo "Binary needs glibc ${NEWEST}, newer than ${GLIBC}" >&2
    exit 1
fi

rm -rf "$STAGE" "dist/${NAME}.tar.gz"
mkdir -p "$STAGE/HalfSpeedBtw.vst3/Contents/x86_64-linux"
cp "$LIB" "$STAGE/HalfSpeedBtw.clap"
cp "$LIB" "$STAGE/HalfSpeedBtw.vst3/Contents/x86_64-linux/HalfSpeedBtw.so"
cp README.md LICENSE "$STAGE/"
python3 -I scripts/third_party_licenses.py "$STAGE/THIRD-PARTY-LICENSES.txt"

cat > "$STAGE/INSTALL.txt" <<EOF
HalfSpeedBtw ${VERSION} for Linux x86-64 (glibc ${GLIBC} or newer)

Install for your user:

    mkdir -p ~/.clap ~/.vst3
    cp HalfSpeedBtw.clap ~/.clap/
    cp -r HalfSpeedBtw.vst3 ~/.vst3/

Or for all users:

    sudo mkdir -p /usr/lib/clap /usr/lib/vst3
    sudo cp HalfSpeedBtw.clap /usr/lib/clap/
    sudo cp -r HalfSpeedBtw.vst3 /usr/lib/vst3/

Then rescan plugins in your DAW. The editor needs X11 or XWayland and a Vulkan driver.
Source code: https://github.com/PluginsBTW/HalfSpeedBtw
EOF

tar -C dist -czf "dist/${NAME}.tar.gz" "$NAME"
(cd dist && sha256sum "${NAME}.tar.gz" > "${NAME}.tar.gz.sha256")

echo "Built dist/${NAME}.tar.gz (needs glibc >= ${NEWEST}, promised ${GLIBC})"
cat "dist/${NAME}.tar.gz.sha256"
