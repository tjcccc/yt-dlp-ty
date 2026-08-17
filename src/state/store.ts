import { create } from "zustand";
import { createConfigSlice, type ConfigSlice } from "./configSlice";
import { createJobsSlice, type JobsSlice } from "./jobsSlice";
import { createTemplatesSlice, type TemplatesSlice } from "./templatesSlice";

export type Store = JobsSlice & TemplatesSlice & ConfigSlice;

export const useStore = create<Store>((set, get) => ({
  ...createJobsSlice(set),
  ...createTemplatesSlice(set, get),
  ...createConfigSlice(set, get),
}));
