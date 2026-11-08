import { useState } from "react";
import { ArrowRight, GraduationCap } from "lucide-react";
import { useStore } from "../lib/store";
import { LESSONS } from "../lib/lessons";
import { Button, Card, PageHeader, cx } from "../components/ui";

export function LearnView() {
  const go = useStore((s) => s.go);
  const [id, setId] = useState(LESSONS[0]?.id);
  const l = LESSONS.find((x) => x.id === id) ?? LESSONS[0];

  return (
    <div className="max-w-[1200px]">
      <PageHeader icon={<GraduationCap size={20} />} title="Академия" subtitle="Как устроен кристалл — от принципа работы до настройки регистров. Каждый урок можно сразу попробовать в приложении." />
      <div className="grid gap-4 lg:grid-cols-[260px_minmax(0,1fr)]">
        <div className="flex flex-col gap-1">
          {LESSONS.map((x, i) => (
            <button
              key={x.id}
              onClick={() => setId(x.id)}
              className={cx("text-left rounded-lg px-3 py-2 text-[12.5px] transition-colors", x.id === l.id ? "bg-panel ring-1 ring-line text-fg font-medium" : "text-fg2 hover:bg-panel3")}
            >
              <span className="text-muted num mr-2">{i + 1}.</span>
              {x.title}
            </button>
          ))}
        </div>
        {l && (
          <Card title={l.title}>
            <div className="prose-ru selectable">
              {l.body.map((p, i) =>
                p.startsWith("§ ") ? (
                  <h3 key={i}>{p.slice(2)}</h3>
                ) : p.startsWith("$ ") ? (
                  <div key={i} className="formula">
                    {p.slice(2)}
                  </div>
                ) : p.startsWith("- ") ? (
                  <ul key={i}>
                    <li>{p.slice(2)}</li>
                  </ul>
                ) : (
                  <p key={i}>{p}</p>
                )
              )}
            </div>
            {l.try && (
              <Button className="mt-2" variant="primary" icon={<ArrowRight size={13} />} onClick={() => go(l.try!.view, l.try!.block)}>
                {l.try.label}
              </Button>
            )}
          </Card>
        )}
      </div>
    </div>
  );
}
