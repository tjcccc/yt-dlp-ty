import { useStore } from "../state/store";
import { TemplateRow } from "./TemplateRow";

// Navigation-layer glass per spec/ui.md: a translucent, blurred panel that
// sits above the (opaque) content area — never the reverse.
export function Sidebar({
  onOpenConfig,
  onOpenHistory,
  onTemplateActivated,
}: {
  onOpenConfig: () => void;
  onOpenHistory: () => void;
  /// Fired whenever the user picks or creates a template, so the shell can
  /// switch back to the main form (see App.tsx).
  onTemplateActivated: () => void;
}) {
  const templates = useStore((s) => s.templates);
  const selectedTemplateId = useStore((s) => s.selectedTemplateId);
  const selectTemplate = useStore((s) => s.selectTemplate);
  const addTemplate = useStore((s) => s.addTemplate);
  const updateTemplate = useStore((s) => s.updateTemplate);
  const deleteTemplate = useStore((s) => s.deleteTemplate);

  return (
    // pt-9 clears the traffic-light buttons: the window uses a transparent
    // title bar so the sidebar runs to the top edge (as in Finder/Mail), which
    // means content here would otherwise sit underneath them.
    // "deep" rather than a bare drag region: Tauri only starts a drag from a
    // bare region when the click lands on that exact element, and this panel
    // is almost entirely covered by its own children — so only the hairline
    // gaps between rows were draggable. "deep" makes the whole subtree drag,
    // while Tauri still exempts buttons and inputs so they keep working.
    <aside
      data-tauri-drag-region="deep"
      className="w-56 shrink-0 h-full flex flex-col gap-3 px-3 pb-4 pt-9 bg-[var(--glass-nav)] backdrop-blur-xl border-r border-[var(--border)]"
    >
      {/* "ty" is "yt" backwards and stands for thank you — yt-dlp does the
          real work, this is a window onto it. Spelled out here rather than
          left as an initialism nobody can decode. */}
      <h1 className="text-[19px] font-semibold px-2 leading-tight">yt-dlp, Thank you!</h1>

      <div className="flex items-center justify-between pl-2 pr-1 mt-2">
        <span className="text-[12px] text-[var(--text-tertiary)] uppercase tracking-wide">
          template
        </span>
        <button
          onClick={() => addTemplate().then(onTemplateActivated)}
          className="w-6 h-6 grid place-items-center rounded-sm text-[16px] leading-none text-[var(--text-secondary)] hover:bg-[var(--hover)] hover:text-[var(--text-primary)]"
          aria-label="Add template"
        >
          +
        </button>
      </div>

      <div className="flex flex-col gap-1 flex-1 overflow-y-auto">
        {templates.map((t) => (
          <TemplateRow
            key={t.id}
            template={t}
            selected={t.id === selectedTemplateId}
            onSelect={() => {
              selectTemplate(t.id);
              onTemplateActivated();
            }}
            onRename={(name) => updateTemplate(t.id, { name })}
            onDelete={() => deleteTemplate(t.id)}
          />
        ))}
        {templates.length === 0 && (
          <p className="text-[12px] text-[var(--text-tertiary)] px-2 py-2">
            No templates. Use + to add one.
          </p>
        )}
      </div>

      {/* Utility actions, not list entries. A filled surface here read as a
          *selected template* — exactly the treatment TemplateRow uses for
          selection — so they looked like extra, permanently-active items in
          the list above. Transparent by default, tinting only on hover, each
          with an icon to separate it from the names at a glance.

          Grouped in their own tighter stack: they belong together, and at the
          sidebar's 12px rhythm they read as two unrelated items rather than
          one footer. */}
      <div className="flex flex-col gap-0.5">
        <button
          onClick={onOpenHistory}
          title="History"
          className="flex items-center gap-2 text-[13px] px-2.5 py-2 rounded-md text-[var(--text-secondary)] hover:bg-[var(--hover)] hover:text-[var(--text-primary)]"
        >
          <svg
            viewBox="0 0 16 16"
            aria-hidden="true"
            className="w-[15px] h-[15px] shrink-0"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.3"
          >
            {/* A clock: the one glyph that reads as "earlier" at this size. */}
            <circle cx="8" cy="8" r="6.1" />
            <path
              d="M8 4.4V8.2l2.5 1.6"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          History
        </button>

        <button
          onClick={onOpenConfig}
          title="Config"
          className="flex items-center gap-2 text-[13px] px-2.5 py-2 rounded-md text-[var(--text-secondary)] hover:bg-[var(--hover)] hover:text-[var(--text-primary)]"
        >
          <svg
            viewBox="0 0 16 16"
            aria-hidden="true"
            className="w-[15px] h-[15px] shrink-0"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.3"
          >
            {/* Sliders rather than a gear: an 8-tooth gear turns to mush at
              15px, while two tracks and two knobs stay legible and read
              just as clearly as "settings". */}
            <path d="M2 5h12M2 11h12" strokeLinecap="round" />
            <circle cx="6" cy="5" r="1.7" fill="var(--glass-nav)" />
            <circle cx="10.5" cy="11" r="1.7" fill="var(--glass-nav)" />
          </svg>
          Config
        </button>
      </div>
    </aside>
  );
}
