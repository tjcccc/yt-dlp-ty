import type { Template } from "../types";
import { deleteTemplate as deleteTemplateApi, listTemplates, saveTemplate as saveTemplateApi } from "../lib/tauri";

const DEFAULT_DOWNLOAD_TO =
  "~/Downloads/yt-dlp/{date:YYYY-MM-DD}_{id:NNN}_{id:guid}_{original_filename}";

export interface TemplatesSlice {
  templates: Template[];
  selectedTemplateId: string | null;
  loadTemplates: () => Promise<void>;
  selectTemplate: (id: string) => void;
  addTemplate: () => Promise<void>;
  /// Persists form edits back onto an existing template. Without this the
  /// main form's fields are per-run only and silently reset on every
  /// template switch, which makes templates useless as saved presets.
  updateTemplate: (id: string, patch: Partial<Template>) => Promise<void>;
  deleteTemplate: (id: string) => Promise<void>;
}

type Set = (fn: (state: TemplatesSlice) => Partial<TemplatesSlice>) => void;
type Get = () => TemplatesSlice;

export const createTemplatesSlice = (set: Set, get: Get): TemplatesSlice => ({
  templates: [],
  selectedTemplateId: null,

  loadTemplates: async () => {
    const templates = [...(await listTemplates())].sort((a, b) => a.order - b.order);
    set((state) => ({
      templates,
      selectedTemplateId: state.selectedTemplateId ?? templates[0]?.id ?? null,
    }));
  },

  selectTemplate: (id) => set(() => ({ selectedTemplateId: id })),

  addTemplate: async () => {
    const { templates } = get();
    const created = await saveTemplateApi({
      id: "",
      name: `New template ${templates.length + 1}`,
      urlsDefault: "",
      downloadTo: DEFAULT_DOWNLOAD_TO,
      parameters: "",
      options: { mode: "raw" },
      nextSeq: 1,
      createdAt: new Date().toISOString(),
      order: templates.length,
    });
    set((state) => ({
      templates: [...state.templates, created],
      selectedTemplateId: created.id,
    }));
  },

  updateTemplate: async (id, patch) => {
    const existing = get().templates.find((t) => t.id === id);
    if (!existing) return;
    // `nextSeq` is owned by the backend (incremented per job at spawn time);
    // never round-trip a stale copy from the form back over it.
    const saved = await saveTemplateApi({ ...existing, ...patch, nextSeq: existing.nextSeq });
    set((state) => ({
      templates: state.templates.map((t) => (t.id === id ? saved : t)),
    }));
  },

  deleteTemplate: async (id) => {
    await deleteTemplateApi(id);
    set((state) => {
      const templates = state.templates.filter((t) => t.id !== id);
      const selectedTemplateId =
        state.selectedTemplateId === id ? (templates[0]?.id ?? null) : state.selectedTemplateId;
      return { templates, selectedTemplateId };
    });
  },
});
