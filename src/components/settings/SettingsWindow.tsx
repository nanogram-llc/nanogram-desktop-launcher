import { useState } from "react";
import GeneralSettings from "./GeneralSettings";
import FolderManager from "./FolderManager";
import AppTable from "./AppTable";
import ItemTable from "./ItemTable";

interface SettingsWindowProps {
  onThemeChange: (theme: string) => void;
}

type Tab = "general" | "folders" | "apps" | "files" | "registered_folders";

function SettingsWindow({ onThemeChange }: SettingsWindowProps) {
  const [tab, setTab] = useState<Tab>("general");

  const tabs: { id: Tab; label: string }[] = [
    { id: "general", label: "一般" },
    { id: "folders", label: "スキャンフォルダ" },
    { id: "apps", label: "アプリ" },
    { id: "files", label: "ファイル" },
    { id: "registered_folders", label: "フォルダ" },
  ];

  return (
    <div className="w-full h-full bg-white dark:bg-gray-900 flex flex-col">
      <div className="flex flex-1 min-h-0">
        {/* Sidebar */}
        <div className="w-40 border-r border-gray-200/50 dark:border-gray-700/50 p-2 shrink-0">
          {tabs.map((t) => (
            <button
              key={t.id}
              onClick={() => setTab(t.id)}
              className={`w-full text-left px-3 py-2 text-sm rounded-lg transition-colors mb-0.5 ${
                tab === t.id
                  ? "bg-blue-500/10 text-blue-600 dark:text-blue-400 font-medium"
                  : "text-gray-600 dark:text-gray-400 hover:bg-gray-500/10"
              }`}
            >
              {t.label}
            </button>
          ))}
        </div>

        {/* Content */}
        <div className="flex-1 p-5 overflow-y-auto">
          {tab === "general" && <GeneralSettings onThemeChange={onThemeChange} />}
          {tab === "folders" && <FolderManager />}
          {tab === "apps" && <AppTable />}
          {tab === "files" && <ItemTable itemType="file" label="ファイル" dropHint="ファイルをドロップして登録" />}
          {tab === "registered_folders" && <ItemTable itemType="folder" label="フォルダ" dropHint="フォルダをドロップして登録" />}
        </div>
      </div>
    </div>
  );
}

export default SettingsWindow;
