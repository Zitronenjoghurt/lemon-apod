import { ref, shallowRef, type Ref } from 'vue'
import { ApiError } from '@/api/client'

export function useAsync<T>(loader: (signal: AbortSignal) => Promise<T>) {
  const data = shallowRef<T>()
  const error = ref<string>()
  const notFound = ref(false)
  const loading = ref(false)
  let controller: AbortController | undefined

  async function run(): Promise<void> {
    controller?.abort()
    controller = new AbortController()
    const { signal } = controller

    loading.value = true
    error.value = undefined
    notFound.value = false

    try {
      const result = await loader(signal)
      if (signal.aborted) return
      data.value = result
    } catch (thrown) {
      if (signal.aborted || (thrown instanceof DOMException && thrown.name === 'AbortError')) return

      if (thrown instanceof ApiError && thrown.notFound) {
        notFound.value = true
        data.value = undefined
      } else {
        error.value = thrown instanceof Error ? thrown.message : 'Something went wrong.'
      }
    } finally {
      if (!signal.aborted) loading.value = false
    }
  }

  return { data: data as Ref<T | undefined>, error, notFound, loading, run }
}
