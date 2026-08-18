import { useEffect, useState } from "react";
import { Sidebar } from "./components/Sidebar";
import { onJobProgress } from "./lib/tauri";
import { ConfigPage } from "./pages/ConfigPage";
import { DownloadingPage } from "./pages/DownloadingPage";
import { HistoryPage } from "./pages/HistoryPage";
import { MainPage } from "./pages/MainPage";
import { useStore } from "./state/store";

function App() {
  const [view, setView] = useState<"main" | "config" | "history" | "downloading">("main");
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

  // One layout for every view rather than an early return for Downloading.
  // That split is what let the drag region regress: the Downloading branch
  // rendered neither the sidebar nor the drag strip, so its window had
  // nothing draggable at all. Keeping a single shell means a new view can't
  // forget the window chrome.
  //
  // `main` deliberately carries no background — the window is transparent so
  // the OS vibrancy material shows through the sidebar, and an opaque
  // ancestor there would hide it. Only the content column paints.
  return (
    <main className="h-screen w-screen text-[var(--text-primary)] overflow-hidden flex">
      {/* Downloading is a focused task view with no sidebar, per the mockups.
          Elsewhere: picking a template must also leave the Config page —
          otherwise the sidebar selection changes under a Config view that has
          no other way back, stranding the user there. */}
      {view !== "downloading" && (
        <Sidebar
          onOpenConfig={() => setView("config")}
          onOpenHistory={() => setView("history")}
          onTemplateActivated={() => setView("main")}
        />
      )}
      {/* Content layer: opaque. Only the sidebar is glass, so the window's
          vibrancy reads there and never behind form fields or tables
          (spec/ui.md layer rules). */}
      <div className="flex-1 min-w-0 flex flex-col bg-[var(--surface-sunken)]">
        {/* The window hides its title bar (titleBarStyle: Overlay), so there
            is no OS chrome to grab — every draggable area has to be provided
            by the app. "deep" because Tauri only drags from a bare region
            when the click lands on that exact element. Height matches the
            sidebar's top inset so the band is continuous across the
            title-bar row. */}
        <div data-tauri-drag-region="deep" className="h-9 shrink-0" />
        <div className="flex-1 min-h-0 overflow-y-auto">
          {view === "main" && <MainPage onStarted={() => setView("downloading")} />}
          {view === "config" && <ConfigPage />}
          {view === "history" && <HistoryPage />}
          {view === "downloading" && <DownloadingPage onBack={() => setView("main")} />}
        </div>
      </div>
    </main>
  );
}

export default App;
