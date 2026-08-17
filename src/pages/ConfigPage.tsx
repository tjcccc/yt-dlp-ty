import { useEffect, useState, type ReactNode } from "react";
import { checkBinary, updateYtdlp } from "../lib/tauri";
import { useStore } from "../state/store";
import type { BinaryCheck } from "../types";

function statusLabel(check: BinaryCheck | null): string {
  if (!check) return "checking…";
  return check.found ? `found (${check.version ?? "unknown version"})` : "not found";
}

function BinaryRow({
  name,
  path,
  onPathSaved,
  extra,
}: {
  name: string;
  path: string | null;
  onPathSaved: (path: string | null) => void;
  extra?: ReactNode;
}) {
  const [check, setCheck] = useState<BinaryCheck | null>(null);
  const [pathInput, setPathInput] = useState(path ?? "");

  const runCheck = async (candidate?: string) => {
    const result = await checkBinary(name, candidate ?? pathInput.trim() ?? undefined);
    setCheck(result);
  };

  useEffect(() => {
    runCheck(path ?? undefined);
    // Only re-check automatically when the saved override path changes —
    // typing in the field re-checks via the explicit "check" button.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path]);

  return (
    <div className="rounded-md border border-[var(--border)] bg-[var(--surface)] p-3 flex flex-col gap-2">
      <div className="flex items-center justify-between gap-3">
        <span className="text-[14px] font-medium">{name}</span>
        <div className="flex items-center gap-2">
          <span
            className={`text-[12px] ${
              // Status colour is a state signal, not decoration, and never the
              // only signal — the words "found"/"not found" carry it too.
              check?.found ? "text-[var(--accent-fg)]" : check ? "text-[var(--danger)]" : "text-[var(--text-secondary)]"
            }`}
          >
            {statusLabel(check)}
          </span>
          {/* Neutral, not accent-tinted: `check` is a utility action, and
              spec/ui.md reserves the tint for primary actions and selected
              state. Two filled green buttons here would out-shout the real
              primary action elsewhere in the app. */}
          <button
            onClick={() => runCheck()}
            className="text-[12px] px-3 py-1.5 rounded-md border border-[var(--border)] hover:bg-[var(--hover)]"
          >
            check
          </button>
          {extra}
        </div>
      </div>
      <div className="flex items-center gap-2">
        <input
          value={pathInput}
          onChange={(e) => setPathInput(e.target.value)}
          placeholder={check?.path ?? `custom ${name} path (optional)`}
          className="flex-1 rounded-md border border-[var(--border)] bg-[var(--surface)] px-2 py-1 text-[12px] font-mono"
        />
        <button
          onClick={() => onPathSaved(pathInput.trim() || null)}
          className="text-[12px] px-2 py-1 rounded-md border border-[var(--border)] hover:bg-[var(--hover)]"
        >
          save
        </button>
      </div>
    </div>
  );
}

function ProxyRow({ value, onSave }: { value: string; onSave: (value: string) => void }) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);

  return (
    <label className="rounded-md border border-[var(--border)] bg-[var(--surface)] p-3 flex items-center justify-between gap-3">
      <span className="text-[14px] font-medium">proxy</span>
      <input
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={() => {
          if (text !== value) onSave(text);
        }}
        placeholder="http://127.0.0.1:7890"
        className="flex-1 max-w-[220px] text-right rounded-md border border-[var(--border)] bg-[var(--surface)] px-2 py-1 text-[13px] font-mono"
      />
    </label>
  );
}

export function ConfigPage() {
  const config = useStore((s) => s.config);
  const updateConfig = useStore((s) => s.updateConfig);
  const [updateOutput, setUpdateOutput] = useState<string | null>(null);
  const [updating, setUpdating] = useState(false);

  // Manual action only — never run automatically per download job. See
  // DEVLOG "Bugfix" entry: `-U` in per-job Parameters fails outright on a
  // Homebrew-managed install and shouldn't re-run on every single job.
  const runUpdate = async () => {
    setUpdating(true);
    try {
      setUpdateOutput(await updateYtdlp());
    } catch (e) {
      setUpdateOutput(String(e));
    } finally {
      setUpdating(false);
    }
  };

  return (
    // Same wrapper geometry as MainPage (pt-1 under the drag strip, one
    // shared max width, centred) so the page title lands in exactly the same
    // spot when switching views and the content doesn't hug the left edge on
    // a stretched window.
    <div className="flex flex-col gap-4 px-6 pt-1 pb-6 w-full max-w-3xl mx-auto">
      <h1 className="text-[20px] font-semibold">Config</h1>

      <BinaryRow
        name="yt-dlp"
        path={config.ytdlpPath}
        onPathSaved={(path) => updateConfig({ ytdlpPath: path })}
        extra={
          <button
            onClick={runUpdate}
            disabled={updating}
            className="text-[12px] px-3 py-1.5 rounded-md border border-[var(--border)] hover:bg-[var(--hover)] disabled:opacity-40"
          >
            {updating ? "checking…" : "Check for updates"}
          </button>
        }
      />
      {updateOutput && (
        <pre className="text-[11px] bg-[var(--hover)] rounded-md p-2 whitespace-pre-wrap max-h-32 overflow-y-auto">
          {updateOutput}
        </pre>
      )}

      <BinaryRow
        name="ffmpeg"
        path={config.ffmpegPath}
        onPathSaved={(path) => updateConfig({ ffmpegPath: path })}
      />

      <ProxyRow value={config.proxy} onSave={(proxy) => updateConfig({ proxy })} />
    </div>
  );
}
