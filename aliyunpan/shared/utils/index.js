import { kebabCase, camelCase, isPlainObject } from 'lodash';
import { userKeys, systemKeys, needRestartKeys } from '@shared/configKeys';
import { APP_THEME, GRAPHIC, NONE_SELECTED_FILES, SELECTED_ALL_FILES, RESOURCE_TAGS, IMAGE_SUFFIXES, AUDIO_SUFFIXES, VIDEO_SUFFIXES, DOCUMENT_SUFFIXES, SUPPORT_RTL_LOCALES } from '@shared/constants';
export const bytesToSize = (bytes, precision = 1) => {
    if (!bytes || bytes === 0)
        return '0 B';
    const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
    const idx = Math.floor(Math.log(bytes) / Math.log(1024));
    return `${(bytes / Math.pow(1024, idx)).toFixed(precision)} ${units[idx]}`;
};
export const extractSpeedUnit = (speed) => {
    const m = (speed || '').match(/[A-Z]+/);
    return m ? m[0] : '';
};
export const calcProgress = (totalLength, completedLength, decimal = 2) => {
    if (!totalLength || totalLength === 0)
        return 0;
    const p = completedLength / totalLength * 100;
    return parseFloat(p.toFixed(decimal));
};
export const calcRatio = (totalLength, uploadLength) => {
    if (!totalLength || totalLength === 0)
        return 0;
    return parseFloat((uploadLength / totalLength).toFixed(2));
};
export const timeRemaining = (total, completed, speed) => {
    if (!speed || speed === 0)
        return Infinity;
    return Math.ceil((total - completed) / speed);
};
export const timeFormat = (seconds, opts = {}) => {
    const { prefix = '', suffix = '' } = opts;
    if (!seconds || seconds <= 0)
        return '-';
    if (seconds > 86400)
        return `${prefix}> 1 day${suffix}`;
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = seconds % 60;
    const parts = [];
    if (h > 0)
        parts.push(`${h} h`);
    if (m > 0)
        parts.push(`${m} m`);
    if (s > 0 || parts.length === 0)
        parts.push(`${s} s`);
    return `${prefix}${parts.join(' ')}${suffix}`;
};
export const localeDateTimeFormat = (timestamp, locale = 'zh-CN') => {
    if (!timestamp)
        return '';
    return new Date(timestamp).toLocaleDateString(locale);
};
export const ellipsis = (str, maxLen = 64) => {
    if (!str || str.length <= maxLen)
        return str;
    return str.slice(0, maxLen) + '...';
};
export const getFileName = (fullPath) => {
    if (!fullPath)
        return '';
    return fullPath.replace(/^.*[/\\]/, '');
};
export const getFileExtension = (filename) => {
    if (!filename)
        return '';
    const m = filename.match(/\.[^./\\]+$/);
    return m ? m[0].toLowerCase() : '';
};
export const removeExtensionDot = (ext) => ext.startsWith('.') ? ext.slice(1) : ext;
export const getFileNameFromFile = (file) => getFileName(file?.path || '');
export const getTaskName = (task, opts = {}) => {
    const bt = task?.bittorrent;
    if (bt?.info?.name)
        return bt.info.name;
    const files = task?.files;
    if (files?.length === 1)
        return getFileNameFromFile(files[0]);
    return '';
};
export const checkTaskIsBT = (task) => !!task?.bittorrent;
export const isMagnetTask = (task) => !!(task?.bittorrent && !task.bittorrent.info);
export const checkTaskIsSeeder = (task) => !!(task?.bittorrent && task.seeder === 'true');
export const checkTaskTitleIsEmpty = (task) => !getTaskName(task);
export const isTorrent = (file) => {
    const name = (file?.name || file?.path || '').toLowerCase();
    return name.endsWith('.torrent');
};
export const getAsBase64 = (file, callback) => {
    const reader = new FileReader();
    reader.onload = () => {
        const result = reader.result;
        const base64 = result.split(',')[1] || result;
        callback(base64);
    };
    reader.readAsDataURL(file);
};
export const getTaskUri = (task, withTracker = false) => {
    if (!checkTaskIsBT(task)) {
        return task?.files?.[0]?.uris?.[0]?.uri || '';
    }
    return buildMagnetLink(task, withTracker);
};
export const buildMagnetLink = (task, withTracker = false, btTracker = '') => {
    const infoHash = task?.infoHash || task?.bittorrent?.info?.infoHash;
    if (!infoHash)
        return '';
    const name = task?.bittorrent?.info?.name || '';
    let magnet = `magnet:?xt=urn:btih:${infoHash}`;
    if (name)
        magnet += `&dn=${encodeURIComponent(name)}`;
    if (withTracker && btTracker) {
        btTracker.split(',').forEach((t) => { if (t)
            magnet += `&tr=${encodeURIComponent(t)}`; });
    }
    return magnet;
};
export const getFileSelection = (files) => {
    if (!files?.length)
        return NONE_SELECTED_FILES;
    const selected = files.filter((f) => f.selected === 'true' || f.selected === true);
    if (selected.length === files.length)
        return SELECTED_ALL_FILES;
    if (selected.length === 0)
        return NONE_SELECTED_FILES;
    return selected.map((f) => f.index).join(',');
};
export const listTorrentFiles = (files) => (files || []).map((f, i) => ({
    ...f,
    idx: i + 1,
    extension: getFileExtension(f.path || '')
}));
export const mergeTaskResult = (response) => {
    if (!Array.isArray(response))
        return [];
    return response.reduce((acc, item) => {
        if (Array.isArray(item)) {
            item.forEach((sub) => {
                if (Array.isArray(sub))
                    acc.push(...sub);
                else
                    acc.push(sub);
            });
        }
        else {
            acc.push(item);
        }
        return acc;
    }, []);
};
export const decodeThunderLink = (url) => {
    if (!url.toLowerCase().startsWith('thunder://'))
        return url;
    const b64 = url.slice('thunder://'.length);
    const decoded = Buffer.from(b64, 'base64').toString('utf-8');
    return decoded.replace(/^AA/, '').replace(/ZZ$/, '');
};
export const splitTaskLinks = (links) => {
    if (!links)
        return [];
    return links
        .split('\n')
        .map((l) => l.trim())
        .filter(Boolean)
        .map((l) => (l.toLowerCase().startsWith('thunder://') ? decodeThunderLink(l) : l));
};
export const splitTextRows = (text) => text.split('\n').map((l) => l.trim()).filter(Boolean);
export const convertCommaToLine = (str) => str.replace(/,/g, '\n');
export const convertLineToComma = (str) => str.replace(/\n/g, ',');
export const detectResource = (content) => RESOURCE_TAGS.some((tag) => content.toLowerCase().includes(tag));
export const buildFileList = (rawFile) => [{
        uid: Date.now().toString(),
        name: rawFile.name,
        status: 'ready',
        file: rawFile
    }];
export const needCheckCopyright = (links) => /\/(av|BV)/i.test(links);
export const isAudioOrVideo = (uri) => {
    const ext = getFileExtension(uri);
    return AUDIO_SUFFIXES.includes(ext) || VIDEO_SUFFIXES.includes(ext);
};
export const filterVideoFiles = (files) => files.filter((f) => VIDEO_SUFFIXES.includes(getFileExtension(f.path || f.name || '')));
export const filterAudioFiles = (files) => files.filter((f) => AUDIO_SUFFIXES.includes(getFileExtension(f.path || f.name || '')));
export const filterImageFiles = (files) => files.filter((f) => IMAGE_SUFFIXES.includes(getFileExtension(f.path || f.name || '')));
export const filterDocumentFiles = (files) => files.filter((f) => DOCUMENT_SUFFIXES.includes(getFileExtension(f.path || f.name || '')));
export const formatOptionsForEngine = (options = {}) => {
    const result = {};
    Object.keys(options).forEach((key) => {
        const kebab = kebabCase(key);
        if (Array.isArray(options[key])) {
            result[kebab] = options[key].join('\n');
        }
        else {
            result[kebab] = `${options[key]}`;
        }
    });
    return result;
};
export const separateConfig = (options = {}) => {
    const user = {};
    const system = {};
    const others = {};
    Object.keys(options).forEach((key) => {
        if (userKeys.includes(key))
            user[key] = options[key];
        else if (systemKeys.includes(key))
            system[key] = options[key];
        else
            others[key] = options[key];
    });
    return { user, system, others };
};
export const changeKeysCase = (obj, caseConverter) => {
    if (!isPlainObject(obj))
        return obj;
    const result = {};
    Object.keys(obj).forEach((key) => { result[caseConverter(key)] = obj[key]; });
    return result;
};
export const changeKeysToCamelCase = (obj) => changeKeysCase(obj, camelCase);
export const changeKeysToKebabCase = (obj) => changeKeysCase(obj, kebabCase);
export const compactUndefined = (arr) => arr.filter((item) => item !== undefined);
export const checkIsNeedRestart = (changed) => Object.keys(changed).some((key) => needRestartKeys.includes(key));
export const checkIsNeedRun = (enable, lastTime, interval) => {
    if (!enable)
        return false;
    if (!lastTime)
        return true;
    return Date.now() - lastTime > interval;
};
export const parseHeader = (header) => {
    if (!header)
        return {};
    const result = {};
    header.split('\n').forEach((line) => {
        const [key, ...rest] = line.split(':');
        if (key && rest.length)
            result[camelCase(key.trim())] = rest.join(':').trim();
    });
    return result;
};
export const buildRpcUrl = ({ port, secret }) => {
    const auth = secret ? `token:${secret}@` : '';
    return `http://${auth}127.0.0.1:${port}/jsonrpc`;
};
export const generateRandomInt = (min, max) => Math.floor(Math.random() * (max - min + 1)) + min;
export const intersection = (a1, a2) => a1.filter((v) => a2.includes(v));
export const cloneArray = (arr, reversed = false) => {
    const copy = [...arr];
    return reversed ? copy.reverse() : copy;
};
export const pushItemToFixedLengthArray = (arr, item, maxLen) => {
    const filtered = arr.filter((i) => i !== item);
    filtered.unshift(item);
    return filtered.slice(0, maxLen);
};
export const removeArrayItem = (arr, item) => arr.filter((i) => i !== item);
export const diffConfig = (current, next) => {
    const diff = {};
    Object.keys(next).forEach((key) => {
        if (JSON.stringify(current[key]) !== JSON.stringify(next[key])) {
            diff[key] = next[key];
        }
    });
    return diff;
};
export const getInverseTheme = (theme) => theme === APP_THEME.LIGHT ? APP_THEME.DARK : APP_THEME.LIGHT;
export const calcFormLabelWidth = (locale) => locale === 'de' ? '28%' : '25%';
export const isRTL = (locale) => SUPPORT_RTL_LOCALES.includes(locale);
export const getLangDirection = (locale) => isRTL(locale) ? 'rtl' : 'ltr';
export const bitfieldToPercent = (text) => {
    if (!text)
        return 0;
    let filled = 0;
    for (const ch of text) {
        const n = parseInt(ch, 16);
        if (isNaN(n))
            continue;
        for (let bit = 3; bit >= 0; bit--)
            if ((n >> bit) & 1)
                filled++;
    }
    return Math.round((filled / (text.length * 4)) * 100);
};
export const bitfieldToGraphic = (text) => {
    if (!text)
        return '';
    return text.split('').map((ch) => {
        const n = parseInt(ch, 16);
        if (isNaN(n))
            return GRAPHIC[0];
        return GRAPHIC[Math.min(3, Math.round((n / 15) * 3))];
    }).join('');
};
export const changedConfig = {
    basic: {}, advanced: {}
};
export const backupConfig = {
    theme: undefined, locale: undefined
};
