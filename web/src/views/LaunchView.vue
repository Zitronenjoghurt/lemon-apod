<script lang="ts" setup>
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import HintPopover from '@/components/HintPopover.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import { useAsync } from '@/composables/useAsync'
import {
  clockOf,
  countdown,
  DAY_LONG,
  isFirm,
  statusTone,
  streamLabel,
  summaryOf,
  TIME,
  tminus,
  windowOf,
} from '@/utils/launches'
import { pageTitle, setTitle } from '@/utils/title'

const TICK_MS = 1_000
const REFRESH_MS = 30_000

const route = useRoute()
const id = computed(() => String(route.params.id ?? ''))

const {
  data: launch,
  error,
  notFound,
  loading,
  run,
} = useAsync((signal) => api.launch(id.value, signal))

const clock = ref(Date.now())
let ticking: ReturnType<typeof setInterval> | undefined
let refreshing: ReturnType<typeof setInterval> | undefined

onMounted(() => {
  ticking = setInterval(() => (clock.value = Date.now()), TICK_MS)
  refreshing = setInterval(() => void run(), REFRESH_MS)
  document.addEventListener('visibilitychange', onVisible)
})

onUnmounted(() => {
  clearInterval(ticking)
  clearInterval(refreshing)
  document.removeEventListener('visibilitychange', onVisible)
})

function onVisible(): void {
  if (document.visibilityState === 'visible') void run()
}

const flown = computed(() => {
  const at = launch.value ? new Date(launch.value.net).getTime() : NaN
  return !Number.isNaN(at) && at < clock.value
})

const tone = computed(() => statusTone(launch.value?.status ?? null))

const counting = computed(() =>
  launch.value && isFirm(launch.value) ? tminus(launch.value.net, clock.value) : null,
)

const when = computed(() => {
  const found = launch.value
  if (!found) return ''

  const at = new Date(found.net)
  if (Number.isNaN(at.getTime())) return found.net

  return isFirm(found) ? `${DAY_LONG.format(at)}, ${TIME.format(at)}` : DAY_LONG.format(at)
})

const hero = computed(() => launch.value?.image_full_url ?? launch.value?.image_url ?? null)

const groups = computed(() => {
  const found = launch.value
  if (!found) return []

  const sections = [
    {
      icon: 'launch',
      name: 'Rocket',
      facts: [
        { label: 'Provider', value: found.provider },
        { label: 'Vehicle', value: found.vehicle },
      ],
    },
    {
      icon: 'payload',
      name: 'Payload',
      facts: [
        { label: 'Mission', value: found.mission_name },
        { label: 'Kind', value: found.mission_type },
        { label: 'Orbit', value: found.orbit },
        { label: 'Programme', value: found.program },
      ],
    },
    {
      icon: 'place',
      name: 'Location',
      facts: [
        { label: 'Pad', value: found.pad },
        {
          label: 'Site',
          value:
            found.pad_location && found.pad?.includes(found.pad_location)
              ? null
              : found.pad_location,
        },
        {
          label: 'Number of launches',
          value: found.pad_launches ? found.pad_launches.toLocaleString() : null,
        },
      ],
    },
    {
      icon: 'clock',
      name: 'Window',
      facts: [
        { label: 'Opens', value: windowOf(found) },
        { label: 'Timing', value: isFirm(found) ? null : clockOf(found) },
        { label: 'Weather', value: found.probability != null ? `${found.probability}% go` : null },
        { label: 'Concerns', value: found.weather_concerns },
        { label: 'Status', value: tone.value ? null : found.status },
      ],
    },
  ]

  return sections
    .map((section) => ({
      ...section,
      facts: section.facts.filter(
        (fact) => fact.value && fact.value !== 'Unknown' && fact.value !== 'TBD',
      ),
    }))
    .filter((section) => section.facts.length > 0)
})

watch(id, () => void run(), { immediate: true })

watch(launch, (found) => {
  if (found) setTitle(pageTitle(found.name))
})
</script>

<template>
  <div class="stack launch">
    <RouterLink class="back" to="/launches">
      <AppIcon name="chevron-left" />
      Rocket launches
    </RouterLink>

    <RetryNotice v-if="error && !notFound" :busy="loading" :message="error" @retry="run" />

    <div v-else-if="notFound" class="card notice">
      <h1>This launch is no longer listed</h1>
      <p class="muted">
        Launches are kept for thirty days after they fly. Beyond that the record here is gone.
      </p>
      <RouterLink class="plain" to="/launches">
        <Button label="Back to the launches" outlined tabindex="-1">
          <template #icon><AppIcon name="arrow-left" /></template>
        </Button>
      </RouterLink>
    </div>

    <div v-else-if="loading && !launch" class="stack">
      <Skeleton height="2rem" width="18rem" />
      <Skeleton height="18rem" width="100%" />
    </div>

    <template v-else-if="launch">
      <header class="stack head">
        <p v-if="summaryOf(launch)" class="muted kicker">{{ summaryOf(launch) }}</p>
        <h1>{{ launch.name }}</h1>
        <div :class="['moment', tone]">
          <p class="now">
            <span v-if="counting" class="tminus">{{ counting }}</span>
            <span v-else class="away">{{ countdown(launch.net, clock) }}</span>
            <span v-if="launch.status" class="status">
              {{ launch.status }}
              <HintPopover v-if="launch.status_note" :label="`About ${launch.status}`">
                <p>{{ launch.status_note }}</p>
              </HintPopover>
            </span>
            <a
              v-if="launch.webcast_live && launch.streams.length"
              :href="launch.streams[0].url"
              class="plain"
              data-ours
              rel="noopener"
              target="_blank"
            >
              <Button label="Watch it live" severity="danger" size="small">
                <template #icon><AppIcon name="video" /></template>
              </Button>
            </a>
          </p>
          <p class="muted at">{{ when }}</p>
        </div>
      </header>

      <figure v-if="hero" class="shot">
        <img :alt="launch.name" :src="hero" decoding="async" />
        <figcaption v-if="launch.image_credit" class="muted">
          Image: {{ launch.image_credit }}
        </figcaption>
      </figure>

      <div class="panels">
        <section v-if="groups.length" class="card panel">
          <h2 class="muted">
            <AppIcon name="info" />
            Details
          </h2>

          <div class="groups">
            <section v-for="group in groups" :key="group.name" class="group">
              <h3><AppIcon :name="group.icon" /> {{ group.name }}</h3>
              <dl class="facts">
                <div v-for="fact in group.facts" :key="fact.label">
                  <dt class="muted">{{ fact.label }}</dt>
                  <dd>{{ fact.value }}</dd>
                </div>
              </dl>
            </section>
          </div>
        </section>

        <section v-if="launch.mission" class="card panel">
          <h2 class="muted"><AppIcon name="payload" /> Mission</h2>
          <p class="mission">{{ launch.mission }}</p>
        </section>

        <section v-if="launch.fail_reason" class="card panel failure">
          <h2 class="muted">
            <AppIcon name="exclamation-triangle" />
            What went wrong
          </h2>
          <p class="mission">{{ launch.fail_reason }}</p>
        </section>

        <section v-if="launch.streams.length || launch.info_url" class="card panel">
          <h2 class="muted">
            <AppIcon name="video" />
            {{ flown ? 'Watch it back' : 'Webcasts' }}
          </h2>

          <ul class="links">
            <li v-for="stream in launch.streams" :key="stream.url">
              <a :href="stream.url" data-ours rel="noopener" target="_blank">
                <AppIcon name="video" />
                {{ streamLabel(stream) }}
                <span v-if="stream.kind" class="muted kind">{{ stream.kind }}</span>
                <AppIcon class="out" name="external" />
              </a>
            </li>
            <li v-if="launch.info_url">
              <a :href="launch.info_url" data-ours rel="noopener" target="_blank">
                <AppIcon name="info" />
                Launch page
                <AppIcon class="out" name="external" />
              </a>
            </li>
          </ul>
        </section>

        <section v-if="launch.pad_map_image" class="card panel site">
          <h2 class="muted">
            <AppIcon name="place" />
            {{ launch.pad_location ?? launch.pad ?? 'Launch site' }}
          </h2>
          <a
            :href="launch.pad_map_url ?? undefined"
            class="map"
            data-ours
            rel="noopener"
            target="_blank"
          >
            <img :alt="launch.pad ?? 'Launch pad'" :src="launch.pad_map_image" decoding="async" />
          </a>
        </section>
      </div>
    </template>
  </div>
</template>

<style scoped>
.launch {
  gap: var(--space-4);
}

.back {
  display: inline-flex;
  align-items: center;
  gap: var(--space-0);
  align-self: flex-start;
  font-size: var(--text-sm);
  text-decoration: none;
}

.back:hover {
  text-decoration: underline;
}

.head {
  gap: var(--space-0);
}

.kicker {
  margin: 0;
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

h1 {
  font-size: var(--text-xl);
  text-wrap: balance;
}

.shot {
  position: relative;
  margin: 0;
}

.shot img {
  display: block;
  width: 100%;
  max-height: 26rem;
  object-fit: cover;
  border: 1px solid var(--border);
  border-radius: var(--radius);
}

.shot figcaption {
  position: absolute;
  right: var(--space-2);
  bottom: var(--space-2);
  max-width: calc(100% - var(--space-5));
  padding: var(--space-0) var(--space-2);
  border-radius: var(--radius-sm);
  background: rgb(0 0 0 / 0.55);
  color: #fff;
  font-size: var(--text-2xs);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.moment {
  display: flex;
  flex-direction: column;
  gap: var(--space-0);
  margin-top: var(--space-2);
}

.moment p {
  margin: 0;
}

.now {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.tminus,
.away {
  font-size: clamp(1.5rem, 1.15rem + 1.2vw, 1.9rem);
  font-weight: 650;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.01em;
  line-height: 1.15;
}

.tminus {
  color: var(--accent);
}

.at {
  font-size: var(--text-sm);
}

.status {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 0 var(--space-2);
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  font-size: var(--text-xs);
  line-height: 1.5;
  white-space: nowrap;
}

.moment.bad .status {
  border-color: var(--bad);
  color: var(--bad);
}

.moment.warn .status {
  border-color: hsl(var(--tone-raised));
  color: hsl(var(--tone-raised));
}

.moment.good .status {
  border-color: var(--good);
  color: var(--good);
}

.plain {
  text-decoration: none;
  color: inherit;
  display: inline-flex;
}

.failure {
  border-color: color-mix(in srgb, var(--bad) 45%, var(--border));
}

.failure h2 .icon {
  color: var(--bad);
}

.map {
  display: block;
  text-decoration: none;
  color: inherit;
}

.map img {
  width: 100%;
  aspect-ratio: 21 / 9;
  max-height: 16rem;
  object-fit: cover;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
}

.panels {
  display: grid;
  gap: var(--space-4);
  grid-template-columns: minmax(0, 1fr);
  align-items: start;
}

.panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-5);
}

.panel h2 {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--text-xs);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.groups {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 13rem), 1fr));
  gap: var(--space-4);
}

.group h3 {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  margin: 0 0 var(--space-2);
  padding-bottom: var(--space-1);
  border-bottom: 1px solid var(--border);
  font-size: var(--text-2xs);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-muted);
}

.facts {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
}

.facts dt {
  font-size: var(--text-2xs);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.facts dd {
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.mission {
  margin: 0;
  font-size: var(--text-sm);
  line-height: 1.5;
  text-align: justify;
  hyphens: auto;
}

.links {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.links a {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-sm);
}

.kind,
.out {
  font-size: var(--text-2xs);
}

.notice {
  padding: var(--space-8) var(--space-7);
  text-align: center;
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  align-items: center;
}

.notice h1 {
  font-size: var(--text-title);
}

@media (min-width: 46rem) {
  .panels {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .panels > :first-child,
  .panels > .site {
    grid-column: 1 / -1;
  }
}
</style>
