# Manual Testing Guide (plain English)

This is a friendly checklist for trying out the app by hand. For each area it says
**what to do**, **what you should see**, and **what you should NOT see / current limits**.
Tick things off and jot notes; when you're back, give me the rundown.

> **How to start it:** open a terminal in the project folder and run `npm run tauri dev`.
> A window titled **"Bible — Operator Console"** opens. That's your control screen.
> A second black **Projection** screen appears the first time you project something
> (on your second monitor if you have one, otherwise fullscreen on your main screen —
> press the Windows key or Alt+Tab to get back to the console).

---

## 0. Very important context before you start

- **The Bible only has 3 verses in it right now** unless you've loaded the full Bible.
  Those three are **John 3:16**, **Psalm 23:1**, and **Romans 8:28**. Anything else will
  say "not found." That's expected. (To load the whole Bible: run
  `python scripts/normalize_web.py <a WEB bible json file>` once, then restart.)
- **Songs:** 5 classic hymns come pre-loaded the first time.
- If something looks frozen, that's usually the speech engine thinking for a second or two.

---

## 1. The Live Listening feature (the "magic" one)

**What to do:** Click **● Start listening**. Allow microphone access if Windows asks.
Speak clearly, then pause for about a second and a half. Try:
- "John chapter three sixteen."
- "Romans chapter eight verse twenty eight."
- "Psalm twenty three."

**What you should see:**
- A live **Transcript** on the left showing roughly what you said.
- On the right, under **Detected verses**, the matching verse appears with a little
  colored badge showing a confidence % (e.g. `95% · explicit`).
- Click a detected verse → it appears big on the Projection screen.

**Try the "smart" bits:**
- **Context memory:** Say "Let's turn to Romans chapter eight." Pause. Then separately say
  "Now look at verse twenty eight." → It should show **Romans 8:28** even though you didn't
  say "Romans" the second time. The badge will say `context`.
- **Quoting without a reference:** Say "For God so loved the world that he gave his only
  son." → It should suggest **John 3:16** with a `quote` badge. (This only works well when
  you quote close to the actual wording, and only for verses that are loaded.)
- **Model dropdown:** switch between **Base (normal)** and **Tiny (low-end PCs)** and feel
  the difference in speed and accuracy. Tiny is faster but less accurate.
- **Auto-project ≥90%:** tick this box, then speak a clear reference. High-confidence hits
  should project **automatically** without you clicking.

**What you should NOT see / limits:**
- It won't be perfect on the first try every single time, especially in a noisy room or with
  the Tiny model. Note how often it gets it right first try.
- Spelled-out numbers work ("three sixteen"), but very unusual phrasings may miss.
- It waits for a short pause before showing anything — a tiny delay is normal.

---

## 2. Scripture (typing references)

**What to do / see:**
- Type `John 3:16` in the Scripture box → **Look up** → the verse shows in a card →
  **Project** puts it on screen, **Blank** clears the screen.
- **＋ Service** adds it to your run order (see section 4).
- Try a **range**: `John 3:16-18` (only works fully once the whole Bible is loaded).
- **Search box** ("Search scripture by word"): type a word like `shepherd` → matching verses
  list; each can be projected or added to the service. (With only 3 verses loaded, results
  are limited.)

**Limits:** anything outside the 3 loaded verses says "not found" until you load the full Bible.

**Translation picker:** a **Translation** dropdown appears next to the "Scripture" heading **only
if you have more than one translation loaded**. With just WEB it stays hidden (that's expected).
If you load extra public-domain Bibles (as `*.canonical.json` files in the `data` folder), you can
switch between them here and it affects lookups, search, and live detection. Your choice is
remembered.

---

## 3. Songs

**What to do / see:**
- You should already see **5 hymns** (Amazing Grace, etc.).
- **Add a song:** type a title and paste lyrics. Leave a blank line between verses/choruses,
  or paste plain lyrics and click **Auto-format · every N lines** to split them for you.
  A **live preview** on the right shows exactly the slides you'll get.
- Click a song → its slides appear on the right → click **Project** on any slide, OR use your
  **arrow keys** (← →) to move between slides while they project live. **Esc** or **B** blanks.
  The current slide is highlighted green.
- **Edit** changes a song (and re-splits it). **Del** removes it. **＋** adds it to the service.
- **Filter songs…** box narrows the list by title.

**What you should NOT see:** editing/deleting should never crash; the live preview should
update as you type.

**Backup / share songs:** expand the **"Backup / share songs"** area at the bottom of Songs.
- **Export all songs (copy)** → copies your whole song library as text (also shown in a box).
  Paste it into a text file to keep as a backup.
- Paste that text back into the **import** box on another machine → **Import songs** → they're
  added. Great for moving your library between computers.

---

## 4. Service order (running a whole service)

**What to do / see:**
- Add a few verses (from Scripture) and songs (the **＋** button) → they appear as a numbered
  list under **Service order**.
- Click the first item to make it live, then drive the **whole service with your arrow keys**:
  → advances (and steps through a song's slides one by one before moving to the next item),
  ← goes back. The live item is highlighted.
- Reorder with ↑ ↓, remove with ✕, **clear** empties it.
- **Close the app and reopen it** → your service list should still be there (it's saved).

**Note:** arrow keys control **either** the Songs panel **or** the Service — whichever you last
clicked into. That's intentional so they don't fight each other.

---

## 5. Display controls (bottom of the console)

**What to do / see:**
- **Blank** (black "ready" screen), **Blackout** (fully black), **Logo** (a simple logo screen).
- **Message:** type an announcement → **Show** → it appears big on screen.
- **Countdown:** set minutes + a label → **Start** → a live ticking countdown appears on screen
  (great for "service starts in 5:00").
- **Font** slider and **Theme** (Dark / Light / Sepia) change how the projection looks — you
  should see the projection screen update live.
- **Stage display:** opens a third window with a big **clock** and whatever's currently on
  screen — meant for a screen the preacher/musicians can see.
- **Phone remote:** click it → a web address appears (like `http://192.168.x.x:8787`). Open
  that on your **phone (same Wi-Fi)** → you can type a reference and project it, or blank the
  screen, from your phone. Fully offline.
- **OBS / browser output:** the same address with `/projection` on the end
  (`http://192.168.x.x:8787/projection`) shows the live projection in any browser — you can add
  it as a Browser Source in OBS for streaming.

**The little "On screen" bar** at the very top of the console always shows what the congregation
is currently seeing, with a red dot when something is live.

---

## 6. Second monitor behavior

- With a second monitor plugged in, the Projection screen should jump to it and go fullscreen.
- With only one monitor, it goes fullscreen on your main screen (use Alt+Tab to get back).
- **Closing** the projection or stage window doesn't destroy it — projecting again brings it
  right back. (This is intentional so an accidental close mid-service doesn't kill the screen.)

---

## 7. Things that are deliberately NOT done yet (don't be surprised)

- **Reworded paraphrases** (very different words but same meaning) may not be caught yet — only
  close quotes are. The stronger AI paraphrase engine is planned but not built.
- **Only WEB is loaded by default.** Multiple translations are supported, but you'd need to add
  the extra Bible files yourself.
- No importing from CCLI/SongSelect or PowerPoint yet.
- No printing, no cloud sync (by design — this is an offline tool).

---

## 8. When you report back, it helps me most if you note:

1. **Live listening:** how often it got the verse right on the **first try** (Base vs Tiny),
   and any phrases it consistently missed.
2. Anything that **crashed, froze, or showed an error message**.
3. Anything that **looked wrong** on the projection screen (text cut off, wrong colors, etc.).
4. Which features felt **great**, and which felt **awkward** to use.
5. Whether the **phone remote** and **stage display** worked for you.

Thanks — take your time, and tell me everything you find.
