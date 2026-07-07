import { describe, expect, it } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'

const root = process.cwd()
const read = (file: string) => fs.readFileSync(path.join(root, file), 'utf8')

describe('BookManagerShell contract', () => {
  it('uses Koodo manager structural class names', () => {
    const shell = read('src/layout/book-manager/BookManagerShell.vue')
    expect(shell).toContain('book-manager-shell')
    expect(shell).toContain('book-manager-sidebar-slot')
    expect(shell).toContain('book-manager-header-slot')
    expect(shell).toContain('book-manager-main-slot')
    expect(shell).toContain('book-manager-footer-slot')
  })

  it('has sidebar and header components wired by PageBookLibrary', () => {
    const page = read('src/layout/PageBookLibrary.vue')
    expect(page).toContain("import BookManagerShell from './book-manager/BookManagerShell.vue'")
    expect(page).toContain("import BookManagerSidebar from './book-manager/BookManagerSidebar.vue'")
    expect(page).toContain("import BookManagerHeader from './book-manager/BookManagerHeader.vue'")
    expect(page).toContain('<BookManagerShell')
    expect(page).toContain('<BookManagerSidebar')
    expect(page).toContain('<BookManagerHeader')
  })
})
