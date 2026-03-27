/** @type {import('tailwindcss').Config} */
export default {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx,js,jsx}"],
  theme: {
    extend: {
      colors: {
        // ── Backgrounds ──────────────────────────────────────────
        'ic-bg-base': 'rgb(var(--ic-bg-base) / <alpha-value>)',
        'ic-bg-surface': 'rgb(var(--ic-bg-surface) / <alpha-value>)',
        'ic-bg-elevated': 'rgb(var(--ic-bg-elevated) / <alpha-value>)',
        'ic-bg-hover': 'rgb(var(--ic-bg-hover) / <alpha-value>)',
        'ic-bg-active': 'rgb(var(--ic-bg-active) / <alpha-value>)',
        'ic-bg-input': 'rgb(var(--ic-bg-input) / <alpha-value>)',
        // ── Text ─────────────────────────────────────────────────
        'ic-text-primary': 'rgb(var(--ic-text-primary) / <alpha-value>)',
        'ic-text-secondary': 'rgb(var(--ic-text-secondary) / <alpha-value>)',
        'ic-text-muted': 'rgb(var(--ic-text-muted) / <alpha-value>)',
        // ── Border ───────────────────────────────────────────────
        'ic-border': 'rgb(var(--ic-border) / <alpha-value>)',
        'ic-border-strong': 'rgb(var(--ic-border-strong) / <alpha-value>)',
        // ── Accent ───────────────────────────────────────────────
        'ic-accent': 'rgb(var(--ic-accent) / <alpha-value>)',
        'ic-accent-subtle': 'rgb(var(--ic-accent-subtle) / <alpha-value>)',
        'ic-accent-hover': 'rgb(var(--ic-accent-hover) / <alpha-value>)',
        'ic-accent-fg': 'rgb(var(--ic-accent-fg) / <alpha-value>)',
        'ic-accent-active': 'rgb(var(--ic-accent-active) / <alpha-value>)',
        // ── Semantic ─────────────────────────────────────────────
        'ic-info': 'rgb(var(--ic-info) / <alpha-value>)',
        'ic-info-subtle': 'rgb(var(--ic-info-subtle) / <alpha-value>)',
        'ic-destructive': 'rgb(var(--ic-destructive) / <alpha-value>)',
        'ic-destructive-subtle': 'rgb(var(--ic-destructive-subtle) / <alpha-value>)',
        'ic-destructive-fg': 'rgb(var(--ic-destructive-fg) / <alpha-value>)',
        'ic-warning': 'rgb(var(--ic-warning) / <alpha-value>)',
        'ic-warning-subtle': 'rgb(var(--ic-warning-subtle) / <alpha-value>)',
        'ic-success': 'rgb(var(--ic-success) / <alpha-value>)',
        // ── Semantic highlights ───────────────────────────────────
        'ic-mention': 'rgb(var(--ic-mention) / <alpha-value>)',
        'ic-url': 'rgb(var(--ic-url) / <alpha-value>)',
      },
      fontSize: {
        'fs-xs': ['0.75rem', { lineHeight: '1rem' }],
        'fs-sm': ['0.875rem', { lineHeight: '1.25rem' }],
        'fs-base': ['1rem', { lineHeight: '1.5rem' }],
        'fs-md': ['1rem', { lineHeight: '1.5rem' }],
        'fs-lg': ['1.125rem', { lineHeight: '1.75rem' }],
        'fs-xl': ['1.25rem', { lineHeight: '1.75rem' }],
        'fs-2xl': ['1.5rem', { lineHeight: '2rem' }],
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
}
