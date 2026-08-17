import { useEffect, useRef, useState } from "react";
import type { Template } from "../types";

/// A sidebar template entry. Beyond selecting, a template can be renamed
/// (double-click, or the pencil affordance on hover) and deleted — without
/// those, "+" could only ever accumulate permanently-named entries that
/// nothing in the UI could remove.
///
/// Deletion confirms inline rather than through `window.confirm`: a native
/// modal blocks the webview's event loop, and an in-DOM confirm stays
/// reachable to the UI-automation used to test this app.
export function TemplateRow({
  template,
  selected,
  onSelect,
  onRename,
  onDelete,
}: {
  template: Template;
  selected: boolean;
  onSelect: () => void;
  onRename: (name: string) => void;
  onDelete: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(template.name);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) inputRef.current?.select();
  }, [editing]);

  const commitRename = () => {
    const next = draft.trim();
    // An empty name would render an unclickable blank row — treat it as a
    // cancel rather than persisting something unrecoverable.
    if (next && next !== template.name) onRename(next);
    else setDraft(template.name);
    setEditing(false);
  };

  if (editing) {
    return (
      <input
        ref={inputRef}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commitRename}
        onKeyDown={(e) => {
          if (e.key === "Enter") commitRename();
          if (e.key === "Escape") {
            setDraft(template.name);
            setEditing(false);
          }
        }}
        className="px-3 py-2 rounded-md text-[14px] border border-[var(--accent)] outline-none bg-[var(--surface)]"
        aria-label="Template name"
      />
    );
  }

  if (confirmingDelete) {
    return (
      <div className="px-3 py-2 rounded-md bg-[var(--danger-bg)] flex flex-col gap-1.5">
        <span className="text-[12px] text-[var(--danger)]">Delete “{template.name}”?</span>
        <div className="flex gap-1.5">
          <button
            onClick={() => {
              setConfirmingDelete(false);
              onDelete();
            }}
            className="text-[12px] px-2 py-1 rounded-sm bg-[var(--danger)] text-[var(--text-on-accent)] hover:opacity-90"
          >
            Delete
          </button>
          <button
            onClick={() => setConfirmingDelete(false)}
            className="text-[12px] px-2 py-1 rounded-sm border border-[var(--border)] bg-[var(--surface)] hover:bg-[var(--hover)]"
          >
            Cancel
          </button>
        </div>
      </div>
    );
  }

  return (
    <div
      className={`group flex items-center rounded-md ${
        selected ? "bg-[var(--surface)] shadow-sm" : "hover:bg-[var(--hover)]"
      }`}
    >
      <button
        onClick={onSelect}
        onDoubleClick={() => {
          setDraft(template.name);
          setEditing(true);
        }}
        className={`flex-1 min-w-0 text-left px-3 py-2 text-[14px] truncate ${
          selected ? "font-medium" : ""
        }`}
        title={`${template.name} (double-click to rename)`}
      >
        {template.name}
      </button>
      <div className="flex items-center opacity-0 group-hover:opacity-100 focus-within:opacity-100 pr-1.5">
        <button
          onClick={() => {
            setDraft(template.name);
            setEditing(true);
          }}
          className="px-1 text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
          aria-label={`Rename ${template.name}`}
          title="Rename"
        >
          ✎
        </button>
        <button
          onClick={() => setConfirmingDelete(true)}
          className="px-1 text-[11px] text-[var(--text-tertiary)] hover:text-[var(--danger)]"
          aria-label={`Delete ${template.name}`}
          title="Delete"
        >
          ✕
        </button>
      </div>
    </div>
  );
}
