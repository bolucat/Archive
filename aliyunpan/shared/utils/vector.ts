export function cosineSimilarity(a: Float32Array, b: Float32Array): number {
  if (a.length !== b.length) {
    throw new Error(`Dimension mismatch: ${a.length} vs ${b.length}`)
  }
  let dotProduct = 0
  let normA = 0
  let normB = 0
  for (let i = 0; i < a.length; i++) {
    dotProduct += a[i] * b[i]
    normA += a[i] * a[i]
    normB += b[i] * b[i]
  }
  const denominator = Math.sqrt(normA) * Math.sqrt(normB)
  if (denominator === 0) return 0
  return dotProduct / denominator
}

export function serializeEmbedding(embedding: number[] | Float32Array): Buffer {
  const arr = embedding instanceof Float32Array ? embedding : new Float32Array(embedding)
  return Buffer.from(arr.buffer)
}

export function deserializeEmbedding(buffer: Buffer): Float32Array {
  return new Float32Array(buffer.buffer, buffer.byteOffset, buffer.byteLength / Float32Array.BYTES_PER_ELEMENT)
}

export function normalizeEmbedding(embedding: Float32Array): Float32Array {
  let norm = 0
  for (let i = 0; i < embedding.length; i++) {
    norm += embedding[i] * embedding[i]
  }
  norm = Math.sqrt(norm)
  const result = new Float32Array(embedding.length)
  if (norm > 0) {
    for (let i = 0; i < embedding.length; i++) {
      result[i] = embedding[i] / norm
    }
  }
  return result
}
