// skemo step player: → space PageDown click = next; ← PageUp = previous;
// ?all = show everything; ?step=N = go to step N (value as written). Event "stepchange".
(() => {
  const st = document.querySelector('.stage');
  if (!st) return;
  const states = JSON.parse(st.dataset.states || '[]').map(s => [s, parseFloat(s)]);
  if (!states.length) return;
  const els = [...st.querySelectorAll('[data-step],[data-until],[data-focus]')];
  const q = new URLSearchParams(location.search);
  const all = q.has('all');
  let i = 0;
  if (q.has('step')) {
    const k = states.findIndex(s => s[0] === q.get('step') || s[1] === parseFloat(q.get('step')));
    if (k >= 0) i = k;
  }
  const vis = (e, s) => {
    const a = e.dataset.step ? parseFloat(e.dataset.step) : 0;
    const b = e.dataset.until ? parseFloat(e.dataset.until) : Infinity;
    return a <= s && s <= b;
  };
  const foc = (e, s) => (e.dataset.focus || '').split(' ').filter(Boolean).some(r => {
    const [a, b] = r.split('-').map(parseFloat);
    return a <= s && s <= (isNaN(b) ? a : b);
  });
  const draw = g => {
    const l = g.querySelector('.line');
    if (!l) return;
    const len = l.getTotalLength ? l.getTotalLength() : 1;
    l.style.transition = 'none'; l.style.strokeDasharray = `${len} ${len}`; l.style.strokeDashoffset = `${len}`;
    l.getBoundingClientRect();
    l.style.transition = 'stroke-dashoffset .6s ease'; l.style.strokeDashoffset = '0';
    l.addEventListener('transitionend', () => { l.style.strokeDasharray = ''; l.style.strokeDashoffset = ''; l.style.transition = ''; }, { once: true });
  };
  const apply = animate => {
    const s = states[i][1];
    let any = false;
    for (const e of els) {
      const v = all || vis(e, s);
      if (v && e.classList.contains('sk-off') && animate && e.dataset.fx === 'draw') draw(e);
      e.classList.toggle('sk-off', !v);
      const f = !all && v && foc(e, s);
      e.classList.toggle('sk-focus-on', f);
      any = any || f;
    }
    st.classList.toggle('sk-focus', any);
    st.dispatchEvent(new CustomEvent('stepchange', { bubbles: true, detail: { index: i, step: states[i][0], count: states.length } }));
  };
  const go = k => { const j = Math.max(0, Math.min(states.length - 1, k)); if (j !== i) { i = j; apply(true); } };
  addEventListener('keydown', ev => {
    if (['ArrowRight', ' ', 'PageDown'].includes(ev.key)) go(i + 1);
    else if (['ArrowLeft', 'PageUp'].includes(ev.key)) go(i - 1);
    else return;
    ev.preventDefault();
  });
  addEventListener('click', () => go(i + 1));
  window.skemo = { states: () => states.map(s => s[0]), goto: step => { const k = states.findIndex(s => s[0] === String(step)); if (k >= 0) go(k); } };
  apply(false);
})();
