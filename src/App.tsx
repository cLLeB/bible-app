import "./App.css";
import { DisplayPanel } from "./components/DisplayPanel";
import { ListenPanel } from "./components/ListenPanel";
import { LiveNow } from "./components/LiveNow";
import { LookupBar } from "./components/LookupBar";
import { RecentVerses } from "./components/RecentVerses";
import { ResultCard } from "./components/ResultCard";
import { ThemeToggle } from "./components/ThemeToggle";
import { ScripturePresenter } from "./components/ScripturePresenter";
import { ScriptureSearch } from "./components/ScriptureSearch";
import { ServicePanel } from "./components/ServicePanel";
import { SongsPanel } from "./components/SongsPanel";
import { TranslationPicker } from "./components/TranslationPicker";

export default function App() {
  return (
    <main className="mx-auto max-w-3xl space-y-6 p-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Bible — Operator Console</h1>
        <ThemeToggle />
      </div>
      <LiveNow />
      <ScripturePresenter />
      <ListenPanel />
      <hr />
      <section className="space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-xl font-semibold">Scripture</h2>
          <TranslationPicker />
        </div>
        <LookupBar />
        <RecentVerses />
        <ResultCard />
        <ScriptureSearch />
      </section>
      <hr />
      <ServicePanel />
      <hr />
      <SongsPanel />
      <hr />
      <DisplayPanel />
    </main>
  );
}
