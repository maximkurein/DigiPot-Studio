import { useEffect, useRef, useState } from "react";
import { ChevronsDown, ChevronsUp, Minus, Plus, SlidersVertical } from "lucide-react";
import { useStore } from "../lib/store";
import { fmtEng } from "../lib/format";
import type { Curve, Meter } from "../lib/types";
import { Plot } from "../components/Plot";
import { Button, Card, EngField, Hint, NumField, PageHeader, Row, Segmented, Slider, Stat } from "../components/ui";

const FS = 256;

/** Schematic of the pot with clickable TCON switches. */
function PotSchematic() {
  const snap = useStore((s) => s.snap)!;
  const setField = useStore((s) => s.setField);
  const f = snap.fields;
  const s = snap.settings;
  const code = Math.min(f.WIPER0, FS);
  const shdn = f.R0HW === 0;
  const a = f.R0A === 1 && !shdn;
  const b = f.R0B === 1;
  const w = f.R0W === 1;
  const metric = (k: string) => snap.derived.metrics.find((m) => m.key === k)?.value;
  const vw = metric("vw");
  const yTop = 80,
    yBot = 300;
  const yW = yBot - (code / FS) * (yBot - yTop);
  const x = 150;

  const Switch = ({ x1, y1, x2, y2, closed, field, label }: { x1: number; y1: number; x2: number; y2: number; closed: boolean; field: string; label: string }) => {
    const ang = closed ? 0 : -28;
    const len = Math.hypot(x2 - x1, y2 - y1);
    const rot = (Math.atan2(y2 - y1, x2 - x1) * 180) / Math.PI + ang;
    return (
      <g className="cursor-pointer" onClick={() => setField(field, f[field] ? 0 : 1)}>
        <rect x={Math.min(x1, x2) - 14} y={Math.min(y1, y2) - 14} width={Math.abs(x2 - x1) + 28} height={Math.abs(y2 - y1) + 28} fill="transparent" />
        <circle cx={x1} cy={y1} r="3.5" fill="var(--fg2)" />
        <circle cx={x2} cy={y2} r="3.5" fill="var(--fg2)" />
        <line x1={x1} y1={y1} x2={x1 + len * Math.cos((rot * Math.PI) / 180)} y2={y1 + len * Math.sin((rot * Math.PI) / 180)} stroke={closed ? "var(--ok)" : "var(--warn)"} strokeWidth="3" strokeLinecap="round" />
        <text x={Math.max(x1, x2) + 12} y={(y1 + y2) / 2 + 4} fontSize="11" fill="var(--muted)">
          {label}
        </text>
      </g>
    );
  };

  return (
    <svg viewBox="0 0 440 380" className="w-full h-auto select-none">
      {/* terminal A */}
      <circle cx={x} cy={22} r="9" fill="var(--panel)" stroke="var(--sig)" strokeWidth="2" />
      <text x={x - 24} y={27} fontSize="14" fontWeight="600" fill="var(--fg)" textAnchor="end">A</text>
      <text x={x + 18} y={27} fontSize="12" fill="var(--sig)" className="num">{fmtEng(s.va, "V", 3)}</text>
      <line x1={x} y1={31} x2={x} y2={44} stroke="var(--fg2)" strokeWidth="2" />
      <Switch x1={x} y1={44} x2={x} y2={70} closed={a} field="R0A" label="R0A" />
      <line x1={x} y1={70} x2={x} y2={yTop} stroke="var(--fg2)" strokeWidth="2" />
      {/* ladder */}
      <rect x={x - 13} y={yTop} width="26" height={yBot - yTop} rx="4" fill="color-mix(in srgb, var(--sig) 10%, var(--panel))" stroke="var(--sig)" strokeWidth="1.6" />
      {Array.from({ length: 16 }, (_, i) => (
        <line key={i} x1={x - 13} x2={x + 13} y1={yTop + ((i + 1) * (yBot - yTop)) / 17} y2={yTop + ((i + 1) * (yBot - yTop)) / 17} stroke="var(--sig)" strokeOpacity="0.25" />
      ))}
      <text x={x - 24} y={(yTop + yW) / 2 + 4} fontSize="11" fill="var(--muted)" textAnchor="end">R_WA {fmtEng(metric("rwa"), "Ω", 3)}</text>
      <text x={x - 24} y={(yW + yBot) / 2 + 4} fontSize="11" fill="var(--muted)" textAnchor="end">R_WB {fmtEng(metric("rwb"), "Ω", 3)}</text>
      {/* wiper */}
      <path d={`M${x + 16} ${yW} l10 -6 v12 z`} fill="var(--accent)" />
      <line x1={x + 26} y1={yW} x2={250} y2={yW} stroke="var(--accent)" strokeWidth="2" />
      {shdn && <path d={`M${x + 20} ${yW} C ${x + 60} ${yW}, ${x + 60} ${yBot + 22}, ${x} ${yBot + 22}`} fill="none" stroke="var(--warn)" strokeDasharray="5 4" strokeWidth="1.6" />}
      <Switch x1={250} y1={yW} x2={290} y2={yW} closed={w} field="R0W" label="" />
      <text x={270} y={yW - 12} fontSize="11" fill="var(--muted)" textAnchor="middle">R0W</text>
      <line x1={290} y1={yW} x2={322} y2={yW} stroke="var(--fg2)" strokeWidth="2" />
      <circle cx={331} cy={yW} r="9" fill="var(--panel)" stroke="var(--accent)" strokeWidth="2" />
      <text x={331} y={yW - 16} fontSize="14" fontWeight="600" fill="var(--fg)" textAnchor="middle">W</text>
      <text x={346} y={yW + 4} fontSize="12" fill="var(--accent)" className="num">{vw != null && Number.isFinite(vw) ? fmtEng(vw, "V", 4) : "—"}</text>
      {s.load_ohm > 0 && (
        <g>
          <line x1={331} y1={yW + 9} x2={331} y2={yW + 30} stroke="var(--fg2)" strokeWidth="2" />
          <rect x={323} y={yW + 30} width="16" height="34" rx="3" fill="var(--panel)" stroke="var(--fg2)" />
          <text x={346} y={yW + 52} fontSize="11" fill="var(--muted)">R_L {fmtEng(s.load_ohm, "Ω", 3)}</text>
          <line x1={331} y1={yW + 64} x2={331} y2={yW + 76} stroke="var(--fg2)" strokeWidth="2" />
          <path d={`M${319} ${yW + 76} h24 M${323} ${yW + 81} h16 M${327} ${yW + 86} h8`} stroke="var(--fg2)" strokeWidth="1.6" />
        </g>
      )}
      {/* terminal B */}
      <line x1={x} y1={yBot} x2={x} y2={yBot + 10} stroke="var(--fg2)" strokeWidth="2" />
      <Switch x1={x} y1={yBot + 10} x2={x} y2={yBot + 36} closed={b} field="R0B" label="R0B" />
      <line x1={x} y1={yBot + 36} x2={x} y2={yBot + 44} stroke="var(--fg2)" strokeWidth="2" />
      <circle cx={x} cy={yBot + 53} r="9" fill="var(--panel)" stroke="var(--sig)" strokeWidth="2" />
      <text x={x - 24} y={yBot + 58} fontSize="14" fontWeight="600" fill="var(--fg)" textAnchor="end">B</text>
      <text x={x + 18} y={yBot + 58} fontSize="12" fill="var(--sig)" className="num">{fmtEng(s.vb, "V", 3)}</text>
      <text x={8} y={14} fontSize="11" fill="var(--muted)">Ключи кликабельны (TCON)</text>
      {shdn && (
        <text x={250} y={yBot + 58} fontSize="11.5" fill="var(--warn)">Отключение: W → B</text>
      )}
    </svg>
  );
}

export function PotView() {
  const snap = useStore((s) => s.snap)!;
  const meta = useStore((s) => s.meta)!;
  const status = useStore((s) => s.status);
  const call = useStore((s) => s.call);
  const setField = useStore((s) => s.setField);
  const setSettings = useStore((s) => s.setSettings);
  const s = snap.settings;
  const f = snap.fields;
  const code = f.WIPER0;
  const eff = Math.min(code, FS);
  const [draft, setDraft] = useState(eff);
  const [curve, setCurve] = useState<Curve | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const meter: Meter | null = status?.meter ?? snap.meter;
  const isSim = snap.link.kind === "sim";
  const metric = (k: string) => snap.derived.metrics.find((m) => m.key === k);

  useEffect(() => setDraft(eff), [eff]);
  const tconKey = `${f.R0HW}${f.R0A}${f.R0W}${f.R0B}`;
  useEffect(() => {
    void call<{ curve: Curve }>("curve", {}, true).then((r) => r && setCurve(r.curve));
  }, [call, s, tconKey]);

  const slide = (v: number) => {
    setDraft(v);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => void setField("WIPER0", v), 40);
  };
  const step = (up: boolean, n: number) => call("step", { up, n });
  const kOhm = (v: number) => fmtEng(v, "Ω", 3);
  const marker = [{ x: eff, label: `N = ${eff}`, color: "var(--accent)", dash: true }];

  return (
    <div className="max-w-[1600px]">
      <PageHeader
        icon={<SlidersVertical size={20} />}
        title="Потенциометр"
        subtitle="Положение движка, схема включения и передаточная характеристика. Значения считает модель по даташиту; в симуляторе рядом — «мультиметр» на конкретном экземпляре."
      />
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_400px]">
        <div className="flex flex-col gap-4 min-w-0">
          <Card title="Движок" subtitle="WIPER0 · 257 положений (0…256)">
            <div className="flex items-end gap-4 flex-wrap">
              <div>
                <div className="text-[11px] text-muted">Код N</div>
                <div className="text-[34px] font-semibold num leading-none">{draft}</div>
                <div className="text-[11px] text-muted num mt-1">
                  {code.toString(16).toUpperCase().padStart(3, "0")}h · {((eff / FS) * 100).toFixed(1)} %
                </div>
              </div>
              <div className="flex-1 min-w-[260px]">
                <Slider value={draft} min={0} max={FS} onChange={slide} />
                <div className="flex justify-between text-[10.5px] text-muted num">
                  <span>0 · W = B</span>
                  <span>128</span>
                  <span>256 · W = A</span>
                </div>
              </div>
            </div>
            <div className="mt-3 flex flex-wrap items-center gap-2">
              <Button size="sm" icon={<ChevronsDown size={13} />} onClick={() => step(false, 10)} title="10 команд декремента в одном кадре">
                −10
              </Button>
              <Button size="sm" icon={<Minus size={13} />} onClick={() => step(false, 1)} title="Команда декремента (1 байт)">
                −1
              </Button>
              <Button size="sm" icon={<Plus size={13} />} onClick={() => step(true, 1)} title="Команда инкремента (1 байт)">
                +1
              </Button>
              <Button size="sm" icon={<ChevronsUp size={13} />} onClick={() => step(true, 10)} title="10 команд инкремента в одном кадре">
                +10
              </Button>
              <span className="w-3" />
              {[0, 64, 128, 192, 256].map((v) => (
                <Button key={v} size="sm" variant={eff === v ? "primary" : "ghost"} onClick={() => setField("WIPER0", v)}>
                  {v}
                </Button>
              ))}
              <span className="flex-1" />
              <NumField value={code} min={0} max={0x1ff} onCommit={(v) => setField("WIPER0", v)} width="w-20" />
            </div>
            <div className="mt-3 grid gap-2 grid-cols-[repeat(auto-fill,minmax(140px,1fr))]">
              <Stat label="R_WB" value={kOhm(metric("rwb")?.value ?? 0)} tone="sig" />
              <Stat label="R_WA" value={kOhm(metric("rwa")?.value ?? 0)} tone="clk" />
              <Stat label="V_W" value={fmtEng(metric("vw")?.value, "V", 4)} tone="accent" />
              <Stat label="Шаг R_S" value={kOhm(metric("rs")?.value ?? 0)} />
              <Stat label="Шаг по напряжению" value={fmtEng(metric("lsb")?.value, "V", 3)} />
              <Stat label="Ток A→B" value={fmtEng(metric("iab")?.value, "A", 3)} />
            </div>
          </Card>

          <Card title="Сопротивление" subtitle="R_WB и R_WA от кода (номинал R_AB, R_W, температура)">
            {curve ? (
              <Plot
                height={240}
                series={[
                  { name: "R_WB", x: curve.code, y: curve.rwb, color: "var(--sig)" },
                  { name: "R_WA", x: curve.code, y: curve.rwa, color: "var(--clk)" },
                ]}
                vMarkers={marker}
                points={isSim && meter ? [{ x: eff, y: meter.rwb, label: "мультиметр", color: "var(--warn)" }] : undefined}
                xDomain={[0, FS]}
                xLabel="код N"
                yLabel="сопротивление"
                yFmt={(v) => fmtEng(v, "Ω", 3)}
                legend
              />
            ) : (
              <div className="h-[240px]" />
            )}
          </Card>

          <Card title="Напряжение на W" subtitle="Идеальный делитель и с нагрузкой R_L (выход W имеет внутреннее сопротивление)">
            {curve ? (
              <Plot
                height={240}
                series={[
                  { name: "идеально", x: curve.code, y: curve.vw_ideal, color: "var(--muted)", dash: "5 4" },
                  { name: "+1 LSB", x: curve.code, y: curve.inl_hi, color: "var(--line-2)", width: 1 },
                  { name: "−1 LSB", x: curve.code, y: curve.inl_lo, color: "var(--line-2)", width: 1 },
                  { name: "V_W", x: curve.code, y: curve.vw, color: "var(--accent)" },
                ]}
                vMarkers={marker}
                points={isSim && meter && Number.isFinite(meter.vw) ? [{ x: eff, y: meter.vw, label: "мультиметр", color: "var(--warn)" }] : undefined}
                xDomain={[0, FS]}
                xLabel="код N"
                yLabel="V_W"
                yFmt={(v) => fmtEng(v, "V", 3)}
                legend
              />
            ) : (
              <div className="h-[240px]" />
            )}
          </Card>
        </div>

        <div className="flex flex-col gap-4">
          <Card bodyClass="!p-2">
            <PotSchematic />
          </Card>

          <Card title="Схема включения" subtitle="Параметры платы, которых нет в регистрах">
            <Row label="Вариант R_AB" hint="суффикс в маркировке U1">
              <Segmented
                size="sm"
                value={s.rab_nominal}
                onChange={(v) => setSettings({ rab_nominal: v })}
                options={meta.rab_options.map(([suf, ohm]) => ({ value: ohm, label: fmtEng(ohm, "Ω", 3), title: `MCP4161${suf}` }))}
              />
            </Row>
            <Row label="VDD кристалла">
              <Segmented size="sm" value={s.vdd} onChange={(v) => setSettings({ vdd: v })} options={[3.3, 5].map((v) => ({ value: v, label: `${v} В` }))} />
            </Row>
            <Row label="Напряжение на A">
              <EngField value={s.va} unit="V" min={-1} max={7} onCommit={(v) => setSettings({ va: v })} />
            </Row>
            <Row label="Напряжение на B">
              <EngField value={s.vb} unit="V" min={-1} max={7} onCommit={(v) => setSettings({ vb: v })} />
            </Row>
            <Row label="Нагрузка W → GND" hint="0 = без нагрузки">
              <EngField value={s.load_ohm} unit="Ω" min={0} max={1e9} onCommit={(v) => setSettings({ load_ohm: v })} />
            </Row>
            <Row label="R_W (движок)" hint="75 Ом типично">
              <EngField value={s.rw_ohm} unit="Ω" min={0} max={1000} onCommit={(v) => setSettings({ rw_ohm: v })} />
            </Row>
            <Row label="Температура">
              <EngField value={s.temp_c} unit="°C" min={-40} max={125} onCommit={(v) => setSettings({ temp_c: v })} />
            </Row>
          </Card>

          <Card title="Задать по значению" subtitle="Код подбирается моделью и округляется до ближайшего шага">
            <Row label="R_WB (реостат)">
              <EngField value={metric("rwb")?.value ?? 0} unit="Ω" min={0} max={1e6} onCommit={(v) => call("goto", { target: "rwb", value: v })} />
            </Row>
            <Row label="V_W (делитель)">
              <EngField value={metric("vw")?.value ?? 0} unit="V" min={-1} max={7} onCommit={(v) => call("goto", { target: "vw", value: v })} />
            </Row>
          </Card>

          {isSim && meter ? (
            <Card title="Мультиметр (симулятор)" subtitle="Этот экземпляр отличается от номинала — как настоящий кристалл">
              <div className="grid grid-cols-2 gap-2">
                <Stat label="R_AB экземпляра" value={kOhm(meter.rab)} tone="warn" sub={`номинал ${kOhm(s.rab_nominal)}`} />
                <Stat label="R_WB измерено" value={kOhm(meter.rwb)} tone="warn" sub={`модель ${kOhm(metric("rwb")?.value ?? 0)}`} />
                <Stat label="V_W измерено" value={fmtEng(meter.vw, "V", 4)} tone="warn" sub={`модель ${fmtEng(metric("vw")?.value, "V", 4)}`} />
                <Stat label="R_WA измерено" value={kOhm(meter.rwa)} tone="warn" />
              </div>
              <Row label="Отклонение R_AB" hint="допуск по даташиту ±20 %">
                <NumField value={s.sim_rab_dev_pct} min={-20} max={20} unit="%" integer={false} step={0.5} onCommit={(v) => setSettings({ sim_rab_dev_pct: v })} width="w-20" />
              </Row>
              <Hint>В режиме делителя V_W совпадает с моделью при любом отклонении R_AB, а R_WB — нет. Поэтому реостат калибруют, а делитель — нет.</Hint>
            </Card>
          ) : (
            <Hint>Подключена реальная плата: измерьте R_WB или V_W мультиметром и сравните с моделью — так находится R_AB вашего экземпляра.</Hint>
          )}
        </div>
      </div>
    </div>
  );
}
