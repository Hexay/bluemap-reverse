"""Pinned versions and on-disk layout shared by all tools. Everything under work/ is git-ignored."""
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORK = ROOT / "work"
DOWNLOADS = WORK / "downloads"
WORLDS = WORK / "worlds"
BLUEMAP = WORK / "bluemap"
FIXTURES = ROOT / "fixtures"

MC_VERSION = "26.3"
BLUEMAP_VERSION = "5.27"
JAVA_MAJOR = 25

JDK_DIR = DOWNLOADS / f"jdk{JAVA_MAJOR}"
JAVA = JDK_DIR / "bin" / "java.exe"
SERVER_JAR = DOWNLOADS / f"minecraft-server-{MC_VERSION}.jar"
BLUEMAP_JAR = DOWNLOADS / f"bluemap-{BLUEMAP_VERSION}-cli.jar"

JDK_URL = f"https://api.adoptium.net/v3/binary/latest/{JAVA_MAJOR}/ga/windows/x64/jdk/hotspot/normal/eclipse"
MC_MANIFEST_URL = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
BLUEMAP_URL = (
    f"https://github.com/BlueMap-Minecraft/BlueMap/releases/download/"
    f"v{BLUEMAP_VERSION}/bluemap-{BLUEMAP_VERSION}-cli.jar"
)

WEB_HOST = "127.0.0.1"
WEB_PORT = 8100
