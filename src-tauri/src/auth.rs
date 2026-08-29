// Microsoft -> Xbox Live -> XSTS -> Minecraft auth chain.
//
// IMPORTANT: none of this will return a usable Minecraft token until your
// Azure app has been approved for the Minecraft Services API. Until then,
// `login_in_minecraft` below will 403. Register your app at
// https://portal.azure.com (App registrations -> New registration,
// "Personal Microsoft accounts only"), then apply for API access at
// https://aka.ms/AppRegInfo before testing this flow for real.
//
// This uses the OAuth 2.0 *device code* flow, which is the simplest to
// implement in a desktop app (no local redirect server needed): the user
// gets a short code, opens a URL in their normal browser, and this backend
// polls Microsoft until they finish.

use crate::AccountInfo;
use serde::Deserialize;
use std::time::Duration;

// TODO: replace with your own Azure app's client ID once registered.
const CLIENT_ID: &str = "REPLACE_WITH_YOUR_AZURE_CLIENT_ID";

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: u64,
    expires_in: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct XboxLiveResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: XboxDisplayClaims,
}

#[derive(Deserialize)]
struct XboxDisplayClaims {
    xui: Vec<XuiEntry>,
}

#[derive(Deserialize)]
struct XuiEntry {
    uhs: String,
}

#[derive(Deserialize)]
struct MinecraftAuthResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct MinecraftProfile {
    id: String,
    name: String,
}

pub async fn login_with_microsoft() -> Result<AccountInfo, String> {
    let client = reqwest::Client::new();

    // 1. Request a device code from Microsoft.
    let device_res: DeviceCodeResponse = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[
            ("client_id", CLIENT_ID),
            ("scope", "XboxLive.signin offline_access"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    // TODO: surface this to the frontend instead of just printing it --
    // e.g. emit a Tauri event so the UI can show "go to X and enter code Y".
    println!(
        "Open {} and enter code: {}",
        device_res.verification_uri, device_res.user_code
    );

    // 2. Poll until the user finishes logging in, or we time out.
    let deadline = std::time::Instant::now() + Duration::from_secs(device_res.expires_in);
    let ms_token: TokenResponse = loop {
        if std::time::Instant::now() > deadline {
            return Err("Login-Zeit abgelaufen".into());
        }
        tokio::time::sleep(Duration::from_secs(device_res.interval)).await;

        let res = client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&[
                ("client_id", CLIENT_ID),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", &device_res.device_code),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status().is_success() {
            break res.json().await.map_err(|e| e.to_string())?;
        }
        // still pending (authorization_pending) -- keep polling
    };

    // 3. Exchange the MS token for an Xbox Live token.
    let xbl: XboxLiveResponse = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&serde_json::json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", ms_token.access_token)
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let uhs = xbl.display_claims.xui[0].uhs.clone();

    // 4. Exchange the Xbox Live token for an XSTS token.
    let xsts: XboxLiveResponse = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl.token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    // 5. Exchange the XSTS token for a Minecraft access token.
    let mc_auth: MinecraftAuthResponse = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&serde_json::json!({
            "identityToken": format!("XBL3.0 x={};{}", uhs, xsts.token)
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    // TODO: check game ownership via
    // GET https://api.minecraftservices.com/entitlements/mcstore
    // before trying to fetch the profile below.

    // 6. Fetch the Minecraft profile (name + UUID).
    let profile: MinecraftProfile = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc_auth.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    // TODO: persist mc_auth.access_token + refresh_token to disk
    // (encrypted, e.g. via the OS keychain) so the user doesn't have
    // to re-login every launch.
    save_account(&Account {
        name: profile.name.clone(),
        uuid: profile.id.clone(),
        access_token: mc_auth.access_token.clone(),
        refresh_token: ms_token.refresh_token.clone(),
    })?;

    Ok(AccountInfo {
        name: profile.name,
        uuid: profile.id,
    })
}

/// The full account record kept on disk -- includes the access token,
/// unlike `AccountInfo` (which is what we hand back to the frontend).
/// NOTE: this is currently stored as plain JSON. Before shipping this to
/// anyone besides yourself for testing, swap this for the OS keychain
/// (e.g. the `keyring` crate) -- a plaintext access token on disk is fine
/// for local dev, not for a real release.
#[derive(Deserialize, serde::Serialize, Clone)]
pub struct Account {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
}

fn account_path() -> std::path::PathBuf {
    crate::profile::root_dir().join("account.json")
}

fn save_account(account: &Account) -> Result<(), String> {
    let dir = crate::profile::root_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(account).map_err(|e| e.to_string())?;
    std::fs::write(account_path(), raw).map_err(|e| e.to_string())
}

pub fn load_account() -> Option<Account> {
    let raw = std::fs::read_to_string(account_path()).ok()?;
    serde_json::from_str(&raw).ok()
}
