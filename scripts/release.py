#!/usr/bin/env python3
"""Windows / Linux 共用发版流程；--build-only 仅构建，不推送或发布。"""
import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import tarfile
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPO = "apricotdream/devEnvMangerTool"
os.chdir(ROOT)


def run(*args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def digest(path):
    with path.open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest() if hasattr(hashlib, "file_digest") else hashlib.sha256(file.read()).hexdigest()


def version():
    core = re.search(r'^version = "([^"]+)"', (ROOT / "core/Cargo.toml").read_text(encoding="utf-8"), re.M)[1]
    shell = re.search(r'^version = "([^"]+)"', (ROOT / "src-tauri/Cargo.toml").read_text(encoding="utf-8"), re.M)[1]
    gui = json.loads((ROOT / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))["version"]
    if core != shell or core != gui:
        raise RuntimeError("core、GUI 与 tauri.conf.json 版本不一致")
    return core


def gitee(method, endpoint, fields=None, file=None):
    # Token 只从环境进入请求，不出现在命令行、日志或仓库文件中。
    token = os.environ["GITEE_TOKEN"]
    url = "https://gitee.com/api/v5/repos/" + REPO + endpoint
    data = None
    headers = {"User-Agent": "DevEnvManager-release"}
    if method == "GET":
        url += ("&" if "?" in url else "?") + urllib.parse.urlencode({"access_token": token})
    elif file:
        boundary = "devenv-release-upload"
        data = (f"--{boundary}\r\nContent-Disposition: form-data; name=\"access_token\"\r\n\r\n{token}\r\n"
                f"--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file.name}\"\r\n"
                "Content-Type: application/octet-stream\r\n\r\n").encode() + file.read_bytes() + f"\r\n--{boundary}--\r\n".encode()
        headers["Content-Type"] = "multipart/form-data; boundary=" + boundary
    else:
        data = urllib.parse.urlencode({"access_token": token, **(fields or {})}).encode()
    try:
        with urllib.request.urlopen(urllib.request.Request(url, data, headers, method=method), timeout=180) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        if method == "GET" and error.code == 404:
            return None
        raise RuntimeError(f"Gitee {method} {endpoint} 失败：HTTP {error.code}") from None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-only", action="store_true")
    parser.add_argument("--skip-build", action="store_true", help="验证并发布当前平台已构建的产物")
    parser.add_argument("--provider", choices=["both", "gitee", "github"], default="both")
    parser.add_argument("--platform", choices=["windows", "linux"], default="windows" if os.name == "nt" else "linux")
    args = parser.parse_args()
    ver = version()
    tag = "v" + ver
    machine = platform.machine().lower()
    arch = "aarch64" if machine in ("arm64", "aarch64") else "x86_64"
    win_arch = "arm64" if arch == "aarch64" else "x64"
    label = f"windows-{win_arch}" if args.platform == "windows" else f"linux-{arch}"
    out = ROOT / "dist" / label
    out.mkdir(parents=True, exist_ok=True)
    manifest = out / f"SHA256SUMS-{label}.txt"
    if not args.build_only:
        if args.provider in ("both", "gitee") and not os.environ.get("GITEE_TOKEN"):
            raise RuntimeError("发布 Gitee 需要 GITEE_TOKEN；可先用 --build-only，或 --provider github")
        if args.provider in ("both", "github"):
            run("gh", "auth", "status", stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if subprocess.check_output(["git", "status", "--porcelain"]).strip():
            raise RuntimeError("请先提交源码，再发布与该提交对应的版本")

    if args.platform == "windows":
        names = [f"DevEnvManager_{ver}_{win_arch}-setup.exe", f"devenv-cli-{ver}-windows-{win_arch}.zip"]
        if not args.skip_build:
            run("cargo", "tauri", "build", "--", "--locked", cwd=ROOT / "src-tauri")
            run("cargo", "build", "-p", "devenv-core", "--bin", "devenv", "--release", "--locked")
            shutil.copy2(ROOT / "target/release/bundle/nsis" / names[0], out / names[0])
            with zipfile.ZipFile(out / names[1], "w", zipfile.ZIP_DEFLATED) as archive:
                archive.write(ROOT / "target/release/devenv.exe", "devenv.exe")
    else:
        target = f"{arch}-unknown-linux-musl"
        name = f"devenv-{target}-{ver}"
        names = [name + ".tar.gz", "install.sh"]
        if not args.skip_build:
            env = {**os.environ, "RUSTFLAGS": "-C linker=rust-lld -C link-self-contained=yes"}
            run("cargo", "build", "-p", "devenv-core", "--bin", "devenv", "--release", "--locked", "--target", target, env=env)
            with tarfile.open(out / names[0], "w:gz") as archive:
                archive.add(ROOT / f"target/{target}/release/devenv", arcname=name + "/devenv")
            shutil.copy2(ROOT / "install.sh", out / "install.sh")

    files = [out / name for name in names]
    if any(not file.is_file() or file.stat().st_size == 0 for file in files):
        raise RuntimeError("当前平台的发布产物不完整")
    checksums = "".join(f"{digest(file)}  {file.name}\n" for file in files)
    if args.skip_build:
        if not manifest.is_file() or manifest.read_text(encoding="utf-8") != checksums:
            raise RuntimeError("已有产物与校验文件不一致，请重新构建")
    else:
        manifest.write_text(checksums, encoding="utf-8", newline="\n")
    print(f"{tag} / {label}：产物与 SHA256 已就绪 -> {out}")
    if args.build_only:
        return

    head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    existing = subprocess.run(["git", "rev-parse", "--verify", tag + "^{commit}"], capture_output=True, text=True)
    if existing.returncode == 0 and existing.stdout.strip() != head:
        raise RuntimeError("同名 tag 指向其它提交，已停止；请使用新版本号")
    if existing.returncode != 0:
        run("git", "tag", tag)
    # origin 配置双 push URL 后，代码和 tag 均会发往 Gitee 与 GitHub。
    run("git", "push", "origin", "HEAD:main", tag)
    body = f"DevEnvManager {ver}：Go 环境管理、GUI / CLI 应用更新、npm 发布修复。"
    if args.provider in ("both", "gitee"):
        release = gitee("GET", "/releases/tags/" + tag)
        if not release:
            release = gitee("POST", "/releases", {"tag_name": tag, "target_commitish": head, "name": tag, "body": body, "prerelease": "-" in ver})
        attachments = {asset["name"] for asset in release.get("assets", [])}
        # 两个平台使用独立目录和独立清单，后发布的一方不会覆盖另一方。
        for file in [*files, manifest]:
            if file.name in attachments:
                raise RuntimeError(f"Gitee 已存在 {file.name}，请核实后再发布，避免覆盖既有版本")
            gitee("POST", f'/releases/{release["id"]}/attach_files', file=file)
    if args.provider in ("both", "github"):
        exists = subprocess.run(["gh", "release", "view", tag, "-R", REPO], capture_output=True)
        if exists.returncode != 0:
            run("gh", "release", "create", tag, "-R", REPO, "--verify-tag", "--title", tag, "--notes", body, *(["--prerelease"] if "-" in ver else []))
        run("gh", "release", "upload", tag, "-R", REPO, *map(str, [*files, manifest]))
    print("发布完成。")


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error))
