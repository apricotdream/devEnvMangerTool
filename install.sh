#!/usr/bin/env bash
# DevEnvManager CLI Linux 安装脚本。
#
# 用法:  ./install.sh [version]        （默认下载最新 release 的 Linux 附件）
#         ./install.sh 0.2.4-beta      （指定版本）
# 若当前目录已有 devenv-*-unknown-linux-*.tar.gz（如本地构建/下载产物），优先本地安装。
set -euo pipefail

REPO="apricotdream/devEnvMangerTool"
GITEE_API="https://gitee.com/api/v5"

VERSION="${1:-latest}"
TAG=""
if [[ "$VERSION" != "latest" ]]; then
  TAG="v${VERSION#v}"
fi

# 探测目标：<arch>-unknown-linux-(gnu|musl)，并允许 musl 静态包在 gnu 上回退（静态链接通用）
case "$(uname -m)" in
  x86_64)  ARCH="x86_64" ;;
  aarch64) ARCH="aarch64" ;;
  *) echo "[ERROR] 不支持的架构: $(uname -m)"; exit 1 ;;
esac
LIBC="gnu"
[[ -e "/lib/ld-musl-$ARCH.so.1" ]] && LIBC="musl"
TARGET="$ARCH-unknown-linux-$LIBC"
echo "[*] 目标: $TARGET  版本参数: $VERSION"

# 目标文件或 URL（本地 tarball 优先）
TARBALL=""
DOWNLOAD_URL=""
CHECKSUM_URL=""

# ---------- 1) 本地已有 tarball（含 musl 回退）----------
version_glob="*"
[[ "$VERSION" != latest ]] && version_glob="${VERSION#v}"
for pat in "devenv-$TARGET-$version_glob.tar.gz" "devenv-$ARCH-unknown-linux-musl-$version_glob.tar.gz"; do
  found="$(compgen -G "$pat" | sort -Vr | head -1 || true)"
  if [[ -n "$found" ]]; then
    TARBALL="$found"
    echo "[*] 使用本地文件: $TARBALL"
    break
  fi
done

# 附件选择：精确 target → 同架构 musl 静态包，不混用其它架构
# 直接 grep 出文件名本身（不含 "name":" 包装），免去引号剥离，避免 JSON 包裹污染下载 URL。
pick_asset() {
  local json="$1" name pat
  for pat in \
    "devenv-$TARGET-[^\"]*\.tar\.gz" \
    "devenv-$ARCH-unknown-linux-musl-[^\"]*\.tar\.gz"; do
    name="$(echo "$json" | grep -o "$pat" | head -1 || true)"
    if [[ -n "$name" ]]; then
      echo "$name"
      return 0
    fi
  done
  return 1
}

# ---------- 2) GitHub / Gitee 发行附件 ----------
get_json() { curl -fLSs --connect-timeout 5 --max-time 20 -H 'User-Agent: DevEnvManager' "$1"; }
find_release() {
  local tag="$1" api host endpoint json
  for host in github.com gitee.com; do
    if [[ "$host" == github.com ]]; then api="https://api.github.com"; else api="$GITEE_API"; fi
    endpoint="$api/repos/$REPO/releases/tags/$tag"
    if json="$(get_json "$endpoint")" && name="$(pick_asset "$json")"; then
      TAG="$tag"
      DOWNLOAD_URL="https://$host/$REPO/releases/download/$TAG/$name"
      RELEASE_BASE="https://$host/$REPO/releases/download/$TAG"
      return 0
    fi
  done
  return 1
}
if [[ -z "$TARBALL" ]]; then
  if [[ -z "$TAG" ]]; then
    tags="$({ get_json "https://api.github.com/repos/$REPO/releases?per_page=100" || true; get_json "$GITEE_API/repos/$REPO/releases?per_page=100" || true; } | grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' | sed 's/.*:[[:space:]]*"//; s/"$//' | sort -Vu | sort -Vr)"
    for candidate in $tags; do
      if find_release "$candidate"; then break; fi
    done
    [[ -n "$DOWNLOAD_URL" ]] || { echo "[ERROR] 发布源暂无当前架构的 Linux 包"; exit 1; }
  else
    find_release "$TAG" || { echo "[ERROR] 该版本没有当前架构的 Linux 包"; exit 1; }
  fi
fi

MANIFEST="SHA256SUMS-linux-$ARCH.txt"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
if [[ -n "$DOWNLOAD_URL" ]]; then
  echo "[*] 下载: $DOWNLOAD_URL"
  curl -f -L -s -o "$TMP/devenv.tar.gz" "$DOWNLOAD_URL"
  TARBALL="$TMP/devenv.tar.gz"
  CHECKSUM_URL="$RELEASE_BASE/$MANIFEST"
  if ! curl -fLSs --connect-timeout 8 --max-time 30 "$CHECKSUM_URL" -o "$TMP/checksums"; then
    curl -fLSs --connect-timeout 8 --max-time 30 "$RELEASE_BASE/SHA256SUMS.txt" -o "$TMP/checksums"
  fi
fi

if [[ -z "$DOWNLOAD_URL" ]]; then
  name="$(basename "$TARBALL")"
  sum_dir="$(dirname "$TARBALL")"
  if [[ -f "$sum_dir/$MANIFEST" ]]; then cp "$sum_dir/$MANIFEST" "$TMP/checksums"
  elif [[ -f "$sum_dir/SHA256SUMS.txt" ]]; then cp "$sum_dir/SHA256SUMS.txt" "$TMP/checksums"
  else echo "[ERROR] 本地包缺少 SHA256 校验文件"; exit 1; fi
fi
expected="$(awk -v file="$name" '$2 == file || $2 == "*" file {print $1; exit}' "$TMP/checksums")"
[[ "$expected" =~ ^[[:xdigit:]]{64}$ ]] || { echo "[ERROR] 校验文件缺少该平台的附件"; exit 1; }
actual="$(sha256sum "$TARBALL" | cut -d ' ' -f1)"
[[ "${expected,,}" == "$actual" ]] || { echo "[ERROR] SHA256 校验失败，未安装"; exit 1; }
tar xzf "$TARBALL" -C "$TMP"

BIN_DIR="$HOME/.local/bin"
mkdir -p "$BIN_DIR"
install -m 0755 "$(ls -d "$TMP"/devenv-*)"/devenv "$BIN_DIR/devenv"
echo "✓ 安装完成: $BIN_DIR/devenv"

# 提示 PATH
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
  echo "[!] ~/.local/bin 不在 PATH，执行以下之一后重开终端："
  echo "      echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc && source ~/.bashrc"
  echo "      或 (zsh): echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc && source ~/.zshrc"
fi
echo "  运行 'devenv --help' 开始使用；'devenv doctor' 体检环境。"
