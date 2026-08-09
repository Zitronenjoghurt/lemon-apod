import { ref } from 'vue'

export const WELCOME_KEY = 'apod:welcome'

const dismissed = ref(load())

function load(): boolean {
  return localStorage.getItem(WELCOME_KEY) === 'dismissed'
}

function persist(): void {
  try {
    if (dismissed.value) localStorage.setItem(WELCOME_KEY, 'dismissed')
    else localStorage.removeItem(WELCOME_KEY)
  } catch {}
}

export function hydrateWelcome(): void {
  dismissed.value = load()
}

export function useWelcome() {
  return {
    dismissed,
    dismiss: () => {
      dismissed.value = true
      persist()
    },
    reset: () => {
      dismissed.value = false
      persist()
    },
  }
}
