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

# ---------- 1) 本地已有 tarball（含 musl 回退）----------
for pat in "devenv-$TARGET-*.tar.gz" "devenv-$ARCH-unknown-linux-musl-*.tar.gz" "devenv-[[:alnum:]_-]*-unknown-linux-*.tar.gz"; do
  if found="$(ls $pat 2>/dev/null | head -1)"; then
    TARBALL="$found"
    echo "[*] 使用本地文件: $TARBALL"
    break
  fi
done

# 附件选择：精确 target → musl 静态回退 → 任意 linux tarball
pick_asset() {
  local json="$1" name pat
  for pat in \
    "\"name\":\"devenv-$TARGET-[^\"]*\.tar\.gz\"" \
    "\"name\":\"devenv-$ARCH-unknown-linux-musl-[^\"]*\.tar\.gz\"" \
    "\"name\":\"devenv-[^\"]*unknown-linux-[^\"]*\.tar\.gz\""; do
    name="$(echo "$json" | grep -o "$pat" | head -1 || true)"
    if [[ -n "$name" ]]; then
      name="${name#\\\"name\\\":\\\"}"   # 去掉 "name":"
      name="${name%\\\"}"                # 去掉尾部括引号
      echo "$name"
      return 0
    fi
  done
  return 1
}

# ---------- 2) 远端 release 附件 ----------
if [[ -z "$TARBALL" ]]; then
  # latest：按创建时间倒序逐条找含 Linux 附件的 release（Gitee API 默认按 id 升序，需显式排序）
  if [[ -z "$TAG" ]]; then
    list_json="$(curl -s "$GITEE_API/repos/$REPO/releases?per_page=20&sort=created_at&direction=desc")"
    for cand_tag in $(echo "$list_json" | grep -o '"tag_name":"[^"]*"' | sed 's/.*:"//; s/"$//'); do
      d="$(curl -s "$GITEE_API/repos/$REPO/releases/tags/$cand_tag")"
      if name="$(pick_asset "$d")"; then
        TAG="$cand_tag"
        break
      fi
    done
    [[ -z "$TAG" ]] && { echo "[ERROR] 未在任何 release 找到 Linux 附件（devenv-*-unknown-linux-*.tar.gz）"; exit 1; }
    echo "[*] 使用最新含 Linux 附件的 release: $TAG"
  else
    d="$(curl -s "$GITEE_API/repos/$REPO/releases/tags/$TAG")"
    name="$(pick_asset "$d")" || { echo "[ERROR] release $TAG 无匹配附件，请检查版本号"; exit 1; }
  fi
  DOWNLOAD_URL="https://gitee.com/$REPO/releases/download/$TAG/$name"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
if [[ -n "$DOWNLOAD_URL" ]]; then
  echo "[*] 下载: $DOWNLOAD_URL"
  curl -f -L -s -o "$TMP/devenv.tar.gz" "$DOWNLOAD_URL"
  TARBALL="$TMP/devenv.tar.gz"
fi

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
