import type { ConsolidatedMemory, MemoryScope } from '@shared/types/reedy'
import { MEMORY_CONSOLIDATION_THRESHOLD, MEMORY_MAX_PER_RUN } from '@shared/types/reedy'
import { writeMemory } from '../ReedyService'
import { v4 as uuidv4 } from 'uuid'

const BLOCKLIST_REGEX = /system|policy|prompt|injection|override/i

export async function consolidateMemories(
  messages: Array<{ role: string; content: string }>,
  scopeKey: string,
  generateSummary: (prompt: string) => Promise<string>,
  maxPerRun: number = MEMORY_MAX_PER_RUN
): Promise<ConsolidatedMemory[]> {
  if (messages.length < MEMORY_CONSOLIDATION_THRESHOLD) return []

  const conversationText = messages.map(m => `[${m.role}]: ${m.content}`).join('\n')

  const systemPrompt = `You are a memory consolidation system. Based on the conversation below, extract 1-${maxPerRun} durable memories.
Each memory must be in this JSON format: { "scope": "user" | "book", "key": "short-key-identifier", "summary": "1-3 sentence summary" }

- "user" scope: user preferences, reading habits, recurring questions
- "book" scope: character notes, plot summaries, thematic observations
- "key" must be 1-128 characters, alphanumeric with hyphens/underscores/colons/dots
- "key" must NOT contain: system, policy, prompt, injection, override

Output ONLY a JSON array. No markdown fences, no prose.

Conversation:
${conversationText}`

  try {
    const raw = await generateSummary(systemPrompt)
    const cleaned = raw.replace(/```json\s*/g, '').replace(/```\s*/g, '').trim()
    const parsed = JSON.parse(cleaned)

    if (!Array.isArray(parsed)) return []

    const results: ConsolidatedMemory[] = []
    for (const item of parsed) {
      if (results.length >= maxPerRun) break
      if (!item.scope || !item.key || !item.summary) continue
      if (!['user', 'book'].includes(item.scope)) continue
      if (typeof item.key !== 'string' || item.key.length > 128) continue
      if (BLOCKLIST_REGEX.test(item.key)) continue
      if (typeof item.summary !== 'string' || item.summary.length > 2000) continue

      results.push({
        scope: item.scope as MemoryScope,
        key: item.key,
        summary: item.summary
      })

      // Persist to DB
      writeMemory({
        scope: item.scope as MemoryScope,
        scope_key: scopeKey,
        key: item.key,
        summary: item.summary,
        source_message_id: messages[messages.length - 1]?.content?.slice(0, 100)
      })
    }

    return results
  } catch {
    return []
  }
}
