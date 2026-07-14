import { invoke } from "@tauri-apps/api/core";

export interface VersePayload {
  reference: string;
  book: string;
  bookOsis: string;
  chapter: number;
  verse: number;
  text: string;
  translation: string;
}

export type NavDir = "next-verse" | "prev-verse" | "next-chapter" | "prev-chapter";

export const navigate = (dir: NavDir): Promise<VersePayload | null> =>
  invoke<VersePayload | null>("navigate", { dir });

export const presentCoords = (
  bookOsis: string,
  chapter: number,
  verse: number,
): Promise<VersePayload> =>
  invoke<VersePayload>("present_coords", { bookOsis, chapter, verse });

export type ProjectionState =
  | { kind: "blank" }
  | { kind: "blackout" }
  | { kind: "logo" }
  | { kind: "verse"; text: string; caption: string }
  | { kind: "song"; text: string; caption: string }
  | { kind: "message"; text: string }
  | { kind: "countdown"; targetMs: number; label: string };

export interface ProjectionSettings {
  fontScale: number;
  theme: "dark" | "light" | "sepia";
}

export const setProjection = (next: ProjectionState): Promise<void> =>
  invoke<void>("set_projection", { next });

export const getProjectionSettings = (): Promise<ProjectionSettings> =>
  invoke<ProjectionSettings>("get_projection_settings");

export const setProjectionSettings = (settings: ProjectionSettings): Promise<void> =>
  invoke<void>("set_projection_settings", { settings });

export const showStage = (): Promise<void> => invoke<void>("show_stage");

export interface StageSlot {
  text: string;
  caption: string;
}

export interface StageInfo {
  current: StageSlot | null;
  next: StageSlot | null;
  message: string;
}

export const getStage = (): Promise<StageInfo> => invoke<StageInfo>("get_stage");

export const setStage = (
  current: StageSlot | null,
  next: StageSlot | null,
): Promise<void> => invoke<void>("set_stage", { current, next });

export const setStageMessage = (message: string): Promise<void> =>
  invoke<void>("set_stage_message", { message });

/// Next verse from the current cursor, without projecting it (stage preview).
export const peekNext = (): Promise<VersePayload | null> =>
  invoke<VersePayload | null>("peek_next");

export const startRemote = (): Promise<string> => invoke<string>("start_remote");

export interface SongSummary {
  id: number;
  title: string;
  author: string | null;
  builtIn: boolean;
}

export interface Slide {
  orderIndex: number;
  text: string;
}

export const lookupReference = (query: string): Promise<VersePayload> =>
  invoke<VersePayload>("lookup_reference", { query });

export const searchScripture = (query: string): Promise<VersePayload[]> =>
  invoke<VersePayload[]>("search_scripture", { query });

export const relatedVerses = (
  bookOsis: string,
  chapter: number,
  verse: number,
): Promise<VersePayload[]> =>
  invoke<VersePayload[]>("related_verses", { bookOsis, chapter, verse });

export const chunkPassage = (text: string, maxChars: number): Promise<string[]> =>
  invoke<string[]>("chunk_passage", { text, maxChars });

export const recordChoice = (
  transcript: string,
  bookOsis: string,
  chapter: number,
  verse: number,
): Promise<void> =>
  invoke<void>("record_choice", { transcript, bookOsis, chapter, verse });

export interface TranslationInfo {
  code: string;
  name: string;
}

export const listTranslations = (): Promise<TranslationInfo[]> =>
  invoke<TranslationInfo[]>("list_translations");

export interface CatalogEntry {
  code: string;
  name: string;
  installed: boolean;
  licensed: boolean;
}

export interface FlavorInfo {
  tier: "personal" | "distribution";
  models: SttModel[];
  defaultModel: SttModel;
}

export const appFlavor = (): Promise<FlavorInfo> => invoke<FlavorInfo>("app_flavor");

/** False while first-run seeding is still installing the bundled library. */
export const libraryReady = (): Promise<boolean> => invoke<boolean>("library_ready");

export const translationCatalog = (): Promise<CatalogEntry[]> =>
  invoke<CatalogEntry[]>("translation_catalog");

export const downloadTranslation = (code: string): Promise<number> =>
  invoke<number>("download_translation", { code });

export const getTranslation = (): Promise<string> => invoke<string>("get_translation");

export const setTranslation = (code: string): Promise<void> =>
  invoke<void>("set_translation", { code });

export const projectVerse = (payload: VersePayload): Promise<void> =>
  invoke<void>("project_verse", { payload });

export const blankProjection = (): Promise<void> => invoke<void>("blank_projection");

export const getProjection = (): Promise<ProjectionState> =>
  invoke<ProjectionState>("get_projection");

export const addSong = (
  title: string,
  author: string | null,
  lyrics: string,
): Promise<number> => invoke<number>("add_song", { title, author, lyrics });

export const listSongs = (): Promise<SongSummary[]> => invoke<SongSummary[]>("list_songs");

export const getSongSlides = (songId: number): Promise<Slide[]> =>
  invoke<Slide[]>("get_song_slides", { songId });

export const projectSlide = (songId: number, index: number): Promise<void> =>
  invoke<void>("project_slide", { songId, index });

export interface SongDetail {
  id: number;
  title: string;
  author: string | null;
  lyrics: string;
}

export const getSong = (songId: number): Promise<SongDetail | null> =>
  invoke<SongDetail | null>("get_song", { songId });

export const updateSong = (
  songId: number,
  title: string,
  author: string | null,
  lyrics: string,
): Promise<void> => invoke<void>("update_song", { songId, title, author, lyrics });

export const deleteSong = (songId: number): Promise<void> =>
  invoke<void>("delete_song", { songId });

export const exportSongs = (): Promise<string> => invoke<string>("export_songs");
export const importSongs = (json: string): Promise<number> =>
  invoke<number>("import_songs", { json });

export interface Candidate {
  verse: VersePayload;
  confidence: number;
  source: string; // "explicit" | "fuzzy" | "context"
}

export type SttModel = "tiny" | "base" | "small" | "medium";

export const startListening = (model: SttModel): Promise<void> =>
  invoke<void>("start_listening", { model });
export const stopListening = (): Promise<void> => invoke<void>("stop_listening");

// ---- Voice calibration ----
// Tunes the recognizer to this speaker's voice, mic and room. Everything stays
// on the machine: the recordings, the comparison and the result.

export interface ScriptLine {
  index: number;
  say: string;
  expect: string;
}

export interface CalibrationClip {
  index: number;
  seconds: number;
}

export interface ConfigScore {
  label: string;
  resolved: number;
  total: number;
  secondsPerClip: number;
}

export interface CalibrationResult {
  best: ConfigScore;
  all: ConfigScore[];
  baseline: ConfigScore;
}

// ---- Audio input ----
// The laptop mic hears the room; a sound-desk feed hears the preacher's microphone.
// Everything the recognizer was measured against is the latter.

export interface AudioInputs {
  /** null = the system default input. */
  chosen: string | null;
  all: string[];
}

export const audioInputs = (): Promise<AudioInputs> => invoke<AudioInputs>("audio_inputs");

export const setAudioInput = (name: string | null): Promise<void> =>
  invoke<void>("set_audio_input", { name });

/** Listens for ~3s and returns the loudest level heard (0..1). */
export const testAudioInput = (): Promise<number> => invoke<number>("test_audio_input");

export interface VoiceProfiles {
  active: string;
  all: string[];
}

/** Who is preaching today. Settings are stored per speaker, so calibrating a guest
 *  never disturbs the regular preacher's tuning. */
export const voiceProfiles = (): Promise<VoiceProfiles> => invoke<VoiceProfiles>("voice_profiles");

export const setVoiceProfile = (name: string): Promise<void> =>
  invoke<void>("set_voice_profile", { name });

// ---- Learning from a sermon recording ----
// Twelve read lines tune the app for someone reading a script at a laptop. A sermon
// tunes it for a preacher: their pace, their accent, how they name references, and the
// signal path the church actually uses. Whatever they read aloud is the ground truth.

export interface LearnProgress {
  stage: string;
  done: number;
  total: number;
  /** Whole job, 0..100. */
  percent: number;
  secondsLeft: number;
}

export interface LearnResult {
  profile: string;
  recordings: number;
  minutes: number;
  /** The version their readings matched, if they agree on one. */
  translation: string | null;
  /** Speech threshold measured from their sound feed. */
  speechAbove: number;
  referencesFound: number;
  before: number;
  after: number;
  settings: string;
  learnedNames: string[];
}

export const learnFromRecordings = (paths: string[], model: SttModel): Promise<LearnResult> =>
  invoke<LearnResult>("learn_from_recordings", { paths, model });

export const calibrationScript = (): Promise<ScriptLine[]> =>
  invoke<ScriptLine[]>("calibration_script");

/** Blocks until the speaker finishes the line (or nobody speaks for 20s). */
export const recordCalibrationLine = (index: number): Promise<CalibrationClip> =>
  invoke<CalibrationClip>("record_calibration_line", { index });

export const runCalibration = (model: SttModel): Promise<CalibrationResult> =>
  invoke<CalibrationResult>("run_calibration", { model });
