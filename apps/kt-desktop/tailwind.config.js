/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        // Zen Brutalism Design System
        // Ground · 地 (chi) — earth
        bg: {
          void: '#15121a',
          deep: '#1a161e',
          base: '#201b24',
          surface: '#282230',
          elevated: '#322b3a',
          hover: '#3a3244',
        },
        // Mauve · 光 (hikari) — light through the slit
        mauve: {
          deep: '#5e4757',
          DEFAULT: '#9b7489',
          mid: '#b8909f',
          light: '#d4b0bf',
        },
        // Earth Semantics · 自然 (shizen) — nature
        sage: {
          dim: '#637556',
          DEFAULT: '#8a9e78',
        },
        ochre: {
          dim: '#9a7550',
          DEFAULT: '#c4976a',
        },
        terracotta: {
          dim: '#8a5555',
          DEFAULT: '#b56e6e',
        },
        // Text · 和紙 (washi) — handmade paper
        text: {
          primary: '#ddd5d9',
          secondary: '#9a8e92',
          muted: '#665c60',
          ghost: '#433b3f',
          whisper: '#342e32',
        },
        // Borders · Formwork lines
        border: {
          faint: '#2e2734',
          DEFAULT: '#3a3240',
          strong: '#4a4054',
        },
      },
      fontFamily: {
        mono: [
          'JetBrains Mono',
          'Fira Code',
          'Monaco',
          'Consolas',
          'monospace',
        ],
        sans: [
          'DM Sans',
          'system-ui',
          '-apple-system',
          'BlinkMacSystemFont',
          'sans-serif',
        ],
      },
      borderRadius: {
        'zen': '3px',
        'zen-lg': '4px',
      },
      spacing: {
        'zen-xs': '6px',
        'zen-sm': '12px',
        'zen-md': '20px',
        'zen-lg': '32px',
        'zen-xl': '48px',
      },
      animation: {
        'breathe': 'breathe 3s ease-in-out infinite',
      },
      keyframes: {
        breathe: {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.35' },
        },
      },
    },
  },
  plugins: [],
};
