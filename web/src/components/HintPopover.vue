<script lang="ts" setup>
import { useTemplateRef } from 'vue'

defineProps<{ label: string }>()
defineOptions({ inheritAttrs: false })

const popover = useTemplateRef<{ toggle: (event: Event) => void }>('popover')
</script>

<template>
  <button :aria-label="label" class="hint" type="button" @click="popover?.toggle($event)">
    <i aria-hidden="true" class="pi pi-question-circle" />
  </button>

  <Popover ref="popover">
    <div class="hint-body">
      <slot />
    </div>
  </Popover>
</template>

<style scoped>
.hint {
  display: inline-grid;
  place-items: center;
  flex: none;
  padding: 0.45rem;
  margin: -0.45rem;
  border: 0;
  background: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 1em;
  line-height: 1;
}

.hint .pi {
  font-size: 1em;
}

.hint:hover,
.hint:focus-visible {
  color: var(--accent);
}

.hint-body {
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
  max-width: 22rem;
  font-size: 0.85rem;
}

.hint-body :slotted(p) {
  margin: 0;
  text-wrap: pretty;
  line-height: 1.45;
}
</style>
