#!/usr/bin/env python3
"""Сборка нативного egui-приложения для macOS: .app + .dmg без сборщика Tauri.

Шаблон из DigiPot Studio (build_macos.py --egui). Положить в корень репозитория,
заменить константы в блоке «настройки проекта». В проекте с уже существующим
build_macos.py лучше встроить build_egui() туда флагом --egui (как в DigiPot).

    python3 build_egui_macos.py                # архитектура этого Mac
    python3 build_egui_macos.py --universal    # arm64 + x86_64 (lipo)
    python3 build_egui_macos.py --open

Результат в release/vX.Y.Z/:
    <SLUG>-egui-vX.Y.Z-macos-<arch>.dmg
    <SLUG>-egui-vX.Y.Z-macos-<arch>.app.zip
    checksum.txt  (строки других сборок той же версии сохраняются)
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

# ── настройки проекта ────────────────────────────────────────────────────────
PRODUCT = "__TITLE__ egui"                     # имя .app и тома .dmg
SLUG = "__SLUG__"                              # префикс файлов в release/
CRATE = "__APP__-egui"                         # пакет cargo
BIN = "__APP__-egui"                           # имя бинарника
BUNDLE_ID = "__BUNDLE_ID__"                    # НЕ как у Tauri-приложения
ICNS = ROOT / "src-tauri" / "icons" / "icon.icns"
MIN_MACOS = "11.0"
COPYRIGHT = "© __YEAR__ __AUTHOR__"
CATEGORY = "public.app-category.developer-tools"
TEST_CRATES = ["__CORE__", CRATE]


def version() -> str:
    """Единый источник версии. Поменяйте под проект (version.json, Cargo.toml …)."""
    vf = ROOT / "config" / "version.json"
    if vf.exists():
        return json.loads(vf.read_text(encoding="utf-8"))["app_info"]["version"]
    import tomllib

    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    return cargo["workspace"]["package"]["version"]
# ─────────────────────────────────────────────────────────────────────────────


def info(msg: str) -> None:
    print(f"\033[36m▸\033[0m {msg}", flush=True)


def ok(msg: str) -> None:
    print(f"\033[32m✓\033[0m {msg}", flush=True)


def fail(msg: str) -> None:
    print(f"\033[31m✗ {msg}\033[0m", file=sys.stderr)
    sys.exit(1)


def run(cmd: list[str], **kw) -> None:
    print(f"  $ {' '.join(cmd)}", flush=True)
    r = subprocess.run(cmd, cwd=ROOT, **kw)
    if r.returncode != 0:
        fail(f"команда завершилась с кодом {r.returncode}: {' '.join(cmd)}")


def sha256(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def write_checksums(out: Path, produced: list[Path]) -> None:
    path = out / "checksum.txt"
    names = {p.name for p in produced}
    keep = []
    if path.exists():
        keep = [l for l in path.read_text(encoding="utf-8").splitlines()
                if l.strip() and l.split("  ", 1)[-1] not in names]
    path.write_text("\n".join(keep + [f"{sha256(p)}  {p.name}" for p in produced]) + "\n", encoding="utf-8")


def info_plist(v: str) -> str:
    entries = {
        "CFBundleDisplayName": PRODUCT,
        "CFBundleExecutable": BIN,
        "CFBundleIconFile": "icon.icns",
        "CFBundleIdentifier": BUNDLE_ID,
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleName": PRODUCT,
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": v,
        "CFBundleVersion": v,
        "LSApplicationCategoryType": CATEGORY,
        "LSMinimumSystemVersion": MIN_MACOS,
        "NSHumanReadableCopyright": COPYRIGHT,
    }
    body = "".join(f"\t<key>{k}</key>\n\t<string>{val}</string>\n" for k, val in entries.items())
    body += "\t<key>NSHighResolutionCapable</key>\n\t<true/>\n"
    return ('<?xml version="1.0" encoding="UTF-8"?>\n'
            '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n'
            f'<plist version="1.0">\n<dict>\n{body}</dict>\n</plist>\n')


def main() -> None:
    ap = argparse.ArgumentParser(description=f"Сборка {PRODUCT} для macOS")
    ap.add_argument("--universal", action="store_true", help="arm64 + x86_64")
    ap.add_argument("--skip-tests", action="store_true")
    ap.add_argument("--open", action="store_true", help="открыть папку с результатом")
    a = ap.parse_args()
    if sys.platform != "darwin":
        fail("только macOS")
    for tool in ("cargo", "codesign", "hdiutil", "ditto", "plutil"):
        if not shutil.which(tool):
            fail(f"не найден {tool}")
    if not ICNS.exists():
        fail(f"нет иконки {ICNS}")

    v = version()
    arch = "universal" if a.universal else ("arm64" if platform.machine() == "arm64" else "x64")
    print(f"\n\033[1m{PRODUCT} v{v} — сборка macOS ({arch})\033[0m\n")
    t0 = time.time()

    if not a.skip_tests:
        info("тесты")
        run(["cargo", "test", "-q"] + [x for c in TEST_CRATES for x in ("-p", c)])

    staging = ROOT / "target" / "egui-bundle"
    shutil.rmtree(staging, ignore_errors=True)
    staging.mkdir(parents=True)
    binary = staging / BIN
    cmd = ["cargo", "build", "--release", "-p", CRATE]
    if a.universal:
        run(["rustup", "target", "add", "aarch64-apple-darwin", "x86_64-apple-darwin"])
        for t in ("aarch64-apple-darwin", "x86_64-apple-darwin"):
            run(cmd + ["--target", t])
        run(["lipo", "-create", "-output", str(binary),
             str(ROOT / f"target/aarch64-apple-darwin/release/{BIN}"),
             str(ROOT / f"target/x86_64-apple-darwin/release/{BIN}")])
    else:
        run(cmd)
        shutil.copy2(ROOT / f"target/release/{BIN}", binary)

    info(f"пакет {PRODUCT}.app")
    app = staging / "dmg" / f"{PRODUCT}.app"
    (app / "Contents" / "MacOS").mkdir(parents=True)
    (app / "Contents" / "Resources").mkdir(parents=True)
    shutil.move(str(binary), app / "Contents" / "MacOS" / BIN)
    shutil.copy2(ICNS, app / "Contents" / "Resources" / "icon.icns")
    (app / "Contents" / "Info.plist").write_text(info_plist(v), encoding="utf-8")
    (app / "Contents" / "PkgInfo").write_text("APPL????", encoding="ascii")
    run(["plutil", "-lint", str(app / "Contents" / "Info.plist")], stdout=subprocess.DEVNULL)
    # ad-hoc подпись: без неё Apple Silicon считает пакет повреждённым
    run(["codesign", "--force", "--deep", "--sign", "-", "--identifier", BUNDLE_ID, str(app)])
    run(["codesign", "--verify", "--strict", str(app)])

    out = ROOT / "release" / f"v{v}"
    out.mkdir(parents=True, exist_ok=True)
    zip_path = out / f"{SLUG}-egui-v{v}-macos-{arch}.app.zip"
    zip_path.unlink(missing_ok=True)
    run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(app), str(zip_path)])

    info("образ .dmg")
    (staging / "dmg" / "Applications").symlink_to("/Applications")
    dmg = out / f"{SLUG}-egui-v{v}-macos-{arch}.dmg"
    dmg.unlink(missing_ok=True)
    run(["hdiutil", "create", "-volname", f"{PRODUCT} {v}", "-srcfolder", str(staging / "dmg"),
         "-ov", "-format", "UDZO", "-quiet", str(dmg)])
    run(["hdiutil", "verify", "-quiet", str(dmg)])

    produced = [zip_path, dmg]
    write_checksums(out, produced)
    print()
    ok(f"готово за {time.time() - t0:.0f} с → {out.relative_to(ROOT)}/")
    for p in produced:
        print(f"    {p.name}  ({p.stat().st_size / 1e6:.1f} МБ)")
    print(f'\nПервый запуск скачанного .dmg: правый клик → «Открыть», или\n'
          f'  xattr -dr com.apple.quarantine "/Applications/{PRODUCT}.app"')
    if a.open:
        subprocess.run(["open", str(out)])


if __name__ == "__main__":
    main()
