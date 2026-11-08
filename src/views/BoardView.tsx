import { useState } from "react";
import { CircuitBoard, ShieldAlert } from "lucide-react";
import { useStore } from "../lib/store";
import { BOARD, CHECKLIST, HOTSPOTS, MIKROBUS_LEFT, MIKROBUS_RIGHT, SAFETY, USED_PINS, type Hotspot } from "../lib/board";
import { Badge, Button, Card, Hint, PageHeader, cx } from "../components/ui";

const KIND: Record<Hotspot["kind"], { label: string; color: string }> = {
  chip: { label: "кристалл", color: "var(--accent)" },
  connector: { label: "разъём", color: "var(--clk)" },
  jumper: { label: "джампер", color: "var(--warn)" },
  power: { label: "питание", color: "var(--pwr)" },
  testpoint: { label: "контр. точка", color: "var(--ref)" },
  led: { label: "индикатор", color: "var(--ok)" },
  switch: { label: "переключатель", color: "var(--ctl)" },
};

/** Schematic top view of the board with clickable hotspots (replace with a traced
 *  drawing or a photo: put the image in public/ and draw <image href="/board.jpg"/>). */
export function BoardView() {
  const go = useStore((s) => s.go);
  const [sel, setSel] = useState<string>(HOTSPOTS[0]?.id ?? "");
  const [done, setDone] = useState<boolean[]>(() => CHECKLIST.map(() => false));
  const h = HOTSPOTS.find((x) => x.id === sel);

  return (
    <div className="max-w-[1600px]">
      <PageHeader icon={<CircuitBoard size={20} />} title={`Плата ${BOARD.name}`} subtitle={`${BOARD.size} · питание: ${BOARD.supply}. Нажмите на элемент платы, чтобы узнать его назначение.`} />
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_400px]">
        <Card bodyClass="!p-3">
          <svg viewBox="0 0 1000 600" className="w-full h-auto">
            <rect x="20" y="20" width="960" height="560" rx="26" fill="color-mix(in srgb, var(--ok) 12%, var(--panel-2))" stroke="var(--line-2)" strokeWidth="2" />
            {[60, 940].map((x) => [60, 540].map((y) => <circle key={`${x}-${y}`} cx={x} cy={y} r="12" fill="var(--panel)" stroke="var(--line-2)" />))}
            {HOTSPOTS.map((p) => {
              const k = KIND[p.kind];
              const on = p.id === sel;
              return (
                <g key={p.id} onClick={() => setSel(p.id)} className="cursor-pointer">
                  <rect x={p.x} y={p.y} width={p.w} height={p.h} rx="8" fill={`color-mix(in srgb, ${k.color} ${on ? 35 : 16}%, var(--panel))`} stroke={k.color} strokeWidth={on ? 3 : 1.5} />
                  {p.id.startsWith("mb-") ? (
                    (p.id === "mb-l" ? MIKROBUS_LEFT : MIKROBUS_RIGHT).map((pin, i) => {
                      const used = USED_PINS.has(pin);
                      const cy = p.y + 38 + i * 50;
                      const left = p.id === "mb-l";
                      return (
                        <g key={pin + i}>
                          <rect x={left ? p.x + 12 : p.x + p.w - 36} y={cy - 12} width="24" height="24" rx="4" fill={used ? "var(--warn)" : "var(--panel-2)"} stroke="var(--line-2)" />
                          <text x={left ? p.x + 44 : p.x + p.w - 44} y={cy + 5} textAnchor={left ? "start" : "end"} fill={used ? "var(--fg)" : "var(--muted)"} fontSize="13" fontWeight={used ? 600 : 400}>
                            {pin === "MISO" ? "MISO·NC" : pin}
                          </text>
                        </g>
                      );
                    })
                  ) : (
                    <text x={p.x + p.w / 2} y={p.y + p.h / 2 + 5} textAnchor="middle" fill="var(--fg)" fontSize="14" fontWeight="600">
                      {p.ref}
                    </text>
                  )}
                </g>
              );
            })}
          </svg>
        </Card>
        <div className="flex flex-col gap-4">
          {h && (
            <Card title={`${h.ref} — ${h.title}`} subtitle={KIND[h.kind].label}>
              <p className="text-[12.5px] text-fg2 leading-relaxed">{h.desc}</p>
              {h.block && (
                <Button size="sm" className="mt-3" onClick={() => go("overview", h.block)}>
                  Показать на блок-схеме
                </Button>
              )}
            </Card>
          )}
          <Card title="Первое включение" subtitle="Отмечайте шаги по мере выполнения">
            <div className="flex flex-col gap-1.5">
              {CHECKLIST.map((c, i) => (
                <label key={i} className="flex items-start gap-2 text-[12.5px] cursor-pointer">
                  <input type="checkbox" className="mt-0.5 accent-[var(--accent)]" checked={done[i]} onChange={() => setDone(done.map((d, j) => (j === i ? !d : d)))} />
                  <span className={cx(done[i] ? "text-muted line-through" : "text-fg2")}>{c}</span>
                </label>
              ))}
            </div>
          </Card>
          <Card title="Безопасность" icon={<ShieldAlert size={15} />}>
            <div className="flex flex-col gap-2">
              {SAFETY.map((s, i) => (
                <Hint key={i} tone="warn">
                  {s}
                </Hint>
              ))}
            </div>
          </Card>
          <div className="flex flex-wrap gap-1.5">
            {Object.entries(KIND).map(([k, v]) => (
              <Badge key={k}>
                <span className="h-2 w-2 rounded-full" style={{ background: v.color }} />
                {v.label}
              </Badge>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
