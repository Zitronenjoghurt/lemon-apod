export type MediaKind =
  | 'image_jpg'
  | 'image_png'
  | 'image_gif'
  | 'video_mp4'
  | 'youtube'
  | 'vimeo'
  | 'embed'
  | 'other'
  | 'none'

export interface Media {
  kind: MediaKind
  url: string | null
  hd_url: string | null
  thumb_url?: string | null
  thumb_width?: number | null
  thumb_height?: number | null
}

export interface Credit {
  role: string
  html: string
  text: string
}

export interface ApodEntry {
  date: string
  title: string
  title_raw?: string
  explanation_html: string
  explanation_text: string
  credits?: Credit[]
  has_copyright: boolean
  license_url?: string
  tomorrow_teaser?: string
  keywords?: string[]
  media: Media
  extra_media?: Media[]
  source_url: string
  /** Set when this picture ran on more than one date. The value is the date it first ran. */
  picture?: string
}

export interface ApodSummary {
  date: string
  title: string
  media: Media
  has_copyright: boolean
  /** See {@link ApodEntry.picture}. */
  picture?: string
}

export interface Matched {
  title: boolean
  explanation: boolean
  credit: boolean
  keywords: boolean
}

export interface SearchHit extends ApodSummary {
  snippet: string
  /** Which indexed fields the query hit. Search reaches the credit and keywords too. */
  matched: Matched
  /** The credit with the match marked, present only when that is where the hit was. */
  credit?: string
  /** The keywords with the match marked, on the same terms. */
  keywords?: string
}

export interface Page<T> {
  items: T[]
  next_cursor?: string
}

export interface SearchResults {
  items: SearchHit[]
  total: number
}

export interface Listing<T> {
  items: T[]
  total: number
}

export interface EntryLength {
  date: string
  title: string
  word_count: number
}

export interface TextSummary {
  measured: number
  total_words: number
  distinct_words: number
  avg_words: number
  median_words: number
  p25_words: number
  p75_words: number
  min_words: number
  max_words: number
  avg_unique_words: number
  avg_chars: number
  avg_sentences: number
  avg_words_per_sentence: number
  avg_links: number
  used_once: number
  lengths?: LengthBucket[]
  shortest?: EntryLength
  longest?: EntryLength
}

export interface LengthBucket {
  from: number
  to?: number
  entries: number
}

export interface ResourceSummary {
  resources: number
  hosts: number
  references: number
  referenced_once: number
}

export interface Stats {
  entries: number
  thumbnails: number
  first: string | null
  latest: string | null
  by_media_kind: { kind: MediaKind; count: number }[]
  copyright: number
  licensed: number
  gaps: number
  gap_dates: string[]
  text: TextSummary
  resources: ResourceSummary
  pictures: PictureSummary
}

export interface PictureSummary {
  hashed: number
  pictures: number
  entries: number
  most_shown?: string
  most_shown_times: number
}

export interface YearStats {
  year: number
  entries: number
  measured: number
  total_words: number
  distinct_words: number
  new_words: number
  avg_words: number
  min_words: number
  max_words: number
  avg_sentences: number
  avg_words_per_sentence: number
  avg_links: number
  copyright: number
  images: number
  videos: number
}

export interface Timeline {
  years: YearStats[]
}

export interface PublishSchedule {
  timezone: string
  abbreviation: string
  hour: number
  minute: number
  today: string
  next_at: string
}

export interface ContactConfig {
  form_key: string | null
  email: string | null
}

export interface NotifyConfig {
  base_url: string | null
  apod_topic: string | null
  aurora_topic: string | null
  space_weather_topic: string | null
  sky_topic: string | null
}

export interface Status {
  latest: ApodSummary | null
  entries: number
  publish: PublishSchedule
  contact: ContactConfig
  notify: NotifyConfig
}

export interface MonthCount {
  year: number
  month: number
  entries: number
}

export interface Coverage {
  months: MonthCount[]
}

export interface Resource {
  id: number
  url: string
  key: string
  host: string
  label?: string
  refs: number
  entries: number
  credited: number
  first?: string
  last?: string
}

export interface ResourceRef extends ApodSummary {
  anchor: string
  in_credit: boolean
  count: number
}

export interface AnchorCount {
  anchor: string
  entries: number
}

export interface ResourceRefs {
  resource: Resource
  items: ResourceRef[]
  total: number
  anchors: AnchorCount[]
}

export interface Picture {
  id: string
  title: string
  media: Media
  appearances: number
  first: string
  last: string
  titles: number
  span_days: number
}

export interface Changed {
  title: boolean
  explanation: boolean
  credit: boolean
  file: boolean
}

export interface Appearance extends ApodSummary {
  changed: Changed
  since_previous_days?: number
}

export interface PictureAppearances {
  picture: Picture
  items: Appearance[]
}

export type PictureSort = 'appearances' | 'first' | 'last' | 'span' | 'title'

export interface HostCount {
  host: string
  resources: number
  refs: number
}

export interface Word {
  word: string
  total: number
  entries: number
}

export interface WordUse extends Word {
  first?: string
  last?: string
  by_year: { year: number; total: number; entries: number }[]
  top_entries: { date: string; title: string; count: number }[]
}

export type MoonPhase =
  | 'new'
  | 'waxing_crescent'
  | 'first_quarter'
  | 'waxing_gibbous'
  | 'full'
  | 'waning_gibbous'
  | 'last_quarter'
  | 'waning_crescent'

export interface MoonQuarter {
  quarter: 'new' | 'first' | 'full' | 'last'
  label: string
  at: string
}

export interface Moon {
  phase: MoonPhase
  label: string
  illumination: number
  age_days: number
  waxing: boolean
  distance_km: number
  perigee_km: number
  apogee_km: number
  closing: boolean
  cycle: number
  last_new_moon: string
  next_quarters: MoonQuarter[]
}

export interface Turning {
  turning: 'march_equinox' | 'june_solstice' | 'september_equinox' | 'december_solstice'
  label: string
  at: string
  opens_northern: string
  opens_southern: string
}

export type PlanetVisibility = 'evening' | 'morning' | 'all_night' | 'lost'

export interface PlanetMilestone {
  name: string
  milestone: 'opposition' | 'greatest_eastern_elongation' | 'greatest_western_elongation'
  label: string
  at: string
  elongation: number
}

export interface Planet {
  planet: string
  name: string
  naked_eye: boolean
  visibility: PlanetVisibility
  visibility_label: string
  elongation: number
  magnitude: number
  distance_au: number
  next_milestone: PlanetMilestone | null
}

export interface ShowerPeak {
  name: string
  radiant: string
  parent: string
  zenith_hourly_rate: number
  peak: string
  moon_illumination: number
  moonlight: 'dark' | 'some' | 'washed_out'
  moonlight_label: string
}

export interface EclipseEvent {
  label: string
  solar: boolean
  at: string
  magnitude: number
}

export type SkyEventKind = 'moon' | 'season' | 'shower' | 'eclipse' | 'planet'

export interface SkyEvent {
  kind: SkyEventKind
  title: string
  detail: string | null
  at: string
}

export interface Launch {
  id: string
  name: string
  provider: string | null
  vehicle: string | null
  pad: string | null
  mission: string | null
  orbit: string | null
  status: string | null
  net: string
  window_start: string | null
  window_end: string | null
  precision: string | null
  image_url: string | null
  info_url: string | null
}

export interface SpaceWeather {
  kp: number
  observed_at: string
}

export type WeatherBand = 'r' | 's' | 'g'

export interface WeatherLevel {
  band: WeatherBand
  scale: number | null
  text: string | null
}

export interface ScaleDay {
  date: string
  levels: WeatherLevel[]
}

export interface KpPoint {
  at: string
  kp: number
  ahead: boolean
}

export interface FluxPoint {
  at: string
  flux: number
}

export interface DstPoint {
  at: string
  dst: number
}

export type NoticeKind = 'alert' | 'warning' | 'watch' | 'summary'

export interface WeatherAlert {
  id: string
  notice: NoticeKind
  headline: string
  scale: string | null
  issued_at: string
  valid_until: string | null
  message: string
}

export interface WeatherSummary {
  kp: number
  observed_at: string
  scales: ScaleDay | null
  alert: WeatherAlert | null
  active: number
}

export interface WeatherReport {
  kp: number
  observed_at: string
  scales: ScaleDay | null
  outlook: ScaleDay[]
  kp_series: KpPoint[]
  flux: FluxPoint[]
  dst: DstPoint[]
  alerts: WeatherAlert[]
}

export interface FeedState {
  name: string
  fetched_at: string | null
  succeeded: boolean
  error: string | null
}

export interface Sky {
  at: string
  moon: Moon
  season: Turning
  next_turning: Turning
  planets: Planet[]
  showers: ShowerPeak[]
  eclipses: EclipseEvent[]
  events: SkyEvent[]
  launches: Launch[]
  space_weather: SpaceWeather | null
  weather: WeatherSummary | null
  feeds: FeedState[]
}

export type GameSlug = 'date' | 'order' | 'match' | 'words'

export interface GamePicture {
  picture: string
  width?: number
  height?: number
  credit?: string[]
}

export interface Puzzle<T> {
  game: GameSlug
  day?: string
  first: string
  last: string
  rounds: T[]
}

/** Rounds overlap: the `b` of one round is the `a` of the next, with its date already learnt. */
export interface OrderPair {
  a: GamePicture
  b: GamePicture
}

export interface KnownWord {
  word: string
  known: boolean
}

export interface MatchRound {
  round: string
  explanation: string
  choices: GamePicture[]
}

export type ClozePiece = { s: string } | { h: string; n: number }

export interface WordsRound extends GamePicture {
  title_words: number
  salt: string
  title: ClozePiece[]
  text: ClozePiece[]
  hidden: number
  distinct: number
}

export interface Reveal extends ApodSummary {
  picture: string
  dates: string[]
  source_url: string
}

export interface MatchAnswer {
  correct: boolean
  answer: Reveal
}

export function isHidden(piece: ClozePiece): piece is { h: string; n: number } {
  return 'h' in piece
}

export type ResourceSort = 'refs' | 'entries' | 'first' | 'last' | 'label' | 'address'
export type WordSort = 'total' | 'entries' | 'word'
export type SortOrder = 'asc' | 'desc'

export const IMAGE_KINDS: MediaKind[] = ['image_jpg', 'image_png', 'image_gif']
export const VIDEO_KINDS: MediaKind[] = ['video_mp4', 'youtube', 'vimeo']

export type KindFilter = 'image' | 'video' | MediaKind

export function isImage(kind: MediaKind): boolean {
  return IMAGE_KINDS.includes(kind)
}

export function isVideo(kind: MediaKind): boolean {
  return VIDEO_KINDS.includes(kind)
}

export function aspectRatio(media: Media): number | null {
  const { thumb_width: width, thumb_height: height } = media
  return width && height ? width / height : null
}
