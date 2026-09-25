"""Download the pinned JDK, Minecraft server jar and BlueMap CLI into work/downloads. Idempotent."""
import hashlib
import json
import shutil
import sys
import urllib.request
import zipfile
from pathlib import Path

from paths import (
    BLUEMAP_JAR, BLUEMAP_URL, DOWNLOADS, JAVA, JDK_DIR, JDK_URL,
    MC_MANIFEST_URL, MC_VERSION, SERVER_JAR,
)


def open_url(url: str):
    # Adoptium/GitHub answer 403 to urllib's default User-Agent
    return urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "bluemap_reverse-setup"}))


def fetch_json(url: str) -> dict:
    with open_url(url) as r:
        return json.load(r)


def download(url: str, dest: Path, sha1: str | None = None) -> None:
    if dest.exists() and (sha1 is None or file_sha1(dest) == sha1):
        print(f"ok      {dest.name}")
        return
    print(f"fetch   {dest.name} <- {url}")
    tmp = dest.with_suffix(dest.suffix + ".part")
    with open_url(url) as r, open(tmp, "wb") as f:
        shutil.copyfileobj(r, f)
    if sha1 is not None and file_sha1(tmp) != sha1:
        tmp.unlink()
        sys.exit(f"sha1 mismatch for {dest.name}")
    tmp.replace(dest)


def file_sha1(path: Path) -> str:
    h = hashlib.sha1()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def install_jdk() -> None:
    if JAVA.exists():
        print(f"ok      {JDK_DIR.name}")
        return
    archive = DOWNLOADS / "jdk.zip"
    download(JDK_URL, archive)
    staging = DOWNLOADS / "jdk-staging"
    shutil.rmtree(staging, ignore_errors=True)
    with zipfile.ZipFile(archive) as z:
        z.extractall(staging)
    # the zip holds a single versioned top-level folder, e.g. jdk-25.0.4.1+1/
    (top,) = staging.iterdir()
    top.rename(JDK_DIR)
    staging.rmdir()
    archive.unlink()


def download_server_jar() -> None:
    manifest = fetch_json(MC_MANIFEST_URL)
    entry = next(v for v in manifest["versions"] if v["id"] == MC_VERSION)
    server = fetch_json(entry["url"])["downloads"]["server"]
    download(server["url"], SERVER_JAR, server["sha1"])


def main() -> None:
    DOWNLOADS.mkdir(parents=True, exist_ok=True)
    install_jdk()
    download_server_jar()
    download(BLUEMAP_URL, BLUEMAP_JAR)
    print(f"java    {JAVA}")


if __name__ == "__main__":
    main()
