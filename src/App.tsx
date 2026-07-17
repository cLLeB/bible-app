import { useEffect, useState } from "react";
import "./App.css";
import { appFlavor } from "./api";
import { DisplayPanel } from "./components/DisplayPanel";
import { ListenPanel } from "./components/ListenPanel";
import { LiveNow } from "./components/LiveNow";
import { LookupBar } from "./components/LookupBar";
import { PdfImport } from "./components/PdfImport";
import { ProjectorPanel } from "./components/ProjectorPanel";
import { RecentVerses } from "./components/RecentVerses";
import { ResultCard } from "./components/ResultCard";
import { ThemeToggle } from "./components/ThemeToggle";
import { ScripturePresenter } from "./components/ScripturePresenter";
import { ScriptureSearch } from "./components/ScriptureSearch";
import { ServicePanel } from "./components/ServicePanel";
import { SongsPanel } from "./components/SongsPanel";
import { SongUsageReport } from "./components/SongUsageReport";
import { ThemesPanel } from "./components/ThemesPanel";
import { TranslationPicker } from "./components/TranslationPicker";
import { TranslationManager } from "./components/TranslationManager";

export default function App() {
  // Personal builds ship every translation, so the "add translations" manager is
  // pointless there — show it only on distribution builds. Default hidden until known.
  const [tier, setTier] = useState<"personal" | "distribution">("personal");
  useEffect(() => {
    void appFlavor().then((f) => setTier(f.tier)).catch(() => undefined);
  }, []);

  return (
    <div className="min-h-screen">
      <header className="sticky top-0 z-20 border-b backdrop-blur" style={{ background: "color-mix(in srgb, var(--bg) 85%, transparent)", borderColor: "var(--border)" }}>
        <div className="mx-auto flex max-w-[1600px] flex-wrap items-center gap-x-4 gap-y-2 px-4 py-2.5 lg:px-6">
          <div className="flex items-center gap-2.5">
            <img
              src="/newbreed_logo.png"
              alt="New Breed"
              style={{ height: "34px", width: "auto", borderRadius: "5px", background: "#fff", padding: "2px" }}
            />
            <span className="text-lg font-medium tracking-tight text-[var(--muted)]">Operator Console</span>
          </div>
          <div className="ml-auto flex flex-wrap items-center gap-2">
            <TranslationPicker />
            {tier !== "personal" && <TranslationManager />}
            <ThemeToggle />
          </div>
        </div>
      </header>

      <main className="mx-auto max-w-[1600px] space-y-4 px-4 py-4 lg:px-6">
        <LiveNow />

        <div className="grid grid-cols-1 gap-4 xl:grid-cols-2 xl:items-start">
          {/* Left column — what's going out + finding scripture */}
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

          {/* Right column — building the service + display control */}
          <div className="space-y-4">
            <div className="card">
              <ServicePanel />
            </div>
            <div className="card">
              <SongsPanel />
            </div>
            <div className="card">
              <SongUsageReport />
            </div>
            <div className="card">
              <PdfImport />
            </div>
            <div className="card">
              <DisplayPanel />
            </div>
            <div className="card">
              <ThemesPanel />
            </div>
            <div className="card">
              <ProjectorPanel />
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}
