<script lang="ts" setup>
withDefaults(defineProps<{ message?: string; busy?: boolean; severity?: 'error' | 'warn' }>(), {
  message: 'Something went wrong.',
  busy: false,
  severity: 'error',
})

defineEmits<{ retry: [] }>()
</script>

<template>
  <Message :closable="false" :severity="severity" class="retry-notice">
    <div class="row">
      <span class="text">{{ message }}</span>
      <Button
        :loading="busy"
        icon="pi pi-refresh"
        label="Try again"
        severity="secondary"
        size="small"
        @click="$emit('retry')"
      />
    </div>
  </Message>
</template>

<style scoped>
.row {
  display: flex;
  align-items: center;
  gap: 0.9rem;
  flex-wrap: wrap;
}

.text {
  flex: 1 1 12rem;
  text-wrap: pretty;
}
</style>
