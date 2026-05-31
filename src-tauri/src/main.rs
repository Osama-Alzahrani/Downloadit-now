// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use downloadit_lib::{download_video as download_video_impl, download_video_with_window, get_video_formats_with_window, pause_download as pause_download_impl, cancel_download as cancel_download_impl};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct DownloadParams {
    url: String,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    download_path: Option<String>,
}

#[tauri::command]
async fn download_video(params: DownloadParams) -> Result<String, String> {
    download_video_impl(params.url, params.format, params.download_path).await
}

#[tauri::command]
async fn download_video_stream(window: tauri::Window, params: DownloadParams) -> Result<String, String> {
    download_video_with_window(window, params.url, params.format, params.download_path).await
}

#[tauri::command]
async fn get_video_formats(window: tauri::Window, url: String) -> Result<String, String> {
    get_video_formats_with_window(window, url).await
}

#[tauri::command]
async fn pause_download() -> Result<String, String> {
    pause_download_impl().await
}

#[tauri::command]
async fn cancel_download() -> Result<String, String> {
    cancel_download_impl().await
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![download_video, download_video_stream, get_video_formats, pause_download, cancel_download])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}