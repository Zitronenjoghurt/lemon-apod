import { computed, ref } from 'vue'

const NARROW = '(max-width: 30rem)'
const ROOMY = '(min-width: 38rem)'

function watched(query: string) {
  const media = window.matchMedia(query)
  const matches = ref(media.matches)

  media.addEventListener('change', (event) => (matches.value = event.matches))
  return matches
}

const narrow = watched(NARROW)
const roomy = watched(ROOMY)

export function useNarrow() {
  return {
    narrow,
    roomy,
    pageLinks: computed(() => (narrow.value ? 3 : 5)),
  }
}
