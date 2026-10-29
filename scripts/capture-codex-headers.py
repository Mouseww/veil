#!/usr/bin/env python3
"""Capture real Codex CLI HTTP headers locally; persist only redacted metadata.

Uses the existing veil profile, overrides its endpoint for this invocation, and
returns a synthetic Responses SSE reply. Never forwards requests upstream.
"""
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import subprocess
import tempfile
import threading


SAFE_HEADERS = {
    "host", "content-type", "content-length", "content-encoding",
    "accept", "accept-encoding", "connection", "user-agent", "originator",
    "openai-beta", "version", "x-codex-beta-features",
}


def main():
    records = []

    class Receiver(BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass

        def do_GET(self):
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()

        def do_POST(self):
            body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
            headers = []
            for name, value in self.headers.items():
                key = name.lower()
                if key == "authorization":
                    value = value.split(" ", 1)[0] + " <redacted>"
                elif key not in SAFE_HEADERS:
                    value = "<redacted>"
                headers.append({"name": name, "value": value})
            records.append({"method": "POST", "path": self.path,
                            "http_version": self.request_version,
                            "headers": headers, "body_bytes": len(body)})
            message = {"id": "msg_header_check", "type": "message", "status": "completed",
                       "role": "assistant", "content": [{"type": "output_text", "text": "HEADER_CHECK_OK", "annotations": []}]}
            response = {"id": "resp_header_check", "object": "response", "status": "completed",
                        "output": [message], "usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2}}
            events = [
                {"type": "response.created", "response": {"id": "resp_header_check", "status": "in_progress", "output": []}},
                {"type": "response.output_item.added", "output_index": 0, "item": message},
                {"type": "response.output_text.delta", "item_id": "msg_header_check", "output_index": 0, "content_index": 0, "delta": "HEADER_CHECK_OK"},
                {"type": "response.output_item.done", "output_index": 0, "item": message},
                {"type": "response.completed", "response": response},
            ]
            payload = "".join("event: " + e["type"] + "\ndata: " + json.dumps(e) + "\n\n" for e in events).encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

    config = Path.home() / ".codex" / "config.toml"
    original = hashlib.sha256(config.read_bytes()).hexdigest()
    server = ThreadingHTTPServer(("127.0.0.1", 0), Receiver)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    endpoint = "http://127.0.0.1:" + str(server.server_port) + "/v1"
    try:
        with tempfile.TemporaryDirectory(prefix="veil-header-check-") as work:
            command = ["codex", "exec", "--profile", "veil", "--ephemeral",
                       "--skip-git-repo-check", "--sandbox", "read-only", "-C", work,
                       "-c", "model_providers.veil.base_url=" + json.dumps(endpoint),
                       "-c", "model_providers.veil.supports_websockets=false",
                       "-c", "model_providers.veil.request_max_retries=0",
                       "-c", "model_providers.veil.stream_max_retries=0",
                       "--json", "Do not use tools. Reply HEADER_CHECK_OK."]
            completed = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                       text=True, timeout=45)
    finally:
        server.shutdown()
        server.server_close()
    events = []
    for line in completed.stdout.splitlines():
        try:
            events.append(json.loads(line))
        except ValueError:
            pass
    evidence = {
        "cli_version": subprocess.check_output(["codex", "--version"], text=True).strip(),
        "profile": "veil", "transport": "HTTP/SSE", "upstream": "local synthetic responder",
        "codex_exit_code": completed.returncode,
        "turn_completed": any(e.get("type") == "turn.completed" for e in events),
        "base_config_unchanged": original == hashlib.sha256(config.read_bytes()).hexdigest(),
        "requests": records,
    }
    output = Path("target/codex-header-check/headers.redacted.json")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(evidence, ensure_ascii=False, indent=2))
    print("Evidence:", output.resolve())
    if not records:
        # Do not print Codex stderr: it may contain endpoint/credential details.
        raise SystemExit("No HTTP request captured; CLI exit code " + str(completed.returncode))


if __name__ == "__main__":
    main()
