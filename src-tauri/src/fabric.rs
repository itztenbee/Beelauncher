// Fabric adds its own loader + intermediary mappings on top of vanilla.
// Fabric's meta API conveniently returns a launch profile in basically the
// same shape as Mojang's client.json, so we can reuse the same library
// download logic and just merge the results.
//
// API docs: https://fabricmc.net/wiki/documentation:fabric_meta

use serde::Deserialize;

use crate::mojang::Library;
use crate::profile::root_dir;

#[derive(Deserialize)]
pub struct FabricProfile {
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub libraries: Vec<Library>,
}

pub async fn fetch_fabric_profile(
    minecraft_version: &str,
    loader_version: &str,
) -> Result<FabricProfile, String> {
    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{minecraft_version}/{loader_version}/profile/json"
    );
    reqwest::get(&url)
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// Downloads all Fabric loader libraries (loader itself, intermediary
/// mappings, ASM, etc.) and returns their classpath entries. These are
/// always plain jars -- no natives/classifiers involved for Fabric.
pub async fn download_fabric_libraries(profile: &FabricProfile) -> Result<Vec<std::path::PathBuf>, String> {
    let libraries_dir = root_dir().join("libraries");
    let mut classpath = vec![];

    for lib in &profile.libraries {
        let parts: Vec<&str> = lib.name.split(':').collect();
        if parts.len() != 3 {
            continue;
        }
        let (group, artifact, version) = (parts[0], parts[1], parts[2]);
        let group_path = group.replace('.', "/");
        let rel = format!("{group_path}/{artifact}/{version}/{artifact}-{version}.jar");
        let dest = libraries_dir.join(&rel);

        if !dest.exists() {
            // Fabric's meta doesn't give us a sha1 up front for these, so we
            // trust the (well-known, HTTPS) Fabric Maven here rather than
            // reusing mojang::download_verified's checksum path.
            let maven_base = "https://maven.fabricmc.net/";
            let url = format!("{maven_base}{rel}");
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let bytes = reqwest::get(&url)
                .await
                .map_err(|e| e.to_string())?
                .bytes()
                .await
                .map_err(|e| e.to_string())?;
            std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
        }
        classpath.push(dest);
    }

    Ok(classpath)
}
