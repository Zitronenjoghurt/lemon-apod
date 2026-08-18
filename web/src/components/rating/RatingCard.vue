<script lang="ts" setup>
import { computed, onMounted, ref } from 'vue'
import { RouterLink } from 'vue-router'
import RatingProgress from '@/components/rating/RatingProgress.vue'
import { api } from '@/api/client'
import type { Board } from '@/api/types'
import { useRatingCard } from '@/composables/useRating'

const { dismissed, dismiss } = useRatingCard()

const board = ref<Board | null>(null)

onMounted(async () => {
  if (dismissed.value) return
  try {
    board.value = await api.rating.board('beautiful', { limit: 0 })
  } catch {
    board.value = null
  }
})

const votes = computed(() => board.value?.votes.toLocaleString() ?? '0')
const filling = computed(() => board.value?.provisional ?? true)
</script>

<template>
  <section v-if="!dismissed && board" class="card rating-card">
    <div class="body">
      <p class="muted kicker">
        <i aria-hidden="true" class="pi pi-images" />
        Vote for your favorite APOD
      </p>
      <h2>Which picture do users think is the best?</h2>
      <p v-if="filling" class="muted line">
        {{ votes }} votes so far, the results are not significant enough yet.
      </p>
      <p v-else class="muted line">
        {{ votes }} votes in total. The results are significant, more votes will sharpen it further
        though.
      </p>

      <RatingProgress v-if="filling" :progress="board.progress" />

      <div class="row actions">
        <RouterLink v-slot="{ navigate }" custom to="/rating/vote">
          <Button icon="pi pi-images" label="Get voting!" size="small" @click="navigate" />
        </RouterLink>
        <RouterLink v-slot="{ navigate }" custom to="/rating">
          <Button
            icon="pi pi-list"
            label="See the results"
            outlined
            severity="secondary"
            size="small"
            @click="navigate"
          />
        </RouterLink>
      </div>
    </div>

    <Button
      aria-label="Dismiss this card"
      class="close"
      icon="pi pi-times"
      rounded
      severity="secondary"
      text
      @click="dismiss"
    />
  </section>
</template>

<style scoped>
.rating-card {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 1rem 0.9rem 1.1rem 1.2rem;
  border-color: color-mix(in srgb, var(--accent) 30%, var(--border));
}

.body {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  min-width: 0;
  flex: 1 1 auto;
}

.kicker {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin: 0;
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.07em;
}

.kicker i {
  color: var(--accent);
}

h2 {
  font-size: 1.05rem;
  text-wrap: balance;
}

.line {
  margin: 0;
  font-size: 0.9rem;
  text-wrap: pretty;
}

.actions {
  gap: 0.5rem;
  margin-top: 0.15rem;
  flex-wrap: wrap;
}

.close {
  flex: none;
  margin-left: auto;
}
</style>
