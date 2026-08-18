import { useState } from "react";

/// Copies `value` and briefly confirms.
///
/// The failure path matters: a webview can refuse clipboard access, and in
/// that case the button stays silent rather than reporting a copy that never
/// happened. Anything it copies is also rendered as selectable text nearby,
/// so there's always a manual route.
export function CopyButton({
  value,
  label = "copy",
  className = "",
}: {
  value: string;
  label?: string;
  className?: string;
}) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      setCopied(false);
    }
  };

  return (
    <button
      onClick={copy}
      title={copied ? "Copied" : "Copy"}
      className={`text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] ${className}`}
    >
      {copied ? "copied" : label}
    </button>
  );
}
