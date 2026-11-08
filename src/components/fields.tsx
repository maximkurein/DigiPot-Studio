import type { ReactNode } from "react";
import { useStore, useField } from "../lib/store";
import { signed } from "../lib/format";
import { Badge, NumField, Segmented, Select, Toggle, cx } from "./ui";

export function bitsLabel(f: { reg: number; msb: number; lsb: number }, regName?: string) {
  return `${regName ?? `R${f.reg}`}[${f.msb === f.lsb ? `${f.msb}` : `${f.msb}:${f.lsb}`}]`;
}

/** Auto-generated editor for any register field (from the Rust metadata). */
export function FieldControl({ name, compact, hideDesc }: { name: string; compact?: boolean; hideDesc?: boolean }) {
  const f = useStore((s) => s.fieldMap[name]);
  const reg = useStore((s) => (f ? s.meta?.regs[f.reg] : undefined));
  const raw = useField(name);
  const setField = useStore((s) => s.setField);
  const nv = useStore((s) => (f ? s.meta?.nv?.[f.reg] ?? false : false));
  const focus = useStore((s) => s.focus);
  const go = useStore((s) => s.go);
  if (!f) return null;
  const width = f.msb - f.lsb + 1;
  const max = width >= 32 ? 0xffffffff : 2 ** width - 1;
  const ro = f.access !== "rw";
  const v = f.signed ? signed(raw, width) : raw;
  const optLabel = f.options.find((o) => o[0] === raw)?.[1];

  let control: ReactNode;
  if (nv && !ro) {
    // EEPROM: written only from the Memory view, with confirmation
    control = (
      <button className="inline-flex" onClick={() => go("memory")} title="Запись в EEPROM — на экране «EEPROM»">
        <Badge tone="ref">
          {v}
          {f.unit && ` ${f.unit}`} · EEPROM →
        </Badge>
      </button>
    );
  } else if (ro) {
    control = (
      <Badge tone={f.access === "sticky" ? "warn" : "muted"}>
        {optLabel ?? v}
        {f.unit && ` ${f.unit}`} · {f.access === "ro" ? "только чтение" : f.access === "sticky" ? "sticky" : "фиксировано"}
      </Badge>
    );
  } else if (f.options.length > 0 && f.options.length <= 4 && f.options.every((o) => o[1].length < 26)) {
    control = <Segmented size="sm" options={f.options.map(([val, label]) => ({ value: val, label }))} value={raw} onChange={(x) => setField(name, x)} />;
  } else if (f.options.length > 0) {
    control = <Select options={f.options.map(([val, label]) => ({ value: val, label: `${label}  (${val})` }))} value={raw} onChange={(x) => setField(name, x)} />;
  } else if (width === 1) {
    control = <Toggle checked={raw === 1} onChange={(x) => setField(name, x ? 1 : 0)} />;
  } else if (f.signed) {
    const lo = -(2 ** (width - 1));
    control = (
      <NumField value={v} min={lo} max={2 ** (width - 1) - 1} unit={f.unit} onCommit={(x) => setField(name, x < 0 ? x + 2 ** width : x)} width="w-20" />
    );
  } else {
    control = <NumField value={v} min={0} max={max} unit={f.unit} onCommit={(x) => setField(name, x)} width={width > 10 ? "w-20" : "w-14"} />;
  }

  return (
    <div className={cx("rounded-xl border border-line bg-panel2/60 px-3 py-2", compact && "py-1.5")}>
      <div className="flex items-center gap-2 flex-wrap">
        <div className="min-w-[150px] flex-1">
          <div className="text-[12.5px] font-medium text-fg leading-snug">{f.title}</div>
          <button
            className="font-mono text-[10.5px] text-muted hover:text-accent"
            title="Открыть в карте регистров"
            onClick={() => {
              focus(name);
              go("registers");
            }}
          >
            {f.name} · {bitsLabel(f, reg?.name)}
          </button>
        </div>
        <div className="shrink-0 max-w-full">{control}</div>
      </div>
      {!compact && !hideDesc && f.desc && <p className="text-[11.5px] text-muted leading-snug mt-1">{f.desc}</p>}
    </div>
  );
}
