<script lang="ts" setup>
import { computed, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import ApodCredit from '@/components/ApodCredit.vue'
import PictureTimeline from '@/components/PictureTimeline.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import { isLost } from '@/api/types'
import { useAsync } from '@/composables/useAsync'
import { formatDate } from '@/utils/date'
import { pageTitle, pictureTitle, setTitle } from '@/utils/title'

const route = useRoute()
const date = computed(() => String(route.params.date ?? ''))

const { data, error, notFound, loading, run } = useAsync((signal) =>
  api.picture(date.value, signal),
)

watch(date, run, { immediate: true })

const picture = computed(() => data.value?.picture)

const titles = computed(() => {
  const counts = new Map<string, number>()
  for (const item of data.value?.items ?? []) {
    counts.set(item.title, (counts.get(item.title) ?? 0) + 1)
  }
  return [...counts.entries()]
    .map(([title, times]) => ({ title, times }))
    .sort((a, b) => b.times - a.times || a.title.localeCompare(b.title))
})

const lost = computed(() => !!picture.value && isLost(picture.value.media))

const years = computed(() => Math.max(1, Math.round((picture.value?.span_days ?? 0) / 365.25)))

watch([picture, notFound], ([found, missing]) => {
  if (found) setTitle(pictureTitle(found.title, found.appearances))
  else if (missing) setTitle(pageTitle('This one only came round once'))
})
</script>

<template>
  <div class="stack">
    <RouterLink class="muted back" to="/pictures">
      <i aria-hidden="true" class="pi pi-arrow-left" /> All encores
    </RouterLink>

    <div v-if="notFound" class="card notice">
      <h1>This one only came round once</h1>
      <p class="muted">
        Either that date is not in the archive, or APOD has never shown its picture again. Pictures
        are regrouped whenever thumbnails change, so a link here can go stale.
      </p>
      <RouterLink class="plain" to="/pictures">
        <Button icon="pi pi-arrow-left" label="Back to the encores" outlined tabindex="-1" />
      </RouterLink>
    </div>

    <RetryNotice v-else-if="error" :busy="loading" :message="error" @retry="run" />

    <div v-else-if="!data" aria-busy="true" aria-label="Loading the picture" class="stack">
      <Skeleton height="16rem" width="100%" />
      <Skeleton height="20rem" width="100%" />
    </div>

    <template v-else-if="picture">
      <header class="card stack head">
        <div class="top">
          <RouterLink :to="`/${picture.first}`" class="shot">
            <img
              v-if="picture.media.thumb_url"
              :alt="picture.title"
              :src="picture.media.thumb_url"
              decoding="async"
              height="300"
              width="480"
            />
            <div v-else :class="{ gone: lost }" class="fallback">
              <template v-if="lost">
                <i aria-hidden="true" class="pi pi-ban" />
                <span class="what">Media lost</span>
              </template>
              <i v-else aria-hidden="true" class="pi pi-image" />
            </div>
          </RouterLink>

          <div class="stack about">
            <h1>{{ picture.title }}</h1>
            <p class="muted lede">
              APOD has come back to this picture {{ picture.appearances }} times across {{ years }}
              {{ years === 1 ? 'year' : 'years' }}. You can see below what exactly has changed over
              time.
            </p>

            <ApodCredit lead="This picture is from NASA's" variant="banner" />

            <dl class="facts">
              <div>
                <dt>Shown</dt>
                <dd>{{ picture.appearances }}&times;</dd>
              </div>
              <div>
                <dt>First</dt>
                <dd class="date">{{ formatDate(picture.first) }}</dd>
              </div>
              <div>
                <dt>Latest</dt>
                <dd class="date">{{ formatDate(picture.last) }}</dd>
              </div>
              <div>
                <dt>Titles</dt>
                <dd>{{ picture.titles }}</dd>
              </div>
            </dl>
          </div>
        </div>

        <div v-if="titles.length > 1" class="stack aka">
          <h2>Titles it has carried</h2>
          <ul class="row">
            <li v-for="{ title, times } in titles" :key="title">
              {{ title }}<span v-if="times > 1" class="muted times"> &times;{{ times }}</span>
            </li>
          </ul>
        </div>
      </header>

      <section class="card timeline-card">
        <h2 class="section">Timeline</h2>
        <PictureTimeline :appearances="data.items" />
      </section>
    </template>
  </div>
</template>

<style scoped>
.back {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-sm);
  text-decoration: none;
  align-self: flex-start;
}

.back:hover {
  color: var(--text);
}

.head {
  padding: var(--space-5) var(--space-5) var(--space-5);
  gap: var(--space-4);
}

.top {
  display: flex;
  gap: var(--space-5);
  align-items: flex-start;
}

@media (max-width: 40rem) {
  .top {
    flex-direction: column;
  }
}

.shot {
  flex: 0 0 auto;
  width: min(16rem, 100%);
  border-radius: var(--radius, 0.6rem);
  overflow: hidden;
  display: block;
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.shot img {
  width: 100%;
  height: auto;
  display: block;
}

.fallback {
  aspect-ratio: 16 / 10;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  font-size: var(--text-xl);
}

.fallback.gone {
  color: hsl(var(--tone-warn));
  background: repeating-linear-gradient(
    -45deg,
    transparent 0 6px,
    color-mix(in srgb, var(--text) 5%, transparent) 6px 12px
  );
}

.fallback .what {
  font-size: var(--text-xs);
  letter-spacing: 0.05em;
  text-transform: uppercase;
}

.about {
  flex: 1 1 20rem;
  gap: var(--space-2);
  min-width: 0;
}

h1 {
  font-size: var(--text-title);
  text-wrap: balance;
}

.lede {
  font-size: var(--text-sm);
  margin: 0;
}

.facts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 7rem), 1fr));
  gap: var(--space-3) var(--space-5);
  margin: 0;
}

.facts dt {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
}

.facts dd {
  margin: 0;
  font-size: var(--text-lg);
  font-variant-numeric: tabular-nums;
}

.facts .date {
  font-size: var(--text-md);
}

.aka {
  gap: var(--space-1);
}

.aka h2,
h2.section {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
  font-weight: 600;
}

.aka ul {
  list-style: none;
  margin: 0;
  padding: 0;
  gap: var(--space-1);
  font-size: var(--text-sm);
}

.aka li {
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  padding: var(--space-0) var(--space-2);
}

.times {
  font-variant-numeric: tabular-nums;
}

.timeline-card {
  padding: var(--space-4) var(--space-5) var(--space-5);
}

.timeline-card h2 {
  margin-bottom: var(--space-4);
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

.notice p {
  max-width: 44ch;
  margin: 0;
}

.plain {
  text-decoration: none;
  color: inherit;
  display: inline-flex;
}
</style>
