//! The phone remote's script.
//!
//! Two rules shape all of it. First, the laptop is the source of truth: every
//! button paints itself from the polled state rather than from what it thinks it
//! did, so a reload, or somebody acting at the desk, can never leave a control
//! here lying about the wall. Second, lists that change at the desk before a
//! service (songs, media, translations) are fetched once and filtered on the
//! phone; only the live state is polled.

/// Tab switching. The tab lives in a variable rather than on the phone, so
/// nothing is stored and a fresh browser opens on Word every time.
pub const TABS_JS: &str = r##"
function $(id){return document.getElementById(id)}
function err(m){$('err').textContent=m||''}
function show(el,on){el.hidden=!on}

function showTab(name){
 ['word','songs','media','screen'].forEach(function(t){
  show($('tab-'+t),t===name);
 });
 document.querySelectorAll('.tabs button').forEach(function(b){
  b.className=b.dataset.tab===name?'on':'';
 });
 // A tab change is a new view of the same service; a stale error from the last
 // one is just noise on it.
 err('');
 window.scrollTo(0,0);
}
"##;

/// Scripture: stepping, projecting, comparing and searching.
pub const WORD_JS: &str = r##"
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
async function project(ref){
 var r=await req('/api/project',{method:'POST',body:ref});
 err(r.ok?'':await r.text()); kick();
}
async function search(){
 var q=$('s').value.trim(); if(!q)return;
 $('hits').textContent='Searching…';
 var r=await req('/api/search',{method:'POST',body:q});
 if(!r.ok){$('hits').textContent='';err(await r.text());return}
 var list=await r.json();
 $('hits').textContent='';
 if(!list.length){$('hits').innerHTML='<p class="empty">No matches.</p>';return}
 list.forEach(function(v){
  var b=document.createElement('button');
  var t=document.createElement('b'); t.textContent=v.reference;
  b.appendChild(t); b.appendChild(document.createTextNode(' · '+v.text));
  b.onclick=function(){project(v.reference)};
  $('hits').appendChild(b);
 });
}
// Compare: an empty box means the verse already on screen, so Both works
// straight after stepping through a passage without retyping the reference.
async function both(){
 var code=$('sec').value; if(!code)return;
 var r=await req('/api/parallel',{method:'POST',body:code+'|'+$('q').value.trim()});
 err(r.ok?'':await r.text()); kick();
}
async function useTranslation(){
 var code=$('tsel').value; if(!code)return;
 var r=await req('/api/translation',{method:'POST',body:code});
 err(r.ok?'':await r.text()); kick();
}
// One request fills both pickers: the one to compare against, and the one that
// changes which translation new lookups project in.
async function loadTranslations(){
 var r=await req('/api/translations'); if(!r.ok)return;
 var d=await r.json();
 var all=d.list||[];
 var others=all.filter(function(t){return t.code!==d.active});
 var s=$('sec'); s.textContent='';
 others.forEach(function(t){
  var o=document.createElement('option'); o.value=t.code; o.textContent=t.code; s.appendChild(o);
 });
 show($('cmp'),others.length>0); // only one installed: nothing to compare against
 var sel=$('tsel'); sel.textContent='';
 all.forEach(function(t){
  var o=document.createElement('option'); o.value=t.code; o.textContent=t.code;
  if(t.code===d.active)o.selected=true;
  sel.appendChild(o);
 });
 show($('transsec'),all.length>1);
}
"##;

/// Browsing the Bible by tapping rather than typing. `1 Thessalonians 4:16` on a
/// phone keyboard, in a dark hall, one-handed, is the worst input the app ever
/// asks for; three taps are not.
///
/// The counts come from the app rather than a table baked in here, so a
/// translation that is missing a book never offers a chapter it does not have.
pub const BROWSE_JS: &str = r##"
var books=[],atBook=null,atChapter=0;

async function loadBooks(){
 var r=await req('/api/books'); if(!r.ok)return;
 books=await r.json();
 var box=$('bookgrid'); box.textContent='';
 books.forEach(function(b){
  var el=document.createElement('button');
  el.textContent=b.name;
  el.onclick=function(){openBook(b)};
  box.appendChild(el);
 });
}
// Which of the three grids is showing, and what the crumb above them says.
function browseLevel(level){
 show($('bookgrid'),level===0);
 show($('chapgrid'),level===1);
 show($('versegrid'),level===2);
 show($('crumb'),level>0);
 $('crumbtext').textContent=level===0?'':(level===1?atBook.name:atBook.name+' '+atChapter);
}
async function count(book,chapter){
 var q='/api/count?book='+encodeURIComponent(book)+(chapter?'&chapter='+chapter:'');
 var r=await req(q); if(!r.ok)return 0;
 var d=await r.json(); return d.count||0;
}
// A grid of 1..n, each tap running `pick`. Chapters and verses differ only in
// what tapping one does, so they share this.
function numGrid(box,n,pick){
 box.textContent='';
 if(!n){box.innerHTML='<p class="empty">Nothing here in this translation.</p>';return}
 for(var i=1;i<=n;i++){
  (function(k){
   var el=document.createElement('button');
   el.textContent=k;
   el.onclick=function(){pick(k)};
   box.appendChild(el);
  })(i);
 }
}
async function openBook(b){
 atBook=b; atChapter=0;
 numGrid($('chapgrid'),await count(b.osis,0),openChapter);
 browseLevel(1); err('');
}
async function openChapter(c){
 atChapter=c;
 numGrid($('versegrid'),await count(atBook.osis,c),projectVerse);
 browseLevel(2); err('');
}
// The book's display name, not its osis code, because this goes through the
// same reference parser the typed box uses.
function projectVerse(v){
 project(atBook.name+' '+atChapter+':'+v);
}
function browseUp(){
 if(atChapter){atChapter=0;browseLevel(1);return}
 atBook=null; browseLevel(0);
}
"##;

/// Songs. The list is a church's whole book, so it is fetched once and filtered
/// here; opening one swaps the browse card for the slides of that song.
pub const SONGS_JS: &str = r##"
var songs=[],slides=[],songId=0,slideAt=-1;

async function loadSongs(){
 var r=await req('/api/songs'); if(!r.ok)return;
 songs=await r.json();
 paintSongs();
}
function paintSongs(){
 var q=$('songq').value.trim().toLowerCase();
 var box=$('songs'); box.textContent='';
 var list=songs.filter(function(s){return !q||s.title.toLowerCase().indexOf(q)>=0});
 if(!list.length){
  var p=document.createElement('p'); p.className='empty';
  p.textContent=songs.length?'No song matches that.':'No songs yet.';
  box.appendChild(p); return;
 }
 // A long book on a phone is a scroll nobody finishes, so the list is capped
 // and the filter box is how you reach the rest.
 list.slice(0,40).forEach(function(s){
  var b=document.createElement('button');
  b.textContent=s.title;
  b.onclick=function(){openSong(s.id,s.title)};
  box.appendChild(b);
 });
 if(list.length>40){
  var more=document.createElement('p'); more.className='empty';
  more.textContent=(list.length-40)+' more — keep typing to narrow it down.';
  box.appendChild(more);
 }
}
async function openSong(id,title){
 var r=await req('/api/song?id='+id);
 if(!r.ok){err(await r.text());return}
 slides=await r.json(); songId=id; slideAt=-1;
 $('songtitle').textContent=title;
 show($('songbrowse'),false); show($('songopen'),true);
 paintSlides(); err(''); window.scrollTo(0,0);
}
function closeSong(){
 show($('songopen'),false); show($('songbrowse'),true);
 songId=0; slides=[]; slideAt=-1; err('');
}
function paintSlides(){
 var box=$('slides'); box.textContent='';
 if(!slides.length){box.innerHTML='<p class="empty">This song has no slides.</p>';return}
 slides.forEach(function(s,i){
  var b=document.createElement('button');
  if(i===slideAt)b.className='cur';
  if(s.label){
   var tag=document.createElement('span'); tag.className='tag'; tag.textContent=s.label;
   b.appendChild(tag);
  }
  b.appendChild(document.createTextNode(s.text));
  b.onclick=function(){projectSlide(i)};
  box.appendChild(b);
 });
}
async function projectSlide(i){
 if(i<0||i>=slides.length)return;
 var r=await req('/api/song',{method:'POST',body:songId+'|'+i});
 if(!r.ok){err(await r.text());return}
 slideAt=i; paintSlides(); err(''); kick();
}
// Next from "nothing live yet" opens the first slide rather than the second.
function slideStep(delta){
 if(slideAt<0){projectSlide(0);return}
 projectSlide(Math.min(slides.length-1,Math.max(0,slideAt+delta)));
}
"##;

/// Media: the library, the announcements loop, and the two sets of controls that
/// only appear while the thing they control is on the wall.
pub const MEDIA_JS: &str = r##"
// The library is loaded once: it changes at the desk before a service, not from
// here.
async function loadMedia(){
 var r=await req('/api/media'); if(!r.ok)return;
 var list=await r.json();
 var box=$('mlist'); box.textContent='';
 if(!list.length){box.innerHTML='<p class="empty">Nothing in the library yet.</p>';return}
 list.forEach(function(m){
  var b=document.createElement('button');
  var t=document.createElement('span'); t.className='tag';
  t.textContent=m.kind==='video'?'Video':(m.kind==='audio'?'Audio':'Image');
  b.appendChild(t); b.appendChild(document.createTextNode(m.title));
  // Tapping a sound file plays it; the app decides that from the file's kind,
  // so this stays one action either way.
  b.onclick=function(){playMedia(m.id)};
  box.appendChild(b);
 });
}
async function playMedia(id){
 var r=await req('/api/media',{method:'POST',body:String(id)});
 err(r.ok?'':await r.text()); kick();
}
var sliding=false;
async function slideshow(){
 var r=await req('/api/slideshow',{method:'POST',body:sliding?'stop':'start'});
 var t=await r.text();
 if(!r.ok){err(t);return}
 setSlideshow(t==='running'); err('');
}
function setSlideshow(on){
 sliding=on;
 var b=$('ssbtn');
 b.textContent=on?'■ Stop slideshow':'▶ Start slideshow';
 b.className=on?'stop':'';
}
// The app works out which item is live and which deck it belongs to, so these
// only ever send a direction.
async function deck(dir){
 var r=await req('/api/deck',{method:'POST',body:dir});
 err(r.ok?'':await r.text()); kick();
}
async function vid(what){
 var r=await req('/api/video',{method:'POST',body:what});
 err(r.ok?'':await r.text()); kick();
}
// Sound runs beside the screen, so its controls are reachable whatever is up.
async function aud(what){
 var r=await req('/api/audio',{method:'POST',body:what});
 err(r.ok?'':await r.text()); kick();
}
"##;

/// The screen itself: blanking, announcements, alerts, size, and the stage
/// monitor.
pub const SCREEN_JS: &str = r##"
async function disp(kind){
 var r=await req('/api/display',{method:'POST',body:kind});
 err(r.ok?'':await r.text()); kick();
}
async function sendAlert(){
 var t=$('alert').value.trim(); if(!t)return;
 var r=await req('/api/alert',{method:'POST',body:t});
 err(r.ok?'':await r.text());
}
async function clearAlert(){
 await req('/api/alert',{method:'POST',body:''}
async function sendTicker(){
 var t=document.getElementById('ticker').value.trim();
 if(!t) return;
 var r=await req('/api/ticker',{method:'POST',body:t});
 toast(r.ok?'Crawling':'Failed');
}
async function stopTicker(){
 await req('/api/ticker',{method:'POST',body:''});
 document.getElementById('ticker').value='';
 toast('Stopped');
});
 $('alert').value=''; err('');
}
async function sendMessage(){
 var t=$('msg').value.trim(); if(!t)return;
 var r=await req('/api/message',{method:'POST',body:t});
 err(r.ok?'':await r.text()); kick();
}
async function clearMessage(){
 await req('/api/message',{method:'POST',body:''});
 $('msg').value=''; err(''); kick();
}
async function countdown(){
 var m=$('cdmin').value.trim();
 var r=await req('/api/countdown',{method:'POST',body:m+'|'+$('cdlbl').value.trim()});
 err(r.ok?'':await r.text()); kick();
}
async function sendNote(){
 var t=$('note').value.trim(); if(!t)return;
 var r=await req('/api/stage-note',{method:'POST',body:t});
 err(r.ok?'':await r.text());
}
async function clearNote(){
 await req('/api/stage-note',{method:'POST',body:''});
 $('note').value=''; err('');
}
async function stageTimer(mode){
 var secs=mode==='countdown'?Math.max(1,parseInt($('tmin').value,10)||0)*60:0;
 var r=await req('/api/stage-timer',{method:'POST',body:mode+'|'+secs});
 err(r.ok?'':await r.text());
}
async function size(step){
 var r=await req('/api/fontscale',{method:'POST',body:step});
 var t=await r.text();
 if(!r.ok){err(t);return}
 setScale(parseFloat(t)); err('');
}
function setScale(n){
 if(!n||isNaN(n))return;
 $('scale').textContent=Math.round(n*100)+'%';
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
 var b=$('mic');
 b.className=on?'iconbtn mic hot':'iconbtn mic';
 b.setAttribute('aria-pressed',on?'true':'false');
 b.title=on?'Stop listening':'Start listening';
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
"##;

/// The poll. Everything the laptop knows better than this page is repainted
/// from here, including which contextual controls belong on screen at all.
pub const POLL_JS: &str = r##"
async function refresh(){
 var r=await req('/api/state');
 if(!r.ok)throw new Error('state');
 var s=await r.json();
 $('now').textContent=s.summary;
 if(s.listening!==listening)setListening(s.listening);
 if(s.slideshow!==sliding)setSlideshow(s.slideshow);
 setScale(s.fontScale);
 paintContext(s);
 $('dot').className='dot on';
}
// Transport belongs on the page only while a video is on the wall, and page
// buttons only while a page of a multi-page document is. Anything else and the
// pinned area shrinks back to the summary alone.
function paintContext(s){
 var v=s.video;
 show($('vid'),!!v);
 if(v){
  $('vpause').textContent=v.paused?'▶ Play':'⏸ Pause';
  $('vmute').className=v.muted?'on':'';
  $('vmute').textContent=v.muted?'Muted':'Mute';
  $('vloop').className=v.looping?'on':'';
 }
 var d=s.deck||'';
 show($('deck'),d!=='');
 if(d)$('deckname').textContent=d;
 // Sound is not tied to what is on the wall, so unlike the two above this
 // appears whenever a track is loaded, verse or blackout or anything else.
 var a=s.audio;
 show($('aud'),!!a);
 if(a){
  $('apause').textContent=a.paused?'▶':'⏸';
  $('aloop').className=a.looping?'on':'';
  $('avol').textContent=Math.round((a.volume||0)*100)+'%';
 }
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
$('msg').addEventListener('keydown',function(e){if(e.key==='Enter')sendMessage()});
$('note').addEventListener('keydown',function(e){if(e.key==='Enter')sendNote()});
$('songq').addEventListener('input',paintSongs);
paintTheme();
// Older iOS only has the deprecated addListener, and an exception here would
// take the rest of the page's setup down with it.
var mq=matchMedia('(prefers-color-scheme: dark)');
if(mq.addEventListener)mq.addEventListener('change',paintTheme);
else if(mq.addListener)mq.addListener(paintTheme);
loadTranslations().catch(function(){});
loadMedia().catch(function(){});
loadSongs().catch(function(){});
loadBooks().catch(function(){});
start();
</script></body></html>"##;
