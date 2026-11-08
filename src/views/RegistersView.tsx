import { useEffect, useMemo, useRef, useState } from "react";
import { BookDown, Binary, Download, FileUp, RotateCcw, Send, Upload } from "lucide-react";
import { useStore } from "../lib/store";
import { hexW } from "../lib/format";
import { openTextFile, saveTextFile } from "../lib/api";
import type { FieldDef } from "../lib/types";
import { FieldControl, bitsLabel } from "../components/fields";
import { Badge, Button, Card, NumField, PageHeader, cx } from "../components/ui";

const PALETTE = ["#6366f1", "#06b6d4", "#f59e0b", "#10b981", "#ec4899", "#8b5cf6", "#ef4444", "#14b8a6", "#eab308", "#3b82f6", "#f97316", "#84cc16"];

function BitGrid({ bits, word, fields, focus, onBit, onField }: { bits: number; word: number; fields: FieldDef[]; focus: string | null; onBit: (bit: number) => void; onField: (f: FieldDef) => void }) {
  const owner = useMemo(() => {
    const o: (FieldDef | null)[] = Array(bits).fill(null);
    fields.forEach((f) => {
      for (let b = f.lsb; b <= f.msb; b++) o[b] = f;
    });
    return o;
  }, [fields, bits]);
  const colorOf = (f: FieldDef) => PALETTE[fields.indexOf(f) % PALETTE.length];
  const order = Array.from({ length: bits }, (_, i) => bits - 1 - i);
  const cols = { gridTemplateColumns: `repeat(${bits}, minmax(0, 1fr))` };

  return (
    <div className="flex flex-col gap-1">
      <div className="grid gap-[2px]" style={cols}>
        {order.map((b) => {
          const f = owner[b];
          const v = (word >>> b) & 1;
          const col = f ? colorOf(f) : undefined;
          return (
            <button
              key={b}
              title={f ? `${b}: ${f.name} — ${f.title}` : `${b}: не используется`}
              onClick={() => (f && f.access !== "rw" ? onField(f) : f ? onBit(b) : undefined)}
              className={cx("h-7 rounded-[5px] text-[11px] font-mono font-semibold transition-all border", f ? "hover:brightness-125" : "cursor-default", f && focus === f.name && "ring-2 ring-accent")}
              style={{
                background: f ? `color-mix(in srgb, ${col} ${v ? 55 : 14}%, transparent)` : "var(--panel-2)",
                borderColor: f ? `color-mix(in srgb, ${col} 50%, transparent)` : "var(--line)",
                color: v ? "var(--fg)" : "var(--muted)",
              }}
            >
              {v}
            </button>
          );
        })}
      </div>
      <div className="grid gap-[2px] text-[9px] text-muted font-mono text-center" style={cols}>
        {order.map((b) => (
          <span key={b}>{b % 4 === 3 || b === 0 ? b : ""}</span>
        ))}
      </div>
      <div className="relative h-5">
        {fields.map((f) => {
          const left = ((bits - 1 - f.msb) / bits) * 100;
          const width = ((f.msb - f.lsb + 1) / bits) * 100;
          return (
            <button
              key={f.name}
              onClick={() => onField(f)}
              className="absolute top-0 h-5 rounded-[4px] text-[9.5px] font-mono truncate px-0.5 hover:brightness-125"
              style={{ left: `${left}%`, width: `calc(${width}% - 2px)`, background: `color-mix(in srgb, ${colorOf(f)} 18%, transparent)`, color: colorOf(f) }}
              title={`${f.name} ${bitsLabel(f)}`}
            >
              {f.name}
            </button>
          );
        })}
      </div>
    </div>
  );
}

export function RegistersView() {
  const snap = useStore((s) => s.snap)!;
  const meta = useStore((s) => s.meta)!;
  const call = useStore((s) => s.call);
  const toast = useStore((s) => s.toast);
  const focusField = useStore((s) => s.focusField);
  const focus = useStore((s) => s.focus);
  const go = useStore((s) => s.go);
  const fieldMap = useStore((s) => s.fieldMap);
  const [reg, setReg] = useState(() => (focusField && fieldMap[focusField] ? fieldMap[focusField].reg : 0));
  const listRef = useRef<HTMLDivElement>(null);
  const bits = meta.chip.data_bits;
  const canRead = snap.link.readback && meta.chip.can_read;

  useEffect(() => {
    if (focusField && fieldMap[focusField]) {
      setReg(fieldMap[focusField].reg);
      setTimeout(() => listRef.current?.querySelector(`[data-field="${focusField}"]`)?.scrollIntoView({ block: "center", behavior: "smooth" }), 60);
    }
  }, [focusField, fieldMap]);

  const fieldsOf = (r: number) => meta.fields.filter((f) => f.reg === r);
  const isNv = (r: number) => meta.nv[r] ?? false;
  const writable = (r: number) => !isNv(r) && fieldsOf(r).some((f) => f.access === "rw");

  const doExport = async () => {
    const r = await call<{ text: string }>("export_config");
    if (!r) return;
    const p = await saveTextFile(`${meta.chip.name}_config.txt`, r.text);
    if (p) toast("ok", `Конфигурация сохранена: ${p}`);
  };
  const doImport = async () => {
    const text = await openTextFile();
    if (text == null) return;
    const r = await call("import_config", { text });
    if (r) toast("ok", "Конфигурация загружена");
  };

  return (
    <div className="max-w-[1600px]">
      <PageHeader
        icon={<Binary size={20} />}
        title="Карта регистров"
        subtitle={`${meta.regs.length} регистров × ${bits} бит · ${meta.chip.bus}. Щелчок по биту инвертирует его, по имени поля — открывает редактор.`}
        actions={
          <>
            <Button icon={<Send size={13} />} onClick={() => call("write_all")} title="Записать всю конфигурацию">
              Записать всё
            </Button>
            <Button icon={<Download size={13} />} onClick={() => call("read_all")} disabled={!canRead} title="Прочитать все регистры">
              Прочитать всё
            </Button>
            <Button icon={<RotateCcw size={13} />} onClick={() => call("reset_defaults", { starter: true })} title="Рабочая стартовая конфигурация">
              Стартовая
            </Button>
            <Button variant="ghost" icon={<BookDown size={13} />} onClick={() => call("reset_defaults", { starter: false })} title="Значения после сброса по даташиту">
              По даташиту
            </Button>
            <Button variant="ghost" icon={<Upload size={13} />} onClick={doExport}>
              Экспорт
            </Button>
            <Button variant="ghost" icon={<FileUp size={13} />} onClick={doImport}>
              Импорт
            </Button>
          </>
        }
      />

      <div className="grid gap-4 2xl:grid-cols-[minmax(0,1fr)_440px] xl:grid-cols-[minmax(0,1fr)_400px]">
        <div className="flex flex-col gap-2 min-w-0">
          {meta.regs.map((rd, r) => {
            const dirty = snap.dirty[r];
            const dev = snap.dev_regs?.[r];
            const rbWord = snap.readback.words[r];
            const match = snap.readback.matches[r];
            const rw = writable(r);
            return (
              <div key={r} onClick={() => setReg(r)} className={cx("card px-4 py-3 cursor-pointer transition-all", r === reg ? "ring-1 ring-accent" : "hover:border-line2")}>
                <div className="flex items-center gap-3 mb-2 flex-wrap">
                  <span className="text-[13px] font-semibold min-w-16">{rd.name}</span>
                  <span className="text-[11px] text-muted font-mono">{hexW(rd.addr, 8)}</span>
                  <div onClick={(e) => e.stopPropagation()}>
                    <NumField hex value={snap.regs[r]} onCommit={(v) => call("set_register", { index: r, value: v >>> 0 })} width="w-20" disabled={!rw} />
                  </div>
                  {isNv(r) ? (
                    <button onClick={(e) => { e.stopPropagation(); go("memory"); }} title="Запись в EEPROM — на экране «EEPROM», с подтверждением">
                      <Badge tone="ref">EEPROM →</Badge>
                    </button>
                  ) : rw ? dirty ? <Badge tone="warn">не записан</Badge> : <Badge tone="ok">записан</Badge> : <Badge>статус</Badge>}
                  {match === true && rbWord != null && <Badge tone="ok">чтение ✓ {hexW(rbWord, bits)}</Badge>}
                  {match === false && rbWord != null && <Badge tone="err">чтение ≠ {hexW(rbWord, bits)}</Badge>}
                  {rw && dev != null && dev !== snap.desired[r] && <span className="text-[11px] text-muted font-mono">в кристалле {hexW(dev, bits)}</span>}
                  <span className="text-[11.5px] text-muted truncate flex-1 min-w-0">{rd.title}</span>
                  <div className="flex gap-1" onClick={(e) => e.stopPropagation()}>
                    <Button size="sm" variant="ghost" onClick={() => call("write_register", { index: r })} disabled={!rw} title={`Записать ${rd.name}`}>
                      W
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => call("read_register", { index: r })} disabled={!canRead} title={`Прочитать ${rd.name}`}>
                      R
                    </Button>
                  </div>
                </div>
                <div onClick={(e) => e.stopPropagation()}>
                  <BitGrid
                    bits={bits}
                    word={snap.regs[r]}
                    fields={fieldsOf(r)}
                    focus={focusField}
                    onBit={(b) => {
                      setReg(r);
                      if (!writable(r)) return;
                      void call("set_register", { index: r, value: (snap.regs[r] ^ (1 << b)) >>> 0 });
                    }}
                    onField={(f) => {
                      setReg(r);
                      focus(f.name);
                    }}
                  />
                </div>
              </div>
            );
          })}
        </div>

        <Card
          title={`Поля ${meta.regs[reg]?.name ?? ""}`}
          subtitle={meta.regs[reg]?.title}
          className="xl:sticky xl:top-0 self-start xl:max-h-[calc(100vh-130px)]"
          bodyClass="overflow-y-auto scroll-thin flex flex-col gap-2 min-h-0 flex-1"
        >
          <div ref={listRef} className="flex flex-col gap-2">
            {fieldsOf(reg).map((f) => (
              <div key={f.name} data-field={f.name} className={cx("rounded-xl", focusField === f.name && "ring-2 ring-accent")}>
                <FieldControl name={f.name} />
              </div>
            ))}
          </div>
        </Card>
      </div>
    </div>
  );
}
