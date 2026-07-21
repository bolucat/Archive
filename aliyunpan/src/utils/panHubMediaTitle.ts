export function getPanHubSearchTitle(title: string): string {
  return title
    .replace(/^\s*#\d+\s*/, '')
    .replace(/^\s*[【[]\s*\d+(?:\.\d+)?\s*[】\]]\s*/, '')
    .trim()
}
