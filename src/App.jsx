import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const TABS = ["Home", "Mods", "Cosmetics", "Einstellungen"];

export default function App() {
  const [activeTab, setActiveTab] = useState("Home");
  const [account, setAccount] = useState(null);
  const [profiles, setProfiles] = useState([]);
  const [selectedProfile, setSelectedProfile] = useState(null);
  const [launching, setLaunching] = useState(false);
  const [status, setStatus] = useState("Bereit");
  const [logLines, setLogLines] = useState([]);
  const [showConsole, setShowConsole] = useState(false);
  const logEndRef = useRef(null);

  const currentProfile = profiles.find((p) => p.id === selectedProfile) ?? null;

  function refreshProfiles() {
    return invoke("list_profiles").then((p) => {
      setProfiles(p);
      return p;
    });
  }

  useEffect(() => {
    refreshProfiles().then((p) => {
      if (p.length > 0) setSelectedProfile(p[0].id);
    });

    invoke("get_account").then((acc) => {
      if (acc) setAccount(acc);
    });
  }, []);

  useEffect(() => {
    const unlistenProgress = listen("launch-progress", (event) => {
      setStatus(event.payload.detail);
    });
    const unlistenLog = listen("game-log", (event) => {
      setLogLines((prev) => [...prev.slice(-500), event.payload]);
    });
    return () => {
      unlistenProgress.then((f) => f());
      unlistenLog.then((f) => f());
    };
  }, []);

  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logLines]);

  async function handleLogin() {
    try {
      const result = await invoke("microsoft_login");
      setAccount(result);
    } catch (err) {
      setStatus(`Login fehlgeschlagen: ${err}`);
    }
  }

  async function handleLaunch() {
    if (!selectedProfile) return;
    setLaunching(true);
    setLogLines([]);
    setShowConsole(true);
    setStatus("Starte...");
    try {
      await invoke("launch_profile", { profileId: selectedProfile });
    } catch (err) {
      setStatus(`Fehler: ${err}`);
    } finally {
      setLaunching(false);
    }
  }

  async function handleToggleMod(filename, enabled) {
    if (!currentProfile) return;
    await invoke("toggle_mod", { profileId: currentProfile.id, filename, enabled });
    refreshProfiles();
  }

  async function handleAddMod() {
    if (!currentProfile) return;
    const url = window.prompt("Download-URL des Mod-Jars:");
    if (!url) return;
    const filename = url.split("/").pop() || "mod.jar";
    await invoke("add_mod", { profileId: currentProfile.id, filename, url });
    refreshProfiles();
  }

  async function handleMemoryChange(mb) {
    if (!currentProfile) return;
    const updated = { ...currentProfile, memory_mb: mb };
    await invoke("save_profile", { profile: updated });
    refreshProfiles();
  }

  async function handleJavaArgsChange(argsString) {
    if (!currentProfile) return;
    const updated = { ...currentProfile, java_args: argsString.split(" ").filter(Boolean) };
    await invoke("save_profile", { profile: updated });
    refreshProfiles();
  }

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">Beelauncher</div>
        {TABS.map((tab) => (
          <div
            key={tab}
            className={`nav-item ${activeTab === tab ? "active" : ""}`}
            onClick={() => setActiveTab(tab)}
          >
            {tab}
          </div>
        ))}
        <div className="account-box">
          {account ? (
            <div>Eingeloggt als {account.name}</div>
          ) : (
            <button className="btn-secondary" onClick={handleLogin}>
              Mit Microsoft anmelden
            </button>
          )}
        </div>
      </aside>

      <main className="main">
        <div className="panel-title">{activeTab}</div>

        {activeTab === "Home" && (
          <>
            {profiles.length === 0 && <p className="status-text">Lade Profile...</p>}
            <div className="profile-grid">
              {profiles.map((p) => (
                <div
                  key={p.id}
                  className="profile-card"
                  style={{
                    borderColor: selectedProfile === p.id ? "var(--accent)" : undefined,
                  }}
                  onClick={() => setSelectedProfile(p.id)}
                >
                  <h3>{p.name}</h3>
                  <p>
                    {p.loader?.fabric ? "Fabric" : "Vanilla"} · {p.minecraft_version}
                  </p>
                </div>
              ))}
            </div>

            <div className="launch-bar">
              <button
                className="btn-launch"
                disabled={launching || !selectedProfile}
                onClick={handleLaunch}
              >
                {launching ? "Startet..." : "Spielen"}
              </button>
              <div style={{ flex: 1 }}>
                <div className="status-text">{status}</div>
              </div>
              <button className="btn-secondary" onClick={() => setShowConsole((s) => !s)}>
                {showConsole ? "Konsole ausblenden" : "Konsole anzeigen"}
              </button>
            </div>

            {showConsole && (
              <div className="console">
                {logLines.length === 0 && <div className="status-text">Noch keine Ausgabe.</div>}
                {logLines.map((line, i) => (
                  <div key={i} className="console-line">
                    {line}
                  </div>
                ))}
                <div ref={logEndRef} />
              </div>
            )}
          </>
        )}

        {activeTab === "Mods" && (
          <>
            {!currentProfile && <p className="status-text">Kein Profil ausgewählt.</p>}
            {currentProfile && (
              <>
                <p className="status-text" style={{ marginBottom: 16 }}>
                  Mods für: {currentProfile.name}
                </p>
                <div className="mod-list">
                  {currentProfile.mods.length === 0 && (
                    <p className="status-text">Noch keine Mods hinzugefügt.</p>
                  )}
                  {currentProfile.mods.map((m) => (
                    <div className="mod-row" key={m.filename}>
                      <label className="mod-toggle">
                        <input
                          type="checkbox"
                          checked={m.enabled}
                          onChange={(e) => handleToggleMod(m.filename, e.target.checked)}
                        />
                        <span>{m.filename}</span>
                      </label>
                    </div>
                  ))}
                </div>
                <button className="btn-secondary" style={{ marginTop: 16 }} onClick={handleAddMod}>
                  + Mod per URL hinzufügen
                </button>
              </>
            )}
          </>
        )}

        {activeTab === "Einstellungen" && (
          <>
            {!currentProfile && <p className="status-text">Kein Profil ausgewählt.</p>}
            {currentProfile && (
              <div className="settings-form">
                <label className="settings-field">
                  <span>RAM: {currentProfile.memory_mb} MB</span>
                  <input
                    type="range"
                    min="1024"
                    max="16384"
                    step="512"
                    value={currentProfile.memory_mb}
                    onChange={(e) => handleMemoryChange(Number(e.target.value))}
                  />
                </label>
                <label className="settings-field">
                  <span>Zusätzliche Java-Argumente</span>
                  <input
                    type="text"
                    defaultValue={currentProfile.java_args.join(" ")}
                    onBlur={(e) => handleJavaArgsChange(e.target.value)}
                    placeholder="-XX:+UseG1GC ..."
                  />
                </label>
              </div>
            )}
          </>
        )}

        {activeTab === "Cosmetics" && (
          <p className="status-text">Noch nicht implementiert — braucht eigene Server-Infrastruktur.</p>
        )}
      </main>
    </div>
  );
}
