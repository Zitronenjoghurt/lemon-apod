<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import { api } from '@/api/client'
import type { RatingTerms } from '@/api/types'

const visible = defineModel<boolean>('visible', { default: false })

const emit = defineEmits<{ forgot: [number] }>()

const terms = ref<RatingTerms | null>(null)
const confirming = ref(false)
const busy = ref(false)
const forgotten = ref<number | null>(null)

const days = computed(() => terms.value?.cookie_days ?? 90)
const least = computed(() => terms.value?.min_comparisons ?? 8)

watch(
  visible,
  async (open) => {
    if (!open || terms.value) return
    try {
      terms.value = await api.rating.terms()
    } catch {
      terms.value = null
    }
  },
  { immediate: true },
)

async function forget(): Promise<void> {
  confirming.value = false
  busy.value = true
  try {
    const gone = await api.rating.forget()
    forgotten.value = gone.forgotten
    emit('forgot', gone.forgotten)
  } catch {
    forgotten.value = null
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <Dialog
    v-model:visible="visible"
    :style="{ width: 'min(36rem, 94vw)' }"
    dismissable-mask
    header="User votes"
    modal
  >
    <div class="help">
      <section>
        <h3>Purpose</h3>
        <p>
          We are trying to find the astronomy picture people find the most beautiful or fascinating
          as a whole.
        </p>
      </section>

      <section>
        <h3>How it works</h3>
        <p>
          You get asked a question and you only have to vote for one of the two pictures shown to
          you that fits your answer best (or let it be a draw).
        </p>
      </section>

      <section>
        <h3>Reading the numbers</h3>
        <p>
          Each picture shows how often readers picked it over a other entries of archive. The band
          that fills the bar is the uncertainty of the result. It will narrow the more votes come
          in.
        </p>
        <p>
          Eventually pictures will fall into tiers, the higher ones being the most regarded. If
          there are enough votes a single entry might come out alone on top.
        </p>
      </section>

      <section>
        <h3>Your votes</h3>
        <p>
          Voting sets a random cookie. It does not carry anything to identify you and there is no
          authentication. It expires {{ days }} days after your last vote.
        </p>

        <div class="row actions">
          <Button
            :disabled="busy"
            :loading="busy"
            icon="pi pi-trash"
            label="Forget my votes"
            outlined
            severity="secondary"
            size="small"
            @click="confirming = true"
          />
          <span v-if="forgotten !== null" class="muted done">
            <i aria-hidden="true" class="pi pi-check" />
            {{ forgotten }} vote{{ forgotten === 1 ? '' : 's' }} removed.
          </span>
        </div>
      </section>
    </div>
  </Dialog>

  <Dialog
    v-model:visible="confirming"
    :style="{ width: 'min(28rem, 92vw)' }"
    dismissable-mask
    header="Forget your votes"
    modal
  >
    <p class="warn">
      This deletes your voting cookie and every vote it was attached to. The results will correct
      themselves soon. It will be as if you had never voted. It cannot be undone.
    </p>

    <template #footer>
      <Button label="Keep them" severity="secondary" text @click="confirming = false" />
      <Button icon="pi pi-trash" label="Forget them" severity="danger" @click="forget" />
    </template>
  </Dialog>
</template>

<style scoped>
.help {
  display: flex;
  flex-direction: column;
  gap: 1.1rem;
}

h3 {
  margin: 0 0 0.3rem;
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.07em;
  font-weight: 600;
  color: var(--text-muted);
}

p {
  margin: 0 0 0.45rem;
  font-size: 0.9rem;
  line-height: 1.55;
  text-wrap: pretty;
}

p:last-child {
  margin-bottom: 0;
}

.actions {
  gap: 0.6rem;
  flex-wrap: wrap;
  margin-top: 0.7rem;
}

.done {
  font-size: 0.82rem;
}

.warn {
  margin: 0;
  font-size: 0.92rem;
  text-wrap: pretty;
}
</style>
