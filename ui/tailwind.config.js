/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  theme: {
    extend: {
      colors: {
        'neon-green': '#00ff41',
        'cyber-black': '#050505'
      },
      fontFamily: {
        mono: ['"Fira Code"', 'monospace']
      }
    },
  },
  plugins: [],
}
