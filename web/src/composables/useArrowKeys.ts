import { onMounted, onUnmounted } from 'vue'

const OWNS_ARROWS = '.p-select, .p-selectbutton, .p-paginator, .p-datepicker, [role="listbox"]'
const OVERLAY = '.p-select-overlay, .p-dialog-mask, .p-drawer-mask, .p-popover'

export interface ArrowHandlers {
  left?: () => void
  right?: () => void
  up?: () => void
  down?: () => void
  space?: () => void
  shiftLeft?: () => void
  shiftRight?: () => void
}

const ARROWS = ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown']

export function useArrowKeys(handlers: ArrowHandlers): void {
  function step(event: KeyboardEvent, target: HTMLElement | null): (() => void) | undefined {
    switch (event.key) {
      case 'ArrowLeft':
        return event.shiftKey ? handlers.shiftLeft : handlers.left
      case 'ArrowRight':
        return event.shiftKey ? handlers.shiftRight : handlers.right
      case 'ArrowUp':
        return handlers.up
      case 'ArrowDown':
        return handlers.down
      case ' ':
        return target?.closest?.('button, a, [role="button"]') ? undefined : handlers.space
      default:
        return undefined
    }
  }

  function onKey(event: KeyboardEvent): void {
    if (event.defaultPrevented) return
    if (event.metaKey || event.ctrlKey || event.altKey) return
    if (!ARROWS.includes(event.key) && event.key !== ' ') return

    const target = event.target as HTMLElement | null
    if (target?.isContentEditable) return
    if (target && ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)) return
    if (target?.closest?.(OWNS_ARROWS)) return
    if (document.querySelector(OVERLAY)) return

    const chosen = step(event, target)
    if (!chosen) return

    event.preventDefault()
    chosen()
  }

  onMounted(() => window.addEventListener('keydown', onKey))
  onUnmounted(() => window.removeEventListener('keydown', onKey))
}
