<script lang="ts" setup>
import { computed } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import type { Gap } from '@/api/types'
import { useArrowKeys } from '@/composables/useArrowKeys'
import { archivePath, formatDate, formatMonth } from '@/utils/date'

const props = defineProps<{ gap: Gap }>()

const router = useRouter()

useArrowKeys({
  left: () => props.gap.previous && void router.push(`/${props.gap.previous}`),
  right: () => props.gap.next && void router.push(`/${props.gap.next}`),
})

const span = computed(() =>
  props.gap.days === 1
    ? formatDate(props.gap.date)
    : `${formatDate(props.gap.from)} to ${formatDate(props.gap.to)}`,
)
</script>

<template>
  <article class="entry gap">
    <header class="head">
      <div class="row justify">
        <RouterLink
          v-tooltip.bottom="`Open ${formatMonth(gap.date)} in the archive`"
          :to="archivePath(gap.date)"
          class="muted when"
        >
          <time :datetime="gap.date">{{ formatDate(gap.date) }}</time>
          <i aria-hidden="true" class="pi pi-calendar" />
        </RouterLink>

        <nav aria-label="Adjacent days" class="row nav">
          <RouterLink v-if="gap.previous" v-slot="{ navigate }" :to="`/${gap.previous}`" custom>
            <Button
              v-tooltip.bottom="`Previous entry, ${formatDate(gap.previous)} (←)`"
              aria-label="Previous entry"
              icon="pi pi-chevron-left"
              outlined
              rounded
              severity="secondary"
              @click="navigate"
            />
          </RouterLink>
          <RouterLink v-if="gap.next" v-slot="{ navigate }" :to="`/${gap.next}`" custom>
            <Button
              v-tooltip.bottom="`Next entry, ${formatDate(gap.next)} (→)`"
              aria-label="Next entry"
              icon="pi pi-chevron-right"
              outlined
              rounded
              severity="secondary"
              @click="navigate"
            />
          </RouterLink>
        </nav>
      </div>

      <p class="muted kicker">
        <i aria-hidden="true" class="pi pi-calendar-times" />
        <template v-if="gap.days === 1">No picture this day</template>
        <template v-else>No picture for {{ gap.days }} days</template>
      </p>
      <h1 class="title">{{ gap.title }}</h1>
      <p v-if="gap.days > 1" class="muted span">{{ span }}</p>
    </header>

    <div class="body">
      <p v-for="(line, at) in gap.paragraphs" :key="at">{{ line }}</p>

      <p v-if="gap.caveat" class="care">{{ gap.caveat }}</p>

      <p v-if="gap.source" class="source">
        <a :href="gap.source.url" rel="noopener" target="_blank">
          {{ gap.source.label }}
          <i aria-hidden="true" class="pi pi-external-link" />
        </a>
      </p>

      <p class="muted lead">
        You can see all of the missed days and other statistics on the
        <RouterLink to="/stats">statistics page</RouterLink>.
      </p>
    </div>
  </article>
</template>

<style scoped>
.gap {
  display: flex;
  flex-direction: column;
  gap: 1.1rem;
}

.head {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.justify {
  justify-content: space-between;
  align-items: center;
  gap: 0.75rem;
}

.when {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.9rem;
  text-decoration: none;
}

.when:hover {
  color: var(--text);
}

.nav {
  gap: 0.4rem;
}

.kicker {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin: 0.2rem 0 0;
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.07em;
}

.kicker i {
  color: var(--accent);
}

.title {
  font-size: 1.7rem;
  text-wrap: balance;
}

.span {
  margin: 0;
  font-size: 0.9rem;
  font-variant-numeric: tabular-nums;
}

.body {
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
  max-width: 68ch;
}

.body p {
  margin: 0;
  line-height: 1.65;
  text-wrap: pretty;
}

.care {
  padding-left: 0.9rem;
  border-left: 2px solid color-mix(in srgb, var(--accent) 45%, var(--border));
  color: var(--text-muted);
}

.source a {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.9rem;
  text-decoration: none;
}

.source a:hover {
  text-decoration: underline;
}

.source i {
  font-size: 0.75em;
}

.lead {
  margin-top: 0.2rem;
  font-size: 0.9rem;
}
</style>
