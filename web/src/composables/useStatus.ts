import { computed, ref } from 'vue'
import { api } from '@/api/client'
import type { ApodSummary, ContactConfig, NotifyConfig, PublishSchedule } from '@/api/types'

const latest = ref<ApodSummary | null>(null)
const entries = ref(0)
const publish = ref<PublishSchedule | null>(null)
const contact = ref<ContactConfig | null>(null)
const notify = ref<NotifyConfig | null>(null)

let loaded = false
let inFlight: Promise<void> | null = null

function load(): Promise<void> {
  if (inFlight) return inFlight

  inFlight = api
    .status()
    .then((status) => {
      latest.value = status.latest
      entries.value = status.entries
      publish.value = status.publish
      contact.value = status.contact
      notify.value = status.notify
      loaded = true
    })
    .catch(() => {})
    .finally(() => {
      inFlight = null
    })

  return inFlight
}

export function useStatus() {
  if (!loaded) void load()

  return {
    latest,
    entries,
    publish,
    contact,
    notify,
    /** False until the status response has arrived, so /contact can wait rather than flicker. */
    loaded: computed(() => contact.value !== null),
    latestDate: computed(() => latest.value?.date ?? null),
    refresh: () => {
      loaded = false
      return load()
    },
  }
}

export function useLatestDate() {
  return useStatus().latestDate
}
