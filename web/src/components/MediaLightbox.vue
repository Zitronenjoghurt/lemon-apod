<script lang="ts" setup>
import { onBeforeUnmount, watch } from 'vue'
import { useRouter } from 'vue-router'
import PhotoSwipeLightbox from 'photoswipe/lightbox'
import 'photoswipe/style.css'
import { APOD_URL } from '@/utils/links'

export interface Slide {
  src: string
  width: number
  height: number
  alt: string
  hd?: string
  thumb?: string
  entry?: string
  source?: string
  from?: () => HTMLImageElement | null
}

const props = defineProps<{
  slides: Slide[]
  at: number | null
}>()

const emit = defineEmits<{ close: [] }>()

const router = useRouter()

let gallery: PhotoSwipeLightbox | null = null

function sizeOf(url: string): Promise<{ width: number; height: number }> {
  return new Promise((resolve) => {
    const probe = new Image()
    probe.onload = () => resolve({ width: probe.naturalWidth, height: probe.naturalHeight })
    probe.onerror = () => resolve({ width: 0, height: 0 })
    probe.src = url
  })
}

function creditFor(slide: Slide): string {
  return `
    <span class="apod-mark">
      <span class="apod-kicker">From NASA's</span>
      <a class="apod-name" href="${slide.source || APOD_URL}" rel="noopener" target="_blank">
        Astronomy Picture of the Day
      </a>
      <span class="apod-work"></span>
    </span>
  `
}

function show(index: number) {
  close()

  const slides = props.slides
  if (!slides.length) return

  gallery = new PhotoSwipeLightbox({
    dataSource: slides.map((slide) => ({
      src: slide.src,
      msrc: slide.thumb,
      width: slide.width,
      height: slide.height,
      alt: slide.alt,
      element: slide.from?.() ?? undefined,
    })),
    pswpModule: () => import('photoswipe'),
    showHideAnimationType: 'zoom',
    bgOpacity: 1,
    paddingFn: (viewport) => ({
      top: viewport.x < 480 ? 56 : 68,
      bottom: 16,
      left: 16,
      right: 16,
    }),
    zoom: false,
    counter: slides.length > 1,
    arrowKeys: slides.length > 1,
    loop: false,
    showAnimationDuration: 220,
    hideAnimationDuration: 200,
    zoomAnimationDuration: 200,
    errorMsg: 'NASA would not serve this picture just now. This usually clears on its own.',
  })

  gallery.on('uiRegister', () => {
    const ui = gallery?.pswp?.ui
    if (!ui) return

    ui.registerElement({
      name: 'apod-credit',
      order: 7,
      isButton: false,
      appendTo: 'root',
      html: creditFor(slides[0] as Slide),
      onInit: (element, pswp) => {
        const paint = () => {
          const slide = props.slides[pswp.currIndex]
          if (!slide) return
          const link = element.querySelector<HTMLAnchorElement>('.apod-name')
          const work = element.querySelector('.apod-work')
          if (link) link.href = slide.source || APOD_URL
          if (work) work.textContent = slide.alt
        }
        paint()
        pswp.on('change', paint)
      },
    })

    ui.registerElement({
      name: 'apod-entry',
      order: 9,
      isButton: true,
      tagName: 'a',
      title: 'Open this entry',
      html: '<i class="pi pi-book" aria-hidden="true"></i>',
      onInit: (element, pswp) => {
        const anchor = element as HTMLAnchorElement
        const paint = () => {
          const to = props.slides[pswp.currIndex]?.entry
          anchor.href = to ?? ''
          anchor.hidden = !to
        }
        paint()
        pswp.on('change', paint)

        anchor.addEventListener('click', (event) => {
          if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) return
          const to = anchor.getAttribute('href')
          if (!to) return
          event.preventDefault()
          pswp.close()
          void router.push(to)
        })
      },
    })

    ui.registerElement({
      name: 'apod-original',
      order: 10,
      isButton: true,
      tagName: 'a',
      title: 'Open the original file at NASA',
      html: '<i class="pi pi-external-link" aria-hidden="true"></i>',
      onInit: (element, pswp) => {
        const anchor = element as HTMLAnchorElement
        anchor.target = '_blank'
        anchor.rel = 'noopener'
        const paint = () => {
          const slide = props.slides[pswp.currIndex]
          anchor.href = slide?.hd || slide?.src || ''
        }
        paint()
        pswp.on('change', paint)
      },
    })
  })

  gallery.on('wheel', (event) => {
    const wheel = event.originalEvent
    if (!wheel.ctrlKey) return

    const pswp = gallery?.pswp
    const slide = pswp?.currSlide
    if (!slide?.isZoomable()) return

    event.preventDefault()
    const factor = 2 ** (-wheel.deltaY * (wheel.deltaMode ? 0.06 : 0.012))
    slide.zoomTo(slide.currZoomLevel * factor, { x: wheel.clientX, y: wheel.clientY }, false)
  })

  gallery.on('change', () => void upgrade())

  gallery.on('destroy', () => {
    gallery = null
    emit('close')
  })

  gallery.init()
  gallery.loadAndOpen(index)

  void upgrade()
}

async function upgrade() {
  const pswp = gallery?.pswp
  const index = pswp?.currIndex
  if (!pswp || index === undefined) return

  const slide = props.slides[index]
  const big = slide?.hd
  if (!slide || !big || big === slide.src) return

  const shown = pswp.currSlide?.data
  if (!shown || shown.src === big) return

  const size = await sizeOf(big)
  if (!size.width || !gallery?.pswp || gallery.pswp.currIndex !== index) return

  const current = gallery.pswp.currSlide?.data
  if (!current) return

  current.src = big
  current.width = size.width
  current.height = size.height
  gallery.pswp.refreshSlideContent(index)
}

function close() {
  if (!gallery) return
  const closing = gallery
  gallery = null
  closing.destroy()
}

watch(
  () => props.at,
  (index) => (index === null || index === undefined ? close() : show(index)),
)

onBeforeUnmount(close)
</script>

<template><span aria-hidden="true" class="lightbox-anchor" /></template>

<style scoped>
.lightbox-anchor {
  display: none;
}
</style>

<style>
.pswp {
  --pswp-bg: #04050c;
  --pswp-icon-color: #fff;
  --pswp-icon-color-secondary: #04050c;
}

.pswp__apod-credit {
  position: absolute;
  top: 0;
  left: 0;
  max-width: min(60%, 22rem);
  padding: 0.6rem 0.9rem;
  pointer-events: auto;
}

.apod-mark {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
  text-align: left;
}

.apod-kicker {
  font-size: 0.72rem;
  letter-spacing: 0.09em;
  text-transform: uppercase;
  color: rgb(255 255 255 / 0.7);
}

.apod-name {
  font-size: 0.85rem;
  font-weight: 600;
  color: #fff;
  text-decoration: none;
}

.apod-name:hover,
.apod-name:focus-visible {
  text-decoration: underline;
}

.apod-work {
  font-size: 0.75rem;
  color: rgb(255 255 255 / 0.78);
  text-wrap: pretty;
}

@media (max-width: 30rem) {
  .apod-work {
    display: none;
  }
}

.pswp__button--apod-original,
.pswp__button--apod-entry {
  display: grid;
  place-items: center;
  color: #fff;
}

.pswp__button--apod-entry[hidden] {
  display: none;
}
</style>
