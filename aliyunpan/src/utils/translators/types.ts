export interface TranslationProvider {
  name: string
  label: string
  translate: (text: string, targetLang: string) => Promise<string>
}

export interface TranslatorRegistry {
  providers: TranslationProvider[]
  get: (name: string) => TranslationProvider | undefined
  defaultName: string
}
