#!/usr/bin/env python3
"""
Build site tĩnh cho RustConvert theo mô hình Programmatic SEO (N-page).

Từ 2 template (frontend/template.html + frontend/template_home.html), script này
sinh ra thư mục `dist/` gồm:
  - dist/index.html                -> trang chủ (hub liệt kê tất cả công cụ)
  - dist/wav-to-mp3/index.html     -> mỗi cặp chuyển đổi 1 page thật, SEO nướng sẵn
  - dist/docx-to-pdf/index.html
  - ...
  - dist/sitemap.xml               -> sitemap chuẩn Google

Mỗi page có sẵn <title>/<meta>/<h1> + from/to trong HTML nên KHÔNG cần rewrite,
KHÔNG cần JS đọc URL. `dist/` deploy thẳng lên Cloudflare Pages/Netlify, hoặc test
local bằng `python -m http.server` (không cần dev server đặc biệt).

Chạy:
    python scripts/build_site.py
    python scripts/build_site.py --domain https://mysite.com --api https://api.mysite.com
"""

import argparse
import html
import json
import os
import shutil
from datetime import date

# ---------------------------------------------------------------------------
# Danh sách định dạng & cặp hợp lệ (khớp CHÍNH XÁC với backend main.rs)
# ---------------------------------------------------------------------------
AUDIO_VIDEO = ["wav", "flac", "m4a", "mp4", "mov", "mkv"]  # -> mp3
DOCS = ["docx", "xlsx", "pptx"]                            # -> pdf
IMAGES = ["png", "jpg"]                                    # -> app-icon
PDF_TARGETS = ["jpg", "png"]                               # pdf -> ảnh

# Nhãn hiển thị đẹp cho từng định dạng.
LABELS = {
    "wav": "WAV", "flac": "FLAC", "m4a": "M4A", "mp4": "MP4", "mov": "MOV", "mkv": "MKV",
    "mp3": "MP3", "docx": "DOCX", "xlsx": "XLSX", "pptx": "PPTX", "pdf": "PDF",
    "png": "PNG", "jpg": "JPG", "app-icon": "Bộ App Icon",
}

HERE = os.path.dirname(__file__)
ROOT = os.path.abspath(os.path.join(HERE, ".."))
FRONTEND = os.path.join(ROOT, "frontend")
DIST = os.path.join(ROOT, "dist")


def build_pairs():
    """Kết hợp các định dạng thành những cặp (from, to) hợp lệ."""
    pairs = []
    pairs += [(f, "mp3") for f in AUDIO_VIDEO]      # Nhóm 1
    pairs += [(f, "pdf") for f in DOCS]             # Nhóm 2
    pairs += [("pdf", t) for t in PDF_TARGETS]      # Nhóm 3
    pairs += [(f, "app-icon") for f in IMAGES]      # Nhóm 4
    return pairs


def label(fmt):
    return LABELS.get(fmt, fmt.upper())


def slug(f, t):
    return f"{f}-to-{t}"


def out_name(f, t):
    """Tên file tải về mặc định cho cặp này."""
    if t == "app-icon":
        return "app-icons.zip"
    if f == "pdf":
        return "converted-images.zip"
    return f"converted.{t}"


def fill(template, mapping):
    """Thay {{KEY}} bằng giá trị tương ứng."""
    out = template
    for k, v in mapping.items():
        out = out.replace("{{" + k + "}}", v)
    return out


def related_links(f, t, pairs):
    """Các cặp liên quan: cùng đích (to) hoặc cùng nguồn (from), trừ chính nó.
    Giúp liên kết nội bộ theo ngữ cảnh — tốt cho Programmatic SEO."""
    rel = [(a, b) for (a, b) in pairs if (a, b) != (f, t) and (b == t or a == f)]
    chips = []
    for (a, b) in rel:
        chips.append(
            f'<a href="/{slug(a, b)}" class="text-sm px-3 py-1.5 rounded-lg '
            f'bg-white border border-slate-200 hover:border-indigo-400 hover:bg-indigo-50">'
            f'{label(a)} → {label(b)}</a>'
        )
    return "\n        ".join(chips)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--domain", default="http://localhost:8080",
                    help="Domain gốc cho canonical & sitemap (vd https://mysite.com)")
    ap.add_argument("--api", default="http://localhost:3000",
                    help="URL backend cho fetch (vd https://api.mysite.com)")
    args = ap.parse_args()
    domain = args.domain.rstrip("/")
    api = args.api.rstrip("/")

    with open(os.path.join(FRONTEND, "template.html"), encoding="utf-8") as fh:
        tpl_page = fh.read()
    with open(os.path.join(FRONTEND, "template_home.html"), encoding="utf-8") as fh:
        tpl_home = fh.read()

    # Dọn dist/ cũ.
    if os.path.isdir(DIST):
        shutil.rmtree(DIST)
    os.makedirs(DIST)

    pairs = build_pairs()
    today = date.today().isoformat()

    # --- Sinh từng page chuyển đổi ---
    for (f, t) in pairs:
        F, T = label(f), label(t)
        title = f"Chuyển đổi {F} sang {T} trực tuyến miễn phí"
        desc = (f"Công cụ trực tuyến giúp bạn chuyển đổi file {F} sang định dạng {T} "
                f"nhanh chóng, an toàn, bảo mật và hoàn toàn miễn phí.")
        canonical = f"{domain}/{slug(f, t)}"
        jsonld = json.dumps({
            "@context": "https://schema.org",
            "@type": "WebApplication",
            "name": title,
            "applicationCategory": "UtilitiesApplication",
            "operatingSystem": "Web",
            "offers": {"@type": "Offer", "price": "0"},
        }, ensure_ascii=False)

        page = fill(tpl_page, {
            "TITLE": html.escape(title, quote=True),
            "DESCRIPTION": html.escape(desc, quote=True),
            "H1": f"Chuyển đổi {F} sang {T}",
            "CANONICAL": canonical,
            "JSONLD": jsonld,
            "FROM": f, "TO": t,
            "FROM_LABEL": F, "TO_LABEL": T,
            "OUT_NAME": out_name(f, t),
            "API_BASE": api,
            "RELATED": related_links(f, t, pairs),
        })

        folder = os.path.join(DIST, slug(f, t))
        os.makedirs(folder)
        with open(os.path.join(folder, "index.html"), "w", encoding="utf-8") as fh:
            fh.write(page)

    # --- Trang chủ (hub) ---
    grid = "\n      ".join(
        f'<a href="/{slug(f, t)}" class="block rounded-xl border border-slate-200 '
        f'bg-white px-4 py-3 text-center hover:border-indigo-400 hover:bg-indigo-50 transition-colors">'
        f'<span class="font-semibold text-slate-800">{label(f)} → {label(t)}</span></a>'
        for (f, t) in pairs
    )
    home = fill(tpl_home, {
        "TITLE": "Convert Đa Năng Online — Chuyển đổi file miễn phí",
        "DESCRIPTION": "Bộ công cụ chuyển đổi file trực tuyến miễn phí: audio/video sang MP3, "
                       "tài liệu sang PDF, PDF sang ảnh, ảnh sang bộ App Icon.",
        "CANONICAL": f"{domain}/",
        "GRID": grid,
    })
    with open(os.path.join(DIST, "index.html"), "w", encoding="utf-8") as fh:
        fh.write(home)

    # --- Sitemap ---
    lines = ['<?xml version="1.0" encoding="UTF-8"?>',
             '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
             "  <url>",
             f"    <loc>{domain}/</loc>",
             f"    <lastmod>{today}</lastmod>",
             "    <changefreq>daily</changefreq>",
             "    <priority>1.0</priority>",
             "  </url>"]
    for (f, t) in pairs:
        lines += ["  <url>",
                  f"    <loc>{domain}/{slug(f, t)}</loc>",
                  f"    <lastmod>{today}</lastmod>",
                  "    <changefreq>daily</changefreq>",
                  "    <priority>0.8</priority>",
                  "  </url>"]
    lines.append("</urlset>")
    with open(os.path.join(DIST, "sitemap.xml"), "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")

    print(f"[OK] Build xong {len(pairs)} page + trang chu + sitemap -> {DIST}")
    print(f"     domain={domain} | api={api}")
    print("     Test local: python -m http.server 8080 --directory dist")


if __name__ == "__main__":
    main()
