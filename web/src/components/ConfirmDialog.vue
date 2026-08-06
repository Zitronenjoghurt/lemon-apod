<script setup lang="ts">
import { ref, watch } from 'vue'

const props = withDefaults(
  defineProps<{
    open: boolean
    title: string
    message?: string
    confirmLabel?: string
    cancelLabel?: string
  }>(),
  { confirmLabel: 'Confirm', cancelLabel: 'Cancel' },
)

const emit = defineEmits<{ confirm: []; cancel: [] }>()

const dialog = ref<HTMLDialogElement>()

watch(
  () => props.open,
  (open) => {
    if (open) dialog.value?.showModal()
    else dialog.value?.close()
  },
)
</script>

<template>
  <dialog
    ref="dialog"
    class="card confirm"
    @cancel.prevent="emit('cancel')"
    @close="emit('cancel')"
  >
    <h2>{{ title }}</h2>
    <p v-if="message" class="muted">{{ message }}</p>
    <div class="row actions">
      <button type="button" class="button" @click="emit('cancel')">{{ cancelLabel }}</button>
      <button type="button" class="button danger" @click="emit('confirm')">
        {{ confirmLabel }}
      </button>
    </div>
  </dialog>
</template>

<style scoped>
.confirm {
  width: min(24rem, calc(100vw - 2.5rem));
  padding: 1.5rem;
  border: 1px solid var(--border);
  color: var(--text);
}

.confirm::backdrop {
  background: rgb(0 0 0 / 0.45);
  backdrop-filter: blur(2px);
}

h2 {
  font-size: 1.15rem;
  font-weight: 600;
}

p {
  margin: 0.5rem 0 0;
  font-size: 0.92rem;
}

.actions {
  margin-top: 1.25rem;
  justify-content: flex-end;
}

.button {
  font: inherit;
  font-size: 0.9rem;
  padding: 0.4rem 0.9rem;
  border-radius: 0.6rem;
  border: 1px solid var(--border);
  background: var(--bg);
  color: inherit;
  cursor: pointer;
}

.button:hover {
  border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
}

.button.danger {
  border-color: color-mix(in srgb, #ef4444 45%, var(--border));
  color: #ef4444;
}

.button.danger:hover {
  background: color-mix(in srgb, #ef4444 12%, transparent);
  border-color: #ef4444;
}
</style>
