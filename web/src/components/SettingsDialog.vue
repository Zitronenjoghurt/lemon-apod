<script lang="ts" setup>
import { computed, ref } from 'vue'
import { useConfirm } from 'primevue/useconfirm'
import { useToast } from 'primevue/usetoast'
import ReadProgress from './ReadProgress.vue'
import { useCoverage } from '@/composables/useCoverage'
import { useExternalLinks } from '@/composables/useExternalLinks'
import { useFavorites } from '@/composables/useFavorites'
import { usePreferences, WEEK_STARTS } from '@/composables/usePreferences'
import { useRead } from '@/composables/useRead'
import { BackupError, type ImportMode, useSiteData } from '@/composables/useSiteData'
import { useStatus } from '@/composables/useStatus'
import { useWelcome } from '@/composables/useWelcome'

const open = defineModel<boolean>('visible', { default: false })

const confirm = useConfirm()
const toast = useToast()

const { count: favoriteCount, clear: clearFavorites } = useFavorites()
const { count: readCount, clear: clearRead, countIn } = useRead()
const { acknowledged, reset: resetWarning } = useExternalLinks()
const { dismissed: welcomeGone, reset: resetWelcome } = useWelcome()
const { weekStart } = usePreferences()
const { download, restore } = useSiteData()
const { entries } = useStatus()
const coverage = useCoverage()

const file = ref<HTMLInputElement>()
const mode = ref<ImportMode>('merge')

const MODES: { label: string; value: ImportMode }[] = [
  { label: 'Merge', value: 'merge' },
  { label: 'Replace', value: 'replace' },
]

const archiveTotal = computed(() => coverage.total.value || entries.value)

function exportData() {
  try {
    const name = download()
    toast.add({
      severity: 'success',
      summary: 'Backup saved',
      detail: name,
      life: 3000,
    })
  } catch {
    toast.add({
      severity: 'error',
      summary: 'Could not save the backup',
      detail: 'Your browser blocked the download.',
      life: 4000,
    })
  }
}

function pickFile() {
  file.value?.click()
}

async function onFile(event: Event) {
  const input = event.target as HTMLInputElement
  const chosen = input.files?.[0]
  input.value = ''
  if (!chosen) return

  let text: string
  try {
    text = await chosen.text()
  } catch {
    toast.add({ severity: 'error', summary: 'Could not read that file', life: 4000 })
    return
  }

  if (mode.value === 'replace') {
    confirm.require({
      header: 'Replace everything in this browser?',
      message:
        'Your favorites and read progress here are thrown away and the backup takes their place. There is no way back.',
      icon: 'pi pi-exclamation-triangle',
      rejectProps: { label: 'Cancel', severity: 'secondary', outlined: true },
      acceptProps: { label: 'Replace', severity: 'danger' },
      accept: () => run(text),
    })
    return
  }

  run(text)
}

function run(text: string) {
  try {
    const summary = restore(text, mode.value)
    toast.add({
      severity: 'success',
      summary: mode.value === 'merge' ? 'Backup merged in' : 'Backup restored',
      detail: summary.changes.join(', ') || 'Nothing changed.',
      life: 4000,
    })
  } catch (thrown) {
    toast.add({
      severity: 'error',
      summary: 'That backup would not load',
      detail: thrown instanceof BackupError ? thrown.message : 'Something went wrong.',
      life: 5000,
    })
  }
}

function confirmClearRead() {
  const marked = readCount.value

  confirm.require({
    header: 'Forget what you have read?',
    message: `This marks all ${marked.toLocaleString()} entries unread again in this browser. There is no way back`,
    icon: 'pi pi-exclamation-triangle',
    rejectProps: { label: 'Cancel', severity: 'secondary', outlined: true },
    acceptProps: { label: 'Mark all unread', severity: 'danger' },
    accept: () => {
      clearRead()
      toast.add({ severity: 'success', summary: 'Read state cleared', life: 2500 })
    },
  })
}

function confirmClearFavorites() {
  const saved = favoriteCount.value

  confirm.require({
    header: 'Remove every favorite?',
    message: `This drops all ${saved.toLocaleString()} saved entries in this browser. There is no way back.`,
    icon: 'pi pi-exclamation-triangle',
    rejectProps: { label: 'Cancel', severity: 'secondary', outlined: true },
    acceptProps: { label: 'Remove them all', severity: 'danger' },
    accept: () => {
      clearFavorites()
      toast.add({ severity: 'success', summary: 'Favorites cleared', life: 2500 })
    },
  })
}

function bringBackWarning() {
  resetWarning()
  toast.add({
    severity: 'secondary',
    summary: 'Link warning is turned back on',
    detail: 'The next link outside of the archive will ask again.',
    life: 3000,
  })
}

function bringBackWelcome() {
  resetWelcome()
  toast.add({
    severity: 'secondary',
    summary: 'Welcome note is back',
    detail: 'You will find it at the top of the start page.',
    life: 3000,
  })
}
</script>

<template>
  <Dialog
    v-model:visible="open"
    :style="{ width: 'min(38rem, 94vw)' }"
    dismissable-mask
    header="Settings"
    modal
  >
    <div class="stack panels">
      <section class="stack panel">
        <h3>Backup</h3>
        <p class="muted">
          Clearing site data or switching machines loses your settings, reads and favorites.
        </p>

        <div class="row controls">
          <Button icon="pi pi-download" label="Export" outlined @click="exportData" />
          <Button icon="pi pi-upload" label="Import" outlined @click="pickFile" />
          <SelectButton
            v-model="mode"
            :allow-empty="false"
            :options="MODES"
            aria-labelledby="import-mode-label"
            option-label="label"
            option-value="value"
            size="small"
          />
          <span id="import-mode-label" class="sr-only">What an import does</span>
        </div>

        <p class="muted hint">
          <template v-if="mode === 'merge'">
            Merging keeps what is here and adds whatever the backup has on top.
          </template>
          <template v-else>
            Replacing throws away what is here first. It asks before it does.
          </template>
        </p>

        <input
          ref="file"
          accept="application/json,.json"
          class="sr-only"
          type="file"
          @change="onFile"
        />
      </section>

      <section class="stack panel">
        <h3>Calendar</h3>
        <div class="row controls">
          <span class="muted">Start of the week</span>
          <SelectButton
            v-model="weekStart"
            :allow-empty="false"
            :options="WEEK_STARTS"
            aria-label="Day the week starts on"
            option-label="label"
            option-value="value"
            size="small"
          />
        </div>
      </section>

      <section class="stack panel">
        <h3>Reading</h3>
        <ReadProgress :read="countIn()" :total="archiveTotal" label="the archive" />
        <div class="row controls">
          <Button
            :disabled="!readCount"
            icon="pi pi-eraser"
            label="Mark everything unread"
            outlined
            severity="danger"
            size="small"
            @click="confirmClearRead"
          />
        </div>
      </section>

      <section class="stack panel">
        <h3>Favorites</h3>
        <p class="muted">
          {{ favoriteCount.toLocaleString() }}
          {{ favoriteCount === 1 ? 'entry saved' : 'entries saved' }}.
        </p>
        <div class="row controls">
          <Button
            :disabled="!favoriteCount"
            icon="pi pi-trash"
            label="Remove all favorites"
            outlined
            severity="danger"
            size="small"
            @click="confirmClearFavorites"
          />
        </div>
      </section>

      <section class="stack panel">
        <h3>External links</h3>
        <p class="muted">
          <template v-if="acknowledged">
            You have told the site to stop warning you before following an external link.
          </template>
          <template v-else>
            The first link you follow outside of the archive will warn you that we have not checked
            that where it leads is safe.
          </template>
        </p>
        <div v-if="acknowledged" class="row controls">
          <Button
            icon="pi pi-shield"
            label="Warn me again"
            outlined
            severity="secondary"
            size="small"
            @click="bringBackWarning"
          />
        </div>
      </section>

      <section v-if="welcomeGone" class="stack panel">
        <h3>Welcome note</h3>
        <p class="muted">You have dismissed the short note on the start page.</p>
        <div class="row controls">
          <Button
            icon="pi pi-info-circle"
            label="Show it again"
            outlined
            severity="secondary"
            size="small"
            @click="bringBackWelcome"
          />
        </div>
      </section>
    </div>
  </Dialog>
</template>

<style scoped>
.panels {
  gap: 1.5rem;
}

.panel {
  gap: 0.6rem;
}

h3 {
  font-size: 0.78rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
  font-weight: 600;
}

p {
  margin: 0;
  font-size: 0.9rem;
  text-wrap: pretty;
}

.hint {
  font-size: 0.82rem;
}

.controls {
  gap: 0.5rem;
}
</style>
