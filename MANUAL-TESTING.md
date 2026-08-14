# Manual Testing Guide (plain English)

This is a friendly checklist for trying out the app by hand. For each area it says
**what to do**, **what you should see**, and **what you should NOT see / current limits**.
Tick things off and jot notes; when you're back, give me the rundown.

> **How to start it:** open a terminal in the project folder and run `npm run tauri dev`.
> A window titled **"Bible Operator Console"** opens. That's your control screen.
> A second black **Projection** screen appears the first time you project something
> (on your second monitor if you have one, otherwise fullscreen on your main screen;
> press the Windows key or Alt+Tab to get back to the console).

---

## 0. Very important context before you start

- **The whole Bible is now loaded, in 6 translations** (KJV, WEB, ASV, YLT, BBE, Darby),
  ~31,000 verses each. Every verse should resolve now. The **first launch** after this may take
  a few extra seconds (it builds a search index once); later launches are fast.
- A **Translation** dropdown (next to "Scripture") lets you switch between the 6 versions.
- **Speech models:** you can pick **Tiny** (fastest, low-end), **Base** (normal), or **Small**
  (most accurate, needs a stronger PC).
- **Songs:** ~32 classic public-domain hymns come pre-loaded (Amazing Grace, It Is Well, Blessed Assurance, and many more).
- If something looks frozen, that's usually the speech engine thinking for a second or two.

## 0.5. THE BIG NEW THING: Presentation-mode navigation (test this hard)

This is the heart of the project: **getting the right scripture on screen faster than any
operator could alone.** Once *any* verse is on screen, a green **"Presenting"** bar appears and
you can fly around scripture:

- **Arrow keys:** → (or ↓) next verse, ← (or ↑) previous verse. **PageDown** next chapter,
  **PageUp** previous chapter. It crosses chapter and book boundaries automatically.
- **Jump box** (in the green bar): type `15` + Enter to jump to verse 15 of the current chapter,
  or `4:5` + Enter to jump to chapter 4 verse 5 of the current book.
- **By voice** (while listening): say **"next verse"**, **"previous verse"**, **"next chapter"**,
  or **"previous chapter"** and it moves, hands-free.
- **Instant project:** in the Scripture box, type a reference and just press **Enter**. It goes
  straight to the screen (no extra click). `John 3.16` with a dot works too. Shift+Enter previews
  without projecting.
- **Recent strip:** recently shown verses appear as little buttons under the Scripture box;
  click one to re-project instantly.

**Test the real scenario:** put `John 3:16` on screen (type it + Enter). Now imagine the preacher
saying "…now look at verse 15… and verse 16… let's jump to chapter 4…". Use the **arrow keys** to
follow along, or say **"next verse"**. This should feel *fast*.

---

## 1. The Live Listening feature (the "magic" one)

**First, give it something to listen to.** Open **Before the service** → *Sound input*.
For a real service you pick the feed from the sound desk. To try it at a desk with no
cable, open the group headed **"Demonstration only (hears the room, not the preacher)"**,
pick this laptop's own microphone, and press **Use it to demonstrate**. The app will
show a `room mic` badge until you choose something else. That badge is meant to stay
up, so nobody runs a live service on the room by accident.

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
- **Model dropdown:** switch between **Tiny**, **Base**, and **Small** and feel
  the difference in speed and accuracy. Tiny is faster but less accurate.
- **Auto-project ≥82%:** tick this box (threshold adjustable). Solid detections project **automatically**. On the Small model this should fire reliably.

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
- Try a **range**: `John 3:16-18`, and a spoken/typed translation: `John 3:16 in ASV`.
- **Search box** ("Search scripture by word"): type a word like `shepherd` → matching verses
  list; each can be projected or added to the service.

**Limits:** essentially everything resolves now that the whole Bible is loaded.

**Translation picker:** a **Translation** dropdown appears next to the "Scripture" heading **only
if you have more than one translation loaded**. With just WEB it stays hidden (that's expected).
If you load extra public-domain Bibles (as `*.canonical.json` files in the `data` folder), you can
switch between them here and it affects lookups, search, and live detection. Your choice is
remembered.

---

## 3. Songs

**What to do / see:**
- You should already see ~32 hymns (Amazing Grace, To God Be the Glory, etc.).
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

## 5. Live and Prepare tabs

The console is split in two, using the tabs next to the logo:

- **Live** — what you touch while a service is running: the presenter, listening and detected
  verses, scripture lookup, the service order, and the display controls.
- **Prepare** — what you set up beforehand: songs, PDF/PowerPoint import, outputs (stage
  display, phone remote, OBS), themes, and projector control. **Planning Center import** and
  **Song usage (CCLI)** are tucked inside a collapsed **Admin & integrations** section at the
  bottom — most churches never need either.

**What to check:** the tab you were last on is remembered when you close and reopen the app,
and the **"On screen" bar** stays visible on both tabs so you always know what the congregation
is seeing.

### Display controls (Live tab)
- **Blank** (black "ready" screen), **Blackout** (fully black), **Logo** (a simple logo screen).
- **Message:** type an announcement → **Show** → it appears big on screen.
- **Countdown:** set minutes + a label → **Start** → a live ticking countdown appears on screen
  (great for "service starts in 5:00").
- **Font** slider changes how big the projected text is — the projection screen updates live.
  (Theme and backgrounds are on the Prepare tab.)

### Outputs (Prepare tab)
- **Stage display:** opens a third window with a big **clock** and whatever's currently on
  screen — meant for a screen the preacher/musicians can see.
- **Phone remote:** click it → an address (like `http://192.168.x.x:8787`) **and a six-character
  pairing code** appear. On your phone (same Wi-Fi) open the address, enter the code once, and
  the phone remembers it. The code stops anyone else on the network projecting to your screen,
  and it changes each time the app restarts. Fully offline.
- **OBS / browser output:** the same address with `/projection` on the end
  (`http://192.168.x.x:8787/projection`) shows the live projection in any browser — add it as a
  Browser Source in OBS. This one needs **no** pairing code, so it keeps working unattended.

### What the phone remote can do
Move through the passage (**◀ verse / verse ▶**, **◀ chapter / chapter ▶**), go to a reference,
**search by words** and tap a result to project it, **Blank / Blackout / Logo**, start and stop
**listening**, and put an **alert** over the screen ("Parent needed in the nursery").

It does **not** show the service order or the detected-verse list — those live in the console
itself, not in the part of the app the phone can reach.

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
