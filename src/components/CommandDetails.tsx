import { useState } from "react";

/// Shows the exact yt-dlp invocation behind an operation, plus its failure
/// output when there is one.
///
/// This exists because the app is otherwise a black box: when a probe or
/// download misbehaves there was no way to tell which binary ran, whether the
/// proxy and cookie flags were actually applied, or how the format selector
/// was assembled — and no way to reproduce it outside the app. The command is
/// shell-quoted, so it can be pasted into a terminal unchanged.
export function CommandDetails({
  command,
  log,
  defaultOpen = false,
}: {
  command: string | null;
  /// Failure output (a stderr tail). When present the block opens by default,
  /// since the user is already looking for an explanation.
  log?: string | null;
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen || !!log);
  const [copied, setCopied] = useState(false);

  if (!command && !log) return null;

  const copy = async () => {
    if (!command) return;
    try {
      await navigator.clipboard.writeText(command);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard access can be refused by the webview; the command text is
      // selectable either way, so fall back to saying nothing rather than
      // claiming a copy that didn't happen.
      setCopied(false);
    }
  };

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-3">
        <button
          onClick={() => setOpen((v) => !v)}
          className="text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
        >
          {open ? "▾" : "▸"} command{log ? " & log" : ""}
        </button>
        {open && command && (
          <button
            onClick={copy}
            className="text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-primary)]"
          >
            {copied ? "copied" : "copy"}
          </button>
        )}
      </div>

      {open && (
        <div className="flex flex-col gap-1.5">
          {command && (
            <pre className="text-[11px] font-mono whitespace-pre-wrap break-all select-text rounded-md bg-[var(--surface-sunken)] border border-[var(--border)] px-2 py-1.5 text-[var(--text-secondary)] max-h-28 overflow-y-auto">
              {command}
            </pre>
          )}
          {log && (
            <pre className="text-[11px] font-mono whitespace-pre-wrap break-words select-text rounded-md bg-[var(--danger-bg)] px-2 py-1.5 text-[var(--danger)] max-h-40 overflow-y-auto">
              {log}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}
