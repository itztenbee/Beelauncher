# Beelauncher

Grundgerüst für einen eigenen Minecraft-Launcher im Stil von NoRisk/Lunar/Feather.
Tech-Stack: Tauri 2 (Rust-Backend) + React (Frontend).

**Status: Skelett.** Login und Launch sind als Struktur mit TODOs angelegt,
nicht fertig funktionsfähig. Siehe unten für die Reihenfolge, in der das
sinnvoll weitergebaut wird.

## Lokal starten

Voraussetzungen (bei dir installieren, nicht in dieser Sandbox verfügbar):
- Rust + Cargo (https://rustup.rs)
- Node.js (hast du schon, v22)
- Tauri-Systemabhängigkeiten für dein OS: https://v2.tauri.app/start/prerequisites/

```bash
npm install
npm run tauri dev
```

Das öffnet das Launcher-Fenster mit der Mock-Oberfläche (Sidebar, Profile,
Play-Button). Der Play-Button ruft aktuell `launch_profile` auf, was noch
`Err("not implemented")` zurückgibt -- das ist erwartet.

## Reihenfolge zum Weiterbauen

1. **Launch-Pipeline ohne Login** (`src-tauri/src/launcher.rs`)
   Erstmal Vanilla + Fabric mit einem *offline*/gecrackten Testaccount lokal
   zum Laufen bringen (TODO 1-9 in der Datei). Das ist der Kern und komplett
   unabhängig vom Microsoft-Login -- damit kannst du sofort testen, ohne auf
   die Azure-Freigabe zu warten.

2. **Microsoft-Login** (`src-tauri/src/auth.rs`)
   Braucht zuerst eine Azure-App-Registrierung (portal.azure.com) und danach
   die Freigabe für die Minecraft-API über https://aka.ms/AppRegInfo.
   **Das ist der einzige Schritt, den wir nicht beschleunigen können** --
   plane das früh ein, damit die Wartezeit nicht am Ende blockiert.

3. **Mod-Management**
   BeeClient + weitere Mods automatisch in `.minecraft/mods/` legen,
   an/abschaltbar über die UI.

4. **Cosmetics/HUD**
   Das ist eigentlich Server-Infrastruktur (du brauchst einen Server, der
   Capes/Cosmetics ausliefert) + Client-seitiges Rendering -- separates
   Projekt für später, baut nicht direkt auf dem Launcher auf.

## Lizenz

MIT, wie BeeClient auch. Kein Code aus dem NoRisk-Client-Repo übernommen
(das steht unter GPL-3.0 und würde uns zwingen, alles offenzulegen) --
PrismLauncher wird oben nur als Referenz zum *Nachlesen* des Ablaufs
verlinkt, nicht zum Kopieren.
