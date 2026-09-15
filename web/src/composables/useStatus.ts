import { computed, ref } from 'vue'
import { api } from '@/api/client'
import type {
  ApodSummary,
  ContactConfig,
  DiscordConfig,
  NotifyConfig,
  PublishPause,
  PublishSchedule,
} from '@/api/types'

const latest = ref<ApodSummary | null>(null)
const entries = ref(0)
const publish = ref<PublishSchedule | null>(null)
const pause = ref<PublishPause | null>(null)
const contact = ref<ContactConfig | null>(null)
const notify = ref<NotifyConfig | null>(null)
const discord = ref<DiscordConfig | null>(null)

const arrived = ref(false)
let inFlight: Promise<void> | null = null

function load(): Promise<void> {
  if (inFlight) return inFlight

  inFlight = api
    .status()
    .then((status) => {
      latest.value = status.latest
      entries.value = status.entries
      publish.value = status.publish
      pause.value = status.pause ?? null
      contact.value = status.contact
      notify.value = status.notify
      discord.value = status.discord
      arrived.value = true
    })
    .catch(() => {})
    .finally(() => {
      inFlight = null
    })

  return inFlight
}

export function useStatus() {
  if (!arrived.value) void load()

  return {
    latest,
    entries,
    publish,
    pause,
    paused: computed(() => (date: string) => held(pause.value, publish.value, date)),
    pauseRunning: computed(
      () => publish.value !== null && covers(pause.value, publish.value.today),
    ),
    contact,
    notify,
    discord,
    botInvite: computed(() => discord.value?.invite_url ?? null),
    botUserInstall: computed(() => discord.value?.user_install_url ?? null),
    botNumbers: computed(() => discord.value?.numbers ?? null),
    loaded: computed(() => arrived.value),
    latestDate: computed(() => latest.value?.date ?? null),
    refresh: load,
  }
}

export function useLatestDate() {
  return useStatus().latestDate
}

function covers(pause: PublishPause | null, date: string): boolean {
  if (!pause) return false
  return date >= pause.start && (pause.end === null || date <= pause.end)
}

function held(pause: PublishPause | null, publish: PublishSchedule | null, date: string): boolean {
  if (!publish || date > publish.today) return false
  return covers(pause, date)
}
