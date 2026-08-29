/* main.js - intro, scroll reveals, art triggers, terminal typing, parallax */

(function () {
  "use strict";

  var doc = document;
  var root = doc.documentElement;
  var reduce = false;
  try { reduce = matchMedia("(prefers-reduced-motion: reduce)").matches; } catch (e) {}
  var art = window.Art && window.Art.ok ? window.Art : null;

  /* ---- nav ------------------------------------------------------- */

  var nav = doc.getElementById("nav");
  function onScroll() {
    if (nav) nav.classList.toggle("solid", window.scrollY > 24);
  }
  window.addEventListener("scroll", onScroll, { passive: true });
  onScroll();

  /* ---- 01 intro overlay ------------------------------------------ */

  var intro = doc.getElementById("intro");
  var introDone = !root.classList.contains("intro");

  function endIntro() {
    if (introDone) return;
    introDone = true;
    if (intro) intro.classList.add("out");
    setTimeout(function () {
      root.classList.remove("intro");
      if (intro && intro.parentNode) intro.parentNode.removeChild(intro);
    }, 420);
    startPage();
  }

  if (!introDone && intro) {
    if (art) art.play(doc.getElementById("intro-canvas"), "intro", { dur: 950, seed: 5 });
    var kill = setTimeout(endIntro, 1200);
    ["wheel", "touchstart", "keydown", "pointerdown"].forEach(function (ev) {
      window.addEventListener(ev, function h() {
        clearTimeout(kill);
        endIntro();
      }, { once: true, passive: true });
    });
  }

  /* ---- reveals + section art ------------------------------------- */

  var started = false;

  function startPage() {
    if (started) return;
    started = true;

    /* 02: hero sweep */
    if (art) art.play(doc.getElementById("hero-canvas"), "hero", { dur: 1700, seed: 9 });

    var io = "IntersectionObserver" in window ? new IntersectionObserver(function (entries) {
      entries.forEach(function (en) {
        if (!en.isIntersecting) return;
        en.target.classList.add("in");
        io.unobserve(en.target);
      });
    }, { threshold: 0.12, rootMargin: "0px 0px -6% 0px" }) : null;

    var reveals = doc.querySelectorAll(".reveal");
    if (io) {
      reveals.forEach(function (el) { io.observe(el); });
    } else {
      reveals.forEach(function (el) { el.classList.add("in"); });
    }

    /* one-shot section triggers */
    function once(el, fn, threshold) {
      if (!el) return;
      if (!("IntersectionObserver" in window)) { fn(); return; }
      var o = new IntersectionObserver(function (entries) {
        entries.forEach(function (en) {
          if (!en.isIntersecting) return;
          o.disconnect();
          fn();
        });
      }, { threshold: threshold === undefined ? 0.25 : threshold });
      o.observe(el);
    }

    /* 03: orbits draw, then nodes appear */
    var orbitArt = doc.querySelector(".orbit-art");
    once(orbitArt, function () {
      if (art) art.play(doc.getElementById("orbit-canvas"), "orbit", { dur: 1700, seed: 11 });
      setTimeout(function () { orbitArt.classList.add("nodes"); }, reduce || !art ? 0 : 750);
    });

    /* 04: card underlines */
    doc.querySelectorAll(".card").forEach(function (card, i) {
      once(card, function () {
        var cv = card.querySelector("canvas.stroke-line");
        if (!cv || !art) return;
        setTimeout(function () {
          art.play(cv, "underline", { dur: 600, seed: 31 + i * 7 });
        }, reduce ? 0 : 250 + i * 130);
      }, 0.2);
    });

    /* 05: terminal sweep + typing */
    var termSec = doc.querySelector(".term-sec");
    once(termSec, function () {
      if (art) art.play(doc.getElementById("term-canvas"), "term", { dur: 1600, seed: 17 });
    }, 0.2);

    initTerminal();

    /* 06: figure + parallax */
    var endSec = doc.querySelector(".end-sec");
    once(endSec, function () {
      if (art) art.play(doc.getElementById("end-canvas"), "figure", { dur: 1900, seed: 23 });
    }, 0.2);

    if (!reduce && endSec && matchMedia("(pointer: fine)").matches) {
      var endArt = doc.getElementById("end-art");
      endSec.addEventListener("mousemove", function (e) {
        var r = endSec.getBoundingClientRect();
        var dx = ((e.clientX - r.left) / r.width - 0.5) * 14;
        var dy = ((e.clientY - r.top) / r.height - 0.5) * 10;
        endArt.style.transform = "translate3d(" + dx + "px," + dy + "px,0)";
      });
      endSec.addEventListener("mouseleave", function () {
        endArt.style.transform = "";
      });
    }
  }

  /* ---- 05 terminal typing ---------------------------------------- */

  function initTerminal() {
    var log = doc.getElementById("term-log");
    if (!log || reduce || !window.requestAnimationFrame) return;

    var term = log.closest(".term");
    var rows = Array.prototype.slice.call(log.querySelectorAll(".trow"));
    if (!rows.length) return;

    var steps = rows.map(function (row) {
      var t = row.querySelector(".tt");
      return { t: t, text: t.textContent, v: row.querySelector(".tv") };
    });

    var cursor = doc.createElement("span");
    cursor.className = "tcur";
    cursor.setAttribute("aria-hidden", "true");

    var inView = false, running = false;

    function sleep(ms) {
      return new Promise(function (res) { setTimeout(res, ms); });
    }

    function gate() {
      if (inView && !doc.hidden) return Promise.resolve();
      return new Promise(function (res) {
        var check = function () {
          if (inView && !doc.hidden) {
            doc.removeEventListener("visibilitychange", check);
            window.removeEventListener("term-visible", check);
            res();
          }
        };
        doc.addEventListener("visibilitychange", check);
        window.addEventListener("term-visible", check);
      });
    }

    function reset() {
      steps.forEach(function (s) {
        s.t.textContent = "";
        s.v.classList.remove("show");
      });
    }

    async function run() {
      running = true;
      term.classList.add("anim");
      for (;;) {
        await gate();
        reset();
        await sleep(400);
        for (var i = 0; i < steps.length; i++) {
          var s = steps[i];
          s.t.parentNode.appendChild(cursor);
          for (var c = 1; c <= s.text.length; c++) {
            s.t.textContent = s.text.slice(0, c);
            await sleep(36);
          }
          await sleep(300);
          s.v.classList.add("show");
          await sleep(i === steps.length - 1 ? 3400 : 680);
          await gate();
        }
      }
    }

    if ("IntersectionObserver" in window) {
      var o = new IntersectionObserver(function (entries) {
        entries.forEach(function (en) {
          inView = en.isIntersecting;
          if (inView) {
            window.dispatchEvent(new Event("term-visible"));
            if (!running) run();
          }
        });
      }, { threshold: 0.35 });
      o.observe(term);
    } else {
      inView = true;
      run();
    }
  }

  /* no intro: start immediately */
  if (introDone) startPage();
})();
