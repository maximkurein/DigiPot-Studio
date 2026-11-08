import { useEffect, useState, type ReactNode } from "react";
import { Database, Download, Power, Save, ShieldAlert } from "lucide-react";
import { useStore } from "../lib/store";
import { Badge, Button, Card, Hint, NumField, PageHeader, Stat } from "../components/ui";

const ENDURANCE = 1_000_000;
const hex3 = (v: number) => v.toString(16).toUpperCase().padStart(3, "0") + "h";
const EE_CELLS = ["EE06", "EE07", "EE08", "EE09", "EE0A", "EE0B", "EE0C", "EE0D", "EE0E", "EE0F"];

/** Two-step button: the first click arms it, the second performs the EEPROM write. */
function ConfirmButton({ children, confirm, onConfirm, icon, disabled }: { children: ReactNode; confirm: string; onConfirm: () => void; icon?: ReactNode; disabled?: boolean }) {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const t = window.setTimeout(() => setArmed(false), 5000);
    return () => window.clearTimeout(t);
  }, [armed]);
  if (!armed)
    return (
      <Button size="sm" icon={icon} onClick={() => setArmed(true)} disabled={disabled}>
        {children}
      </Button>
    );
  return (
    <span className="inline-flex gap-1">
      <Button
        size="sm"
        variant="danger"
        onClick={() => {
          setArmed(false);
          onConfirm();
        }}
      >
        {confirm}
      </Button>
      <Button size="sm" variant="ghost" onClick={() => setArmed(false)}>
        Отмена
      </Button>
    </span>
  );
}

export function MemoryView() {
  const snap = useStore((s) => s.snap)!;
  const fieldMap = useStore((s) => s.fieldMap);
  const sim = useStore((s) => s.status?.sim ?? s.snap?.sim ?? null);
  const call = useStore((s) => s.call);
  const f = snap.fields;
  const nvReg = fieldMap.NV_WIPER0?.reg ?? 1;
  const wiper = Math.min(f.WIPER0, 0x100);
  const [nvDraft, setNvDraft] = useState(f.NV_WIPER0);
  const [drafts, setDrafts] = useState<Record<string, number>>(() => Object.fromEntries(EE_CELLS.map((n) => [n, f[n] ?? 0xff])));
  const isSim = snap.link.kind === "sim";
  const simWrites = sim?.signals.find((x) => x.name.startsWith("Записей EEPROM"))?.value;

  useEffect(() => setNvDraft(f.NV_WIPER0), [f.NV_WIPER0]);

  const nvWrite = (reg: number, value: number) => call("nv_write", { index: reg, value, confirm: true });
  const readAllNv = async () => {
    for (const n of ["NV_WIPER0", ...EE_CELLS]) {
      const r = fieldMap[n]?.reg;
      if (r != null && !(await call("read_register", { index: r }))) break;
    }
  };

  return (
    <div className="max-w-[1500px]">
      <PageHeader
        icon={<Database size={20} />}
        title="Память EEPROM"
        subtitle="Положение движка при включении и 10 ячеек пользовательских данных. Каждая запись — один цикл из ~1 млн и до 10 мс ожидания, поэтому здесь она только по подтверждению."
        actions={
          <Button icon={<Download size={13} />} onClick={readAllNv} title="Прочитать NV_WIPER0 и ячейки 06h…0Fh">
            Прочитать EEPROM
          </Button>
        }
      />
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_380px]">
        <div className="flex flex-col gap-4 min-w-0">
          <Card title="Движок при включении питания" subtitle="POR копирует NV_WIPER0 (02h) в WIPER0 (00h)">
            <div className="grid gap-2 grid-cols-[repeat(auto-fill,minmax(170px,1fr))]">
              <Stat label="Сейчас (WIPER0)" value={f.WIPER0} sub={hex3(f.WIPER0)} tone="accent" />
              <Stat label="В EEPROM (NV_WIPER0)" value={f.NV_WIPER0} sub={hex3(f.NV_WIPER0)} tone="ref" />
              <Stat
                label="После выключения"
                value={f.NV_WIPER0 === f.WIPER0 ? "то же" : `вернётся к ${f.NV_WIPER0}`}
                tone={f.NV_WIPER0 === f.WIPER0 ? "ok" : "warn"}
              />
            </div>
            <div className="mt-3 flex flex-wrap items-center gap-2">
              <ConfirmButton icon={<Save size={13} />} confirm={`Записать ${wiper} в EEPROM`} onConfirm={() => nvWrite(nvReg, wiper)} disabled={f.NV_WIPER0 === wiper}>
                Сохранить текущее положение
              </ConfirmButton>
              <span className="text-[12px] text-muted">или значение</span>
              <NumField value={nvDraft} min={0} max={0x1ff} onCommit={setNvDraft} width="w-20" />
              <ConfirmButton confirm={`Записать ${nvDraft}`} onConfirm={() => nvWrite(nvReg, nvDraft)}>
                Записать
              </ConfirmButton>
              <span className="flex-1" />
              {isSim ? (
                <Button icon={<Power size={13} />} onClick={() => call("power_cycle")} title="Выключить и включить питание виртуального кристалла">
                  Цикл питания
                </Button>
              ) : (
                <Badge>цикл питания — вручную на плате</Badge>
              )}
            </div>
          </Card>

          <Card title="Ячейки данных 06h…0Fh" subtitle="9 бит каждая, с завода 0FFh. Удобно хранить калибровку R_AB, серийный номер, настройки">
            <div className="grid gap-1.5">
              {EE_CELLS.map((n) => {
                const fd = fieldMap[n];
                if (!fd) return null;
                const v = f[n] ?? 0;
                const rb = snap.readback.words[fd.reg];
                return (
                  <div key={n} className="flex items-center gap-3 rounded-lg border border-line bg-panel2/60 px-3 py-1.5">
                    <span className="font-mono text-[12px] w-12">{n.slice(2)}h</span>
                    <span className="font-mono text-[12.5px] w-16">{hex3(v)}</span>
                    <span className="text-[11px] text-muted w-28 num">{rb != null ? `прочитано ${hex3(rb)}` : "не читалось"}</span>
                    <span className="flex-1" />
                    <NumField hex value={drafts[n] ?? v} min={0} max={0x1ff} onCommit={(x) => setDrafts({ ...drafts, [n]: x })} width="w-20" />
                    <ConfirmButton confirm="Подтвердить" onConfirm={() => nvWrite(fd.reg, drafts[n] ?? v)} disabled={(drafts[n] ?? v) === v}>
                      Записать
                    </ConfirmButton>
                  </div>
                );
              })}
            </div>
          </Card>
        </div>

        <div className="flex flex-col gap-4">
          <Card title="Ресурс EEPROM" subtitle="Endurance 1 млн циклов, tWC 5 мс (макс. 10 мс)">
            <div className="grid grid-cols-2 gap-2">
              <Stat label="Записей за сеанс" value={snap.ee_writes} tone={snap.ee_writes > 0 ? "warn" : undefined} />
              {simWrites != null && <Stat label="Записей в этот кристалл" value={simWrites} sub="симулятор" />}
              <Stat label="Раз в секунду" value="≈ 12 сут" sub="до исчерпания ресурса" />
              <Stat label="Раз в минуту" value="≈ 1.9 года" sub="до исчерпания ресурса" />
            </div>
            <div className="mt-2 text-[11px] text-muted">Ресурс: {ENDURANCE.toLocaleString("ru-RU")} циклов на ячейку.</div>
          </Card>
          <Hint tone="warn">
            Библиотека MikroE для DIGI POT click при каждой установке движка шлёт кадр <span className="kbd">00 nn 20 nn</span>: пишет и WIPER0, и NV_WIPER0. Если двигать движок в цикле, EEPROM изнашивается. Пишите в EEPROM только итоговое значение.
          </Hint>
          <Hint>
            Пока идёт запись (STATUS.EEWA = 1), кристалл принимает команды только к 00h, 01h, 04h, 05h. Любая команда в том же кадре после записи в EEPROM — ошибка (CMDERR). Приложение ждёт 10 мс после каждой записи.
          </Hint>
          <Hint>
            <span className="inline-flex gap-1.5">
              <ShieldAlert size={14} className="text-muted shrink-0 mt-0.5" />
              WiperLock и защита записи (WP) включаются командами с высоким напряжением на CS (V<sub>IHH</sub>). Click такого напряжения не формирует, поэтому в приложении они только отображаются.
            </span>
          </Hint>
        </div>
      </div>
    </div>
  );
}
