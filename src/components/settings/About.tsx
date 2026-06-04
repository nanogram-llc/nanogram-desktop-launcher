import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { open } from "@tauri-apps/plugin-shell";

const PRODUCT_URL = "https://nanogram.jp/products/1775662482.php";

function About() {
  const [version, setVersion] = useState<string>("");

  useEffect(() => {
    getVersion()
      .then((v) => setVersion(v))
      .catch(() => setVersion(""));
  }, []);

  const openUrl = (url: string) => {
    open(url).catch(console.error);
  };

  return (
    <div className="space-y-6 max-w-xl">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          このアプリについて
        </h2>
      </div>

      <div className="space-y-1">
        <div className="text-xl font-semibold text-gray-900 dark:text-gray-100">
          nanogram desktop LauncheR
        </div>
        {version && (
          <div className="text-sm text-gray-500 dark:text-gray-400">
            Version {version}
          </div>
        )}
      </div>

      <div className="text-sm text-gray-700 dark:text-gray-300 leading-relaxed">
        Ctrl 連打で呼び出せる、シンプルで軽量なアプリランチャーです。
        フォルダごとコピーするだけで別環境に移行できるポータブル設計。
      </div>

      <div className="border-t border-gray-200 dark:border-gray-700 pt-4 space-y-3">
        <div>
          <div className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-1">
            マニュアル / 最新版ダウンロード
          </div>
          <button
            onClick={() => openUrl(PRODUCT_URL)}
            className="text-sm text-blue-600 dark:text-blue-400 hover:underline break-all text-left"
          >
            {PRODUCT_URL}
          </button>
        </div>
      </div>

      <div className="border-t border-gray-200 dark:border-gray-700 pt-4 space-y-2">
        <div className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
          クレジット
        </div>
        <div className="text-sm text-gray-700 dark:text-gray-300">
          Copyright © 2026{" "}
          <button
            onClick={() => openUrl("https://nanogram.jp/")}
            className="text-blue-600 dark:text-blue-400 hover:underline"
          >
            nanogram, llc.
          </button>
        </div>
        <div className="text-xs text-gray-500 dark:text-gray-400 leading-relaxed pt-1">
          Built with Tauri, React, TypeScript and Rust.
        </div>
      </div>
    </div>
  );
}

export default About;
