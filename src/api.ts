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

/** Project the same verse in the active translation + a secondary one, side by side. */
export const projectParallel = (
  bookOsis: string,
  chapter: number,
  verse: number,
  secondary: string,
): Promise<VersePayload> =>
  invoke<VersePayload>("project_parallel", { bookOsis, chapter, verse, secondary });

export type ProjectionState =
  | { kind: "blank" }
  | { kind: "blackout" }
  | { kind: "logo" }
  | { kind: "verse"; text: string; caption: string }
  | { kind: "song"; text: string; caption: string }
  | { kind: "image"; src: string }
  | {
      kind: "video";
      src: string;
      title: string;
      paused: boolean;
      muted: boolean;
      looping: boolean;
    }
  | {
      kind: "parallel";
      primaryText: string;
      primaryCode: string;
      secondaryText: string;
      secondaryCode: string;
      caption: string;
    }
  | { kind: "message"; text: string }
  | { kind: "countdown"; targetMs: number; label: string };

/** Where the congregation screen's background comes from. `image`/video are a
 *  later slice; the shape is forward-compatible so that's a render-only addition. */
export interface Background {
  kind: "color" | "gradient" | "image" | "video";
  color: string;
  color2: string;
  angle: number;
  /** Absolute path to an image/video file (empty for color/gradient). */
  src: string;
  /** How media fills the screen. */
  fit: "cover" | "contain";
  /** 0..1 dark overlay over media for text legibility. */
  dim: number;
}

export interface TextStyle {
  fontFamily: string;
  color: string;
  captionColor: string;
  align: "center" | "left" | "right";
  weight: number;
  shadow: boolean;
  uppercase: boolean;
}

/** Where things sit on the slide, as opposed to how they look.
 *
 *  Part of a theme rather than a global setting, so a church can keep more than one
 *  arrangement and switch: the reference under the verse for a teaching series,
 *  hidden for a reading, the words held high when a stream keys a lower third over
 *  the bottom of the screen. */
export interface Layout {
  /** Where the reference or song title goes. */
  captionPosition: "below" | "above" | "hidden";
  /** Where the body sits in the frame. */
  vertical: "center" | "top" | "bottom";
  /** Caption size relative to the body; 1.0 is the original relationship. */
  captionScale: number;
  /** Percent of screen width kept clear down each side. */
  sideMargin: number;
}

/** A named, self-contained look for the congregation screen. */
export interface Theme {
  id: string;
  name: string;
  background: Background;
  text: TextStyle;
  layout: Layout;
  builtIn: boolean;
}

export interface ProjectionSettings {
  fontScale: number;
  theme: Theme;
}

export const setProjection = (next: ProjectionState): Promise<void> =>
  invoke<void>("set_projection", { next });

/** Project a full-screen image (data URL or path) — e.g. a rendered PDF page. */
export const projectImage = (src: string): Promise<void> =>
  setProjection({ kind: "image", src });

// ---- Media library ----

/** One item in the media library. `present` is false once the file has moved. */
export interface MediaLibraryItem {
  id: number;
  path: string;
  title: string;
  /** `audio` is in the library but never takes the screen — see `AudioState`. */
  kind: "image" | "video" | "audio";
  /** The document this page came from; empty for a standalone file. */
  deck: string;
  present: boolean;
}

export const listMedia = (): Promise<MediaLibraryItem[]> =>
  invoke<MediaLibraryItem[]>("list_media");

/** Add files by absolute path; returns the refreshed library. */
export const addMedia = (paths: string[]): Promise<MediaLibraryItem[]> =>
  invoke<MediaLibraryItem[]>("add_media", { paths });

export const removeMedia = (id: number): Promise<MediaLibraryItem[]> =>
  invoke<MediaLibraryItem[]>("remove_media", { id });

export const renameMedia = (id: number, title: string): Promise<MediaLibraryItem[]> =>
  invoke<MediaLibraryItem[]>("rename_media", { id, title });

export const moveMedia = (id: number, up: boolean): Promise<MediaLibraryItem[]> =>
  invoke<MediaLibraryItem[]>("move_media", { id, up });

export const projectMedia = (id: number): Promise<MediaLibraryItem> =>
  invoke<MediaLibraryItem>("project_media", { id });

/** One screen the operating system is offering. */
export interface DisplayInfo {
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  /** The screen the desktop lives on: the operator's own. */
  primary: boolean;
}

/** Every screen, plus the name the operator chose ("" = automatic). */
export const listDisplays = (): Promise<[DisplayInfo[], string]> =>
  invoke<[DisplayInfo[], string]>("list_displays");

/** Send output to a screen by name; empty means automatic. */
export const setOutputDisplay = (name: string): Promise<void> =>
  invoke<void>("set_output_display", { name });

/** Office suites on this machine that can convert a deck: [name, path], best first. */
export const listConverters = (): Promise<[string, string][]> =>
  invoke<[string, string][]>("list_converters");

/** Point the app at a converter it did not find; empty clears the override. */
export const setConverter = (path: string): Promise<[string, string][]> =>
  invoke<[string, string][]>("set_converter", { path });

/** A deck as something we can render: PowerPoint is converted, PDF passes through. */
export const deckAsPdf = (path: string): Promise<string> =>
  invoke<string>("deck_as_pdf", { path });

/** Store one rendered deck page as a library image. */
export const importSlide = (
  deck: string,
  index: number,
  pngBase64: string,
): Promise<MediaLibraryItem> =>
  invoke<MediaLibraryItem>("import_slide", { deck, index, pngBase64 });

/** Step to the previous/next page within the same deck. Null at either end. */
export const stepDeck = (id: number, forward: boolean): Promise<MediaLibraryItem | null> =>
  invoke<MediaLibraryItem | null>("step_deck", { id, forward });

/** The projection window reporting that the live video reached its end. */
export const videoEnded = (): Promise<void> => invoke<void>("video_ended");

/** Transport for the video already on screen; all three travel together. */
export const setVideoPlayback = (
  paused: boolean,
  muted: boolean,
  looping: boolean,
): Promise<void> => invoke<void>("set_video_playback", { paused, muted, looping });

export const seekVideo = (positionMs: number): Promise<void> =>
  invoke<void>("seek_video", { positionMs });

/**
 * Sound running under the service. Deliberately not a `ProjectionState`: music
 * plays *while* a verse or song holds the screen, so it travels on its own
 * channel and leaves the wall alone.
 */
export interface AudioState {
  src: string;
  title: string;
  paused: boolean;
  looping: boolean;
  /** 0..1. */
  volume: number;
}

export const getAudio = (): Promise<AudioState> => invoke<AudioState>("get_audio");

/** Start a sound file from the media library playing. */
export const playAudio = (id: number): Promise<MediaLibraryItem> =>
  invoke<MediaLibraryItem>("play_audio", { id });

/** Transport for the sound already loaded; all three travel together. */
export const setAudioPlayback = (
  paused: boolean,
  looping: boolean,
  volume: number,
): Promise<void> => invoke<void>("set_audio_playback", { paused, looping, volume });

export const seekAudio = (positionMs: number): Promise<void> =>
  invoke<void>("seek_audio", { positionMs });

export const stopAudio = (): Promise<void> => invoke<void>("stop_audio");

export const slideshowRunning = (): Promise<boolean> => invoke<boolean>("slideshow_running");

export const startSlideshow = (seconds: number, looping: boolean): Promise<void> =>
  invoke<void>("start_slideshow", { seconds, looping });

export const stopSlideshow = (): Promise<void> => invoke<void>("stop_slideshow");

export const getProjectionSettings = (): Promise<ProjectionSettings> =>
  invoke<ProjectionSettings>("get_projection_settings");

/** Built-in themes first, then the operator's custom ones. */
export const listThemes = (): Promise<Theme[]> => invoke<Theme[]>("list_themes");

/** Make a theme live; returns the resolved settings now in force. */
export const setActiveTheme = (id: string): Promise<ProjectionSettings> =>
  invoke<ProjectionSettings>("set_active_theme", { id });

export const setFontScale = (scale: number): Promise<void> =>
  invoke<void>("set_font_scale", { scale });

/** Create or update a custom theme (built-ins are duplicated before editing). */
export const saveTheme = (theme: Theme): Promise<void> =>
  invoke<void>("save_theme", { theme });

export const deleteTheme = (id: string): Promise<void> =>
  invoke<void>("delete_theme", { id });

export const showStage = (): Promise<void> => invoke<void>("show_stage");

/** A lower-third alert over the congregation screen. Empty text = none;
 *  `untilMs` is an epoch-ms auto-dismiss time (0 = stays until cleared). */
export interface Alert {
  text: string;
  untilMs: number;
}

/** A line of announcements crawling under whatever is on the wall.
 *
 *  Not an alert, which interrupts and then goes; not a projection state, which would
 *  take the screen. This runs underneath a verse for twenty minutes without
 *  disturbing it. */
export interface Ticker {
  /** Empty means nothing is showing. */
  text: string;
  /** Seconds for one pass across the screen. Lower is faster. */
  seconds: number;
  /** Hold it still instead of scrolling, for one short line. */
  still: boolean;
}

export const getTicker = (): Promise<Ticker> => invoke<Ticker>("get_ticker");

export const setTicker = (text: string, seconds: number, still: boolean): Promise<void> =>
  invoke<void>("set_ticker", { text, seconds, still });

export const getAlert = (): Promise<Alert> => invoke<Alert>("get_alert");

export const showAlert = (text: string, seconds: number): Promise<void> =>
  invoke<void>("show_alert", { text, seconds });

export const clearAlert = (): Promise<void> => invoke<void>("clear_alert");

export interface StageSlot {
  text: string;
  caption: string;
}

export interface StageTimer {
  mode: "off" | "countup" | "countdown";
  anchorMs: number;
}

export interface StageInfo {
  current: StageSlot | null;
  next: StageSlot | null;
  message: string;
  timer: StageTimer;
}

export const getStage = (): Promise<StageInfo> => invoke<StageInfo>("get_stage");

/** Start/stop the stage-only timer. mode: "countup" | "countdown" | "off". */
export const setStageTimer = (mode: StageTimer["mode"], seconds: number): Promise<void> =>
  invoke<void>("set_stage_timer", { mode, seconds });

export const setStage = (
  current: StageSlot | null,
  next: StageSlot | null,
): Promise<void> => invoke<void>("set_stage", { current, next });

export const setStageMessage = (message: string): Promise<void> =>
  invoke<void>("set_stage_message", { message });

/// Next verse from the current cursor, without projecting it (stage preview).
export const peekNext = (): Promise<VersePayload | null> =>
  invoke<VersePayload | null>("peek_next");

/**
 * Every address a phone might reach the remote on, best guess first. The server
 * listens on all of them; which one works depends on the network the phone is
 * sharing, so the operator picks. Opening one is all there is to connecting.
 */
export const startRemote = (): Promise<string[]> => invoke<string[]>("start_remote");

/** One item in a Planning Center plan. */
export interface PlanItem {
  title: string;
  kind: string;
}

export const pcoImportPlan = (
  appId: string,
  secret: string,
  serviceTypeId: string,
  planId: string,
): Promise<PlanItem[]> =>
  invoke<PlanItem[]>("pco_import_plan", { appId, secret, serviceTypeId, planId });

/** Send a raw PJLink command body (e.g. "%1POWR 1") to a LAN projector. */
export const pjlinkCommand = (
  host: string,
  port: number,
  password: string,
  body: string,
): Promise<string> => invoke<string>("pjlink_command", { host, port, password, body });

export interface SongSummary {
  id: number;
  title: string;
  author: string | null;
  builtIn: boolean;
}

export interface Slide {
  orderIndex: number;
  text: string;
  /** Section label ("Verse 1", "Chorus", …) for the operator, or null. */
  label: string | null;
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

/** One line of the CCLI song-usage report. */
export interface UsageRow {
  title: string;
  author: string | null;
  times: number;
  lastUsed: string | null;
}

export const songUsageReport = (): Promise<UsageRow[]> =>
  invoke<UsageRow[]>("song_usage_report");

export const exportSongs = (): Promise<string> => invoke<string>("export_songs");
export const importSongs = (json: string): Promise<number> =>
  invoke<number>("import_songs", { json });

export interface Candidate {
  verse: VersePayload;
  confidence: number;
  source: string; // "explicit" | "fuzzy" | "context"
}

export type SttModel = "small" | "medium";

/** Which speakers video and music come out of.
 *
 *  Separate from the projection display on purpose, because Windows keeps them
 *  separate: sound goes to the system default output regardless of which monitor a
 *  window sits on. Put the projection on a TV over HDMI and, left alone, the picture
 *  goes to the TV while the sound stays in the laptop.
 */
export interface AudioOutput {
  /** Browser device id, passed to setSinkId. Empty means the Windows default. */
  id: string;
  label: string;
}

export const getAudioOutput = (): Promise<AudioOutput> => invoke<AudioOutput>("get_audio_output");

export const setAudioOutput = (id: string, label: string): Promise<AudioOutput> =>
  invoke<AudioOutput>("set_audio_output", { id, label });

/** Where whisper can run on this machine. */
export interface AccelOption {
  key: "cpu" | "vulkan" | "cuda";
  label: string;
  /** This build ships a whisper build for it. */
  installed: boolean;
  /** This machine's drivers can run it. Separate from `installed`: a build can
   *  carry CUDA to a laptop with an AMD card in it. */
  usable: boolean;
  /** Milliseconds per utterance, once measured here. */
  measuredMs: number | null;
}

export interface AccelStatus {
  /** "auto", or a forced backend key. */
  preference: string;
  chosen: string;
  chosenLabel: string;
  threads: number;
  options: AccelOption[];
  /** What is doing the work right now, named by whisper itself. Empty when nothing
   *  is listening, or when it named nothing, which means the processor. */
  device: string;
}

export const accelStatus = (): Promise<AccelStatus> => invoke<AccelStatus>("accel_status");

/** "auto" uses the fastest thing that works here. Forcing a backend is the way
 *  back to the processor if a graphics driver misbehaves mid-service. */
export const setAccelPreference = (preference: string): Promise<AccelStatus> =>
  invoke<AccelStatus>("set_accel_preference", { preference });

/** Time every backend this machine can run and keep the fastest. Takes the better
 *  part of a minute; reports progress on "accel-trial" and finishes on
 *  "accel-measured" (or "accel-error"). Refused while listening. */
export const measureAccel = (model?: SttModel): Promise<void> =>
  invoke<void>("measure_accel", { model });

export const startListening = (model: SttModel): Promise<void> =>
  invoke<void>("start_listening", { model });
export const stopListening = (): Promise<void> => invoke<void>("stop_listening");

/** Is it listening right now? Asked when the panel mounts, because switching to
 *  Prepare unmounts it and the events alone leave it showing the wrong button. */
export const listeningEnabled = (): Promise<boolean> => invoke<boolean>("listening_enabled");

// Record the live service (for on-device learning). Off by default; set before Start.
// Refused for a speaker who hasn't been opted in.
export const setRecording = (on: boolean): Promise<void> => invoke<void>("set_recording", { on });
export const recordingEnabled = (): Promise<boolean> => invoke<boolean>("recording_enabled");

/** Has today's speaker been opted in to having their services recorded? */
export const recordingConsent = (): Promise<boolean> => invoke<boolean>("recording_consent");

/** Opt today's speaker in or out. Opting out disarms recording immediately. */
export const setRecordingConsent = (on: boolean): Promise<void> =>
  invoke<void>("set_recording_consent", { on });

/** Delete every recording of today's speaker and withdraw the opt-in. Returns how many went. */
export const forgetRecordings = (): Promise<number> => invoke<number>("forget_recordings");

/** One thing that happened during a recorded service. `kind` decides how much the
 *  learner trusts it: "confirmed" (speaker read it aloud) > "operator" (the operator
 *  chose it) > "auto" (the app chose it, unconfirmed); "corrected" is a labelled
 *  mistake — the app projected `replaced` and the operator swapped in this verse. */
export interface Moment {
  kind: "auto" | "operator" | "confirmed" | "corrected";
  reference: string;
  bookOsis: string;
  chapter: number;
  verse: number;
  /** The recognizer's confidence, when the app chose it. */
  confidence: number;
  /** How it was recognized ("explicit", "fuzzy", "quote", …). */
  source: string;
  /** What the speaker was heard to say around this moment. */
  spoken: string;
  /** For a correction: the wrong reference the operator replaced. */
  replaced: string;
}

/** Log a moment against the service being recorded. Only meaningful while recording;
 *  the backend keeps them in memory and writes them into the session on stop. */
export const recordMoment = (moment: Moment): Promise<void> =>
  invoke<void>("record_moment", { moment });

/** One recorded service belonging to the speaker preaching today. */
export interface SessionSummary {
  /** Timestamp name — also how approve/discard address this service. */
  name: string;
  savedAt: string;
  minutes: number;
  /** Nothing is learned from a service until the operator has approved it. */
  approved: boolean;
  moments: Moment[];
}

/** Recorded services for today's speaker, newest first. */
export const reviewSessions = (): Promise<SessionSummary[]> =>
  invoke<SessionSummary[]>("review_sessions");

/** Let the app learn from this service. Approval is of the whole recording: learning
 *  listens to it and works out what was read aloud for itself, so there is no way to
 *  approve part of one. */
export const approveSession = (name: string): Promise<void> =>
  invoke<void>("approve_session", { name });

export const discardSession = (name: string): Promise<void> =>
  invoke<void>("discard_session", { name });

// ---- Learning from the church's own services ----
// The app can tune itself on the services it recorded — but only ones the operator
// approved, only when the machine is plainly not needed, and never without asking.

/** What a learning pass would like to change, and the evidence for it. */
export interface Proposal {
  profile: string;
  /** The tuning itself. Opaque to the console: it is shown by its numbers, not its innards. */
  next: unknown;
  modelFile: string;
  /** Plain-language name of the proposed settings. */
  settings: string;
  /** Passages read aloud across the services — the marks available. */
  references: number;
  /** How many of them the settings in force recover. */
  incumbentScore: number;
  /** How many the proposed settings recover. */
  newScore: number;
  /** Misheard book names the pass found that the speaker didn't already have. */
  newAliases: string[];
  minutes: number;
  sessions: string[];
  at: string;
}

export interface LearningStatus {
  profile: string;
  approved: number;
  running: boolean;
  /** Why learning won't start right now; null when it could. */
  blocked: string | null;
  proposal: Proposal | null;
  canRollback: boolean;
  canReset: boolean;
}

export const learningStatus = (): Promise<LearningStatus> =>
  invoke<LearningStatus>("learning_status");

/** Start a background pass over the approved services. Leaves a proposal, changes nothing. */
export const learnNow = (): Promise<void> => invoke<void>("learn_now");

export const stopLearning = (): Promise<void> => invoke<void>("stop_learning");

/** Take up the proposal. Reversible: what it replaces is kept. */
export const acceptProposal = (): Promise<boolean> => invoke<boolean>("accept_proposal");

export const rejectProposal = (): Promise<void> => invoke<void>("reject_proposal");

/** Put back the tuning in force before the last change. */
export const rollbackProfile = (): Promise<boolean> => invoke<boolean>("rollback_profile");

/** Put this speaker back exactly as the app shipped them. */
export const resetProfileToBaked = (): Promise<boolean> =>
  invoke<boolean>("reset_profile_to_baked");

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
  /** null = nothing chosen. There is no default: the app will not listen until one is. */
  chosen: string | null;
  /** Feeds from the sound desk — what the app is for. */
  all: string[];
  /** This machine's own microphones, kept apart from the desk feeds on purpose. */
  roomMics: string[];
  /** Is the app pointed at the room right now rather than the desk? */
  onRoomMic: boolean;
}

export const audioInputs = (): Promise<AudioInputs> => invoke<AudioInputs>("audio_inputs");

/**
 * Choose the input. `roomMic` is the operator saying "yes, this laptop's own
 * microphone, I know it hears the room" — the backend honours it only for a device
 * that really is one, and clears it as soon as anything else is picked.
 */
export const setAudioInput = (name: string | null, roomMic = false): Promise<void> =>
  invoke<void>("set_audio_input", { name, roomMic });

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

export const removeVoiceProfile = (name: string): Promise<void> =>
  invoke<void>("remove_voice_profile", { name });

/// The baked-in preachers that can't be removed.
export const PROTECTED_PROFILES = ["President", "Vice-President"];

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
