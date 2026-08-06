const CREATIVE_COMMONS =
  /creativecommons\.org\/(?:licenses|publicdomain)\/([a-z-]+)(?:\/([\d.]+))?/i

const RIGHTS_SUFFIX = /\s*&\s*(?:copyright|licen[cs]e)s?$/i
const CREDIT_SUFFIX = /\s+credits?$/i

export function licenseName(url: string): string {
  const match = url.match(CREATIVE_COMMONS)
  const terms = match?.[1]
  if (!terms) return 'Licensed'

  const version = match[2]
  if (terms === 'zero') return 'CC0'
  if (terms === 'mark') return 'Public Domain Mark'

  const name = `CC ${terms.toUpperCase()}`
  return version ? `${name} ${version}` : name
}

export function roleLabel(role: string): string {
  const withoutRights = role.replace(RIGHTS_SUFFIX, '').trim()
  const withoutCredit = withoutRights.replace(CREDIT_SUFFIX, '').trim()
  return withoutCredit || withoutRights || role
}
