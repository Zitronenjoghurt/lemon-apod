<script lang="ts" setup>
import { computed, onMounted } from 'vue'
import { RouterLink } from 'vue-router'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import { useAsync } from '@/composables/useAsync'
import { formatDate } from '@/utils/date'
import { APOD_URL } from '@/utils/links'

defineOptions({ name: 'ModernizationView' })

const { data, error, loading, run } = useAsync((signal) => api.migration(signal))

const coverage = computed(() => data.value?.coverage ?? null)

const checked = computed(() => {
  const split = coverage.value
  return split ? split.carried + split.absent : 0
})

const missing = computed(() => coverage.value?.absent_dates ?? [])

const dated = computed(() => checked.value + (coverage.value?.unchecked ?? 0))

const mostlyUnchecked = computed(
  () => dated.value === 0 || (coverage.value?.unchecked ?? 0) / dated.value > 0.25,
)

const columns = computed(() => {
  const years = coverage.value?.years ?? []
  const tallest = Math.max(...years.map((year) => year.entries), 1)

  return years.map((year) => ({
    ...year,
    height: (year.entries / tallest) * 100,
    tick: year.year % 5 === 0,
    caption:
      `${year.year}: ${year.carried} of ${year.entries} on NASA's site` +
      (year.absent ? `, ${year.absent} missing` : '') +
      (year.unchecked ? `, ${year.unchecked} not checked` : ''),
  }))
})

function count(value: number | undefined): string {
  return value === undefined ? 'n/a' : value.toLocaleString()
}

function fieldLabel(field: string): string {
  return field.replace(/_/g, ' ')
}

onMounted(run)
</script>

<template>
  <div class="stack modernization">
    <header class="stack head">
      <h1>Modernization</h1>
    </header>

    <RetryNotice v-if="error" :busy="loading" :message="error" @retry="run" />

    <template v-if="data">
      <section class="tiles">
        <div class="card tile">
          <span class="muted name">Missing from NASA's site</span>
          <strong class="value">{{ count(coverage?.absent) }}</strong>
          <span class="muted foot">of {{ count(checked) }} checked so far</span>
        </div>
        <div class="card tile">
          <span class="muted name">Entries that differ</span>
          <strong class="value">{{ count(data.divergent_entries) }}</strong>
          <span class="muted foot">
            <RouterLink to="/modernization/changes">see what changed</RouterLink>
          </span>
        </div>
        <div class="card tile">
          <span class="muted name">Differences recorded</span>
          <strong class="value">{{ count(data.differences) }}</strong>
          <span class="muted foot">across {{ data.divergences.length }} fields</span>
        </div>
      </section>

      <section class="card panel">
        <h2>Explanation</h2>
        <p>
          As part of NASA's web modernization, the
          <a :href="APOD_URL" rel="noopener" target="_blank">Astronomy Picture of the Day</a> moved
          from <code>apod.nasa.gov</code> to <code>science.nasa.gov/apod</code>. The migration to
          the modern format is not exactly one to one: a few entries might have changed or are still
          missing. We are keeping the data form the old website safe and making changes transparent.
        </p>
        <p v-if="coverage?.unchecked" class="muted note">
          {{ count(coverage.unchecked) }} dates have not been checked against the new site yet.
        </p>
      </section>

      <section v-if="missing.length" class="card panel">
        <h2>Missing on the modern site</h2>
        <ul class="dates row">
          <li v-for="date in missing" :key="date">
            <RouterLink :to="`/${date}`">{{ formatDate(date) }}</RouterLink>
          </li>
        </ul>
      </section>

      <section v-if="data.divergences.length" class="card panel">
        <h2>What changed</h2>
        <ul class="breakdown">
          <li v-for="row in data.divergences" :key="row.field">
            <RouterLink :to="{ path: '/modernization/changes', query: { field: row.field } }">
              {{ fieldLabel(row.field) }}
            </RouterLink>
            <span class="muted tally">{{ count(row.entries) }}</span>
          </li>
        </ul>
      </section>

      <section v-if="!coverage" class="card panel">
        <h2>Coverage is not measured right now</h2>
      </section>

      <section v-else class="card panel">
        <h2>Coverage</h2>

        <ul class="key row">
          <li><span class="swatch carried" />On NASA's site</li>
          <li><span class="swatch absent" />Missing</li>
          <li><span class="swatch unchecked" />Not checked</li>
        </ul>

        <template v-if="mostlyUnchecked">
          <div
            :aria-label="`${count(coverage.carried)} on NASA's site, ${count(coverage.absent)} missing, ${count(coverage.unchecked)} not checked`"
            class="progress"
            role="img"
          >
            <span
              v-if="coverage.carried"
              :style="{ flexGrow: coverage.carried }"
              class="seg carried"
            />
            <span
              v-if="coverage.absent"
              :style="{ flexGrow: coverage.absent }"
              class="seg absent"
            />
            <span
              v-if="coverage.unchecked"
              :style="{ flexGrow: coverage.unchecked }"
              class="seg unchecked"
            />
          </div>
          <p class="muted note">
            {{ count(checked) }} of {{ count(dated) }} dates checked,
            {{ count(coverage.absent) }} of them missing from the modern page.
          </p>
        </template>

        <div
          v-else
          :aria-label="`Coverage from ${columns[0]?.year} to ${columns.at(-1)?.year}`"
          class="chart"
          role="img"
        >
          <ol class="cols">
            <li v-for="year in columns" :key="year.year" :title="year.caption">
              <span class="col">
                <span :style="{ height: `${year.height}%` }" class="stack-bar">
                  <span
                    v-if="year.unchecked"
                    :style="{ flexGrow: year.unchecked }"
                    class="seg unchecked"
                  />
                  <span v-if="year.absent" :style="{ flexGrow: year.absent }" class="seg absent" />
                  <span
                    v-if="year.carried"
                    :style="{ flexGrow: year.carried }"
                    class="seg carried"
                  />
                </span>
              </span>
              <span :class="{ on: year.tick }" class="tick">{{ year.tick ? year.year : '' }}</span>
            </li>
          </ol>
        </div>
      </section>
    </template>
  </div>
</template>

<style scoped>
.modernization {
  max-width: 62rem;
  margin-inline: auto;
  gap: 1.5rem;
}

h1 {
  font-size: 1.6rem;
}

h2 {
  font-size: 1.05rem;
  margin: 0;
}

.tiles {
  display: grid;
  gap: var(--gap);
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 13rem), 1fr));
}

.tile {
  padding: 1rem 1.1rem;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}

.tile .name {
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.tile .value {
  font-size: 1.7rem;
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.02em;
  line-height: 1.2;
}

.tile .foot {
  font-size: 0.8rem;
}

.panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-5);
}

.panel p {
  margin: 0;
  text-wrap: pretty;
}

.note {
  font-size: 0.9rem;
}

.dates {
  list-style: none;
  margin: 0;
  padding: 0;
  gap: 0.4rem 0.9rem;
  flex-wrap: wrap;
  font-size: 0.88rem;
  font-variant-numeric: tabular-nums;
}

.key {
  list-style: none;
  margin: 0;
  padding: 0;
  gap: var(--space-4);
  flex-wrap: wrap;
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.key li {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
}

.swatch {
  width: var(--space-3);
  height: var(--space-3);
  border-radius: var(--radius-sm);
}

.chart {
  overflow-x: auto;
}

.progress {
  display: flex;
  height: var(--space-3);
  border-radius: var(--radius-pill);
  overflow: hidden;
  background: color-mix(in srgb, var(--text) 8%, transparent);
}

.progress .seg {
  min-width: 3px;
}

.cols {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  align-items: flex-end;
  gap: 2px;
  min-width: 22rem;
}

.cols li {
  flex: 1 1 0;
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 0.3rem;
}

.col {
  display: flex;
  align-items: flex-end;
  height: 7rem;
}

.stack-bar {
  display: flex;
  flex-direction: column;
  width: 100%;
  border-radius: 2px;
  overflow: hidden;
  background: color-mix(in srgb, var(--text) 8%, transparent);
}

.seg {
  display: block;
  min-height: 1px;
}

.seg.carried,
.swatch.carried {
  background: var(--accent);
}

.seg.absent,
.swatch.absent {
  background: hsl(var(--tone-warn));
}

.seg.unchecked,
.swatch.unchecked {
  background: color-mix(in srgb, var(--text) 16%, transparent);
}

.tick {
  font-size: var(--text-xs);
  color: transparent;
  text-align: center;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.tick.on {
  color: var(--text-muted);
}

.breakdown {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.breakdown li {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  padding-bottom: 0.35rem;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
}

.breakdown li:last-child {
  border-bottom: 0;
  padding-bottom: 0;
}

.tally {
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
</style>
