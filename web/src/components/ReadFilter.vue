<script lang="ts" setup>
import { useConfirm } from 'primevue/useconfirm'
import { useToast } from 'primevue/usetoast'
import { READ_FILTERS, useRead } from '@/composables/useRead'

const { filter, count, clear } = useRead()
const confirm = useConfirm()
const toast = useToast()

defineProps<{
  hidden?: number
}>()

function confirmClear() {
  const marked = count.value

  confirm.require({
    header: 'Forget what you have read?',
    message: `This marks all ${marked.toLocaleString()} entries unread again in this browser. There is no undo.`,
    icon: 'pi pi-exclamation-triangle',
    rejectProps: { label: 'Cancel', severity: 'secondary', outlined: true },
    acceptProps: { label: 'Mark all unread', severity: 'danger' },
    accept: () => {
      clear()
      toast.add({
        severity: 'success',
        summary: 'Read state cleared',
        detail: `${marked.toLocaleString()} ${marked === 1 ? 'entry' : 'entries'} marked unread.`,
        life: 2500,
      })
    },
  })
}
</script>

<template>
  <div class="read-filter">
    <SelectButton
      v-model="filter"
      :allow-empty="false"
      :options="READ_FILTERS"
      aria-labelledby="read-filter-label"
      option-label="label"
      option-value="value"
      size="small"
    >
      <template #option="{ option }">
        <i :class="option.icon" aria-hidden="true" />
        <span class="label">{{ option.label }}</span>
      </template>
    </SelectButton>
    <span id="read-filter-label" class="sr-only">Filter by read state</span>

    <Button
      v-if="count"
      v-tooltip.bottom="`${count.toLocaleString()} read. Mark them all unread again.`"
      aria-label="Clear read state"
      icon="pi pi-eraser"
      rounded
      severity="secondary"
      size="small"
      text
      @click="confirmClear"
    />

    <span v-if="hidden" aria-live="polite" class="muted hidden-note">
      {{ hidden }} hidden on this page
    </span>
  </div>
</template>

<style scoped>
.read-filter {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.label {
  margin-left: var(--space-1);
}

.hidden-note {
  font-size: var(--text-sm);
  margin-left: var(--space-1);
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}

@media (max-width: 26rem) {
  .label {
    display: none;
  }
}
</style>
