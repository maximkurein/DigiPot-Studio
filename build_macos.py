#!/usr/bin/env python3
"""Сборка DigiPot Studio для macOS: .app + .dmg (Tauri 2 или нативный egui).

Версия берётся из config/version.json — перед сборкой проверяется, что все
файлы синхронизированы (tools/update_version.py status).

    python3 build_macos.py                 # архитектура этого Mac
    python3 build_macos.py --universal     # Apple Silicon + Intel в одном бинарнике
    python3 build_macos.py --with-server   # плюс консольный digipot-server (веб-режим)
    python3 build_macos.py --open          # открыть папку с результатом
    python3 build_macos.py --egui          # нативный интерфейс egui вместо Tauri (без Node)
    python3 build_macos.py --egui --universal

Результат: release/vX.Y.Z/
    DigiPot-Studio-vX.Y.Z-macos-<arch>.dmg
    DigiPot-Studio-vX.Y.Z-macos-<arch>.app.zip
    [digipot-server-vX.Y.Z-macos-<arch>]
    DigiPot-Studio-egui-vX.Y.Z-macos-<arch>.dmg      (--egui)
    DigiPot-Studio-egui-vX.Y.Z-macos-<arch>.app.zip  (--egui)
    checksum.txt  (SHA-256; строки других сборок той же версии сохраняются)
"""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
VERSION_FILE = ROOT / "config" / "version.json"
PRODUCT = "DigiPot Studio"
SLUG = "DigiPot-Studio"
# egui: отдельное имя и bundle id, чтобы не конфликтовать с Tauri-приложением
# в «Программах» и в LaunchServices. Файл настроек у них общий (см. app.rs).
EGUI_PRODUCT = "DigiPot Studio egui"
EGUI_BIN = "digipot-egui"
EGUI_ID = "com.digipotstudio.egui"
ICNS = ROOT / "src-tauri" / "icons" / "icon.icns"
MIN_MACOS = "11.0"
COPYRIGHT = "© 2026 Kurein M N · MIT"


def info(msg: str) -> None:
    print(f"\033[36m▸\033[0m {msg}", flush=True)


def ok(msg: str) -> None:
    print(f"\033[32m✓\033[0m {msg}", flush=True)


def fail(msg: str) -> "None":
    print(f"\033[31m✗ {msg}\033[0m", file=sys.stderr, flush=True)
    sys.exit(1)


def run(cmd: list[str], **kw) -> None:
    print(f"  $ {' '.join(cmd)}", flush=True)
    r = subprocess.run(cmd, cwd=ROOT, **kw)
    if r.returncode != 0:
        fail(f"команда завершилась с кодом {r.returncode}: {' '.join(cmd)}")


def version() -> str:
    return json.loads(VERSION_FILE.read_text(encoding="utf-8"))["app_info"]["version"]


def need(tool: str, hint: str) -> None:
    if shutil.which(tool) is None:
        fail(f"не найден `{tool}` — {hint}")


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def write_checksums(out: Path, produced: list[Path]) -> None:
    """Обновить checksum.txt: строки этих файлов заменяются, остальные (другая
    сборка той же версии — Tauri или egui) остаются."""
    path = out / "checksum.txt"
    names = {p.name for p in produced}
    keep = []
    if path.exists():
        keep = [l for l in path.read_text(encoding="utf-8").splitlines() if l.strip() and l.split("  ", 1)[-1] not in names]
    lines = keep + [f"{sha256(p)}  {p.name}" for p in produced]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def info_plist(v: str) -> str:
    entries = {
        "CFBundleDevelopmentRegion": "ru",
        "CFBundleDisplayName": EGUI_PRODUCT,
        "CFBundleExecutable": EGUI_BIN,
        "CFBundleIconFile": "icon.icns",
        "CFBundleIdentifier": EGUI_ID,
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleName": EGUI_PRODUCT,
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": v,
        "CFBundleVersion": v,
        "LSApplicationCategoryType": "public.app-category.developer-tools",
        "LSMinimumSystemVersion": MIN_MACOS,
        "NSHumanReadableCopyright": COPYRIGHT,
    }
    body = "".join(f"\t<key>{k}</key>\n\t<string>{val}</string>\n" for k, val in entries.items())
    body += "\t<key>NSHighResolutionCapable</key>\n\t<true/>\n"
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n'
        f'<plist version="1.0">\n<dict>\n{body}</dict>\n</plist>\n'
    )


def build_egui(a: argparse.Namespace, v: str, arch: str, out: Path) -> list[Path]:
    """Нативный интерфейс: cargo build → .app вручную → ad-hoc подпись → .dmg (hdiutil)."""
    if not ICNS.exists():
        fail(f"нет иконки {ICNS.relative_to(ROOT)}")
    info("тесты digipot-egui")
    run(["cargo", "test", "-q", "-p", "digipot-egui"])

    info("сборка digipot-egui (release)")
    cmd = ["cargo", "build", "--release", "-p", "digipot-egui"]
    staging = ROOT / "target" / "egui-bundle"
    shutil.rmtree(staging, ignore_errors=True)
    staging.mkdir(parents=True)
    binary = staging / EGUI_BIN
    if a.universal:
        for t in ("aarch64-apple-darwin", "x86_64-apple-darwin"):
            run(cmd + ["--target", t])
        run(["lipo", "-create", "-output", str(binary),
             str(ROOT / f"target/aarch64-apple-darwin/release/{EGUI_BIN}"),
             str(ROOT / f"target/x86_64-apple-darwin/release/{EGUI_BIN}")])
    else:
        run(cmd)
        shutil.copy2(ROOT / f"target/release/{EGUI_BIN}", binary)

    info(f"пакет {EGUI_PRODUCT}.app")
    app = staging / "dmg" / f"{EGUI_PRODUCT}.app"
    (app / "Contents" / "MacOS").mkdir(parents=True)
    (app / "Contents" / "Resources").mkdir(parents=True)
    shutil.move(str(binary), app / "Contents" / "MacOS" / EGUI_BIN)
    shutil.copy2(ICNS, app / "Contents" / "Resources" / "icon.icns")
    (app / "Contents" / "Info.plist").write_text(info_plist(v), encoding="utf-8")
    (app / "Contents" / "PkgInfo").write_text("APPL????", encoding="ascii")
    run(["plutil", "-lint", str(app / "Contents" / "Info.plist")], stdout=subprocess.DEVNULL)
    # ad-hoc подпись всего пакета: без неё Gatekeeper на Apple Silicon считает его повреждённым
    run(["codesign", "--force", "--deep", "--sign", "-", "--identifier", EGUI_ID, str(app)])
    run(["codesign", "--verify", "--strict", str(app)])

    produced: list[Path] = []
    zip_path = out / f"{SLUG}-egui-v{v}-macos-{arch}.app.zip"
    zip_path.unlink(missing_ok=True)
    run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(app), str(zip_path)])
    produced.append(zip_path)

    info("образ .dmg")
    (staging / "dmg" / "Applications").symlink_to("/Applications")
    dmg = out / f"{SLUG}-egui-v{v}-macos-{arch}.dmg"
    dmg.unlink(missing_ok=True)
    run(["hdiutil", "create", "-volname", f"{EGUI_PRODUCT} {v}", "-srcfolder", str(staging / "dmg"),
         "-ov", "-format", "UDZO", "-quiet", str(dmg)])
    run(["hdiutil", "verify", "-quiet", str(dmg)])
    produced.append(dmg)
    return produced


def main() -> None:
    ap = argparse.ArgumentParser(description="Сборка DigiPot Studio для macOS")
    ap.add_argument("--universal", action="store_true", help="универсальный бинарник arm64 + x86_64")
    ap.add_argument("--with-server", action="store_true", help="дополнительно собрать digipot-server (веб / удалённая лаборатория)")
    ap.add_argument("--skip-version-check", action="store_true", help="не проверять синхронизацию версий")
    ap.add_argument("--skip-npm-install", action="store_true", help="не запускать npm ci, даже если node_modules нет")
    ap.add_argument("--open", action="store_true", help="открыть папку с результатом в Finder")
    ap.add_argument("--egui", action="store_true", help="собрать нативный интерфейс egui вместо Tauri (Node не нужен)")
    a = ap.parse_args()

    if sys.platform != "darwin":
        fail("этот скрипт собирает только под macOS; для Windows используйте build_windows.ps1")

    v = version()
    arch = "universal" if a.universal else ("arm64" if platform.machine() == "arm64" else "x64")
    title = EGUI_PRODUCT if a.egui else PRODUCT
    print(f"\n\033[1m{title} v{v} — сборка macOS ({arch})\033[0m\n")
    t0 = time.time()

    # 1. окружение
    need("cargo", "установите Rust: https://rustup.rs")
    if not a.egui:
        need("node", "установите Node.js 20+ (https://nodejs.org или brew install node)")
        need("npm", "входит в Node.js")
    need("xcrun", "установите Xcode Command Line Tools: xcode-select --install")
    ok("инструменты найдены")

    # 2. версии
    if not a.skip_version_check:
        info("проверка версий (tools/update_version.py status)")
        r = subprocess.run([sys.executable, "tools/update_version.py", "status"], cwd=ROOT, capture_output=True, text=True)
        if r.returncode != 0:
            print(r.stdout)
            fail("версии в файлах расходятся — выполните: python3 tools/update_version.py sync")
        ok(f"версия v{v} синхронизирована")

    # 3. зависимости фронтенда
    if not a.egui and not (ROOT / "node_modules").exists() and not a.skip_npm_install:
        info("установка npm-зависимостей")
        run(["npm", "ci"])

    # 4. цели Rust
    if a.universal:
        info("цели Rust для universal-сборки")
        run(["rustup", "target", "add", "aarch64-apple-darwin", "x86_64-apple-darwin"])

    # 5. тесты модели (быстро, ловят регрессии расчётов)
    info("тесты digipot-core")
    run(["cargo", "test", "-q", "-p", "digipot-core"])

    out = ROOT / "release" / f"v{v}"
    out.mkdir(parents=True, exist_ok=True)
    if a.egui:
        produced = build_egui(a, v, arch, out)
    else:
        produced = build_tauri(a, v, arch, out)

    if a.with_server:
        produced.append(build_server(a, v, arch, out))

    write_checksums(out, produced)

    print()
    ok(f"готово за {time.time() - t0:.0f} с → {out.relative_to(ROOT)}/")
    for p in produced:
        print(f"    {p.name}  ({p.stat().st_size / 1e6:.1f} МБ)")
    print("    checksum.txt")
    app_name = EGUI_PRODUCT if a.egui else PRODUCT
    print("\nПриложение подписано ad-hoc. При первом запуске macOS может предупредить о неизвестном разработчике:")
    print(f"  правый клик → «Открыть», или: xattr -dr com.apple.quarantine \"/Applications/{app_name}.app\"")
    if a.open:
        subprocess.run(["open", str(out)])


def build_tauri(a: argparse.Namespace, v: str, arch: str, out: Path) -> list[Path]:
    triple = "universal-apple-darwin" if a.universal else None
    # 6. сборка Tauri (beforeBuildCommand сам соберёт фронтенд)
    info("сборка приложения (tauri build)")
    cmd = ["npx", "tauri", "build", "--bundles", "app,dmg"]
    if triple:
        cmd += ["--target", triple]
    run(cmd)

    bundle = ROOT / "target" / (triple or "") / "release" / "bundle"
    apps = sorted((bundle / "macos").glob("*.app"))
    dmgs = sorted((bundle / "dmg").glob("*.dmg"))
    if not apps:
        fail(f"не найден .app в {bundle / 'macos'}")

    produced: list[Path] = []
    zip_path = out / f"{SLUG}-v{v}-macos-{arch}.app.zip"
    zip_path.unlink(missing_ok=True)
    run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(apps[0]), str(zip_path)])
    produced.append(zip_path)
    if dmgs:
        dst = out / f"{SLUG}-v{v}-macos-{arch}.dmg"
        shutil.copy2(dmgs[0], dst)
        produced.append(dst)

    return produced


def build_server(a: argparse.Namespace, v: str, arch: str, out: Path) -> Path:
    info("сборка digipot-server")
    scmd = ["cargo", "build", "--release", "-p", "digipot-server"]
    if a.universal:
        for t in ("aarch64-apple-darwin", "x86_64-apple-darwin"):
            run(scmd + ["--target", t])
        dst = out / f"digipot-server-v{v}-macos-universal"
        run(["lipo", "-create", "-output", str(dst),
             str(ROOT / "target/aarch64-apple-darwin/release/digipot-server"),
             str(ROOT / "target/x86_64-apple-darwin/release/digipot-server")])
    else:
        run(scmd)
        dst = out / f"digipot-server-v{v}-macos-{arch}"
        shutil.copy2(ROOT / "target/release/digipot-server", dst)
    return dst


if __name__ == "__main__":
    main()
