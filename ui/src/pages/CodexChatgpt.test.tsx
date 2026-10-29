import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../api";
import CodexChatgpt from "./CodexChatgpt";

vi.mock("../api", () => ({ api: { codexChatgpt: vi.fn(), connectCodexChatgpt: vi.fn() } }));
vi.mock("../i18n", () => ({ useT: () => ({ t: {
  codexChatgptTitle: "ChatGPT", codexChatgptHelp: "Keep login",
  codexChatgptConfigured: "Configured", codexChatgptNotConfigured: "Not configured",
  codexChatgptConnect: "Connect", codexChatgptSaved: "Run codex --profile veil",
  codexChatgptVerify: "Check Traffic", codexProfilePath: "Profile",
  clientBase: "Base URL", loading: "Loading", failed: "Failed",
} }) }));

const connection = { configured: false, profile_path: "/test/veil.config.toml", launch_command: "codex --profile veil", base_url: "http://127.0.0.1:18787/codex" };
afterEach(() => { cleanup(); vi.resetAllMocks(); });

describe("Codex subscription connection", () => {
  it("creates an opt-in profile without asking for credentials and shows the launch command", async () => {
    vi.mocked(api.codexChatgpt).mockResolvedValue(connection);
    vi.mocked(api.connectCodexChatgpt).mockResolvedValue({ ...connection, configured: true });
    render(<CodexChatgpt />);
    const button = screen.getByRole("button", { name: "Connect" });
    await waitFor(() => expect(button).toBeEnabled());
    expect(api.connectCodexChatgpt).not.toHaveBeenCalled();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    fireEvent.click(button);
    expect(await screen.findByRole("status")).toHaveTextContent("Run codex --profile veil");
    expect(screen.getByText("/test/veil.config.toml")).toBeInTheDocument();
    expect(screen.getByText("codex --profile veil")).toBeInTheDocument();
    expect(screen.getByText("Configured")).toBeInTheDocument();
    expect(api.connectCodexChatgpt).toHaveBeenCalledTimes(1);
  });

  it("reports a failed write without claiming the connection was configured", async () => {
    vi.mocked(api.codexChatgpt).mockResolvedValue(connection);
    vi.mocked(api.connectCodexChatgpt).mockRejectedValue(new Error("Permission denied"));
    render(<CodexChatgpt />);
    const button = screen.getByRole("button", { name: "Connect" });
    await waitFor(() => expect(button).toBeEnabled());
    fireEvent.click(button);
    expect(await screen.findByRole("alert")).toHaveTextContent("Permission denied");
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(screen.getByText("Not configured")).toBeInTheDocument();
  });
});
