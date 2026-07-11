import { create } from "zustand";

/**
 * Root application store (Zustand).
 *
 * This is an intentionally minimal placeholder scaffolded in Task 1.
 * Later tasks will extend `AppState` with real slices (current reference,
 * search results, projection state, etc.) as those features are built.
 */
export interface AppState {
  /** Human-readable app version, useful as a smoke-test field for the store wiring. */
  version: string;
}

export const useAppStore = create<AppState>(() => ({
  version: "0.1.0",
}));
