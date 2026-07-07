import type { ProxyScopeType } from '@shared/constants';
export interface ProxyConfig {
    enable?: boolean;
    server?: string;
    bypass?: string;
    scope?: ProxyScopeType[];
}
export declare const convertToAxiosProxy: (proxyServer: string) => any;
export declare const fetchBtTrackerFromSource: (source?: string[], proxyConfig?: ProxyConfig) => Promise<string[]>;
export declare const convertTrackerDataToLine: (arr?: string[]) => string;
export declare const convertTrackerDataToComma: (arr?: string[]) => string;
export declare const reduceTrackerString: (str: string) => string;
