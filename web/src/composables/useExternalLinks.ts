import { ref } from 'vue'

export const EXTERNAL_WARNING_KEY = 'apod:external-warning'

const OURS = ['apod.nasa.gov', 'antwrp.gsfc.nasa.gov']

const acknowledged = ref(load())
const pending = ref<string | null>(null)

function load(): boolean {
  return localStorage.getItem(EXTERNAL_WARNING_KEY) === 'acknowledged'
}

function persist(): void {
  try {
    if (acknowledged.value) localStorage.setItem(EXTERNAL_WARNING_KEY, 'acknowledged')
    else localStorage.removeItem(EXTERNAL_WARNING_KEY)
  } catch {}
}

export function hydrateExternalLinks(): void {
  acknowledged.value = load()
}

function isOurs(host: string): boolean {
  return OURS.some((ours) => host === ours || host.endsWith(`.${ours}`))
}

function isUnaudited(href: string): boolean {
  let url: URL
  try {
    url = new URL(href, location.href)
  } catch {
    return false
  }

  if (url.protocol !== 'http:' && url.protocol !== 'https:') return false
  if (url.origin === location.origin) return false

  return !isOurs(url.hostname)
}

export function useExternalLinks() {
  function intercept(event: MouseEvent): void {
    if (acknowledged.value) return
    if (event.defaultPrevented || event.button !== 0) return
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return

    const anchor = (event.target as HTMLElement | null)?.closest?.('a')
    if (!anchor || anchor.hasAttribute('data-ours')) return

    const href = anchor.getAttribute('href')
    if (!href || !isUnaudited(href)) return

    event.preventDefault()
    event.stopPropagation()
    pending.value = new URL(href, location.href).href
  }

  function follow(remember: boolean): void {
    const url = pending.value
    pending.value = null
    if (!url) return

    if (remember) {
      acknowledged.value = true
      persist()
    }

    window.open(url, '_blank', 'noopener,noreferrer')
  }

  function dismiss(): void {
    pending.value = null
  }

  function reset(): void {
    acknowledged.value = false
    persist()
  }

  return { acknowledged, pending, intercept, follow, dismiss, reset }
}
