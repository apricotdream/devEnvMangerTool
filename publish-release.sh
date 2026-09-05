#!/usr/bin/env bash
# DevEnvManager Linux/CLI 发布脚本（在 WSL2 或 Linux 上运行）。
#
# 前置：GITEE_TOKEN 环境变量；curl / git / cargo（rustup）
# 用法：./publish-release.sh
#
# 对比 Windows 版 publish-release.cmd 的差异：
#   - 只发布 CLI 二进制（devenv-Linux，无 GUI 安装包）
#   - 默认构建 x86_64-unknown-linux-musl 静态自包含二进制
#     （无需系统 cc/gcc + glibc，覆盖面最广；无缝依赖 rust-lld）
#   - 产物：devenv-<target>-<version>.tar.gz + install.sh + SHA256SUMS.txt
set -euo pipefail
cd "$(dirname "$0")"

# =====================================================
#   Config
# =====================================================
REPO="apricotdream/devEnvMangerTool"
GITEE_API="https://gitee.com/api/v5"
CORE_DIR="$(pwd)/core"
OUT="$(pwd)/dist"
TARGET="x86_64-unknown-linux-musl"
SOURCE_BRANCH="main"          # 默认从 main 打 tag（发布前需已推送 release 提交）
TMP_DIR="$(mktemp -d)"
RJSON="$TMP_DIR/release.json"
UJSON="$TMP_DIR/upload.json"

# =====================================================
#   Version from core/Cargo.toml (single source)
# =====================================================
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$CORE_DIR/Cargo.toml" | head -1)"
TAG="v$VERSION"
BODY_TEXT="DevEnvManager v$VERSION (auto release)"
echo "============================================"
echo "  DevEnvManager Release publish v$VERSION"
echo "  target: $TARGET"
echo "============================================"

if [[ -z "$VERSION" ]]; then
  echo "[ERROR] Cannot read version from core/Cargo.toml"; exit 1
fi
: "${GITEE_TOKEN:?[ERROR] GITEE_TOKEN env var not set. Run: export GITEE_TOKEN=your_token}"
command -v curl >/dev/null || { echo "[ERROR] curl not found"; exit 1; }
command -v git  >/dev/null || { echo "[ERROR] git not found"; exit 1; }
# rustup 默认不写 PATH（--no-modify-path），自动补入
if ! command -v cargo >/dev/null 2>&1; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi
command -v cargo >/dev/null || { echo "[ERROR] cargo not found (rustup)"; exit 1; }
# git 网络操作全部走非交互 + 短超时，避免 WSL 无 SSH 密钥时卡死
export GIT_SSH_COMMAND="ssh -o BatchMode=yes -o ConnectTimeout=5"
export GIT_TERMINAL_PROMPT=0

# -----------------------------------------------------
# Conflict checks（都用 Gitee API，不依赖本地 git 网络）
# -----------------------------------------------------
echo "[*] Checking Gitee for existing release $TAG ..."
if curl -s "$GITEE_API/repos/$REPO/releases/tags/$TAG?access_token=$GITEE_TOKEN" | grep -qi "tag_name"; then
  echo "[ERROR] Release $TAG already exists on Gitee."; exit 1
fi
echo "[OK] No existing release on Gitee."

echo "[*] Checking Gitee for existing git tag $TAG ..."
if curl -s "$GITEE_API/repos/$REPO/git/refs/tags/$TAG?access_token=$GITEE_TOKEN" | grep -q '"ref"'; then
  echo "[ERROR] Git tag $TAG already exists on origin."; exit 1
fi
echo "[OK] No existing git tag."

# =====================================================
#  1. Clean & build (musl static, rust-lld self-contained)
# =====================================================
echo "[1/5] Cleaning dist/ ..."
rm -rf "$OUT"
mkdir -p "$OUT"

echo "[2/5] cargo build --release --target $TARGET ..."
export RUSTFLAGS="-C linker=rust-lld -C link-self-contained=yes"
cargo build -p devenv-core --bin devenv --release --target "$TARGET"
echo "[OK] build done."

# =====================================================
#  2. Collect artifacts
# =====================================================
echo "[3/5] Collecting artifacts ..."
BIN="target/$TARGET/release/devenv"
if [[ ! -f "$BIN" ]]; then
  echo "[ERROR] binary not found: $BIN"; exit 1
fi

# tarball: 解压即得 devenv 单文件
PKG_NAME="devenv-$TARGET-$VERSION"
PKG_DIR="$TMP_DIR/$PKG_NAME"
mkdir -p "$PKG_DIR"
cp "$BIN" "$PKG_DIR/devenv"
tar czf "$OUT/$PKG_NAME.tar.gz" -C "$TMP_DIR" "$PKG_NAME"
echo "[OK] tarball    : $OUT/$PKG_NAME.tar.gz"

cp "$(pwd)/install.sh" "$OUT/install.sh"
echo "[OK] install.sh : $OUT/install.sh"

# SHA256SUMS（相对文件名，不含自身；下载后与产物同目录即可 sha256sum -c 校验）
( cd "$OUT" && sha256sum devenv-*.tar.gz install.sh ) > "$OUT/SHA256SUMS.txt"
echo "[OK] checksums  : $OUT/SHA256SUMS.txt"

# =====================================================
#  3. Git: tag origin/$SOURCE_BRANCH (do NOT move it)
# =====================================================
echo "[4/5] Creating git tag $TAG at origin/$SOURCE_BRANCH ..."
git rev-parse --verify "origin/$SOURCE_BRANCH" >/dev/null 2>&1 \
  || { echo "[ERROR] 本地缺少 origin/$SOURCE_BRANCH（先执行: git fetch origin ${SOURCE_BRANCH}）"; exit 1; }
git tag "$TAG" "origin/$SOURCE_BRANCH" 2>/dev/null || { echo "[ERROR] Local tag $TAG exists."; exit 1; }
if ! git push origin "$TAG" 2>/dev/null; then
  echo "[ERROR] 推送 tag 失败（请检查 git push 凭据）: git push origin $TAG"
  exit 1
fi
echo "[OK] Pushed git tag $TAG at origin/$SOURCE_BRANCH."

# =====================================================
#  4. Gitee: create release and upload artifacts
# =====================================================
echo "[5/5] Creating Gitee release $TAG ..."
curl -s -f -X POST "$GITEE_API/repos/$REPO/releases" \
  -F "access_token=$GITEE_TOKEN" \
  -F "tag_name=$TAG" \
  -F "target_commitish=$SOURCE_BRANCH" \
  -F "name=$TAG" \
  -F "body=$BODY_TEXT" \
  -F "prerelease=true" \
  -o "$RJSON" || { echo "[ERROR] Create release failed. Response:"; cat "$RJSON" 2>/dev/null; exit 1; }
RELEASE_ID="$(sed -n 's/.*"id":\([0-9]*\).*/\1/p' "$RJSON" | head -1)"
if [[ -z "$RELEASE_ID" ]]; then
  echo "[ERROR] Could not parse release id. Response:"; cat "$RJSON"; exit 1
fi
echo "[OK] Created release $TAG id=$RELEASE_ID"

echo "[*] Uploading artifacts ..."
for f in "$OUT"/*; do
  echo "     Upload $(basename "$f") ..."
  curl -s -f -X POST "$GITEE_API/repos/$REPO/releases/$RELEASE_ID/attach_files" \
    -F "access_token=$GITEE_TOKEN" \
    -F "file=@$f" \
    -o "$UJSON" && echo "     [OK] $(basename "$f") uploaded" \
      || { echo "     [WARN] $(basename "$f") upload may have failed:"; cat "$UJSON" 2>/dev/null; }
done

rm -rf "$TMP_DIR"
echo
echo "============================================================"
echo "  Done. Verify release attachments on Gitee (incl SHA256SUMS.txt)."
echo "  dist: $OUT"
echo "  tag : $TAG"
echo "============================================================"
