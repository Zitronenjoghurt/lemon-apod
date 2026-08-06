import { ref, watchEffect } from 'vue'

export type Theme = 'dark' | 'light' | 'auto'

const STORAGE_KEY = 'apod:theme'

function stored(): Theme {
  const raw = localStorage.getItem(STORAGE_KEY)
  return raw === 'dark' || raw === 'light' || raw === 'auto' ? raw : 'auto'
}

const theme = ref<Theme>(stored())
const prefersDark = window.matchMedia('(prefers-color-scheme: dark)')

function apply(): void {
  const dark = theme.value === 'dark' || (theme.value === 'auto' && prefersDark.matches)
  document.documentElement.classList.toggle('app-dark', dark)
  document.documentElement.dataset.theme = dark ? 'dark' : 'light'
}

prefersDark.addEventListener('change', apply)
watchEffect(() => {
  localStorage.setItem(STORAGE_KEY, theme.value)
  apply()
})

export function useTheme() {
  function cycle(): void {
    theme.value = theme.value === 'auto' ? 'dark' : theme.value === 'dark' ? 'light' : 'auto'
  }

  return { theme, cycle }
}
