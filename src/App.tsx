import { useEffect, useState } from "react";
import { Sidebar } from "./components/Sidebar";
import { onJobProgress } from "./lib/tauri";
import { ConfigPage } from "./pages/ConfigPage";
import { DownloadingPage } from "./pages/DownloadingPage";
import { MainPage } from "./pages/MainPage";
import { useStore } from "./state/store";

function App() {
  const [view, setView] = useState<"main" | "config" | "downloading">("main");
  const updateJobProgress = useStore((s) => s.updateJobProgress);
  const loadTemplates = useStore((s) => s.loadTemplates);
  const loadConfig = useStore((s) => s.loadConfig);

  useEffect(() => {
    const unlisten = onJobProgress(updateJobProgress);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [updateJobProgress]);

  useEffect(() => {
    loadTemplates();
    loadConfig();
  }, [loadTemplates, loadConfig]);

  if (view === "downloading") {
    return (
      <main className="h-screen w-screen text-[var(--text-primary)] overflow-hidden bg-[var(--surface-sunken)]">
        <DownloadingPage onBack={() => setView("main")} />
      </main>
    );
  }

  return (
    <main className="h-screen w-screen text-[var(--text-primary)] overflow-hidden flex">
      {/* Picking a template must also leave the Config page — otherwise the
          sidebar selection changes under a Config view that has no other way
          back, stranding the user there. */}
      <Sidebar onOpenConfig={() => setView("config")} onTemplateActivated={() => setView("main")} />
      {/* Content layer: opaque. Only the sidebar is glass, so the window's
          vibrancy reads there and never behind form fields or tables
          (spec/ui.md layer rules). */}
      <div className="flex-1 overflow-y-auto bg-[var(--surface-sunken)]">
        {view === "main" && <MainPage onStarted={() => setView("downloading")} />}
        {view === "config" && <ConfigPage />}
      </div>
    </main>
  );
}

export default App;
