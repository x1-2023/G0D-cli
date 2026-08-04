"""One-request OpenAI-compatible SSE server used by the local CLI smoke test."""

from http.server import BaseHTTPRequestHandler, HTTPServer
import time


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):  # noqa: N802 - BaseHTTPRequestHandler API
        length = int(self.headers.get("Content-Length", "0"))
        self.rfile.read(length)
        payload = (
            'data: {"choices":[{"delta":{"content":"chào từ mock"}}]}\n\n'
            "data: [DONE]\n\n"
        ).encode("utf-8")
        split = payload.index("à".encode("utf-8")) + 1

        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream; charset=utf-8")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload[:split])
        self.wfile.flush()
        time.sleep(0.12)
        self.wfile.write(payload[split:])
        self.wfile.flush()

    def log_message(self, _format, *_args):
        return


if __name__ == "__main__":
    server = HTTPServer(("127.0.0.1", 1234), Handler)
    server.handle_request()
    server.server_close()
