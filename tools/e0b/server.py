#!/usr/bin/env python3
"""E0b (solo medición): servidor local de prueba para el build diag SIN custom-protocol (que carga
devUrl = http://localhost:1420). Sirve una COPIA de dist/ con el CSS de la variante activa y un script
que fija data-state (idle/resting) sin depender de la maquinaria de Rust. No es parte del producto.

Uso: server.py <dist> <variants_dir> <state_file> <out_dir>
  state_file: JSON {"cfg": "...", "first": "idle", "second": "resting", "switch_ms": 40000}
"""
import json, sys, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

DIST, VARS, STATE, OUT = (Path(a) for a in sys.argv[1:5])
OUT.mkdir(parents=True, exist_ok=True)


def cfg():
    return json.loads(STATE.read_text(encoding="utf-8"))


PIN = r"""
(function(){
  var C = __CFG__;
  var cur = C.first, t0 = performance.now();
  function apply(){ var b=document.querySelector('.bee'); if(b && b.getAttribute('data-state')!==cur) b.setAttribute('data-state',cur); }
  new MutationObserver(apply).observe(document.documentElement,{subtree:true,attributes:true,attributeFilter:['data-state']});
  var iv = setInterval(function(){ apply(); if(document.querySelector('.bee')) clearInterval(iv); }, 50);
  setTimeout(function(){ cur = C.second; apply(); }, C.switch_ms);
  function report(tag){
    var an = document.getAnimations().map(function(a){ return {n:a.animationName||a.transitionProperty||'?', s:a.playState}; });
    var cnt = {}; an.forEach(function(x){ var k=x.n+':'+x.s; cnt[k]=(cnt[k]||0)+1; });
    var b = document.querySelector('.bee');
    var body = JSON.stringify({cfg:C.cfg, tag:tag, state:b&&b.getAttribute('data-state'), total:an.length,
      counts:cnt, reducedMotion:matchMedia('(prefers-reduced-motion: reduce)').matches, dpr:devicePixelRatio});
    var x = new XMLHttpRequest(); x.open('POST','/__report'); x.send(body);
  }
  setTimeout(function(){ report('idle'); }, 5000);
  setTimeout(function(){ report('rest'); }, C.switch_ms + 5000);
  if (C.fps) {
    // solo para la verificacion: cadencia real de fotogramas durante 3 s (rAF) — NO se usa en las mediciones
    setTimeout(function(){ var n=0, s=performance.now(); (function f(t){ n++; if(t-s<3000) requestAnimationFrame(f); else { var x=new XMLHttpRequest(); x.open('POST','/__report'); x.send(JSON.stringify({cfg:C.cfg,tag:'fps',frames:n,ms:t-s})); } })(s); }, 8000);
  }
})();
"""


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _send(self, code, body, ctype):
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0))
        data = self.rfile.read(n).decode("utf-8", "replace")
        with open(OUT / "reports.jsonl", "a", encoding="utf-8") as f:
            f.write(json.dumps({"t": time.time(), "body": json.loads(data)}) + "\n")
        self._send(200, b"ok", "text/plain")

    def do_GET(self):
        path = self.path.split("?")[0]
        c = cfg()
        if path in ("/", "/index.html"):
            html = (DIST / "index.html").read_text(encoding="utf-8")
            html = html.replace("</head>", '<script src="/__pin.js"></script></head>', 1)
            return self._send(200, html.encode("utf-8"), "text/html; charset=utf-8")
        if path == "/__pin.js":
            (OUT / "pagestart.txt").write_text(repr(time.time()), encoding="utf-8")
            if c.get("nopin"):
                return self._send(200, b"/* sin fijar estado */", "application/javascript")
            return self._send(200, PIN.replace("__CFG__", json.dumps(c)).encode("utf-8"), "application/javascript")
        f = (DIST / path.lstrip("/")).resolve()
        if DIST.resolve() not in f.parents or not f.is_file():
            return self._send(404, b"nf", "text/plain")
        body = f.read_bytes()
        if f.suffix == ".css":
            extra = (VARS / (c["cfg"] + ".css"))
            if extra.exists():
                body += b"\n" + extra.read_bytes()
        ctype = {".js": "application/javascript", ".css": "text/css", ".svg": "image/svg+xml"}.get(f.suffix, "application/octet-stream")
        self._send(200, body, ctype)


ThreadingHTTPServer(("127.0.0.1", 1420), H).serve_forever()
