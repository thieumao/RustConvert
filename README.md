# RustConvert — Website Convert Đa Năng Online

Công cụ chuyển đổi file trực tuyến **miễn phí, không cần đăng nhập**, hoạt động
theo mô hình **Programmatic SEO**: mỗi cặp chuyển đổi là **một trang HTML tĩnh
riêng** (`/wav-to-mp3`, `/docx-to-pdf`, `/png-to-app-icon`...) được sinh sẵn từ
một template chung — thẻ SEO nướng thẳng vào HTML nên **không cần rewrite, không
cần JS đọc URL**.

```
RustConvert/
├── backend/              # API Rust + Axum
│   ├── Cargo.toml
│   └── src/main.rs
├── frontend/             # Template nguồn (KHÔNG deploy trực tiếp)
│   ├── template.html         # template trang convert
│   └── template_home.html    # template trang chủ (hub)
├── scripts/
│   └── build_site.py     # sinh dist/ (N page + trang chủ + sitemap)
├── dist/                 # KẾT QUẢ build -> deploy thư mục này (gitignored)
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

## 2. Frontend (N-page tĩnh, build từ template)

Không sửa trực tiếp file deploy — sửa **template** rồi **build** ra `dist/`:
- Giao diện trang convert: `frontend/template.html`
- Giao diện trang chủ: `frontend/template_home.html`
- Danh sách cặp + nhãn hiển thị: `scripts/build_site.py` (biến `LABELS`, các hàm `build_pairs`)

### Build
```bash
# Mặc định: domain=localhost:8080, api=localhost:3000 (để test local)
python scripts/build_site.py

# Khi deploy thật: truyền domain + URL backend
python scripts/build_site.py --domain https://mysite.com --api https://api.mysite.com
```
Kết quả nằm trong `dist/`: `dist/index.html` (hub) + `dist/wav-to-mp3/index.html` …
+ `dist/sitemap.xml`. Mỗi page đã nướng sẵn `<title>`/`<meta>`/`<h1>` + `from`/`to`
+ link chéo tới các công cụ liên quan (nội bộ, tốt cho SEO).

### Chạy thử local
```bash
python -m http.server 8080 --directory dist
# http://localhost:8080/              -> trang chủ (hub)
# http://localhost:8080/wav-to-mp3/   -> file thật, không cần rewrite
```

### Deploy
- **Cloudflare Pages / Netlify**: build local rồi kéo thả thư mục **`dist/`**.
  Không cần `_redirects` vì `/wav-to-mp3` đã là file thật. (Hoặc đặt build
  command = `python scripts/build_site.py --domain ... --api ...`, output = `dist`.)

### Quản lý lượt dùng (No-Login)
`LocalStorage` giới hạn **5 lượt/ngày/thiết bị**, tự reset khi sang ngày mới.
Hết lượt → nút Convert bị khóa kèm thông báo thân thiện. (Đây là chặn phía
client cho UX; nếu cần chống lạm dụng thật sự nên thêm rate-limit theo IP ở backend.)

## 3. Sitemap

Sitemap được sinh **tự động cùng lúc với build** (`dist/sitemap.xml`) — không cần
script riêng. Nhớ truyền đúng `--domain` khi build để `<loc>` đúng domain, rồi khai
báo `https://mysite.com/sitemap.xml` với Google Search Console.

## Thêm một cặp/định dạng mới

1. Thêm nhánh xử lý trong `classify()` ở `backend/src/main.rs` (+ hàm convert nếu là nhóm mới).
2. Thêm định dạng vào danh sách + `LABELS` trong `scripts/build_site.py`.
3. Chạy lại `python scripts/build_site.py` và deploy `dist/`.
