<script lang="ts" setup>
import {computed} from 'vue'
import {useStatus} from '@/composables/useStatus'

defineOptions({name: 'NotificationsView'})

const NTFY_ANDROID = 'https://play.google.com/store/apps/details?id=io.heckel.ntfy'
const NTFY_IOS = 'https://apps.apple.com/us/app/ntfy/id1625396347'
const NTFY_FDROID = 'https://f-droid.org/en/packages/io.heckel.ntfy/'
const NTFY_DOCS = 'https://docs.ntfy.sh/subscribe/phone/'

const {notify, loaded} = useStatus()

const base = computed(() => notify.value?.base_url ?? '')

const feeds = [
  {
    icon: 'pi pi-wifi',
    label: 'Atom',
    href: '/atom.xml',
    hint: 'The newer of the two formats. Pick this one if your reader supports it.',
  },
  {
    icon: 'pi pi-share-alt',
    label: 'RSS',
    href: '/feed.xml',
    hint: 'RSS 2.0, for readers that do not support Atom.',
  },
]

const topics = computed(() => {
  const config = notify.value
  if (!config || !base.value) return []

  return [
    {
      key: 'apod',
      topic: config.apod_topic,
      icon: 'pi pi-sparkles',
      label: 'Picture of the day',
      hint: 'One message a day when a new picture goes up, with the thumbnail attached.',
      cadence: 'About once a day',
    },
    {
      key: 'aurora',
      topic: config.aurora_topic,
      icon: 'pi pi-bolt',
      label: 'Aurora alerts',
      hint: "Geomagnetic storms through NOAA's G-scale alerts",
      cadence: 'Rare, depending on solar activity',
    },
    {
      key: 'space-weather',
      topic: config.space_weather_topic,
      icon: 'pi pi-sun',
      label: 'Other space weather',
      hint: 'Everything else NOAA issues: proton events, radio blackouts, electron flux.',
      cadence: 'Occasional',
    },
    {
      key: 'sky',
      topic: config.sky_topic,
      icon: 'pi pi-star',
      label: 'Sky events',
      hint: 'Meteor shower peaks, eclipses and supermoons, a day or so ahead.',
      cadence: 'A handful a year',
    },
  ].filter((entry): entry is typeof entry & { topic: string } => Boolean(entry.topic))
})

const anyTopic = computed(() => topics.value.length > 0)
const isAndroid = /android/i.test(navigator.userAgent)

function topicUrl(topic: string): string {
  return `${base.value}/${topic}`
}

function feedUrl(path: string): string {
  return `${window.location.origin}${path}`
}

function appLink(topic: string): string {
  return `${base.value.replace(/^https?:\/\//, 'ntfy://')}/${topic}`
}
</script>

<template>
  <div class="stack notifications">
    <header class="stack head">
      <h1>Notifications</h1>
      <p class="muted lede">
        Add the APOD feed to your RSS or Atom reader or use our ntfy topics to receive push
        notifications straight to your device.
      </p>
    </header>

    <section class="card panel">
      <h2 class="muted">Feeds</h2>
      <p class="note">
        The last 25 APOD entries with their explanations. You can point any feed reader at these.
      </p>

      <ul class="items">
        <li v-for="feed in feeds" :key="feed.href">
          <a :href="feed.href">
            <i :class="feed.icon" aria-hidden="true"/>
            <span class="text">
              <span class="value">{{ feed.label }}</span>
              <span class="muted label">{{ feed.hint }}</span>
            </span>
            <code class="url">{{ feedUrl(feed.href) }}</code>
          </a>
        </li>
      </ul>
    </section>

    <section class="card panel">
      <h2 class="muted">NTFY Push</h2>

      <div v-if="!loaded" class="stack">
        <Skeleton height="3rem"/>
        <Skeleton height="3rem"/>
      </div>

      <p v-else-if="!anyTopic" class="note">
        No push server is configured on this deployment. The feeds above still work.
      </p>

      <template v-else>
        <p class="note">
          Install the ntfy app and subscribe to any of the topics below. Ntfy is an open source
          pub-sub notification service.
        </p>

        <ul class="topics">
          <li v-for="entry in topics" :key="entry.key">
            <i :class="entry.icon" aria-hidden="true"/>
            <span class="text">
              <span class="value">{{ entry.label }}</span>
              <span class="muted label">{{ entry.hint }}</span>
            </span>
            <span class="foot">
              <a v-if="isAndroid" :href="appLink(entry.topic)" class="plain">
                <Button icon="pi pi-mobile" label="Open in app" size="small"/>
              </a>
              <code class="url">{{ topicUrl(entry.topic) }}</code>
              <span class="muted cadence">{{ entry.cadence }}</span>
            </span>
          </li>
        </ul>

        <div class="apps">
          <span class="muted label">Get ntfy</span>
          <a :href="NTFY_ANDROID" rel="noopener" target="_blank">Android</a>
          <a :href="NTFY_FDROID" rel="noopener" target="_blank">F-Droid</a>
          <a :href="NTFY_IOS" rel="noopener" target="_blank">iOS</a>
          <a :href="NTFY_DOCS" rel="noopener" target="_blank">How it works</a>
        </div>

        <p v-if="isAndroid" class="muted note">
          If a topic does not open in the app, click "use another server" and enter
          <code>{{ base }}</code
          >, then enter any topic name from above, e.g. <code>{{ topics[0].topic }}</code> or
          <code>{{ topics[1].topic }}</code
          >.
        </p>
        <p v-else class="muted note">
          When adding a topic in the ntfy app, click "use another server" and enter
          <code>{{ base }}</code
          >, then enter any topic name from above, e.g. <code>{{ topics[0].topic }}</code> or
          <code>{{ topics[1].topic }}</code
          >.
        </p>
      </template>
    </section>
  </div>
</template>

<style scoped>
.notifications {
  max-width: 52rem;
  margin-inline: auto;
  gap: 1.5rem;
}

h1 {
  font-size: 1.6rem;
}

.head {
  gap: 0.5rem;
}

.lede {
  margin: 0;
  max-width: 62ch;
  text-wrap: pretty;
}

.panel {
  padding: 1.1rem 1.2rem 1.3rem;
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

.panel h2 {
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.07em;
  font-weight: 600;
}

.note {
  margin: 0;
  font-size: 0.85rem;
  text-wrap: pretty;
  max-width: 62ch;
}

.items {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.items a {
  display: flex;
  align-items: center;
  gap: 0.7rem;
  padding: 0.6rem;
  border-radius: 0.6rem;
  text-decoration: none;
  color: inherit;
}

.items > li > a:hover {
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.items i,
.topics i {
  font-size: 1.05rem;
  color: var(--accent);
  width: 1.3rem;
  text-align: center;
  flex: none;
}

.text {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
}

.label {
  font-size: 0.78rem;
  line-height: 1.45;
  text-wrap: pretty;
}

.cadence {
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  margin-left: auto;
}

.value {
  font-size: 0.92rem;
  font-weight: 550;
}

.url {
  font-size: 0.76rem;
  opacity: 0.7;
  overflow-wrap: anywhere;
}

.topics {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.topics > li {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 0.5rem 0.7rem;
  padding: 0.8rem 0.9rem;
  border-radius: 0.6rem;
  background: color-mix(in srgb, var(--text) 4%, transparent);
}

.topics i {
  grid-column: 1;
  grid-row: 1;
  line-height: 1.5;
}

.topics .text {
  grid-column: 2;
  grid-row: 1;
}

.foot {
  grid-column: 2;
  grid-row: 2;
  display: flex;
  align-items: center;
  gap: 0.5rem 0.75rem;
  flex-wrap: wrap;
}

.plain {
  text-decoration: none;
  color: inherit;
  display: inline-flex;
}

.apps {
  display: flex;
  align-items: center;
  gap: 0.9rem;
  flex-wrap: wrap;
  font-size: 0.82rem;
}

@media (max-width: 40rem) {
  .items .url {
    display: none;
  }

  .cadence {
    margin-left: 0;
  }
}
</style>
