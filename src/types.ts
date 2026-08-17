export type OptionsMode = "raw" | "bestVideo" | "bestAudio" | "chooseFormat";

export interface OptionsConfig {
  mode: OptionsMode;
}

export interface Template {
  id: string;
  name: string;
  urlsDefault: string;
  downloadTo: string;
  parameters: string;
  options: OptionsConfig;
  nextSeq: number;
  createdAt: string;
  order: number;
}

export interface AppConfig {
  ytdlpPath: string | null;
  ffmpegPath: string | null;
  proxy: string;
  concurrency: number;
}

/// A format the user picked in the "Choose format first" flow. `videoOnly`
/// is derived from the probed entry's codecs and tells the backend whether
/// to pair an audio stream in — a video-only format downloaded alone would
/// be silent.
export interface ChosenFormat {
  formatId: string;
  videoOnly: boolean;
}

export interface MultiDownloadRequest {
  urls: string[];
  downloadTo: string;
  parameters: string;
  mode: OptionsMode;
  templateId: string;
  /// url -> chosen format. Only meaningful when mode === "chooseFormat".
  chosenFormats: Record<string, ChosenFormat>;
}

export interface FormatEntry {
  formatId: string;
  ext: string;
  resolution: string;
  fps: number | null;
  filesize: number | null;
  tbr: number | null;
  proto: string;
  vcodec: string;
  acodec: string;
}

export interface VideoFormats {
  url: string;
  title: string;
  videoId: string;
  formats: FormatEntry[];
  /// Per-URL rather than a batch failure: one dead link shouldn't block
  /// choosing formats for the others.
  error: string | null;
  /// The exact shell-quoted yt-dlp invocation used for this probe.
  command: string;
}

/// True when a format carries video but no audio, so it needs pairing.
/// Note the literal "none" test — an extractor reporting "unknown" codecs
/// is a real stream with missing metadata, not a video-only one.
export function isVideoOnly(format: FormatEntry): boolean {
  return format.vcodec !== "none" && format.acodec === "none";
}

export type JobPhase = "queued" | "downloading" | "merging" | "completed" | "error" | "cancelled";

export interface JobProgressEvent {
  jobId: string;
  phase: JobPhase;
  downloadedBytes: number | null;
  totalBytes: number | null;
  speedBps: number | null;
  etaSeconds: number | null;
  overallPercent: number | null;
  errorMessage: string | null;
  /// Sent once on the spawn event, null on subsequent ticks.
  command: string | null;
}

export interface BinaryCheck {
  found: boolean;
  path: string | null;
  version: string | null;
  error: string | null;
}
