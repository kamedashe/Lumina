/** @type {import('tailwindcss').Config} */
export default {
    content: [
        "./index.html",
        "./App.tsx",
        "./index.tsx",
        "./src/**/*.{js,ts,jsx,tsx}",
    ],
    theme: {
        extend: {
            colors: {
                // Значения берутся из CSS-переменных → работает и в светлой, и в тёмной теме.
                bg: 'rgb(var(--bg) / <alpha-value>)',
                surface: 'rgb(var(--surface) / <alpha-value>)',
                elevated: 'rgb(var(--elevated) / <alpha-value>)',
                border: 'rgb(var(--border) / <alpha-value>)',
                fg: 'rgb(var(--fg) / <alpha-value>)',
                muted: 'rgb(var(--muted) / <alpha-value>)',
                subtle: 'rgb(var(--subtle) / <alpha-value>)',
                accent: 'rgb(var(--accent) / <alpha-value>)',
                'accent-fg': 'rgb(var(--accent-fg) / <alpha-value>)',
            },
            fontFamily: {
                sans: ['Inter', 'system-ui', 'sans-serif'],
                mono: ['"JetBrains Mono"', 'ui-monospace', 'SFMono-Regular', 'monospace'],
            },
            borderRadius: {
                xl: '0.75rem',
            },
        },
    },
    plugins: [],
}
