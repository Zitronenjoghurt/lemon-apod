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

const reminders = computed(() => acknowledged.value || welcomeGone.value)

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
    :style="{ width: 'min(34rem, 94vw)' }"
    class="settings"
    dismissable-mask
    header="Settings"
    modal
  >
    <div class="panels">
      <section class="panel">
        <h3><i aria-hidden="true" class="pi pi-book" />Reading</h3>
        <ReadProgress :read="countIn()" :total="archiveTotal" label="the archive" />
        <div class="rows">
          <div class="line">
            <span class="name">Read</span>
            <span class="value">{{ readCount.toLocaleString() }}</span>
            <Button
              :disabled="!readCount"
              class="act"
              icon="pi pi-eraser"
              label="Mark all unread"
              outlined
              severity="danger"
              size="small"
              @click="confirmClearRead"
            />
          </div>
        </div>
      </section>

      <section class="panel">
        <h3><i aria-hidden="true" class="pi pi-star" />Favorites</h3>
        <div class="rows">
          <div class="line">
            <span class="name">Saved</span>
            <span class="value">{{ favoriteCount.toLocaleString() }}</span>
            <Button
              :disabled="!favoriteCount"
              class="act"
              icon="pi pi-trash"
              label="Remove all"
              outlined
              severity="danger"
              size="small"
              @click="confirmClearFavorites"
            />
          </div>
        </div>
      </section>

      <section class="panel">
        <h3><i aria-hidden="true" class="pi pi-calendar" />Calendar</h3>
        <div class="rows">
          <div class="line">
            <span class="name">Week starts on</span>
            <SelectButton
              v-model="weekStart"
              :allow-empty="false"
              :options="WEEK_STARTS"
              aria-label="Day the week starts on"
              class="act"
              option-label="label"
              option-value="value"
              size="small"
            />
          </div>
        </div>
      </section>

      <section class="panel">
        <h3><i aria-hidden="true" class="pi pi-database" />Backup</h3>
        <p class="lead muted">All your data is stored solely in this browser.</p>
        <div class="rows">
          <div class="line">
            <span class="name">Save a copy</span>
            <Button
              class="act"
              icon="pi pi-download"
              label="Export"
              outlined
              severity="secondary"
              size="small"
              @click="exportData"
            />
          </div>

          <div class="line">
            <span class="name">Load a copy</span>
            <span class="act pair">
              <SelectButton
                v-model="mode"
                :allow-empty="false"
                :options="MODES"
                aria-label="What an import does"
                option-label="label"
                option-value="value"
                size="small"
              />
              <Button
                :title="
                  mode === 'merge'
                    ? 'Keeps what exists and adds the backup on top'
                    : 'Throws away what exists and replaces it with the backup'
                "
                icon="pi pi-upload"
                label="Import"
                outlined
                severity="secondary"
                size="small"
                @click="pickFile"
              />
            </span>
          </div>
        </div>

        <input
          ref="file"
          accept="application/json,.json"
          class="sr-only"
          type="file"
          @change="onFile"
        />
      </section>

      <section v-if="reminders" class="panel">
        <h3><i aria-hidden="true" class="pi pi-bell" />Dismissed notices</h3>
        <div class="rows">
          <div v-if="acknowledged" class="line">
            <span class="name">Warning before external links</span>
            <Button
              class="act"
              icon="pi pi-shield"
              label="Warn me again"
              outlined
              severity="secondary"
              size="small"
              @click="bringBackWarning"
            />
          </div>

          <div v-if="welcomeGone" class="line">
            <span class="name">Welcome note</span>
            <Button
              class="act"
              icon="pi pi-info-circle"
              label="Show it again"
              outlined
              severity="secondary"
              size="small"
              @click="bringBackWelcome"
            />
          </div>
        </div>
      </section>
    </div>
  </Dialog>
</template>

<style scoped>
.panels {
  display: flex;
  flex-direction: column;
  gap: 1.4rem;
}

.panel {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

h3 {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  margin: 0;
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.07em;
  color: var(--text-muted);
  font-weight: 600;
}

h3 i {
  font-size: 0.85em;
}

.lead {
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.rows {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--bg-elevated) 55%, transparent);
}

.line {
  display: flex;
  align-items: center;
  gap: 0.5rem 0.75rem;
  padding: 0.6rem 0.75rem;
  min-height: 3rem;
}

.line + .line {
  border-top: 1px solid var(--border);
}

.name {
  font-size: 0.92rem;
  min-width: 0;
}

.value {
  font-size: 0.92rem;
  font-variant-numeric: tabular-nums;
  color: var(--text-muted);
}

.act {
  margin-left: auto;
  flex: none;
}

.pair {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}

@media (max-width: 26rem) {
  .line {
    flex-wrap: wrap;
  }

  .pair {
    justify-content: flex-end;
  }
}
</style>
