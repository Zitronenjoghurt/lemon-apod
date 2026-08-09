import { computed, ref } from 'vue'

const NARROW = '(max-width: 30rem)'

const media = window.matchMedia(NARROW)
const narrow = ref(media.matches)

media.addEventListener('change', (event) => (narrow.value = event.matches))

export function useNarrow() {
  return {
    narrow,
    pageLinks: computed(() => (narrow.value ? 3 : 5)),
  }
}
