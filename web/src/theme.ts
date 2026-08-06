import { definePreset } from '@primeuix/themes'
import Aura from '@primeuix/themes/aura'

export const ApodPreset = definePreset(Aura, {
  semantic: {
    primary: {
      50: '#eef2ff',
      100: '#e0e7ff',
      200: '#c7d2fe',
      300: '#a5b4fc',
      400: '#818cf8',
      500: '#6366f1',
      600: '#4f46e5',
      700: '#4338ca',
      800: '#3730a3',
      900: '#312e81',
      950: '#1e1b4b',
    },
    colorScheme: {
      light: {
        surface: {
          0: '#ffffff',
          50: '#f8fafc',
          100: '#f1f5f9',
          200: '#e2e8f0',
          300: '#cbd5e1',
          400: '#94a3b8',
          500: '#64748b',
          600: '#475569',
          700: '#334155',
          800: '#1e293b',
          900: '#0f172a',
          950: '#020617',
        },
      },
      dark: {
        surface: {
          0: '#ffffff',
          50: '#e6e8f0',
          100: '#c3c7d9',
          200: '#9ba1bd',
          300: '#6f7797',
          400: '#4d5473',
          500: '#343a54',
          600: '#262b41',
          700: '#1c2033',
          800: '#141726',
          900: '#0d0f1a',
          950: '#070810',
        },
      },
    },
  },
  components: {
    button: { root: { borderRadius: '0.6rem' } },
    inputtext: { root: { borderRadius: '0.6rem' } },
  },
})
