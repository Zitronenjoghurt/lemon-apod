<script lang="ts" setup>
import { computed, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import ApodCredit from '@/components/ApodCredit.vue'
import PictureTimeline from '@/components/PictureTimeline.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
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
            <div v-else class="fallback"><i aria-hidden="true" class="pi pi-image" /></div>
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
  gap: 0.4rem;
  font-size: 0.85rem;
  text-decoration: none;
  align-self: flex-start;
}

.back:hover {
  color: var(--text);
}

.head {
  padding: 1.2rem 1.3rem 1.3rem;
  gap: 1rem;
}

.top {
  display: flex;
  gap: 1.2rem;
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
  display: grid;
  place-items: center;
  color: var(--text-muted);
  font-size: 1.6rem;
}

.about {
  flex: 1 1 20rem;
  gap: 0.6rem;
  min-width: 0;
}

h1 {
  font-size: 1.45rem;
  text-wrap: balance;
}

.lede {
  font-size: 0.9rem;
  margin: 0;
}

.facts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 7rem), 1fr));
  gap: 0.7rem 1.2rem;
  margin: 0;
}

.facts dt {
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
}

.facts dd {
  margin: 0;
  font-size: 1.2rem;
  font-variant-numeric: tabular-nums;
}

.facts .date {
  font-size: 0.95rem;
}

.aka {
  gap: 0.35rem;
}

.aka h2,
h2.section {
  font-size: 0.78rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
  font-weight: 600;
}

.aka ul {
  list-style: none;
  margin: 0;
  padding: 0;
  gap: 0.35rem;
  font-size: 0.85rem;
}

.aka li {
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 0.1rem 0.6rem;
}

.times {
  font-variant-numeric: tabular-nums;
}

.timeline-card {
  padding: 1.1rem 1.3rem 1.3rem;
}

.timeline-card h2 {
  margin-bottom: 0.9rem;
}

.notice {
  padding: 3rem 2rem;
  text-align: center;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  align-items: center;
}

.notice h1 {
  font-size: 1.4rem;
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
