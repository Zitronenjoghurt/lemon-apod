export function measure(url: string): Promise<{ width: number; height: number }> {
  return new Promise((resolve) => {
    const probe = new Image()
    probe.onload = () => resolve({ width: probe.naturalWidth, height: probe.naturalHeight })
    probe.onerror = () => resolve({ width: 0, height: 0 })
    probe.src = url
  })
}
