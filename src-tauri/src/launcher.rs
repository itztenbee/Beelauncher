use tauri::{Emitter, Window};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::auth;
use crate::fabric;
use crate::mojang;
use crate::profile::{self, Loader, Profile};

/// Emitted to the frontend during launch so the UI can show real progress
/// instead of a static "Startet..." label. Listen for "launch-progress" in
/// App.jsx via @tauri-apps/api/event.
#[derive(Clone, serde::Serialize)]
struct LaunchProgress {
    stage: String,
    detail: String,
}

fn emit_progress(window: &Window, stage: &str, detail: &str) {
    let _ = window.emit(
        "launch-progress",
        LaunchProgress {
            stage: stage.into(),
            detail: detail.into(),
        },
    );
}

pub async fn launch(window: Window, profile_id: &str) -> Result<(), String> {
    // 1. Load profile config.
    profile::ensure_default_profiles()?;
    let profile: Profile = profile::load_profile(profile_id)?;

    // 2 & 3. Version manifest + client.json + libraries + client jar.
    emit_progress(&window, "manifest", "Lade Versionsinfos...");
    let client_json = mojang::fetch_client_json(&profile.minecraft_version).await?;

    emit_progress(&window, "libraries", "Lade Bibliotheken...");
    let (mut classpath, natives_dir) =
        mojang::download_libraries_and_client(&client_json, &profile.minecraft_version).await?;

    // 5. Assets (textures, sounds, lang files).
    emit_progress(&window, "assets", "Lade Assets (kann beim ersten Mal dauern)...");
    mojang::download_assets(&client_json).await?;

    // 6. Fabric merge, if this profile uses it.
    let main_class = match &profile.loader {
        Loader::Vanilla => client_json.main_class.clone(),
        Loader::Fabric { loader_version } => {
            emit_progress(&window, "fabric", "Lade Fabric Loader...");
            let fabric_profile =
                fabric::fetch_fabric_profile(&profile.minecraft_version, loader_version).await?;
            let fabric_classpath = fabric::download_fabric_libraries(&fabric_profile).await?;
            classpath.extend(fabric_classpath);
            fabric_profile.main_class
        }
    };

    // 7. Make sure this profile's mods are in the instance's mods/ folder.
    let instance_dir = profile::root_dir().join("instances").join(&profile.id);
    let mods_dir = instance_dir.join("mods");
    std::fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;

    emit_progress(&window, "mods", "Prüfe Mods...");
    let mod_cache_dir = profile::root_dir().join("mod_cache");
    std::fs::create_dir_all(&mod_cache_dir).map_err(|e| e.to_string())?;

    for entry in profile.mods.iter().filter(|m| m.enabled) {
        let cached_path = mod_cache_dir.join(&entry.filename);
        if !cached_path.exists() {
            emit_progress(&window, "mods", &format!("Lade {}...", entry.filename));
            let bytes = reqwest::get(&entry.url)
                .await
                .map_err(|e| format!("Mod-Download fehlgeschlagen ({}): {e}", entry.filename))?
                .bytes()
                .await
                .map_err(|e| e.to_string())?;
            std::fs::write(&cached_path, &bytes).map_err(|e| e.to_string())?;
        }
        let dest = mods_dir.join(&entry.filename);
        if !dest.exists() {
            std::fs::copy(&cached_path, &dest).map_err(|e| e.to_string())?;
        }
    }

    // Remove jars from the instance mods/ dir that are no longer in the
    // (enabled) profile mod list -- keeps toggling a mod off actually take effect.
    let enabled_names: std::collections::HashSet<_> =
        profile.mods.iter().filter(|m| m.enabled).map(|m| m.filename.clone()).collect();
    if let Ok(entries) = std::fs::read_dir(&mods_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if !enabled_names.contains(&name) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    // 8. Build the java invocation.
    let account = auth::load_account();
    let (username, uuid, access_token) = match &account {
        Some(acc) => (acc.name.as_str(), acc.uuid.as_str(), acc.access_token.as_str()),
        None => {
            emit_progress(&window, "auth", "Kein Account eingeloggt -- offline-Modus (nur lokaler Test)");
            ("Player", "00000000-0000-0000-0000-000000000000", "-")
        }
    };

    let classpath_str = std::env::join_paths(&classpath)
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();

    let ctx = mojang::LaunchContext {
        classpath: &classpath_str,
        natives_dir: &natives_dir,
        game_dir: &instance_dir,
        assets_dir: &profile::root_dir().join("assets"),
        asset_index_id: &client_json.asset_index.id,
        minecraft_version: &profile.minecraft_version,
        username,
        uuid,
        access_token,
    };
    let (jvm_args, game_args) = mojang::resolve_arguments(&client_json, &ctx);

    let mut cmd = Command::new("java");
    cmd.arg(format!("-Xmx{}M", profile.memory_mb))
        .arg(format!("-Djava.library.path={}", natives_dir.display()));

    if jvm_args.is_empty() {
        // Pre-1.13 versions have no structured jvm args -- set the bare
        // minimum by hand so the classpath is still on the command line.
        cmd.arg("-cp").arg(&classpath_str);
    } else {
        cmd.args(&jvm_args);
    }

    cmd.args(&profile.java_args)
        .arg(&main_class)
        .args(&game_args);

    // 9. Spawn + stream output back to the UI.
    emit_progress(&window, "launching", "Starte Minecraft...");
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("Java-Start fehlgeschlagen: {e}"))?;

    if let Some(stdout) = child.stdout.take() {
        let window = window.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = window.emit("game-log", line);
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let window = window.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = window.emit("game-log", line);
            }
        });
    }

    emit_progress(&window, "running", "Minecraft läuft");
    Ok(())
}
