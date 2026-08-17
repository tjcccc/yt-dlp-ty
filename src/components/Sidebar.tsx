import { useStore } from "../state/store";
import { TemplateRow } from "./TemplateRow";

// Navigation-layer glass per spec/ui.md: a translucent, blurred panel that
// sits above the (opaque) content area — never the reverse.
export function Sidebar({
  onOpenConfig,
  onTemplateActivated,
}: {
  onOpenConfig: () => void;
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
    <aside
      data-tauri-drag-region
      className="w-56 shrink-0 h-full flex flex-col gap-3 px-3 pb-4 pt-9 bg-[var(--glass-nav)] backdrop-blur-xl border-r border-[var(--border)]"
    >
      <h1 className="text-[20px] font-semibold px-2">yt-dlp-ty</h1>

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

      <button
        onClick={onOpenConfig}
        className="text-[13px] px-3 py-2 rounded-md border border-[var(--border)] bg-[var(--surface)]/70 hover:bg-[var(--surface)]"
      >
        config
      </button>
    </aside>
  );
}
