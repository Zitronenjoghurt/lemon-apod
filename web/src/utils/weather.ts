import type { NoticeKind, WeatherAlert, WeatherBand, WeatherLevel } from '@/api/types'

const ALERT_LIFE_MS = 3 * 3_600_000

export function inForce(alert: WeatherAlert, now = Date.now()): boolean {
  if (alert.notice !== 'alert' && alert.notice !== 'warning') return false
  if (alert.valid_until) return Date.parse(alert.valid_until) >= now
  return now - Date.parse(alert.issued_at) <= ALERT_LIFE_MS
}

export const SWPC_URL = 'https://www.swpc.noaa.gov/products/planetary-k-index'
export const SCALES_URL = 'https://www.swpc.noaa.gov/noaa-scales-explanation'

export const BANDS: WeatherBand[] = ['g', 's', 'r']

export const BAND_NAMES: Record<WeatherBand, string> = {
  g: 'Geomagnetic storms',
  s: 'Solar radiation',
  r: 'Radio blackouts',
}

export const BAND_ABOUT: Record<WeatherBand, string> = {
  g: 'The magnetic field shaken by the solar wind, which influences power grids and brings auroras closer to the equator.',
  s: 'Energetic protons streaming past Earth, which reaches satellites and polar flights.',
  r: 'Flares soaking the sunlit side of Earth in x-rays, which drowns out high frequency radio.',
}

export const SCALE_WORDS = ['none', 'minor', 'moderate', 'strong', 'severe', 'extreme']

export function levelName(level: WeatherLevel): string {
  if (level.scale === null) return 'not forecast'
  return level.scale === 0 ? 'none' : `${level.band.toUpperCase()}${level.scale}`
}

export function levelWord(level: WeatherLevel): string {
  return level.text ?? SCALE_WORDS[level.scale ?? 0] ?? 'none'
}

export const NOTICE_LABELS: Record<NoticeKind, string> = {
  alert: 'Alert',
  warning: 'Warning',
  watch: 'Watch',
  summary: 'Summary',
}

export const NOTICE_ICONS: Record<NoticeKind, string> = {
  alert: 'pi pi-exclamation-triangle',
  warning: 'pi pi-bell',
  watch: 'pi pi-eye',
  summary: 'pi pi-file',
}

export const KP_SCALE: { at: number; label: string; note: string }[] = [
  {
    at: 9,
    label: 'Extreme storm (G5)',
    note: 'The aurora may be visible as far south as 40 degrees latitude.',
  },
  {
    at: 8,
    label: 'Severe storm (G4)',
    note: 'The aurora may be visible as far south as 45 degrees latitude.',
  },
  {
    at: 7,
    label: 'Strong storm (G3)',
    note: 'The aurora may be visible as far south as 50 degrees latitude.',
  },
  {
    at: 6,
    label: 'Moderate storm (G2)',
    note: 'The aurora may be visible as far south as 55 degrees latitude.',
  },
  {
    at: 5,
    label: 'Minor storm (G1)',
    note: 'The aurora may be visible as far south as 60 degrees latitude.',
  },
  {
    at: 4,
    label: 'Unsettled',
    note: 'Busier than usual, but not a storm. The aurora stays near the poles.',
  },
  { at: 0, label: 'Quiet', note: 'A normal day. The aurora stays near the poles.' },
]

export function kpReading(kp: number): { at: number; label: string; note: string } {
  return KP_SCALE.find((step) => kp >= step.at) ?? KP_SCALE[KP_SCALE.length - 1]
}

export function kpPercent(kp: number): number {
  return Math.min(100, Math.max(0, (kp / 9) * 100))
}
