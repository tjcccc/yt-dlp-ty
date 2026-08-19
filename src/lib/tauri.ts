import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppConfig,
  BinaryCheck,
  JobProgressEvent,
  MultiDownloadRequest,
  HistoryEntry,
  Template,
  VideoFormats,
} from "../types";

export function startDownloads(request: MultiDownloadRequest): Promise<string[]> {
  return invoke("start_downloads", { request });
}

/// Metadata-only: lists available formats without downloading. Slow (spawns
/// a yt-dlp process per URL), so callers should show a pending state.
export function probeFormats(urls: string[], parameters: string): Promise<VideoFormats[]> {
  return invoke("probe_formats", { request: { urls, parameters } });
}

/// Stops the running probe batch at its next chunk boundary. Partial by
/// design: yt-dlp processes already running are left to finish, so a
/// single-URL probe is unaffected.
export function cancelProbe(): Promise<void> {
  return invoke("cancel_probe");
}

export function cancelJob(jobId: string): Promise<void> {
  return invoke("cancel_job", { jobId });
}

export function cancelAll(): Promise<void> {
  return invoke("cancel_all");
}

export function checkBinary(name: string, customPath?: string): Promise<BinaryCheck> {
  return invoke("check_binary", { name, customPath: customPath ?? null });
}

export function updateYtdlp(): Promise<string> {
  return invoke("update_ytdlp");
}

export function onJobProgress(cb: (event: JobProgressEvent) => void): Promise<UnlistenFn> {
  return listen<JobProgressEvent>("job://progress", (e) => cb(e.payload));
}

export function listTemplates(): Promise<Template[]> {
  return invoke("list_templates");
}

export function saveTemplate(template: Template): Promise<Template> {
  return invoke("save_template", { template });
}

export function deleteTemplate(id: string): Promise<void> {
  return invoke("delete_template", { id });
}

export function reorderTemplates(ids: string[]): Promise<void> {
  return invoke("reorder_templates", { ids });
}

export function getConfig(): Promise<AppConfig> {
  return invoke("get_config");
}

export function setConfig(config: AppConfig): Promise<AppConfig> {
  return invoke("set_config", { config });
}

export function listHistory(limit?: number, offset?: number): Promise<HistoryEntry[]> {
  return invoke("list_history", { limit: limit ?? null, offset: offset ?? null });
}

export function clearHistory(): Promise<void> {
  return invoke("clear_history");
}
