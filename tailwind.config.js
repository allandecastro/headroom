/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}', './tools/screenshots/**/*.{ts,tsx,html}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // State colors — see DESIGN_SYSTEM.md
        'state-ok': {
          fill: '#6a8e4a',
          text: '#5a7d3a',
          'fill-dark': '#8ab368',
          'text-dark': '#9bc176',
        },
        'state-warn': {
          fill: '#c08a2a',
          text: '#a87420',
          'fill-dark': '#d99c52',
          'text-dark': '#e0a85e',
        },
        'state-crit': {
          fill: '#b03533',
          text: '#9c2e2c',
          'fill-dark': '#d4625d',
          'text-dark': '#e07670',
        },
      },
      fontFamily: {
        sans: [
          'ui-sans-serif',
          'system-ui',
          '-apple-system',
          'Segoe UI Variable',
          'Segoe UI',
          'Roboto',
          'sans-serif',
        ],
        mono: ['ui-monospace', 'SF Mono', 'Menlo', 'Cascadia Code', 'monospace'],
      },
      fontSize: {
        // Custom small sizes for the dense popover layout
        '2xs': ['9.5px', { lineHeight: '1.4' }],
        xxs: ['10.5px', { lineHeight: '1.4' }],
      },
      borderWidth: {
        hairline: '0.5px',
      },
    },
  },
  plugins: [],
};
