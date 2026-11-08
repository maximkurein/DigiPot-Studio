import { useEffect, useState } from "react";
import { Cable, Cpu, PlugZap, RefreshCw, ShieldAlert, Usb } from "lucide-react";
import { useStore } from "../lib/store";
import { SAFETY, WIRING } from "../lib/board";
import type { PortInfo, UsbDev } from "../lib/types";
import { Button, Card, Hint, Led, PageHeader, Select, cx } from "../components/ui";

const hex4 = (v: number | null) => (v == null ? "—" : v.toString(16).toUpperCase().padStart(4, "0"));

export function HardwareView() {
  const snap = useStore((s) => s.snap)!;
  const meta = useStore((s) => s.meta)!;
  const call = useStore((s) => s.call);
  const [ports, setPorts] = useState<PortInfo[]>([]);
  const [usb, setUsb] = useState<UsbDev[] | null>(null);
  const [port, setPort] = useState("");
  const [baud, setBaud] = useState(115200);

  const refresh = async () => {
    const p = await call<PortInfo[]>("list_ports", {}, true);
    if (p) {
      setPorts(p);
      if (!port && p.length) setPort((p.find((x) => x.kind === "usb") ?? p[0]).path);
    }
    const u = await call<UsbDev[]>("scan_usb", {}, true);
    setUsb(u ?? []);
  };
  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="max-w-[1400px]">
      <PageHeader icon={<Usb size={20} />} title="Подключение" subtitle={`Симулятор или реальная плата через USB-мост (${meta.chip.bus}). Протокол моста и прошивка — firmware/bridge.`} />
      <div className="grid gap-4 lg:grid-cols-2">
        <Card title="Текущее подключение" icon={<PlugZap size={15} />}>
          <div className="flex items-center gap-3">
            <Led tone={snap.link_error ? "err" : "ok"} />
            <div className="min-w-0">
              <div className="text-[13px] font-semibold">{snap.link.name}</div>
              <div className="text-[11.5px] text-muted truncate">{snap.link_error ?? snap.link.detail}</div>
            </div>
            <div className="flex-1" />
            {snap.link.kind !== "sim" && (
              <Button size="sm" onClick={() => call("connect", { kind: "sim" })}>
                На симулятор
              </Button>
            )}
          </div>
          <Hint>
            <span className="font-medium">Чтение по 3-проводной схеме.</span> MISO на DIGI POT click не разведён: при чтении мост выдаёт 6 бит команды по MOSI, отпускает линию и принимает ответ кристалла по ней же (команда моста <span className="kbd">T</span>, ≤ 100 кГц). Запись работает с любым мастером SPI.
          </Hint>
        </Card>

        <Card title="USB-мост" icon={<Cable size={15} />} actions={<Button size="sm" variant="ghost" icon={<RefreshCw size={13} />} onClick={refresh} title="Обновить список" />}>
          <div className="flex flex-wrap items-center gap-2">
            <Select
              className="min-w-[260px]"
              value={port}
              onChange={setPort}
              options={ports.length ? ports.map((p) => ({ value: p.path, label: `${p.path}${p.product ? ` — ${p.product}` : ""}` })) : [{ value: "", label: "порты не найдены" }]}
            />
            <Select value={baud} onChange={setBaud} options={[9600, 57600, 115200, 230400, 460800, 921600].map((b) => ({ value: b, label: `${b} бод` }))} />
            <Button variant="primary" disabled={!port} onClick={() => call("connect", { kind: "serial", path: port, baud })}>
              Подключить
            </Button>
          </div>
          <Hint>
            Прошейте <span className="kbd">firmware/bridge/bridge.ino</span> в плату с логикой 3.3 В (RP2040, STM32 Nucleo, ESP32, Arduino Zero/Due) — в ней есть команда <span className="kbd">T</span> для чтения по одной линии. При подключении приложение спрашивает <span className="kbd">?</span> и ждёт ответ <span className="kbd">BRIDGE …</span>.
          </Hint>
        </Card>

        <Card title="Схема соединений" icon={<Cpu size={15} />}>
          <table className="w-full text-[12.5px]">
            <thead className="text-muted text-[11px]">
              <tr className="text-left">
                <th className="py-1 font-medium">Мост</th>
                <th className="py-1 font-medium" />
                <th className="py-1 font-medium">Плата</th>
                <th className="py-1 font-medium">Сигнал</th>
                <th className="py-1 font-medium">Примечание</th>
              </tr>
            </thead>
            <tbody>
              {WIRING.map((w) => (
                <tr key={w.signal} className="border-t border-line/60">
                  <td className="py-1 font-mono">{w.mcu}</td>
                  <td className="py-1 text-muted">{w.dir}</td>
                  <td className="py-1 font-mono">{w.board}</td>
                  <td className="py-1 font-mono text-accent">{w.signal}</td>
                  <td className="py-1 text-muted">{w.note ?? ""}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="mt-3 flex flex-col gap-2">
            {SAFETY.map((s, i) => (
              <Hint key={i} tone="warn">
                <span className="inline-flex gap-1.5">
                  <ShieldAlert size={14} className="text-warn shrink-0 mt-0.5" />
                  {s}
                </span>
              </Hint>
            ))}
          </div>
        </Card>

        <Card title="USB-устройства" subtitle="Подсказки по производителю помогают найти интерфейс платы или мост">
          {usb == null ? (
            <div className="text-muted text-[12px]">Сканирование…</div>
          ) : usb.length === 0 ? (
            <div className="text-muted text-[12px]">Устройства не найдены (или нет доступа к USB).</div>
          ) : (
            <div className="flex flex-col gap-1.5 max-h-[340px] overflow-y-auto scroll-thin">
              {usb.map((d) => (
                <div key={`${d.bus}-${d.address}`} className={cx("rounded-lg border px-3 py-1.5", d.likely_bridge ? "border-accent/40 bg-accent/5" : "border-line bg-panel2")}>
                  <div className="flex items-center gap-2 text-[12px]">
                    <span className="font-mono text-muted">
                      {hex4(d.vid)}:{hex4(d.pid)}
                    </span>
                    <span className="font-medium truncate">{d.product ?? "—"}</span>
                    <span className="text-muted truncate">{d.manufacturer ?? ""}</span>
                  </div>
                  {d.hint && <div className="text-[11px] text-muted">{d.hint}</div>}
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>
    </div>
  );
}
