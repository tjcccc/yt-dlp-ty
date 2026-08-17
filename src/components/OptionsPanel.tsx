import type { OptionsMode } from "../types";

const OPTIONS: { mode: OptionsMode; label: string; disabled?: boolean }[] = [
  { mode: "chooseFormat", label: "Choose format first" },
  { mode: "bestVideo", label: "Best video" },
  { mode: "bestAudio", label: "Best audio" },
];

// The three toggles behave as one mutually-exclusive strategy, not
// independent booleans: turning one on turns the others off (clicking the
// active one again returns to "raw").
export function OptionsPanel({ mode, onChange }: { mode: OptionsMode; onChange: (mode: OptionsMode) => void }) {
  return (
    <div className="flex flex-col gap-3">
      <span className="text-[14px] font-medium">Options (override)</span>
      <div className="flex flex-col gap-2">
        {OPTIONS.map((opt) => {
          const on = mode === opt.mode;
          return (
            <div key={opt.mode} className="flex items-center justify-between gap-3">
              <span className={`text-[13px] ${opt.disabled ? "text-[var(--text-tertiary)]" : ""}`}>
                {opt.label}
                {opt.disabled && <span className="text-[var(--text-tertiary)]"> (coming soon)</span>}
              </span>
              <button
                type="button"
                disabled={opt.disabled}
                aria-pressed={on}
                onClick={() => onChange(on ? "raw" : opt.mode)}
                className={`w-10 h-6 rounded-full relative transition-colors ${
                  opt.disabled ? "bg-[var(--hover)] cursor-not-allowed" : on ? "bg-[var(--accent)]" : "bg-[var(--border-strong)]"
                }`}
              >
                <span
                  className={`absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-[var(--surface)] shadow transition-transform ${
                    on ? "translate-x-4" : ""
                  }`}
                />
              </button>
            </div>
          );
        })}
      </div>
      {mode !== "raw" && (
        <p className="text-[12px] text-[var(--text-tertiary)]">
          {mode === "chooseFormat"
            ? "Download will list each video's formats to pick from first. Any -f/--format flag in Parameters is replaced."
            : "A format option is on — any -f/--format flag already in Parameters is replaced."}
        </p>
      )}
    </div>
  );
}
