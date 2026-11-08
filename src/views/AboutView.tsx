import { BookOpen, Copy, ExternalLink, GitBranch, Globe, Info, Scale, Server, ShieldAlert } from "lucide-react";
import type { ReactNode } from "react";
import { useStore } from "../lib/store";
import { copyText, isTauri, openUrl } from "../lib/api";
import { Button, Card, Hint, PageHeader, Row } from "../components/ui";

/** Repository card: open in the browser or copy the `git clone` URL. */
function Repo({ icon, title, note, url, clone }: { icon: ReactNode; title: string; note: string; url: string; clone: string }) {
  const toast = useStore((s) => s.toast);
  return (
    <div className="rounded-xl border border-line bg-panel2 p-3 flex flex-wrap items-center gap-3">
      <span className="h-9 w-9 shrink-0 rounded-lg bg-panel3 border border-line grid place-items-center text-accent">{icon}</span>
      <div className="min-w-[min(100%,240px)] flex-1">
        <div className="text-[12.5px] font-semibold">{title}</div>
        <div className="text-[11.5px] text-muted break-all num">{url}</div>
        <div className="text-[11px] text-muted mt-0.5">{note}</div>
      </div>
      <div className="flex items-center gap-1.5 ml-auto">
        <Button
          size="sm"
          variant="ghost"
          icon={<Copy size={13} />}
          title="Скопировать команду git clone"
          onClick={async () => {
            const ok = await copyText(`git clone ${clone}`);
            toast(ok ? "ok" : "err", ok ? "Команда git clone скопирована" : "Буфер обмена недоступен");
          }}
        />
        <Button size="sm" icon={<ExternalLink size={13} />} onClick={() => void openUrl(url)}>
          Открыть
        </Button>
      </div>
    </div>
  );
}

export function AboutView() {
  const meta = useStore((s) => s.meta)!;
  const snap = useStore((s) => s.snap)!;
  const go = useStore((s) => s.go);
  const app = meta.app;

  return (
    <div className="max-w-[1100px]">
      <PageHeader icon={<Info size={20} />} title="О программе" subtitle="Версия, разработчик, исходный код и условия использования." />

      <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
        <Card>
          <div className="flex items-center gap-4">
            <img src="/icon.svg" alt="" width={72} height={72} className="rounded-2xl shrink-0" />
            <div className="min-w-0">
              <div className="text-[18px] font-semibold tracking-tight">{app.name}</div>
              <div className="text-[12.5px] text-muted">
                Версия <span className="num font-medium text-fg">{app.version}</span> · от {app.release_date}
              </div>
              <div className="text-[12.5px] mt-1">
                <span className="text-muted">Разработчик</span> <span className="font-semibold">{app.author}</span>
              </div>
            </div>
          </div>

          <div className="mt-4 space-y-1.5">
            <Row label={<span className="flex items-center gap-1.5"><Scale size={13} /> Лицензия</span>}>
              <span className="text-[12.5px]">{app.license} · © {app.release_date.slice(-4)} {app.author}</span>
            </Row>
            <Row label="Кристалл">
              <span className="text-[12.5px]">
                {meta.chip.vendor} {meta.chip.name}
              </span>
            </Row>
            <Row label="Плата">
              <span className="text-[12.5px]">{meta.chip.board}</span>
            </Row>
            <Row label="Шина">
              <span className="text-[12.5px]">{meta.chip.bus}</span>
            </Row>
            <Row label="Режим запуска">
              <span className="text-[12.5px]">{isTauri ? "Приложение (Tauri)" : "Браузер (digipot-server)"}</span>
            </Row>
            <Row label="Подключение">
              <span className="text-[12.5px]">{snap.link.kind === "sim" ? "Симулятор кристалла" : snap.link.name}</span>
            </Row>
          </div>
        </Card>

        <div className="space-y-4">
          <Card title="Исходный код" icon={<GitBranch size={15} />} subtitle="Проект открыт под лицензией MIT">
            <div className="space-y-2.5">
              <Repo
                icon={<Globe size={16} />}
                title="GitHub — основной репозиторий"
                note="Релизы, сборки CI, обсуждения и issue."
                url={app.repository}
                clone={`${app.repository}.git`}
              />
              <Repo
                icon={<Server size={16} />}
                title="Forgejo — зеркало в локальной сети"
                note="Доступно из сети лаборатории (100.64.0.12:3000)."
                url={app.mirror}
                clone={`${app.mirror}.git`}
              />
              <Button
                variant="ghost"
                size="sm"
                icon={<BookOpen size={13} />}
                onClick={() => void openUrl(`${app.repository}#readme`)}
              >
                Документация проекта (README)
              </Button>
            </div>
          </Card>

          <Card title="Условия использования" icon={<ShieldAlert size={15} />}>
            <Hint tone="warn">
              <span className="font-medium">Программа поставляется «как есть».</span> Модель — инженерное приближение по даташиту MCP4161 (DS22059B); для точных применений калибруйте по измерениям. Уровни SPI равны VDD кристалла, напряжения на A, W, B — только 0…VDD, ток — не больше 2.5 мА. Если click стоит в отладочной плате MikroE, её микроконтроллер тоже мастер SPI — остановите его.
            </Hint>
            <div className="mt-2.5 flex flex-wrap gap-1.5">
              <Button size="sm" variant="ghost" onClick={() => go("board")}>
                Правила безопасности платы
              </Button>
              <Button size="sm" variant="ghost" onClick={() => go("learn")}>
                Академия
              </Button>
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}
