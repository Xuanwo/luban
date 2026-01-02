#!/usr/bin/env bash
set -euo pipefail

version="0.14.1"
dest_dir=".context/zig"
dest_bin="${dest_dir}/zig"

if [[ -x "${dest_bin}" ]]; then
  "${dest_bin}" version >/dev/null 2>&1 && exit 0
fi

mkdir -p "${dest_dir}"

os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"

case "${os}" in
  darwin)
    platform="macos"
    ;;
  linux)
    platform="linux"
    ;;
  *)
    echo "Unsupported OS: ${os}" >&2
    exit 1
    ;;
esac

case "${arch}" in
  arm64|aarch64)
    arch="aarch64"
    ;;
  x86_64|amd64)
    arch="x86_64"
    ;;
  *)
    echo "Unsupported arch: ${arch}" >&2
    exit 1
    ;;
esac

archive="zig-${arch}-${platform}-${version}.tar.xz"
url="https://ziglang.org/download/${version}/${archive}"

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

echo "Downloading ${url}" >&2
curl -fsSL "${url}" -o "${tmp}/${archive}"
tar -C "${tmp}" -xf "${tmp}/${archive}"

zig_dir="${tmp}/zig-${arch}-${platform}-${version}"
if [[ ! -x "${zig_dir}/zig" ]]; then
  echo "Unexpected archive layout: ${zig_dir}/zig missing" >&2
  exit 1
fi

rm -rf "${dest_dir}/lib"
cp "${zig_dir}/zig" "${dest_bin}"
cp -R "${zig_dir}/lib" "${dest_dir}/lib"
chmod +x "${dest_bin}"

echo "Installed Zig ${version} to $(pwd)/${dest_bin}" >&2
