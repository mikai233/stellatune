#!/usr/bin/env bash

set -euo pipefail

configuration="Release"
build_target="wasm32-wasip2"
out_dir=""
skip_build=0

usage() {
  cat <<'EOF'
Usage: package-unix.sh [options]

Options:
  --configuration <Debug|Release>  Build profile to package. Default: Release
  --build-target <triple>          Rust target for the plugin wasm. Default: wasm32-wasip2
  --out-dir <path>                 Output directory for the packaged zip. Default: <repo>/target/plugins
  --skip-build                     Reuse existing build artifacts without running cargo build
  -h, --help                       Show this help text
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --configuration)
      [[ $# -ge 2 ]] || { echo "missing value for --configuration" >&2; exit 1; }
      configuration="$2"
      shift 2
      ;;
    --build-target)
      [[ $# -ge 2 ]] || { echo "missing value for --build-target" >&2; exit 1; }
      build_target="$2"
      shift 2
      ;;
    --out-dir)
      [[ $# -ge 2 ]] || { echo "missing value for --out-dir" >&2; exit 1; }
      out_dir="$2"
      shift 2
      ;;
    --skip-build)
      skip_build=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

case "$configuration" in
  Debug) profile_dir="debug" ;;
  Release) profile_dir="release" ;;
  *)
    echo "invalid configuration: $configuration (expected Debug or Release)" >&2
    exit 1
    ;;
esac

case "$(uname -s)" in
  Linux|Darwin) ;;
  *)
    echo "This script only supports Unix-like environments." >&2
    exit 1
    ;;
esac

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required but was not found in PATH" >&2
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required but was not found in PATH" >&2
  exit 1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
plugin_crate_dir="$(cd "${script_dir}/.." && pwd)"
plugin_manifest_path="${plugin_crate_dir}/Cargo.toml"
plugin_json_path="${plugin_crate_dir}/plugin.json"
repo_root="$(cd "${plugin_crate_dir}/../../.." && pwd)"
cargo_target_dir="${repo_root}/target"

[[ -f "$plugin_manifest_path" ]] || { echo "plugin manifest not found: $plugin_manifest_path" >&2; exit 1; }
[[ -f "$plugin_json_path" ]] || { echo "plugin.json not found: $plugin_json_path" >&2; exit 1; }

if [[ -z "$out_dir" ]]; then
  out_dir="${cargo_target_dir}/plugins"
fi
mkdir -p "$out_dir"
out_dir="$(cd "$out_dir" && pwd)"

plugin_metadata="$(
  python3 - "$plugin_json_path" <<'PY'
import json
import sys

plugin_json = sys.argv[1]
with open(plugin_json, "r", encoding="utf-8") as fh:
    manifest = json.load(fh)

plugin_id = manifest.get("id")
version = manifest.get("version")
components = manifest.get("components") or []
if not plugin_id:
    raise SystemExit("plugin.json missing id")
if not version:
    raise SystemExit("plugin.json missing version")
if not components:
    raise SystemExit("plugin.json has no components")

print(plugin_id)
print(version)
for component in components:
    path = component.get("path", "").strip()
    if not path:
        raise SystemExit("component.path is empty in plugin.json")
    print(path)
PY
)"

plugin_id="$(printf '%s\n' "$plugin_metadata" | sed -n '1p')"
plugin_version="$(printf '%s\n' "$plugin_metadata" | sed -n '2p')"
mapfile -t component_paths < <(printf '%s\n' "$plugin_metadata" | sed -n '3,$p')

invoke_cargo() {
  cargo "$@"
}

create_zip() {
  local stage_dir="$1"
  local zip_path="$2"

  rm -f "$zip_path"
  if command -v zip >/dev/null 2>&1; then
    (
      cd "$stage_dir"
      zip -qr "$zip_path" .
    )
    return
  fi

  python3 - "$stage_dir" "$zip_path" <<'PY'
import pathlib
import sys
import zipfile

stage_dir = pathlib.Path(sys.argv[1])
zip_path = pathlib.Path(sys.argv[2])

with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
    for path in sorted(stage_dir.rglob("*")):
        if path.is_file():
            zf.write(path, path.relative_to(stage_dir))
PY
}

safe_zip_stem="$(printf '%s' "${plugin_id}-${plugin_version}-${build_target}-${profile_dir}" | tr '/:\\' '___')"
wasm_build_dir="${cargo_target_dir}/${build_target}/${profile_dir}"
stage_dir="${out_dir}/stellatune-plugin-ncm-stage"
zip_path="${out_dir}/${safe_zip_stem}.zip"

prev_cargo_target_dir="${CARGO_TARGET_DIR-__UNSET__}"
export CARGO_TARGET_DIR="$cargo_target_dir"

cleanup() {
  if [[ "$prev_cargo_target_dir" == "__UNSET__" ]]; then
    unset CARGO_TARGET_DIR || true
  else
    export CARGO_TARGET_DIR="$prev_cargo_target_dir"
  fi
}
trap cleanup EXIT

if [[ "$skip_build" -eq 0 ]]; then
  build_args=(build --manifest-path "$plugin_manifest_path" --target "$build_target")
  if [[ "$configuration" == "Release" ]]; then
    build_args+=(--release)
  fi
  (
    cd "$repo_root"
    invoke_cargo "${build_args[@]}"
  )
fi

rm -rf "$stage_dir"
mkdir -p "$stage_dir"
cp "$plugin_json_path" "${stage_dir}/plugin.json"

for relative_path in "${component_paths[@]}"; do
  file_name="$(basename "$relative_path")"
  source_path="${wasm_build_dir}/${file_name}"
  [[ -f "$source_path" ]] || { echo "component wasm not found: $source_path" >&2; exit 1; }

  destination_path="${stage_dir}/${relative_path}"
  mkdir -p "$(dirname "$destination_path")"
  cp "$source_path" "$destination_path"
done

create_zip "$stage_dir" "$zip_path"

echo
echo "Package ready:"
echo "  $zip_path"
echo
echo "Install this zip from StellaTune Settings -> Plugins -> Install."
