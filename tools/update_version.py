#!/usr/bin/env python3
"""Управление версией приложения.

Единственный источник версии — config/version.json (app_info).
Скрипт синхронизирует её во все файлы, где версия должна совпадать:
Cargo.toml (workspace), Cargo.lock, package.json, package-lock.json,
src-tauri/tauri.conf.json, README.md и шаблон баг-репорта.

Оттуда же берутся сведения о программе (название, разработчик, лицензия,
ссылки на репозитории) для crates/digipot-core/src/about.rs — экран
«О программе» показывает именно их.

Только стандартная библиотека Python — venv не нужен.

    python3 tools/update_version.py status
    python3 tools/update_version.py bump patch|minor|major [--dry-run]
    python3 tools/update_version.py sync [X.Y.Z] [--release-date DD.MM.YYYY] [--last-updated YYYY-MM-DD]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import date, datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "config" / "version.json"
CARGO_TOML = ROOT / "Cargo.toml"
CARGO_LOCK = ROOT / "Cargo.lock"
PACKAGE_JSON = ROOT / "package.json"
PACKAGE_LOCK = ROOT / "package-lock.json"
TAURI_CONF = ROOT / "src-tauri" / "tauri.conf.json"
README = ROOT / "README.md"
BUG_TEMPLATE = ROOT / ".github" / "ISSUE_TEMPLATE" / "bug.yml"
ABOUT_RS = ROOT / "crates" / "digipot-core" / "src" / "about.rs"

def workspace_crates() -> tuple[str, ...]:
    """Package names of the workspace members (their Cargo.lock entries carry the app version)."""
    m = re.search(r"^members\s*=\s*\[(.*?)\]", read(CARGO_TOML), re.M | re.S)
    names = []
    for member in re.findall(r'"([^"]+)"', m.group(1) if m else ""):
        toml = ROOT / member / "Cargo.toml"
        if toml.exists():
            n = re.search(r'^name\s*=\s*"([^"]+)"', read(toml), re.M)
            if n:
                names.append(n.group(1))
    return tuple(names)


def app_name() -> str:
    return load_json(SOURCE)["app_info"].get("name", "App")

SEMVER = re.compile(r"^\d+\.\d+\.\d+$")


# ───────────────────────── helpers ─────────────────────────

def rel(path: Path) -> str:
    return str(path.relative_to(ROOT))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write(path: Path, text: str, dry: bool) -> None:
    if read(path) == text:
        print(f"Unchanged {rel(path)}")
        return
    if dry:
        print(f"[DRY RUN] Would update {rel(path)}")
        return
    # LF везде (см. .gitattributes)
    path.write_text(text, encoding="utf-8", newline="\n")
    print(f"Updated {rel(path)}")


def load_json(path: Path) -> dict:
    return json.loads(read(path))


def dump_json(data: dict) -> str:
    return json.dumps(data, indent=2, ensure_ascii=False) + "\n"


def parse_semver(v: str) -> tuple[int, int, int]:
    if not SEMVER.match(v):
        raise ValueError(f"Некорректная семантическая версия: {v!r} (ожидается X.Y.Z)")
    a, b, c = (int(x) for x in v.split("."))
    return a, b, c


def bump(v: str, part: str) -> str:
    major, minor, patch = parse_semver(v)
    if part == "major":
        return f"{major + 1}.0.0"
    if part == "minor":
        return f"{major}.{minor + 1}.0"
    return f"{major}.{minor}.{patch + 1}"


def replace_once(text: str, pattern: str, repl: str, what: str, flags: int = 0) -> str:
    new, n = re.subn(pattern, repl, text, count=1, flags=flags)
    if n == 0:
        raise RuntimeError(f"Не удалось обновить {what}: строка по шаблону {pattern!r} не найдена")
    return new


# ───────────────────────── source of truth ─────────────────────────

def get_source() -> dict:
    info = load_json(SOURCE)["app_info"]
    parse_semver(info["version"])
    return info


def set_source(meta: dict, dry: bool) -> None:
    data = load_json(SOURCE)
    data["app_info"].update(meta)
    if dry:
        print(f"[DRY RUN] Would update {rel(SOURCE)}")
        return
    SOURCE.write_text(dump_json(data), encoding="utf-8")
    print(f"Updated {rel(SOURCE)}")


# ───────────────────────── tracked files ─────────────────────────

def cargo_get() -> str | None:
    m = re.search(r'\[workspace\.package\][^\[]*?^version\s*=\s*"([^"]+)"', read(CARGO_TOML), re.M | re.S)
    return m.group(1) if m else None


def cargo_set(v: str, dry: bool) -> None:
    t = read(CARGO_TOML)
    t = replace_once(t, r'(\[workspace\.package\][^\[]*?^version\s*=\s*")[^"]+(")', rf"\g<1>{v}\g<2>", "Cargo.toml [workspace.package] version", re.M | re.S)
    write(CARGO_TOML, t, dry)


def lock_get() -> str | None:
    if not CARGO_LOCK.exists():
        return None
    vals = set()
    for c in workspace_crates():
        m = re.search(rf'^name = "{re.escape(c)}"\r?\nversion = "([^"]+)"', read(CARGO_LOCK), re.M)
        if m:
            vals.add(m.group(1))
    if len(vals) == 1:
        return vals.pop()
    return "mixed" if vals else None


def lock_set(v: str, dry: bool) -> None:
    if not CARGO_LOCK.exists():
        print("Skip Cargo.lock (нет файла — будет создан при первой сборке)")
        return
    t = read(CARGO_LOCK)
    for c in workspace_crates():
        if re.search(rf'^name = "{re.escape(c)}"$', t, re.M):
            t = replace_once(t, rf'(^name = "{re.escape(c)}"\r?\nversion = ")[^"]+(")', rf"\g<1>{v}\g<2>", f"Cargo.lock ({c})", re.M)
    write(CARGO_LOCK, t, dry)


def json_get(path: Path, *keys: str) -> str | None:
    if not path.exists():
        return None
    d = load_json(path)
    for k in keys:
        d = d.get(k, {}) if isinstance(d, dict) else {}
    return d if isinstance(d, str) else None


def npm_set(v: str, dry: bool) -> None:
    t = read(PACKAGE_JSON)
    t = replace_once(t, r'("version"\s*:\s*")[^"]+(")', rf"\g<1>{v}\g<2>", "package.json version")
    write(PACKAGE_JSON, t, dry)


def npm_lock_get() -> str | None:
    if not PACKAGE_LOCK.exists():
        return None
    d = load_json(PACKAGE_LOCK)
    a = d.get("version")
    b = d.get("packages", {}).get("", {}).get("version")
    return a if a == b else "mixed"


def npm_lock_set(v: str, dry: bool) -> None:
    if not PACKAGE_LOCK.exists():
        print("Skip package-lock.json (нет файла — появится после npm install)")
        return
    d = load_json(PACKAGE_LOCK)
    d["version"] = v
    d.setdefault("packages", {}).setdefault("", {})["version"] = v
    write(PACKAGE_LOCK, dump_json(d), dry)


def tauri_set(v: str, dry: bool) -> None:
    t = read(TAURI_CONF)
    t = replace_once(t, r'("version"\s*:\s*")[^"]+(")', rf"\g<1>{v}\g<2>", "tauri.conf.json version")
    write(TAURI_CONF, t, dry)


def readme_get() -> str | None:
    if not README.exists():
        return None
    m = re.search(rf"^# {re.escape(app_name())} v(\d+\.\d+\.\d+)", read(README), re.M)
    return m.group(1) if m else None


def readme_set(v: str, dry: bool) -> None:
    t = read(README)
    t = replace_once(t, rf"^# {re.escape(app_name())} v\d+\.\d+\.\d+", f"# {app_name()} v{v}", "заголовок README.md", re.M)
    write(README, t, dry)


def bug_get() -> str | None:
    if not BUG_TEMPLATE.exists():
        return None
    m = re.search(r'id: version.*?placeholder: "([^"]+)"', read(BUG_TEMPLATE), re.S)
    return m.group(1) if m else None


def bug_set(v: str, dry: bool) -> None:
    t = read(BUG_TEMPLATE)
    t = replace_once(t, r'(id: version.*?placeholder: ")[^"]+(")', rf"\g<1>{v}\g<2>", "placeholder версии в bug.yml", re.S)
    write(BUG_TEMPLATE, t, dry)


# about.rs: константы экрана «О программе» (имя, версия, дата, разработчик, ссылки)
ABOUT_CONSTS = (
    ("NAME", "name"),
    ("RELEASE_DATE", "release_date"),
    ("AUTHOR", "author"),
    ("LICENSE", "license"),
    ("REPOSITORY", "repository"),
    ("MIRROR", "mirror"),
)


def about_const(name: str) -> str | None:
    m = re.search(rf'pub const {name}: &str = "([^"]*)";', read(ABOUT_RS))
    return m.group(1) if m else None


def about_get() -> str | None:
    return about_const("VERSION") if ABOUT_RS.exists() else None


def about_set(v: str, dry: bool) -> None:
    # sync() уже записал источник, поэтому даты и имена берём из него.
    info = load_json(SOURCE)["app_info"]
    t = read(ABOUT_RS)
    values = [("VERSION", v)] + [(c, info[k]) for c, k in ABOUT_CONSTS if info.get(k)]
    for const, value in values:
        if '"' in value or "\\" in value:
            raise RuntimeError(f"about.rs {const}: кавычки и обратные слеши недопустимы ({value!r})")
        t = replace_once(t, rf'(pub const {const}: &str = ")[^"]*(";)', lambda m, val=value: m.group(1) + val + m.group(2), f"about.rs {const}")
    # Пример в doc-комментарии title() тоже держим актуальным.
    t = re.sub(r"/// «[^»]*» — для журналов", f"/// «{info['name']} v{v} · {info.get('author', '')}» — для журналов", t, count=1)
    write(ABOUT_RS, t, dry)


TRACKED = [
    # label, file, getter, setter
    ("cargo", CARGO_TOML, cargo_get, cargo_set),
    ("cargo_lock", CARGO_LOCK, lock_get, lock_set),
    ("npm", PACKAGE_JSON, lambda: json_get(PACKAGE_JSON, "version"), npm_set),
    ("npm_lock", PACKAGE_LOCK, npm_lock_get, npm_lock_set),
    ("tauri", TAURI_CONF, lambda: json_get(TAURI_CONF, "version"), tauri_set),
    ("readme", README, readme_get, readme_set),
    ("bug_form", BUG_TEMPLATE, bug_get, bug_set),
    ("about_rs", ABOUT_RS, about_get, about_set),
]


# ───────────────────────── commands ─────────────────────────

def status() -> int:
    src = get_source()
    v = src["version"]
    print(f"Source  {rel(SOURCE)}: v{v}  (release {src.get('release_date', '?')}, updated {src.get('last_updated', '?')})")
    print(f"        {src.get('name', '?')} · разработчик {src.get('author', '?')} · {src.get('repository', '?')}\n")
    bad = 0
    for label, path, getter, _ in TRACKED:
        if not path.exists():
            print(f"  {label:<11} MISSING  {rel(path)}")
            bad += 1
            continue
        got = getter()
        state = "OK" if got == v else "DIFF"
        if state != "OK":
            bad += 1
        print(f"  {label:<11} {state:<8} {rel(path):<36} {got}")
    print()
    if bad:
        print(f"{bad} file(s) out of sync — run: python3 tools/update_version.py sync")
        return 1
    print("All tracked files match.")
    return 0


def sync(meta: dict, dry: bool) -> None:
    v = meta["version"]
    print(f"Target version:      {v}")
    print(f"Target release date: {meta['release_date']}")
    print(f"Target last updated: {meta['last_updated']}\n")
    set_source(meta, dry)
    for label, path, _, setter in TRACKED:
        if not path.exists() and label not in ("cargo_lock", "npm_lock"):
            raise RuntimeError(f"Нет файла {rel(path)} ({label})")
        setter(v, dry)
    print("\nНе забудьте раздел в CHANGELOG.md: ## [%s] - %s" % (v, meta["last_updated"]))


def check_date(value: str, fmt: str, flag: str) -> str:
    try:
        datetime.strptime(value, fmt)
    except ValueError as e:
        raise SystemExit(f"{flag}: неверная дата {value!r}: {e}")
    return value


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description="Управление версией (источник: config/version.json)")
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("status", help="показать состояние всех файлов с версией")
    for name, hlp in (("sync", "синхронизировать файлы с источником (или задать X.Y.Z)"), ("bump", "увеличить версию patch|minor|major и синхронизировать")):
        sp = sub.add_parser(name, help=hlp)
        if name == "sync":
            sp.add_argument("version", nargs="?", help="новая версия X.Y.Z (по умолчанию — текущая из источника)")
        else:
            sp.add_argument("part", choices=["patch", "minor", "major"])
        sp.add_argument("--release-date", help="дата релиза DD.MM.YYYY (по умолчанию — сегодня)")
        sp.add_argument("--last-updated", help="дата обновления YYYY-MM-DD (по умолчанию — сегодня)")
        sp.add_argument("--dry-run", action="store_true", help="показать изменения, ничего не записывая")
    a = p.parse_args(argv)

    try:
        if a.cmd == "status":
            return status()
        src = get_source()
        today = date.today()
        if a.cmd == "bump":
            version = bump(src["version"], a.part)
            fresh = True
        elif a.version:
            parse_semver(a.version)
            version = a.version
            fresh = True
        else:
            version = src["version"]
            fresh = False
        meta = {
            "version": version,
            "release_date": a.release_date or (today.strftime("%d.%m.%Y") if fresh else src.get("release_date", today.strftime("%d.%m.%Y"))),
            "last_updated": a.last_updated or (today.isoformat() if fresh else src.get("last_updated", today.isoformat())),
        }
        check_date(meta["release_date"], "%d.%m.%Y", "--release-date")
        check_date(meta["last_updated"], "%Y-%m-%d", "--last-updated")
        sync(meta, a.dry_run)
        return 0
    except (ValueError, RuntimeError, KeyError, FileNotFoundError) as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
