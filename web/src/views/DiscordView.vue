<script lang="ts" setup>
import { useStatus } from '@/composables/useStatus'
import { APOD_URL } from '@/utils/links'

defineOptions({ name: 'DiscordView' })

const { botInvite, botUserInstall, loaded } = useStatus()

const commands = [
  { name: '/apod today', hint: "Today's Astronomy Picture of the Day." },
  { name: '/apod date', hint: 'The Astronomy Picture of the Day for a specific date.' },
  {
    name: '/apod random',
    hint: 'A random Astronomy Picture of the Day.',
  },
  {
    name: '/apod search',
    hint: 'A full-text search across every Astronomy Picture of the Day.',
  },
  {
    name: '/apod dm',
    hint: 'Get each new entry as a direct message.',
  },
  {
    name: '/apod settings',
    hint: 'Configure the daily announcement (server manager only).',
  },
  {
    name: '/apod announce',
    hint: "Force-send today's entry: into the configured channel, or to your DMs if you run it there.",
  },
]

const setup = [
  {
    icon: 'pi pi-plus-circle',
    title: 'Add the bot to your server',
    body: 'You need the Manage Server permission to add and manage the bot.',
  },
  {
    icon: 'pi pi-cog',
    title: 'Point it at a channel',
    body: 'Run /apod settings, set announce to true and specify a channel. You can also add a message or adjust the explanation length.',
  },
  {
    icon: 'pi pi-send',
    title: 'Force post or wait',
    body: "Run /apod announce to force-post today's entry in the configured channel immediately. Otherwise it will arrive every day once it becomes available.",
  },
  {
    icon: 'pi pi-envelope',
    title: 'Or get it in your DMs',
    body: 'Use the lookup commands or run /apod dm subscribe:true and each new entry arrives as a direct message. Discord only lets the bot message you if you share a server with it and your privacy settings allow it. The first announcement is sent right away as a test.',
  },
]
</script>

<template>
  <div class="stack discord">
    <header class="stack head">
      <h1>Discord bot</h1>
    </header>

    <section class="card panel invite">
      <div class="stack pitch">
        <h2 class="muted">Add it</h2>
        <p class="note">
          A discord bot that posts each new
          <a :href="APOD_URL" rel="noopener" target="_blank">Astronomy Picture of the Day</a> into a
          configured channel or your DMs. It also lets users lookup past entries.
        </p>
      </div>

      <Skeleton v-if="!loaded" height="2.6rem" width="21rem" />

      <div v-else-if="botInvite || botUserInstall" class="buttons">
        <a v-if="botInvite" :href="botInvite" class="plain" rel="noopener" target="_blank">
          <Button icon="pi pi-discord" label="Add to a server" size="large" />
        </a>
        <a
          v-if="botUserInstall"
          :href="botUserInstall"
          class="plain"
          rel="noopener"
          target="_blank"
        >
          <Button icon="pi pi-user" label="Add to your DMs" size="large" />
        </a>
      </div>

      <p v-else class="muted note">No bot is configured on this deployment.</p>
    </section>

    <section class="card panel">
      <h2 class="muted">Setup</h2>
      <ol class="steps">
        <li v-for="step in setup" :key="step.title">
          <i :class="step.icon" aria-hidden="true" />
          <span class="text">
            <span class="value">{{ step.title }}</span>
            <span class="muted label">{{ step.body }}</span>
          </span>
        </li>
      </ol>
    </section>

    <section class="card panel">
      <h2 class="muted">Commands</h2>
      <ul class="items">
        <li v-for="command in commands" :key="command.name">
          <code class="name">{{ command.name }}</code>
          <span class="muted label">{{ command.hint }}</span>
        </li>
      </ul>
    </section>
  </div>
</template>

<style scoped>
.discord {
  max-width: 52rem;
  margin-inline: auto;
  gap: 1.5rem;
}

h1 {
  font-size: 1.6rem;
}

.head {
  gap: 0.5rem;
}

.lede {
  margin: 0;
  text-wrap: pretty;
}

.panel {
  padding: 1.1rem 1.2rem 1.3rem;
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

.panel h2 {
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.07em;
  margin: 0;
}

.note {
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.invite {
  flex-direction: column;
  align-items: stretch;
  gap: 1.1rem;
}

.buttons {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.6rem;
}

.buttons a {
  display: block;
}

.buttons :deep(.p-button) {
  width: 100%;
  justify-content: center;
  white-space: nowrap;
}

@media (max-width: 26rem) {
  .buttons {
    grid-template-columns: minmax(0, 1fr);
  }
}

.pitch {
  gap: 0.5rem;
}

.steps,
.items {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
}

.steps li {
  display: flex;
  gap: 0.7rem;
  align-items: flex-start;
}

.steps i {
  margin-top: 0.15rem;
  color: var(--text-muted);
}

.text {
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
  min-width: 0;
}

.value {
  font-size: var(--text-sm);
  font-weight: 600;
}

.label {
  font-size: var(--text-xs);
  text-wrap: pretty;
}

.items li {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 0.2rem 0.7rem;
}

.name {
  font-size: var(--text-sm);
}

@media (min-width: 34rem) {
  .items li {
    display: grid;
    grid-template-columns: 9.5rem minmax(0, 1fr);
  }
}
</style>
