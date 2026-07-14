//! RustConvert — Backend API cho website Convert Đa Năng Online
//!
//! Kiến trúc: MỘT endpoint động duy nhất `/api/convert` xử lý tất cả cặp
//! chuyển đổi (Programmatic SEO dùng chung 1 backend). Cặp `from`/`to` được
//! phân loại bằng `match` rồi gọi công cụ hệ thống tương ứng.
//!
//! Bảo mật:
//!   - Giới hạn Content-Length tối đa 50MB (DefaultBodyLimit).
//!   - CORS nghiêm ngặt (chỉ cho phép origin khai báo trong ALLOWED_ORIGINS).
//!   - Mọi file được đổi tên thành UUID ngẫu nhiên (chống path traversal / đoán tên).
//!   - `from`/`to` chỉ nhận giá trị trong danh sách trắng -> không có nguy cơ
//!     command injection (không dùng shell, chỉ truyền args).
//!   - Dùng `tempfile::TempDir`: toàn bộ file gốc + file tạm bị XÓA SẠCH tự động
//!     khi handler kết thúc (kể cả khi lỗi), nhờ cơ chế Drop của Rust.

use std::io::{Cursor, Write as _};
use std::path::{Path, PathBuf};

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart},
    http::{header, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use tempfile::TempDir;
use tower_http::cors::{AllowOrigin, CorsLayer};
use uuid::Uuid;

/// Giới hạn kích thước request: 50 MB.
const MAX_BYTES: usize = 50 * 1024 * 1024;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();

    let app = Router::new()
        .route("/", get(|| async { "RustConvert API OK" }))
        .route("/api/convert", post(convert))
        .layer(cors_layer())
        // Giới hạn dung lượng toàn bộ request body (Content-Length) = 50MB.
        .layer(DefaultBodyLimit::max(MAX_BYTES));

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind failed");
    tracing::info!("🚀 RustConvert đang chạy tại http://{addr}");
    axum::serve(listener, app).await.expect("server error");
}

/// Cấu hình CORS nghiêm ngặt: chỉ nhận origin liệt kê trong biến môi trường
/// `ALLOWED_ORIGINS` (phân tách bằng dấu phẩy). Mặc định cho localhost dev.
fn cors_layer() -> CorsLayer {
    let raw = std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:8080,http://127.0.0.1:8080".to_string());
    let origins: Vec<HeaderValue> = raw
        .split(',')
        .filter_map(|o| o.trim().parse::<HeaderValue>().ok())
        .collect();

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE])
        // Cho phép JS phía client đọc tên file trả về để lưu đúng đuôi.
        .expose_headers([header::CONTENT_DISPOSITION])
}

// ---------------------------------------------------------------------------
// Phân loại cặp chuyển đổi
// ---------------------------------------------------------------------------

/// 4 nhóm chuyển đổi mà backend hỗ trợ.
enum Kind {
    AudioToMp3,   // Nhóm 1: FFmpeg
    DocToPdf,     // Nhóm 2: LibreOffice
    PdfToImage,   // Nhóm 3: pdftoppm + zip
    ImageToIcon,  // Nhóm 4: thư viện `image` + zip
}

/// Danh sách trắng: quyết định cặp (from, to) có hợp lệ không.
/// Đây cũng là lớp bảo mật chống command injection — mọi giá trị lạ bị từ chối.
fn classify(from: &str, to: &str) -> Option<Kind> {
    match (from, to) {
        ("wav" | "flac" | "m4a" | "mp4" | "mov" | "mkv", "mp3") => Some(Kind::AudioToMp3),
        ("docx" | "xlsx" | "pptx", "pdf") => Some(Kind::DocToPdf),
        ("pdf", "jpg" | "jpeg" | "png") => Some(Kind::PdfToImage),
        ("png" | "jpg" | "jpeg", "app-icon") => Some(Kind::ImageToIcon),
        _ => None,
    }
}

/// Kết quả sẵn sàng trả về client.
struct Output {
    bytes: Vec<u8>,
    filename: String,
    content_type: &'static str,
}

// ---------------------------------------------------------------------------
// Handler chính
// ---------------------------------------------------------------------------

async fn convert(mut multipart: Multipart) -> Result<Response, AppError> {
    // 1. Đọc các field trong multipart form.
    let mut from: Option<String> = None;
    let mut to: Option<String> = None;
    let mut data: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad(format!("Multipart lỗi: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "from" => from = Some(read_text(field).await?),
            "to" => to = Some(read_text(field).await?),
            "file" => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::bad(format!("Đọc file lỗi: {e}")))?;
                data = Some(bytes.to_vec());
            }
            _ => {} // bỏ qua field không dùng
        }
    }

    // 2. Validate đầu vào.
    let from = from.ok_or_else(|| AppError::bad("Thiếu tham số 'from'".into()))?.to_lowercase();
    let to = to.ok_or_else(|| AppError::bad("Thiếu tham số 'to'".into()))?.to_lowercase();
    let data = data.ok_or_else(|| AppError::bad("Thiếu file upload".into()))?;

    if data.is_empty() {
        return Err(AppError::bad("File rỗng".into()));
    }
    let kind = classify(&from, &to)
        .ok_or_else(|| AppError::bad(format!("Không hỗ trợ chuyển '{from}' sang '{to}'")))?;

    // 3. Tạo thư mục tạm tự hủy + file input mang tên UUID ngẫu nhiên.
    let tmp = TempDir::new().map_err(|e| AppError::internal(format!("Không tạo temp dir: {e}")))?;
    let id = Uuid::new_v4();
    let input_path = tmp.path().join(format!("{id}.{from}"));
    tokio::fs::write(&input_path, &data)
        .await
        .map_err(|e| AppError::internal(format!("Ghi file tạm lỗi: {e}")))?;

    // 4. Dispatch theo nhóm.
    let out = match kind {
        Kind::AudioToMp3 => audio_to_mp3(tmp.path(), &input_path, &id).await?,
        Kind::DocToPdf => doc_to_pdf(tmp.path(), &input_path, &id).await?,
        Kind::PdfToImage => pdf_to_image(tmp.path(), &input_path, &id, &to, &from).await?,
        Kind::ImageToIcon => image_to_icon(data, &from).await?,
    };

    // 5. Build response. `tmp` (TempDir) sẽ tự Drop ở cuối hàm -> XÓA SẠCH mọi
    //    file gốc/tạm. `out.bytes` đã copy vào body nên an toàn.
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, out.content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", out.filename),
        )
        .body(Body::from(out.bytes))
        .map_err(|e| AppError::internal(format!("Build response lỗi: {e}")))?;

    Ok(resp)
}

async fn read_text(field: axum::extract::multipart::Field<'_>) -> Result<String, AppError> {
    field
        .text()
        .await
        .map_err(|e| AppError::bad(format!("Đọc text field lỗi: {e}")))
}

// ---------------------------------------------------------------------------
// Nhóm 1: Audio/Video -> MP3 (FFmpeg)
// ---------------------------------------------------------------------------

async fn audio_to_mp3(dir: &Path, input: &Path, id: &Uuid) -> Result<Output, AppError> {
    let output = dir.join(format!("{id}.mp3"));
    // ffmpeg -y -i <input> -vn -acodec libmp3lame -b:a 192k <output.mp3>
    run(
        "ffmpeg",
        &[
            "-y",
            "-i",
            path(input),
            "-vn",
            "-acodec",
            "libmp3lame",
            "-b:a",
            "192k",
            path(&output),
        ],
    )
    .await?;

    let bytes = read_file(&output).await?;
    Ok(Output {
        bytes,
        filename: "converted.mp3".into(),
        content_type: "audio/mpeg",
    })
}

// ---------------------------------------------------------------------------
// Nhóm 2: DOCX/XLSX/PPTX -> PDF (LibreOffice)
// ---------------------------------------------------------------------------

async fn doc_to_pdf(dir: &Path, input: &Path, id: &Uuid) -> Result<Output, AppError> {
    // Mỗi request dùng 1 profile riêng để chạy song song an toàn.
    // Dựng file URI đúng cho cả Windows (C:\x -> file:///C:/x) lẫn Linux (/tmp/x -> file:///tmp/x).
    let profile = dir.join("loprofile");
    let p = profile.to_string_lossy().replace('\\', "/");
    let profile_uri = if p.starts_with('/') {
        format!("-env:UserInstallation=file://{p}")
    } else {
        format!("-env:UserInstallation=file:///{p}")
    };

    // soffice --headless --convert-to pdf --outdir <dir> <input>
    run(
        "soffice",
        &[
            profile_uri.as_str(),
            "--headless",
            "--convert-to",
            "pdf",
            "--outdir",
            path(dir),
            path(input),
        ],
    )
    .await?;

    // LibreOffice xuất ra <dir>/<id>.pdf (cùng stem với input).
    let output = dir.join(format!("{id}.pdf"));
    let bytes = read_file(&output).await?;
    Ok(Output {
        bytes,
        filename: "converted.pdf".into(),
        content_type: "application/pdf",
    })
}

// ---------------------------------------------------------------------------
// Nhóm 3: PDF -> JPG/PNG (pdftoppm) rồi nén .zip
// ---------------------------------------------------------------------------

async fn pdf_to_image(
    dir: &Path,
    input: &Path,
    id: &Uuid,
    to: &str,
    _from: &str,
) -> Result<Output, AppError> {
    // pdftoppm dùng cờ -jpeg cho jpg, -png cho png.
    let (flag, ext) = match to {
        "jpg" | "jpeg" => ("-jpeg", "jpg"),
        "png" => ("-png", "png"),
        _ => return Err(AppError::bad("Định dạng ảnh không hợp lệ".into())),
    };
    let prefix = dir.join(id.to_string());

    // pdftoppm -<flag> -r 150 <input.pdf> <prefix>  => sinh ra <prefix>-1.jpg, ...
    run("pdftoppm", &[flag, "-r", "150", path(input), path(&prefix)]).await?;

    // Gom tất cả ảnh <id>-*.ext, sort tên rồi nén thành zip (làm trong spawn_blocking).
    let dir = dir.to_path_buf();
    let id = *id;
    let ext = ext.to_string();
    let bytes = tokio::task::spawn_blocking(move || zip_images(&dir, &id, &ext))
        .await
        .map_err(|e| AppError::internal(format!("Task lỗi: {e}")))??;

    Ok(Output {
        bytes,
        filename: "converted-images.zip".into(),
        content_type: "application/zip",
    })
}

/// Đọc mọi ảnh khớp `<id>-*.<ext>` trong `dir`, sắp xếp theo tên, nén vào zip.
fn zip_images(dir: &Path, id: &Uuid, ext: &str) -> Result<Vec<u8>, AppError> {
    let id_str = id.to_string();
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| AppError::internal(format!("Đọc thư mục lỗi: {e}")))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.starts_with(&id_str) && name.ends_with(&format!(".{ext}"))
        })
        .collect();
    files.sort();

    if files.is_empty() {
        return Err(AppError::internal("Không sinh được trang ảnh nào".into()));
    }

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for (i, f) in files.iter().enumerate() {
        let content = std::fs::read(f).map_err(|e| AppError::internal(e.to_string()))?;
        zip.start_file(format!("page-{}.{ext}", i + 1), opts)
            .map_err(|e| AppError::internal(e.to_string()))?;
        zip.write_all(&content).map_err(|e| AppError::internal(e.to_string()))?;
    }
    let cursor = zip.finish().map_err(|e| AppError::internal(e.to_string()))?;
    Ok(cursor.into_inner())
}

// ---------------------------------------------------------------------------
// Nhóm 4: PNG/JPG -> Bộ App Icon (.zip) — dùng thư viện `image` của Rust
// ---------------------------------------------------------------------------

async fn image_to_icon(data: Vec<u8>, _from: &str) -> Result<Output, AppError> {
    // Resize là tác vụ CPU thuần -> chạy trong spawn_blocking để không chặn runtime.
    let bytes = tokio::task::spawn_blocking(move || build_app_icons(&data))
        .await
        .map_err(|e| AppError::internal(format!("Task lỗi: {e}")))??;

    Ok(Output {
        bytes,
        filename: "app-icons.zip".into(),
        content_type: "application/zip",
    })
}

fn build_app_icons(data: &[u8]) -> Result<Vec<u8>, AppError> {
    let img = image::load_from_memory(data)
        .map_err(|e| AppError::bad(format!("Không đọc được ảnh: {e}")))?;

    // Bộ kích thước tiêu chuẩn iOS + Android.
    let icons: &[(&str, u32)] = &[
        // iOS AppIcon.appiconset
        ("ios/Icon-20.png", 20),
        ("ios/Icon-20@2x.png", 40),
        ("ios/Icon-20@3x.png", 60),
        ("ios/Icon-29.png", 29),
        ("ios/Icon-29@2x.png", 58),
        ("ios/Icon-29@3x.png", 87),
        ("ios/Icon-40.png", 40),
        ("ios/Icon-40@2x.png", 80),
        ("ios/Icon-40@3x.png", 120),
        ("ios/Icon-60@2x.png", 120),
        ("ios/Icon-60@3x.png", 180),
        ("ios/Icon-76.png", 76),
        ("ios/Icon-76@2x.png", 152),
        ("ios/Icon-83.5@2x.png", 167),
        ("ios/Icon-1024.png", 1024),
        // Android mipmap
        ("android/mipmap-mdpi/ic_launcher.png", 48),
        ("android/mipmap-hdpi/ic_launcher.png", 72),
        ("android/mipmap-xhdpi/ic_launcher.png", 96),
        ("android/mipmap-xxhdpi/ic_launcher.png", 144),
        ("android/mipmap-xxxhdpi/ic_launcher.png", 192),
        ("android/playstore-512.png", 512),
    ];

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for (name, size) in icons {
        let resized = img.resize_exact(*size, *size, image::imageops::FilterType::Lanczos3);
        let mut buf = Cursor::new(Vec::<u8>::new());
        resized
            .write_to(&mut buf, image::ImageFormat::Png)
            .map_err(|e| AppError::internal(format!("Encode PNG lỗi: {e}")))?;
        zip.start_file(*name, opts)
            .map_err(|e| AppError::internal(e.to_string()))?;
        zip.write_all(&buf.into_inner())
            .map_err(|e| AppError::internal(e.to_string()))?;
    }

    let cursor = zip.finish().map_err(|e| AppError::internal(e.to_string()))?;
    Ok(cursor.into_inner())
}

// ---------------------------------------------------------------------------
// Tiện ích
// ---------------------------------------------------------------------------

/// Chạy một lệnh hệ thống với args (KHÔNG qua shell -> an toàn injection).
async fn run(cmd: &str, args: &[&str]) -> Result<(), AppError> {
    let output = tokio::process::Command::new(cmd)
        .args(args)
        .output()
        .await
        .map_err(|e| {
            AppError::internal(format!("Không chạy được '{cmd}' (đã cài chưa?): {e}"))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!("Lệnh '{cmd}' thất bại: {stderr}");
        return Err(AppError::internal(format!("Chuyển đổi thất bại ({cmd})")));
    }
    Ok(())
}

/// Đọc file kết quả vào bộ nhớ (file sẽ bị xóa cùng TempDir sau đó).
async fn read_file(p: &Path) -> Result<Vec<u8>, AppError> {
    tokio::fs::read(p)
        .await
        .map_err(|e| AppError::internal(format!("Không đọc được kết quả: {e}")))
}

/// Chuyển &Path thành &str để truyền vào args (mọi path đều do server tạo).
fn path(p: &Path) -> &str {
    p.to_str().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Xử lý lỗi
// ---------------------------------------------------------------------------

struct AppError(StatusCode, String);

impl AppError {
    fn bad(msg: String) -> Self {
        AppError(StatusCode::BAD_REQUEST, msg)
    }
    fn internal(msg: String) -> Self {
        AppError(StatusCode::INTERNAL_SERVER_ERROR, msg)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}
