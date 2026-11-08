import { CircleAlert, Info, Microchip, TriangleAlert } from "lucide-react";
import { useStore } from "../lib/store";
import { fmtEng } from "../lib/format";
import { BLOCKS, WIRES, blockById, type BlockDef } from "../lib/blocks";
import { FieldControl } from "../components/fields";
import { Card, Hint, PageHeader, Stat, cx } from "../components/ui";

const LEVEL = {
  error: { icon: <CircleAlert size={14} />, cls: "text-err", tone: "err" as const },
  warn: { icon: <TriangleAlert size={14} />, cls: "text-warn", tone: "warn" as const },
  info: { icon: <Info size={14} />, cls: "text-accent", tone: "info" as const },
};

function anchor(b: BlockDef, side: "l" | "r" | "t" | "b") {
  if (side === "l") return [b.x, b.y + b.h / 2];
  if (side === "r") return [b.x + b.w, b.y + b.h / 2];
  if (side === "t") return [b.x + b.w / 2, b.y];
  return [b.x + b.w / 2, b.y + b.h];
}

/** Orthogonal wire between two blocks (left→right, or vertical when stacked). */
function wirePath(a: BlockDef, b: BlockDef) {
  const vertical = Math.abs(a.x + a.w / 2 - (b.x + b.w / 2)) < 40;
  if (vertical) {
    const [x1, y1] = anchor(a, a.y < b.y ? "b" : "t");
    const [, y2] = anchor(b, a.y < b.y ? "t" : "b");
    return `M${x1} ${y1} V${y2}`;
  }
  const [x1, y1] = anchor(a, "r");
  const [x2, y2] = anchor(b, "l");
  if (x2 < x1) {
    const [tx, ty] = anchor(a, b.y < a.y ? "t" : "b");
    const [ux, uy] = anchor(b, b.y < a.y ? "b" : "t");
    const my = (ty + uy) / 2;
    return `M${tx} ${ty} V${my} H${ux} V${uy}`;
  }
  const mx = (x1 + x2) / 2;
  return `M${x1} ${y1} H${mx} V${y2} H${x2}`;
}

function Diagram() {
  const block = useStore((s) => s.block);
  const select = useStore((s) => s.selectBlock);
  const snap = useStore((s) => s.snap)!;
  const meta = useStore((s) => s.meta)!;
  const active = snap.sim ? snap.sim.state !== "off" : true;
  const issueOf = (id: string) => snap.derived.issues.find((i) => i.block === id && i.level !== "info");

  return (
    <svg viewBox="60 118 1120 490" className="w-full h-auto select-none">
      <rect x="80" y="170" width="1080" height="420" rx="28" fill="var(--panel-2)" stroke="var(--line-2)" strokeWidth="2" />
      <text x="104" y="204" fill="var(--muted)" fontSize="15" fontWeight="600">
        {meta.chip.vendor} {meta.chip.name}
      </text>
      {WIRES.map(([from, to, tone]) => {
        const a = blockById(from);
        const b = blockById(to);
        if (!a || !b) return null;
        const d = wirePath(a, b);
        return (
          <g key={`${from}-${to}`}>
            <path d={d} fill="none" stroke={`var(--${tone})`} strokeOpacity="0.35" strokeWidth="5" />
            {active && <path d={d} fill="none" stroke={`var(--${tone})`} strokeWidth="2.5" className="wire-flow" />}
          </g>
        );
      })}
      {BLOCKS.map((b) => {
        const sel = block === b.id;
        const iss = issueOf(b.id);
        const metric = snap.derived.metrics.find((m) => m.block === b.id);
        return (
          <g key={b.id} onClick={() => select(sel ? null : b.id)} className="cursor-pointer">
            <rect
              x={b.x}
              y={b.y}
              width={b.w}
              height={b.h}
              rx="16"
              fill={`color-mix(in srgb, var(--${b.tone}) ${sel ? 22 : 10}%, var(--panel))`}
              stroke={iss ? `var(--${iss.level === "error" ? "err" : "warn"})` : `var(--${b.tone})`}
              strokeWidth={sel ? 3 : 1.6}
            />
            <text x={b.x + b.w / 2} y={b.y + b.h / 2 - (metric ? 6 : -5)} textAnchor="middle" fill="var(--fg)" fontSize="16" fontWeight="600">
              {b.title}
            </text>
            {metric && (
              <text x={b.x + b.w / 2} y={b.y + b.h / 2 + 18} textAnchor="middle" fill={`var(--${b.tone})`} fontSize="13" className="num">
                {fmtEng(metric.value, metric.unit, 4)}
              </text>
            )}
          </g>
        );
      })}
      <text x="80" y="140" fill="var(--muted)" fontSize="13">
        Нажмите на блок, чтобы увидеть его параметры и регистры
      </text>
    </svg>
  );
}

export function OverviewView() {
  const snap = useStore((s) => s.snap)!;
  const meta = useStore((s) => s.meta)!;
  const blockId = useStore((s) => s.block);
  const b = blockById(blockId);
  const d = snap.derived;
  const fields = meta.fields.filter((f) => !b || f.block === b.id);
  const issues = d.issues.filter((i) => !b || i.block === b.id);
  const metrics = d.metrics.filter((m) => !b || m.block === b.id);

  return (
    <div className="max-w-[1600px]">
      <PageHeader icon={<Microchip size={20} />} title="Обзор кристалла" subtitle={`${meta.chip.vendor} ${meta.chip.name} на плате ${meta.chip.board}. Блок-схема живая: значения пересчитываются моделью при каждом изменении регистров.`} />
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_420px]">
        <div className="flex flex-col gap-4 min-w-0">
          <Card bodyClass="!p-2">
            <Diagram />
          </Card>
          <div className="grid gap-2 grid-cols-[repeat(auto-fill,minmax(160px,1fr))]">
            {d.metrics.map((m) => (
              <Stat key={m.key} label={m.label} value={fmtEng(m.value, m.unit)} tone={blockById(m.block)?.tone} />
            ))}
          </div>
        </div>
        <Card
          title={b ? b.title : "Все блоки"}
          subtitle={b ? b.desc : "Выберите блок на схеме"}
          className="xl:sticky xl:top-0 self-start xl:max-h-[calc(100vh-130px)]"
          bodyClass="overflow-y-auto scroll-thin flex flex-col gap-2 min-h-0 flex-1"
        >
          {issues.map((i, k) => (
            <Hint key={k} tone={LEVEL[i.level].tone}>
              <span className={cx("inline-flex items-center gap-1.5", LEVEL[i.level].cls)}>
                {LEVEL[i.level].icon}
                <span className="text-fg2">{i.text}</span>
              </span>
            </Hint>
          ))}
          {b && metrics.length > 0 && (
            <div className="grid grid-cols-2 gap-2">
              {metrics.map((m) => (
                <Stat key={m.key} label={m.label} value={fmtEng(m.value, m.unit)} tone={b.tone} />
              ))}
            </div>
          )}
          {fields.filter((f) => f.access === "rw").map((f) => (
            <FieldControl key={f.name} name={f.name} />
          ))}
          {fields.filter((f) => f.access !== "rw").map((f) => (
            <FieldControl key={f.name} name={f.name} compact />
          ))}
        </Card>
      </div>
    </div>
  );
}
