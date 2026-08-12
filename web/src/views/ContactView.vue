<script lang="ts" setup>
import { computed, ref } from 'vue'
import { useStatus } from '@/composables/useStatus'
import { MASTODON_URL, REPO_URL } from '@/utils/links'

defineOptions({ name: 'ContactView' })

const ENDPOINT = 'https://api.web3forms.com/submit'

const MAX_MESSAGE = 4000
const MAX_EMAIL = 200

const { contact, loaded } = useStatus()

const accessKey = computed(() => contact.value?.form_key ?? '')
const configured = computed(() => Boolean(accessKey.value))

const address = computed(() => contact.value?.email ?? '')

const TOPICS = [
  { label: 'Something is broken', value: 'Bug report' },
  { label: 'An entry looks wrong', value: 'Parser mistake' },
  { label: 'I noticed an inaccuracy', value: 'Inaccuracy' },
  { label: 'A suggestion', value: 'Suggestion' },
  { label: 'Something else', value: 'Message' },
]

const topic = ref(TOPICS[0]!.value)
const message = ref('')
const email = ref('')
const botcheck = ref(false)

const sending = ref(false)
const sent = ref(false)
const error = ref<string>()

const remaining = computed(() => MAX_MESSAGE - message.value.length)
const tooLong = computed(() => remaining.value < 0)
const emailLooksWrong = computed(() => {
  const value = email.value.trim()
  return value.length > 0 && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value)
})

const canSend = computed(
  () => Boolean(message.value.trim()) && !tooLong.value && !emailLooksWrong.value && !sending.value,
)

const mailto = computed(() => {
  if (!address.value) return ''
  const subject = encodeURIComponent(`APOD Archive: ${topic.value}`)
  const body = encodeURIComponent(message.value)
  return `mailto:${address.value}?subject=${subject}&body=${body}`
})

async function send() {
  if (!canSend.value) return

  sending.value = true
  error.value = undefined

  const reply = email.value.trim()

  try {
    const response = await fetch(ENDPOINT, {
      method: 'POST',
      headers: { 'content-type': 'application/json', accept: 'application/json' },
      body: JSON.stringify({
        access_key: accessKey.value,
        subject: `APOD Archive: ${topic.value}`,
        from_name: 'APOD Archive',
        message: message.value.trim(),
        botcheck: botcheck.value,
        ...(reply ? { email: reply, replyto: reply } : {}),
      }),
    })

    if (!accepted(await body(response), response)) return

    sent.value = true
    message.value = ''
    email.value = ''
  } catch {
    error.value = 'Could not reach the form service. Check your connection and try again.'
  } finally {
    sending.value = false
  }
}

async function body(response: Response): Promise<Record<string, unknown>> {
  try {
    return ((await response.json()) as Record<string, unknown>) ?? {}
  } catch {
    return {}
  }
}

function accepted(parsed: Record<string, unknown>, response: Response): boolean {
  if (parsed.success === true) return true

  if (response.status === 429) {
    error.value = 'That is a lot of messages at once. Give it a minute and try again.'
    return false
  }

  const nested = parsed.body as { message?: unknown } | undefined
  const detail = [nested?.message, parsed.message, parsed.error].find(
    (value): value is string => typeof value === 'string' && value.length > 0,
  )

  error.value = detail
    ? `The form service turned it down: ${detail}`
    : 'The form service turned the message down. Try to reach me another way.'
  return false
}

function writeAnother() {
  sent.value = false
  error.value = undefined
}

const elsewhere = computed(() => [
  {
    icon: 'pi pi-github',
    label: 'GitHub',
    value: 'Zitronenjoghurt/lemon-apod',
    href: REPO_URL,
  },
  {
    icon: 'pi pi-at',
    label: 'Mastodon',
    value: '@zitronenjoghurt@mastodon.social',
    href: MASTODON_URL,
  },
  ...(address.value
    ? [
        {
          icon: 'pi pi-envelope',
          label: 'Mail',
          value: address.value,
          href: `mailto:${address.value}`,
        },
      ]
    : []),
])
</script>

<template>
  <div class="stack contact">
    <header class="stack head">
      <h1>Contact</h1>
      <p class="muted lede">
        This archive is a one-person-project and extracting data from the official APOD entries is
        imperfect. If you notice any bugs or other issues, just have something in mind to improve or
        got another reason to message me, you can reach me through the form or the other linked
        platforms.
      </p>
    </header>

    <div class="columns">
      <section class="card panel form">
        <h2 class="muted">Send a message</h2>

        <div v-if="!loaded" class="stack fields">
          <Skeleton height="2.5rem" />
          <Skeleton height="9rem" />
        </div>

        <template v-else-if="!configured">
          <p class="note">
            No form service is configured on this deployment, please reach out to me through the
            other platforms.
          </p>
          <a v-if="address" :href="`mailto:${address}`" class="plain" data-ours>
            <Button :label="address" icon="pi pi-envelope" outlined severity="secondary" />
          </a>
          <p v-else class="muted note">
            No address is published either. The links on the right still reach me.
          </p>
        </template>

        <template v-else-if="sent">
          <Message :closable="false" severity="success">
            I received your message. If you specified your E-Mail I will try to respond ASAP.
          </Message>
          <div class="row">
            <Button
              icon="pi pi-pencil"
              label="Write another"
              outlined
              severity="secondary"
              size="small"
              @click="writeAnother"
            />
          </div>
        </template>

        <form v-else class="stack fields" @submit.prevent="send">
          <div class="stack field">
            <label for="contact-topic">What is this about?</label>
            <Select
              v-model="topic"
              :options="TOPICS"
              input-id="contact-topic"
              option-label="label"
              option-value="value"
            />
          </div>

          <div class="stack field">
            <label for="contact-message">Message</label>
            <Textarea
              id="contact-message"
              v-model="message"
              :invalid="tooLong"
              :rows="7"
              auto-resize
              required
            />
            <p :class="{ over: tooLong }" class="muted counter">
              {{ tooLong ? `${-remaining} over the limit` : `${remaining} characters left` }}
            </p>
          </div>

          <div class="stack field">
            <label for="contact-email">Your email <span class="muted">(optional)</span></label>
            <InputText
              id="contact-email"
              v-model="email"
              :invalid="emailLooksWrong"
              :maxlength="MAX_EMAIL"
              autocomplete="email"
              inputmode="email"
              type="email"
            />
            <p class="muted counter">
              {{
                emailLooksWrong
                  ? 'This is not a valid E-Mail address.'
                  : 'Only needed if you want an answer. It will not be stored on the server, only referenced in the message that reaches me.'
              }}
            </p>
          </div>

          <label aria-hidden="true" class="honeypot">
            <input v-model="botcheck" autocomplete="off" tabindex="-1" type="checkbox" />
          </label>

          <Message v-if="error" :closable="false" severity="warn">
            {{ error }}
            <a v-if="mailto" :href="mailto" class="fallback" data-ours>Mail it instead</a>
          </Message>

          <div class="row actions">
            <Button
              :disabled="!canSend"
              :loading="sending"
              icon="pi pi-send"
              label="Send"
              type="submit"
            />
            <span class="muted hint">
              {{ address ? `Goes to ${address}` : 'Goes straight to my inbox' }}
            </span>
          </div>
        </form>
      </section>

      <section class="card panel">
        <h2 class="muted">Reach out elsewhere</h2>

        <ul class="links">
          <li v-for="link in elsewhere" :key="link.href">
            <a
              :href="link.href"
              :rel="link.href.startsWith('mailto:') ? undefined : 'me noopener'"
              :target="link.href.startsWith('mailto:') ? undefined : '_blank'"
              data-ours
            >
              <i :class="link.icon" aria-hidden="true" />
              <span class="text">
                <span class="muted label">{{ link.label }}</span>
                <span class="value">{{ link.value }}</span>
              </span>
            </a>
          </li>
        </ul>

        <p class="muted note">
          The archive's contents belong to NASA and the people credited on each entry. Anything
          wrong with a picture or its text is worth taking up with
          <a
            data-ours
            href="https://apod.nasa.gov/apod/lib/about_apod.html"
            rel="noopener"
            target="_blank"
          >
            APOD itself</a
          >. Anything wrong with how this site shows them is on me.
        </p>
      </section>
    </div>
  </div>
</template>

<style scoped>
.contact {
  max-width: 58rem;
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

.columns {
  display: grid;
  gap: var(--gap);
  grid-template-columns: minmax(0, 1.6fr) minmax(0, 1fr);
  align-items: start;
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
  font-weight: 600;
}

.fields {
  gap: 1rem;
}

.field {
  gap: 0.35rem;
}

.field label {
  font-size: 0.88rem;
  font-weight: 550;
}

.counter {
  margin: 0;
  font-size: 0.78rem;
}

.counter.over {
  color: var(--p-message-error-color, #dc2626);
}

.honeypot {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}

.actions {
  gap: 0.75rem;
  margin-top: 0.2rem;
}

.hint {
  font-size: 0.8rem;
}

.fallback {
  margin-left: 0.4rem;
  white-space: nowrap;
}

.links {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.links a {
  display: flex;
  align-items: center;
  gap: 0.7rem;
  padding: 0.55rem 0.6rem;
  border-radius: 0.6rem;
  text-decoration: none;
  color: inherit;
}

.links a:hover {
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.links i {
  font-size: 1.05rem;
  color: var(--accent);
  width: 1.3rem;
  text-align: center;
  flex: none;
}

.text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.label {
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  line-height: 1.4;
}

.value {
  font-size: 0.88rem;
  overflow-wrap: anywhere;
}

.note {
  margin: 0;
  font-size: 0.82rem;
  text-wrap: pretty;
  margin-top: auto;
}

.plain {
  text-decoration: none;
  color: inherit;
  display: inline-flex;
}

@media (max-width: 48rem) {
  .columns {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
