import { useEffect, useState } from "react";
import "./App.css";
import { appFlavor } from "./api";
import { DisplayPanel } from "./components/DisplayPanel";
import { ListenPanel } from "./components/ListenPanel";
import { LiveNow } from "./components/LiveNow";
import { LookupBar } from "./components/LookupBar";
import { OutputsPanel } from "./components/OutputsPanel";
import { PdfImport } from "./components/PdfImport";
import { PlanningCenterPanel } from "./components/PlanningCenterPanel";
import { ProjectorPanel } from "./components/ProjectorPanel";
import { RecentVerses } from "./components/RecentVerses";
import { ResultCard } from "./components/ResultCard";
import { ThemeToggle } from "./components/ThemeToggle";
import { LiveSync } from "./components/LiveSync";
import { MediaPanel } from "./components/MediaPanel";
import { ScripturePresenter } from "./components/ScripturePresenter";
import { ScriptureSearch } from "./components/ScriptureSearch";
import { ServicePanel } from "./components/ServicePanel";
import { SongsPanel } from "./components/SongsPanel";
import { SongUsageReport } from "./components/SongUsageReport";
import { ThemesPanel } from "./components/ThemesPanel";
import { TranslationPicker } from "./components/TranslationPicker";
import { TranslationManager } from "./components/TranslationManager";
import { loadTab, saveTab, type ConsoleTab } from "./lib/consoleTab";

export default function App() {
  // Personal builds ship every translation, so the "add translations" manager is
  // pointless there, so show it only on distribution builds. Hidden until known.
  const [tier, setTier] = useState<"personal" | "distribution">("personal");
  // Live is what you operate a service from; Prepare is everything you set up
  // beforehand. Sticky so the console reopens where the operator left it.
  const [tab, setTab] = useState<ConsoleTab>(loadTab);

  useEffect(() => {
    void appFlavor().then((f) => setTier(f.tier)).catch(() => undefined);
  }, []);

  function changeTab(next: ConsoleTab): void {
    setTab(next);
    saveTab(next);
  }

  return (
    <div className="min-h-screen">
      <header
        className="sticky top-0 z-20 border-b backdrop-blur"
        style={{
          background: "color-mix(in srgb, var(--bg) 85%, transparent)",
          borderColor: "var(--border)",
        }}
      >
        <div className="mx-auto flex max-w-[1600px] flex-wrap items-center gap-x-4 gap-y-2 px-4 py-2.5 lg:px-6">
          <div className="flex items-center gap-2.5">
            <img
              src="/newbreed_logo.png"
              alt="New Breed"
              style={{ height: "34px", width: "auto", borderRadius: "5px", background: "#fff", padding: "2px" }}
            />
            <span className="text-lg font-medium tracking-tight text-[var(--muted)]">Operator Console</span>
          </div>

          <nav className="flex items-center gap-1 rounded-lg p-0.5" style={{ background: "var(--surface-2, rgba(127,127,127,0.12))" }}>
            <button
              onClick={() => changeTab("live")}
              aria-pressed={tab === "live"}
              className={`btn btn-sm ${tab === "live" ? "btn-primary" : ""}`}
              title="What you use while a service is running"
            >
              Live
            </button>
            <button
              onClick={() => changeTab("prepare")}
              aria-pressed={tab === "prepare"}
              className={`btn btn-sm ${tab === "prepare" ? "btn-primary" : ""}`}
              title="Everything you set up before the service: songs, slides, themes, outputs"
            >
              Prepare
            </button>
          </nav>

          <div className="ml-auto flex flex-wrap items-center gap-2">
            <TranslationPicker />
            {tier !== "personal" && <TranslationManager />}
            <ThemeToggle />
          </div>
        </div>
      </header>

      <main className="mx-auto max-w-[1600px] space-y-4 px-4 py-4 lg:px-6">
        {/* Renders nothing, and must stay outside the tab switch: the phone
            remote and the listening loop present while the operator is on
            Prepare, and the stage monitor has to follow them there. */}
        <LiveSync />

        {/* Always visible, on both tabs: what the congregation is seeing right now. */}
        <LiveNow />

        {tab === "live" ? (
          <div className="grid grid-cols-1 gap-4 xl:grid-cols-2 xl:items-start">
            {/* Left: the listening loop and finding scripture */}
            <div className="space-y-4">
              <ScripturePresenter />
              <div className="card">
                <ListenPanel />
              </div>
              <div className="card space-y-3">
                <h2 className="panel-title">Scripture</h2>
                <LookupBar />
                <RecentVerses />
                <ResultCard />
                <ScriptureSearch />
              </div>
            </div>

            {/* Right: the run sheet and the controls you reach for mid-service */}
            <div className="space-y-4">
              <div className="card">
                <ServicePanel />
              </div>
              <div className="card">
                <DisplayPanel />
              </div>
            </div>
          </div>
        ) : (
          <div className="space-y-4">
            <div className="grid grid-cols-1 gap-4 xl:grid-cols-2 xl:items-start">
              <div className="space-y-4">
                <div className="card">
                  <SongsPanel />
                </div>
                <div className="card">
                  <MediaPanel />
                </div>
                <div className="card">
                  <PdfImport />
                </div>
              </div>
              <div className="space-y-4">
                <div className="card">
                  <OutputsPanel />
                </div>
                <div className="card">
                  <ThemesPanel />
                </div>
                <div className="card">
                  <ProjectorPanel />
                </div>
              </div>
            </div>

            {/* Occasional admin. Collapsed by default, so it opens only when you
                deliberately go looking for it. */}
            <details className="card">
              <summary className="cursor-pointer select-none panel-title">
                Admin &amp; integrations
              </summary>
              <p className="mt-1 text-xs text-[var(--faint)]">
                Only needed if your church uses Planning Center Online, or holds a CCLI licence and
                has to file song-usage reports.
              </p>
              <div className="mt-3 grid grid-cols-1 gap-4 xl:grid-cols-2 xl:items-start">
                <PlanningCenterPanel />
                <SongUsageReport />
              </div>
            </details>
          </div>
        )}
      </main>
    </div>
  );
}
