use crate::commands::{do_lookup, project_via_handle, AppState};
use crate::events::ProjectionState;
use std::net::{UdpSocket, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub const REMOTE_PORT: u16 = 8787;

const PAGE: &str = r#"<!doctype html><html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Bible Remote</title>
<style>
body{font-family:system-ui,sans-serif;margin:0;padding:16px;background:#111;color:#eee}
h1{font-size:1.1rem;color:#9cf}
input{width:100%;box-sizing:border-box;padding:12px;font-size:1.1rem;border-radius:8px;border:1px solid #444;background:#222;color:#fff}
button{padding:12px 16px;font-size:1rem;border:0;border-radius:8px;color:#fff;margin-top:8px}
.row{display:flex;gap:8px}.row button{flex:1}
.go{background:#2563eb}.blank{background:#555}
#now{margin-top:16px;padding:12px;background:#222;border-radius:8px;min-height:2rem}
#err{color:#f88}
</style></head><body>
<h1>Bible Remote</h1>
<input id="q" placeholder="e.g. John 3:16" autofocus>
<div class="row"><button class="go" onclick="go()">Project</button>
<button class="blank" onclick="blank()">Blank</button></div>
<div class="row"><button id="lisbtn" class="go" onclick="listen()">● Start listening</button></div>
<p id="err"></p>
<div id="now">…</div>
<script>
async function go(){let q=document.getElementById('q').value.trim();if(!q)return;
 let r=await fetch('/api/project',{method:'POST',body:q});let t=await r.text();
 document.getElementById('err').textContent=r.ok?'':t;refresh();}
async function blank(){await fetch('/api/blank',{method:'POST'});refresh();}
let listening=false;
async function listen(){
 let r=await fetch('/api/listen',{method:'POST',body:listening?'stop':'start'});
 let t=await r.text();
 if(!r.ok){document.getElementById('err').textContent=t;return;}
 listening=(t==='listening');
 let b=document.getElementById('lisbtn');
 b.textContent=listening?'■ Stop listening':'● Start listening';
 b.className=listening?'blank':'go';
 document.getElementById('err').textContent='';}
async function refresh(){try{let r=await fetch('/api/state');document.getElementById('now').textContent=await r.text();}catch(e){}}
document.getElementById('q').addEventListener('keydown',e=>{if(e.key==='Enter')go()});
setInterval(refresh,2000);refresh();
</script></body></html>"#;

/// Best-effort LAN IP by opening a UDP socket to a public address (no data sent).
fn lan_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr: SocketAddr = socket.local_addr().ok()?;
    Some(addr.ip().to_string())
}

const PROJECTION_PAGE: &str = r#"<!doctype html><html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>Projection</title>
<style>html,body{margin:0;height:100%;background:#000;color:#fff;font-family:system-ui,sans-serif;overflow:hidden}
#wrap{height:100vh;display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;padding:0 6vw;box-sizing:border-box}
#body{font-size:5vw;line-height:1.15;white-space:pre-line;max-width:90vw}
#cap{font-size:2.2vw;color:#bbb;margin-top:4vh}</style></head>
<body><div id="wrap"><div id="body"></div><div id="cap"></div></div>
<script>
function fmt(ms){let t=Math.max(0,Math.floor(ms/1000));let m=Math.floor(t/60),s=t%60;return m+':'+String(s).padStart(2,'0');}
let cur=null;
async function poll(){try{let r=await fetch('/api/projection');cur=await r.json();}catch(e){}render();}
function fitvw(n){return n<120?5:n<220?4:n<340?3.2:n<500?2.6:2.1;}
function render(){let b=document.getElementById('body'),c=document.getElementById('cap');if(!cur){b.textContent='';c.textContent='';return;}
 b.style.fontSize='5vw';
 switch(cur.kind){case 'verse':case 'song':b.textContent=cur.text;b.style.fontSize=fitvw(cur.text.length)+'vw';c.textContent=cur.caption;break;
 case 'message':b.textContent=cur.text;b.style.fontSize=fitvw(cur.text.length)+'vw';c.textContent='';break;
 case 'countdown':b.textContent=fmt(cur.targetMs-Date.now());c.textContent=cur.label||'';break;
 case 'logo':b.textContent='✝';c.textContent='';break;
 default:b.textContent='';c.textContent='';}}
setInterval(poll,400);setInterval(render,250);poll();
</script></body></html>"#;

fn projection_json(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let guard = state.current.lock();
    
    match guard {
        Ok(ref g) => serde_json::to_string(&**g).unwrap_or_else(|_| "{\"kind\":\"blank\"}".into()),
        Err(_) => "{\"kind\":\"blank\"}".into(),
    }
}

fn state_summary(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let guard = state.current.lock();
    let summary = match guard {
        Ok(ref g) => match &**g {
            ProjectionState::Verse { caption, .. } | ProjectionState::Song { caption, .. } => {
                format!("On screen: {caption}")
            }
            ProjectionState::Message { text } => format!("Message: {text}"),
            ProjectionState::Countdown { label, .. } => format!("Countdown: {label}"),
            ProjectionState::Logo => "Logo".into(),
            ProjectionState::Blackout => "Blackout".into(),
            ProjectionState::Blank => "Nothing (blank)".into(),
        },
        Err(_) => "…".into(),
    };
    summary
}

/// Start the LAN remote HTTP server on a background thread (idempotent).
/// Returns the URL to open on a phone.
pub fn start(app: AppHandle, running: Arc<AtomicBool>) -> Result<String, String> {
    let ip = lan_ip().unwrap_or_else(|| "127.0.0.1".into());
    let url = format!("http://{ip}:{REMOTE_PORT}");
    if running.swap(true, Ordering::SeqCst) {
        return Ok(url); // already running
    }

    let server = tiny_http::Server::http(("0.0.0.0", REMOTE_PORT))
        .map_err(|e| format!("could not start remote server: {e}"))?;

    std::thread::spawn(move || {
        for mut req in server.incoming_requests() {
            let url_path = req.url().split('?').next().unwrap_or("/").to_string();
            let method = req.method().to_string();
            let (code, body) = match (method.as_str(), url_path.as_str()) {
                ("GET", "/") => (200, PAGE.to_string()),
                ("GET", "/projection") => (200, PROJECTION_PAGE.to_string()),
                ("GET", "/api/projection") => (200, projection_json(&app)),
                ("GET", "/api/state") => (200, state_summary(&app)),
                // The operator is at the projector when the preacher steps up, not at
                // the laptop. Listening starts and stops from the phone too.
                ("POST", "/api/listen") => {
                    let mut body = String::new();
                    let _ = req.as_reader().read_to_string(&mut body);
                    let on = body.trim() == "start";
                    let result = if on {
                        crate::commands::begin_listening(&app, None)
                    } else {
                        app.state::<crate::commands::AppState>()
                            .listening
                            .store(false, std::sync::atomic::Ordering::SeqCst);
                        Ok(())
                    };
                    match result {
                        Ok(()) => (200, if on { "listening" } else { "stopped" }.to_string()),
                        Err(e) => (500, e),
                    }
                }
                ("POST", "/api/blank") => {
                    let _ = project_via_handle(&app, ProjectionState::Blank);
                    (200, "ok".into())
                }
                ("POST", "/api/project") => {
                    let mut q = String::new();
                    let _ = req.as_reader().read_to_string(&mut q);
                    let state = app.state::<AppState>();
                    match do_lookup(&state, q.trim()) {
                        Ok(v) => {
                            let next = ProjectionState::Verse {
                                text: v.text,
                                caption: format!("{} · {}", v.reference, v.translation),
                            };
                            let _ = project_via_handle(&app, next);
                            (200, "ok".into())
                        }
                        Err(e) => (400, e),
                    }
                }
                _ => (404, "not found".into()),
            };
            let header = if url_path == "/" || url_path == "/projection" {
                "text/html; charset=utf-8"
            } else if url_path == "/api/projection" {
                "application/json; charset=utf-8"
            } else {
                "text/plain; charset=utf-8"
            };
            let response = tiny_http::Response::from_string(body)
                .with_status_code(code)
                .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], header.as_bytes()).unwrap());
            let _ = req.respond(response);
        }
    });

    Ok(url)
}
