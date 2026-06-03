// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use downloadit_lib::{download_video as download_video_impl, download_video_with_window, get_video_formats_with_window, pause_download as pause_download_impl, cancel_download as cancel_download_impl, get_video_info as get_video_info_impl, open_file as open_file_impl, reveal_in_folder as reveal_in_folder_impl, check_dependencies as check_deps_impl, download_dependencies as download_deps_impl, DepsStatus, VideoInfo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct DownloadParams {
    url: String,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    download_path: Option<String>,
    #[serde(default)]
    download_id: Option<String>,
}

#[tauri::command]
async fn download_video(params: DownloadParams) -> Result<String, String> {
    download_video_impl(params.url, params.format, params.download_path).await
}

#[tauri::command]
async fn download_video_stream(window: tauri::Window, params: DownloadParams) -> Result<String, String> {
    let download_id = params.download_id.unwrap_or_else(|| "default".to_string());
    download_video_with_window(window, params.url, params.format, params.download_path, download_id).await
}

#[tauri::command]
async fn get_video_info(url: String) -> Result<VideoInfo, String> {
    get_video_info_impl(url).await
}

#[tauri::command]
async fn get_video_formats(window: tauri::Window, url: String) -> Result<String, String> {
    get_video_formats_with_window(window, url).await
}

#[tauri::command]
async fn pause_download(download_id: String) -> Result<String, String> {
    pause_download_impl(download_id).await
}

#[tauri::command]
async fn cancel_download(download_id: String) -> Result<String, String> {
    cancel_download_impl(download_id).await
}

#[tauri::command]
async fn open_file(path: String) -> Result<(), String> {
    open_file_impl(path).await
}

#[tauri::command]
async fn reveal_in_folder(path: String) -> Result<(), String> {
    reveal_in_folder_impl(path).await
}

#[tauri::command]
async fn check_dependencies() -> DepsStatus {
    check_deps_impl().await
}

#[tauri::command]
async fn download_dependencies(window: tauri::Window) -> Result<(), String> {
    download_deps_impl(window).await
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![download_video, download_video_stream, get_video_formats, pause_download, cancel_download, get_video_info, open_file, reveal_in_folder, check_dependencies, download_dependencies])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}