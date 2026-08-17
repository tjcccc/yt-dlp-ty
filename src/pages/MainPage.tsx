import { useEffect, useState } from "react";
import { OptionsPanel } from "../components/OptionsPanel";
import { checkBinary, probeFormats, startDownloads } from "../lib/tauri";
import { useStore } from "../state/store";
import type { BinaryCheck, ChosenFormat, OptionsMode, VideoFormats } from "../types";
import { ChooseFormatModal } from "./ChooseFormatModal";

function statusLabel(check: BinaryCheck | null): string {
  if (!check) return "checking…";
  return check.found ? `found (${check.version ?? "unknown version"})` : "not found";
}

export function MainPage({ onStarted }: { onStarted: () => void }) {
  const templates = useStore((s) => s.templates);
  const selectedTemplateId = useStore((s) => s.selectedTemplateId);
  const selectedTemplate = templates.find((t) => t.id === selectedTemplateId) ?? null;
  const addJob = useStore((s) => s.addJob);
  const updateTemplate = useStore((s) => s.updateTemplate);

  const [urlsText, setUrlsText] = useState("");
  const [downloadTo, setDownloadTo] = useState("");
  const [parameters, setParameters] = useState("");
  const [mode, setMode] = useState<OptionsMode>("raw");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedNote, setSavedNote] = useState<string | null>(null);
  const [ytdlpStatus, setYtdlpStatus] = useState<BinaryCheck | null>(null);
  const [ffmpegStatus, setFfmpegStatus] = useState<BinaryCheck | null>(null);
  // Non-null once the "Choose format first" picker is open; `probing` covers
  // the gap while yt-dlp fetches metadata, so the modal can show a pending
  // state instead of the window appearing frozen.
  const [probed, setProbed] = useState<VideoFormats[] | null>(null);
  const [probing, setProbing] = useState(false);

  // Switching templates loads its fields into the form. Edits stay per-run
  // (so a one-off tweak doesn't mutate the preset) until the user explicitly
  // saves them back via "Save to template".
  useEffect(() => {
    if (!selectedTemplate) return;
    setUrlsText(selectedTemplate.urlsDefault);
    setDownloadTo(selectedTemplate.downloadTo);
    setParameters(selectedTemplate.parameters);
    setMode(selectedTemplate.options.mode);
  }, [selectedTemplate]);

  useEffect(() => {
    checkBinary("yt-dlp").then(setYtdlpStatus);
    checkBinary("ffmpeg").then(setFfmpegStatus);
  }, []);

  const urls = urlsText
    .split("\n")
    .map((u) => u.trim())
    .filter(Boolean);

  const dirty =
    !!selectedTemplate &&
    (urlsText !== selectedTemplate.urlsDefault ||
      downloadTo !== selectedTemplate.downloadTo ||
      parameters !== selectedTemplate.parameters ||
      mode !== selectedTemplate.options.mode);

  async function handleSaveTemplate() {
    if (!selectedTemplate) return;
    await updateTemplate(selectedTemplate.id, {
      urlsDefault: urlsText,
      downloadTo,
      parameters,
      options: { mode },
    });
    setSavedNote("Saved");
    setTimeout(() => setSavedNote(null), 2000);
  }

  /// In "Choose format first" mode the Download button probes and opens the
  /// picker instead of downloading; the actual download starts from the
  /// modal's own button, once every video has a format.
  async function handleDownload() {
    if (!selectedTemplate || urls.length === 0) return;
    if (mode === "chooseFormat") {
      setError(null);
      setProbed([]);
      setProbing(true);
      try {
        setProbed(await probeFormats(urls, parameters));
      } catch (e) {
        setProbed(null);
        setError(String(e));
      } finally {
        setProbing(false);
      }
      return;
    }
    await startJobs({});
  }

  async function startJobs(chosenFormats: Record<string, ChosenFormat>) {
    if (!selectedTemplate || urls.length === 0) return;
    // In chooseFormat mode, URLs that failed to probe have no selection and
    // are dropped rather than silently downloaded at a default format.
    const targetUrls = mode === "chooseFormat" ? urls.filter((u) => chosenFormats[u]) : urls;
    if (targetUrls.length === 0) return;
    setError(null);
    setSubmitting(true);
    try {
      const jobIds = await startDownloads({
        urls: targetUrls,
        downloadTo,
        parameters,
        mode,
        templateId: selectedTemplate.id,
        chosenFormats,
      });
      jobIds.forEach((jobId, i) => {
        addJob({
          jobId,
          // Must index `targetUrls`, not `urls` — in chooseFormat mode the
          // two differ whenever a URL failed to probe, which would label
          // rows with the wrong video.
          url: targetUrls[i] ?? targetUrls[0],
          phase: "queued",
          downloadedBytes: null,
          totalBytes: null,
          speedBps: null,
          etaSeconds: null,
          overallPercent: 0,
          errorMessage: null,
        });
      });
      onStarted();
    } catch (e) {
      setError(String(e));
    } finally {
      setSubmitting(false);
    }
  }

  if (!selectedTemplate) {
    return <div className="p-6 text-[13px] text-[var(--text-secondary)]">No template selected.</div>;
  }

  return (
    <div className="p-6 flex flex-col gap-5 max-w-2xl">
      {probed !== null && (
        <ChooseFormatModal
          videos={probed}
          loading={probing}
          onCancel={() => setProbed(null)}
          onConfirm={(chosen) => {
            setProbed(null);
            startJobs(chosen);
          }}
        />
      )}

      <div>
        <h1 className="text-[20px] font-semibold">{selectedTemplate.name}</h1>
        <p className="text-[12px] text-[var(--text-secondary)] mt-1">
          yt-dlp: {statusLabel(ytdlpStatus)} · ffmpeg: {statusLabel(ffmpegStatus)}
        </p>
      </div>

      <label className="flex flex-col gap-1.5">
        <span className="text-[14px] font-medium">URL</span>
        <textarea
          value={urlsText}
          onChange={(e) => setUrlsText(e.target.value)}
          placeholder="(support multiple urls)"
          rows={4}
          // resize-none: the webview's drag-to-resize grabber is a browser
          // affordance with no equivalent in a native macOS text field, and
          // it's one of the clearest tells that a window is a web view.
          className="resize-none rounded-md border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-[13px] focus:outline-none focus:ring-2 focus:ring-[var(--accent)]/35"
        />
      </label>

      <label className="flex flex-col gap-1.5">
        <span className="text-[14px] font-medium">Download to</span>
        <input
          value={downloadTo}
          onChange={(e) => setDownloadTo(e.target.value)}
          className="rounded-md border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-[12px] font-mono focus:outline-none focus:ring-2 focus:ring-[var(--accent)]/35"
        />
      </label>

      <div className="flex gap-6">
        <label className="flex flex-col gap-1.5 flex-1">
          <span className="text-[14px] font-medium">Parameters</span>
          <textarea
            value={parameters}
            onChange={(e) => setParameters(e.target.value)}
            rows={4}
            placeholder="--no-playlist"
            className="resize-none rounded-md border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-[13px] font-mono focus:outline-none focus:ring-2 focus:ring-[var(--accent)]/35"
          />
        </label>

        <div className="w-56 shrink-0">
          <OptionsPanel mode={mode} onChange={setMode} />
        </div>
      </div>

      {error && <p className="text-[13px] text-[var(--danger)]">{error}</p>}

      <div className="self-end flex items-center gap-3">
        {savedNote && <span className="text-[12px] text-[var(--text-tertiary)]">{savedNote}</span>}
        <button
          onClick={handleSaveTemplate}
          disabled={!dirty}
          className="rounded-md border border-[var(--border)] bg-[var(--surface)] disabled:opacity-40 px-3 py-2 text-[13px] hover:bg-[var(--hover)]"
        >
          Save to template
        </button>
        <button
          onClick={handleDownload}
          disabled={urls.length === 0 || submitting}
          className="rounded-md bg-[var(--accent)] disabled:opacity-40 text-[var(--text-on-accent)] px-4 py-2 text-[14px] font-medium hover:bg-[var(--accent-hover)]"
        >
          Download
        </button>
      </div>
    </div>
  );
}
