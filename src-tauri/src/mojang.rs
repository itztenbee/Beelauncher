// Talks to Mojang's piston-meta endpoints to figure out what files a given
// vanilla version needs, then downloads them into ~/.beelauncher/.
//
// Schema reference (unofficial but accurate): https://minecraft.wiki/w/Client.json

use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};

use crate::profile::root_dir;

const VERSION_MANIFEST_URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Deserialize)]
struct VersionManifest {
    versions: Vec<VersionManifestEntry>,
}

#[derive(Deserialize)]
struct VersionManifestEntry {
    id: String,
    url: String,
}

#[derive(Deserialize, Clone)]
pub struct ClientJson {
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub libraries: Vec<Library>,
    pub downloads: ClientDownloads,
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndexRef,
    pub arguments: Option<Arguments>,
    // Older versions (<1.13) use "minecraftArguments" instead of the
    // structured "arguments" block above.
    #[serde(rename = "minecraftArguments")]
    pub legacy_arguments: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct ClientDownloads {
    pub client: DownloadEntry,
}

#[derive(Deserialize, Clone)]
pub struct DownloadEntry {
    pub url: String,
    pub sha1: String,
    #[allow(dead_code)]
    pub size: u64,
}

#[derive(Deserialize, Clone)]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    pub sha1: String,
}

#[derive(Deserialize, Clone)]
pub struct Arguments {
    pub game: Vec<serde_json::Value>, // mixed strings + conditional-rule objects; kept raw, resolved in launcher.rs
    pub jvm: Vec<serde_json::Value>,
}

#[derive(Deserialize, Clone)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub rules: Option<Vec<Rule>>,
    pub natives: Option<std::collections::HashMap<String, String>>,
}

#[derive(Deserialize, Clone)]
pub struct LibraryDownloads {
    pub artifact: Option<DownloadEntry>,
    pub classifiers: Option<std::collections::HashMap<String, DownloadEntry>>,
}

#[derive(Deserialize, Clone)]
pub struct Rule {
    pub action: String, // "allow" | "disallow"
    pub os: Option<RuleOs>,
}

#[derive(Deserialize, Clone)]
pub struct RuleOs {
    pub name: Option<String>, // "windows" | "osx" | "linux"
}

/// Returns true if this library should be used on the current OS,
/// per the version JSON's rule list (absence of rules = always allowed).
fn library_applies(lib: &Library) -> bool {
    let Some(rules) = &lib.rules else { return true };
    let current_os = current_os_name();
    let mut allowed = false;
    for rule in rules {
        let os_matches = rule
            .os
            .as_ref()
            .and_then(|o| o.name.as_deref())
            .map(|name| name == current_os)
            .unwrap_or(true); // no os field = rule applies to all OSes
        if os_matches {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

fn current_os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

pub async fn fetch_client_json(minecraft_version: &str) -> Result<ClientJson, String> {
    let client = reqwest::Client::new();
    let manifest: VersionManifest = client
        .get(VERSION_MANIFEST_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let entry = manifest
        .versions
        .iter()
        .find(|v| v.id == minecraft_version)
        .ok_or_else(|| format!("Version {minecraft_version} nicht im Manifest gefunden"))?;

    client
        .get(&entry.url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// Downloads a file if it doesn't already exist on disk with the right
/// sha1 -- so re-launching doesn't redownload gigabytes every time.
async fn download_verified(url: &str, dest: &Path, expected_sha1: &str) -> Result<(), String> {
    if dest.exists() {
        if let Ok(existing) = std::fs::read(dest) {
            let mut hasher = Sha1::new();
            hasher.update(&existing);
            if format!("{:x}", hasher.finalize()) == expected_sha1 {
                return Ok(()); // already good, skip
            }
        }
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = reqwest::get(url)
        .await
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    std::fs::write(dest, &bytes).map_err(|e| e.to_string())
}

/// Downloads the client jar + all applicable libraries + extracts natives.
/// Returns (classpath entries, natives_dir).
pub async fn download_libraries_and_client(
    client_json: &ClientJson,
    minecraft_version: &str,
) -> Result<(Vec<PathBuf>, PathBuf), String> {
    let libraries_dir = root_dir().join("libraries");
    let versions_dir = root_dir().join("versions").join(minecraft_version);
    let natives_dir = versions_dir.join("natives");
    std::fs::create_dir_all(&natives_dir).map_err(|e| e.to_string())?;

    let mut classpath = vec![];

    // Client jar itself.
    let client_jar_path = versions_dir.join(format!("{minecraft_version}.jar"));
    download_verified(
        &client_json.downloads.client.url,
        &client_jar_path,
        &client_json.downloads.client.sha1,
    )
    .await?;
    classpath.push(client_jar_path);

    for lib in &client_json.libraries {
        if !library_applies(lib) {
            continue;
        }
        let Some(downloads) = &lib.downloads else { continue };

        if let Some(artifact) = &downloads.artifact {
            let rel_path = maven_name_to_path(&lib.name);
            let dest = libraries_dir.join(&rel_path);
            download_verified(&artifact.url, &dest, &artifact.sha1).await?;
            classpath.push(dest);
        }

        // Natives (LWJGL etc.) come as classifier jars that get *extracted*
        // into natives_dir rather than added to the classpath.
        if let (Some(natives_map), Some(classifiers)) = (&lib.natives, &downloads.classifiers) {
            if let Some(classifier_key) = natives_map.get(current_os_name()) {
                if let Some(entry) = classifiers.get(classifier_key) {
                    let tmp_jar = libraries_dir.join(format!("{}-natives-tmp.jar", lib.name.replace([':', '.'], "_")));
                    download_verified(&entry.url, &tmp_jar, &entry.sha1).await?;
                    extract_natives(&tmp_jar, &natives_dir)?;
                }
            }
        }
    }

    Ok((classpath, natives_dir))
}

/// Converts a Maven-style coordinate ("group:artifact:version") into the
/// relative jar path Mojang's library repo uses.
fn maven_name_to_path(name: &str) -> PathBuf {
    let parts: Vec<&str> = name.split(':').collect();
    let (group, artifact, version) = (parts[0], parts[1], parts[2]);
    let group_path = group.replace('.', "/");
    PathBuf::from(format!(
        "{group_path}/{artifact}/{version}/{artifact}-{version}.jar"
    ))
}

fn extract_natives(jar_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(jar_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        // Skip META-INF and directories -- we only want the actual
        // .dll/.so/.dylib files at the root of the natives jar.
        if name.starts_with("META-INF") || entry.is_dir() {
            continue;
        }
        let out_path = dest_dir.join(Path::new(&name).file_name().unwrap_or_default());
        let mut out_file = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out_file).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Resolves the version JSON's "arguments" block (game + jvm args, with
/// ${placeholder} substitution and OS-conditional rules) into a plain
/// Vec<String> ready to hand to std::process::Command.
///
/// Handles both the modern structured "arguments" (1.13+) and falls back
/// to splitting the legacy "minecraftArguments" string (pre-1.13) if
/// that's what the version JSON has instead.
pub struct LaunchContext<'a> {
    pub classpath: &'a str,
    pub natives_dir: &'a std::path::Path,
    pub game_dir: &'a std::path::Path,
    pub assets_dir: &'a std::path::Path,
    pub asset_index_id: &'a str,
    pub minecraft_version: &'a str,
    pub username: &'a str,
    pub uuid: &'a str,
    pub access_token: &'a str,
}

fn substitute(template: &str, ctx: &LaunchContext) -> String {
    template
        .replace("${auth_player_name}", ctx.username)
        .replace("${auth_uuid}", ctx.uuid)
        .replace("${auth_access_token}", ctx.access_token)
        .replace("${auth_session}", ctx.access_token) // pre-1.13 legacy name for the same thing
        .replace("${user_type}", "msa")
        .replace("${version_name}", ctx.minecraft_version)
        .replace("${version_type}", "release")
        .replace("${game_directory}", &ctx.game_dir.to_string_lossy())
        .replace("${assets_root}", &ctx.assets_dir.to_string_lossy())
        .replace("${game_assets}", &ctx.assets_dir.to_string_lossy()) // legacy pre-1.7 name
        .replace("${assets_index_name}", ctx.asset_index_id)
        .replace("${classpath}", ctx.classpath)
        .replace("${natives_directory}", &ctx.natives_dir.to_string_lossy())
        .replace("${launcher_name}", "Beelauncher")
        .replace("${launcher_version}", "0.1.0")
        .replace("${clientid}", "-")
        .replace("${auth_xuid}", "-")
}

fn rule_allows(rules: &serde_json::Value) -> bool {
    let Some(rules) = rules.as_array() else { return true };
    let current_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    };
    let mut allowed = false;
    for rule in rules {
        let action = rule.get("action").and_then(|a| a.as_str()).unwrap_or("allow");
        let os_matches = rule
            .get("os")
            .and_then(|o| o.get("name"))
            .and_then(|n| n.as_str())
            .map(|name| name == current_os)
            .unwrap_or(true);
        if os_matches {
            allowed = action == "allow";
        }
    }
    allowed
}

/// Resolves one entry from the "game"/"jvm" arrays -- each entry is either
/// a plain string, or a `{"rules": [...], "value": "x" | ["x","y"]}` object
/// that only applies on certain OSes/conditions.
fn resolve_argument_entries(entries: &[serde_json::Value], ctx: &LaunchContext) -> Vec<String> {
    let mut out = vec![];
    for entry in entries {
        if let Some(s) = entry.as_str() {
            out.push(substitute(s, ctx));
        } else if let Some(obj) = entry.as_object() {
            let allowed = obj
                .get("rules")
                .map(|r| rule_allows(r))
                .unwrap_or(true);
            if !allowed {
                continue;
            }
            match obj.get("value") {
                Some(serde_json::Value::String(s)) => out.push(substitute(s, ctx)),
                Some(serde_json::Value::Array(arr)) => {
                    for v in arr {
                        if let Some(s) = v.as_str() {
                            out.push(substitute(s, ctx));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// Returns (jvm_args, game_args).
pub fn resolve_arguments(client_json: &ClientJson, ctx: &LaunchContext) -> (Vec<String>, Vec<String>) {
    if let Some(args) = &client_json.arguments {
        let jvm = resolve_argument_entries(&args.jvm, ctx);
        let game = resolve_argument_entries(&args.game, ctx);
        (jvm, game)
    } else if let Some(legacy) = &client_json.legacy_arguments {
        // Pre-1.13: no separate jvm array, just a flat game-args string.
        // We add the same minimal jvm baseline the launcher already sets
        // (classpath, natives dir) via launcher.rs, so return that part empty here.
        let game = legacy
            .split_whitespace()
            .map(|s| substitute(s, ctx))
            .collect();
        (vec![], game)
    } else {
        (vec![], vec![])
    }
}
/// used by vanilla UI, lang files, etc.) into the shared assets dir.
/// This mirrors ~1-2 GB the first time, then is skip-if-present after.
/// Downloads the asset index + every referenced asset (sounds, textures
/// used by vanilla UI, lang files, etc.) into the shared assets dir.
/// This mirrors ~1-2 GB the first time, then is skip-if-present after.
pub async fn download_assets(client_json: &ClientJson) -> Result<(), String> {
    let assets_dir = root_dir().join("assets");
    let indexes_dir = assets_dir.join("indexes");
    let objects_dir = assets_dir.join("objects");
    std::fs::create_dir_all(&indexes_dir).map_err(|e| e.to_string())?;

    let index_bytes = reqwest::get(&client_json.asset_index.url)
        .await
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    std::fs::write(
        indexes_dir.join(format!("{}.json", client_json.asset_index.id)),
        &index_bytes,
    )
    .map_err(|e| e.to_string())?;

    #[derive(Deserialize)]
    struct AssetIndex {
        objects: std::collections::HashMap<String, AssetObject>,
    }
    #[derive(Deserialize)]
    struct AssetObject {
        hash: String,
    }

    let index: AssetIndex = serde_json::from_slice(&index_bytes).map_err(|e| e.to_string())?;

    // TODO: parallelize this (currently sequential -- fine for a re-launch
    // where most assets already exist, slow on a completely fresh install).
    for (_name, obj) in index.objects {
        let hash_prefix = &obj.hash[0..2];
        let dest = objects_dir.join(hash_prefix).join(&obj.hash);
        let url = format!(
            "https://resources.download.minecraft.net/{hash_prefix}/{}",
            obj.hash
        );
        download_verified(&url, &dest, &obj.hash).await?;
    }

    Ok(())
}
