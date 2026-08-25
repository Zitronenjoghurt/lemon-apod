import type { ChartBand, ChartMark, Tone } from '@/components/SeriesChart.vue'
import type { NoticeKind, WeatherAlert, WeatherBand, WeatherLevel } from '@/api/types'

const ALERT_LIFE_MS = 3 * 3_600_000

export function inForce(alert: WeatherAlert, now = Date.now()): boolean {
  if (alert.notice !== 'alert' && alert.notice !== 'warning') return false
  if (alert.valid_until) return Date.parse(alert.valid_until) >= now
  return now - Date.parse(alert.issued_at) <= ALERT_LIFE_MS
}

export const SWPC_URL = 'https://www.swpc.noaa.gov'
export const SCALES_URL = 'https://www.swpc.noaa.gov/noaa-scales-explanation'
export const KYOTO_URL = 'https://wdc.kugi.kyoto-u.ac.jp/dstdir/'

export const BANDS: WeatherBand[] = ['g', 's', 'r']

export const BAND_NAMES: Record<WeatherBand, string> = {
  g: 'Geomagnetic storms',
  s: 'Solar radiation',
  r: 'Radio blackouts',
}

export const BAND_ABOUT: Record<WeatherBand, string> = {
  g: 'The magnetic field shaken by the solar wind, which influences power grids and brings auroras closer to the equator.',
  s: 'Energetic protons streaming past Earth, which reach satellites and polar flights.',
  r: 'Flares soaking the daylit side of Earth in x-rays, which absorb high frequency radio.',
}

export const SCALE_WORDS = ['none', 'minor', 'moderate', 'strong', 'severe', 'extreme']

export function levelName(level: WeatherLevel): string {
  if (level.scale === null) return levelOdds(level).length ? '' : 'not forecast'
  return level.scale === 0 ? 'none' : `${level.band.toUpperCase()}${level.scale}`
}

export function levelOdds(level: WeatherLevel): { chance: number; of: string }[] {
  const chance = level.chance
  if (!chance) return []

  const tiers =
    level.band === 'r'
      ? [
          { chance: chance.minor, of: 'R1 to R2' },
          { chance: chance.major, of: 'R3 or more' },
        ]
      : [{ chance: chance.minor, of: `${level.band.toUpperCase()}1 or more` }]

  return tiers.filter((tier): tier is { chance: number; of: string } => tier.chance !== null)
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
  alert: 'pi-exclamation-triangle',
  warning: 'pi-bell',
  watch: 'pi-eye',
  summary: 'pi-file',
}

export const KP_SCALE: { at: number; label: string; note: string }[] = [
  {
    at: 9,
    label: 'Extreme storm (G5)',
    note: 'The aurora may be visible as far south as 40 degrees geomagnetic latitude.',
  },
  {
    at: 8,
    label: 'Severe storm (G4)',
    note: 'The aurora may be visible as far south as 45 degrees geomagnetic latitude.',
  },
  {
    at: 7,
    label: 'Strong storm (G3)',
    note: 'The aurora may be visible as far south as 50 degrees geomagnetic latitude.',
  },
  {
    at: 6,
    label: 'Moderate storm (G2)',
    note: 'The aurora may be visible as far south as 55 degrees geomagnetic latitude.',
  },
  {
    at: 5,
    label: 'Minor storm (G1)',
    note: 'The aurora may be visible as far south as 60 degrees geomagnetic latitude.',
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

export function kpTone(kp: number): Tone {
  if (kp >= 8) return 'severe'
  if (kp >= 7) return 'alert'
  if (kp >= 5) return 'warn'
  if (kp >= 4) return 'raised'
  return 'calm'
}

export const KP_FRAME = { min: 0, max: 9 }
export const KP_TICKS = [5, 9]
export const KP_BANDS: ChartBand[] = [
  {
    from: 7,
    to: 9,
    tone: 'alert',
    label: 'severe',
    range: 'Kp 7 to 9, G3 to G5',
    effect:
      'Aurora as low as Illinois, or Florida at the extreme. Grid voltage problems up to transformer damage, shortwave signals become patchy or out for up to two days, satellite navigation degraded for hours to days.',
  },
  {
    from: 5,
    to: 7,
    tone: 'warn',
    label: 'storm',
    range: 'Kp 5 and 6, G1 to G2',
    effect:
      'Aurora as low as New York or Idaho. Weak grid voltage swings and shortwave signals fading at high latitudes.',
  },
  {
    from: 0,
    to: 5,
    tone: 'calm',
    label: 'quiet',
    range: 'Kp below 5',
    effect: 'Aurora stays at high latitudes. Nothing affected on the ground.',
  },
]

export const FLUX_FRAME = { min: 60 }
export const FLUX_TICKS = [100, 200, 300]
export const FLUX_MARKS: ChartMark[] = [
  {
    at: 250,
    label: 'major',
    tone: 'alert',
    range: 'above 250 sfu',
    effect:
      'Large active solar regions facing Earth with a severely increased flare and blackout risk.',
  },
  {
    at: 150,
    label: 'active',
    tone: 'warn',
    range: 'around 150 to 200 sfu',
    effect:
      'Ordinary solar maximum with an increased risk for solar flares. The upper atmosphere swells and drags low satellites down faster.',
  },
  {
    at: 70,
    label: 'quiet',
    tone: 'calm',
    range: 'around 70 sfu',
    effect:
      "Few sunspots, little flaring and minimal effect on Earth's atmosphere. Low satellites hold their altitude longer.",
  },
]

export const DST_FRAME = { min: -120, max: 50 }
export const DST_TICKS = [-50, -100]
export const DST_BANDS: ChartBand[] = [
  {
    from: 20,
    to: 400,
    tone: 'raised',
    label: 'compression',
    range: 'above +20 nT',
    effect:
      'A shock front squeezing the magnetosphere from the outside, which strengthens the field instead of weakening it. Lasts an hour or two. Often runs just ahead of a storm.',
  },
  { from: -50, to: 20, tone: 'calm' },
  {
    from: -100,
    to: -50,
    tone: 'warn',
    label: 'moderate storm',
    range: '-50 to -100 nT',
    effect: 'Aurora moves off the poles, GPS accuracy starts to wander.',
  },
  {
    from: -400,
    to: -100,
    tone: 'alert',
    label: 'intense storm',
    range: 'below -100 nT',
    effect:
      'Currents induced in power lines, pipelines and undersea cables. More satellite faults, aurora at mid latitudes.',
  },
]
export const DST_MARKS: ChartMark[] = [
  {
    at: 0,
    label: 'baseline',
    tone: 'calm',
    range: 'zero',
    effect: 'A quiet day rests just below this: the ring current never empties completely.',
  },
]
