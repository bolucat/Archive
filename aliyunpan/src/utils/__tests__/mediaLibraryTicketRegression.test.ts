import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('media-library ticket regressions', () => {
  it('keeps local scan media associated with its persisted source and batches storage writes', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/utils/mediaScanner.ts'), 'utf8')
    const storeSource = readFileSync(resolve(process.cwd(), 'src/store/medialibrary.ts'), 'utf8')

    expect(source).toContain('this.processVideoFile(file, folderName, `local_${folderPath}`)')
    expect(source).toContain('this.mediaStore.beginPersistenceBatch()')
    expect(source).toContain('this.mediaStore.endPersistenceBatch()')
    expect(storeSource).toContain('const beginPersistenceBatch')
    expect(storeSource).toContain('const checkpointPersistenceBatch')
    expect(storeSource).toContain('const endPersistenceBatch')
    expect(storeSource).toContain('stopPersistenceWatchers()')
    expect(storeSource).toContain('mediaItems.value.push(item)')
    expect(source).toContain('PERSISTENCE_CHECKPOINT_ITEMS = 100')
    expect(source).toContain('PERSISTENCE_CHECKPOINT_MS = 3000')
  })
})
