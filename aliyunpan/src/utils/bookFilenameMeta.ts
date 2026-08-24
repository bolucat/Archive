import type { IBookItem } from '../types/book'

const AUTHOR_TITLE_RE = /^(.+?)\s*[-–—_]\s*(.+)$/
const BRACKET_AUTHOR_RE = /^[\[【（(](.+?)[\]】）)]\s*(.+)$/
const TITLE_AUTHOR_RE = /^(.+?)\s*[\(（](.+?)[\)）]$/
const COMMON_TAGS_RE = /(?:\[[^\]]+\]|【[^】]+】|（[^）]+）|\([^)]+\))/g
const CATALOG_BEFORE_YEAR_RE = /^0\d{3}(?=(?:19|20)\d{2}\p{Script=Han})/u
const CATALOG_PREFIX_RE = /^(?:(?:0\d{3,}|\d{5,})(?=(?:[.\s_-]|\p{Script=Han})))(?:[.\s_-])*/u
const RELEASE_SUFFIX_RE = /(?:\s*[-–—_]\s*|\s+)(?:(?:19|20)\d{2}\s*(?:更新|新版|修订版|修订|完整版|珍藏版|收藏版|扫描版)|(?:最新版|更新版|修订版|扫描版))$/iu
const ENGLISH_AUTHOR_RE = /^[A-Z][A-Za-z.'’-]*(?:\s+[A-Z][A-Za-z.'’-]*){1,3}$/

function stripExt(name: string): string {
  const i = name.lastIndexOf('.')
  return i > 0 ? name.slice(0, i) : name
}

function cleanBookPart(value: string): string {
  return value.replace(CATALOG_BEFORE_YEAR_RE, '').replace(CATALOG_PREFIX_RE, '').replace(COMMON_TAGS_RE, ' ').replace(RELEASE_SUFFIX_RE, '').replace(/\s+/g, ' ').trim()
}

export function parseBookMeta(fileName: string): Pick<IBookItem, 'title' | 'author' | 'summary' | 'metadata_source'> {
  const raw = stripExt(fileName).replace(/\s+/g, ' ').trim()
  if (!raw) return { title: fileName || '未命名书籍', author: '未知作者', summary: '', metadata_source: 'unknown' }

  const bracket = raw.match(BRACKET_AUTHOR_RE)
  if (bracket?.[1] && bracket?.[2]) return { author: bracket[1].trim(), title: cleanBookPart(bracket[2]) || raw, summary: '', metadata_source: 'filename' }

  const cleanedRaw = cleanBookPart(raw) || raw
  const titleAuthor = cleanedRaw.match(TITLE_AUTHOR_RE)
  if (titleAuthor?.[1] && titleAuthor?.[2] && titleAuthor[2].length <= 40) return { title: cleanBookPart(titleAuthor[1]), author: titleAuthor[2].trim(), summary: '', metadata_source: 'filename' }

  const pair = cleanedRaw.match(AUTHOR_TITLE_RE)
  if (pair?.[1] && pair?.[2] && pair[1].length <= 40) {
    const left = cleanBookPart(pair[1])
    const right = cleanBookPart(pair[2])
    if (CATALOG_PREFIX_RE.test(raw) || ENGLISH_AUTHOR_RE.test(right)) return { title: left || raw, author: right || '未知作者', summary: '', metadata_source: 'filename' }
    return { author: left || '未知作者', title: right || raw, summary: '', metadata_source: 'filename' }
  }

  return { title: cleanedRaw, author: '未知作者', summary: '', metadata_source: 'filename' }
}
