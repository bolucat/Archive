import MarkdownIt from 'markdown-it'

const md = new MarkdownIt({
  html: false,
  breaks: true,
  linkify: true,
  typographer: true,
})

export function renderMarkdown(text: string): string {
  if (!text) return ''
  return md.render(text)
}
