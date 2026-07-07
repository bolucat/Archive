export declare const EMPTY_STRING = "";
export declare const PORTABLE_EXECUTABLE_DIR: string;
export declare const IS_PORTABLE: boolean;
export declare const APP_THEME: {
    readonly AUTO: "auto";
    readonly LIGHT: "light";
    readonly DARK: "dark";
};
export declare const APP_RUN_MODE: {
    readonly STANDARD: 1;
    readonly TRAY: 2;
    readonly HIDE_TRAY: 3;
};
export declare const ADD_TASK_TYPE: {
    readonly URI: "uri";
    readonly TORRENT: "torrent";
};
export declare const TASK_STATUS: {
    readonly ACTIVE: "active";
    readonly WAITING: "waiting";
    readonly PAUSED: "paused";
    readonly ERROR: "error";
    readonly COMPLETE: "complete";
    readonly REMOVED: "removed";
    readonly SEEDING: "seeding";
};
export declare const LOG_LEVELS: string[];
export declare const MAX_NUM_OF_DIRECTORIES = 5;
export declare const ENGINE_RPC_HOST = "127.0.0.1";
export declare const ENGINE_RPC_PORT = 16800;
export declare const ENGINE_MAX_CONCURRENT_DOWNLOADS = 10;
export declare const ENGINE_MAX_CONNECTION_PER_SERVER = 64;
export declare const UNKNOWN_PEERID = "%00%00%00%00%00%00%00%00%00%00%00%00%00%00%00%00%00%00%00%00";
export declare const UNKNOWN_PEERID_NAME = "unknown";
export declare const GRAPHIC = "\u2591\u2592\u2593\u2588";
export declare const ONE_SECOND = 1000;
export declare const ONE_MINUTE: number;
export declare const ONE_HOUR: number;
export declare const ONE_DAY: number;
export declare const AUTO_SYNC_TRACKER_INTERVAL: number;
export declare const AUTO_CHECK_UPDATE_INTERVAL: number;
export declare const MAX_BT_TRACKER_LENGTH = 6144;
export declare const NGOSANG_TRACKERS_BEST_URL = "https://ngosang.github.io/trackerslist/trackers_best.txt";
export declare const NGOSANG_TRACKERS_BEST_IP_URL = "https://ngosang.github.io/trackerslist/trackers_best_ip.txt";
export declare const NGOSANG_TRACKERS_ALL_URL = "https://ngosang.github.io/trackerslist/trackers_all.txt";
export declare const NGOSANG_TRACKERS_ALL_IP_URL = "https://ngosang.github.io/trackerslist/trackers_all_ip.txt";
export declare const NGOSANG_TRACKERS_BEST_CDN_URL = "https://cdn.jsdelivr.net/gh/ngosang/trackerslist@master/trackers_best.txt";
export declare const NGOSANG_TRACKERS_BEST_IP_CDN_URL = "https://cdn.jsdelivr.net/gh/ngosang/trackerslist@master/trackers_best_ip.txt";
export declare const NGOSANG_TRACKERS_ALL_CDN_URL = "https://cdn.jsdelivr.net/gh/ngosang/trackerslist@master/trackers_all.txt";
export declare const NGOSANG_TRACKERS_ALL_IP_CDN_URL = "https://cdn.jsdelivr.net/gh/ngosang/trackerslist@master/trackers_all_ip.txt";
export declare const XIU2_TRACKERS_BEST_URL = "https://raw.githubusercontent.com/XIU2/TrackersListCollection/master/best.txt";
export declare const XIU2_TRACKERS_ALL_URL = "https://raw.githubusercontent.com/XIU2/TrackersListCollection/master/all.txt";
export declare const XIU2_TRACKERS_HTTP_URL = "https://raw.githubusercontent.com/XIU2/TrackersListCollection/master/http.txt";
export declare const XIU2_TRACKERS_BEST_CDN_URL = "https://cdn.jsdelivr.net/gh/XIU2/TrackersListCollection@master/best.txt";
export declare const XIU2_TRACKERS_ALL_CDN_URL = "https://cdn.jsdelivr.net/gh/XIU2/TrackersListCollection@master/all.txt";
export declare const XIU2_TRACKERS_HTTP_CDN_URL = "https://cdn.jsdelivr.net/gh/XIU2/TrackersListCollection@master/http.txt";
export declare const XIU2_TRACKERS_BLACK_URL = "https://raw.githubusercontent.com/XIU2/TrackersListCollection/master/blacklist.txt";
export declare const TRACKER_SOURCE_OPTIONS: {
    label: string;
    value: string;
}[];
export declare const PROXY_SCOPES: {
    readonly DOWNLOAD: "download";
    readonly UPDATE_APP: "update-app";
    readonly UPDATE_TRACKERS: "update-trackers";
};
export type ProxyScopeType = typeof PROXY_SCOPES[keyof typeof PROXY_SCOPES];
export declare const PROXY_SCOPE_OPTIONS: ("download" | "update-app" | "update-trackers")[];
export declare const NONE_SELECTED_FILES = "none";
export declare const SELECTED_ALL_FILES = "all";
export declare const IP_VERSION: {
    readonly V4: 4;
    readonly V6: 6;
};
export declare const LOGIN_SETTING_OPTIONS: {
    args: string[];
};
export declare const TRAY_CANVAS_CONFIG: {
    WIDTH: number;
    HEIGHT: number;
    ICON_WIDTH: number;
    ICON_HEIGHT: number;
    TEXT_WIDTH: number;
    TEXT_FONT_SIZE: number;
};
export declare const COMMON_RESOURCE_TAGS: string[];
export declare const THUNDER_RESOURCE_TAGS: string[];
export declare const RESOURCE_TAGS: string[];
export declare const SUPPORT_RTL_LOCALES: string[];
export declare const IMAGE_SUFFIXES: string[];
export declare const AUDIO_SUFFIXES: string[];
export declare const VIDEO_SUFFIXES: string[];
export declare const SUB_SUFFIXES: string[];
export declare const DOCUMENT_SUFFIXES: string[];
