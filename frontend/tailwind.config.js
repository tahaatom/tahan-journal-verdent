/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      fontFamily: {
        sans: ["Vazirmatn", "Tahoma", "sans-serif"],
      },
      colors: {
        surface: "var(--color-surface)",
        "surface-alt": "var(--color-surface-alt)",
        "surface-raised": "var(--color-surface-raised)",
        border: "var(--color-border)",
        accent: "var(--color-accent)",
        "accent-soft": "var(--color-accent-soft)",
        "text-main": "var(--color-text-main)",
        "text-muted": "var(--color-text-muted)",
        danger: "var(--color-danger)",
        success: "var(--color-success)",
      },
      borderRadius: {
        DEFAULT: "0.5rem",
        lg: "0.75rem",
      },
    },
  },
  plugins: [],
};
