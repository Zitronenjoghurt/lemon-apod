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
}

export interface ApodSummary {
  date: string
  title: string
  media: Media
  has_copyright: boolean
}

export interface SearchHit extends ApodSummary {
  snippet: string
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
  min_words: number
  max_words: number
  avg_unique_words: number
  avg_chars: number
  avg_sentences: number
  avg_words_per_sentence: number
  avg_links: number
  used_once: number
  shortest?: EntryLength
  longest?: EntryLength
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
  text: TextSummary
  resources: ResourceSummary
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

export interface ResourceRefs {
  resource: Resource
  items: ResourceRef[]
  total: number
}

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
