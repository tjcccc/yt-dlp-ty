import type { AppConfig } from "../types";
import { getConfig, setConfig as setConfigApi } from "../lib/tauri";

const DEFAULT_CONFIG: AppConfig = {
  ytdlpPath: null,
  ffmpegPath: null,
  proxy: "",
  concurrency: 3,
};

export interface ConfigSlice {
  config: AppConfig;
  loadConfig: () => Promise<void>;
  updateConfig: (patch: Partial<AppConfig>) => Promise<void>;
}

type Set = (fn: (state: ConfigSlice) => Partial<ConfigSlice>) => void;
type Get = () => ConfigSlice;

export const createConfigSlice = (set: Set, get: Get): ConfigSlice => ({
  config: DEFAULT_CONFIG,

  loadConfig: async () => {
    const config = await getConfig();
    set(() => ({ config }));
  },

  updateConfig: async (patch) => {
    const next = { ...get().config, ...patch };
    set(() => ({ config: next }));
    await setConfigApi(next);
  },
});
