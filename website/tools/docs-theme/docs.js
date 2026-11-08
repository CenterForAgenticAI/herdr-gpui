/* sheprd.org/docs: search palette, copy buttons, contents menu, and the current section. */
(function () {
  "use strict";
  var $$ = function (s, r) { return Array.prototype.slice.call((r || document).querySelectorAll(s)); };
  var esc = function (s) { return String(s).replace(/[&<>"]/g, function (c) { return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]; }); };

  /* Comments, strings and a few keywords; the text itself is never interpreted. */
  var RULES = {
    lua: /(--.*$)|("(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*')|\b(local|function|end|if|then|not|else|elseif|for|in|do|return|and|or|nil|true|false)\b|\b(\d[\d_.]*)\b/gm,
    luau: null,
    toml: /(#.*$)|("(?:[^"\\\n]|\\.)*"|'[^'\n]*')|^(\s*\[[^\]\n]+\])|\b(true|false|\d[\d_.]*)\b/gm,
    sh: /(#.*$)|("(?:[^"\\\n]|\\.)*"|'[^'\n]*')|^(\$ )/gm,
    powershell: /(#.*$)|("(?:[^"`\n]|`.)*"|'[^'\n]*')/gm
  };
  RULES.luau = RULES.lua; RULES.bash = RULES.sh; RULES.shell = RULES.sh; RULES.console = RULES.sh;
  function highlight(code) {
    var lang = (code.className.match(/language-(\w+)/) || [])[1], re = RULES[lang];
    if (!re) return;
    var src = code.textContent, out = "", last = 0, m;
    re.lastIndex = 0;
    while ((m = re.exec(src))) {
      if (!m[0]) { re.lastIndex++; continue; }
      out += esc(src.slice(last, m.index));
      var cls = m[1] ? "c" : m[2] ? "s" : m[3] ? "k" : "n";
      out += '<span class="t-' + cls + '">' + esc(m[0]) + "</span>";
      last = m.index + m[0].length;
    }
    code.innerHTML = out + esc(src.slice(last));
  }
  $$("pre > code").forEach(function (code) {
    highlight(code);
    var b = document.createElement("button");
    b.type = "button"; b.className = "copy"; b.textContent = "Copy";
    b.addEventListener("click", function () {
      var done = function () { b.textContent = "Copied"; setTimeout(function () { b.textContent = "Copy"; }, 1200); };
      if (navigator.clipboard) navigator.clipboard.writeText(code.textContent).then(done, function () {});
    });
    code.parentNode.appendChild(b);
  });

  var menu = document.querySelector("[data-menu]");
  if (menu) menu.addEventListener("click", function () {
    var open = document.documentElement.classList.toggle("menu-open");
    menu.setAttribute("aria-expanded", open);
  });

  var tocLinks = $$("[data-toc]");
  if (tocLinks.length && "IntersectionObserver" in window) {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (!e.isIntersecting) return;
        tocLinks.forEach(function (l) { l.classList.toggle("on", l.dataset.toc === e.target.id); });
      });
    }, { rootMargin: "-15% 0px -75% 0px" });
    tocLinks.forEach(function (l) { var h = document.getElementById(l.dataset.toc); if (h) io.observe(h); });
  }

  /* Search: page titles first, then section titles, then section text. */
  var entries = [];
  function indexEntries() {
    if (entries.length || !window.SHEPRD_DOCS_INDEX) return;
    window.SHEPRD_DOCS_INDEX.forEach(function (p) {
      entries.push({ title: p.t, sub: p.g + " · " + p.b, url: p.u, hay: [p.t, p.b, ""] });
      p.s.forEach(function (s) { entries.push({ title: s.title, sub: p.t, url: p.u + "#" + s.id, hay: ["", s.title, s.text], text: s.text }); });
    });
  }
  function score(e, words) {
    var total = 0;
    for (var i = 0; i < words.length; i++) {
      var w = words[i], t = e.hay[0].toLowerCase(), h = e.hay[1].toLowerCase(), x = e.hay[2].toLowerCase();
      var s = t.indexOf(w) >= 0 ? 30 : h.indexOf(w) >= 0 ? 12 : x.indexOf(w) >= 0 ? 3 : 0;
      if (!s) return 0;
      total += s;
    }
    return total;
  }
  function snippet(e, words) {
    if (!e.text) return e.sub;
    var lower = e.text.toLowerCase(), at = -1;
    words.forEach(function (w) { var i = lower.indexOf(w); if (i >= 0 && (at < 0 || i < at)) at = i; });
    if (at < 0) return e.sub;
    return e.sub + " · " + (at > 30 ? "…" : "") + e.text.slice(Math.max(0, at - 30), at + 90);
  }
  var pal, input, list, results = [], active = 0;
  function draw() {
    var words = input.value.toLowerCase().split(/\s+/).filter(Boolean);
    results = words.length
      ? entries.map(function (e) { return [score(e, words), e]; }).filter(function (r) { return r[0]; })
          .sort(function (a, b) { return b[0] - a[0]; }).slice(0, 12).map(function (r) { return r[1]; })
      : entries.filter(function (e) { return !e.text; }).slice(0, 12);
    active = 0;
    list.innerHTML = results.length ? results.map(function (e, n) {
      return '<li><a href="' + esc(e.url) + '"' + (n ? "" : ' class="on"') + "><b>" + esc(e.title) + "</b><span>" + esc(snippet(e, words)) + "</span></a></li>";
    }).join("") : '<li class="pal-none">No page matches.</li>';
  }
  function mark() { $$("a", list).forEach(function (a, n) { a.classList.toggle("on", n === active); if (n === active) a.scrollIntoView({ block: "nearest" }); }); }
  function close() { if (pal) pal.hidden = true; }
  function open() {
    indexEntries();
    if (!pal) {
      pal = document.createElement("div"); pal.id = "palette";
      pal.innerHTML = '<div class="pal-box" role="dialog" aria-modal="true" aria-label="Search the docs"><input type="search" placeholder="Search the docs…" aria-label="Search the docs"><ul></ul></div>';
      document.body.appendChild(pal);
      input = pal.querySelector("input"); list = pal.querySelector("ul");
      pal.addEventListener("click", function (e) { if (e.target === pal) close(); if (e.target.closest("a")) close(); });
      input.addEventListener("input", draw);
      input.addEventListener("keydown", function (e) {
        if (e.key === "ArrowDown") { active = Math.min(active + 1, results.length - 1); mark(); e.preventDefault(); }
        else if (e.key === "ArrowUp") { active = Math.max(active - 1, 0); mark(); e.preventDefault(); }
        else if (e.key === "Enter" && results[active]) { close(); location.href = results[active].url; }
      });
    }
    pal.hidden = false; input.value = ""; draw(); input.focus();
  }
  document.addEventListener("click", function (e) { if (e.target.closest("[data-search]")) open(); });
  document.addEventListener("keydown", function (e) {
    var typing = /^(INPUT|TEXTAREA|SELECT)$/.test((e.target.tagName || "")) && e.target !== input;
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") { e.preventDefault(); open(); }
    else if (e.key === "/" && !typing && (!pal || pal.hidden)) { e.preventDefault(); open(); }
    else if (e.key === "Escape") close();
  });
})();
