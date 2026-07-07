import { describe, expect, it } from 'vitest'
import {
  buildSpeechStartText,
  buildSpeechText,
  canSpeakText,
  hasSpeechSupport,
  speakText,
  speakTextSequence,
  splitSpeechText
} from '../bookTextToSpeech'

describe('bookTextToSpeech', () => {
  it('normalizes text before speech', () => {
    expect(buildSpeechText([' hello ', '\nworld\n'])).toBe('hello world')
  })

  it('splits speech text into sentence chunks', () => {
    expect(splitSpeechText('One. Two? Three!', 12)).toEqual(['One. Two?', 'Three!'])
    expect(splitSpeechText('One. Two? Three!', 240, false)).toEqual(['One.', 'Two?', 'Three!'])
    expect(splitSpeechText('Long sentence without punctuation', 10)).toEqual(['Long sente', 'nce withou', 't punctuat', 'ion'])
  })

  it('starts speech from the containing sentence when available', () => {
    expect(buildSpeechStartText('world', ' hello\nworld. ')).toBe('hello world.')
    expect(buildSpeechStartText(' world ', '')).toBe('world')
  })

  it('continues speech from the matched sentence in visible text', () => {
    expect(buildSpeechStartText(
      'world',
      'Hello world.',
      'Before. Hello world. Next sentence.'
    )).toBe('Hello world. Next sentence.')
    expect(buildSpeechStartText(
      'world',
      'Missing sentence.',
      'Before world and after.'
    )).toBe('world and after.')
  })

  it('requires non-empty speech text and browser support', () => {
    expect(canSpeakText('', true)).toBe(false)
    expect(canSpeakText('hello', false)).toBe(false)
    expect(canSpeakText('hello', true)).toBe(true)
  })

  it('requires utterance support before speaking', () => {
    const speechSynthesis = { cancel: () => {}, speak: () => {} }
    expect(hasSpeechSupport({ speechSynthesis })).toBe(false)
    expect(speakText('hello', { speechSynthesis })).toBe(false)
  })

  it('normalizes text when speaking', () => {
    const spoken: string[] = []
    class Utterance {
      text: string

      constructor(text: string) {
        this.text = text
      }
    }
    const host = {
      SpeechSynthesisUtterance: Utterance,
      speechSynthesis: {
        cancel: () => {},
        speak: (utterance: Utterance) => { spoken.push(utterance.text) }
      }
    }
    expect(speakText(' hello\nworld ', host)).toBe(true)
    expect(spoken).toEqual(['hello world'])
  })

  it('speaks sentence chunks in sequence', () => {
    const spoken: string[] = []
    const started: string[] = []
    const utterances: Array<{ text: string, onend?: () => void }> = []
    let completed = false
    class Utterance {
      onend?: () => void

      constructor(public text: string) {}
    }
    const host = {
      SpeechSynthesisUtterance: Utterance,
      speechSynthesis: {
        cancel: () => {},
        speak: (utterance: Utterance) => {
          spoken.push(utterance.text)
          utterances.push(utterance)
        }
      }
    }
    const session = speakTextSequence('One. Two.', {
      onChunkStart: (text, index, total) => { started.push(`${index + 1}/${total}:${text}`) },
      onComplete: () => { completed = true },
      combineSentences: false
    }, host)
    expect(session).not.toBeNull()
    expect(session?.getCurrentIndex()).toBe(0)
    expect(session?.getTotal()).toBe(2)
    expect(spoken).toEqual(['One.'])
    expect(started).toEqual(['1/2:One.'])
    utterances[0].onend?.()
    expect(session?.getCurrentIndex()).toBe(1)
    expect(spoken).toEqual(['One.', 'Two.'])
    expect(started).toEqual(['1/2:One.', '2/2:Two.'])
    utterances[1].onend?.()
    expect(completed).toBe(true)
  })

  it('applies language, voice uri and rate to speech chunks', () => {
    const selectedVoice = { name: 'Tingting', lang: 'zh-TW', voiceURI: 'voice-2' }
    const utterances: Array<{ text: string, lang?: string, voice?: unknown, rate?: number, volume?: number }> = []
    class Utterance {
      lang?: string
      voice?: unknown
      rate?: number
      volume?: number

      constructor(public text: string) {}
    }
    const host = {
      SpeechSynthesisUtterance: Utterance,
      speechSynthesis: {
        cancel: () => {},
        getVoices: () => [
          { name: 'Tingting', lang: 'zh-CN', voiceURI: 'voice-1' },
          selectedVoice
        ],
        speak: (utterance: Utterance) => { utterances.push(utterance) }
      }
    }

    speakTextSequence('One.', {
      lang: 'zh-TW',
      voiceURI: 'voice-2',
      voiceName: 'Tingting',
      rate: 1.5,
      volume: 0.75
    }, host)

    expect(utterances[0]).toMatchObject({
      text: 'One.',
      lang: 'zh-TW',
      voice: selectedVoice,
      rate: 1.5,
      volume: 0.75
    })
  })

  it('stops a speech sequence without completing it', () => {
    const utterances: Array<{ text: string, onend?: () => void }> = []
    let cancelCount = 0
    let completed = false
    class Utterance {
      onend?: () => void

      constructor(public text: string) {}
    }
    const host = {
      SpeechSynthesisUtterance: Utterance,
      speechSynthesis: {
        cancel: () => { cancelCount++ },
        speak: (utterance: Utterance) => { utterances.push(utterance) }
      }
    }
    const session = speakTextSequence('One. Two. Three.', {
      onComplete: () => { completed = true },
      combineSentences: false
    }, host)
    session?.stop()
    utterances[0].onend?.()
    expect(cancelCount).toBe(2)
    expect(utterances).toHaveLength(1)
    expect(completed).toBe(false)
  })

  it('moves between speech chunks and ignores stale utterance endings', () => {
    const utterances: Array<{ text: string, onend?: () => void }> = []
    const started: string[] = []
    let cancelCount = 0
    let completed = false
    class Utterance {
      onend?: () => void

      constructor(public text: string) {}
    }
    const host = {
      SpeechSynthesisUtterance: Utterance,
      speechSynthesis: {
        cancel: () => { cancelCount++ },
        speak: (utterance: Utterance) => { utterances.push(utterance) }
      }
    }
    const session = speakTextSequence('One. Two. Three.', {
      onChunkStart: (text, index) => { started.push(`${index}:${text}`) },
      onComplete: () => { completed = true },
      combineSentences: false
    }, host)

    expect(session?.next()).toBe(true)
    expect(utterances.map((item) => item.text)).toEqual(['One.', 'Two.'])
    utterances[0].onend?.()
    expect(utterances.map((item) => item.text)).toEqual(['One.', 'Two.'])

    expect(session?.previous()).toBe(true)
    expect(utterances.map((item) => item.text)).toEqual(['One.', 'Two.', 'One.'])
    utterances[1].onend?.()
    expect(utterances.map((item) => item.text)).toEqual(['One.', 'Two.', 'One.'])

    expect(session?.next()).toBe(true)
    expect(session?.next()).toBe(true)
    expect(utterances.map((item) => item.text)).toEqual(['One.', 'Two.', 'One.', 'Two.', 'Three.'])
    expect(session?.next()).toBe(true)
    expect(completed).toBe(true)
    expect(session?.next()).toBe(false)
    expect(cancelCount).toBe(6)
    expect(started).toEqual(['0:One.', '1:Two.', '0:One.', '1:Two.', '2:Three.'])
  })

  it('replays the first chunk on previous and clears paused state when moving', () => {
    const utterances: Array<{ text: string, onend?: () => void }> = []
    const calls: string[] = []
    class Utterance {
      onend?: () => void

      constructor(public text: string) {}
    }
    const host = {
      SpeechSynthesisUtterance: Utterance,
      speechSynthesis: {
        cancel: () => { calls.push('cancel') },
        pause: () => { calls.push('pause') },
        resume: () => { calls.push('resume') },
        speak: (utterance: Utterance) => { utterances.push(utterance) }
      }
    }
    const session = speakTextSequence('One. Two.', { combineSentences: false }, host)

    expect(session?.previous()).toBe(true)
    expect(utterances.map((item) => item.text)).toEqual(['One.', 'One.'])
    expect(session?.pause()).toBe(true)
    expect(session?.isPaused()).toBe(true)
    expect(session?.next()).toBe(true)
    expect(session?.isPaused()).toBe(false)
    expect(session?.resume()).toBe(false)
    expect(utterances.map((item) => item.text)).toEqual(['One.', 'One.', 'Two.'])
    expect(calls).toEqual(['cancel', 'cancel', 'pause', 'cancel'])
  })

  it('pauses and resumes a speech sequence', () => {
    const calls: string[] = []
    class Utterance {
      constructor(public text: string) {}
    }
    const host = {
      SpeechSynthesisUtterance: Utterance,
      speechSynthesis: {
        cancel: () => { calls.push('cancel') },
        pause: () => { calls.push('pause') },
        resume: () => { calls.push('resume') },
        speak: () => { calls.push('speak') }
      }
    }
    const session = speakTextSequence('One.', {}, host)
    expect(session?.isPaused()).toBe(false)
    expect(session?.pause()).toBe(true)
    expect(session?.pause()).toBe(false)
    expect(session?.isPaused()).toBe(true)
    expect(session?.resume()).toBe(true)
    expect(session?.resume()).toBe(false)
    expect(session?.isPaused()).toBe(false)
    session?.stop()
    expect(session?.pause()).toBe(false)
    expect(session?.resume()).toBe(false)
    expect(calls).toEqual(['cancel', 'speak', 'pause', 'resume', 'cancel'])
  })

  it('does not pause or resume after a speech sequence completes', () => {
    const utterances: Array<{ text: string, onend?: () => void }> = []
    class Utterance {
      onend?: () => void

      constructor(public text: string) {}
    }
    const host = {
      SpeechSynthesisUtterance: Utterance,
      speechSynthesis: {
        cancel: () => {},
        pause: () => {},
        resume: () => {},
        speak: (utterance: Utterance) => { utterances.push(utterance) }
      }
    }
    const session = speakTextSequence('One.', {}, host)
    utterances[0].onend?.()
    expect(session?.pause()).toBe(false)
    expect(session?.resume()).toBe(false)
    expect(session?.isPaused()).toBe(false)
  })

  it('does not resume after a speech sequence errors', () => {
    const utterances: Array<{ text: string, onerror?: (event: unknown) => void }> = []
    class Utterance {
      onerror?: (event: unknown) => void

      constructor(public text: string) {}
    }
    const host = {
      SpeechSynthesisUtterance: Utterance,
      speechSynthesis: {
        cancel: () => {},
        pause: () => {},
        resume: () => {},
        speak: (utterance: Utterance) => { utterances.push(utterance) }
      }
    }
    const session = speakTextSequence('One.', {}, host)
    expect(session?.pause()).toBe(true)
    utterances[0].onerror?.({})
    expect(session?.resume()).toBe(false)
    expect(session?.pause()).toBe(false)
    expect(session?.isPaused()).toBe(false)
  })
})
