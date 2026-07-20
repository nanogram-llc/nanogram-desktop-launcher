import { useState, useEffect, useCallback, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import LauncherWindow from "./components/launcher/LauncherWindow";
import SettingsWindow from "./components/settings/SettingsWindow";
import * as api from "./lib/tauri";
import { applyTheme, isDarkTheme } from "./lib/theme";
import type { Settings } from "./lib/types";

function App() {
  const [windowLabel, setWindowLabel] = useState<string | null>(null);
  const settingsRef = useRef<Settings | null>(null);
  // Bumped whenever the theme changes so LauncherWindow re-reads the background.
  const [themeVersion, setThemeVersion] = useState(0);

  useEffect(() => {
    setWindowLabel(getCurrentWindow().label);
  }, []);

  const applyAll = useCallback((s: Settings) => {
    settingsRef.current = s;
    applyTheme(s);
    setThemeVersion((v) => v + 1);
    // Mica/Acrylic composites the desktop behind the webview, which would show
    // through a background image or a reduced-opacity surface — turn the native
    // backdrop off in those modes and let CSS paint instead.
    const plain = !!s.background_image || (s.window_opacity ?? 100) < 100;
    api.setVibracyTheme(isDarkTheme(s.theme), plain).catch(() => {});
  }, []);

  useEffect(() => {
    api
      .getSettings()
      .then(applyAll)
      .catch(() => {
        document.documentElement.classList.toggle(
          "dark",
          window.matchMedia("(prefers-color-scheme: dark)").matches
        );
      });
  }, [applyAll]);

  // Follow the OS theme while "system" is selected.
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => {
      const s = settingsRef.current;
      if (s && s.theme === "system") applyAll(s);
    };
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [applyAll]);

  // Live updates from the settings window.
  useEffect(() => {
    const unlisten = listen<Settings>("settings-changed", (event) => {
      applyAll(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [applyAll]);

  // The settings window still drives theme changes through this callback; it
  // re-reads the full settings so accent/opacity stay in sync with the theme.
  const handleThemeChange = useCallback(() => {
    api.getSettings().then(applyAll).catch(() => {});
  }, [applyAll]);

  if (windowLabel === null) return null;

  return (
    <div className="w-full h-full">
      {windowLabel === "settings" ? (
        <SettingsWindow onThemeChange={handleThemeChange} />
      ) : (
        <LauncherWindow themeVersion={themeVersion} />
      )}
    </div>
  );
}

export default App;
