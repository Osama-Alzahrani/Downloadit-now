use dirs::desktop_dir;
use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use std::io::Write;
use tokio::sync::Mutex;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::{Command, Child};
use tauri::Emitter;

/// Helper macro to print and flush immediately
macro_rules! println_flush {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = std::io::stdout().flush();
    }};
}

// Global state to track active download
lazy_static::lazy_static! {
    pub static ref DOWNLOAD_STATE: Mutex<Option<Child>> = Mutex::new(None);
    pub static ref DOWNLOAD_PID: Mutex<Option<u32>> = Mutex::new(None);
    pub static ref DOWNLOAD_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
    // Snapshot of files before download started, so cancel can find the exact new .part file
    pub static ref DOWNLOAD_PRE_SNAPSHOT: Mutex<std::collections::HashSet<PathBuf>> = Mutex::new(std::collections::HashSet::new());
}

/// Pause the active download (kill process but keep file)
pub async fn pause_download() -> Result<String, String> {
    println_flush!("⏸ pause_download called");

    let pid = {
        let pid_guard = DOWNLOAD_PID.lock().await;
        pid_guard.as_ref().copied()
    };

    if let Some(pid) = pid {
        println_flush!("⏸ Found PID: {}, pausing...", pid);

        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("taskkill")
                .args(&["/PID", &pid.to_string(), "/F", "/T"])
                .output();
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = std::process::Command::new("kill")
                .args(&["-9", &pid.to_string()])
                .output();
        }

        // Kill the child process
        let mut state = DOWNLOAD_STATE.lock().await;
        if let Some(mut child) = state.take() {
            let _ = child.kill().await;
        }

        let mut pid_guard = DOWNLOAD_PID.lock().await;
        *pid_guard = None;

        Ok("Download paused".to_string())
    } else {
        println_flush!("❌ No active download to pause");
        Err("No active download to pause".to_string())
    }
}

/// Cancel the active download and delete the downloaded file
pub async fn cancel_download() -> Result<String, String> {
    println_flush!("🔴 cancel_download called");

    // Try to kill the process
    let pid = {
        let pid_guard = DOWNLOAD_PID.lock().await;
        println_flush!("🔍 Checking DOWNLOAD_PID...");
        let p = pid_guard.as_ref().copied();
        println_flush!("🔍 PID result: {:?}", p);
        p
    };

    if let Some(pid) = pid {
        println_flush!("🔴 Found PID: {}, attempting to kill...", pid);

        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("taskkill")
                .args(&["/PID", &pid.to_string(), "/F", "/T"])
                .output();
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = std::process::Command::new("kill")
                .args(&["-9", &pid.to_string()])
                .output();
        }

        // Kill the child process
        let mut state = DOWNLOAD_STATE.lock().await;
        if let Some(mut child) = state.take() {
            let _ = child.kill().await;
        }

        let mut pid_guard = DOWNLOAD_PID.lock().await;
        *pid_guard = None;
    }

    // Find and delete the new .part file created by this download
    // by comparing current directory contents against pre-download snapshot
    let dir_path = {
        let mut dir_guard = DOWNLOAD_DIR.lock().await;
        dir_guard.take()
    };
    let snapshot = {
        let mut snap_guard = DOWNLOAD_PRE_SNAPSHOT.lock().await;
        std::mem::take(&mut *snap_guard)
    };

    if let Some(dir) = dir_path {
        println_flush!("🔍 Scanning for new .part files in: {}", dir.display());
        match std::fs::read_dir(&dir) {
            Ok(entries) => {
                let mut deleted = false;
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.to_string_lossy().ends_with(".part") && !snapshot.contains(&path) {
                        println_flush!("🗑️ Deleting new .part file: {}", path.display());
                        match std::fs::remove_file(&path) {
                            Ok(_) => { println_flush!("✅ Deleted"); deleted = true; }
                            Err(e) => { println_flush!("⚠️ Failed: {}", e); }
                        }
                    }
                }
                if deleted {
                    Ok("Download cancelled and file deleted".to_string())
                } else {
                    Ok("Download cancelled (no incomplete file found)".to_string())
                }
            }
            Err(e) => {
                println_flush!("❌ Failed to read directory: {}", e);
                Ok("Download cancelled".to_string())
            }
        }
    } else {
        Ok("Download cancelled".to_string())
    }
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
    download_path: Option<String>,
) -> Result<String, String> {
    println_flush!("\n========== DOWNLOAD_VIDEO (STREAMING - ASYNC) ==========");
    println_flush!("📌 START");
    println_flush!("📝 URL: {}", url);
    println_flush!("📝 Download Path: {:?}", download_path);
    println_flush!("📝 Format: {:?}", format);

    let _ = window.emit("download-progress", serde_json::json!({ "status": "Finding yt-dlp..." }));

    if url.trim().is_empty() {
        let _ = window.emit("download-error", "URL is empty");
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
    let _ = window.emit("download-progress", serde_json::json!({ "status": "Getting filename..." }));

    // Use original video title as filename (supports any language/Unicode)
    // Store the download directory so cancel can find and delete the newest .part file
    {
        let mut dir_guard = DOWNLOAD_DIR.lock().await;
        *dir_guard = Some(target_path.clone());
        println_flush!("📁 Stored download directory: {}", target_path.display());

        // Snapshot only NON-.part files before download starts
        // This ensures resumed .part files from previous sessions are also cleaned up on cancel
        let snapshot: std::collections::HashSet<PathBuf> = std::fs::read_dir(&target_path)
            .map(|entries| entries.flatten()
                .map(|e| e.path())
                .filter(|p| !p.to_string_lossy().ends_with(".part"))
                .collect())
            .unwrap_or_default();
        let mut snap_guard = DOWNLOAD_PRE_SNAPSHOT.lock().await;
        *snap_guard = snapshot;
        println_flush!("📸 Snapshotted {} existing non-.part files", snap_guard.len());
    }

    let _ = window.emit("download-progress", serde_json::json!({ "status": "Starting download..." }));

    // Create async command
    let mut cmd = Command::new(&ytdlp_path);
    cmd.arg("--progress")
        .arg("--no-warnings")
        .arg("--no-update")
        .arg("--newline");

    if let Some(fmt) = format {
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
            let _ = window.emit("download-error", &err_msg);
            return Err(err_msg);
        }
    };

    // Extract stdout before storing child
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = window.emit("download-error", "Failed to capture stdout");
            return Err("Failed to capture stdout".to_string());
        }
    };

    // Store child process and PID in global state for cancel/pause control
    {
        let pid = child.id();
        println_flush!("📌 Child spawned with PID: {:?}", pid);

        let mut state = DOWNLOAD_STATE.lock().await;
        *state = Some(child);
        println_flush!("📌 Stored child process");

        let mut pid_state = DOWNLOAD_PID.lock().await;
        *pid_state = pid;
        println_flush!("📌 Stored PID: {:?}", *pid_state);
    }

    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // Read stdout line by line asynchronously
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                println_flush!("📖 EOF reached");
                break; // EOF
            }
            Ok(_) => {
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
                                        "progress": percent,
                                        "status": trimmed
                                    }));
                                    continue;
                                }
                            }
                        }
                        let _ = window.emit("download-progress", serde_json::json!({
                            "status": trimmed
                        }));
                    } else if trimmed.contains("[ffmpeg]") {
                        let _ = window.emit("download-progress", serde_json::json!({
                            "status": "Merging streams..."
                        }));
                    } else {
                        let _ = window.emit("download-progress", serde_json::json!({
                            "status": trimmed
                        }));
                    }
                }
            }
            Err(e) => {
                // Check if this is because the process was killed
                let err_str = e.to_string();
                println_flush!("⚠️ Read error: {}", err_str);

                // Check if child is still in state (was it killed?)
                let state = DOWNLOAD_STATE.lock().await;
                if state.is_none() {
                    println_flush!("✅ Process was killed by cancel");
                    let _ = window.emit("download-error", "Download cancelled by user");
                    // Don't clear DOWNLOAD_DIR - let cancel_download delete the file
                    // Just wait for child.wait() to finish below
                    break;
                }
                drop(state);

                let err_msg = format!("Error reading output: {}", e);
                let _ = window.emit("download-error", &err_msg);
                let mut state = DOWNLOAD_STATE.lock().await;
                *state = None;
                return Err(err_msg);
            }
        }
    }

    // Wait for child process to finish
    let mut state = DOWNLOAD_STATE.lock().await;
    let status = match state.take() {
        Some(mut child) => {
            match child.wait().await {
                Ok(status) => status,
                Err(e) => {
                    let err_msg = format!("Failed to wait for child: {}", e);
                    let _ = window.emit("download-error", &err_msg);
                    // Clear PID only, NOT FILE (let cancel_download handle it)
                    let mut pid_state = DOWNLOAD_PID.lock().await;
                    *pid_state = None;
                    return Err(err_msg);
                }
            }
        }
        None => {
            // If child is None, it was probably killed by pause/cancel
            // Don't emit error - let the pause/cancel function handle the message
            // Clear PID only, NOT FILE (let cancel_download handle it)
            let mut pid_state = DOWNLOAD_PID.lock().await;
            *pid_state = None;
            return Err("Download was paused or cancelled".to_string());
        }
    };
    drop(state); // Release lock before checking status

    if status.success() {
        // Clear download info on success
        {
            let mut pid_state = DOWNLOAD_PID.lock().await;
            *pid_state = None;
            let mut dir_guard = DOWNLOAD_DIR.lock().await;
            *dir_guard = None;
        }
        let msg = format!("✓ Video downloaded successfully to: {}", target_path.display());
        let _ = window.emit("download-complete", &msg);
        println_flush!("✅ SUCCESS");
        Ok(msg)
    } else {
        // On failure, keep DOWNLOAD_DIR set so cancel can still delete incomplete files
        // But clear PID since process is done
        {
            let mut pid_state = DOWNLOAD_PID.lock().await;
            *pid_state = None;
        }
        let err_msg = format!("Download failed with exit code: {}", status.code().unwrap_or(-1));
        let _ = window.emit("download-error", &err_msg);
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
    let _ = window.emit("format-output", "⏳ Waiting for YouTube response...");

    let ytdlp_path_clone = ytdlp_path.clone();
    let url_clone = url.clone();
    let window_clone = window.clone();

    let result = tokio::task::spawn_blocking(move || {
        use std::process::{Command, Stdio};
        use std::io::BufRead;

        let mut child = match Command::new(&ytdlp_path_clone)
            .arg("-F")
            .arg("--no-playlist")
            .arg("--no-warnings")
            .arg(&url_clone)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
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

        // Read stdout
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if !line.is_empty() {
                        all_output.push_str(&line);
                        all_output.push('\n');
                        let _ = window_clone.emit("format-output", line);
                    }
                }
                Err(e) => {
                    let err_msg = format!("❌ Error reading output: {}", e);
                    let _ = window_clone.emit("format-output", err_msg.clone());
                    return Err(err_msg);
                }
            }
        }

        // Read stderr
        for line in stderr_reader.lines() {
            match line {
                Ok(line) => {
                    if !line.is_empty() {
                        stderr_output.push_str(&line);
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
    println_flush!("⏳ Fetching formats (waiting for YouTube response)...");

    // Use spawn_blocking to prevent hanging the async runtime
    let ytdlp_path_clone = ytdlp_path.clone();
    let url_clone = url.clone();

    let result = tokio::task::spawn_blocking(move || {
        std::process::Command::new(&ytdlp_path_clone)
            .arg("-F")
            .arg(&url_clone)
            .output()
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
