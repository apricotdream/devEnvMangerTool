#!/usr/bin/env bash
# Windows / Linux 共用版本和校验逻辑；--build-only 只构建。
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
exec python3 scripts/release.py --platform linux "$@"
