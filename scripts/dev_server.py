#!/usr/bin/env python3
"""
Dev server tĩnh có SPA-fallback cho RustConvert.

Mọi đường dẫn không trỏ tới file có thật (vd: /wav-to-mp3, /docx-to-pdf) sẽ được
phục vụ bằng index.html mà GIỮ NGUYÊN URL trên thanh địa chỉ — mô phỏng đúng hành
vi của file `_redirects` trên Cloudflare Pages / Netlify. Nhờ đó test URL đẹp ngay
ở local (python http.server thường thì các path này trả 404).

Chạy:
    python scripts/dev_server.py          # phục vụ ./frontend tại port 8080
    python scripts/dev_server.py 9000     # đổi port
"""

import http.server
import os
import socketserver
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "frontend"))
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8080


class SPAHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=ROOT, **kwargs)

    def do_GET(self):
        # Nếu path không phải file thật -> trả index.html (fallback kiểu SPA).
        target = self.translate_path(self.path)
        if not os.path.isfile(target):
            self.path = "/index.html"
        return super().do_GET()


if __name__ == "__main__":
    with socketserver.TCPServer(("", PORT), SPAHandler) as httpd:
        print(f"Dev server (SPA fallback) http://localhost:{PORT}  ->  {ROOT}")
        print("URL dep vd: http://localhost:%d/wav-to-mp3  (Ctrl+C de dung)" % PORT)
        httpd.serve_forever()
