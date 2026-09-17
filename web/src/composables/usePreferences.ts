import { computed, ref, watch } from 'vue'

export const WEEK_START_KEY = 'apod:week-start'
export const ARCHIVE_VIEW_KEY = 'apod:archive-view'
export const INDEX_MARKS_KEY = 'apod:index-marks'
export const ENCORE_RAIL_KEY = 'apod:encore-rail'
export const MODERNIZATION_RAIL_KEY = 'apod:modernization-rail'
export const HEMISPHERE_KEY = 'apod:hemisphere'

export type WeekStart = 'monday' | 'sunday'
export type ArchiveView = 'grid' | 'calendar'
export type Hemisphere = 'north' | 'south'

export const WEEK_STARTS: { label: string; value: WeekStart }[] = [
  { label: 'Monday', value: 'monday' },
  { label: 'Sunday', value: 'sunday' },
]

export const HEMISPHERES: { label: string; value: Hemisphere }[] = [
  { label: 'Northern', value: 'north' },
  { label: 'Southern', value: 'south' },
]

function loadHemisphere(): Hemisphere {
  return localStorage.getItem(HEMISPHERE_KEY) === 'south' ? 'south' : 'north'
}

function loadWeekStart(): WeekStart {
  return localStorage.getItem(WEEK_START_KEY) === 'sunday' ? 'sunday' : 'monday'
}

function loadArchiveView(): ArchiveView {
  return localStorage.getItem(ARCHIVE_VIEW_KEY) === 'calendar' ? 'calendar' : 'grid'
}

function loadSwitch(key: string): boolean {
  return localStorage.getItem(key) !== 'off'
}

const weekStart = ref<WeekStart>(loadWeekStart())
const archiveView = ref<ArchiveView>(loadArchiveView())
const indexMarks = ref(loadSwitch(INDEX_MARKS_KEY))
const encoreRail = ref(loadSwitch(ENCORE_RAIL_KEY))
const modernizationRail = ref(loadSwitch(MODERNIZATION_RAIL_KEY))
const hemisphere = ref<Hemisphere>(loadHemisphere())

function persist(key: string, value: string): void {
  try {
    localStorage.setItem(key, value)
  } catch {}
}

watch(weekStart, (value) => persist(WEEK_START_KEY, value))
watch(archiveView, (value) => persist(ARCHIVE_VIEW_KEY, value))
watch(indexMarks, (value) => persist(INDEX_MARKS_KEY, value ? 'on' : 'off'))
watch(encoreRail, (value) => persist(ENCORE_RAIL_KEY, value ? 'on' : 'off'))
watch(modernizationRail, (value) => persist(MODERNIZATION_RAIL_KEY, value ? 'on' : 'off'))
watch(hemisphere, (value) => persist(HEMISPHERE_KEY, value))

export function hydratePreferences(): void {
  weekStart.value = loadWeekStart()
  archiveView.value = loadArchiveView()
  indexMarks.value = loadSwitch(INDEX_MARKS_KEY)
  encoreRail.value = loadSwitch(ENCORE_RAIL_KEY)
  modernizationRail.value = loadSwitch(MODERNIZATION_RAIL_KEY)
  hemisphere.value = loadHemisphere()
}

export function usePreferences() {
  return {
    weekStart,
    weekStartsOn: computed(() => (weekStart.value === 'sunday' ? 0 : 1)),
    archiveView,
    indexMarks,
    encoreRail,
    modernizationRail,
    hemisphere,
  }
}
