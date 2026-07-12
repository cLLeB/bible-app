import "./App.css";
import { DisplayPanel } from "./components/DisplayPanel";
import { ListenPanel } from "./components/ListenPanel";
import { LookupBar } from "./components/LookupBar";
import { ResultCard } from "./components/ResultCard";
import { SongsPanel } from "./components/SongsPanel";

export default function App() {
  return (
    <main className="mx-auto max-w-3xl space-y-6 p-6">
      <h1 className="text-2xl font-bold">Bible — Operator Console</h1>
      <ListenPanel />
      <hr />
      <section className="space-y-4">
        <h2 className="text-xl font-semibold">Scripture</h2>
        <LookupBar />
        <ResultCard />
      </section>
      <hr />
      <SongsPanel />
      <hr />
      <DisplayPanel />
    </main>
  );
}
