/** @type {import('tailwindcss').Config} */
export default {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx,js,jsx}"],
  theme: {
    extend: {
      colors: {
        // ── Surface ───────────────────────────────────────────────
        'surface-base':    'var(--surface-base)',
        'surface-layer':   'var(--surface-layer)',
        'surface-card':    'var(--surface-card)',
        'surface-subtle':  'var(--surface-subtle)',
        'surface-flyout':  'var(--surface-flyout)',
        'surface-control': 'var(--surface-control)',
        // ── Text ──────────────────────────────────────────────────
        'text-primary':    'var(--text-primary)',
        'text-secondary':  'var(--text-secondary)',
        'text-tertiary':   'var(--text-tertiary)',
        'text-disabled':   'var(--text-disabled)',
        'text-link':       'var(--text-link)',
        'text-on-accent':  'var(--text-on-accent)',
        // ── Accent ────────────────────────────────────────────────
        'accent-default':  'var(--accent-default)',
        'accent-light1':   'var(--accent-light1)',
        'accent-light2':   'var(--accent-light2)',
        'accent-dark1':    'var(--accent-dark1)',
        // ── Stroke ────────────────────────────────────────────────
        'stroke-card':      'var(--stroke-card)',
        'stroke-control':   'var(--stroke-control)',
        'stroke-divider':   'var(--stroke-divider)',
        'stroke-strong':    'var(--stroke-strong)',
        'stroke-focus':     'var(--stroke-focus)',
        // ── Knowledge ─────────────────────────────────────────────
        'knowledge-data':         'var(--knowledge-data)',
        'knowledge-data-bg':      'var(--knowledge-data-bg)',
        'knowledge-data-text':    'var(--knowledge-data-text)',
        'knowledge-pattern':      'var(--knowledge-pattern)',
        'knowledge-pattern-bg':   'var(--knowledge-pattern-bg)',
        'knowledge-pattern-text': 'var(--knowledge-pattern-text)',
        'knowledge-log':          'var(--knowledge-log)',
        'knowledge-log-bg':       'var(--knowledge-log-bg)',
        'knowledge-log-text':     'var(--knowledge-log-text)',
        // ── Semantic ──────────────────────────────────────────────
        'color-success':      'var(--color-success)',
        'color-success-bg':   'var(--color-success-bg)',
        'color-warning':      'var(--color-warning)',
        'color-warning-bg':   'var(--color-warning-bg)',
        'color-danger':       'var(--color-danger)',
        'color-danger-bg':    'var(--color-danger-bg)',
        'color-danger-hover': 'var(--color-danger-hover)',
      },
      fontFamily: {
        'display': ['Noto Serif TC', 'Georgia', 'serif'],
        'ui':      ['Segoe UI Variable', 'PingFang TC', 'Microsoft JhengHei UI', 'system-ui', 'sans-serif'],
        'mono':    ['JetBrains Mono', 'Cascadia Code', 'Consolas', 'monospace'],
      },
      fontSize: {
        'fs-xs':   ['var(--fs-xs)',   { lineHeight: 'var(--fs-xs-lh)' }],
        'fs-sm':   ['var(--fs-sm)',   { lineHeight: 'var(--fs-sm-lh)' }],
        'fs-base': ['var(--fs-base)', { lineHeight: 'var(--fs-base-lh)' }],
        'fs-md':   ['var(--fs-base)', { lineHeight: 'var(--fs-base-lh)' }],
        'fs-lg':   ['var(--fs-lg)',   { lineHeight: 'var(--fs-lg-lh)' }],
        'fs-xl':   ['var(--fs-xl)',   { lineHeight: 'var(--fs-xl-lh)' }],
        'fs-2xl':  ['var(--fs-2xl)',  { lineHeight: 'var(--fs-2xl-lh)' }],
      },
      boxShadow: {
        'card':       'var(--shadow-card)',
        'card-hover': 'var(--shadow-card-hover)',
        'flyout':     'var(--shadow-flyout)',
        'dialog':     'var(--shadow-dialog)',
      },
      borderRadius: {
        'micro': 'var(--radius-micro)',
        'sm':    'var(--radius-sm)',
        'md':    'var(--radius-md)',
        'lg':    'var(--radius-lg)',
        'xl':    'var(--radius-xl)',
        'full':  'var(--radius-full)',
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
}
