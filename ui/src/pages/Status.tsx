import { useEffect, useState } from "react";
import { api, type StatusBody } from "../api";

export default function StatusPage() {
  const [s, setS] = useState<StatusBody | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    api.status().then(setS).catch((e) => setErr(String(e)));
  }, []);
  if (err) return <p className="err">管理页连不上网关：{err}</p>;
  if (!s) return <p className="muted">加载中…</p>;
  const base = "http://" + s.bind + ":" + s.proxy_port;
  return (
    <section>
      <h2>开始使用</h2>
      <p className="kicker">三步，敏感信息就不会出网</p>
      <ol className="steps">
        <li>
          <strong>网关已在本机运行</strong>
          <div className="muted">代理 {base}　·　管理 {s.bind}:{s.management_port}</div>
        </li>
        <li>
          <strong>让 Claude Code 走网关</strong>
          <p>在终端执行 <code>veil setup</code>，或把下面这一行加到用户环境变量 / Claude 设置：</p>
          <pre className="copybox">ANTHROPIC_BASE_URL={base}</pre>
          <button
            type="button"
            onClick={async () => {
              await navigator.clipboard.writeText("ANTHROPIC_BASE_URL=" + base);
              setCopied(true);
            }}
          >
            {copied ? "已复制" : "复制这一行"}
          </button>
          <p className="muted">官方订阅不用填 API Key。改完请重启 Claude Code。</p>
        </li>
        <li>
          <strong>正常聊天即可</strong>
          <p className="muted">手机号、连接串、密钥会被替换成占位符再发给模型，回复里会自动变回原文。</p>
        </li>
      </ol>
      <h3>当前状态</h3>
      <dl className="grid">
        <dt>产品</dt><dd>{s.product}</dd>
        <dt>主密钥</dt><dd>{s.master_key_set ? "已就绪（不显示明文）" : "缺失"}</dd>
        <dt>规则数</dt><dd>{s.rule_count}</dd>
        <dt>最近错误</dt><dd>{s.last_error_class ?? "无"}</dd>
      </dl>
    </section>
  );
}
