#!/usr/bin/env python3
"""Inventory of a Tauri project before porting its UI to egui.

    python3 inventory.py <repo>            # Markdown report to stdout

Lists: workspace crates, rustc and tauri/eframe versions, #[tauri::command]s,
the single RPC dispatcher (if any), frontend calls to the backend, views/pages,
Tauri plugins, frontend data files to mirror, math that lives in TS (should
move to Rust first), CSS variables (palette for theme.rs) and existing egui crates.
Read-only; standard library only.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

SKIP = {"node_modules", "target", "dist", ".git", ".claude", ".agents", ".cursor", "release", "build", ".next", ".svelte-kit"}
FRONT_EXT = {".ts", ".tsx", ".js", ".jsx", ".vue", ".svelte"}


def walk(root: Path, exts: set[str]):
    for p in root.rglob("*"):
        if p.is_file() and p.suffix in exts and not (set(p.relative_to(root).parts) & SKIP):
            yield p


def rel(p: Path, root: Path) -> str:
    return str(p.relative_to(root))


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    root = Path(sys.argv[1]).expanduser().resolve()
    out: list[str] = [f"# Port inventory: {root.name}\n"]

    # toolchain
    try:
        rustc = subprocess.run(["rustc", "-V"], capture_output=True, text=True).stdout.strip()
    except FileNotFoundError:
        rustc = "rustc not found"
    out.append(f"- Local toolchain: `{rustc}` (pick the eframe line whose MSRV fits)")

    # crates
    crates = []
    for ct in root.rglob("Cargo.toml"):
        if set(ct.relative_to(root).parts) & SKIP:
            continue
        txt = ct.read_text(encoding="utf-8", errors="ignore")
        m = re.search(r'^\s*name\s*=\s*"([^"]+)"', txt, re.M)
        deps = [d for d in ("tauri", "eframe", "egui", "axum", "serde_json") if re.search(rf'^\s*{d}\s*=', txt, re.M)]
        if m:
            crates.append((m.group(1), rel(ct.parent, root), deps))
    out.append("\n## Rust crates\n")
    for name, path, deps in crates:
        out.append(f"- `{name}` — `{path}/` deps: {', '.join(deps) or '—'}")
    if any("eframe" in d or "egui" in d for _, _, d in crates):
        out.append("\n> An egui/eframe crate already exists — extend it instead of creating a new one.")

    rs_files = list(walk(root, {".rs"}))

    # tauri commands
    cmds = []
    for f in rs_files:
        txt = f.read_text(encoding="utf-8", errors="ignore")
        for m in re.finditer(r"#\[tauri::command[^\]]*\]\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)", txt):
            cmds.append((m.group(1), rel(f, root)))
    out.append(f"\n## `#[tauri::command]` ({len(cmds)})\n")
    for n, f in cmds:
        out.append(f"- `{n}` — {f}")

    # single dispatcher
    disp = []
    for f in rs_files:
        txt = f.read_text(encoding="utf-8", errors="ignore")
        if re.search(r"fn\s+handle\s*\(\s*&mut\s+self\s*,\s*\w+\s*:\s*&str", txt):
            arms = sorted(set(re.findall(r'^\s*"([a-z_][a-z0-9_]*)"\s*(?:\|[^=]*)?=>', txt, re.M)))
            disp.append((rel(f, root), arms))
    out.append("\n## RPC dispatcher (`fn handle(&mut self, cmd: &str, …)`)\n")
    if disp:
        for f, arms in disp:
            out.append(f"- {f}: {len(arms)} commands — {', '.join(f'`{a}`' for a in arms)}")
    else:
        out.append("- none: the worker will call the command functions directly, or fold them into one `handle` first")

    # frontend
    front = [p for p in walk(root, FRONT_EXT) if "src-tauri" not in p.parts]
    calls: Counter[str] = Counter()
    for f in front:
        txt = f.read_text(encoding="utf-8", errors="ignore")
        for m in re.finditer(r"\b(?:invoke|rpc|call)\s*(?:<[^>]*>)?\(\s*['\"`]([\w:.-]+)['\"`]", txt):
            calls[m.group(1)] += 1
    out.append(f"\n## Frontend → backend calls ({len(calls)} distinct)\n")
    for c, n in calls.most_common():
        out.append(f"- `{c}` ×{n}")

    views = [p for p in front if re.search(r"(views?|pages?|routes?|screens?)$", p.parent.name, re.I)]
    out.append(f"\n## Views / pages ({len(views)}) — one egui `views/*.rs` each\n")
    for v in sorted(views):
        lines = sum(1 for _ in v.open(encoding="utf-8", errors="ignore"))
        out.append(f"- {rel(v, root)} ({lines} lines)")

    comps = [p for p in front if p.parent.name.lower() == "components"]
    if comps:
        out.append(f"\n## Shared components ({len(comps)}) — candidates for `widgets.rs`\n")
        for c in sorted(comps):
            out.append(f"- {rel(c, root)}")

    data = [p for p in front if p.parent.name == "lib" and re.search(r"(blocks|board|lessons|content|texts?|i18n|data)", p.stem, re.I)]
    out.append("\n## Static data/texts to mirror in `content.rs`\n")
    out += [f"- {rel(d, root)}" for d in sorted(data)] or ["- none found (check src/lib manually)"]

    # math in TS
    math = []
    for f in front:
        txt = f.read_text(encoding="utf-8", errors="ignore")
        n = len(re.findall(r"Math\.(?:log|log10|exp|pow|sqrt|sin|cos|atan2?)\b", txt))
        if n >= 3:
            math.append((rel(f, root), n))
    out.append("\n## Math in the frontend (move to Rust before porting if it is model logic)\n")
    out += [f"- {f}: {n} Math.* calls" for f, n in sorted(math, key=lambda x: -x[1])] or ["- none significant"]

    # plugins / conf
    conf = next(iter(root.glob("src-tauri/tauri.conf.json")), None)
    out.append("\n## Tauri config\n")
    if conf:
        c = json.loads(conf.read_text(encoding="utf-8"))
        out.append(f"- identifier: `{c.get('identifier')}` (config dir for the shared settings file; give egui a different bundle id)")
        out.append(f"- productName: `{c.get('productName')}`, version: `{c.get('version')}`")
        icons = c.get("bundle", {}).get("icon", [])
        out.append(f"- icons: {', '.join(icons) or '—'}")
    plugins = sorted({m for f in rs_files for m in re.findall(r"tauri_plugin_(\w+)", f.read_text(encoding='utf-8', errors='ignore'))})
    out.append(f"- plugins: {', '.join(plugins) or 'none'} → replace with std/rfd equivalents")

    # css variables
    css_vars: dict[str, str] = {}
    for f in walk(root, {".css"}):
        for m in re.finditer(r"(--[\w-]+)\s*:\s*(#[0-9a-fA-F]{3,8})", f.read_text(encoding="utf-8", errors="ignore")):
            css_vars.setdefault(m.group(1), m.group(2))
    out.append(f"\n## CSS colour variables ({len(css_vars)}) → `Pal` in theme.rs (check the dark-theme block too)\n")
    for k, v in list(css_vars.items())[:60]:
        out.append(f"- `{k}`: `{v}`")

    print("\n".join(out))


if __name__ == "__main__":
    main()
