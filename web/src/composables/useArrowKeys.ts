import { onMounted, onUnmounted } from 'vue'

const OWNS_ARROWS = '.p-select, .p-selectbutton, .p-paginator, .p-datepicker, [role="listbox"]'
const OVERLAY = '.p-select-overlay, .p-dialog-mask, .p-drawer-mask, .p-popover'

export interface ArrowHandlers {
  left?: () => void
  right?: () => void
  shiftLeft?: () => void
  shiftRight?: () => void
}

export function useArrowKeys(handlers: ArrowHandlers): void {
  function onKey(event: KeyboardEvent): void {
    if (event.defaultPrevented) return
    if (event.metaKey || event.ctrlKey || event.altKey) return
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return

    const target = event.target as HTMLElement | null
    if (target?.isContentEditable) return
    if (target && ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)) return
    if (target?.closest?.(OWNS_ARROWS)) return
    if (document.querySelector(OVERLAY)) return

    const back = event.key === 'ArrowLeft'
    const step = event.shiftKey
      ? handlers[back ? 'shiftLeft' : 'shiftRight']
      : handlers[back ? 'left' : 'right']
    if (!step) return

    event.preventDefault()
    step()
  }

  onMounted(() => window.addEventListener('keydown', onKey))
  onUnmounted(() => window.removeEventListener('keydown', onKey))
}
