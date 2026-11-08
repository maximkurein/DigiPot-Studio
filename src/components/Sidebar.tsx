import { Binary, CircuitBoard, Database, GraduationCap, Info, Microchip, SlidersVertical, Usb } from "lucide-react";
import type { ReactNode } from "react";
import { useSim, useStore } from "../lib/store";
import { fmtEng } from "../lib/format";
import type { ViewId } from "../lib/types";
import { cx } from "./ui";

// Views of the app; keep the order in sync with App.tsx (⌘/Ctrl+1…9).
export const NAV: { id: ViewId; label: string; hint: string; icon: ReactNode; group: string }[] = [
  { id: "pot", label: "Потенциометр", hint: "Движок, схема, кривые", icon: <SlidersVertical size={16} />, group: "Устройство" },
  { id: "overview", label: "Обзор", hint: "Блок-схема кристалла", icon: <Microchip size={16} />, group: "Устройство" },
  { id: "memory", label: "EEPROM", hint: "Память, включение питания", icon: <Database size={16} />, group: "Устройство" },
  { id: "registers", label: "Регистры", hint: "Биты, запись, чтение", icon: <Binary size={16} />, group: "Устройство" },
  { id: "board", label: "Плата", hint: "mikroBUS, выводы, питание", icon: <CircuitBoard size={16} />, group: "Практика" },
  { id: "hardware", label: "Подключение", hint: "USB-мост, симулятор", icon: <Usb size={16} />, group: "Практика" },
  { id: "learn", label: "Академия", hint: "Как работает кристалл", icon: <GraduationCap size={16} />, group: "Практика" },
  { id: "about", label: "О программе", hint: "Версия, автор, исходный код", icon: <Info size={16} />, group: "Программа" },
];

export function Sidebar() {
  const view = useStore((s) => s.view);
  const go = useStore((s) => s.go);
  const snap = useStore((s) => s.snap)!;
  const sim = useSim();
  const app = useStore((s) => s.meta?.app);
  const side = snap.derived.metrics.filter((m) => !m.top).slice(0, 4);
  let group = "";

  return (
    <nav className="w-[212px] shrink-0 border-r border-line bg-bg2/60 flex flex-col">
      <div className="flex-1 overflow-y-auto scroll-thin px-2.5 py-3">
        {NAV.map((n, i) => {
          const header = n.group !== group;
          group = n.group;
          return (
            <div key={n.id}>
              {header && <div className={cx("px-2 pb-1 text-[10.5px] uppercase tracking-wider text-muted font-medium", i > 0 && "pt-3")}>{n.group}</div>}
              <button
                onClick={() => go(n.id)}
                className={cx(
                  "group w-full flex items-center gap-2.5 rounded-lg px-2 py-1.5 text-left transition-colors",
                  view === n.id ? "bg-panel text-fg shadow-sm ring-1 ring-line" : "text-fg2 hover:bg-panel3/70 hover:text-fg"
                )}
              >
                <span className={cx("shrink-0", view === n.id ? "text-accent" : "text-muted group-hover:text-fg2")}>{n.icon}</span>
                <span className="min-w-0">
                  <span className="block text-[12.5px] font-medium leading-tight">{n.label}</span>
                  <span className="block text-[10.5px] text-muted leading-tight truncate">{n.hint}</span>
                </span>
                <span className="ml-auto text-[10px] text-muted opacity-0 group-hover:opacity-100 num">{i + 1}</span>
              </button>
            </div>
          );
        })}
      </div>

      <div className="m-2.5 rounded-xl border border-line bg-panel p-3 text-[11.5px] space-y-1.5">
        {side.map((m) => (
          <div key={m.key} className="flex items-center justify-between gap-2">
            <span className="text-muted truncate">{m.label}</span>
            <span className="num font-medium">{fmtEng(m.value, m.unit, 3)}</span>
          </div>
        ))}
        {sim?.signals.map((s) => (
          <div key={s.name} className="flex items-center justify-between gap-2">
            <span className="text-muted truncate">{s.name}</span>
            <span className="num">{fmtEng(s.value, s.unit, 3)}</span>
          </div>
        ))}
      </div>
      {app && (
        <button
          onClick={() => go("about")}
          className="px-4 pb-3 -mt-1 text-left text-[10.5px] text-muted hover:text-fg2 transition-colors"
          title="О программе: версия, разработчик, исходный код"
        >
          <span className="num">
            {app.name} v{app.version}
          </span>
          <br />
          {app.author}
        </button>
      )}
    </nav>
  );
}
