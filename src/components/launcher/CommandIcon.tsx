import type { Shell } from "../../lib/tauri";

export function parseCommandExePath(exePath: string): {
  cmd: string;
  admin: boolean;
  keep_open: boolean;
  shell: Shell;
} | null {
  const match = exePath.match(/^cmd:\/\/[^|]+\|(.+)$/);
  if (!match) return null;
  try {
    const data = JSON.parse(match[1]);
    return {
      cmd: String(data.cmd ?? ""),
      admin: !!data.admin,
      keep_open: data.keep_open !== false,
      shell: data.shell === "pwsh" ? "pwsh" : "cmd",
    };
  } catch {
    return null;
  }
}

interface Props {
  shell: Shell;
  size?: "sm" | "lg";
}

export default function CommandIcon({ shell, size = "lg" }: Props) {
  const wrapperSize = size === "sm" ? "w-8 h-8 rounded-md" : "w-10 h-10 rounded-lg";
  const isPwsh = shell === "pwsh";
  const bg = isPwsh ? "bg-[#012456]" : "bg-gray-900";
  const label = isPwsh ? "PS" : ">_";
  const labelClass =
    size === "sm"
      ? "text-[10px] font-mono font-semibold tracking-tight"
      : "text-xs font-mono font-semibold tracking-tight";
  return (
    <div
      className={`${wrapperSize} ${bg} flex items-center justify-center text-white shrink-0 pointer-events-none`}
    >
      <span className={labelClass}>{label}</span>
    </div>
  );
}
