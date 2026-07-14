import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";

import { Select } from "../components/ui";
import { LANGUAGES, TRANSLATIONS, type Language, type MessageKey, type PluralForms } from "./translations";
import {
  detectSystemLanguage,
  loadLanguagePreference,
  saveLanguagePreference,
  subscribeSystemLanguage,
  type LanguagePreference,
} from "./i18nCore";

export type { Language, MessageKey };

export type TranslateParams = Record<string, string | number>;
export type Translate = (key: MessageKey, params?: TranslateParams) => string;

const pluralRulesCache = new Map<Language, Intl.PluralRules>();

function translate(language: Language, key: MessageKey, params?: TranslateParams): string {
  const entry: string | PluralForms = language === "en" ? key : (TRANSLATIONS[key]?.[language] ?? key);
  let text: string;
  if (typeof entry === "string") {
    text = entry;
  } else {
    const count = typeof params?.count === "number" ? params.count : null;
    if (count === null) {
      text = entry.other;
    } else {
      let rules = pluralRulesCache.get(language);
      if (!rules) {
        rules = new Intl.PluralRules(language);
        pluralRulesCache.set(language, rules);
      }
      text = entry[rules.select(count)] ?? entry.other;
    }
  }
  if (params) {
    for (const [name, value] of Object.entries(params)) {
      text = text.replaceAll(`{${name}}`, String(value));
    }
  }
  return text;
}

interface I18n {
  language: Language;
  preference: LanguagePreference;
  setPreference: (preference: LanguagePreference) => void;
  t: Translate;
}

const I18nContext = createContext<I18n | null>(null);

export function useI18n(): I18n {
  const i18n = useContext(I18nContext);
  if (!i18n) {
    throw new Error("missing i18n context");
  }
  return i18n;
}

export function I18nProvider(props: { children: ReactNode }) {
  const [preference, setPreferenceState] = useState<LanguagePreference>(() =>
    loadLanguagePreference(),
  );
  const systemLanguage = useSyncExternalStore(
    subscribeSystemLanguage,
    detectSystemLanguage,
    detectSystemLanguage,
  );

  const language = preference === "auto" ? systemLanguage : preference;

  useEffect(() => {
    document.documentElement.lang = language;
    document.documentElement.dir = language === "fa" ? "rtl" : "ltr";
  }, [language]);

  const value = useMemo<I18n>(
    () => ({
      language,
      preference,
      setPreference: (next) => {
        saveLanguagePreference(next);
        setPreferenceState(() => next);
      },
      t: (key, params) => translate(language, key, params),
    }),
    [language, preference],
  );

  return <I18nContext.Provider value={value}>{props.children}</I18nContext.Provider>;
}

export function LanguageSelect() {
  const { t, preference, setPreference } = useI18n();
  const options: { value: LanguagePreference; label: string }[] = [
    { value: "auto", label: t("System") },
    ...LANGUAGES.map((language) => ({ value: language.value, label: language.label })),
  ];
  return <Select inline options={options} value={preference} onChange={setPreference} />;
}
