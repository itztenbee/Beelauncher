use serde::Serialize;
use serde_json::Value;

use crate::profile::{self, Loader, ModEntry, Profile};

const API: &str = "https://api.modrinth.com/v2";

#[derive(Serialize)]
pub struct SearchHit {
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub categories: Vec<String>,
}

fn loader(profile: &Profile) -> &'static str {
    match &profile.loader { Loader::Fabric { .. } => "fabric", Loader::Vanilla => "minecraft" }
}

pub async fn search(profile_id: &str, query: &str) -> Result<Vec<SearchHit>, String> {
    let profile = profile::load_profile(profile_id)?;
    let mut facets = vec![vec!["project_type:mod".to_string()], vec![format!("versions:{}", profile.minecraft_version)]];
    if matches!(&profile.loader, Loader::Fabric { .. }) { facets.push(vec!["categories:fabric".to_string()]); }
    let response: Value = reqwest::Client::new().get(format!("{API}/search"))
        .query(&[("query", query), ("limit", "20"), ("facets", &serde_json::to_string(&facets).unwrap())])
        .send().await.map_err(|e| format!("Modrinth-Suche fehlgeschlagen: {e}"))?
        .error_for_status().map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    Ok(response["hits"].as_array().into_iter().flatten().filter_map(|hit| Some(SearchHit {
        project_id: hit["project_id"].as_str()?.to_string(), title: hit["title"].as_str()?.to_string(),
        description: hit["description"].as_str().unwrap_or("Keine Beschreibung.").to_string(),
        author: hit["author"].as_str().unwrap_or("Unbekannt").to_string(), icon_url: hit["icon_url"].as_str().map(str::to_string),
        downloads: hit["downloads"].as_u64().unwrap_or(0), categories: hit["categories"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect(),
    })).collect())
}

pub async fn install(profile_id: &str, project_id: &str) -> Result<Profile, String> {
    let mut profile = profile::load_profile(profile_id)?;
    let versions: Value = reqwest::Client::new().get(format!("{API}/project/{project_id}/version"))
        .query(&[("loaders", serde_json::to_string(&vec![loader(&profile)]).unwrap()), ("game_versions", serde_json::to_string(&vec![&profile.minecraft_version]).unwrap())])
        .send().await.map_err(|e| format!("Modrinth-Versionen konnten nicht geladen werden: {e}"))?
        .error_for_status().map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let version = versions.as_array().and_then(|list| list.first()).ok_or("Keine passende Mod-Version für dieses Profil gefunden.")?;
    let file = version["files"].as_array().and_then(|files| files.iter().find(|f| f["primary"].as_bool() == Some(true)).or_else(|| files.first())).ok_or("Modrinth lieferte keine herunterladbare Datei.")?;
    let filename = file["filename"].as_str().ok_or("Ungültiger Mod-Dateiname.")?.to_string();
    let url = file["url"].as_str().ok_or("Ungültige Mod-Download-URL.")?.to_string();
    if profile.mods.iter().any(|m| m.filename == filename) {
        return Ok(profile);
    }

    // Install immediately so the user sees the mod in the selected profile
    // right after clicking the button, instead of waiting for the next launch.
    let bytes = reqwest::get(&url)
        .await
        .map_err(|e| format!("Mod-Download fehlgeschlagen: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Mod-Download fehlgeschlagen: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("Mod-Datei konnte nicht gelesen werden: {e}"))?;
    let mods_dir = profile::root_dir().join("instances").join(&profile.id).join("mods");
    let cache_dir = profile::root_dir().join("mod_cache");
    std::fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    std::fs::write(cache_dir.join(&filename), &bytes).map_err(|e| e.to_string())?;
    std::fs::write(mods_dir.join(&filename), &bytes).map_err(|e| e.to_string())?;

    profile.mods.push(ModEntry { filename, url, enabled: true });
    profile::save_profile(&profile)?;
    Ok(profile)
}
