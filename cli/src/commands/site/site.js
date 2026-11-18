/* skemo site: pan, zoom, the way between the diagrams and a search of their cards (D-097). No
   library, no network. */
(function () {
  'use strict';
  var root = document.documentElement;
  var reduced = window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches;

  /* the side panel on narrow screens */
  var side = document.querySelector('.sk-side');
  var menu = document.querySelector('.sk-menu');
  if (menu && side) {
    menu.addEventListener('click', function () {
      var open = side.classList.toggle('sk-open');
      menu.setAttribute('aria-expanded', open ? 'true' : 'false');
    });
  }

  /* filter the list of diagrams */
  var filter = document.querySelector('.sk-filter');
  if (filter) {
    filter.addEventListener('input', function () {
      var q = filter.value.trim().toLowerCase();
      var items = document.querySelectorAll('.sk-tree li');
      for (var i = items.length - 1; i >= 0; i--) {
        var li = items[i];
        var hit;
        if (li.classList.contains('sk-folder')) {
          hit = !q || li.querySelector('li:not(.sk-hidden):not(.sk-folder)') !== null;
        } else {
          var a = li.querySelector('a');
          hit = !q || (a.textContent + ' ' + a.title).toLowerCase().indexOf(q) >= 0;
        }
        li.classList.toggle('sk-hidden', !hit);
      }
    });
  }

  /* the panel's width: dragged on its edge (or arrow keys), kept for the next pages; a double-click
     goes back to the default */
  var handle = document.querySelector('.sk-resize');
  if (handle && side) {
    var NARROWEST = 180;
    var WIDEST = 640;
    var refit = function () { window.dispatchEvent(new Event('resize')); };
    var setWidth = function (w, keep) {
      w = Math.max(NARROWEST, Math.min(WIDEST, Math.round(w)));
      root.style.setProperty('--sk-side', w + 'px');
      handle.setAttribute('aria-valuenow', String(w));
      if (keep) {
        try { localStorage.setItem('skemo-site-side', String(w)); } catch (e) { /* private window */ }
      }
      refit();
    };
    handle.setAttribute('aria-valuenow', String(Math.round(side.getBoundingClientRect().width)));
    handle.addEventListener('pointerdown', function (e) {
      if (e.button !== 0) return;
      e.preventDefault();
      try { handle.setPointerCapture(e.pointerId); } catch (err) { /* a pointer already gone */ }
      root.classList.add('sk-resizing');
      var left = side.getBoundingClientRect().left;
      var move = function (ev) { setWidth(ev.clientX - left, false); };
      var up = function (ev) {
        handle.removeEventListener('pointermove', move);
        handle.removeEventListener('pointerup', up);
        handle.removeEventListener('pointercancel', up);
        root.classList.remove('sk-resizing');
        setWidth(ev.clientX - left, true);
      };
      handle.addEventListener('pointermove', move);
      handle.addEventListener('pointerup', up);
      handle.addEventListener('pointercancel', up);
    });
    handle.addEventListener('dblclick', function () {
      root.style.removeProperty('--sk-side');
      try { localStorage.removeItem('skemo-site-side'); } catch (e) { /* private window */ }
      handle.setAttribute('aria-valuenow', String(Math.round(side.getBoundingClientRect().width)));
      refit();
    });
    handle.addEventListener('keydown', function (e) {
      var w = side.getBoundingClientRect().width;
      if (e.key === 'ArrowLeft') setWidth(w - 16, true);
      else if (e.key === 'ArrowRight') setWidth(w + 16, true);
      else return;
      e.preventDefault();
    });
  }

  /* light or dark (--theme auto) */
  var sw = document.querySelector('.sk-theme');
  if (sw) {
    sw.addEventListener('click', function () {
      var t = root.dataset.theme === 'dark' ? 'light' : 'dark';
      root.dataset.theme = t;
      try { localStorage.setItem('skemo-site-theme', t); } catch (e) { /* private window */ }
    });
  }

  /* search the cards of every diagram: the index (assets/search.js) is a script, loaded when the box
     is first used, so that a site opened from the disk can read it */
  var focusHere = null; /* set below on a diagram's page: centres one of its cards */
  var q = document.querySelector('.sk-q');
  var hits = document.getElementById('sk-hits');
  if (q && hits) {
    var base = q.getAttribute('data-root') || '';
    var cards = null;
    var loading = false;
    var shown = [];
    var sel = -1;
    var SHOWN = 60;
    var fold = function (s) {
      return (s || '').normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase();
    };
    var load = function (then) {
      if (cards || loading) { if (cards) then(); return; }
      loading = true;
      var s = document.createElement('script');
      s.src = base + 'assets/search.js';
      s.onload = function () {
        var ix = window.skemoIndex || { d: [], c: [] };
        cards = ix.c.map(function (c, i) {
          var doc = ix.d[c[0]] || ['', '', ''];
          var name = doc[2] ? doc[2] + ' \u203a ' + doc[0] : doc[0];
          return {
            i: i, id: c[1], title: c[2], category: c[3], tag: c[4], zone: c[5], lines: c[6],
            noteOf: c[7], doc: name, page: doc[1], ft: fold(c[2]),
            all: fold([c[2], c[3], c[4], c[5], c[6], c[7], name].join(' \u0001 '))
          };
        });
        then();
      };
      s.onerror = function () { loading = false; };
      document.head.appendChild(s);
    };
    /* every word somewhere; the title first: the same, then starting so, then holding it; a note
       (found by the card it annotates or its lines) after the cards */
    var rank = function (c, query, words) {
      for (var i = 0; i < words.length; i++) if (c.all.indexOf(words[i]) < 0) return -1;
      if (c.ft === query) return 0;
      if (c.ft.indexOf(query) === 0) return 1;
      if (c.ft.indexOf(query) >= 0) return 2;
      var inTitle = words.every(function (w) { return c.ft.indexOf(w) >= 0; });
      return (inTitle ? 3 : 4) + (c.noteOf ? 2 : 0);
    };
    /* `text` into `el`, the words found marked (only when folding kept its length) */
    var marked = function (el, text, words) {
      var f = fold(text);
      var on = [];
      if (f.length === text.length) {
        words.forEach(function (w) {
          for (var at = f.indexOf(w); w && at >= 0; at = f.indexOf(w, at + 1)) {
            for (var k = at; k < at + w.length; k++) on[k] = true;
          }
        });
      }
      var run = '';
      var flush = function (m) {
        if (!run) return;
        var n = m ? document.createElement('mark') : document.createTextNode(run);
        if (m) n.textContent = run;
        el.appendChild(n);
        run = '';
      };
      for (var i = 0; i < text.length; i++) {
        if (i > 0 && !!on[i] !== !!on[i - 1]) flush(!!on[i - 1]);
        run += text.charAt(i);
      }
      flush(!!on[text.length - 1]);
    };
    var span = function (cls, text, words) {
      var e = document.createElement('span');
      e.className = cls;
      if (words) marked(e, text, words); else e.textContent = text;
      return e;
    };
    var link = function (c) {
      return base + c.page.split('/').map(encodeURIComponent).join('/') + '#' + encodeURIComponent(c.id);
    };
    var close = function () {
      hits.hidden = true;
      q.setAttribute('aria-expanded', 'false');
      q.removeAttribute('aria-activedescendant');
      sel = -1;
    };
    var select = function (k) {
      var items = hits.querySelectorAll('.sk-hit');
      if (!items.length) return;
      sel = (k + items.length) % items.length;
      for (var i = 0; i < items.length; i++) items[i].setAttribute('aria-selected', i === sel ? 'true' : 'false');
      q.setAttribute('aria-activedescendant', items[sel].id);
      items[sel].scrollIntoView({ block: 'nearest' });
    };
    var render = function (found, words, query) {
      hits.textContent = '';
      var pages = {};
      found.forEach(function (f) { pages[f.c.page] = true; });
      var n = Object.keys(pages).length;
      var head = document.createElement('div');
      head.className = 'sk-hits-head';
      head.textContent = found.length
        ? found.length + (found.length > 1 ? ' cards' : ' card') + ' in ' + n + (n > 1 ? ' diagrams' : ' diagram')
        : 'No card matches \u201c' + q.value.trim() + '\u201d';
      hits.appendChild(head);
      shown = found.slice(0, SHOWN).map(function (f) { return f.c; });
      shown.forEach(function (c, k) {
        var a = document.createElement('a');
        a.className = 'sk-hit';
        a.id = 'sk-hit-' + k;
        a.href = link(c);
        a.setAttribute('role', 'option');
        a.setAttribute('aria-selected', 'false');
        var top = document.createElement('span');
        top.className = 'sk-hit-top';
        top.appendChild(span('sk-hit-title', c.title || c.id, words));
        [c.category, c.tag].forEach(function (t) { if (t) top.appendChild(span('sk-hit-chip', t, words)); });
        a.appendChild(top);
        var where = [c.doc];
        if (c.zone) where.push(c.zone);
        if (c.noteOf) where.push('note on ' + c.noteOf);
        a.appendChild(span('sk-hit-where', where.join(' \u00b7 '), words));
        if (c.lines) a.appendChild(span('sk-hit-lines', c.lines, words));
        hits.appendChild(a);
      });
      if (found.length > SHOWN) {
        var more = document.createElement('div');
        more.className = 'sk-hits-more';
        more.textContent = (found.length - SHOWN) + ' more: add a word to narrow the search';
        hits.appendChild(more);
      }
      hits.hidden = false;
      q.setAttribute('aria-expanded', 'true');
    };
    var search = function () {
      var query = fold(q.value.trim()).replace(/\s+/g, ' ');
      if (!query) { close(); return; }
      if (!cards) { load(search); return; }
      var words = query.split(' ');
      var found = [];
      cards.forEach(function (c) {
        var r = rank(c, query, words);
        if (r >= 0) found.push({ r: r, c: c });
      });
      found.sort(function (a, b) {
        return a.r - b.r || (a.c.ft < b.c.ft ? -1 : a.c.ft > b.c.ft ? 1 : 0) || a.c.i - b.c.i;
      });
      render(found, words, query);
    };
    /* a card of this very page: centred without reloading it */
    var open = function (a, e) {
      var url = new URL(a.href, location.href);
      if (url.pathname === location.pathname && focusHere) {
        if (e) e.preventDefault();
        history.replaceState(null, '', url.hash);
        close();
        q.blur();
        focusHere(decodeURIComponent(url.hash.slice(1)));
      } else if (!e) {
        location.href = url.href;
      }
    };
    q.addEventListener('input', search);
    q.addEventListener('focus', function () { load(search); });
    q.addEventListener('keydown', function (e) {
      if (e.key === 'ArrowDown') { select(sel + 1); e.preventDefault(); }
      else if (e.key === 'ArrowUp') { select(sel - 1); e.preventDefault(); }
      else if (e.key === 'Enter') {
        var items = hits.querySelectorAll('.sk-hit');
        if (!hits.hidden && items.length) { open(items[Math.max(sel, 0)]); e.preventDefault(); }
      } else if (e.key === 'Escape') {
        if (!hits.hidden) close(); else q.blur();
        e.preventDefault();
      }
    });
    hits.addEventListener('click', function (e) {
      var a = e.target.closest && e.target.closest('.sk-hit');
      if (a && !(e.ctrlKey || e.metaKey || e.shiftKey)) open(a, e);
    });
    document.addEventListener('pointerdown', function (e) {
      if (!e.target.closest || !e.target.closest('.sk-search')) close();
    });
    document.addEventListener('keydown', function (e) {
      var t = e.target;
      if (e.key !== '/' || e.ctrlKey || e.metaKey || e.altKey) return;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
      e.preventDefault();
      q.focus();
      q.select();
    });
  }

  var canvas = document.querySelector('.sk-canvas');
  if (!canvas) return;
  var stages = canvas.querySelectorAll('.sk-stage');
  var v = { x: 0, y: 0, k: 1 };
  var moved = false; /* the reader zoomed or panned: a resize keeps the view */

  function visibleSvg() {
    for (var i = 0; i < stages.length; i++) {
      if (stages[i].getClientRects().length) return stages[i].querySelector('svg');
    }
    return stages[0] && stages[0].querySelector('svg');
  }
  function size() {
    var s = visibleSvg();
    return { w: +s.getAttribute('width') || 1, h: +s.getAttribute('height') || 1 };
  }
  function apply() {
    var t = 'translate(' + v.x + 'px,' + v.y + 'px) scale(' + v.k + ')';
    for (var i = 0; i < stages.length; i++) stages[i].style.transform = t;
  }
  function fit() {
    var r = canvas.getBoundingClientRect();
    var s = size();
    var pad = Math.min(32, r.width * 0.05);
    v.k = Math.min(1, (r.width - 2 * pad) / s.w, (r.height - 2 * pad) / s.h);
    v.k = Math.max(v.k, 0.02);
    v.x = (r.width - s.w * v.k) / 2;
    v.y = Math.max(pad, (r.height - s.h * v.k) / 2);
    moved = false;
    apply();
  }
  function zoomAt(cx, cy, f) {
    var k = Math.min(8, Math.max(0.02, v.k * f));
    v.x = cx - (cx - v.x) * (k / v.k);
    v.y = cy - (cy - v.y) * (k / v.k);
    v.k = k;
    moved = true;
    apply();
  }
  function centre() {
    var r = canvas.getBoundingClientRect();
    return { x: r.width / 2, y: r.height / 2 };
  }

  /* buttons and keys */
  document.querySelectorAll('[data-zoom]').forEach(function (b) {
    b.addEventListener('click', function () {
      var z = b.getAttribute('data-zoom');
      var c = centre();
      if (z === 'fit') fit();
      else zoomAt(c.x, c.y, z === 'in' ? 1.25 : 0.8);
    });
  });
  document.addEventListener('keydown', function (e) {
    var t = e.target;
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
    var c = centre();
    if (e.key === '+' || e.key === '=') zoomAt(c.x, c.y, 1.25);
    else if (e.key === '-' || e.key === '_') zoomAt(c.x, c.y, 0.8);
    else if (e.key === '0') fit();
    else if (e.key === 'ArrowLeft') { v.x += 60; moved = true; apply(); }
    else if (e.key === 'ArrowRight') { v.x -= 60; moved = true; apply(); }
    else if (e.key === 'ArrowUp') { v.y += 60; moved = true; apply(); }
    else if (e.key === 'ArrowDown') { v.y -= 60; moved = true; apply(); }
    else return;
    e.preventDefault();
  });

  /* the wheel zooms around the pointer; a trackpad pinch comes as a wheel with ctrl */
  canvas.addEventListener('wheel', function (e) {
    e.preventDefault();
    var r = canvas.getBoundingClientRect();
    var d = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaY;
    zoomAt(e.clientX - r.left, e.clientY - r.top, Math.exp(-d * (e.ctrlKey ? 0.01 : 0.0018)));
  }, { passive: false });

  /* drag to pan, two fingers to zoom; a drag is never a click */
  var pointers = {};
  var start = null;
  var dragged = false;
  function count() { return Object.keys(pointers).length; }
  function pair() {
    var p = Object.keys(pointers).map(function (k) { return pointers[k]; });
    return {
      d: Math.hypot(p[0].x - p[1].x, p[0].y - p[1].y),
      x: (p[0].x + p[1].x) / 2,
      y: (p[0].y + p[1].y) / 2
    };
  }
  canvas.addEventListener('pointerdown', function (e) {
    if (e.button !== 0) return;
    pointers[e.pointerId] = { x: e.clientX, y: e.clientY };
    dragged = false;
    start = count() === 2 ? pair() : { x: e.clientX, y: e.clientY };
  });
  canvas.addEventListener('pointermove', function (e) {
    if (!pointers[e.pointerId] || !start) return;
    pointers[e.pointerId] = { x: e.clientX, y: e.clientY };
    var r = canvas.getBoundingClientRect();
    if (count() === 2) {
      var p = pair();
      if (start.d) zoomAt(p.x - r.left, p.y - r.top, p.d / start.d);
      v.x += p.x - start.x;
      v.y += p.y - start.y;
      apply();
      start = p;
      dragged = true;
      return;
    }
    var dx = e.clientX - start.x;
    var dy = e.clientY - start.y;
    if (!dragged && Math.hypot(dx, dy) < 4) return;
    if (!dragged) {
      dragged = true;
      canvas.setPointerCapture(e.pointerId);
      canvas.classList.add('sk-dragging');
    }
    v.x += dx;
    v.y += dy;
    start = { x: e.clientX, y: e.clientY };
    moved = true;
    apply();
  });
  function release(e) {
    delete pointers[e.pointerId];
    canvas.classList.remove('sk-dragging');
    start = count() === 1 ? (function () {
      var k = Object.keys(pointers)[0];
      return { x: pointers[k].x, y: pointers[k].y };
    })() : null;
  }
  canvas.addEventListener('pointerup', release);
  canvas.addEventListener('pointercancel', release);

  /* an element of this page (#id): centred and flashed */
  function focus(id, smooth) {
    if (!id) return false;
    var s = visibleSvg();
    var el = null;
    var all = s.querySelectorAll('[data-id]');
    for (var i = 0; i < all.length; i++) {
      if (all[i].getAttribute('data-id') === id) { el = all[i]; break; }
    }
    if (!el) return false;
    var b = el.getBBox();
    var r = canvas.getBoundingClientRect();
    var k = Math.max(v.k, Math.min(1, 0.6 * r.width / b.width, 0.6 * r.height / b.height));
    v.k = k;
    v.x = r.width / 2 - (b.x + b.width / 2) * k;
    v.y = r.height / 2 - (b.y + b.height / 2) * k;
    moved = true;
    for (var j = 0; j < stages.length; j++) {
      stages[j].style.transition = smooth && !reduced ? 'transform .3s ease' : '';
    }
    apply();
    setTimeout(function () {
      for (var j = 0; j < stages.length; j++) stages[j].style.transition = '';
    }, 350);
    el.classList.remove('sk-flash');
    void el.getBoundingClientRect();
    el.classList.add('sk-flash');
    canvas.scrollTop = 0;
    canvas.scrollLeft = 0;
    return true;
  }
  function hashId() {
    try { return decodeURIComponent(location.hash.slice(1)); } catch (e) { return ''; }
  }

  /* a click on a linked element: another page zooms into it before it opens */
  canvas.addEventListener('click', function (e) {
    if (dragged) { e.preventDefault(); e.stopPropagation(); dragged = false; return; }
    var a = e.target.closest && e.target.closest('a');
    if (!a) return;
    var raw = a.getAttribute('href') || '';
    if (raw.charAt(0) === '#') {
      e.preventDefault();
      history.pushState(null, '', raw);
      focus(raw.slice(1), true);
      return;
    }
    var url = new URL(a.href || raw, location.href);
    var local = url.origin === location.origin && /\.html$/.test(url.pathname);
    if (!local) {
      if (/^https?:$/.test(url.protocol) && url.origin !== location.origin) {
        e.preventDefault();
        window.open(url.href, '_blank', 'noopener');
      }
      return;
    }
    if (e.ctrlKey || e.metaKey || e.shiftKey || e.button !== 0 || reduced) return;
    e.preventDefault();
    var box = (a.firstElementChild || a).getBoundingClientRect();
    var r = canvas.getBoundingClientRect();
    var cx = box.left + box.width / 2 - r.left;
    var cy = box.top + box.height / 2 - r.top;
    var f = Math.min(6, Math.max(1.6, 0.9 * Math.min(r.width / box.width, r.height / box.height)));
    for (var i = 0; i < stages.length; i++) stages[i].classList.add('sk-leaving');
    v.x = r.width / 2 - (cx - v.x) * f;
    v.y = r.height / 2 - (cy - v.y) * f;
    v.k = v.k * f;
    apply();
    setTimeout(function () { location.href = url.href; }, 300);
  });
  /* back from another page: the bfcache shows the page as it was left */
  window.addEventListener('pageshow', function (e) {
    if (!e.persisted) return;
    for (var i = 0; i < stages.length; i++) stages[i].classList.remove('sk-leaving');
    fit();
  });

  focusHere = function (id) { focus(id, true); };
  window.addEventListener('hashchange', function () { focus(hashId(), true); });
  window.addEventListener('resize', function () { if (!moved) fit(); });
  if (sw) sw.addEventListener('click', function () { if (!moved) fit(); });

  fit();
  if (location.hash) {
    var id = hashId();
    /* after the fonts: the boxes are measured once the text is drawn */
    (document.fonts && document.fonts.ready ? document.fonts.ready : Promise.resolve()).then(function () {
      focus(id, false);
    });
  }
})();
