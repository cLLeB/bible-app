import "./App.css";
import { LookupBar } from "./components/LookupBar";
import { ResultCard } from "./components/ResultCard";

export default function App() {
  return (
    <main className="mx-auto max-w-xl space-y-4 p-6">
      <h1 className="text-2xl font-bold">Bible — Operator Console</h1>
      <LookupBar />
      <ResultCard />
    </main>
  );
}
