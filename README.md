# RustConvert — Website Convert Đa Năng Online

Công cụ chuyển đổi file trực tuyến **miễn phí, không cần đăng nhập**, hoạt động
theo mô hình **Programmatic SEO**: mỗi cặp chuyển đổi là một trang riêng
(`/wav-to-mp3`, `/docx-to-pdf`, `/png-to-app-icon`...) nhưng dùng chung một
codebase động duy nhất.

```
RustConvert/
├── backend/              # API Rust + Axum
│   ├── Cargo.toml
│   └── src/main.rs
├── frontend/             # HTML tĩnh (deploy Cloudflare Pages / Netlify)
│   ├── index.html
│   └── _redirects        # rewrite /from-to-to -> index.html
├── scripts/
│   └── generate_sitemap.py
└── README.md
```

## Các cặp chuyển đổi được hỗ trợ

| Nhóm | Từ | Sang | Công cụ |
|------|----|------|---------|
| 1 | wav, flac, m4a, mp4, mov, mkv | mp3 | FFmpeg |
| 2 | docx, xlsx, pptx | pdf | LibreOffice |
| 3 | pdf | jpg, png (nén .zip) | pdftoppm (poppler) |
| 4 | png, jpg | app-icon (bộ iOS+Android .zip) | thư viện `image` của Rust |

## 1. Backend (Rust)

### Yêu cầu công cụ hệ thống
Backend gọi các CLI sau — cần cài sẵn trên server:

- **FFmpeg** — nhóm audio/video
- **LibreOffice** (`soffice`) — nhóm tài liệu
- **poppler-utils** (`pdftoppm`) — nhóm PDF→ảnh

```bash
# Ubuntu/Debian
sudo apt update && sudo apt install -y ffmpeg libreoffice poppler-utils

# macOS
brew install ffmpeg libreoffice poppler
```

### Chạy
```bash
cd backend
cargo run --release
# API chạy tại http://localhost:3000
```

### Biến môi trường
| Biến | Mặc định | Ý nghĩa |
|------|----------|---------|
| `PORT` | `3000` | Cổng lắng nghe |
| `ALLOWED_ORIGINS` | `http://localhost:8080,http://127.0.0.1:8080` | Danh sách origin được CORS chấp nhận (phân tách bằng dấu phẩy) |

```bash
# Ví dụ khi deploy production
ALLOWED_ORIGINS="https://yourdomain.com" PORT=8000 cargo run --release
```

### Bảo mật đã triển khai
- Giới hạn Content-Length **50MB** (`DefaultBodyLimit`).
- **CORS nghiêm ngặt** theo whitelist origin.
- Mọi file input được đổi tên thành **UUID ngẫu nhiên**.
- `from`/`to` đi qua **danh sách trắng** (`classify`) → không thể command injection.
- Lệnh hệ thống chạy qua `args` (không qua shell).
- **Xóa sạch file tạm tự động**: mỗi request dùng một `TempDir` tự hủy khi
  handler kết thúc (kể cả khi lỗi) nhờ cơ chế `Drop`.

## 2. Frontend (HTML tĩnh)

Chỉ là một file `index.html` + `_redirects`. Không cần build.

### Chạy thử local
Có 2 cách:

```bash
# Cách A (khuyến nghị): dev server có SPA-fallback -> URL đẹp /wav-to-mp3 chạy được như production
python scripts/dev_server.py 8080
# Mở http://localhost:8080/            (trang gốc: danh sách công cụ)
#     http://localhost:8080/wav-to-mp3 (URL đẹp)

# Cách B: server tĩnh thường -> CHỈ dùng được dạng ?from=..&to=..
cd frontend && python -m http.server 8080
# Mở http://localhost:8080/?from=wav&to=mp3   (dạng /wav-to-mp3 sẽ 404 vì không có rewrite)
```

> Trang gốc (không có `from`/`to`) hiển thị lưới các công cụ để bấm chọn — không còn báo lỗi "chưa xác định cặp chuyển đổi".

> ⚠️ Trước khi deploy, sửa hằng `API_BASE` trong `index.html` trỏ về domain API thật.

### Deploy
- **Cloudflare Pages / Netlify**: kéo thả thư mục `frontend/`. File `_redirects`
  giúp URL đẹp `/wav-to-mp3` hoạt động (nếu không, dùng `/?from=wav&to=mp3`).

### Quản lý lượt dùng (No-Login)
`LocalStorage` giới hạn **5 lượt/ngày/thiết bị**, tự reset khi sang ngày mới.
Hết lượt → nút Convert bị khóa kèm thông báo thân thiện. (Đây là chặn phía
client cho UX; nếu cần chống lạm dụng thật sự nên thêm rate-limit theo IP ở backend.)

## 3. Sitemap

```bash
cd scripts
python generate_sitemap.py     # sinh sitemap.xml
```

Sửa `DOMAIN` và `USE_PRETTY_URL` trong script cho khớp cách deploy, rồi copy
`sitemap.xml` vào thư mục `frontend/` và khai báo với Google Search Console.
