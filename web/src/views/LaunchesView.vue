<script lang="ts" setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { RouterLink } from 'vue-router'
import HintPopover from '@/components/HintPopover.vue'
import LaunchImminent from '@/components/LaunchImminent.vue'
import LaunchRow from '@/components/LaunchRow.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import TabBar, { type Tab } from '@/components/TabBar.vue'
import type { Launch } from '@/api/types'
import { useLaunches } from '@/composables/useLaunches'
import { countdown, DAY_SHORT, dayOf, daysAway, hasOutcome, isImminent } from '@/utils/launches'

const SPACE_DEVS = 'https://thespacedevs.com'
const CALM_MS = 30_000
const WATCHING_MS = 1_000

const { data, failed, fetchedAt, refresh } = useLaunches()

const clock = ref(Date.now())
const showing = ref<'upcoming' | 'flown'>('upcoming')
let ticking: ReturnType<typeof setTimeout> | undefined

function tick() {
  clock.value = Date.now()
  ticking = setTimeout(tick, imminent.value.length ? WATCHING_MS : CALM_MS)
}

onMounted(tick)
onUnmounted(() => clearTimeout(ticking))

const imminent = computed(() =>
  [...(data.value?.upcoming ?? []), ...(data.value?.flown ?? [])]
    .filter((launch) => isImminent(launch, clock.value))
    .sort(
      (one, other) =>
        stage(one) - stage(other) ||
        Math.abs(Date.parse(one.net) - clock.value) - Math.abs(Date.parse(other.net) - clock.value),
    ),
)

function stage(launch: Launch): number {
  if (Date.parse(launch.net) > clock.value) return 1
  return hasOutcome(launch.status) ? 2 : 0
}

const upcoming = computed(() =>
  byDay((data.value?.upcoming ?? []).filter((launch) => !isImminent(launch, clock.value))),
)
const flown = computed(() =>
  byDay((data.value?.flown ?? []).filter((launch) => !isImminent(launch, clock.value))),
)

const bandTitle = computed(() => {
  if (imminent.value.some((launch) => stage(launch) === 0)) return 'Flying now'
  if (imminent.value.some((launch) => stage(launch) === 1)) return 'About to fly'
  return 'Just flew'
})

const checked = computed(() =>
  fetchedAt.value ? countdown(new Date(fetchedAt.value).toISOString(), clock.value) : '',
)

function byDay(
  launches: Launch[],
): { day: string; label: string; away: string; launches: Launch[] }[] {
  const days = new Map<string, Launch[]>()

  for (const launch of launches) {
    const day = dayOf(launch.net)
    const held = days.get(day)
    if (held) held.push(launch)
    else days.set(day, [launch])
  }

  return [...days].map(([day, held]) => ({
    day,
    label: label(day),
    away: daysAway(day, clock.value),
    launches: held,
  }))
}

function countIn(groups: { launches: Launch[] }[]): number {
  return groups.reduce((total, group) => total + group.launches.length, 0)
}

const tabs = computed<Tab[]>(() => [
  { value: 'upcoming', label: 'Coming up', count: countIn(upcoming.value) },
  { value: 'flown', label: 'Already launched', count: countIn(flown.value) },
])

const groups = computed(() => (showing.value === 'upcoming' ? upcoming.value : flown.value))

function label(day: string): string {
  const [year, month, date] = day.split('-').map(Number)
  if (!year || !month || !date) return day

  return DAY_SHORT.format(new Date(year, month - 1, date))
}
</script>

<template>
  <div class="stack launches">
    <header class="row head">
      <h1>Rocket launches</h1>
      <HintPopover label="About this page">
        <p>Every recent and upcoming rocket launch.</p>
        <p>
          Launch times can move without warning. Data and photographs from
          <a :href="SPACE_DEVS" data-ours rel="noopener" target="_blank">The Space Devs</a>.
        </p>
      </HintPopover>
    </header>

    <RetryNotice
      v-if="failed && !data"
      message="Could not reach the launch feed."
      @retry="refresh"
    />

    <div v-else-if="!data" class="stack">
      <Skeleton height="12rem" width="100%" />
      <Skeleton height="12rem" width="100%" />
    </div>

    <template v-else-if="data">
      <p v-if="!upcoming.length && !flown.length && !imminent.length" class="card empty muted">
        Nothing has been polled from The Space Devs yet.
      </p>

      <section v-if="imminent.length" class="stack now">
        <h2 class="muted">
          <AppIcon name="imminent" />
          {{ bandTitle }}
        </h2>
        <TransitionGroup class="stack band" name="band" tag="div">
          <LaunchImminent v-for="launch in imminent" :key="launch.id" :launch="launch" />
        </TransitionGroup>
      </section>

      <section class="panel">
        <TabBar
          :on="(tab) => tab.value === showing"
          :tabs="tabs"
          label="Which launches"
          @pick="showing = $event as 'upcoming' | 'flown'"
        />

        <p v-if="!groups.length" class="muted empty">
          {{ showing === 'upcoming' ? 'Nothing is scheduled.' : 'Nothing has flown recently.' }}
        </p>

        <div v-for="group in groups" :key="group.day" class="group">
          <h3 class="day">
            <span class="date">{{ group.label }}</span>
            <span class="muted away">{{ group.away }}</span>
          </h3>
          <LaunchRow
            v-for="launch in group.launches"
            :key="launch.id"
            :flown="showing === 'flown'"
            :launch="launch"
          />
        </div>
      </section>

      <p class="muted source">
        <span :class="{ stale: failed }">
          <AppIcon :name="failed ? 'exclamation-triangle' : 'refresh'" />
          {{ failed ? 'Could not refresh, showing what was last read' : `Checked ${checked}` }}
        </span>
        <button class="again" type="button" @click="refresh">Refresh</button>
        <RouterLink to="/contact">Something wrong?</RouterLink>
      </p>
    </template>
  </div>
</template>

<style scoped>
.launches {
  gap: var(--space-4);
}

.head {
  gap: var(--space-2);
}

h1 {
  font-size: var(--text-xl);
}

.panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.panel .group:first-of-type .day {
  margin-top: var(--space-2);
}

.group {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.day {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: var(--space-3) 0 var(--space-1);
  font-size: var(--text-xs);
  font-weight: 600;
}

.day::after {
  content: '';
  flex: 1;
  height: 1px;
  background: var(--border);
}

.date {
  flex: none;
}

.away {
  flex: none;
  font-weight: 400;
  font-variant-numeric: tabular-nums;
}

.now {
  gap: var(--space-2);
}

.band {
  position: relative;
  gap: var(--space-2);
}

.band-move,
.band-enter-active,
.band-leave-active {
  transition:
    transform var(--dur-slow) var(--ease-out),
    opacity var(--dur-base) var(--ease-out);
}

.band-enter-from,
.band-leave-to {
  opacity: 0;
  transform: translateY(-0.4rem);
}

.band-leave-active {
  position: absolute;
  width: 100%;
}

.now h2 {
  margin: 0;
  font-size: var(--text-xs);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.empty {
  padding: var(--space-7);
  text-align: center;
}

.panel .empty {
  padding: var(--space-6) 0;
}

.source {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
  margin: 0;
  font-size: var(--text-xs);
}

.source .icon {
  font-size: 0.9em;
}

.stale {
  color: hsl(var(--tone-raised));
}

.again {
  padding: 0;
  border: 0;
  background: none;
  color: inherit;
  font: inherit;
  text-decoration: underline;
  cursor: pointer;
}

@media (max-width: 40rem) {
  h1 {
    font-size: var(--text-lg);
  }
}
</style>
