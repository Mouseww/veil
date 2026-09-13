import { useEffect, useMemo, useState } from "react";
import { api, type TrafficEvent } from "../api";
import { useT } from "../i18n";

type UpdateInfo = { current: string; latest: string; newer: boolean };

const WINDOW_MIN = 20;

function buckets(events: TrafficEvent[], now: number) {
  const arr = Array.from({ length: WINDOW_MIN }, () => 0);
  for (const e of events) {
    const i = Math.floor((now - e.ts) / 60000);
    if (i >= 0 && i < WINDOW_MIN) arr[WINDOW_MIN - 1 - i] += 1;
  }
  return arr;
}

function hitMix(events: TrafficEvent[]) {
  const map: Record<string, number> = {};
  for (const e of events) {
    e.hit_types.forEach((ty, i) => {
      map[ty] = (map[ty] ?? 0) + (e.hit_counts[i] ?? 1);
    });
  }
  return Object.entries(map).sort((a, b) => b[1] - a[1]);
}

function BarChart({ values, labels }: { values: number[]; labels?: string[] }) {
  const max = Math.max(1, ...values);
  const w = 560;
  const h = 120;
  const gap = 3;
  const bw = (w - gap * (values.length + 1)) / values.length;
  return (
    <svg viewBox={"0 0 " + w + " " + h} className="chart" role="img">
      {values.map((v, i) => {
        const bh = (v / max) * (h - 18);
        const x = gap + i * (bw + gap);
        return (
          <g key={i}>
            <rect x={x} y={h - 16 - bh} width={bw} height={bh} rx="2" fill="currentColor" opacity={v ? 0.9 : 0.15} />
            {labels && i % 4 === 0 && (
              <text x={x + bw / 2} y={h - 4} textAnchor="middle" fontSize="9" fill="currentColor" opacity="0.5">
                {labels[i]}
              </text>
            )}
          </g>
        );
      })}
    </svg>
  );
}

function HBars({ rows }: { rows: [string, number][] }) {
  const max = Math.max(1, ...rows.map((r) => r[1]));
  if (!rows.length) return <p className="muted">—</p>;
  return (
    <div className="hbars">
      {rows.map(([name, n]) => (
        <div className="hbar" key={name}>
          <span className="hbar-l">{name}</span>
          <span className="hbar-t">
            <span style={{ width: ((n / max) * 100) + "%" }} />
          </span>
          <span className="hbar-n">{n}</span>
        </div>
      ))}
    </div>
  );
}

export default function DashboardPage() {
  const { t } = useT();
  const [events, setEvents] = useState<TrafficEvent[]>([]);
  const [err, setErr] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now());
  const [ver, setVer] = useState("");
  const [upd, setUpd] = useState<UpdateInfo | null>(null);
  const [updMsg, setUpdMsg] = useState("");
  useEffect(() => {
    let on = true;
    const tick = () => {
      api.traffic().then((e) => { if (on) { setEvents(e); setNow(Date.now()); } }).catch((e) => { if (on) setErr(String(e)); });
    };
    tick();
    const id = setInterval(tick, 4000);
    api.status().then((s) => { if (on) setVer(s.version ?? ""); }).catch(() => undefined);
    return () => { on = false; clearInterval(id); };
  }, []);
  const stats = useMemo(() => {
    const n = events.length;
    const hits = events.filter((e) => e.hit_types.length > 0).length;
    const errors = events.filter((e) => e.status >= 400 || e.error_class).length;
    const avg = n ? Math.round(events.reduce((s, e) => s + e.latency_ms, 0) / n) : 0;
    const ok = events.filter((e) => e.status >= 200 && e.status < 300).length;
    const sse = events.filter((e) => e.streaming).length;
    return { n, hits, errors, avg, ok, sse };
  }, [events]);
  const series = useMemo(() => buckets(events, now), [events, now]);
  const mix = useMemo(() => hitMix(events), [events]);
  const labels = series.map((_, i) => {
    const m = WINDOW_MIN - 1 - i;
    return m === 0 ? "now" : "-" + m;
  });
  if (err) return <p className="err">{t.statusErr}{err}</p>;
  return (
    <section>
      <h1>{t.dashTitle}</h1>
      <p className="help">{t.helpDash}</p>
      <div className="cards">
        <div className="card"><div className="label">{t.statReqs}</div><div className="value">{stats.n}</div></div>
        <div className="card"><div className="label">{t.statHits}</div><div className="value">{stats.hits}</div></div>
        <div className="card"><div className="label">{t.statErrs}</div><div className="value">{stats.errors}</div></div>
        <div className="card"><div className="label">{t.statAvg}</div><div className="value">{stats.avg}<span className="unit">ms</span></div></div>
      </div>
      {events.length === 0 ? (
        <p className="muted">{t.emptyDash}</p>
      ) : (
        <>
          <div className="chart-grid">
            <div className="card">
              <div className="label">{t.reqOverTime}</div>
              <BarChart values={series} labels={labels} />
            </div>
            <div className="card">
              <div className="label">{t.hitMix}</div>
              <HBars rows={mix} />
            </div>
          </div>
          <div className="cards" style={{ marginTop: 12 }}>
            <div className="card"><div className="label">2xx</div><div className="value">{stats.ok}</div></div>
            <div className="card"><div className="label">SSE</div><div className="value">{stats.sse}</div></div>
          </div>
        </>
      )}
      <div className="update-row">
        <span className="muted">{t.version}: {ver || "?"}{upd ? (upd.newer ? ` → ${upd.latest}` : ` · ${t.upToDate}`) : ""}</span>
        <button type="button" onClick={async () => {
          setUpdMsg("");
          try {
            const u = await api.checkUpdate();
            setUpd(u);
            setUpdMsg(u.newer ? t.updateAvail : t.upToDate);
          } catch (e) { setUpdMsg(String(e)); }
        }}>{t.checkUpdate}</button>
        {upd?.newer && (
          <button type="button" onClick={async () => {
            setUpdMsg(t.updating);
            try { await api.applyUpdate(); } catch (e) { setUpdMsg(String(e)); }
          }}>{t.updateNow}</button>
        )}
        {updMsg && <span className="flash">{updMsg}</span>}
      </div>
    </section>
  );
}
