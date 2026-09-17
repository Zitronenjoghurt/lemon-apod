import { ref, type Ref, watch } from 'vue'
import { api } from '@/api/client'
import type { Contributor, ObjectMention } from '@/api/types'
import { usePreferences } from './usePreferences'

export function useIndexMarks(date: Ref<string | undefined>) {
  const { indexMarks } = usePreferences()
  const contributors = ref<Contributor[]>([])
  const mentions = ref<ObjectMention[]>([])

  async function load() {
    const at = date.value
    contributors.value = []
    mentions.value = []
    if (!at || !indexMarks.value) return

    const [credited, named] = await Promise.allSettled([api.creditsFor(at), api.objectsFor(at)])
    if (date.value !== at) return

    contributors.value = credited.status === 'fulfilled' ? credited.value : []
    mentions.value = named.status === 'fulfilled' ? named.value : []
  }

  watch([date, indexMarks], load, { immediate: true })

  return { contributors, mentions }
}
