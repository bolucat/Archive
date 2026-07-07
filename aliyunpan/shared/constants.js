export const EMPTY_STRING = '';
export const PORTABLE_EXECUTABLE_DIR = process.env.PORTABLE_EXECUTABLE_DIR || '';
export const IS_PORTABLE = !!PORTABLE_EXECUTABLE_DIR;
export const APP_THEME = { AUTO: 'auto', LIGHT: 'light', DARK: 'dark' };
export const APP_RUN_MODE = { STANDARD: 1, TRAY: 2, HIDE_TRAY: 3 };
export const ADD_TASK_TYPE = { URI: 'uri', TORRENT: 'torrent' };
export const TASK_STATUS = {
    ACTIVE: 'active', WAITING: 'waiting', PAUSED: 'paused',
    ERROR: 'error', COMPLETE: 'complete', REMOVED: 'removed', SEEDING: 'seeding'
};
export const LOG_LEVELS = ['error', 'warn', 'info', 'verbose', 'debug', 'silly'];
export const MAX_NUM_OF_DIRECTORIES = 5;
export const ENGINE_RPC_HOST = '127.0.0.1';
export const ENGINE_RPC_PORT = 16800;
export const ENGINE_MAX_CONCURRENT_DOWNLOADS = 10;
export const ENGINE_MAX_CONNECTION_PER_SERVER = 64;
export const UNKNOWN_PEERID = '%00%00%00%00%00%00%00%00%00%00%00%00%00%00%00%00%00%00%00%00';
export const UNKNOWN_PEERID_NAME = 'unknown';
export const GRAPHIC = '░▒▓█';
export const ONE_SECOND = 1000;
export const ONE_MINUTE = 60 * ONE_SECOND;
export const ONE_HOUR = 60 * ONE_MINUTE;
export const ONE_DAY = 24 * ONE_HOUR;
export const AUTO_SYNC_TRACKER_INTERVAL = ONE_HOUR * 12;
export const AUTO_CHECK_UPDATE_INTERVAL = ONE_DAY * 7;
export const MAX_BT_TRACKER_LENGTH = 6144;
export const NGOSANG_TRACKERS_BEST_URL = 'https://ngosang.github.io/trackerslist/trackers_best.txt';
export const NGOSANG_TRACKERS_BEST_IP_URL = 'https://ngosang.github.io/trackerslist/trackers_best_ip.txt';
export const NGOSANG_TRACKERS_ALL_URL = 'https://ngosang.github.io/trackerslist/trackers_all.txt';
export const NGOSANG_TRACKERS_ALL_IP_URL = 'https://ngosang.github.io/trackerslist/trackers_all_ip.txt';
export const NGOSANG_TRACKERS_BEST_CDN_URL = 'https://cdn.jsdelivr.net/gh/ngosang/trackerslist@master/trackers_best.txt';
export const NGOSANG_TRACKERS_BEST_IP_CDN_URL = 'https://cdn.jsdelivr.net/gh/ngosang/trackerslist@master/trackers_best_ip.txt';
export const NGOSANG_TRACKERS_ALL_CDN_URL = 'https://cdn.jsdelivr.net/gh/ngosang/trackerslist@master/trackers_all.txt';
export const NGOSANG_TRACKERS_ALL_IP_CDN_URL = 'https://cdn.jsdelivr.net/gh/ngosang/trackerslist@master/trackers_all_ip.txt';
export const XIU2_TRACKERS_BEST_URL = 'https://raw.githubusercontent.com/XIU2/TrackersListCollection/master/best.txt';
export const XIU2_TRACKERS_ALL_URL = 'https://raw.githubusercontent.com/XIU2/TrackersListCollection/master/all.txt';
export const XIU2_TRACKERS_HTTP_URL = 'https://raw.githubusercontent.com/XIU2/TrackersListCollection/master/http.txt';
export const XIU2_TRACKERS_BEST_CDN_URL = 'https://cdn.jsdelivr.net/gh/XIU2/TrackersListCollection@master/best.txt';
export const XIU2_TRACKERS_ALL_CDN_URL = 'https://cdn.jsdelivr.net/gh/XIU2/TrackersListCollection@master/all.txt';
export const XIU2_TRACKERS_HTTP_CDN_URL = 'https://cdn.jsdelivr.net/gh/XIU2/TrackersListCollection@master/http.txt';
export const XIU2_TRACKERS_BLACK_URL = 'https://raw.githubusercontent.com/XIU2/TrackersListCollection/master/blacklist.txt';
export const TRACKER_SOURCE_OPTIONS = [
    { label: 'ngosang/trackerslist - best', value: NGOSANG_TRACKERS_BEST_URL },
    { label: 'ngosang/trackerslist - best (ip)', value: NGOSANG_TRACKERS_BEST_IP_URL },
    { label: 'ngosang/trackerslist - all', value: NGOSANG_TRACKERS_ALL_URL },
    { label: 'ngosang/trackerslist - all (ip)', value: NGOSANG_TRACKERS_ALL_IP_URL },
    { label: 'XIU2/TrackersListCollection - best', value: XIU2_TRACKERS_BEST_URL },
    { label: 'XIU2/TrackersListCollection - all', value: XIU2_TRACKERS_ALL_URL },
    { label: 'XIU2/TrackersListCollection - http', value: XIU2_TRACKERS_HTTP_URL }
];
export const PROXY_SCOPES = {
    DOWNLOAD: 'download',
    UPDATE_APP: 'update-app',
    UPDATE_TRACKERS: 'update-trackers'
};
export const PROXY_SCOPE_OPTIONS = [
    PROXY_SCOPES.DOWNLOAD, PROXY_SCOPES.UPDATE_APP, PROXY_SCOPES.UPDATE_TRACKERS
];
export const NONE_SELECTED_FILES = 'none';
export const SELECTED_ALL_FILES = 'all';
export const IP_VERSION = { V4: 4, V6: 6 };
export const LOGIN_SETTING_OPTIONS = { args: ['--opened-at-login=1'] };
export const TRAY_CANVAS_CONFIG = {
    WIDTH: 66, HEIGHT: 16, ICON_WIDTH: 16, ICON_HEIGHT: 16,
    TEXT_WIDTH: 46, TEXT_FONT_SIZE: 8
};
export const COMMON_RESOURCE_TAGS = ['http://', 'https://', 'ftp://', 'magnet:'];
export const THUNDER_RESOURCE_TAGS = ['thunder://'];
export const RESOURCE_TAGS = [...COMMON_RESOURCE_TAGS, ...THUNDER_RESOURCE_TAGS];
export const SUPPORT_RTL_LOCALES = ['ar', 'fa', 'he', 'ku', 'pa', 'ps', 'sd', 'ur', 'yi'];
export const IMAGE_SUFFIXES = ['.ai', '.bmp', '.eps', '.fig', '.gif', '.heic', '.icn', '.ico', '.jpeg', '.jpg', '.png', '.psd', '.raw', '.sketch', '.svg', '.tif', '.webp', '.xd'];
export const AUDIO_SUFFIXES = ['.aac', '.ape', '.flac', '.flav', '.m4a', '.mp3', '.ogg', '.wav', '.wma'];
export const VIDEO_SUFFIXES = ['.avi', '.m4v', '.mkv', '.mov', '.mp4', '.mpg', '.rmvb', '.vob', '.wmv'];
export const SUB_SUFFIXES = ['.ass', '.idx', '.smi', '.srt', '.ssa', '.sst', '.sub'];
export const DOCUMENT_SUFFIXES = ['.azw3', '.csv', '.doc', '.docx', '.epub', '.key', '.mobi', '.numbers', '.pages', '.pdf', '.ppt', '.pptx', '.txt', '.xsl', '.xslx'];
