import type { AppEntry } from "../../lib/types";
import ResultItem from "./ResultItem";

interface ResultsListProps {
  apps: AppEntry[];
  selectedIndex: number;
  onLaunch: (app: AppEntry) => void;
  onExclude: (app: AppEntry) => void;
  onSetTags: (app: AppEntry, tags: string[]) => void;
  onPin: (appId: string) => void;
  pinnedIds: string[];
  maxPinned: number;
  onSelect: (index: number) => void;
}

function ResultsList({ apps, selectedIndex, onLaunch, onExclude, onSetTags, onPin, pinnedIds, maxPinned, onSelect }: ResultsListProps) {
  if (apps.length === 0) {
    return (
      <div className="px-5 py-8 text-center text-sm text-gray-400 dark:text-gray-500">
        見つかりません
      </div>
    );
  }

  return (
    <div className="max-h-[320px] overflow-y-auto">
      {apps.map((app, index) => (
        <ResultItem
          key={app.id}
          app={app}
          isSelected={index === selectedIndex}
          onLaunch={onLaunch}
          onExclude={onExclude}
          onSetTags={onSetTags}
          onPin={onPin}
          pinnedIds={pinnedIds}
          maxPinned={maxPinned}
          onMouseEnter={() => onSelect(index)}
        />
      ))}
    </div>
  );
}

export default ResultsList;
