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
          <span v-if="coverage.absent" :style="{ flexGrow: coverage.absent }" class="seg absent" />
          <span
            v-if="coverage.unchecked"
            :style="{ flexGrow: coverage.unchecked }"
            class="seg unchecked"
          />
        </div>
        <p class="muted note">
          {{ count(checked) }} of {{ count(dated) }} dates checked, {{ count(coverage.absent) }} of
          them missing from the modern page.
        </p>
      </section>
    </template>
  </div>
</template>

<style scoped>
.modernization {
  max-width: 62rem;
  margin-inline: auto;
  gap: var(--space-6);
}

h1 {
  font-size: var(--text-xl);
}

h2 {
  font-size: var(--text-md);
  margin: 0;
}

.tiles {
  display: grid;
  gap: var(--gap);
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 13rem), 1fr));
}

.tile {
  padding: var(--space-4) var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-0);
}

.tile .name {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.tile .value {
  font-size: var(--text-xl);
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.02em;
  line-height: 1.2;
}

.tile .foot {
  font-size: var(--text-sm);
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
  font-size: var(--text-sm);
}

.dates {
  list-style: none;
  margin: 0;
  padding: 0;
  gap: var(--space-2) var(--space-4);
  flex-wrap: wrap;
  font-size: var(--text-sm);
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

.breakdown {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.breakdown li {
  display: flex;
  justify-content: space-between;
  gap: var(--space-4);
  padding-bottom: var(--space-1);
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
