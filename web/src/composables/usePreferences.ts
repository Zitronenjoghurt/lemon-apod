import { computed, ref, watch } from 'vue'

export const WEEK_START_KEY = 'apod:week-start'
export const ARCHIVE_VIEW_KEY = 'apod:archive-view'

export type WeekStart = 'monday' | 'sunday'
export type ArchiveView = 'grid' | 'calendar'

export const WEEK_STARTS: { label: string; value: WeekStart }[] = [
  { label: 'Monday', value: 'monday' },
  { label: 'Sunday', value: 'sunday' },
]

function loadWeekStart(): WeekStart {
  return localStorage.getItem(WEEK_START_KEY) === 'sunday' ? 'sunday' : 'monday'
}

function loadArchiveView(): ArchiveView {
  return localStorage.getItem(ARCHIVE_VIEW_KEY) === 'calendar' ? 'calendar' : 'grid'
}

const weekStart = ref<WeekStart>(loadWeekStart())
const archiveView = ref<ArchiveView>(loadArchiveView())

function persist(key: string, value: string): void {
  try {
    localStorage.setItem(key, value)
  } catch {}
}

watch(weekStart, (value) => persist(WEEK_START_KEY, value))
watch(archiveView, (value) => persist(ARCHIVE_VIEW_KEY, value))

export function hydratePreferences(): void {
  weekStart.value = loadWeekStart()
  archiveView.value = loadArchiveView()
}

export function usePreferences() {
  return {
    weekStart,
    weekStartsOn: computed(() => (weekStart.value === 'sunday' ? 0 : 1)),
    archiveView,
  }
}
