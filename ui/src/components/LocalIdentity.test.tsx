import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setAdminToken, type LocalIdentityCategory, type LocalIdentityState } from "../api";
import { I18nProvider } from "../i18n";
import RulesPage from "../pages/Rules";
import LocalIdentity from "./LocalIdentity";

const initial: LocalIdentityState = {
  enabled: false,
  app_ids: true,
  hardware: true,
  user_context: false,
  git_email: false,
  supported_platform: "windows",
  rules: [],
  last_scan: null,
  collection_available: true,
  restart_required: false,
  running_enabled: false,
  running_rule_count: 0,
};

const labels = {
  enabled: "Turn on local identity protection at the next start",
  app: "Claude's local device and installation IDs",
  hardware: "Windows, disk, and network adapter IDs",
  user: "Your username and computer name",
  git: "The email address in global Git settings",
  save: "Save settings",
  refresh: "Reread local identifiers",
};

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
}

const fetchMock = vi.fn<typeof fetch>();

function mount() {
  return render(<I18nProvider><LocalIdentity /></I18nProvider>);
}

function checkbox(name: string) {
  return screen.getByRole("checkbox", { name });
}

function currentProcess() {
  return within(screen.getByRole("group", { name: "Current Veil process" }));
}

function savedSettings() {
  return within(screen.getByRole("group", { name: "Saved settings for next start" }));
}

async function loaded() {
  await waitFor(() => expect(checkbox(labels.enabled)).toBeEnabled());
}

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
  localStorage.setItem("veil_lang", "en");
  setAdminToken("test-admin-token");
});

afterEach(() => {
  cleanup();
  setAdminToken("");
  localStorage.clear();
  vi.unstubAllGlobals();
});

describe("Local identity protection", () => {
  it("requires administrator sign-in for GET even on localhost and permits retry", async () => {
    setAdminToken("");
    fetchMock.mockResolvedValueOnce(json({ error: "unauthorized" }, 401));
    mount();
    expect(await screen.findByRole("alert")).toHaveTextContent("Sign in with the admin token first");
    expect(checkbox(labels.enabled)).toBeDisabled();
    expect(fetchMock.mock.calls[0][0]).toBe("/api/local-identity");
    expect(new Headers(fetchMock.mock.calls[0][1]?.headers).has("x-veil-admin-token")).toBe(false);

    setAdminToken("new-admin-token");
    fetchMock.mockResolvedValueOnce(json(initial));
    fireEvent.click(screen.getByRole("button", { name: "Retry loading" }));
    await loaded();
    expect(new Headers(fetchMock.mock.calls[1][1]?.headers).get("x-veil-admin-token")).toBe("new-admin-token");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("saves selected flags without starting Veil, blocks duplicates, and shows enabled settings awaiting restart", async () => {
    let finishSave!: (response: Response) => void;
    const saving = new Promise<Response>((resolve) => { finishSave = resolve; });
    fetchMock.mockResolvedValueOnce(json(initial)).mockReturnValueOnce(saving);
    mount();
    await loaded();
    expect(checkbox(labels.enabled)).not.toBeChecked();
    expect(checkbox(labels.app)).toBeChecked();
    expect(checkbox(labels.hardware)).toBeChecked();
    expect(checkbox(labels.user)).not.toBeChecked();
    expect(checkbox(labels.git)).not.toBeChecked();
    expect(screen.getByText("Not read yet")).toBeInTheDocument();
    expect(checkbox(labels.enabled)).toHaveAccessibleDescription(/does not start or restart Veil/);

    fireEvent.click(checkbox(labels.enabled));
    fireEvent.click(checkbox(labels.app));
    fireEvent.click(checkbox(labels.user));
    fireEvent.click(checkbox(labels.git));
    // Draft editing must not pretend that the settings or the process have changed.
    expect(currentProcess().getByText("Off")).toBeInTheDocument();
    expect(savedSettings().getByText("Off at next start")).toBeInTheDocument();
    expect(screen.getByRole("note")).toHaveTextContent("No restart is needed");
    expect(screen.getByRole("button", { name: labels.refresh })).toBeDisabled();
    expect(screen.getByText(/You have unsaved selections/)).toBeInTheDocument();
    const save = screen.getByRole("button", { name: labels.save });
    fireEvent.click(save);
    fireEvent.click(save);
    fireEvent.click(screen.getByRole("button", { name: labels.refresh }));
    expect(save).toBeDisabled();
    expect(checkbox(labels.hardware)).toBeDisabled();
    expect(screen.getByRole("status")).toHaveTextContent("20 seconds");
    expect(fetchMock).toHaveBeenCalledTimes(2);
    const [path, request] = fetchMock.mock.calls[1];
    const selected = { enabled: true, app_ids: false, hardware: true, user_context: true, git_email: true };
    expect(path).toBe("/api/local-identity");
    expect(request?.method).toBe("PUT");
    expect(JSON.parse(request?.body as string)).toEqual(selected);
    expect(new Headers(request?.headers).get("x-veil-admin-token")).toBe("test-admin-token");
    expect(new Headers(request?.headers).get("content-type")).toBe("application/json");

    await act(async () => { finishSave(json({ ...initial, ...selected, last_scan: 1_800_000_000, restart_required: true })); });
    expect(screen.getByRole("status")).toHaveTextContent("Settings saved. Restart Veil");
    expect(currentProcess().getByText("Off")).toBeInTheDocument();
    expect(currentProcess().getByText("0")).toBeInTheDocument();
    expect(savedSettings().getByText("On at next start")).toBeInTheDocument();
    expect(screen.getByRole("note")).toHaveTextContent("this process is not protecting local identifiers");
    expect(screen.getByRole("note")).toHaveTextContent("will turn protection on after you restart Veil");
    expect(save).toBeEnabled();
    expect(save).toHaveTextContent(labels.save);
    expect(screen.getByRole("button", { name: labels.refresh })).toBeEnabled();
  });

  it("requires at least one group only when protection is selected for next start", async () => {
    const noGroups = { ...initial, app_ids: false, hardware: false };
    fetchMock.mockResolvedValueOnce(json(noGroups));
    mount();
    await loaded();
    const save = screen.getByRole("button", { name: labels.save });
    expect(save).toBeEnabled();
    fireEvent.click(checkbox(labels.enabled));
    expect(save).toBeDisabled();
    expect(screen.getByText(/Choose at least one identifier group/)).toBeInTheDocument();
    fireEvent.click(save);
    fireEvent.submit(screen.getByRole("form", { name: "Saved settings for next start" }));
    expect(fetchMock).toHaveBeenCalledTimes(1);
    fireEvent.click(checkbox(labels.enabled));
    expect(save).toBeEnabled();
    expect(screen.queryByText(/Choose at least one identifier group/)).not.toBeInTheDocument();
    fireEvent.click(checkbox(labels.enabled));
    fireEvent.click(checkbox(labels.hardware));
    expect(save).toBeEnabled();
    fetchMock.mockResolvedValueOnce(json({ ...noGroups, enabled: true, hardware: true, restart_required: true }));
    fireEvent.click(save);
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Settings saved"));
    expect(JSON.parse(fetchMock.mock.calls[1][1]?.body as string)).toEqual({
      enabled: true, app_ids: false, hardware: true, user_context: false, git_email: false,
    });
  });

  it("preserves current running protection and collected summaries when saving disabled settings", async () => {
    const rule = { id: "hardware", type_prefix: "HWUUID", category: "hardware" as const, variants: 3, active: true };
    const running = { ...initial, enabled: true, running_enabled: true, running_rule_count: 15, rules: [rule] };
    fetchMock.mockResolvedValueOnce(json(running));
    fetchMock.mockResolvedValueOnce(json({ ...running, enabled: false, rules: [{ ...rule, active: false }], restart_required: true }));
    mount();
    await loaded();
    fireEvent.click(checkbox(labels.enabled));
    fireEvent.click(screen.getByRole("button", { name: labels.save }));
    await screen.findByText("Not selected", { selector: "td" });
    expect(JSON.parse(fetchMock.mock.calls[1][1]?.body as string).enabled).toBe(false);
    expect(currentProcess().getByText("On")).toBeInTheDocument();
    expect(currentProcess().getByText("15")).toBeInTheDocument();
    expect(savedSettings().getByText("Off at next start")).toBeInTheDocument();
    expect(screen.getByRole("note")).toHaveTextContent("this process is still protecting local identifiers");
    expect(screen.getByRole("note")).toHaveTextContent("will turn protection off after you restart Veil");
    expect(screen.getByText("HWUUID")).toBeInTheDocument();
    expect(screen.getByText("3", { selector: "td" })).toBeInTheDocument();
    expect(screen.getByText(/Turning protection off keeps the saved identifiers/)).toBeInTheDocument();
  });

  it("rereads with an empty POST body, blocks concurrent changes, and retains the last snapshot on failure", async () => {
    const rule = { id: "device", type_prefix: "DEVICEID", category: "app_ids", variants: 2, active: true };
    const snapshot = { ...initial, enabled: true, running_enabled: true, running_rule_count: 1, last_scan: 1_800_000_000, rules: [rule] };
    let finishRefresh!: (response: Response) => void;
    const refreshing = new Promise<Response>((resolve) => { finishRefresh = resolve; });
    fetchMock.mockResolvedValueOnce(json(snapshot)).mockReturnValueOnce(refreshing);
    mount();
    await loaded();
    const refresh = screen.getByRole("button", { name: labels.refresh });
    const scanBefore = savedSettings().getByText(new Date(snapshot.last_scan * 1000).toLocaleString("en")).textContent!;
    expect(refresh).toHaveAccessibleDescription(/replacing hardware or reinstalling/);
    fireEvent.click(refresh);
    fireEvent.click(refresh);
    expect(refresh).toBeDisabled();
    expect(screen.getByRole("button", { name: labels.save })).toBeDisabled();
    expect(checkbox(labels.user)).toBeDisabled();
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls[1][0]).toBe("/api/local-identity/refresh");
    expect(fetchMock.mock.calls[1][1]?.method).toBe("POST");
    expect(JSON.parse(fetchMock.mock.calls[1][1]?.body as string)).toEqual({});
    expect(new Headers(fetchMock.mock.calls[1][1]?.headers).get("x-veil-admin-token")).toBe("test-admin-token");
    await act(async () => { finishRefresh(json({ error: "SECRET_MACHINE_ID" }, 500)); });
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not reread identifiers");
    expect(screen.getByText("2", { selector: "td" })).toBeInTheDocument();
    expect(screen.getByText("Selected", { selector: "td" })).toBeInTheDocument();
    expect(currentProcess().getByText("On")).toBeInTheDocument();
    expect(currentProcess().getByText("1")).toBeInTheDocument();
    expect(savedSettings().getByText(scanBefore)).toBeInTheDocument();
    expect(screen.getByRole("note")).toHaveTextContent("No restart is needed");
    expect(screen.getByRole("region")).not.toHaveTextContent("SECRET_MACHINE_ID");

    fetchMock.mockResolvedValueOnce(json({ ...snapshot, last_scan: 1_800_000_100, rules: [{ ...rule, variants: 4 }], restart_required: true }));
    fireEvent.click(refresh);
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Identifiers reread and saved. Restart Veil"));
    expect(screen.getByText("4", { selector: "td" })).toBeInTheDocument();
    expect(currentProcess().getByText("1")).toBeInTheDocument();
    expect(screen.getByRole("note")).toHaveTextContent("saved selections or identifiers differ");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("does not request a restart when rereading only changes last_scan", async () => {
    const rule = { id: "device", type_prefix: "DEVICEID", category: "app_ids", variants: 1, active: true };
    const running = { ...initial, enabled: true, running_enabled: true, running_rule_count: 1, last_scan: 1_800_000_000, rules: [rule] };
    fetchMock.mockResolvedValueOnce(json(running)).mockResolvedValueOnce(json({ ...running, last_scan: 1_800_000_100 }));
    mount();
    await loaded();
    fireEvent.click(screen.getByRole("button", { name: labels.refresh }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Identifiers reread and saved. No restart is needed"));
    expect(screen.getByRole("note")).toHaveTextContent("No restart is needed");
    expect(screen.getByRole("region")).not.toHaveTextContent("Waiting for restart");
    expect(currentProcess().getByText("On")).toBeInTheDocument();
    expect(currentProcess().getByText("1")).toBeInTheDocument();
    expect(screen.getByText("1", { selector: "td" })).toBeInTheDocument();
    expect(savedSettings().getByText(new Date(1_800_000_100 * 1000).toLocaleString("en"))).toBeInTheDocument();
  });

  it.each([true, false])("uses backend process state after restart with saved enabled=%s", async (enabled) => {
    fetchMock.mockResolvedValueOnce(json({ ...initial, enabled, running_enabled: !enabled, running_rule_count: enabled ? 0 : 15, restart_required: true }));
    const view = mount();
    await loaded();
    expect(screen.getByRole("note")).toHaveTextContent("Waiting for restart");
    expect(currentProcess().getByText(enabled ? "Off" : "On")).toBeInTheDocument();
    view.unmount();

    fetchMock.mockResolvedValueOnce(json({ ...initial, enabled, running_enabled: enabled, running_rule_count: enabled ? 15 : 0 }));
    mount();
    await loaded();
    expect(currentProcess().getByText(enabled ? "On" : "Off")).toBeInTheDocument();
    expect(currentProcess().getByText(enabled ? "15" : "0")).toBeInTheDocument();
    expect(savedSettings().getByText(enabled ? "On at next start" : "Off at next start")).toBeInTheDocument();
    expect(screen.getByRole("note")).toHaveTextContent("No restart is needed");
  });

  it("preserves unsaved choices and both saved and process state after a failed save, then retries", async () => {
    fetchMock.mockResolvedValueOnce(json(initial)).mockRejectedValueOnce(new Error("private-user@private-host"));
    mount();
    await loaded();
    fireEvent.click(checkbox(labels.enabled));
    fireEvent.click(checkbox(labels.user));
    fireEvent.click(screen.getByRole("button", { name: labels.save }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not save");
    expect(checkbox(labels.enabled)).toBeChecked();
    expect(checkbox(labels.user)).toBeChecked();
    expect(currentProcess().getByText("Off")).toBeInTheDocument();
    expect(savedSettings().getByText("Off at next start")).toBeInTheDocument();
    expect(screen.getByRole("region")).not.toHaveTextContent("private-user@private-host");
    expect(screen.getByRole("button", { name: labels.refresh })).toBeDisabled();

    fetchMock.mockResolvedValueOnce(json({ ...initial, enabled: true, user_context: true, restart_required: true }));
    fireEvent.click(screen.getByRole("button", { name: labels.save }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Settings saved"));
    expect(JSON.parse(fetchMock.mock.calls[2][1]?.body as string).user_context).toBe(true);
    expect(savedSettings().getByText("On at next start")).toBeInTheDocument();
    expect(currentProcess().getByText("Off")).toBeInTheDocument();
  });

  it.each(["save", "refresh"] as const)("shows sign-in guidance for a 401 during %s without discarding state", async (action) => {
    fetchMock.mockResolvedValueOnce(json({ ...initial, enabled: true, running_enabled: true, running_rule_count: 5 })).mockResolvedValueOnce(json({}, 401));
    mount();
    await loaded();
    fireEvent.click(screen.getByRole("button", { name: action === "save" ? labels.save : labels.refresh }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Sign in with the admin token first");
    expect(checkbox(labels.enabled)).toBeChecked();
    expect(currentProcess().getByText("On")).toBeInTheDocument();
    expect(currentProcess().getByText("5")).toBeInTheDocument();
    expect(savedSettings().getByText("On at next start")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: labels.save })).toBeEnabled();
  });

  it("shows a recoverable load failure without allowing defaults to overwrite the server", async () => {
    fetchMock.mockResolvedValueOnce(json({ error: "PRIVATE_SERIAL" }, 503));
    mount();
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not load");
    expect(checkbox(labels.app)).toBeDisabled();
    expect(screen.getByRole("button", { name: labels.save })).toBeDisabled();
    expect(screen.queryByRole("group", { name: "Current Veil process" })).not.toBeInTheDocument();
    expect(screen.getByRole("region")).not.toHaveTextContent("PRIVATE_SERIAL");
    fetchMock.mockResolvedValueOnce(json({ ...initial, app_ids: false, git_email: true }));
    fireEvent.click(screen.getByRole("button", { name: "Retry loading" }));
    await loaded();
    expect(checkbox(labels.app)).not.toBeChecked();
    expect(checkbox(labels.git)).toBeChecked();
  });

  it("renders only safe summaries even if unexpected raw values or patterns are present", async () => {
    fetchMock.mockResolvedValueOnce(json({
      ...initial,
      raw_value: "PRIVATE_HOSTNAME",
      rules: [{ id: "PRIVATE_RULE_ID", type_prefix: "DEVICEID", category: "app_ids", variants: 5, active: true,
        pattern: "PRIVATE_REGEX_PATTERN", value: "PRIVATE_DEVICE_SERIAL", words: ["PRIVATE_EMAIL@example.invalid"] }],
    }));
    const { container } = mount();
    await loaded();
    const table = screen.getByRole("table");
    expect(within(table).getAllByRole("columnheader").map((cell) => cell.textContent)).toEqual([
      "Identifier / type code", "What it matches", "At next start", "Saved groups",
    ]);
    expect(within(table).getByText("Claude user ID")).toBeInTheDocument();
    expect(within(table).getByText("DEVICEID")).toBeInTheDocument();
    expect(within(table).getByText("Selected")).toBeInTheDocument();
    expect(within(table).getByText("5")).toBeInTheDocument();
    // Selected is explicitly a next-start setting, not evidence of runtime matching.
    expect(currentProcess().getByText("Off")).toBeInTheDocument();
    expect(screen.getByText(/Counts are saved groups of matching patterns/)).toHaveTextContent("not device counts or confirmed redaction counts");
    expect(container.innerHTML).not.toContain("PRIVATE_");
    expect(container.querySelector("textarea")).not.toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("explains each of the four choices in plain language and supports Chinese", async () => {
    localStorage.setItem("veil_lang", "zh");
    fetchMock.mockResolvedValueOnce(json(initial));
    mount();
    expect(await screen.findByText("尚未读取")).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "下次启动时开启本机标识保护" })).toHaveAccessibleDescription(/此按钮不会启动或重启 Veil/);
    expect(screen.getByRole("checkbox", { name: "Claude 保存在本机的设备和安装编号" })).toHaveAccessibleDescription(/userID.*machineID.*安装编号/);
    expect(screen.getByRole("checkbox", { name: "Windows、硬盘和网卡的编号" })).toHaveAccessibleDescription(/物理网卡地址.*Windows/);
    expect(screen.getByRole("checkbox", { name: "你的用户名和电脑名" })).toHaveAccessibleDescription(/普通标题.*不会替换/);
    expect(screen.getByRole("checkbox", { name: "Git 全局设置中的邮箱" })).toHaveAccessibleDescription(/Git user.email/);
    expect(screen.getByRole("button", { name: "保存设置" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重新读取本机标识" })).toHaveAccessibleDescription(/换硬件或重装后使用/);
    expect(screen.getByRole("note")).toHaveTextContent("无需重启");
    expect(screen.getByText(/不会每次请求采集/)).toHaveTextContent("不保证覆盖截图或所有编码");
    expect(screen.getByText(/组数表示该类型已保存的匹配模式组/)).toHaveTextContent("重复读取相同值不会新增");
  });

  const identifierDescriptions: Array<[string, LocalIdentityCategory, string, string, RegExp, RegExp]> = [
    ["DEVICEID", "app_ids", "Claude user ID", "Claude 用户编号", /complete 64-character userID/, /userID.*64/],
    ["MACHINEID", "app_ids", "Claude machine ID", "Claude 机器编号", /64-character machineID.*12 characters/, /64.*12/],
    ["INSTALLID", "app_ids", "Claude desktop installation ID", "Claude 桌面版安装编号", /UUID.*hyphens.*Base64/, /UUID.*横线.*Base64/],
    ["WINID", "hardware", "Windows installation ID", "Windows 安装标识", /MachineGuid/, /MachineGuid/],
    ["HWUUID", "hardware", "Whole-computer UUID", "整机 UUID 编号", /SMBIOS UUID/, /SMBIOS UUID/],
    ["SQMID", "hardware", "Windows usage statistics ID", "Windows 使用统计编号", /SQM.*statistics/, /SQM/],
    ["PRODUCTID", "hardware", "Windows product ID", "Windows 产品 ID", /ProductId.*not the activation key/, /ProductId.*不等同于激活密钥/],
    ["USERSID", "hardware", "Local account SID", "本机账户 SID 编号", /machine-specific.*numeric suffix is kept/, /机器.*保留.*账户数字尾号/],
    ["DISKSN", "hardware", "Disk serial number", "硬盘序列号", /serial number.*separators removed/, /序列号.*分隔符/],
    ["VOLSN", "hardware", "Drive volume serial number", "盘符的卷序列号", /eight characters.*4-4/, /8.*4-4/],
    ["MAC", "hardware", "Physical network adapter address", "物理网卡 MAC 地址", /MAC.*colons.*hyphens.*without separators/, /冒号.*横线.*连写/],
    ["BIOSSN", "hardware", "BIOS, board, and chassis serials", "BIOS、主板和机箱序列号", /Non-generic.*BIOS.*motherboard.*chassis/, /BIOS.*主板.*机箱.*非通用/],
    ["USERNAME", "user_context", "Username in account context", "账户语境里的用户名", /user directories.*assignments.*computer-name.*Ordinary headings are left alone/, /用户目录.*赋值.*电脑名.*独占一行.*普通标题不替换/],
    ["HOSTNAME", "user_context", "Computer name", "电脑名称", /complete computer name.*longer name/, /完整匹配.*更长名称/],
    ["GITEMAIL", "git_email", "Global Git email address", "Git 全局邮箱地址", /exact match.*global Git user.email/, /精确匹配.*Git user.email/],
  ];

  it.each(["en", "zh"] as const)("describes all 15 identifier types individually in %s with auxiliary codes", async (lang) => {
    localStorage.setItem("veil_lang", lang);
    const rules = identifierDescriptions.map(([type_prefix, category], index) => ({ id: `rule-${index}`, type_prefix, category, variants: index + 1, active: true }));
    fetchMock.mockResolvedValueOnce(json({ ...initial, rules }));
    mount();
    const table = await screen.findByRole("table");
    expect(within(table).getAllByRole("row")).toHaveLength(16);
    for (const [code, , englishName, chineseName, englishDescription, chineseDescription] of identifierDescriptions) {
      const row = within(table).getByText(code).closest("tr")!;
      expect(within(row).getByRole("rowheader")).toHaveTextContent(lang === "en" ? englishName : chineseName);
      expect(row).toHaveTextContent(lang === "en" ? englishDescription : chineseDescription);
    }
    expect(within(table).queryByText(lang === "en" ? "Other identifier" : "其他标识")).not.toBeInTheDocument();
  });

  it("explains unavailable collection and prevents rereading while allowing configuration changes", async () => {
    fetchMock.mockResolvedValueOnce(json({ ...initial, collection_available: false, supported_platform: "other" }));
    mount();
    await loaded();
    expect(screen.getByText(/Collection is unavailable/)).toHaveTextContent("user home directory");
    expect(screen.getByRole("button", { name: labels.refresh })).toBeDisabled();
    expect(screen.getByRole("button", { name: labels.save })).toBeEnabled();
    expect(screen.getByText(/Hardware collection is currently Windows-only/)).toBeInTheDocument();
  });

  it("appears above manual rules and saves without sending manual rules back", async () => {
    fetchMock.mockImplementation(async (path) => {
      if (path === "/api/local-identity") return json(initial);
      if (path === "/api/rules") return json([{ id: "manual-rule", type_prefix: "CUSTOM", enabled: true, priority: 50, kind: "regex", source: "custom", pattern: "manual-pattern" }]);
      if (path === "/api/allowlist") return json(["manual-allowlist"]);
      throw new Error("Unexpected request");
    });
    render(<I18nProvider><RulesPage /></I18nProvider>);
    await loaded();
    const manualRule = await screen.findByText("manual-rule");
    const local = screen.getByRole("region", { name: "Local identity protection" });
    expect(local.compareDocumentPosition(manualRule) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    fireEvent.click(checkbox(labels.enabled));
    fireEvent.click(screen.getByRole("button", { name: labels.save }));
    await screen.findByText(/Settings saved\./);
    const writes = fetchMock.mock.calls.filter(([, request]) => request?.method === "PUT");
    expect(writes).toHaveLength(1);
    expect(writes[0][0]).toBe("/api/local-identity");
    expect(screen.getByDisplayValue("manual-allowlist")).toBeInTheDocument();
    expect(manualRule).toBeInTheDocument();
  });
});
