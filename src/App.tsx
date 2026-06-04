import { useState, useEffect, useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import LauncherWindow from "./components/launcher/LauncherWindow";
import SettingsWindow from "./components/settings/SettingsWindow";
import * as api from "./lib/tauri";
import type { Settings } from "./lib/types";

function applyThemeClass(theme: string) {
  const root = document.documentElement;
  if (theme === "dark") {
    root.classList.add("dark");
  } else if (theme === "light") {
    root.classList.remove("dark");
  } else {
    if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
      root.classList.add("dark");
    } else {
      root.classList.remove("dark");
    }
  }
}

function App() {
  const [windowLabel, setWindowLabel] = useState<string | null>(null);
  const [theme, setTheme] = useState<string>("system");

  useEffect(() => {
    setWindowLabel(getCurrentWindow().label);
  }, []);

  useEffect(() => {
    api.getSettings().then((s) => {
      setTheme(s.theme);
      applyThemeClass(s.theme);
    }).catch(() => {
      applyThemeClass("system");
    });
  }, []);

  useEffect(() => {
    applyThemeClass(theme);
  }, [theme]);

  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => {
      if (theme === "system") applyThemeClass("system");
    };
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme]);

  const handleThemeChange = useCallback((newTheme: string) => {
    setTheme(newTheme);
    applyThemeClass(newTheme);
    // Mica/Acrylic の角が正しいテーマで表示されるよう再適用
    const isDark = newTheme === "dark"
      || (newTheme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
    api.setVibracyTheme(isDark).catch(() => {});
  }, []);

  // ランチャーウィンドウ側でもテーマ変更を受け取る
  useEffect(() => {
    const unlisten = listen<Settings>("settings-changed", (event) => {
      handleThemeChange(event.payload.theme ?? "system");
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [handleThemeChange]);

  if (windowLabel === null) return null;

  return (
    <div className="w-full h-full">
      {windowLabel === "settings" ? (
        <SettingsWindow onThemeChange={handleThemeChange} />
      ) : (
        <LauncherWindow />
      )}
    </div>
  );
}

export default App;
