import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const NAV_ITEMS = ["Home", "Profile", "Mods", "Einstellungen"];
const loaderName = (profile) => (profile?.loader?.fabric ? "Fabric" : "Vanilla");
const modFilename = (url) => { try { return decodeURIComponent(new URL(url).pathname.split("/").pop()) || "mod.jar"; } catch { return url.split("/").pop() || "mod.jar"; } };

export default function App() {
  const [activeTab, setActiveTab] = useState("Home");
  const [account, setAccount] = useState(null);
  const [profiles, setProfiles] = useState([]);
  const [selectedProfile, setSelectedProfile] = useState(null);
  const [detailOpen, setDetailOpen] = useState(false);
  const [launching, setLaunching] = useState(false);
  const [status, setStatus] = useState("Bereit zum Starten");
  const [logLines, setLogLines] = useState([]);
  const [showConsole, setShowConsole] = useState(false);
  const [modUrl, setModUrl] = useState("");
  const [modError, setModError] = useState("");
  const logEndRef = useRef(null);
  const currentProfile = profiles.find((p) => p.id === selectedProfile) ?? null;

  function refreshProfiles() { return invoke("list_profiles").then((items) => { setProfiles(items); return items; }); }
  useEffect(() => { refreshProfiles().then((items) => items[0] && setSelectedProfile(items[0].id)); invoke("get_account").then((result) => result && setAccount(result)); }, []);
  useEffect(() => { const a = listen("launch-progress", (e) => setStatus(e.payload.detail)); const b = listen("game-log", (e) => setLogLines((x) => [...x.slice(-500), e.payload])); return () => { a.then((f) => f()); b.then((f) => f()); }; }, []);
  useEffect(() => logEndRef.current?.scrollIntoView({ behavior: "smooth" }), [logLines]);

  function selectTab(tab) { setActiveTab(tab); setDetailOpen(false); }
  function openProfile(id) { setSelectedProfile(id); setActiveTab("Profile"); setDetailOpen(true); setModError(""); }
  async function handleLogin() { try { setStatus("Microsoft-Anmeldung wird vorbereitet …"); setAccount(await invoke("microsoft_login")); } catch (error) { setStatus(`Login fehlgeschlagen: ${error}`); } }
  async function handleLaunch() { if (!selectedProfile) return; setLaunching(true); setLogLines([]); setShowConsole(true); setStatus("Starte Minecraft …"); try { await invoke("launch_profile", { profileId: selectedProfile }); } catch (error) { setStatus(`Fehler: ${error}`); } finally { setLaunching(false); } }
  async function toggleMod(filename, enabled) { if (!currentProfile) return; await invoke("toggle_mod", { profileId: currentProfile.id, filename, enabled }); refreshProfiles(); }
  async function addMod(event) { event.preventDefault(); if (!currentProfile || !modUrl.trim()) return; try { const url = modUrl.trim(); await invoke("add_mod", { profileId: currentProfile.id, filename: modFilename(url), url }); setModUrl(""); setModError(""); await refreshProfiles(); } catch (error) { setModError(`Mod konnte nicht hinzugefügt werden: ${error}`); } }
  async function saveProfile(patch) { if (!currentProfile) return; await invoke("save_profile", { profile: { ...currentProfile, ...patch } }); refreshProfiles(); }

  const profileDetail = currentProfile && <>
    <section className="profile-hero">
      <div className="profile-emblem">{currentProfile.name.slice(0, 2).toUpperCase()}</div>
      <div className="profile-hero-copy"><span className="eyebrow">SPIELPROFIL · {loaderName(currentProfile).toUpperCase()}</span><h1>{currentProfile.name}</h1><p>Minecraft {currentProfile.minecraft_version} · {currentProfile.memory_mb / 1024} GB Arbeitsspeicher</p><div className="tag-row"><span className="tag tag-accent">{loaderName(currentProfile)}</span><span className="tag">{currentProfile.mods.length} Mods</span><span className="tag">{currentProfile.minecraft_version}</span></div></div>
      <button className="btn-play large" disabled={launching} onClick={handleLaunch}>▶ {launching ? "STARTET …" : "SPIELEN"}</button>
    </section>
    <section className="content-grid">
      <div className="panel mods-panel"><div className="panel-heading"><div><span className="eyebrow">TEXTUREPACKS / MODS</span><h2>Mods in diesem Profil</h2></div><span className="count-badge">{currentProfile.mods.length}</span></div>
        <form className="add-mod-form" onSubmit={addMod}><input value={modUrl} onChange={(e) => setModUrl(e.target.value)} placeholder="Direkte Download-URL zu einer .jar-Datei" /><button className="btn-gradient" type="submit">+ MOD HINZUFÜGEN</button></form>{modError && <p className="form-error">{modError}</p>}
        <div className="mod-list modern">{currentProfile.mods.length === 0 ? <div className="empty-state">Noch keine Mods. Füge oben eine direkte .jar-Download-URL hinzu.</div> : currentProfile.mods.map((mod) => <div className="mod-row" key={mod.filename}><div className="mod-icon">M</div><div className="mod-details"><strong>{mod.filename}</strong><span>{mod.enabled ? "Aktiv im nächsten Start" : "Deaktiviert"}</span></div><label className="switch"><input type="checkbox" checked={mod.enabled} onChange={(e) => toggleMod(mod.filename, e.target.checked)} /><span /></label></div>)}</div>
      </div>
      <div className="panel profile-facts"><span className="eyebrow">PROFILSTATUS</span><h2>Bereit zum Spielen</h2><div className="fact"><span>Loader</span><b>{loaderName(currentProfile)}</b></div><div className="fact"><span>Version</span><b>{currentProfile.minecraft_version}</b></div><div className="fact"><span>RAM</span><b>{currentProfile.memory_mb} MB</b></div><button className="text-button" onClick={() => { setDetailOpen(false); setActiveTab("Einstellungen"); }}>Profil-Einstellungen →</button></div>
    </section>
  </>;

  return <div className="app-shell"><header className="topbar"><button className="brand" onClick={() => selectTab("Home")}><span>Bee</span>launcher</button><nav>{NAV_ITEMS.map((tab) => <button key={tab} className={activeTab === tab ? "nav-active" : ""} onClick={() => selectTab(tab)}>{tab}</button>)}</nav><div className="account-area">{account ? <><span className="online-dot" /> {account.name}</> : <button className="login-button" onClick={handleLogin}>Microsoft anmelden</button>}</div></header>
    <main className="main-content">
      {(activeTab === "Home" || (activeTab === "Profile" && !detailOpen)) && <><section className="hero"><div><span className="eyebrow">BEELAUNCHER · MINECRAFT</span><h1>Deine Welt.<br /><em>Dein Profil.</em></h1><p>Starte Minecraft mit deinem persönlichen Setup, deinen Mods und deiner bevorzugten Version.</p><button className="btn-gradient" onClick={() => currentProfile && openProfile(currentProfile.id)}>PROFIL ÖFFNEN →</button></div><div className="hero-mark">BL<span>_</span></div></section><div className="section-heading"><div><span className="eyebrow">PROFILE</span><h2>Wähle dein Setup</h2></div><button className="text-button" onClick={() => selectTab("Profile")}>Alle Profile →</button></div><section className="profile-grid">{profiles.map((profile) => <button className={`profile-card ${selectedProfile === profile.id ? "selected" : ""}`} key={profile.id} onClick={() => openProfile(profile.id)}><div className="card-top"><span className="profile-card-icon">{profile.name.slice(0, 1)}</span><span className="tag">{loaderName(profile)}</span></div><h3>{profile.name}</h3><p>Minecraft {profile.minecraft_version}</p><div className="card-footer"><span>{profile.mods.length} Mods</span><span>Öffnen →</span></div></button>)}</section></>}
      {activeTab === "Profile" && detailOpen && <><button className="back-button" onClick={() => setDetailOpen(false)}>← ZURÜCK ZU PROFILEN</button>{profileDetail}</>}
      {activeTab === "Mods" && <>{!currentProfile ? <p className="empty-state">Wähle zuerst ein Profil aus.</p> : <><button className="back-button" onClick={() => openProfile(currentProfile.id)}>← ZUM PROFIL</button>{profileDetail}</>}</>}
      {activeTab === "Einstellungen" && currentProfile && <section className="panel settings-panel"><span className="eyebrow">EINSTELLUNGEN · {currentProfile.name.toUpperCase()}</span><h1>Profil-Einstellungen</h1><label>Arbeitsspeicher <b>{currentProfile.memory_mb} MB</b><input type="range" min="1024" max="16384" step="512" value={currentProfile.memory_mb} onChange={(e) => saveProfile({ memory_mb: Number(e.target.value) })} /></label><label>Zusätzliche Java-Argumente<input type="text" defaultValue={currentProfile.java_args.join(" ")} onBlur={(e) => saveProfile({ java_args: e.target.value.split(" ").filter(Boolean) })} placeholder="-XX:+UseG1GC …" /></label></section>}
      {activeTab === "Einstellungen" && !currentProfile && <p className="empty-state">Kein Profil ausgewählt.</p>}
      <section className="launch-dock"><div><span className="status-dot" /> {currentProfile ? `${currentProfile.name} · ${status}` : status}</div><button className="btn-play" disabled={launching || !selectedProfile} onClick={handleLaunch}>▶ {launching ? "STARTET" : "SPIELEN"}</button><button className="console-button" onClick={() => setShowConsole((v) => !v)}>{showConsole ? "Konsole schließen" : "Konsole"}</button></section>
      {showConsole && <section className="console">{logLines.length === 0 && <div>Warte auf Ausgabe …</div>}{logLines.map((line, index) => <div key={index}>{line}</div>)}<div ref={logEndRef} /></section>}
    </main></div>;
}
