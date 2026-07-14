import type { IconName } from "../components/Icon";
import { loadStoredJson, removeStoredValue, saveStoredJson } from "../lib/storage";
import type { MessageKey } from "./translations";

export type DashboardCardId =
  | "uploadTraffic"
  | "downloadTraffic"
  | "status"
  | "connections"
  | "systemProxy"
  | "clashMode"
  | "profile";

export const DASHBOARD_CARDS: Record<
  DashboardCardId,
  { title: MessageKey; icon: IconName; desktop?: boolean; permanent?: boolean }
> = {
  uploadTraffic: { title: "Upload", icon: "upload" },
  downloadTraffic: { title: "Download", icon: "download" },
  status: { title: "Status", icon: "bug_report" },
  connections: { title: "Connections", icon: "cable" },
  systemProxy: { title: "System HTTP Proxy", icon: "router", desktop: true },
  clashMode: { title: "Clash Mode", icon: "route" },
  profile: { title: "Profile", icon: "folder", desktop: true, permanent: true },
};

const DEFAULT_CARD_ORDER: DashboardCardId[] = [
  "uploadTraffic",
  "downloadTraffic",
  "status",
  "connections",
  "systemProxy",
  "clashMode",
  "profile",
];

export interface DashboardCardsConfig {
  enabled: string[];
  order: string[];
}

const STORAGE_KEY = "dashboard-cards";

export function isDashboardCardId(value: string): value is DashboardCardId {
  return value in DASHBOARD_CARDS;
}

export function dashboardCardIds(desktop: boolean): DashboardCardId[] {
  return DEFAULT_CARD_ORDER.filter((card) => desktop || DASHBOARD_CARDS[card].desktop !== true);
}

export function loadDashboardCardsConfig(desktop: boolean): DashboardCardsConfig {
  const defaults = dashboardCardIds(desktop);
  const known = new Set<string>(defaults);
  const parsed = loadStoredJson(STORAGE_KEY) as Partial<DashboardCardsConfig> | null;
  if (parsed) {
    let enabled = (Array.isArray(parsed.enabled) ? parsed.enabled : []).filter(
      (card): card is string => typeof card === "string" && known.has(card),
    );
    let order = (Array.isArray(parsed.order) ? parsed.order : []).filter(
      (card): card is string => typeof card === "string" && known.has(card),
    );
    const ordered = new Set(order);
    order = order.concat(defaults.filter((card) => !ordered.has(card)));
    const enabledCards = new Set(enabled);
    for (const card of defaults) {
      if (DASHBOARD_CARDS[card].permanent && !enabledCards.has(card)) {
        enabled = orderedEnabledCards({ enabled: [...enabled, card], order });
        enabledCards.add(card);
      }
    }
    return { enabled, order };
  }
  return { enabled: [...defaults], order: [...defaults] };
}

export function saveDashboardCardsConfig(config: DashboardCardsConfig) {
  saveStoredJson(STORAGE_KEY, config);
}

export function resetDashboardCardsConfig(desktop: boolean): DashboardCardsConfig {
  removeStoredValue(STORAGE_KEY);
  const defaults = dashboardCardIds(desktop);
  return { enabled: [...defaults], order: [...defaults] };
}

export function orderedEnabledCards(config: DashboardCardsConfig): string[] {
  const enabled = new Set(config.enabled);
  return config.order.filter((card) => enabled.has(card));
}

export function toggleCard(config: DashboardCardsConfig, card: string): DashboardCardsConfig {
  if (isDashboardCardId(card) && DASHBOARD_CARDS[card].permanent) {
    return config;
  }
  if (config.enabled.includes(card)) {
    return { ...config, enabled: config.enabled.filter((entry) => entry !== card) };
  }
  const enabled = orderedEnabledCards({ ...config, enabled: [...config.enabled, card] });
  return { ...config, enabled };
}

export function moveCard(
  config: DashboardCardsConfig,
  from: number,
  to: number,
): DashboardCardsConfig {
  const order = [...config.order];
  const [moved] = order.splice(from, 1);
  order.splice(to, 0, moved);
  return { ...config, order };
}
