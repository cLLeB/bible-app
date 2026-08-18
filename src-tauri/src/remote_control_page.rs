//! The operator's phone remote: markup and styling.
//!
//! Shaped for one hand at the back of a hall, not for a desk. Three things are
//! always on screen no matter which tab is open, because they are the three an
//! operator standing in an aisle reaches for without looking: what the
//! congregation is seeing, the microphone, and the controls for whatever is
//! live right now. Everything else lives behind a thumb-height tab bar.
//!
//! The script is in `remote_control_js`; the two are concatenated by
//! `remote_pages`. They are kept apart only for size: as one file this page runs
//! past what is comfortable to read or edit.

/// Design tokens, layout and components. Light is the same mild brightness as
/// the operator console: an off-white that reads on a phone in a bright hall
/// without glaring in a dark one. The page follows the phone's own appearance
/// setting, and the ☀/☾ button overrides it for the visit. Nothing is stored, so
/// a fresh browser still works on first load.
const REMOTE_CSS: &str = r#"
:root{
 color-scheme:light dark;
 --bg:#dfe3ea;--surface:#ffffff;--raised:#f4f6fa;--field:#ffffff;--border:#c4ccd8;
 --text:#172230;--muted:#566072;--faint:#7b8496;--head:#1d4ed8;
 --btn:#e2e7ef;--btn-text:#172230;--idle:#b3bcca;--err:#b3261e;
 --accent:#2563eb;--accent-text:#ffffff;--accent-soft:#dbe6ff;
 --live:#0f9d58;--danger:#a12b2b;--warn:#8f4700;
 --shadow:0 1px 2px rgba(16,24,40,.06),0 1px 3px rgba(16,24,40,.10);
}
:root[data-theme="dark"]{
 --bg:#0e1116;--surface:#171c25;--raised:#1e2530;--field:#171c25;--border:#232a36;
 --text:#e8eaed;--muted:#7d8798;--faint:#69738a;--head:#9cc4ff;
 --btn:#2b3240;--btn-text:#ffffff;--idle:#3a4250;--err:#ff9b9b;
 --accent:#3b82f6;--accent-text:#ffffff;--accent-soft:#1e304f;
 --live:#31c48d;--danger:#c2410c;--warn:#b45309;
 --shadow:0 1px 2px rgba(0,0,0,.4);
}
@media (prefers-color-scheme:dark){
 :root:not([data-theme="light"]){
  --bg:#0e1116;--surface:#171c25;--raised:#1e2530;--field:#171c25;--border:#232a36;
  --text:#e8eaed;--muted:#7d8798;--faint:#69738a;--head:#9cc4ff;
  --btn:#2b3240;--btn-text:#ffffff;--idle:#3a4250;--err:#ff9b9b;
  --accent:#3b82f6;--accent-text:#ffffff;--accent-soft:#1e304f;
  --live:#31c48d;--danger:#c2410c;--warn:#b45309;
  --shadow:0 1px 2px rgba(0,0,0,.4);
 }
}
*{box-sizing:border-box;-webkit-tap-highlight-color:transparent}
html,body{margin:0}
body{
 font-family:system-ui,-apple-system,'Segoe UI',sans-serif;
 background:var(--bg);color:var(--text);
 /* The bottom bar floats over the page, so the last section needs room to
    clear it, plus whatever the phone's own home indicator takes. */
 padding:0 12px calc(96px + env(safe-area-inset-bottom));
}

/* ---- The part that never scrolls away ---- */
.pinned{
 position:sticky;top:0;z-index:20;
 margin:0 -12px;padding:calc(8px + env(safe-area-inset-top)) 12px 8px;
 background:var(--bg);border-bottom:1px solid var(--border);
}
.top{display:flex;align-items:center;gap:8px}
h1{font-size:.95rem;margin:0;color:var(--head);letter-spacing:.02em;font-weight:600}
.dot{width:9px;height:9px;border-radius:50%;background:var(--idle);flex:none;margin-left:2px}
.dot.on{background:var(--live)}
.dot.off{background:#f05252}
.iconbtn{
 flex:none;margin:0;padding:0;width:38px;height:38px;font-size:16px;line-height:1;
 border-radius:11px;background:var(--btn);color:var(--btn-text);border:0;
}
.iconbtn.mic{margin-left:auto}
.iconbtn.hot{background:var(--danger);color:#fff}
#now{
 margin-top:8px;padding:11px 12px;background:var(--surface);border:1px solid var(--border);
 border-radius:12px;min-height:2.6rem;font-size:14px;line-height:1.35;box-shadow:var(--shadow);
}

/* Controls for whatever is live: raised only when there is something to
   control, so the page is never taller than the moment needs. */
.ctx{display:flex;gap:6px;align-items:center;margin-top:8px}
.ctx button{flex:1;padding:11px 6px;font-size:14px;background:var(--raised);border:1px solid var(--border)}
.ctx button.on{background:var(--accent-soft);border-color:var(--accent);color:var(--text)}
.ctx .name{
 flex:2;min-width:0;text-align:center;font-size:11px;color:var(--faint);
 overflow:hidden;text-overflow:ellipsis;white-space:nowrap;
}

/* ---- Common controls ---- */
input,select{
 width:100%;padding:12px;font-size:16px;border-radius:11px;
 border:1px solid var(--border);background:var(--field);color:var(--text);
}
button{
 padding:13px 14px;font-size:15px;border:0;border-radius:11px;
 color:var(--btn-text);background:var(--btn);font-weight:500;
}
button:active{opacity:.6}
.go{background:var(--accent);color:var(--accent-text)}
.warn{background:var(--warn);color:#fff}
.dark{background:#111;color:#fff}
.stop{background:var(--danger);color:#fff}
.row{display:flex;gap:8px;margin-top:8px}
.row>*{flex:1}
.row.thin>*{flex:none;width:auto}
.nav button{font-size:17px;padding:16px 8px}

/* ---- Sections ---- */
.tab{padding-top:4px}
.sec{margin-top:14px;background:var(--surface);border:1px solid var(--border);border-radius:14px;padding:12px;box-shadow:var(--shadow)}
.lbl{font-size:11px;text-transform:uppercase;letter-spacing:.07em;color:var(--muted);margin-bottom:8px;font-weight:600}
.sub{margin-top:12px}
.hint{font-size:11px;color:var(--faint);margin:8px 0 0}
details.sec>summary{
 list-style:none;cursor:pointer;font-size:11px;text-transform:uppercase;
 letter-spacing:.07em;color:var(--muted);font-weight:600;display:flex;align-items:center;
}
details.sec>summary::-webkit-details-marker{display:none}
details.sec>summary::after{content:'⌄';margin-left:auto;font-size:15px;color:var(--faint)}
details.sec[open]>summary::after{content:'⌃'}
details.sec[open]>summary{margin-bottom:10px}

/* ---- Tappable lists (search hits, media, songs, slides) ---- */
.list button{
 display:block;width:100%;text-align:left;margin-top:6px;
 background:var(--raised);color:var(--text);font-weight:400;font-size:14px;line-height:1.35;
 border:1px solid var(--border);
}
.list b{color:var(--head);font-weight:600}
.list .tag{
 display:inline-block;margin-right:6px;padding:1px 6px;border-radius:5px;
 background:var(--accent-soft);color:var(--head);font-size:10px;font-weight:600;
 text-transform:uppercase;letter-spacing:.05em;
}
.list button.cur{border-color:var(--accent);background:var(--accent-soft)}
.empty{font-size:13px;color:var(--faint);padding:6px 2px}

/* ---- Browse grids ---- */
/* Book names need their own width; chapter and verse numbers are a keypad, and
   read fastest as an even grid of squares. */
.grid{display:flex;flex-wrap:wrap;gap:6px}
.grid button{
 flex:0 0 auto;padding:10px 12px;font-size:14px;font-weight:400;
 background:var(--raised);border:1px solid var(--border);color:var(--text);
}
.grid.num{display:grid;grid-template-columns:repeat(auto-fill,minmax(46px,1fr))}
.grid.num button{padding:12px 0;text-align:center;font-variant-numeric:tabular-nums}

/* ---- Song detail ---- */
.songhead{display:flex;align-items:center;gap:8px;margin-bottom:2px}
.songhead b{font-size:15px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.back{flex:none;padding:8px 13px;font-size:17px;line-height:1;background:var(--raised);border:1px solid var(--border)}

#err{color:var(--err);font-size:13px;margin:10px 2px 0;min-height:1em}

/* ---- Tab bar ---- */
.tabs{
 position:fixed;left:0;right:0;bottom:0;z-index:30;display:flex;
 padding:6px 6px calc(6px + env(safe-area-inset-bottom));
 background:var(--surface);border-top:1px solid var(--border);
}
.tabs button{
 flex:1;background:none;border:0;padding:7px 2px;border-radius:12px;
 color:var(--muted);font-size:10px;font-weight:600;letter-spacing:.03em;
 display:flex;flex-direction:column;align-items:center;gap:3px;
}
.tabs button .ico{font-size:17px;line-height:1}
.tabs button.on{background:var(--accent-soft);color:var(--head)}
"#;

const REMOTE_BODY: &str = r#"
<div class="pinned">
  <div class="top">
    <h1>Bible Remote</h1>
    <button id="mic" class="iconbtn mic" onclick="listen()" title="Start or stop listening" aria-label="Listening">&#127908;</button>
    <button id="tbtn" class="iconbtn" onclick="toggleTheme()" title="Light or dark">&#9790;</button>
    <span id="dot" class="dot"></span>
  </div>
  <div id="now">&#8230;</div>

  <div id="vid" class="ctx" hidden>
    <button id="vpause" onclick="vid('pause')">&#9208; Pause</button>
    <button onclick="vid('restart')">&#9198; Start</button>
    <button id="vmute" onclick="vid('mute')">Mute</button>
    <button id="vloop" onclick="vid('loop')">Loop</button>
  </div>

  <div id="deck" class="ctx" hidden>
    <button onclick="deck('prev')">&#9664; page</button>
    <span id="deckname" class="name"></span>
    <button onclick="deck('next')">page &#9654;</button>
  </div>

  <div id="aud" class="ctx" hidden>
    <button id="apause" onclick="aud('pause')">&#9208;</button>
    <button onclick="aud('restart')">&#9198;</button>
    <button onclick="aud('quieter')">&#8722;</button>
    <span id="avol" class="name"></span>
    <button onclick="aud('louder')">&#43;</button>
    <button id="aloop" onclick="aud('loop')">&#8635;</button>
    <button onclick="aud('stop')">&#9632;</button>
  </div>
</div>

<main>

<section id="tab-word" class="tab">
  <div class="sec">
    <div class="lbl">Move through the passage</div>
    <div class="row nav">
      <button onclick="nav('prev-verse')">&#9664; verse</button>
      <button onclick="nav('next-verse')">verse &#9654;</button>
    </div>
    <div class="row nav">
      <button onclick="nav('prev-chapter')">&#9664; chapter</button>
      <button onclick="nav('next-chapter')">chapter &#9654;</button>
    </div>
  </div>

  <div class="sec">
    <div class="lbl">Go to a reference</div>
    <input id="q" placeholder="e.g. John 3:16" autocapitalize="words" autocomplete="off">
    <div class="row"><button class="go" onclick="go()">Project</button></div>
    <div id="cmp" class="sub" hidden>
      <div class="lbl">Compare with</div>
      <div class="row">
        <select id="sec"></select>
        <button onclick="both()" title="Show it in both translations, side by side">Both</button>
      </div>
    </div>
  </div>

  <div class="sec">
    <div class="lbl">Browse</div>
    <div id="crumb" class="songhead" hidden>
      <button class="back" onclick="browseUp()" aria-label="Back">&#8249;</button>
      <b id="crumbtext"></b>
    </div>
    <div id="bookgrid" class="grid"></div>
    <div id="chapgrid" class="grid num" hidden></div>
    <div id="versegrid" class="grid num" hidden></div>
  </div>

  <div class="sec">
    <div class="lbl">Search by words</div>
    <input id="s" placeholder="e.g. lamp unto my feet" autocomplete="off">
    <div class="row"><button onclick="search()">Search</button></div>
    <div id="hits" class="list"></div>
  </div>
</section>

<section id="tab-songs" class="tab" hidden>
  <div id="songbrowse" class="sec">
    <div class="lbl">Songs</div>
    <input id="songq" placeholder="Find a song" autocomplete="off">
    <div id="songs" class="list"></div>
  </div>

  <div id="songopen" class="sec" hidden>
    <div class="songhead">
      <button class="back" onclick="closeSong()" aria-label="Back to the song list">&#8249;</button>
      <b id="songtitle"></b>
    </div>
    <div class="row nav">
      <button onclick="slideStep(-1)">&#9664; Prev</button>
      <button class="go" onclick="slideStep(1)">Next &#9654;</button>
    </div>
    <div id="slides" class="list"></div>
  </div>
</section>

<section id="tab-media" class="tab" hidden>
  <div class="sec">
    <div class="lbl">Announcements loop</div>
    <div class="row"><button id="ssbtn" onclick="slideshow()">&#9654; Start slideshow</button></div>
    <p class="hint">Walks the library on a timer. A video plays to its own end before the loop moves on.</p>
  </div>

  <div class="sec">
    <div class="lbl">Library</div>
    <div id="mlist" class="list"><p class="empty">Nothing in the library yet.</p></div>
  </div>
</section>

<section id="tab-screen" class="tab" hidden>
  <div class="sec">
    <div class="lbl">Screen</div>
    <div class="row">
      <button onclick="disp('blank')">Blank</button>
      <button class="dark" onclick="disp('blackout')">Blackout</button>
      <button onclick="disp('logo')">Logo</button>
    </div>
  </div>

  <div class="sec">
    <div class="lbl">Alert over the screen</div>
    <input id="alert" placeholder="e.g. Parent needed in the nursery" autocomplete="off">
    <div class="row">
      <button class="warn" onclick="sendAlert()">Show alert</button>
      <button onclick="clearAlert()">Clear</button>
    </div>
    <p class="hint">Sits over the live verse or song, and clears itself after a few seconds.</p>
  </div>

  <div class="sec">
    <div class="lbl">Announcement crawl</div>
    <input id="ticker" placeholder="e.g. Youth meets Thursday, 7pm" autocomplete="off">
    <div class="row">
      <button onclick="sendTicker()">Start crawl</button>
      <button onclick="stopTicker()">Stop</button>
    </div>
    <p class="hint">Runs along the foot of the screen under whatever is live, until you stop it.</p>
  </div>

  <div class="sec">
    <div class="lbl">Text size on the wall</div>
    <div class="row">
      <button onclick="size('down')">A&#8722;</button>
      <button onclick="size('reset')"><span id="scale">100%</span></button>
      <button onclick="size('up')">A&#43;</button>
    </div>
  </div>

  <details class="sec">
    <summary>Full-screen message</summary>
    <input id="msg" placeholder="e.g. Welcome to Grace Chapel" autocomplete="off">
    <div class="row">
      <button class="go" onclick="sendMessage()">Show</button>
      <button onclick="clearMessage()">Clear</button>
    </div>
  </details>

  <details class="sec">
    <summary>Countdown</summary>
    <div class="row thin">
      <input id="cdmin" inputmode="numeric" value="5" style="width:72px;text-align:center">
      <input id="cdlbl" placeholder="Starting soon" autocomplete="off" style="flex:1">
    </div>
    <div class="row"><button class="go" onclick="countdown()">Start countdown</button></div>
  </details>

  <details class="sec">
    <summary>Stage monitor</summary>
    <input id="note" placeholder="Private note to the stage" autocomplete="off">
    <div class="row">
      <button class="go" onclick="sendNote()">Send</button>
      <button onclick="clearNote()">Clear</button>
    </div>
    <div class="lbl sub">Timer</div>
    <div class="row thin">
      <input id="tmin" inputmode="numeric" value="20" style="width:72px;text-align:center">
      <button onclick="stageTimer('countdown')" style="flex:1">Count down</button>
    </div>
    <div class="row">
      <button onclick="stageTimer('countup')">Count up</button>
      <button onclick="stageTimer('off')">Off</button>
    </div>
    <p class="hint">Shows on the stage monitor only, never on the congregation screen.</p>
  </details>

  <details class="sec" id="transsec" hidden>
    <summary>Translation</summary>
    <div class="row">
      <select id="tsel"></select>
      <button onclick="useTranslation()">Use</button>
    </div>
    <p class="hint">Changes which translation new lookups project in.</p>
  </details>
</section>

</main>

<p id="err"></p>

<nav class="tabs">
  <button data-tab="word" class="on" onclick="showTab('word')"><span class="ico">&#128214;</span>Word</button>
  <button data-tab="songs" onclick="showTab('songs')"><span class="ico">&#9834;</span>Songs</button>
  <button data-tab="media" onclick="showTab('media')"><span class="ico">&#127916;</span>Media</button>
  <button data-tab="screen" onclick="showTab('screen')"><span class="ico">&#128421;</span>Screen</button>
</nav>
"#;

/// The page down to the opening `<script>`; the script itself follows.
pub fn head_and_body() -> String {
    [
        r#"<!doctype html><html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<meta name="apple-mobile-web-app-capable" content="yes">
<title>Bible Remote</title>
<style>"#,
        REMOTE_CSS,
        "</style></head><body>",
        REMOTE_BODY,
        "<script>",
    ]
    .concat()
}
