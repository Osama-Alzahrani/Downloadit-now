use dirs::desktop_dir;
use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use std::io::Write;
use tokio::sync::Mutex;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tauri::Emitter;
use futures_util::StreamExt;

/// Helper macro to print and flush immediately
macro_rules! println_flush {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VideoInfo {
    pub title: String,
    pub thumbnail: Option<String>,
    pub duration: Option<f64>,
    pub filesize: Option<i64>,
    pub ext: Option<String>,
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub is_live: Option<bool>,
}

// Per-download state entry
struct DownloadEntry {
    pid: u32,
    dir: PathBuf,
    snapshot: std::collections::HashSet<PathBuf>,
}

// Global registry: download_id -> DownloadEntry
lazy_static::lazy_static! {
    static ref DOWNLOADS: Mutex<std::collections::HashMap<String, DownloadEntry>> =
        Mutex::new(std::collections::HashMap::new());
    // Live preview registry: download_id -> current ffmpeg preview PID
    static ref PREVIEWS: Mutex<std::collections::HashMap<String, u32>> =
        Mutex::new(std::collections::HashMap::new());
}

fn kill_pid(pid: u32) {
    #[cfg(target_os = "windows")]
    { let _ = std::process::Command::new("taskkill").args(&["/PID", &pid.to_string(), "/F", "/T"]).output(); }
    #[cfg(not(target_os = "windows"))]
    { let _ = std::process::Command::new("kill").args(&["-9", &pid.to_string()]).output(); }
}

/// Returns the directory where yt-dlp and ffmpeg are stored
pub fn get_tools_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Downloadit Now")
        .join("bin")
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct DepsStatus {
    pub ytdlp: bool,
    pub ffmpeg: bool,
}

pub fn check_deps_status() -> DepsStatus {
    let dir = get_tools_dir();
    DepsStatus {
        ytdlp: dir.join("yt-dlp.exe").exists(),
        ffmpeg: dir.join("ffmpeg.exe").exists(),
    }
}

async fn download_file_with_progress(
    client: &reqwest::Client,
    window: &tauri::Window,
    tool: &str,
    url: &str,
    dest: &PathBuf,
) -> Result<(), String> {
    let resp = client
        .get(url)
        .header("User-Agent", "Downloadit Now/1.0")
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();

    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("Cannot create file: {}", e))?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("Write error: {}", e))?;
        downloaded += chunk.len() as u64;
        if total > 0 {
            let progress = (downloaded as f64 / total as f64 * 100.0).min(100.0);
            let _ = window.emit("setup-progress", serde_json::json!({
                "tool": tool,
                "progress": progress,
                "status": format!("Downloading {}… {:.0}%", tool, progress)
            }));
        }
    }

    Ok(())
}

async fn get_ffmpeg_download_url(client: &reqwest::Client) -> Result<String, String> {
    let json: serde_json::Value = client
        .get("https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/latest")
        .header("User-Agent", "Downloadit Now/1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let assets = json["assets"].as_array()
        .ok_or("No assets in release")?;

    for asset in assets {
        let name = asset["name"].as_str().unwrap_or("");
        // Pick the non-shared win64 GPL zip
        if name.contains("win64-gpl") && name.ends_with(".zip") && !name.contains("shared") {
            return asset["browser_download_url"]
                .as_str()
                .map(|s| s.to_string())
                .ok_or("No download URL".to_string());
        }
    }

    Err("ffmpeg release asset not found".to_string())
}

async fn extract_ffmpeg_exe(zip_path: &PathBuf, dest_dir: &PathBuf) -> Result<(), String> {
    let zip_path = zip_path.clone();
    let dest_dir = dest_dir.clone();
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let file = std::fs::File::open(&zip_path).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let name = entry.name().replace('\\', "/").to_lowercase();
            if name.ends_with("bin/ffmpeg.exe") {
                let mut data = Vec::new();
                entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
                std::fs::write(dest_dir.join("ffmpeg.exe"), &data)
                    .map_err(|e| e.to_string())?;
                let _ = std::fs::remove_file(&zip_path);
                return Ok(());
            }
        }
        let _ = std::fs::remove_file(&zip_path);
        Err("ffmpeg.exe not found in archive".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn check_dependencies() -> DepsStatus {
    check_deps_status()
}

pub async fn download_dependencies(window: tauri::Window) -> Result<(), String> {
    let dir = get_tools_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create tools dir: {}", e))?;

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| e.to_string())?;

    // --- yt-dlp ---
    let ytdlp_path = dir.join("yt-dlp.exe");
    if !ytdlp_path.exists() {
        let _ = window.emit("setup-progress", serde_json::json!({
            "tool": "yt-dlp", "progress": 0, "status": "Starting yt-dlp download…"
        }));
        download_file_with_progress(
            &client, &window, "yt-dlp",
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe",
            &ytdlp_path,
        ).await?;
    }

    // --- ffmpeg ---
    let ffmpeg_path = dir.join("ffmpeg.exe");
    if !ffmpeg_path.exists() {
        let _ = window.emit("setup-progress", serde_json::json!({
            "tool": "ffmpeg", "progress": 0, "status": "Looking up latest ffmpeg release…"
        }));

        let ffmpeg_url = get_ffmpeg_download_url(&client).await?;
        println_flush!("📦 ffmpeg URL: {}", ffmpeg_url);

        let zip_path = std::env::temp_dir().join("downloadit_ffmpeg.zip");
        download_file_with_progress(&client, &window, "ffmpeg", &ffmpeg_url, &zip_path).await?;

        let _ = window.emit("setup-progress", serde_json::json!({
            "tool": "ffmpeg", "progress": 99, "status": "Extracting ffmpeg…"
        }));
        extract_ffmpeg_exe(&zip_path, &dir).await?;
    }

    let _ = window.emit("setup-progress", serde_json::json!({
        "tool": "done", "progress": 100, "status": "Setup complete!"
    }));
    Ok(())
}

/// Pause a specific download by ID (kill process, keep .part file)
pub async fn pause_download(download_id: String) -> Result<String, String> {
    println_flush!("⏸ pause_download [{}]", download_id);
    let entry = DOWNLOADS.lock().await.remove(&download_id);
    if let Some(e) = entry {
        kill_pid(e.pid);
        Ok("Download paused".to_string())
    } else {
        Err("No active download with that ID".to_string())
    }
}

/// Cancel a specific download by ID (kill process + delete .part file)
pub async fn cancel_download(download_id: String) -> Result<String, String> {
    println_flush!("🔴 cancel_download [{}]", download_id);
    let entry = DOWNLOADS.lock().await.remove(&download_id);
    if let Some(e) = entry {
        kill_pid(e.pid);
        // Delete the .part file created by this download
        if let Ok(entries) = std::fs::read_dir(&e.dir) {
            for ent in entries.flatten() {
                let path = ent.path();
                if path.to_string_lossy().ends_with(".part") && !e.snapshot.contains(&path) {
                    println_flush!("🗑️ Deleting: {}", path.display());
                    std::fs::remove_file(&path).ok();
                }
            }
        }
        Ok("Download cancelled".to_string())
    } else {
        Ok("Download cancelled (already stopped)".to_string())
    }
}

/// Minimal Base64 encoder — avoids adding a crate dependency.
fn encode_base64(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// Returns the path to the managed ffmpeg binary, falling back to PATH.
fn get_ffmpeg_command() -> String {
    #[cfg(target_os = "windows")]
    let exe = get_tools_dir().join("ffmpeg.exe");
    #[cfg(not(target_os = "windows"))]
    let exe = get_tools_dir().join("ffmpeg");
    if exe.exists() { exe.to_string_lossy().to_string() } else { "ffmpeg".to_string() }
}

// ─── Live preview — local MJPEG HTTP server ──────────────────────────────────
//
// Binds a TCP listener on a random port, accepts one browser connection, and
// serves MJPEG via a persistent ffmpeg process.  The frontend just sets:
//   <img :src="`http://127.0.0.1:${port}/stream`">
// The browser renders MJPEG natively — no IPC per frame, no base64, ~15 fps.

async fn serve_mjpeg(
    stream: tokio::net::TcpStream,
    ffmpeg: String,
    path: PathBuf,
    download_id: String,
) {
    let (mut rx, mut tx) = stream.into_split();

    // Drain the HTTP request (content doesn't matter)
    let mut req = [0u8; 2048];
    let _ = rx.read(&mut req).await;

    // Boundary "ffmpeg" matches mpjpeg muxer default
    let hdr = b"HTTP/1.1 200 OK\r\n\
                Content-Type: multipart/x-mixed-replace;boundary=ffmpeg\r\n\
                Cache-Control: no-cache\r\n\
                Access-Control-Allow-Origin: *\r\n\
                Connection: keep-alive\r\n\r\n";
    if tx.write_all(hdr).await.is_err() { return; }

    let mut buf = vec![0u8; 65_536];

    loop {
        if !PREVIEWS.lock().await.contains_key(&download_id) { break; }

        let mut cmd = Command::new(&ffmpeg);
        cmd.args(&["-y", "-sseof", "-60", "-re", "-i"]).arg(&path)
           .args(&["-an", "-vf", "fps=15,scale=640:-1", "-f", "mpjpeg", "pipe:1"])
           .stdout(Stdio::piped()).stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(_) => { tokio::time::sleep(std::time::Duration::from_secs(2)).await; continue; }
        };

        if let Some(pid) = child.id() {
            PREVIEWS.lock().await.insert(download_id.clone(), pid);
        }

        let mut client_gone = false;
        if let Some(mut ffout) = child.stdout.take() {
            loop {
                match ffout.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => if tx.write_all(&buf[..n]).await.is_err() {
                        client_gone = true;
                        break;
                    },
                }
            }
        }
        let _ = child.wait().await;
        if client_gone { break; }
        if !PREVIEWS.lock().await.contains_key(&download_id) { break; }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
}

pub async fn start_live_preview(download_id: String) -> Result<u16, String> {
    stop_live_preview(download_id.clone()).await?;

    let dir_and_snapshot = {
        let downloads = DOWNLOADS.lock().await;
        downloads.get(&download_id).map(|e| (e.dir.clone(), e.snapshot.clone()))
    };
    let Some((dir, snapshot)) = dir_and_snapshot else {
        return Err("No active recording".to_string());
    };

    let rec_path = std::fs::read_dir(&dir).ok()
        .and_then(|entries| {
            entries.flatten()
                .map(|ent| ent.path())
                .filter(|p| !snapshot.contains(p))
                .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
        })
        .ok_or_else(|| "Recording file not found".to_string())?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await.map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();

    // u32::MAX = server running but no ffmpeg PID yet (prevent accidental kill)
    PREVIEWS.lock().await.insert(download_id.clone(), u32::MAX);

    let id = download_id.clone();
    let ffmpeg = get_ffmpeg_command();
    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            if PREVIEWS.lock().await.contains_key(&id) {
                serve_mjpeg(stream, ffmpeg, rec_path, id).await;
            }
        }
    });

    Ok(port)
}

pub async fn stop_live_preview(download_id: String) -> Result<(), String> {
    let pid = PREVIEWS.lock().await.remove(&download_id);
    if let Some(p) = pid { if p != u32::MAX { kill_pid(p); } }
    Ok(())
}

/// Stop a live stream recording:
///   1. Kill yt-dlp (file stays as .part or incomplete container)
///   2. Remux with `ffmpeg -c copy` to fix container headers and strip corrupt tail
///   3. If remux fails, at least rename .part → final extension
/// Returns the path of the usable recorded file.
pub async fn stop_recording(download_id: String) -> Result<Option<String>, String> {
    println_flush!("⏹ stop_recording [{}]", download_id);
    let entry = DOWNLOADS.lock().await.remove(&download_id);
    let Some(e) = entry else { return Ok(None); };

    kill_pid(e.pid);
    // Give yt-dlp time to flush its last write
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Find the newest file created since recording started (includes .part files)
    let raw_path = match std::fs::read_dir(&e.dir)
        .ok()
        .and_then(|entries| {
            entries.flatten()
                .map(|ent| ent.path())
                .filter(|p| !e.snapshot.contains(p))
                .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
        }) {
        Some(p) => p,
        None => {
            println_flush!("⏹ No recorded file found");
            return Ok(None);
        }
    };

    println_flush!("⏹ Raw file: {}", raw_path.display());

    let is_part = raw_path.to_string_lossy().ends_with(".part");

    // Determine the output path:
    //   .part file  → strip .part  (e.g. video.mp4.part → video.mp4)
    //   other file  → _rec_ prefix (avoid reading and writing the same file)
    let final_path = if is_part {
        raw_path.with_extension("") // removes .part, keeps inner extension
    } else {
        let name = raw_path.file_name().unwrap_or_default().to_string_lossy();
        raw_path.with_file_name(format!("_rec_{}", name))
    };

    // Remux via ffmpeg -c copy to repair container headers and drop corrupt tail
    let ffmpeg = get_ffmpeg_command();
    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-y")
       .arg("-i").arg(&raw_path)
       .arg("-c").arg("copy")
       .arg(&final_path);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    match cmd.status().await {
        Ok(s) if s.success() => {
            println_flush!("✅ Remux OK → {}", final_path.display());
            let _ = std::fs::remove_file(&raw_path);
            if !is_part {
                // Replace original with remuxed version
                let _ = std::fs::rename(&final_path, &raw_path);
                return Ok(Some(raw_path.to_string_lossy().to_string()));
            }
            return Ok(Some(final_path.to_string_lossy().to_string()));
        }
        err => {
            println_flush!("⚠️ ffmpeg remux failed ({:?}), falling back", err);
            let _ = std::fs::remove_file(&final_path);
        }
    }

    // Fallback: rename .part → final extension (no container fix, but at least openable)
    if is_part {
        let renamed = raw_path.with_extension("");
        if std::fs::rename(&raw_path, &renamed).is_ok() {
            println_flush!("↪ Renamed: {}", renamed.display());
            return Ok(Some(renamed.to_string_lossy().to_string()));
        }
    }

    Ok(Some(raw_path.to_string_lossy().to_string()))
}

/// Grab the latest decodable video frame from an in-progress recording.
/// Returns a JPEG as a base64 data URL so the frontend can display it directly.
pub async fn capture_frame(download_id: String) -> Result<String, String> {
    // Snapshot dir/snapshot while holding the lock only briefly
    let dir_and_snapshot = {
        let downloads = DOWNLOADS.lock().await;
        downloads.get(&download_id).map(|e| (e.dir.clone(), e.snapshot.clone()))
    };
    let Some((dir, snapshot)) = dir_and_snapshot else {
        return Err("No active recording".to_string());
    };

    // Find the newest file created since the recording started
    let rec_path = std::fs::read_dir(&dir)
        .ok()
        .and_then(|entries| {
            entries.flatten()
                .map(|ent| ent.path())
                .filter(|p| !snapshot.contains(p))
                .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
        })
        .ok_or_else(|| "Recording file not found".to_string())?;

    let preview_path = std::env::temp_dir()
        .join(format!("dnit_preview_{}.jpg", download_id));
    let ffmpeg = get_ffmpeg_command();

    // First try: seek to 10 s before the current end of file ("latest" frame)
    let captured = {
        let mut cmd = Command::new(&ffmpeg);
        cmd.args(&["-y", "-sseof", "-10"])
           .arg("-i").arg(&rec_path)
           .args(&["-an", "-vframes", "1", "-vf", "scale=640:-1"])
           .arg(&preview_path)
           .stdout(Stdio::null())
           .stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);
        matches!(cmd.status().await, Ok(s) if s.success())
            && preview_path.metadata().map(|m| m.len() > 0).unwrap_or(false)
    };

    // Fallback: grab the first decodable frame (no seek)
    if !captured {
        let mut cmd = Command::new(&ffmpeg);
        cmd.args(&["-y"])
           .arg("-i").arg(&rec_path)
           .args(&["-an", "-vframes", "1", "-vf", "scale=640:-1"])
           .arg(&preview_path)
           .stdout(Stdio::null())
           .stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);
        let _ = cmd.status().await;
    }

    let jpeg = std::fs::read(&preview_path)
        .map_err(|_| "Frame capture failed — recording may still be starting".to_string())?;
    let _ = std::fs::remove_file(&preview_path);
    if jpeg.is_empty() {
        return Err("Empty frame — try again in a moment".to_string());
    }
    Ok(format!("data:image/jpeg;base64,{}", encode_base64(&jpeg)))
}

/// Cross-platform resolution of the user's Desktop directory
/// Works on Windows, macOS, and Linux
fn get_desktop_dir() -> Result<PathBuf, String> {
    desktop_dir().ok_or_else(|| {
        "Unable to determine Desktop directory for your system".to_string()
    })
}

/// Get the path to yt-dlp executable
/// First checks the app's directory for a local copy (yt-dlp or ytd binary),
/// then falls back to system PATH
fn get_ytdlp_command() -> Result<String, String> {
    println_flush!("  🔍 get_ytdlp_command: Finding yt-dlp...");

    // Get the directory of the current executable
    let exe_path = env::current_exe().map_err(|e| {
        println_flush!("  ❌ Failed to get current exe: {}", e);
        format!("Failed to determine app directory: {}", e)
    })?;
    println_flush!("  📍 Current exe: {}", exe_path.display());

    let app_dir = exe_path.parent().ok_or_else(|| {
        println_flush!("  ❌ Failed to get parent directory");
        "Failed to get app directory".to_string()
    })?;
    println_flush!("  📂 App directory: {}", app_dir.display());

    // Check the managed tools directory first (downloaded by setup)
    let tools_dir = get_tools_dir();
    let managed_ytdlp = tools_dir.join("yt-dlp.exe");
    if managed_ytdlp.exists() {
        println_flush!("  ✅ Found managed yt-dlp in tools dir!");
        return Ok(managed_ytdlp.to_string_lossy().to_string());
    }

    // Try local executable names on Windows
    #[cfg(target_os = "windows")]
    {
        let local_ytdlp = app_dir.join("yt-dlp.exe");
        let local_ytd = app_dir.join("ytd.exe");

        println_flush!("  🔍 Checking Windows local paths...");
        println_flush!("    - {}", local_ytdlp.display());
        if local_ytdlp.exists() {
            println_flush!("  ✅ Found yt-dlp.exe locally!");
            return Ok(local_ytdlp.to_string_lossy().to_string());
        }
        println_flush!("    - {}", local_ytd.display());
        if local_ytd.exists() {
            println_flush!("  ✅ Found ytd.exe locally!");
            return Ok(local_ytd.to_string_lossy().to_string());
        }
    }

    // Try local executable names on Unix (macOS/Linux)
    #[cfg(not(target_os = "windows"))]
    {
        let local_ytdlp = app_dir.join("yt-dlp");
        let local_ytd = app_dir.join("ytd");

        println_flush!("  🔍 Checking Unix local paths...");
        println_flush!("    - {}", local_ytdlp.display());
        if local_ytdlp.exists() {
            println_flush!("  ✅ Found yt-dlp locally!");
            return Ok(local_ytdlp.to_string_lossy().to_string());
        }
        println_flush!("    - {}", local_ytd.display());
        if local_ytd.exists() {
            println_flush!("  ✅ Found ytd locally!");
            return Ok(local_ytd.to_string_lossy().to_string());
        }
    }

    // Fall back to system PATH
    println_flush!("  🔍 No local copy found, using system PATH");
    println_flush!("  ✅ Will use: yt-dlp (from system PATH)");
    Ok("yt-dlp".to_string())
}

/// Asynchronous function to download a video from a URL using yt-dlp
///
/// Downloads the video to the specified directory or Desktop if not provided.
/// Handles cross-platform path resolution and proper async process spawning.
///
/// # Arguments
/// * `url` - The video URL to download
/// * `format` - Optional format ID to use (e.g., "22", "best", etc.)
/// * `download_path` - Optional custom download directory path
///
/// # Returns
/// * `Ok(String)` - Success message with file location
/// * `Err(String)` - Error description if download fails
pub async fn download_video(url: String, format: Option<String>, download_path: Option<String>) -> Result<String, String> {
    println_flush!("\n========== DOWNLOAD_VIDEO DEBUG ==========");
    println_flush!("📌 START: download_video called");
    println_flush!("📝 URL: {}", url);
    if let Some(ref fmt) = format {
        println_flush!("📝 Format: {}", fmt);
    }

    // Validate URL is not empty
    if url.trim().is_empty() {
        println_flush!("❌ ERROR: URL is empty");
        return Err("URL cannot be empty".to_string());
    }

    // Resolve download directory
    println_flush!("🔍 Resolving download directory...");
    let target_path = if let Some(path) = download_path {
        println_flush!("📝 Using custom path: {}", path);
        PathBuf::from(path)
    } else {
        println_flush!("📝 Using Desktop directory");
        get_desktop_dir()?
    };
    println_flush!("✅ Download path: {}", target_path.display());

    // Ensure the directory exists
    if !target_path.exists() {
        println_flush!("❌ ERROR: Download directory does not exist");
        return Err(format!(
            "Download directory does not exist: {}",
            target_path.display()
        ));
    }

    // Get yt-dlp executable path (local or system)
    println_flush!("🔍 Finding yt-dlp executable...");
    let ytdlp_path = get_ytdlp_command()?;
    println_flush!("✅ yt-dlp path: {}", ytdlp_path);

    // Spawn yt-dlp process asynchronously
    println_flush!("🚀 Spawning yt-dlp process...");
    let mut cmd = Command::new(&ytdlp_path);
    cmd.arg("--quiet")
        .arg("--no-warnings")
        .arg("--no-update");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    // Add format if specified
    if let Some(fmt) = format {
        println_flush!("  Command: {} --quiet --no-warnings --no-update -f \"{}\" -o \"%(title)s.%(ext)s\" \"{}\"", ytdlp_path, fmt, url);
        cmd.arg("-f").arg(fmt);
    } else {
        println_flush!("  Command: {} --quiet --no-warnings --no-update -o \"%(title)s.%(ext)s\" \"{}\"", ytdlp_path, url);
    }

    cmd.arg("-o")
        .arg("%(title)s.%(ext)s")
        .arg(&url)
        .current_dir(&target_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        println_flush!("❌ ERROR: Failed to spawn yt-dlp: {}", e);
        format!(
            "yt-dlp not found or failed to start: {}. \
             Place 'yt-dlp.exe' or 'ytd.exe' in the app directory, \
             or ensure yt-dlp is in your system PATH.",
            e
        )
    })?;
    println_flush!("✅ Process spawned successfully");

    // Capture and read stderr for error messages
    println_flush!("📋 Capturing stderr...");
    let stderr = child
        .stderr
        .take()
        .ok_or("Failed to capture stderr from yt-dlp")?;
    let mut stderr_reader = BufReader::new(stderr);
    let mut error_buffer = String::new();

    // Wait for the process to complete
    println_flush!("⏳ Waiting for download to complete...");
    let status = child.wait().await.map_err(|e| {
        println_flush!("❌ ERROR: Process interrupted: {}", e);
        format!("Download process was interrupted: {}", e)
    })?;
    println_flush!("✅ Process finished with status: {}", status);

    // Attempt to read error output
    let _ = stderr_reader
        .read_line(&mut error_buffer)
        .await
        .ok();

    // Return success or error based on exit status
    if status.success() {
        let success_msg = format!(
            "✓ Video downloaded successfully to: {}",
            target_path.display()
        );
        println_flush!("✅ SUCCESS: {}", success_msg);
        println_flush!("========== END ==========\n");
        Ok(success_msg)
    } else {
        let error_msg = if error_buffer.is_empty() {
            format!(
                "Download failed with exit code: {}",
                status.code().unwrap_or(-1)
            )
        } else {
            error_buffer.trim().to_string()
        };
        println_flush!("❌ ERROR: {}", error_msg);
        println_flush!("========== END ==========\n");
        Err(error_msg)
    }
}

/// Download video with progress streaming to frontend (fully async)
pub async fn download_video_with_window(
    window: tauri::Window,
    url: String,
    format: Option<String>,
    audio_only: bool,
    download_path: Option<String>,
    download_id: String,
) -> Result<String, String> {
    println_flush!("\n========== DOWNLOAD_VIDEO (STREAMING - ASYNC) ==========");
    println_flush!("📌 START [id={}]", download_id);
    println_flush!("📝 URL: {}", url);
    println_flush!("📝 Download Path: {:?}", download_path);
    println_flush!("📝 Format: {:?}", format);

    let _ = window.emit("download-progress", serde_json::json!({ "download_id": &download_id, "status": "Finding yt-dlp..." }));

    if url.trim().is_empty() {
        let _ = window.emit("download-error", serde_json::json!({ "download_id": &download_id, "message": "URL is empty" }));
        return Err("URL cannot be empty".to_string());
    }

    let target_path = if let Some(path) = download_path {
        PathBuf::from(path)
    } else {
        get_desktop_dir()?
    };

    if !target_path.exists() {
        return Err(format!("Download directory does not exist: {}", target_path.display()));
    }

    let ytdlp_path = get_ytdlp_command()?;
    let _ = window.emit("download-progress", serde_json::json!({ "download_id": &download_id, "status": "Starting download..." }));

    // Snapshot non-.part files so cancel can identify new .part files to delete
    let snapshot: std::collections::HashSet<PathBuf> = std::fs::read_dir(&target_path)
        .map(|entries| entries.flatten()
            .map(|e| e.path())
            .filter(|p| !p.to_string_lossy().ends_with(".part"))
            .collect())
        .unwrap_or_default();
    println_flush!("📸 Snapshotted {} non-.part files for [{}]", snapshot.len(), download_id);

    // Create async command
    let mut cmd = Command::new(&ytdlp_path);
    cmd.arg("--progress")
        .arg("--no-warnings")
        .arg("--no-update")
        .arg("--newline");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    if audio_only {
        cmd.arg("-f").arg("bestaudio/best")
           .arg("-x")
           .arg("--audio-format").arg("mp3");
    } else if let Some(fmt) = format {
        cmd.arg("-f").arg(fmt);
    }

    cmd.arg("-o")
        .arg("%(title)s.%(ext)s")
        .arg(&url)
        .current_dir(&target_path)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Spawn child process
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Failed to spawn: {}", e);
            let _ = window.emit("download-error", serde_json::json!({ "download_id": &download_id, "message": &err_msg }));
            return Err(err_msg);
        }
    };

    // Extract stdout before storing child
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = window.emit("download-error", serde_json::json!({ "download_id": &download_id, "message": "Failed to capture stdout" }));
            return Err("Failed to capture stdout".to_string());
        }
    };

    // Register in the per-download map (used by pause/cancel)
    {
        let pid = child.id().unwrap_or(0);
        println_flush!("📌 Child PID {} registered for [{}]", pid, download_id);
        DOWNLOADS.lock().await.insert(download_id.clone(), DownloadEntry {
            pid,
            dir: target_path.clone(),
            snapshot,
        });
    }

    let mut reader = BufReader::new(stdout);
    let mut buf: Vec<u8> = Vec::new();

    // Read stdout line by line asynchronously (byte-level to handle non-UTF-8 output)
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) => {
                println_flush!("📖 EOF reached");
                break; // EOF
            }
            Ok(_) => {
                let line = String::from_utf8_lossy(&buf);
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    // Parse yt-dlp progress format: [download] 45.2% of ~120.50MiB at 5.20MiB/s ETA 00:23
                    if trimmed.contains("[download]") && trimmed.contains("%") {
                        if let Some(pct_pos) = trimmed.find('%') {
                            let before_pct = &trimmed[..pct_pos];
                            let parts: Vec<&str> = before_pct.split_whitespace().collect();
                            if let Some(last_part) = parts.last() {
                                if let Ok(percent) = last_part.parse::<f32>() {
                                    println_flush!("📊 Parsed progress: {}%", percent);
                                    let _ = window.emit("download-progress", serde_json::json!({
                                        "download_id": &download_id,
                                        "progress": percent,
                                        "status": trimmed
                                    }));
                                    continue;
                                }
                            }
                        }
                        let _ = window.emit("download-progress", serde_json::json!({
                            "download_id": &download_id,
                            "status": trimmed
                        }));
                    } else if trimmed.contains("[ffmpeg]") {
                        let _ = window.emit("download-progress", serde_json::json!({
                            "download_id": &download_id,
                            "status": "Merging streams..."
                        }));
                    } else {
                        let _ = window.emit("download-progress", serde_json::json!({
                            "download_id": &download_id,
                            "status": trimmed
                        }));
                    }
                }
            }
            Err(e) => {
                println_flush!("⚠️ Read error [{}]: {}", download_id, e);
                // If entry was removed it means pause/cancel killed the process
                let was_killed = !DOWNLOADS.lock().await.contains_key(&download_id);
                if was_killed {
                    println_flush!("✅ Killed by pause/cancel [{}]", download_id);
                    break;
                }
                let err_msg = format!("Error reading output: {}", e);
                let _ = window.emit("download-error", serde_json::json!({ "download_id": &download_id, "message": &err_msg }));
                DOWNLOADS.lock().await.remove(&download_id);
                return Err(err_msg);
            }
        }
    }

    // Wait for the process to fully exit
    let status = match child.wait().await {
        Ok(s) => s,
        Err(e) => {
            let err_msg = format!("Failed to wait for child: {}", e);
            let _ = window.emit("download-error", serde_json::json!({ "download_id": &download_id, "message": &err_msg }));
            DOWNLOADS.lock().await.remove(&download_id);
            return Err(err_msg);
        }
    };

    // If entry was already removed by pause/cancel, don't treat as error/success
    let entry = DOWNLOADS.lock().await.remove(&download_id);
    if entry.is_none() {
        return Err("Download was paused or cancelled".to_string());
    }

    if status.success() {
        // Find the newly created file by diffing against the pre-download snapshot
        let file_path: Option<String> = entry.and_then(|e| {
            println_flush!("🔍 Searching for new files in: {}", e.dir.display());
            println_flush!("📸 Snapshot has {} files", e.snapshot.len());

            match std::fs::read_dir(&e.dir) {
                Ok(entries) => {
                    let new_files: Vec<_> = entries.flatten()
                        .map(|ent| ent.path())
                        .filter(|p| {
                            let is_new = !p.to_string_lossy().ends_with(".part") && !e.snapshot.contains(p);
                            if is_new {
                                println_flush!("  ✓ New file: {}", p.display());
                            }
                            is_new
                        })
                        .collect();

                    if new_files.is_empty() {
                        println_flush!("  ❌ No new files found!");
                        return None;
                    }

                    new_files.into_iter().max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
                }
                Err(e) => {
                    println_flush!("  ❌ Failed to read directory: {}", e);
                    None
                }
            }
        }).map(|p| {
            let path_str = p.to_string_lossy().to_string();
            println_flush!("✅ Selected file: {}", path_str);
            path_str
        });

        println_flush!("📁 Final result: {:?}", file_path);
        let msg = format!("✓ Video downloaded successfully to: {}", target_path.display());
        let _ = window.emit("download-complete", serde_json::json!({
            "download_id": &download_id,
            "message": &msg,
            "file_path": file_path
        }));
        println_flush!("✅ SUCCESS [{}]", download_id);
        Ok(msg)
    } else {
        let err_msg = format!("Download failed with exit code: {}", status.code().unwrap_or(-1));
        let _ = window.emit("download-error", serde_json::json!({ "download_id": &download_id, "message": &err_msg }));
        Err(err_msg)
    }
}

/// Get available formats with real-time output streaming to frontend
pub async fn get_video_formats_with_window(
    window: tauri::Window,
    url: String,
) -> Result<String, String> {
    println_flush!("\n========== GET_VIDEO_FORMATS (STREAMING) ==========");
    println_flush!("📌 START");
    println_flush!("📝 URL: {}", url);

    let _ = window.emit("format-output", "🔍 Finding yt-dlp executable...");

    if url.trim().is_empty() {
        let _ = window.emit("format-output", "❌ URL is empty");
        return Err("URL cannot be empty".to_string());
    }

    let ytdlp_path = get_ytdlp_command()?;
    println_flush!("✅ yt-dlp: {}", ytdlp_path);
    let _ = window.emit("format-output", format!("✅ Using: {}", ytdlp_path));

    let _ = window.emit("format-output", format!("🚀 Running: {} -F \"{}\"", ytdlp_path, url));
    let _ = window.emit("format-output", "⏳ Fetching available formats…");

    let ytdlp_path_clone = ytdlp_path.clone();
    let url_clone = url.clone();
    let window_clone = window.clone();

    let result = tokio::task::spawn_blocking(move || {
        use std::process::{Command, Stdio};
        use std::io::BufRead;
        #[cfg(target_os = "windows")]
        use std::os::windows::process::CommandExt;

        let mut cmd = Command::new(&ytdlp_path_clone);
        cmd.arg("-F")
            .arg("--no-playlist")
            .arg("--no-warnings")
            .arg(&url_clone)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return Err(format!("Failed to spawn: {}", e)),
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => return Err("Failed to capture stdout".to_string()),
        };

        let stderr = match child.stderr.take() {
            Some(s) => s,
            None => return Err("Failed to capture stderr".to_string()),
        };

        let reader = std::io::BufReader::new(stdout);
        let stderr_reader = std::io::BufReader::new(stderr);

        let mut all_output = String::new();
        let mut stderr_output = String::new();

        // Read stdout byte-by-byte to tolerate non-UTF-8 (e.g. Arabic titles)
        for raw in reader.split(b'\n') {
            match raw {
                Ok(bytes) => {
                    let line = String::from_utf8_lossy(&bytes);
                    let line = line.trim_end_matches('\r');
                    if !line.is_empty() {
                        all_output.push_str(line);
                        all_output.push('\n');
                        let _ = window_clone.emit("format-output", line.to_string());
                    }
                }
                Err(e) => {
                    let err_msg = format!("❌ Error reading output: {}", e);
                    let _ = window_clone.emit("format-output", err_msg.clone());
                    return Err(err_msg);
                }
            }
        }

        // Read stderr byte-by-byte
        for raw in stderr_reader.split(b'\n') {
            match raw {
                Ok(bytes) => {
                    let line = String::from_utf8_lossy(&bytes);
                    let line = line.trim_end_matches('\r');
                    if !line.is_empty() {
                        stderr_output.push_str(line);
                        stderr_output.push('\n');
                        let _ = window_clone.emit("format-output", format!("⚠️  {}", line));
                    }
                }
                Err(_) => {}
            }
        }

        match child.wait() {
            Ok(status) => {
                // Combine stdout and stderr
                let combined = if stderr_output.is_empty() {
                    all_output
                } else {
                    format!("{}\n{}", all_output, stderr_output)
                };
                Ok((status, combined))
            }
            Err(e) => Err(format!("Failed to wait for child: {}", e)),
        }
    })
    .await;

    match result {
        Ok(Ok((status, output))) => {
            if status.success() {
                let _ = window.emit("format-output", "✅ SUCCESS: Formats retrieved");
                println_flush!("✅ Done");
                Ok(output)
            } else {
                // Show the output which should contain error details
                let exit_code = status.code().unwrap_or(-1);

                // If output is empty, provide generic error
                if output.is_empty() {
                    let msg = format!("❌ Command failed with exit code: {}\n(No error details available)", exit_code);
                    let _ = window.emit("format-output", &msg);
                    println_flush!("{}", msg);
                    Err(msg)
                } else {
                    // Output should contain the error message
                    let msg = format!("❌ Command failed with exit code: {}", exit_code);
                    let _ = window.emit("format-output", msg);
                    // The error is already in the output which was emitted
                    println_flush!("Command failed. See output above for details.");
                    Err(output)
                }
            }
        }
        Ok(Err(e)) => {
            let msg = format!("❌ ERROR: {}", e);
            let _ = window.emit("format-output", &msg);
            Err(msg)
        }
        Err(e) => {
            let msg = format!("❌ Task error: {}", e);
            let _ = window.emit("format-output", &msg);
            Err(msg)
        }
    }
}

/// Get available formats for a video URL using spawn_blocking to avoid hanging
pub async fn get_video_formats(url: String) -> Result<String, String> {
    println_flush!("\n========== GET_VIDEO_FORMATS DEBUG ==========");
    println_flush!("📌 START: get_video_formats called");
    println_flush!("📝 URL: {}", url);

    // Validate URL
    if url.trim().is_empty() {
        println_flush!("❌ ERROR: URL is empty");
        return Err("URL cannot be empty".to_string());
    }

    // Get yt-dlp executable path (local or system)
    println_flush!("🔍 Finding yt-dlp executable...");
    let ytdlp_path = get_ytdlp_command()?;
    println_flush!("✅ yt-dlp path: {}", ytdlp_path);

    println_flush!("🚀 Running: {} -F \"{}\"", ytdlp_path, url);
    println_flush!("⏳ Fetching available formats…");

    // Use spawn_blocking to prevent hanging the async runtime
    let ytdlp_path_clone = ytdlp_path.clone();
    let url_clone = url.clone();

    let result = tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new(&ytdlp_path_clone);
        cmd.arg("-F").arg(&url_clone);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);
        cmd.output()
    })
    .await;

    let output = match result {
        Ok(Ok(output)) => {
            println_flush!("✅ Got response from yt-dlp");
            output
        }
        Ok(Err(e)) => {
            println_flush!("❌ ERROR: Failed to execute yt-dlp: {}", e);
            println_flush!("========== END ==========\n");
            return Err(format!("yt-dlp not found or not executable: {}", e));
        }
        Err(e) => {
            println_flush!("❌ ERROR: Task panicked: {}", e);
            println_flush!("========== END ==========\n");
            return Err(format!("Task error: {}", e));
        }
    };

    // Parse output
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let stderr_str = String::from_utf8_lossy(&output.stderr);

    let stdout_lines = stdout_str.lines().count();
    let stderr_lines = stderr_str.lines().count();

    println_flush!("📊 Response:");
    println_flush!("   Stdout lines: {}", stdout_lines);
    println_flush!("   Stderr lines: {}", stderr_lines);

    if output.status.success() {
        println_flush!("✅ SUCCESS: Retrieved formats");
        println_flush!("========== END ==========\n");
        Ok(stdout_str.to_string())
    } else {
        let error_detail = if !stderr_str.is_empty() {
            stderr_str
                .lines()
                .filter(|l| !l.is_empty())
                .take(3)
                .collect::<Vec<_>>()
                .join(" | ")
        } else {
            format!("exit code: {:?}", output.status.code())
        };
        println_flush!("❌ FAILED: {}", error_detail);
        println_flush!("========== END ==========\n");
        Err(error_detail)
    }
}

/// Open a file with the default application
pub async fn open_file(path: String) -> Result<(), String> {
    println_flush!("📂 open_file: {}", path);
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(&["/c", "start", "", &path])
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    Ok(())
}

/// Reveal a file in the system file manager
pub async fn reveal_in_folder(path: String) -> Result<(), String> {
    println_flush!("📁 reveal_in_folder: {}", path);
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args(&["/select,", &path])
            .spawn()
            .map_err(|e| format!("Failed to reveal in folder: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(&["-R", &path])
            .spawn()
            .map_err(|e| format!("Failed to reveal in folder: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = std::path::Path::new(&path).parent() {
            std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| format!("Failed to reveal in folder: {}", e))?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaylistEntry {
    pub id: String,
    pub title: String,
    pub url: String,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
}

/// Fetch all entries in a playlist using --flat-playlist --dump-json
pub async fn get_playlist_info(url: String) -> Result<Vec<PlaylistEntry>, String> {
    if url.trim().is_empty() {
        return Err("URL cannot be empty".to_string());
    }

    let ytdlp_path = get_ytdlp_command()?;
    let url_clone = url.clone();
    let ytdlp_clone = ytdlp_path.clone();

    let result = tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new(&ytdlp_clone);
        cmd.args(&["--flat-playlist", "--dump-json", "--no-warnings", "--no-update"])
            .arg(&url_clone);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);
        cmd.output()
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
    .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

    let stdout = String::from_utf8_lossy(&result.stdout);
    let mut entries = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) else { continue };

        let id = v["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() { continue; }

        let title = v["title"].as_str().unwrap_or(&id).to_string();

        let video_url = v["url"].as_str()
            .map(|u| if u.starts_with("http") {
                u.to_string()
            } else {
                format!("https://www.youtube.com/watch?v={}", id)
            })
            .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={}", id));

        let duration = v["duration"].as_f64();

        let thumbnail = v["thumbnail"].as_str().map(String::from)
            .or_else(|| Some(format!("https://i.ytimg.com/vi/{}/mqdefault.jpg", id)));

        entries.push(PlaylistEntry { id, title, url: video_url, duration, thumbnail });
    }

    if entries.is_empty() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        let msg = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("No videos found in playlist").to_string();
        return Err(msg);
    }

    Ok(entries)
}

/// Fetch video metadata (title, thumbnail, duration, filesize) using yt-dlp --dump-json
pub async fn get_video_info(url: String) -> Result<VideoInfo, String> {
    if url.trim().is_empty() {
        return Err("URL cannot be empty".to_string());
    }

    let ytdlp_path = get_ytdlp_command()?;
    let url_clone = url.clone();
    let ytdlp_clone = ytdlp_path.clone();

    let result = tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new(&ytdlp_clone);
        cmd.args(&["--dump-json", "--no-playlist", "--no-warnings"])
            .arg(&url_clone);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);
        cmd.output()
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
    .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        let msg = stderr.lines().next().unwrap_or("Unknown error").to_string();
        return Err(msg);
    }

    let json_str = String::from_utf8_lossy(&result.stdout);
    let first_line = json_str.lines().next().unwrap_or("{}");

    let json: serde_json::Value = serde_json::from_str(first_line)
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(VideoInfo {
        title: json["title"].as_str().unwrap_or("Unknown").to_string(),
        thumbnail: json["thumbnail"].as_str().map(|s| s.to_string()),
        duration: json["duration"].as_f64(),
        filesize: json["filesize"].as_i64()
            .or_else(|| json["filesize_approx"].as_i64()),
        ext: json["ext"].as_str().map(|s| s.to_string()),
        width: json["width"].as_u64(),
        height: json["height"].as_u64(),
        is_live: json["is_live"].as_bool(),
    })
}
