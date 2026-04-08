import { useState, useEffect, useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import LauncherWindow from "./components/launcher/LauncherWindow";
import SettingsWindow from "./components/settings/SettingsWindow";
import * as api from "./lib/tauri";

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
  }, []);

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
