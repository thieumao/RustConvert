#!/usr/bin/env python3
"""
Tạo sitemap.xml cho website Convert Đa Năng Online (Programmatic SEO).

Script tự sinh TẤT CẢ cặp chuyển đổi hợp lệ mà backend hỗ trợ, rồi xuất ra
file sitemap.xml chuẩn Google. Chạy trên máy cá nhân:

    python generate_sitemap.py

Kết quả: file `sitemap.xml` (đặt cùng thư mục với index.html khi deploy).
"""

from datetime import date

# ---------------------------------------------------------------------------
# CẤU HÌNH
# ---------------------------------------------------------------------------
DOMAIN = "https://yourdomain.com"        # ⚠️ Đổi thành domain thật của bạn
OUTPUT = "sitemap.xml"

# True  -> URL đẹp:   https://yourdomain.com/wav-to-mp3      (cần _redirects)
# False -> URL param: https://yourdomain.com/?from=wav&to=mp3 (host tĩnh thuần)
USE_PRETTY_URL = True

# ---------------------------------------------------------------------------
# DANH SÁCH ĐỊNH DẠNG (khớp chính xác với backend main.rs)
# ---------------------------------------------------------------------------
AUDIO_VIDEO = ["wav", "flac", "m4a", "mp4", "mov", "mkv"]  # -> mp3
DOCS = ["docx", "xlsx", "pptx"]                            # -> pdf
IMAGES = ["png", "jpg"]                                    # -> app-icon
PDF_TARGETS = ["jpg", "png"]                               # pdf -> ảnh


def build_pairs():
    """Kết hợp các định dạng thành những cặp (from, to) hợp lệ."""
    pairs = []
    pairs += [(f, "mp3") for f in AUDIO_VIDEO]      # Nhóm 1
    pairs += [(f, "pdf") for f in DOCS]             # Nhóm 2
    pairs += [("pdf", t) for t in PDF_TARGETS]      # Nhóm 3
    pairs += [(f, "app-icon") for f in IMAGES]      # Nhóm 4
    return pairs


def url_for(from_fmt, to_fmt):
    if USE_PRETTY_URL:
        return f"{DOMAIN}/{from_fmt}-to-{to_fmt}"
    return f"{DOMAIN}/?from={from_fmt}&to={to_fmt}"


def generate():
    pairs = build_pairs()
    today = date.today().isoformat()

    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
    ]

    # Trang chủ (ưu tiên cao nhất)
    lines += [
        "  <url>",
        f"    <loc>{DOMAIN}/</loc>",
        f"    <lastmod>{today}</lastmod>",
        "    <changefreq>daily</changefreq>",
        "    <priority>1.0</priority>",
        "  </url>",
    ]

    # Từng trang chuyển đổi
    for from_fmt, to_fmt in pairs:
        lines += [
            "  <url>",
            f"    <loc>{url_for(from_fmt, to_fmt)}</loc>",
            f"    <lastmod>{today}</lastmod>",
            "    <changefreq>daily</changefreq>",
            "    <priority>0.8</priority>",
            "  </url>",
        ]

    lines.append("</urlset>")

    with open(OUTPUT, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")

    # In thông báo ASCII-safe để không lỗi trên console Windows (cp1252).
    print(f"[OK] Da tao {OUTPUT} voi {len(pairs)} trang chuyen doi (+ trang chu).")
    print("     Cac cap:", ", ".join(f"{a}-to-{b}" for a, b in pairs))


if __name__ == "__main__":
    generate()
