import type { TailscaleEndpointStatus, TailscalePeer } from "../gen/daemon/started_service_pb";

import { loadStoredJson, saveStoredJson } from "./storage";

export interface TailscaleSSHPrefs {
  username: string;
  terminalType: string;
  remember: boolean;
}

export interface SSHSessionOptions {
  endpointTag: string;
  peerAddress: string;
  peerName: string;
  username: string;
  terminalType: string;
  hostKeys: string[];
}

const SSH_PREFS_KEY = "tailscale-ssh";
export const SSH_DEFAULT_USERNAME = "root";
export const SSH_DEFAULT_TERMINAL_TYPE = "xterm-256color";

export function loadSSHPrefs(): Record<string, TailscaleSSHPrefs> {
  const parsed = loadStoredJson(SSH_PREFS_KEY);
  if (parsed && typeof parsed === "object") {
    return parsed as Record<string, TailscaleSSHPrefs>;
  }
  return {};
}

export function saveSSHPrefs(stableID: string, prefs: TailscaleSSHPrefs) {
  const map = loadSSHPrefs();
  map[stableID] = prefs;
  saveStoredJson(SSH_PREFS_KEY, map);
}

// Default theme names match sing-box-for-apple.
export const DEFAULT_LIGHT_THEME_NAME = "Alabaster";
export const DEFAULT_DARK_THEME_NAME = "Afterglow";

export interface TerminalConfig {
  symbolBarAlwaysShow: boolean;

  lightThemeName: string;
  darkThemeName: string;
  lightThemeCustom: string;
  darkThemeCustom: string;

  fontFamily: string;
  fontSize: number;
}

const TERMINAL_CONFIG_KEY = "terminal-config";
export const TERMINAL_CONFIG_EVENT = "sing-box-dashboard:terminal-config";

export const DEFAULT_TERMINAL_FONT_SIZE = 13;

const DEFAULT_TERMINAL_CONFIG: TerminalConfig = {
  symbolBarAlwaysShow: false,
  lightThemeName: DEFAULT_LIGHT_THEME_NAME,
  darkThemeName: DEFAULT_DARK_THEME_NAME,
  lightThemeCustom: "",
  darkThemeCustom: "",
  fontFamily: "",
  fontSize: DEFAULT_TERMINAL_FONT_SIZE,
};

export function loadTerminalConfig(): TerminalConfig {
  const parsed = loadStoredJson(TERMINAL_CONFIG_KEY);
  if (parsed && typeof parsed === "object") {
    return { ...DEFAULT_TERMINAL_CONFIG, ...(parsed as Partial<TerminalConfig>) };
  }
  return { ...DEFAULT_TERMINAL_CONFIG };
}

export function saveTerminalConfig(config: TerminalConfig) {
  saveStoredJson(TERMINAL_CONFIG_KEY, config);
  window.dispatchEvent(new Event(TERMINAL_CONFIG_EVENT));
}

export function allPeers(endpoint: TailscaleEndpointStatus | undefined): TailscalePeer[] {
  return endpoint?.userGroups.flatMap((group) => group.peers) ?? [];
}

export function peerDisplayName(peer: TailscalePeer | undefined): string {
  if (!peer) {
    return "";
  }
  if (peer.dnsName !== "") {
    return peer.dnsName.split(".")[0];
  }
  return peer.hostName;
}

export function peerSSHAddress(peer: TailscalePeer): string {
  return (
    peer.tailscaleIPs.find((address) => !address.includes(":")) ??
    peer.tailscaleIPs[0] ??
    peer.dnsName
  );
}

export function peerSSHAvailable(peer: TailscalePeer): boolean {
  return peer.online && peer.sshHostKeys.length > 0 && peer.tailscaleIPs.length > 0;
}

export function buildSSHSession(
  endpointTag: string,
  peer: TailscalePeer,
  username: string,
  terminalType: string,
): SSHSessionOptions {
  return {
    endpointTag,
    peerAddress: peerSSHAddress(peer),
    peerName: peerDisplayName(peer),
    username,
    terminalType,
    hostKeys: peer.sshHostKeys,
  };
}
