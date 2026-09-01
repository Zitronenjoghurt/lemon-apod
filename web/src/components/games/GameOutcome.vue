<script lang="ts" setup>
import { ref } from 'vue'
import { useToast } from 'primevue/usetoast'
import { type Band, copyText } from '@/composables/useGames'

const props = defineProps<{
  headline: string
  lines?: string[]
  bands?: Band[]
  meter?: { of: number; label: string }
  share: string
  daily?: boolean
  day?: string
  replayed?: boolean
}>()

const emit = defineEmits<{ again: [] }>()

const toast = useToast()
const copying = ref(false)

async function copy() {
  copying.value = true
  const copied = await copyText(props.share)
  copying.value = false

  toast.add({
    severity: copied ? 'success' : 'warn',
    summary: copied ? 'Copied' : 'Could not copy',
    detail: copied
      ? 'You can share it with your friends :)'
      : 'Select the text below and copy it by hand.',
    life: 2500,
  })
}
</script>

<template>
  <section aria-live="polite" class="card outcome">
    <p class="eyebrow">
      <i :class="['pi', daily ? 'pi-calendar' : 'pi-sync']" aria-hidden="true" />
      <template v-if="daily">Daily result{{ day ? ` · ${day}` : '' }}</template>
      <template v-else>Free play result</template>
    </p>

    <p class="headline">{{ headline }}</p>
    <GameBands v-if="bands?.length" :bands="bands" />

    <p v-for="line in lines" :key="line" class="muted line">{{ line }}</p>

    <div v-if="meter" class="meter">
      <div class="row meter-head">
        <span class="muted">{{ meter.label }}</span>
        <strong>{{ meter.of }}%</strong>
      </div>
      <div class="track">
        <div :style="{ width: `${meter.of}%` }" class="fill" />
      </div>
    </div>

    <div v-if="$slots.default" class="detail">
      <slot />
    </div>

    <div class="row actions">
      <Button
        :icon="daily ? 'pi pi-sync' : 'pi pi-refresh'"
        :label="daily ? 'Free play' : 'Play again'"
        @click="emit('again')"
      />
      <Button
        :loading="copying"
        icon="pi pi-clipboard"
        label="Copy result"
        outlined
        severity="secondary"
        @click="copy"
      />
    </div>

    <p v-if="replayed" class="muted note">You have already played today's game.</p>
    <p v-else-if="daily" class="muted note">
      That was today's puzzle. Free play is unlimited and never counts against your daily streak.
    </p>
  </section>
</template>

<style scoped>
.outcome {
  max-width: 34rem;
  margin-inline: auto;
  padding: var(--space-6) var(--space-6) var(--space-6);
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: var(--space-2);
}

.eyebrow {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  margin: 0;
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.09em;
  font-weight: 600;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.eyebrow i {
  font-size: 0.85em;
}

.headline {
  margin: 0;
  font-size: var(--text-xl);
  font-weight: 650;
  letter-spacing: -0.02em;
  font-variant-numeric: tabular-nums;
  text-wrap: balance;
}

.line {
  margin: 0;
  max-width: 44ch;
  text-wrap: pretty;
}

.detail {
  align-self: stretch;
  text-align: left;
  margin-top: var(--space-1);
}

.meter {
  align-self: stretch;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  font-size: var(--text-sm);
}

.meter-head {
  justify-content: space-between;
}

.track {
  height: 0.5rem;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--text) 10%, transparent);
  overflow: hidden;
}

.fill {
  height: 100%;
  border-radius: var(--radius-pill);
  background: var(--accent);
}

.actions {
  justify-content: center;
  margin-top: var(--space-2);
}

.note {
  margin: var(--space-0) 0 0;
  max-width: 46ch;
  font-size: var(--text-sm);
  text-wrap: pretty;
}
</style>
