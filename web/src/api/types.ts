export type MediaKind =
  'image_jpg' | 'image_png' | 'image_gif' | 'video_mp4' | 'youtube' | 'vimeo' | 'other' | 'none'

export interface Media {
  kind: MediaKind
  url: string | null
  hd_url: string | null
  thumb_url?: string | null
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

export interface Stats {
  entries: number
  thumbnails: number
  first: string | null
  latest: string | null
  by_media_kind: { kind: MediaKind; count: number }[]
}

export const IMAGE_KINDS: MediaKind[] = ['image_jpg', 'image_png', 'image_gif']
export const VIDEO_KINDS: MediaKind[] = ['video_mp4', 'youtube', 'vimeo']

export function isImage(kind: MediaKind): boolean {
  return IMAGE_KINDS.includes(kind)
}

export function isVideo(kind: MediaKind): boolean {
  return VIDEO_KINDS.includes(kind)
}
