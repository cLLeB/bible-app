//! The read-only projection mirror, used for a phone in the crèche or an OBS
//! browser source. It polls the live projection and never sends anything back.

pub const PROJECTION_HTML: &str = r#"<!doctype html><html><head><meta charset="utf-8">
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
pub const PROJECTION_THEME_JS: &str = r#"
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

pub const PROJECTION_JS: &str = r#"
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
