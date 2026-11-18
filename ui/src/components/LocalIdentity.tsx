import { useEffect, useId, useRef, useState, type FormEvent } from "react";
import { api, type LocalIdentitySettings, type LocalIdentityState } from "../api";
import { useT } from "../i18n";

const DEFAULTS: LocalIdentitySettings = {
  enabled: false,
  app_ids: true,
  hardware: true,
  user_context: false,
  git_email: false,
};

// Only send the five editable flags, never rule metadata or collected values.
function settingsOf(state: LocalIdentityState): LocalIdentitySettings {
  return {
    enabled: state.enabled,
    app_ids: state.app_ids,
    hardware: state.hardware,
    user_context: state.user_context,
    git_email: state.git_email,
  };
}

type Failure = "auth" | "load" | "save" | "refresh";

function isUnauthorized(error: unknown) {
  return error instanceof Error &&
    ((error as Error & { status?: number }).status === 401 || error.message === "401");
}

export default function LocalIdentity() {
  const { t, lang } = useT();
  const id = useId();
  const [state, setState] = useState<LocalIdentityState | null>(null);
  const [draft, setDraft] = useState<LocalIdentitySettings>(DEFAULTS);
  const [busy, setBusy] = useState<"load" | "save" | "refresh" | null>("load");
  const [failure, setFailure] = useState<Failure | null>(null);
  const [success, setSuccess] = useState<"save" | "refresh" | null>(null);
  const submitting = useRef(false);

  function accept(next: LocalIdentityState) {
    setState(next);
    setDraft(settingsOf(next));
  }

  async function load(isCurrent = () => true) {
    setBusy("load");
    setFailure(null);
    try {
      const next = await api.localIdentity();
      if (isCurrent()) accept(next);
    } catch (error) {
      if (isCurrent()) setFailure(isUnauthorized(error) ? "auth" : "load");
    } finally {
      if (isCurrent()) setBusy(null);
    }
  }

  useEffect(() => {
    let current = true;
    void load(() => current);
    return () => { current = false; };
  }, []);

  const dirty = state !== null && (Object.keys(DEFAULTS) as Array<keyof LocalIdentitySettings>)
    .some((key) => draft[key] !== state[key]);
  const needsCategory = draft.enabled && !draft.app_ids && !draft.hardware && !draft.user_context && !draft.git_email;
  const choices: Array<{ key: keyof LocalIdentitySettings; label: string; help: string }> = [
    { key: "enabled", label: t.localIdentityEnabled, help: t.localIdentitySaveHelp },
    { key: "app_ids", label: t.localIdentityAppIds, help: t.localIdentityAppIdsHelp },
    { key: "hardware", label: t.localIdentityHardware, help: t.localIdentityHardwareHelp },
    { key: "user_context", label: t.localIdentityUserContext, help: t.localIdentityUserContextHelp },
    { key: "git_email", label: t.localIdentityGitEmail, help: t.localIdentityGitEmailHelp },
  ];
  const types = new Map<string, { name: string; help: string }>([
    ["DEVICEID", { name: t.localIdentityDeviceId, help: t.localIdentityDeviceIdHelp }],
    ["MACHINEID", { name: t.localIdentityMachineId, help: t.localIdentityMachineIdHelp }],
    ["INSTALLID", { name: t.localIdentityInstallId, help: t.localIdentityInstallIdHelp }],
    ["WINID", { name: t.localIdentityWinId, help: t.localIdentityWinIdHelp }],
    ["HWUUID", { name: t.localIdentityHwUuid, help: t.localIdentityHwUuidHelp }],
    ["SQMID", { name: t.localIdentitySqmId, help: t.localIdentitySqmIdHelp }],
    ["PRODUCTID", { name: t.localIdentityProductId, help: t.localIdentityProductIdHelp }],
    ["USERSID", { name: t.localIdentityUserSid, help: t.localIdentityUserSidHelp }],
    ["DISKSN", { name: t.localIdentityDiskSn, help: t.localIdentityDiskSnHelp }],
    ["VOLSN", { name: t.localIdentityVolSn, help: t.localIdentityVolSnHelp }],
    ["MAC", { name: t.localIdentityMac, help: t.localIdentityMacHelp }],
    ["BIOSSN", { name: t.localIdentityBiosSn, help: t.localIdentityBiosSnHelp }],
    ["USERNAME", { name: t.localIdentityUsername, help: t.localIdentityUsernameHelp }],
    ["HOSTNAME", { name: t.localIdentityHostname, help: t.localIdentityHostnameHelp }],
    ["GITEMAIL", { name: t.localIdentityGitEmailType, help: t.localIdentityGitEmailTypeHelp }],
  ]);
  const errors: Record<Failure, string> = {
    auth: t.localIdentityNeedLogin,
    load: t.localIdentityLoadFailed,
    save: t.localIdentitySaveFailed,
    refresh: t.localIdentityRefreshFailed,
  };
  const pendingMessage = state?.running_enabled !== state?.enabled
    ? (state?.enabled ? t.localIdentityPendingOn : t.localIdentityPendingOff)
    : t.localIdentityPendingChanges;

  async function submit(action: "save" | "refresh") {
    if (!state || busy || submitting.current) return;
    if (action === "save" && needsCategory) return;
    if (action === "refresh" && (dirty || !state.collection_available)) return;
    submitting.current = true;
    setBusy(action);
    setFailure(null);
    setSuccess(null);
    try {
      const next = action === "save"
        ? await api.putLocalIdentity(draft)
        : await api.refreshLocalIdentity();
      accept(next);
      setSuccess(action);
    } catch (error) {
      // Keep both the last successful snapshot and unsaved choices on failure.
      // Do not echo server error bodies, which could contain local identifiers.
      setFailure(isUnauthorized(error) ? "auth" : action);
    } finally {
      submitting.current = false;
      setBusy(null);
    }
  }

  function save(event: FormEvent) {
    event.preventDefault();
    void submit("save");
  }

  return (
    <section className="card" aria-label={t.localIdentityTitle} aria-busy={busy !== null} style={{ margin: "18px 0" }}>
      <h2>{t.localIdentityTitle}</h2>
      {state && (
        <>
          <div className="row">
            <div role="group" aria-labelledby={`${id}-running`}>
              <h3 id={`${id}-running`} style={{ fontSize: 14 }}>{t.localIdentityCurrent}</h3>
              <dl className="grid" style={{ gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1fr)", overflowWrap: "anywhere" }}>
                <dt>{t.localIdentityProtection}</dt>
                <dd><strong>{state.running_enabled ? t.localIdentityOn : t.localIdentityOff}</strong></dd>
                <dt>{t.localIdentityLoadedTypes}</dt><dd>{state.running_rule_count}</dd>
              </dl>
            </div>
            <div role="group" aria-labelledby={`${id}-saved`}>
              <h3 id={`${id}-saved`} style={{ fontSize: 14 }}>{t.localIdentitySavedTitle}</h3>
              <dl className="grid" style={{ gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1fr)", overflowWrap: "anywhere" }}>
                <dt>{t.localIdentityProtection}</dt><dd>{state.enabled ? t.localIdentityNextOn : t.localIdentityNextOff}</dd>
                <dt>{t.localIdentityPlatform}</dt><dd>{state.supported_platform}</dd>
                <dt>{t.localIdentityLastScan}</dt>
                <dd>{state.last_scan === null ? t.localIdentityNever : new Date(state.last_scan * 1000).toLocaleString(lang)}</dd>
              </dl>
            </div>
          </div>
          <p className={state.restart_required ? "err copybox" : "help"} role="note">
            {state.restart_required ? <strong>{pendingMessage}</strong> : t.localIdentityNoRestart}
          </p>
        </>
      )}
      <p className="help">{t.localIdentityHelp}</p>
      {state && !state.collection_available && <p className="help">{t.localIdentityUnavailable}</p>}
      {failure && <p className="err" role="alert">{errors[failure]}</p>}
      {!state && !busy && <button className="ghost" type="button" onClick={() => void load()}>{t.localIdentityRetry}</button>}
      <form aria-label={t.localIdentitySavedTitle} onSubmit={save}>
        <fieldset disabled={busy !== null || state === null} style={{ border: 0, padding: 0, margin: 0, minWidth: 0 }}>
          {choices.map(({ key, label, help }) => (
            <div key={key}>
              <label style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <input
                  type="checkbox"
                  style={{ width: "auto", margin: 0 }}
                  checked={draft[key]}
                  aria-describedby={`${id}-${key}-help`}
                  onChange={(event) => {
                    setDraft({ ...draft, [key]: event.target.checked });
                    setSuccess(null);
                  }}
                />
                {label}
              </label>
              <p id={`${id}-${key}-help`} className="help" style={{ margin: "4px 0 12px 24px" }}>{help}</p>
            </div>
          ))}
          {needsCategory && <p className="err">{t.localIdentityChooseOne}</p>}
          <button className="btn" type="submit" disabled={needsCategory}>{t.localIdentitySave}</button>
          <button className="ghost" type="button" disabled={dirty || !state?.collection_available} aria-describedby={`${id}-refresh-help`} onClick={() => void submit("refresh")}>
            {t.localIdentityRefresh}
          </button>
        </fieldset>
      </form>
      <p id={`${id}-refresh-help`} className="help">{t.localIdentityRefreshHelp}</p>
      {dirty && <p className="help">{t.localIdentitySaveFirst}</p>}
      {busy && <p className="help" role="status">{busy === "load" ? t.loading : t.localIdentityWait}</p>}
      {success && state && <p className="flash" role="status">
        {success === "save" ? t.localIdentitySaved : t.localIdentityRefreshed}{" "}
        {state.restart_required ? t.localIdentityRestartNeeded : t.localIdentityNoRestart}
      </p>}
      <p className="help">{t.localIdentityPrivacy}</p>
      <p className="help">{t.localIdentityGroupsHelp}</p>
      {state && (state.rules.length === 0 ? <p className="help">{t.localIdentityEmpty}</p> : (
        <div style={{ overflowX: "auto", contain: "inline-size" }}>
        <table aria-label={t.localIdentityTitle} style={{ tableLayout: "fixed", minWidth: 640, overflowWrap: "anywhere" }}>
          <thead><tr>
            <th scope="col" style={{ width: "26%" }}>{t.localIdentityType}</th>
            <th scope="col" style={{ width: "46%" }}>{t.localIdentityScope}</th>
            <th scope="col" style={{ width: "16%" }}>{t.localIdentityStatus}</th>
            <th scope="col" style={{ width: "12%" }}>{t.localIdentityPatternGroups}</th>
          </tr></thead>
          <tbody>{state.rules.map((rule) => {
            const info = types.get(rule.type_prefix) ?? { name: t.localIdentityOther, help: t.localIdentityOtherHelp };
            return (
              <tr key={rule.id}>
                <th scope="row" style={{ fontSize: 14 }}>
                  <strong style={{ color: "var(--foreground)" }}>{info.name}</strong>
                  <div style={{ marginTop: 4 }}><span className="chip">{rule.type_prefix}</span></div>
                </th>
                <td>{info.help}</td>
                <td>{rule.active ? t.localIdentityActive : t.localIdentityInactive}</td>
                <td>{rule.variants}</td>
              </tr>
            );
          })}</tbody>
        </table>
        </div>
      ))}
    </section>
  );
}
