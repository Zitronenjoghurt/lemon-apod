<script lang="ts" setup>
import { onMounted, ref } from 'vue'
import IndexRow from '@/components/IndexRow.vue'
import { api } from '@/api/client'

const INDEXES = [
  {
    to: '/objects',
    label: 'Objects',
    unit: 'objects',
    note: 'Most prominent objects featured in APOD.',
  },
  {
    to: '/credits',
    label: 'Credits',
    unit: 'credits',
    note: 'Every person and institution credited for APODs.',
  },
  {
    to: '/pictures',
    label: 'Encores',
    unit: 'pictures',
    note: 'Pictures that have been featured multiple times.',
  },
  {
    to: '/resources',
    label: 'Resources',
    unit: 'resources',
    note: 'Every site every APOD has ever pointed to.',
  },
]

const totals = ref<Record<string, number>>({})

onMounted(() => {
  const asked: [string, Promise<{ total: number }>][] = [
    ['/objects', api.objects({ limit: 1 })],
    ['/credits', api.credits({ limit: 1 })],
    ['/pictures', api.pictures({ limit: 1 })],
    ['/resources', api.resources({ limit: 1 })],
  ]

  for (const [to, request] of asked) {
    request
      .then(({ total }) => {
        totals.value = { ...totals.value, [to]: total }
      })
      .catch(() => {})
  }
})
</script>

<template>
  <div class="stack">
    <h1>Indexes</h1>

    <ul class="stack results">
      <IndexRow v-for="index in INDEXES" :key="index.to" :name="index.label" :to="index.to">
        <template #note>{{ index.note }}</template>
        <template #meta>
          <span v-if="totals[index.to] !== undefined" class="size">
            {{ totals[index.to].toLocaleString() }}
            <span class="muted unit">{{ index.unit }}</span>
          </span>
          <Skeleton v-else height="1.1rem" width="6rem" />
        </template>
      </IndexRow>
    </ul>
  </div>
</template>

<style scoped>
h1 {
  font-size: var(--text-xl);
}

.results {
  list-style: none;
  margin: 0;
  padding: 0;
  gap: var(--space-2);
}

.size {
  font-weight: 600;
  font-size: var(--text-md);
}

.unit {
  font-weight: 400;
  font-size: var(--text-xs);
}
</style>
