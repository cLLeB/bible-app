import { create } from "zustand";
import type { VersePayload } from "./api";

interface LookupState {
  query: string;
  result: VersePayload | null;
  error: string | null;
  setQuery: (q: string) => void;
  setResult: (r: VersePayload | null) => void;
  setError: (e: string | null) => void;
}

export const useLookupStore = create<LookupState>((set) => ({
  query: "",
  result: null,
  error: null,
  setQuery: (query) => set({ query }),
  setResult: (result) => set({ result, error: null }),
  setError: (error) => set({ error, result: null }),
}));
