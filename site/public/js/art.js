/* art.js - p5.brush (standalone build) effects for nothing.
   All brush drawing happens on ONE master offscreen WebGL canvas (p5.brush's
   framebuffers do not survive switching between canvases). Each effect
   redraws its full scene each frame at progress t (deterministic via
   brush.seed), then the result is blitted onto the effect's visible 2D
   canvas. 2D canvases keep their pixels, so the final frame just stays.
   Respects prefers-reduced-motion by drawing the final state once. */

window.Art = (function () {
  "use strict";

  var reduce = false;
  try { reduce = matchMedia("(prefers-reduced-motion: reduce)").matches; } catch (e) {}

  var ok = (function () {
    if (!window.brush) return false;
    try {
      var c = document.createElement("canvas");
      return !!c.getContext("webgl2");
    } catch (e) { return false; }
  })();

  var DPR = Math.min(window.devicePixelRatio || 1, 1.75);
  var MW = 1600, MH = 1100; /* master canvas pixel size */
  var master = null;

  function initMaster() {
    if (master) return true;
    try {
      master = document.createElement("canvas");
      master.width = MW;
      master.height = MH;
      brush.load(master);
      brush.scaleBrushes(1.2);
      return true;
    } catch (e) {
      master = null;
      ok = false;
      return false;
    }
  }

  /* tiny deterministic PRNG for geometry jitter (independent of brush's RNG) */
  function prng(seed) {
    var s = seed >>> 0;
    return function () {
      s = (s * 1664525 + 1013904223) >>> 0;
      return s / 4294967296;
    };
  }

  function easeOut(t) { return 1 - Math.pow(1 - t, 3); }

  var active = [];
  var raf = 0;

  function tick(now) {
    raf = 0;
    if (document.hidden) return; /* resumes on visibilitychange */
    var i, e;
    for (i = 0; i < active.length; i++) {
      e = active[i];
      if (e.done) continue;
      if (e.pausedAt) { e.t0 += now - e.pausedAt; e.pausedAt = 0; }
      var t = Math.min(1, (now - e.t0) / e.dur);
      if (t >= 1) { finish(e); continue; }
      drawFrame(e, easeOut(t));
    }
    active = active.filter(function (a) { return !a.done; });
    if (active.length) raf = requestAnimationFrame(tick);
  }

  document.addEventListener("visibilitychange", function () {
    var now = performance.now();
    if (document.hidden) {
      active.forEach(function (e) { if (!e.pausedAt) e.pausedAt = now; });
    } else if (active.length && !raf) {
      raf = requestAnimationFrame(tick);
    }
  });

  function drawFrame(e, t) {
    e.rnd = prng(e.seed);
    brush.seed(e.seed);
    brush.noiseSeed(e.seed);
    brush.clear();
    /* brush.clear() leaves the buffer at [1,1,1,0]: white with zero alpha,
       which is invalid premultiplied data and blits as opaque white.
       Re-clear to valid transparent black (keep brush.clear() above for its
       internal composite-state reset). */
    var gl = master.getContext("webgl2");
    if (gl) {
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    }
    brush.push();
    brush.translate(-MW / 2, -MH / 2);
    brush.scale(e.k);
    e.fn(e.w, e.h, t, e.rnd);
    brush.pop();
    brush.render();
    /* blit master -> visible 2D canvas (same task as render, buffer intact) */
    var pw = e.cv.width, ph = e.cv.height;
    e.ctx.clearRect(0, 0, pw, ph);
    e.ctx.drawImage(master, 0, 0, pw, ph, 0, 0, pw, ph);
  }

  function finish(e) {
    drawFrame(e, 1);
    e.done = true;
    if (e.onDone) e.onDone();
  }

  /* Kick off an effect on a canvas. name: key in FX. */
  function play(cv, name, opts) {
    if (!ok || !cv || !FX[name] || !initMaster()) return false;
    opts = opts || {};
    var w = cv.clientWidth || cv.offsetWidth;
    var h = cv.clientHeight || cv.offsetHeight;
    if (!w || !h) return false;
    var k = Math.min(DPR, MW / w, MH / h);
    var ctx = null;
    cv.width = Math.round(w * k);
    cv.height = Math.round(h * k);
    try { ctx = cv.getContext("2d"); } catch (e2) {}
    if (!ctx) return false;
    var e = {
      cv: cv, ctx: ctx, w: w, h: h, k: k,
      fn: FX[name],
      seed: opts.seed || 7,
      dur: opts.dur || 1400,
      t0: performance.now(),
      rnd: null,
      onDone: opts.onDone || null,
      done: false,
      pausedAt: 0
    };
    if (reduce || opts.instant) { finish(e); return true; }
    active.push(e);
    if (!raf) raf = requestAnimationFrame(tick);
    return true;
  }

  /* ---- geometry helpers ------------------------------------------ */

  function arcPts(cx, cy, rx, ry, a0, a1, n, rot, jitter, rnd) {
    var pts = [], cosR = Math.cos(rot || 0), sinR = Math.sin(rot || 0);
    for (var i = 0; i <= n; i++) {
      var a = a0 + (a1 - a0) * (i / n);
      var x = Math.cos(a) * rx, y = Math.sin(a) * ry;
      if (jitter) {
        x += (rnd() - 0.5) * jitter;
        y += (rnd() - 0.5) * jitter;
      }
      var px = cx + x * cosR - y * sinR;
      var py = cy + x * sinR + y * cosR;
      var pr = 0.7 + 0.6 * Math.sin(Math.PI * (i / n));
      pts.push([px, py, pr]);
    }
    return pts;
  }

  function part(pts, f) {
    if (f >= 1) return pts;
    var n = Math.max(2, Math.ceil(pts.length * f));
    return pts.slice(0, n);
  }

  /* sub-progress: full progress t remapped to a [from, to] window */
  function win(t, from, to) {
    return Math.max(0, Math.min(1, (t - from) / (to - from)));
  }

  function stroke(name, color, weight, pts, curv) {
    if (pts.length < 2) return;
    brush.set(name, color, weight);
    brush.spline(pts, curv === undefined ? 0.35 : curv);
  }

  /* ---- effects ---------------------------------------------------- */

  var FX = {

    /* 01: scribbled ellipses drawing in around the wordmark */
    intro: function (W, H, t, rnd) {
      var cx = W / 2, cy = H / 2;
      var loops = [
        { rx: W * 0.40, ry: H * 0.32, rot: -0.16, a0: -0.9, span: 6.8, w: 0.75, c: "#c9c9cf", from: 0.00, to: 0.62 },
        { rx: W * 0.44, ry: H * 0.26, rot: 0.10, a0: 2.1, span: 6.4, w: 0.65, c: "#8f8f97", from: 0.16, to: 0.80 },
        { rx: W * 0.37, ry: H * 0.38, rot: -0.04, a0: 4.4, span: 6.0, w: 0.6, c: "#6a6a72", from: 0.34, to: 1.00 }
      ];
      loops.forEach(function (L) {
        var f = win(t, L.from, L.to);
        if (f <= 0) return;
        var pts = arcPts(cx, cy, L.rx, L.ry, L.a0, L.a0 + L.span, 72, L.rot, 5, rnd);
        stroke("2B", L.c, L.w, part(pts, f), 0.3);
      });
    },

    /* 02: big tilted ring sweep, top right, revealing the hero */
    hero: function (W, H, t, rnd) {
      var cx = W * 0.66, cy = H * 0.38;
      var rx = W * 0.30, ry = H * 0.27;
      var rot = -0.42;
      var a0 = 2.15, span = Math.PI * 1.86; /* open gap at lower left */
      /* soft charcoal bed */
      var f1 = win(t, 0, 0.85);
      stroke("charcoal", "#3c3c42", 2.1,
        part(arcPts(cx, cy, rx * 1.01, ry * 1.02, a0, a0 + span, 96, rot, 7, rnd), f1), 0.3);
      /* bright core line */
      var f2 = win(t, 0.08, 0.96);
      stroke("2B", "#cfcfd4", 1.0,
        part(arcPts(cx, cy, rx * 0.985, ry * 0.985, a0 + 0.04, a0 + span - 0.02, 96, rot, 3, rnd), f2), 0.3);
      /* faint echo ring, slightly offset */
      var f3 = win(t, 0.3, 1.0);
      stroke("HB", "#46464d", 0.8,
        part(arcPts(cx, cy, rx * 1.12, ry * 1.16, a0 + 0.5, a0 + span * 0.82, 70, rot + 0.05, 8, rnd), f3), 0.3);
      /* particle specks drifting off the ring, upper right */
      if (t > 0.4) {
        var fT = win(t, 0.4, 1.0);
        for (var i = 0; i < 8 * fT; i++) {
          var a = a0 + span * (0.55 + rnd() * 0.45);
          var k = 1.05 + rnd() * 0.3;
          var x0 = Math.cos(a) * rx * k, y0 = Math.sin(a) * ry * k;
          var x = cx + x0 * Math.cos(rot) - y0 * Math.sin(rot);
          var y = cy + x0 * Math.sin(rot) + y0 * Math.cos(rot);
          brush.set("spray", "#6a6a72", 0.5 + rnd() * 0.6);
          brush.line(x, y, x + (rnd() - 0.5) * 22, y + (rnd() - 0.5) * 16);
        }
      }
      /* single cool accent: tinted tail at the sweep tip */
      if (t > 0.72) {
        var fA = win(t, 0.72, 1.0);
        var full = arcPts(cx, cy, rx * 0.985, ry * 0.985, a0 + 0.04, a0 + span - 0.02, 96, rot, 2, rnd);
        var i0 = Math.floor(full.length * 0.86);
        var i1 = Math.floor(full.length * (0.86 + 0.14 * fA)) + 1;
        stroke("pen", "#7b8cf0", 0.8, full.slice(i0, i1), 0.3);
        if (fA > 0.55) stroke("pen", "#9a6bff", 0.5,
          part(full.slice(Math.floor(full.length * 0.94)), win(fA, 0.55, 1)), 0.3);
      }
    },

    /* 03: orbit paths drawing in around the AST core */
    orbit: function (W, H, t, rnd) {
      var cx = W / 2, cy = H / 2;
      var orbits = [
        { rx: W * 0.42, ry: H * 0.30, rot: -0.30, c: "#5f5f66", w: 0.6, from: 0.0, to: 0.6 },
        { rx: W * 0.33, ry: H * 0.40, rot: 0.28, c: "#4a4a52", w: 0.55, from: 0.15, to: 0.78 },
        { rx: W * 0.46, ry: H * 0.22, rot: 0.06, c: "#6a6a72", w: 0.5, from: 0.3, to: 1.0 }
      ];
      orbits.forEach(function (O, k) {
        var f = win(t, O.from, O.to);
        if (f <= 0) return;
        var a0 = k * 2.3;
        var pts = arcPts(cx, cy, O.rx, O.ry, a0, a0 + Math.PI * 2.02, 96, O.rot, 2.5, rnd);
        stroke("rotring", O.c, O.w, part(pts, f), 0.4);
      });
      /* small satellites on the paths */
      if (t > 0.75) {
        var sats = [[0.30, 0.24], [0.72, 0.20], [0.55, 0.86], [0.16, 0.62]];
        var fS = win(t, 0.75, 1.0);
        for (var i = 0; i < sats.length * fS; i++) {
          brush.set("pen", "#c9c9cf", 0.7);
          brush.circle(W * sats[i][0], H * sats[i][1], 2.6);
        }
      }
    },

    /* 04: hand-drawn underline for feature cards */
    underline: function (W, H, t, rnd) {
      var y = H * 0.55;
      var pts = [];
      for (var i = 0; i <= 8; i++) {
        pts.push([2 + (W - 6) * (i / 8), y + (rnd() - 0.5) * 3.2, 0.8 + rnd() * 0.5]);
      }
      stroke("marker", "#a9a9b1", 0.9, part(pts, t), 0.25);
    },

    /* 05: low shallow sweep behind the terminal */
    term: function (W, H, t, rnd) {
      var cy = -H * 2.05;
      var R = H * 2.85;
      var f1 = win(t, 0, 0.9);
      stroke("charcoal", "#232327", 3.2,
        part(arcPts(W * 0.5, cy, R, R, 1.22, 1.92, 70, 0, 12, rnd), f1), 0.3);
      var f2 = win(t, 0.1, 1.0);
      stroke("charcoal", "#3a3a40", 1.6,
        part(arcPts(W * 0.5, cy, R * 0.985, R * 0.985, 1.24, 1.9, 70, 0, 7, rnd), f2), 0.3);
      var f3 = win(t, 0.2, 1.0);
      stroke("2B", "#55555c", 0.9,
        part(arcPts(W * 0.5, cy, R * 0.975, R * 0.975, 1.26, 1.88, 70, 0, 4, rnd), f3), 0.3);
      if (t > 0.5) {
        var fT = win(t, 0.5, 1.0);
        for (var i = 0; i < 6 * fT; i++) {
          var x = W * (0.2 + rnd() * 0.6), y = H * (0.55 + rnd() * 0.3);
          brush.set("spray", "#4a4a52", 0.8 + rnd() * 0.8);
          brush.line(x, y, x + (rnd() - 0.5) * 50, y + (rnd() - 0.5) * 12);
        }
      }
    },

    /* 06: giant brushed hole brackets, the empty program */
    figure: function (W, H, t, rnd) {
      var cy = H * 0.5;
      var rh = H * 0.30;          /* bracket half-height */
      var rw = W * 0.115;         /* bracket curve width */
      var gap = W * 0.155;        /* half distance between brackets */
      var cxL = W * 0.5 - gap, cxR = W * 0.5 + gap;

      /* left "(" : arc opens right; charcoal body + pen edge */
      var fL = win(t, 0.0, 0.62);
      var aL0 = Math.PI * 0.5 + 0.35, aL1 = Math.PI * 1.5 - 0.35;
      stroke("charcoal", "#9a9aa2", 2.2,
        part(arcPts(cxL + rw * 0.4, cy, rw, rh, aL1, aL0, 60, 0, 5, rnd), fL), 0.35);
      if (t > 0.1) stroke("pen", "#d8d8dc", 0.6,
        part(arcPts(cxL + rw * 0.36, cy, rw * 0.94, rh * 0.97, aL1, aL0, 60, 0, 2, rnd), win(t, 0.1, 0.66)), 0.35);
      /* left inner bar "|", tight against the arc tips */
      if (t > 0.22) {
        var fB = win(t, 0.22, 0.72);
        var bx = cxL + rw * 0.18;
        var bpts = [];
        for (var i = 0; i <= 10; i++) {
          bpts.push([bx + (rnd() - 0.5) * 2.5, cy - rh * 0.62 + rh * 1.24 * (i / 10), 0.85]);
        }
        stroke("2B", "#77777f", 1.1, part(bpts, fB), 0.15);
      }

      /* right ")" mirrored */
      var fR = win(t, 0.28, 0.9);
      var aR0 = -Math.PI * 0.5 + 0.35, aR1 = Math.PI * 0.5 - 0.35;
      stroke("charcoal", "#9a9aa2", 2.2,
        part(arcPts(cxR - rw * 0.4, cy, rw, rh, aR0, aR1, 60, 0, 5, rnd), fR), 0.35);
      if (t > 0.38) stroke("pen", "#d8d8dc", 0.6,
        part(arcPts(cxR - rw * 0.36, cy, rw * 0.94, rh * 0.97, aR0, aR1, 60, 0, 2, rnd), win(t, 0.38, 0.94)), 0.35);
      if (t > 0.5) {
        var fB2 = win(t, 0.5, 1.0);
        var bx2 = cxR - rw * 0.18;
        var bpts2 = [];
        for (var j = 0; j <= 10; j++) {
          bpts2.push([bx2 + (rnd() - 0.5) * 2.5, cy - rh * 0.62 + rh * 1.24 * (j / 10), 0.85]);
        }
        stroke("2B", "#77777f", 1.1, part(bpts2, fB2), 0.15);
      }

      /* bloom: fine spray drifting off the brackets */
      if (t > 0.55) {
        var fS = win(t, 0.55, 1.0);
        for (var k = 0; k < 8 * fS; k++) {
          var side = rnd() > 0.5 ? cxL : cxR;
          var x = side + (rnd() - 0.5) * rw * 2.2;
          var y = cy + (rnd() - 0.5) * rh * 2.1;
          brush.set("spray", "#46464d", 0.5 + rnd() * 0.6);
          brush.line(x, y, x + (rnd() - 0.5) * 24, y + (rnd() - 0.5) * 24);
        }
      }
    }
  };

  return { ok: ok, reduce: reduce, play: play };
})();
