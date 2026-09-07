#!/bin/bash
set -euo pipefail
runtime_tools="$(cd "$(dirname "$0")" && pwd)"
runtime_dest="${TARGET_BUILD_DIR:?}/${EXECUTABLE_FOLDER_PATH:?}/plugin-runtime"
# Release app bundles can be universal; select the binary at runtime, not build-host architecture.
for node_arch in arm64 x64; do
  cmake "-DNODE_TARGET=darwin-$node_arch" \
    "-DNODE_CACHE=$runtime_tools/../../target/node-runtime-cache" \
    "-DNODE_DEST=$runtime_dest" -P "$runtime_tools/prepare.cmake"
  chmod +x "$runtime_dest/node-$node_arch"
  if [ "${CODE_SIGNING_ALLOWED:-NO}" = YES ]; then
    /usr/bin/codesign --force --sign "${EXPANDED_CODE_SIGN_IDENTITY:--}" \
      --options runtime --entitlements "$runtime_tools/node-macos.entitlements" \
      "$runtime_dest/node-$node_arch"
  fi
done
