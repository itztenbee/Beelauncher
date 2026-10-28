// Beelauncher backend entry point.
// A launcher is a GUI app: do not show a terminal window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod fabric;
mod launcher;
mod mojang;
mod modrinth;
mod profile;

use serde::Serialize;
use tauri::Window;

#[derive(Serialize)]
pub struct AccountInfo {
    pub name: String,
    pub uuid: String,
}

#[tauri::command]
async fn microsoft_login() -> Result<AccountInfo, String> {
    auth::login_with_microsoft().await
}

#[tauri::command]
fn get_account() -> Option<AccountInfo> {
    auth::load_account().map(|a| AccountInfo {
        name: a.name,
        uuid: a.uuid,
    })
}

#[tauri::command]
async fn launch_profile(window: Window, profile_id: String) -> Result<(), String> {
    launcher::launch(window, &profile_id).await
}

#[tauri::command]
fn list_profiles() -> Vec<profile::Profile> {
    profile::ensure_default_profiles().ok();
    profile::list_profiles()
}

#[tauri::command]
fn save_profile(profile: profile::Profile) -> Result<(), String> {
    profile::save_profile(&profile)
}

#[tauri::command]
fn add_mod(profile_id: String, filename: String, url: String) -> Result<profile::Profile, String> {
    let mut p = profile::load_profile(&profile_id)?;
    p.mods.push(profile::ModEntry {
        filename,
        url,
        enabled: true,
    });
    profile::save_profile(&p)?;
    Ok(p)
}

#[tauri::command]
fn toggle_mod(profile_id: String, filename: String, enabled: bool) -> Result<profile::Profile, String> {
    let mut p = profile::load_profile(&profile_id)?;
    if let Some(m) = p.mods.iter_mut().find(|m| m.filename == filename) {
        m.enabled = enabled;
    }
    profile::save_profile(&p)?;
    Ok(p)
}

#[tauri::command]
async fn search_modrinth(profile_id: String, query: String) -> Result<Vec<modrinth::SearchHit>, String> {
    modrinth::search(&profile_id, &query).await
}

#[tauri::command]
async fn add_modrinth_mod(profile_id: String, project_id: String) -> Result<profile::Profile, String> {
    modrinth::install(&profile_id, &project_id).await
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            microsoft_login,
            get_account,
            launch_profile,
            list_profiles,
            save_profile,
            add_mod,
            toggle_mod,
            search_modrinth,
            add_modrinth_mod
        ])
        .run(tauri::generate_context!())
        .expect("error while running Beelauncher");
}
