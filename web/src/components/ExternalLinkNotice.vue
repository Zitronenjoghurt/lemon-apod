<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import { useExternalLinks } from '@/composables/useExternalLinks'

const { pending, follow, dismiss } = useExternalLinks()

const remember = ref(true)

const open = computed({
  get: () => pending.value !== null,
  set: (value: boolean) => {
    if (!value) dismiss()
  },
})

const host = computed(() => {
  if (!pending.value) return ''
  try {
    return new URL(pending.value).hostname
  } catch {
    return pending.value
  }
})

watch(pending, (value) => {
  if (value) remember.value = true
})
</script>

<template>
  <Dialog
    v-model:visible="open"
    :style="{ width: 'min(34rem, 92vw)' }"
    dismissable-mask
    header="You are leaving the archive"
    modal
  >
    <div class="stack body">
      <p>
        This link leads to <strong class="host">{{ host }}</strong
        >. Links inside APOD explanations point wherever the author pointed them at and we have not
        checked that the destination is safe.
      </p>
      <p class="muted">
        A link to a page referenced a long time ago may have moved and could serve different content
        than the post author intended to reference. Continue at your own risk.
      </p>

      <p class="address">{{ pending }}</p>

      <div class="row remember">
        <Checkbox v-model="remember" binary input-id="remember-external" />
        <label for="remember-external">Do not warn me again</label>
      </div>
    </div>

    <template #footer>
      <Button label="Stay here" outlined severity="secondary" @click="dismiss" />
      <Button
        icon="pi pi-external-link"
        icon-pos="right"
        label="Open it anyway"
        @click="follow(remember)"
      />
    </template>
  </Dialog>
</template>

<style scoped>
.body {
  gap: var(--space-3);
}

p {
  margin: 0;
  text-wrap: pretty;
}

.host {
  word-break: break-all;
}

.address {
  font-size: var(--text-sm);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  background: color-mix(in srgb, var(--text) 7%, transparent);
  border-radius: 0.45rem;
  padding: var(--space-2) var(--space-3);
  word-break: break-all;
  color: var(--text-muted);
}

.remember {
  gap: var(--space-2);
}

.remember label {
  font-size: var(--text-sm);
  cursor: pointer;
}
</style>
