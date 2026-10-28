// <skemo-view src="diagram.skemo" css="custom.css" step="2" theme="dark"> — a standalone element (F-WASM-3).
// API (F-ANIM-9): el.states() → steps as written; el.goto(step); event "stepchange".
// The element's text content is the source when `src` is absent.
import init, { render } from './pkg/skemo_engine_wasm.js';

const ready = init();
const DIM = 0.28;

class SkemoView extends HTMLElement {
  static observedAttributes = ['src', 'css', 'step', 'theme'];

  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
    this._states = [];
  }

  connectedCallback() { this._load(); }

  attributeChangedCallback(name, old, val) {
    if (old === val || !this.isConnected) return;
    if (name === 'step') this._apply(); else this._load();
  }

  states() { return this._states.slice(); }

  goto(step) { this.setAttribute('step', String(step)); }

  async _load() {
    await ready;
    const get = async u => (await fetch(u)).text();
    const src = this.getAttribute('src') ? await get(this.getAttribute('src')) : this.textContent;
    const css = this.getAttribute('css') ? await get(this.getAttribute('css')) : '';
    const theme = this.getAttribute('theme') || undefined;
    const r = JSON.parse(render(src, css, JSON.stringify({ theme })));
    if (!r.ok) {
      const pre = document.createElement('pre');
      pre.style.cssText = 'color:#b00;font:12px monospace;white-space:pre-wrap';
      pre.textContent = (r.diagnostics || []).map(d => d.text).join('\n') || r.error || 'erreur';
      this.shadowRoot.replaceChildren(pre);
      return;
    }
    this.shadowRoot.innerHTML = `<style>:host{display:block}svg{max-width:100%;height:auto}[data-step],[data-until]{transition:opacity .45s ease}</style>${r.svg}`;
    this._states = r.layout.layout.states || [];
    this._apply();
  }

  _apply() {
    const svg = this.shadowRoot.querySelector('svg');
    if (!svg) return;
    const raw = this.getAttribute('step');
    // sans `step` : tout afficher
    const s = raw === null || raw === '' ? Infinity : parseFloat(raw);
    const visible = e => {
      const a = e.dataset.step ? parseFloat(e.dataset.step) : -Infinity;
      const b = e.dataset.until ? parseFloat(e.dataset.until) : Infinity;
      return s === Infinity ? !e.dataset.until : a <= s && s <= b;
    };
    const focused = e => (e.dataset.focus || '').split(' ').filter(Boolean).some(r => {
      const [a, b] = r.split('-').map(parseFloat);
      return a <= s && s <= (isNaN(b) ? a : b);
    });
    const all = [...svg.querySelectorAll('[data-step],[data-until],[data-focus]')];
    for (const e of all) e.style.opacity = visible(e) ? '' : '0';
    // emphasis: dim the highest group that is neither in focus, nor parent nor child of an element in focus
    const groups = [...svg.querySelectorAll('g[data-id]')];
    const foc = groups.filter(g => g.dataset.focus && focused(g) && visible(g));
    for (const g of groups) if (g.style.opacity !== '0') g.style.opacity = '';
    if (foc.length) {
      for (const g of groups) {
        if (g.style.opacity === '0') continue;
        const related = foc.some(f => f === g || f.contains(g) || g.contains(f));
        const parent = g.parentElement && g.parentElement.closest('g[data-id]');
        const parentDim = parent && parent.style.opacity === String(DIM);
        if (!related && !parentDim) g.style.opacity = String(DIM);
      }
    }
    const idx = this._states.findIndex(x => parseFloat(x) === s);
    this.dispatchEvent(new CustomEvent('stepchange', { bubbles: true, detail: { step: raw, index: idx, count: this._states.length } }));
  }
}

customElements.define('skemo-view', SkemoView);
