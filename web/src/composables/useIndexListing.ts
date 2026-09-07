import { ref, type Ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useAsync } from '@/composables/useAsync'

const DEBOUNCE_MS = 250

export function useIndexListing<T>(options: {
  routeName: string
  query: () => Record<string, string | undefined>
  fetch: (offset: number, signal: AbortSignal) => Promise<T>
  pageSize: number
}) {
  const route = useRoute()
  const router = useRouter()

  const page = ref(Number.parseInt(String(route.query.page ?? '1'), 10) || 1)

  const listing = useAsync((signal) => options.fetch((page.value - 1) * options.pageSize, signal))

  let debounce: ReturnType<typeof setTimeout> | undefined

  function load(): void {
    void router.replace({
      name: options.routeName,
      query: {
        ...options.query(),
        page: page.value === 1 ? undefined : String(page.value),
      },
    })
    void listing.run()
  }

  function search(resetPage: boolean): void {
    if (resetPage) page.value = 1

    clearTimeout(debounce)
    debounce = setTimeout(load, DEBOUNCE_MS)
  }

  function onPage(event: { page: number }): void {
    page.value = event.page + 1
    clearTimeout(debounce)
    load()
    window.scrollTo({ top: 0, behavior: 'smooth' })
  }

  return {
    ...listing,
    page: page as Ref<number>,
    first: () => (page.value - 1) * options.pageSize,
    search,
    onPage,
  }
}
