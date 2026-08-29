# Bible App

An offline, projection-ready Bible application designed for churches. It features an integrated AI-powered voice recognition system, seamless dual-screen projection, and a mobile-friendly remote control interface—all operating fully offline.

## Features for End Users

- **Dual-Screen Projection**: Type a verse reference (e.g., `John 3:16`) and instantly project it to a secondary monitor or projector. The text beautifully fills the screen over a clean black background. Click **Blank** to clear it instantly.
- **AI Voice Transcription (STT)**: Speaks verse references or sermons out loud, and the app transcribes them in real-time. Built-in Whisper integration handles everything locally on your machine without an internet connection.
- **Mobile Remote Control**: Control the app directly from your phone! The app hosts a local webpage that you can open on your mobile browser (when connected to the same WiFi or Hotspot), letting you search and project verses remotely. 
- **Offline Translations**: Includes built-in Bible versions (e.g., KJV, WEB). Additional versions can be loaded locally.
- **Saved Recordings**: All recorded audio clips (sessions and utterances) are automatically saved locally on your Windows machine. You can find them in: 
  `%APPDATA%\com.kyere.bible-app\` (usually `C:\Users\<YourUsername>\AppData\Roaming\com.kyere.bible-app\`) under the `captures` or `sessions` folders.

## Which Flavor Should I Download?

We provide different installers to ensure optimal performance based on your hardware.

**1. Model Sizes:**
- **Small**: Uses the "small" Whisper model. Extremely fast and lightweight. Recommended for most machines.
- **Medium**: Uses the "medium" Whisper model. More accurate, but requires significantly more processing power (and the installer is much larger).

**2. Hardware Acceleration (GPU):**
- **CPU**: Runs entirely on your processor. Works on *any* computer, but transcription may be slower.
- **Vulkan**: Optimized for machines with Intel or AMD graphics cards.
- **CUDA**: Extremely fast, but only works if your computer has an **NVIDIA** graphics card.

*If you aren't sure, grab the **`small-personal-vulkan`** installer for the best mix of speed and compatibility!*

---

## Developer Guide

For developers looking to clone, fork, or run this app from the source code.

### Prerequisites
1. Install [Rust](https://rustup.rs/).
2. Install [Node.js](https://nodejs.org/) (v18 or higher).
3. (Windows) Install MSVC Build Tools and the WebView2 runtime.

### Setup and Running
1. Clone the repository and install frontend dependencies:
   ```bash
   npm install
   ```
2. **Provide Bible data**: 
   The app needs Bible data to run. You can download a public-domain WEB JSON and normalize it:
   ```bash
   python scripts/normalize_web.py <source_web.json>
   ```
   *Alternative*: Until you download a full Bible, the app uses a 3-verse placeholder (`data/fixtures/web.sample.json`) automatically copied to `data/web.canonical.json`, allowing you to smoke-test `John 3:16`, `Psalm 23`, and `Romans 8:28`.

3. **Start the Development Server**:
   ```bash
   npm run tauri dev
   ```
   This will boot up the Vite frontend and compile the Rust backend.

### Project Structure
- **`src-tauri/src/`**: The Rust backend. Contains the core logic for projection, the API server for the phone remote, and the Whisper AI backend integration (`accel.rs`, `stt.rs`).
- **`src/`**: The Vite + React (or vanilla JS) frontend.
- **`scripts/`**: Helper scripts for building the installers and generating data (e.g., `build_flavors.py`).
- **`data/`**: Processed `.canonical.json` files containing the Bible versions.

### Tests
- **Rust Backend**: `cd src-tauri && cargo test`
- **Frontend**: `npm run build`
