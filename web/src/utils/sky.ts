import type { PlanetId, PlanetVisibility, SkyEvent, SkyEventKind, Turning } from '@/api/types'
import type { Hemisphere } from '@/composables/usePreferences'

export const EVENT_ICONS: Record<SkyEventKind, string> = {
  moon: 'moon-phase',
  season: 'sun',
  shower: 'shower',
  eclipse: 'eclipse',
  planet: 'globe',
  conjunction: 'conjunction',
}

export function planetIcon(planet: PlanetId): string {
  return planet
}

export function planetColor(planet: PlanetId): string {
  return `var(--planet-${planet})`
}

export const PLANET_SCALE: Record<PlanetId, number> = {
  jupiter: 1.0,
  saturn: 0.96,
  uranus: 0.88,
  neptune: 0.86,
  venus: 0.76,
  mars: 0.72,
  mercury: 0.68,
}

export function planetScale(planet: PlanetId): number {
  return PLANET_SCALE[planet]
}

export const VISIBILITY: Record<PlanetVisibility, { icon: string; tone: string }> = {
  evening: { icon: 'sunset', tone: 'dusk' },
  morning: { icon: 'sunrise', tone: 'dawn' },
  all_night: { icon: 'moon', tone: 'all-night' },
  lost: { icon: 'sun', tone: 'lost' },
}

export type Season = 'spring' | 'summer' | 'autumn' | 'winter'

export function isSeason(word: string): word is Season {
  return word === 'spring' || word === 'summer' || word === 'autumn' || word === 'winter'
}

export function seasonColor(season: string): string {
  return isSeason(season) ? `var(--season-${season})` : 'var(--accent)'
}

export function seasonOpened(turning: Turning, hemisphere: Hemisphere): string {
  return hemisphere === 'south' ? turning.opens_southern : turning.opens_northern
}

const OPPOSITE: Record<Season, Season> = {
  spring: 'autumn',
  summer: 'winter',
  autumn: 'spring',
  winter: 'summer',
}

function capitalized(word: string): string {
  return word.charAt(0).toUpperCase() + word.slice(1)
}

export function eventDetail(event: SkyEvent, hemisphere: Hemisphere): string | null {
  if (event.kind !== 'season') return event.detail
  const north = seasonOpenedAt(event.at, 'north')
  if (!north) return event.detail
  const south = OPPOSITE[north]
  return hemisphere === 'south'
    ? `${capitalized(south)} begins in the south, ${north} in the north`
    : `${capitalized(north)} begins in the north, ${south} in the south`
}

export function seasonOpenedAt(iso: string, hemisphere: Hemisphere): Season | null {
  const month = new Date(iso).getUTCMonth() + 1
  if (Number.isNaN(month)) return null
  const north: Season =
    month <= 4 ? 'spring' : month <= 7 ? 'summer' : month <= 10 ? 'autumn' : 'winter'
  return hemisphere === 'south' ? OPPOSITE[north] : north
}

export function eventColor(event: SkyEvent, hemisphere: Hemisphere): string {
  if (event.planets?.length === 1) return planetColor(event.planets[0] as PlanetId)
  if (event.kind === 'season') {
    const season = seasonOpenedAt(event.at, hemisphere)
    return season ? seasonColor(season) : 'var(--sun)'
  }
  if (event.kind === 'eclipse') return event.solar ? 'var(--sun)' : 'var(--blood-moon)'
  if (event.kind === 'shower') return 'var(--meteor)'
  return 'var(--moon)'
}

export function seasonProgress(
  began: string,
  ends: string,
  at: string,
): { fraction: number; daysIn: number; daysLeft: number } | null {
  const [from, to, now] = [Date.parse(began), Date.parse(ends), Date.parse(at)]
  if ([from, to, now].some(Number.isNaN) || to <= from) return null

  const day = 86_400_000
  const fraction = Math.min(1, Math.max(0, (now - from) / (to - from)))
  return {
    fraction,
    daysIn: Math.max(0, Math.round((now - from) / day)),
    daysLeft: Math.max(0, Math.round((to - now) / day)),
  }
}
