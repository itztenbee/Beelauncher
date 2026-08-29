// Profile = one launchable setup (a Minecraft version + loader + mod list).
// Stored as plain JSON files under ~/.beelauncher/profiles/<id>.json so
// they're easy to inspect/edit by hand while this is still early.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub minecraft_version: String,
    pub loader: Loader,
    /// Mods this profile wants active. `url` is where to fetch it from if it's
    /// not already in the shared mod cache (~/.beelauncher/mod_cache/).
    pub mods: Vec<ModEntry>,
    pub java_args: Vec<String>,
    pub memory_mb: u32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ModEntry {
    pub filename: String,
    pub url: String,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    Vanilla,
    Fabric { loader_version: String },
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            id: "vanilla-1211".into(),
            name: "Vanilla".into(),
            minecraft_version: "1.21.11".into(),
            loader: Loader::Vanilla,
            mods: vec![],
            java_args: vec![],
            memory_mb: 4096,
        }
    }
}

pub fn root_dir() -> PathBuf {
    // ~/.beelauncher on Linux/macOS, %APPDATA%\.beelauncher on Windows (dirs handles this)
    dirs::home_dir()
        .expect("no home dir found")
        .join(".beelauncher")
}

pub fn profiles_dir() -> PathBuf {
    root_dir().join("profiles")
}

pub fn load_profile(id: &str) -> Result<Profile, String> {
    let path = profiles_dir().join(format!("{id}.json"));
    if !path.exists() {
        return Err(format!("Profil '{id}' existiert nicht ({path:?})"));
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn save_profile(profile: &Profile) -> Result<(), String> {
    let dir = profiles_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", profile.id));
    let raw = serde_json::to_string_pretty(profile).map_err(|e| e.to_string())?;
    std::fs::write(path, raw).map_err(|e| e.to_string())
}

pub fn list_profiles() -> Vec<Profile> {
    let dir = profiles_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return vec![];
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|raw| serde_json::from_str(&raw).ok())
        .collect()
}

/// Creates the two default profiles (Vanilla + a BeeClient/Fabric one) on
/// first run so the UI isn't empty. Safe to call every startup -- it
/// no-ops if profiles already exist on disk.
pub fn ensure_default_profiles() -> Result<(), String> {
    if !profiles_dir().exists() || list_profiles().is_empty() {
        save_profile(&Profile::default())?;
        save_profile(&Profile {
            id: "beeclient-1211".into(),
            name: "BeeClient".into(),
            minecraft_version: "1.21.11".into(),
            loader: Loader::Fabric {
                loader_version: "0.16.9".into(), // TODO: pin/update to whatever BeeClient actually targets
            },
            // TODO: point this at wherever BeeClient release jars actually
            // get hosted (GitHub Releases URL, your own CDN, etc.) -- this
            // placeholder URL will 404.
            mods: vec![ModEntry {
                filename: "beeclient.jar".into(),
                url: "https://example.com/beeclient-latest.jar".into(),
                enabled: true,
            }],
            java_args: vec![],
            memory_mb: 4096,
        })?;
    }
    Ok(())
}
