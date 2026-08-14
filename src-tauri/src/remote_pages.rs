//! The two pages the LAN server serves: the operator's phone remote, and the
//! read-only projection mirror used for OBS / browser output.
//!
//! Both poll the app for state. They do it with a *sequential* loop: each cycle
//! waits for the previous response before scheduling the next, and every request
//! carries an abort timeout. A naive `setInterval` fetch pile-up is what makes a
//! phone browser exhaust its per-host connection pool and leave a second tab
//! hanging forever, so the pattern here matters more than it looks.
//!
//! The pages are assembled by concatenation rather than `format!`. Both are
//! mostly CSS and JavaScript, and `format!` would demand every brace in them be
//! doubled, an easy way to ship subtly broken script that only misbehaves on a
//! phone. Plain `concat` keeps the source readable and removes the hazard.

/// Polling and timeout helpers. Shared: a stuck request must never hang a page
/// forever, whichever page it is.
const LOOP_JS: &str = r#"
async function timedFetch(path,opts){
 opts=opts||{};
 const c=new AbortController();
 const t=setTimeout(function(){c.abort()},6000);
 try{return await fetch(path,Object.assign({},opts,{signal:c.signal,cache:'no-store'}));}
 finally{clearTimeout(t);}
}
async function loop(fn,ms){for(;;){try{await fn()}catch(e){}await new Promise(function(r){setTimeout(r,ms)});}}
"#;

/// The control page's request helper. There is no pairing step: opening the
/// address is the whole of it, so this just forwards to the shared fetch. Kept as
/// its own function because every handler calls `req`.
const REQ_JS: &str = r#"
async function req(path,opts){
 return await timedFetch(path,opts||{});
}
"#;

const REMOTE_HTML: &str = r#"<!doctype html><html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<meta name="apple-mobile-web-app-capable" content="yes">
<title>Bible Remote</title>
<style>
/* Light is the same mild brightness as the operator console: an off-white that
   reads on a phone in a bright hall without glaring in a dark one. The page
   follows the phone's own appearance setting, and the ☀/☾ button overrides it
   for the visit. Nothing is stored, so a fresh browser still works on first
   load. */
:root{
 color-scheme:light dark;
 --bg:#dfe3ea;--surface:#ffffff;--field:#ffffff;--border:#c4ccd8;
 --text:#172230;--muted:#566072;--head:#1d4ed8;
 --btn:#e2e7ef;--btn-text:#172230;--idle:#b3bcca;--err:#b3261e;
}
:root[data-theme="dark"]{
 --bg:#0e1116;--surface:#171c25;--field:#171c25;--border:#232a36;
 --text:#e8eaed;--muted:#7d8798;--head:#9cc4ff;
 --btn:#2b3240;--btn-text:#ffffff;--idle:#3a4250;--err:#ff9b9b;
}
@media (prefers-color-scheme:dark){
 :root:not([data-theme="light"]){
  --bg:#0e1116;--surface:#171c25;--field:#171c25;--border:#232a36;
  --text:#e8eaed;--muted:#7d8798;--head:#9cc4ff;
  --btn:#2b3240;--btn-text:#ffffff;--idle:#3a4250;--err:#ff9b9b;
 }
}
*{box-sizing:border-box}
body{font-family:system-ui,-apple-system,sans-serif;margin:0;padding:12px 12px 32px;background:var(--bg);color:var(--text)}
h1{font-size:1rem;margin:0;color:var(--head);letter-spacing:.02em}
.top{display:flex;align-items:center;gap:8px;margin-bottom:10px}
.dot{width:9px;height:9px;border-radius:50%;background:var(--idle);flex:none}
.dot.on{background:#31c48d}
.dot.off{background:#f05252}
input,select{width:100%;padding:12px;font-size:16px;border-radius:10px;border:1px solid var(--border);background:var(--field);color:var(--text)}
button{padding:13px 14px;font-size:15px;border:0;border-radius:10px;color:var(--btn-text);background:var(--btn);font-weight:500}
button:active{opacity:.65}
.tbtn{margin-left:auto;flex:none;padding:8px 11px;font-size:14px;line-height:1}
.row{display:flex;gap:8px;margin-top:8px}
.row>*{flex:1}
.go{background:#2563eb;color:#fff}.warn{background:#8f4700;color:#fff}
.dark{background:#111;color:#fff}.stop{background:#a12b2b;color:#fff}
.nav button{font-size:17px;padding:16px 8px}
#now{margin:10px 0;padding:11px 12px;background:var(--surface);border-radius:10px;min-height:2.6rem;font-size:14px;line-height:1.35;border:1px solid var(--border)}
#err{color:var(--err);font-size:13px;margin:8px 0 0;min-height:1em}
.sec{margin-top:16px;border-top:1px solid var(--border);padding-top:12px}
.lbl{font-size:11px;text-transform:uppercase;letter-spacing:.07em;color:var(--muted);margin-bottom:6px}
.sub{margin-top:12px}
.hits button{display:block;width:100%;text-align:left;margin-top:6px;background:var(--surface);color:var(--text);font-weight:400;font-size:14px;line-height:1.35}
.hits b{color:var(--head);font-weight:600}
</style></head><body>

<div id="app">
  <div class="top">
    <h1>Bible Remote</h1>
    <button id="tbtn" class="tbtn" onclick="toggleTheme()" title="Light or dark">☾</button>
    <span id="dot" class="dot"></span>
  </div>
  <div id="now">…</div>

  <div class="lbl">Move through the passage</div>
  <div class="row nav">
    <button onclick="nav('prev-verse')">◀ verse</button>
    <button onclick="nav('next-verse')">verse ▶</button>
  </div>
  <div class="row nav">
    <button onclick="nav('prev-chapter')">◀ chapter</button>
    <button onclick="nav('next-chapter')">chapter ▶</button>
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
    <div class="lbl">Search by words</div>
    <input id="s" placeholder="e.g. lamp unto my feet" autocomplete="off">
    <div class="row"><button onclick="search()">Search</button></div>
    <div id="hits" class="hits"></div>
  </div>

  <div class="sec">
    <div class="lbl">Screen</div>
    <div class="row">
      <button onclick="disp('blank')">Blank</button>
      <button class="dark" onclick="disp('blackout')">Blackout</button>
      <button onclick="disp('logo')">Logo</button>
    </div>
  </div>

  <div class="sec">
    <div class="lbl">Listening</div>
    <div class="row"><button id="lisbtn" class="go" onclick="listen()">● Start listening</button></div>
  </div>

  <div class="sec">
    <div class="lbl">Alert over the screen</div>
    <input id="alert" placeholder="e.g. Parent needed in the nursery" autocomplete="off">
    <div class="row">
      <button class="warn" onclick="sendAlert()">Show alert</button>
      <button onclick="clearAlert()">Clear</button>
    </div>
  </div>

  <p id="err"></p>
</div>

<script>
"#;

const REMOTE_JS: &str = r#"
function $(id){return document.getElementById(id)}
function err(m){$('err').textContent=m||''}

async function nav(d){
 var r=await req('/api/nav',{method:'POST',body:d});
 err(r.ok?'':(r.status===409?'Nothing is on screen yet. Project a verse first.':await r.text()));
 kick();
}
async function go(){
 var q=$('q').value.trim(); if(!q)return;
 var r=await req('/api/project',{method:'POST',body:q});
 err(r.ok?'':await r.text()); kick();
}
async function disp(kind){
 var r=await req('/api/display',{method:'POST',body:kind});
 err(r.ok?'':await r.text()); kick();
}
async function search(){
 var q=$('s').value.trim(); if(!q)return;
 $('hits').textContent='Searching…';
 var r=await req('/api/search',{method:'POST',body:q});
 if(!r.ok){$('hits').textContent='';err(await r.text());return}
 var list=await r.json();
 $('hits').textContent='';
 if(!list.length){$('hits').textContent='No matches.';return}
 list.forEach(function(v){
  var b=document.createElement('button');
  var t=document.createElement('b'); t.textContent=v.reference;
  b.appendChild(t); b.appendChild(document.createTextNode(' · '+v.text));
  b.onclick=function(){project(v.reference)};
  $('hits').appendChild(b);
 });
}
async function project(ref){
 var r=await req('/api/project',{method:'POST',body:ref});
 err(r.ok?'':await r.text()); kick();
}
// Compare: an empty box means the verse already on screen, so Both works
// straight after stepping through a passage without retyping the reference.
async function both(){
 var code=$('sec').value; if(!code)return;
 var r=await req('/api/parallel',{method:'POST',body:code+'|'+$('q').value.trim()});
 err(r.ok?'':await r.text()); kick();
}
async function loadTranslations(){
 var r=await req('/api/translations'); if(!r.ok)return;
 var d=await r.json();
 var others=(d.list||[]).filter(function(t){return t.code!==d.active});
 if(!others.length)return; // only one installed: nothing to compare against
 var s=$('sec'); s.textContent='';
 others.forEach(function(t){
  var o=document.createElement('option'); o.value=t.code; o.textContent=t.code; s.appendChild(o);
 });
 $('cmp').hidden=false;
}
function themeIsDark(){
 var t=document.documentElement.getAttribute('data-theme');
 return t?t==='dark':matchMedia('(prefers-color-scheme: dark)').matches;
}
function paintTheme(){$('tbtn').textContent=themeIsDark()?'☀':'☾'}
function toggleTheme(){
 document.documentElement.setAttribute('data-theme',themeIsDark()?'light':'dark');
 paintTheme();
}
var listening=false;
async function listen(){
 var r=await req('/api/listen',{method:'POST',body:listening?'stop':'start'});
 var t=await r.text();
 if(!r.ok){err(t);return}
 setListening(t==='listening'); err('');
}
function setListening(on){
 listening=on;
 var b=$('lisbtn');
 b.textContent=on?'■ Stop listening':'● Start listening';
 b.className=on?'stop':'go';
}
async function sendAlert(){
 var t=$('alert').value.trim(); if(!t)return;
 var r=await req('/api/alert',{method:'POST',body:t});
 err(r.ok?'':await r.text());
}
async function clearAlert(){
 await req('/api/alert',{method:'POST',body:''});
 $('alert').value=''; err('');
}

async function refresh(){
 var r=await req('/api/state');
 if(!r.ok)throw new Error('state');
 var s=await r.json();
 $('now').textContent=s.summary;
 // The laptop is the source of truth for whether it is listening, so a reload,
 // or someone starting it at the laptop, never leaves this button lying.
 if(s.listening!==listening)setListening(s.listening);
 $('dot').className='dot on';
}
// Nudge the next poll after an action instead of adding a second timer.
var pending=false;
function kick(){if(!pending){pending=true;refresh().catch(function(){}).finally(function(){pending=false})}}
function start(){
 loop(async function(){
  try{await refresh()}catch(e){$('dot').className='dot off';throw e}
 },1500);
}
$('q').addEventListener('keydown',function(e){if(e.key==='Enter')go()});
$('s').addEventListener('keydown',function(e){if(e.key==='Enter')search()});
$('alert').addEventListener('keydown',function(e){if(e.key==='Enter')sendAlert()});
paintTheme();
// Older iOS only has the deprecated addListener, and an exception here would
// take the rest of the page's setup down with it.
var mq=matchMedia('(prefers-color-scheme: dark)');
if(mq.addEventListener)mq.addEventListener('change',paintTheme);
else if(mq.addListener)mq.addListener(paintTheme);
loadTranslations().catch(function(){});
start();
</script></body></html>"#;

const PROJECTION_HTML: &str = r#"<!doctype html><html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>Projection</title>
<style>html,body{margin:0;height:100%;background:#000;color:#fff;font-family:system-ui,sans-serif;overflow:hidden}
#wrap{height:100vh;display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;padding:0 6vw;box-sizing:border-box}
#body{font-size:5vw;line-height:1.15;white-space:pre-line;max-width:90vw}
#cap{font-size:2.2vw;color:#bbb;margin-top:4vh}</style></head>
<body><div id="wrap"><div id="body"></div><div id="cap"></div></div>
<script>
"#;

/// The mirror's half of the theme, kept in step with `src/lib/theme.ts`: the
/// same background, colours, weight and auto-fit rules the congregation screen
/// uses. Without this a sepia service still mirrors as white on black, which is
/// wrong on a phone and wrong again in an OBS scene.
const PROJECTION_THEME_JS: &str = r#"
var theme=null,scale=1;
function bgCss(b){
 if(b.kind==='gradient')return 'linear-gradient('+b.angle+'deg,'+b.color+','+b.color2+')';
 // Image and video backgrounds are files on the laptop that a phone on the LAN
 // cannot read. The theme's base colour is what sits behind them anyway.
 return b.color;
}
function applyTheme(){
 if(!theme)return;
 var t=theme.text,b=document.getElementById('body'),c=document.getElementById('cap');
 // Blackout means dark, whatever the theme says.
 var dark=cur&&cur.kind==='blackout';
 var paint=dark?'#000':bgCss(theme.background);
 document.documentElement.style.background=paint;
 document.body.style.background=paint;
 var shadow=t.shadow?'0 2px 8px rgba(0,0,0,0.55)':'none';
 b.style.color=t.color;b.style.fontFamily=t.fontFamily;b.style.fontWeight=t.weight;
 b.style.textTransform=t.uppercase?'uppercase':'none';b.style.textShadow=shadow;
 c.style.color=t.captionColor;c.style.fontFamily=t.fontFamily;c.style.textShadow=shadow;
 c.style.fontSize=(2.2*scale)+'vw';
 document.getElementById('wrap').style.textAlign=t.align;
}
loop(async function(){
 var r=await timedFetch('/api/appearance');
 var s=await r.json();
 if(s&&s.theme){theme=s.theme;scale=s.fontScale||1;applyTheme();render();}
},2000);
"#;

const PROJECTION_JS: &str = r#"
function fmt(ms){var t=Math.max(0,Math.floor(ms/1000));var m=Math.floor(t/60),s=t%60;return m+':'+String(s).padStart(2,'0');}
var cur=null;
function fitvw(n){return (n<120?5:n<220?4:n<340?3.2:n<500?2.6:2.1)*scale;}
function render(){
 var b=document.getElementById('body'),c=document.getElementById('cap');
 // Blackout is a background change, so the theme is repainted on every frame.
 applyTheme();
 if(!cur){b.textContent='';c.textContent='';return;}
 b.style.fontSize=(5*scale)+'vw';
 switch(cur.kind){
  case 'verse': case 'song':
   b.textContent=cur.text;b.style.fontSize=fitvw(cur.text.length)+'vw';c.textContent=cur.caption;break;
  case 'parallel':
   b.textContent=cur.primaryText+'\n\n'+cur.secondaryText;
   b.style.fontSize=fitvw((cur.primaryText+cur.secondaryText).length)+'vw';
   c.textContent=cur.caption+' ('+cur.primaryCode+' / '+cur.secondaryCode+')';break;
  case 'message':
   b.textContent=cur.text;b.style.fontSize=fitvw(cur.text.length)+'vw';c.textContent='';break;
  case 'countdown':
   b.textContent=fmt(cur.targetMs-Date.now());c.textContent=cur.label||'';break;
  case 'logo': b.textContent='✝';c.textContent='';break;
  default: b.textContent='';c.textContent='';
 }
}
// Read-only mirror: it polls the live projection and never sends anything back.
loop(async function(){
 var r=await timedFetch('/api/projection');
 cur=await r.json();
 render();
},400);
setInterval(render,250);
</script></body></html>"#;

pub fn remote_page() -> String {
    [REMOTE_HTML, LOOP_JS, REQ_JS, REMOTE_JS].concat()
}

pub fn projection_page() -> String {
    [PROJECTION_HTML, LOOP_JS, PROJECTION_THEME_JS, PROJECTION_JS].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_are_whole_documents() {
        for page in [remote_page(), projection_page()] {
            assert!(page.starts_with("<!doctype html>"));
            assert!(page.trim_end().ends_with("</html>"));
            assert_eq!(
                page.matches("<script>").count(),
                page.matches("</script>").count(),
                "unbalanced script tags"
            );
        }
    }

    #[test]
    fn both_pages_poll_sequentially_and_time_out() {
        // Without these a phone piles up overlapping requests and can exhaust its
        // per-host connection pool, which strands any other tab on the same host.
        for page in [remote_page(), projection_page()] {
            assert!(page.contains("async function loop("), "poll loop missing");
            assert!(page.contains("AbortController"), "request timeout missing");
        }
    }

    #[test]
    fn neither_page_asks_the_phone_to_prove_anything() {
        // Opening the address is the whole of it. Nothing is stored on the phone
        // and no header is demanded, so a fresh browser works on first load.
        for page in [remote_page(), projection_page()] {
            assert!(!page.contains("X-Remote-Key"));
            assert!(!page.contains("remote-key"));
            assert!(!page.contains("localStorage"));
            assert!(!page.contains("Pairing code"));
        }
    }

    #[test]
    fn every_control_the_remote_offers_has_a_handler() {
        let page = remote_page();
        for handler in [
            "function nav(",
            "function go(",
            "function disp(",
            "function search(",
            "function listen(",
            "function sendAlert(",
            "function clearAlert(",
            "function both(",
            "function toggleTheme(",
        ] {
            assert!(page.contains(handler), "{handler} is wired to a button but not defined");
        }
    }

    #[test]
    fn the_remote_can_compare_translations() {
        // The console offers "Compare with … / Both" on a looked-up verse; the
        // operator holding the phone needs the same, not a walk back to the desk.
        let page = remote_page();
        assert!(page.contains("Compare with"), "the compare control is missing");
        assert!(page.contains("/api/translations"), "the picker is never filled");
        assert!(page.contains("/api/parallel"), "Both never reaches the app");
    }

    #[test]
    fn the_remote_reads_in_both_light_and_dark() {
        let page = remote_page();
        // Following the phone's own setting is the default; the button only
        // overrides it, and nothing is written to the phone to remember that.
        assert!(page.contains("prefers-color-scheme:dark"), "no light/dark following");
        assert!(page.contains("data-theme"), "the override has nothing to set");
        assert!(!page.contains("color-scheme:dark}"), "the page is still pinned to dark");
    }

    #[test]
    fn the_mirror_wears_the_operators_theme() {
        // A sepia service mirrored as white-on-black is the wrong screen, both on
        // a phone and in an OBS scene.
        let page = projection_page();
        assert!(page.contains("/api/appearance"), "the mirror never asks for the theme");
        assert!(page.contains("function applyTheme("), "the theme is fetched but not applied");
        assert!(page.contains("captionColor"), "the caption keeps a hardcoded colour");
        assert!(page.contains("linear-gradient("), "gradient themes are not mirrored");
    }

    #[test]
    fn the_remote_starts_polling_without_a_gate_in_front_of_it() {
        let page = remote_page();
        assert!(page.contains("\nstart();"), "the page must start polling on load");
        assert!(!page.contains("function boot("), "the pairing gate is gone");
    }

    #[test]
    fn the_remote_uses_helpers_it_actually_defines() {
        let page = remote_page();
        assert!(page.contains("async function req("), "req() is called but never defined");
        assert!(page.contains("async function timedFetch("), "timedFetch() missing");
    }
}
