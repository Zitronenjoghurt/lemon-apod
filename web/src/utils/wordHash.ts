const OFFSET = 0xcbf29ce484222325n
const PRIME = 0x100000001b3n
const MASK = 0xffffffffffffffffn

export function hash64(bytes: Uint8Array): bigint {
  let hash = OFFSET
  for (const byte of bytes) {
    hash = (hash ^ BigInt(byte)) & MASK
    hash = (hash * PRIME) & MASK
  }
  return hash
}

export function normaliseWord(raw: string): string {
  const first =
    raw
      .trim()
      .toLowerCase()
      .replace(/’/g, "'")
      .split(/[^\p{L}\p{N}'-]+/u)[0] ?? ''
  const trimmed = first.replace(/^[-']+|[-']+$/g, '')
  return /\p{L}/u.test(trimmed) ? trimmed : ''
}

export function wordHash(salt: string, word: string): string {
  const value = BigInt(`0x${salt || '0'}`) & MASK
  const bytes = new Uint8Array(8 + word.length * 4)

  for (let index = 0; index < 8; index++) {
    bytes[index] = Number((value >> BigInt(8 * (7 - index))) & 0xffn)
  }

  const encoded = new TextEncoder().encode(word)
  bytes.set(encoded, 8)

  return hash64(bytes.subarray(0, 8 + encoded.length)).toString(16)
}
