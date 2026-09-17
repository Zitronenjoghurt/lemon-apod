<script lang="ts" setup>
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'
import FieldChange from './FieldChange.vue'
import type { FieldDivergence } from '@/api/types'

const props = defineProps<{
  changed: FieldDivergence[]
  absent: boolean
}>()

const open = ref(false)

const FIELD_NAMES: Record<string, string> = {
  title: 'the title',
  explanation_text: 'the explanation',
  credit_text: 'the credit',
  has_copyright: 'the copyright note',
  license_url: 'the licence link',
  tomorrow_teaser: "tomorrow's teaser",
  media_kind: 'the file format',
}

const changedNames = computed(() =>
  props.changed.map((row) => FIELD_NAMES[row.field] ?? row.field.replace(/_/g, ' ')),
)

const lead = computed(() =>
  props.absent
    ? { icon: 'absent', text: "Missing from APOD's modernized site" }
    : { icon: 'compare', text: "Changed through APOD's modernization" },
)
</script>

<template>
  <div class="rail migration">
    <button :aria-expanded="open" class="row migrated" type="button" @click="open = !open">
      <span class="lead">
        <AppIcon :name="lead.icon" />
        {{ lead.text }}
      </span>
      <span v-if="changed.length" class="row fields">
        <span v-for="name in changedNames" :key="name" class="what">{{ name }}</span>
      </span>
      <AppIcon name="chevron-down" class="turn" />
    </button>

    <Transition name="unfold">
      <div v-if="open" class="unfold-shell">
        <div class="differences">
          <p v-if="absent" class="gone">
            NASA's modernized site has no page for this date. What you are reading was archived from
            the legacy page.
          </p>
          <FieldChange v-for="row in changed" :key="row.field" :row="row" />
          <RouterLink class="about" to="/modernization">
            More information about the modernization of the official APOD website
            <AppIcon name="arrow-right" />
          </RouterLink>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.migrated {
  width: 100%;
  gap: var(--space-2) var(--space-3);
  justify-content: space-between;
  padding: 0;
  border: 0;
  background: none;
  color: inherit;
  font: inherit;
  font-size: inherit;
  text-align: left;
  cursor: pointer;
  transition: color var(--dur-fast) var(--ease-out);
}

.migrated:hover,
.migrated:focus-visible {
  color: var(--text);
}

.turn {
  margin-left: auto;
  font-size: 0.8em;
  transition: transform var(--dur-base) var(--ease-out);
}

.migrated[aria-expanded='true'] .turn {
  transform: rotate(180deg);
}

.lead {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  white-space: nowrap;
}

.fields {
  gap: var(--space-0);
  flex-wrap: wrap;
  align-items: center;
}

.what {
  padding: var(--space-0) var(--space-2);
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--text) 8%, transparent);
}

.differences {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin-top: var(--space-3);
  padding-top: var(--space-3);
  border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
}

.unfold-enter-active,
.unfold-leave-active {
  overflow: hidden;
  transition:
    height var(--dur-base) var(--ease-out),
    opacity var(--dur-base) var(--ease-out);
}

.unfold-enter-from,
.unfold-leave-to {
  height: 0;
  opacity: 0;
}

.unfold-enter-to,
.unfold-leave-from {
  height: auto;
}

.gone {
  margin: 0;
  font-size: var(--text-sm);
}

.about {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  width: fit-content;
  font-size: var(--text-xs);
  text-decoration: none;
}

.about .icon {
  font-size: 0.8em;
}
</style>
