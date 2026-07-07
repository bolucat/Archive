import { AppWindow, createElectronWindow, createReaderWindow, Referer, ua } from './window'
import path from 'path'
import is from 'electron-is'
import { app, BrowserWindow, dialog, ipcMain, Menu, net, powerSaveBlocker, session, shell } from 'electron'
import { chmodSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'fs'
import { exec, execFile, spawn, SpawnOptions } from 'child_process'
import os from 'os'
import { ShowError } from './dialog'
import { getStaticPath, getUserDataPath } from '../utils/mainfile'
import { registerMediaImageCacheIpc } from '../mediaImageCache'
import { createHash } from 'crypto'
import { getMotrixApplicationRpcPort } from '../aria/runtime'
import { pathToFileURL } from 'url'
import { requestPanHub } from './panHubRequest'
import {
  getBookMeta,
  isBookIndexed,
  clearBookData,
  storeChunks,
  storeEmbeddings,
  storeMeta,
  hybridSearch,
  writeMemory,
  searchMemories,
  listMemories,
  deleteMemory,
  listSkills,
  getSkill,
  setSkillEnabled,
  recordMetric,
  getMetrics,
  wipeAllData,
  destroyDb
} from '../reedy/ReedyService'

let psbId: any

const QUARK_DOWNLOAD_AGENT =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) quark-cloud-drive/2.5.56 Chrome/100.0.4896.160 Electron/18.3.5.12-a038f7b798 Safari/537.36 Channel/pckk_other_ch'

function shellQuote(value: string): string {
  return `'${value.replace(/'/g, `'\\''`)}'`
}

function pathToFileUrl(filePath: string): string {
  const normalized = path.resolve(filePath).replace(/\\/g, '/')
  return 'file://' + (normalized.startsWith('/') ? normalized : '/' + normalized)
}

function findSoffice(): string {
  const candidates = [
    process.env.LIBREOFFICE_PATH || '',
    process.env.SOFFICE_PATH || '',
    is.macOS() ? '/Applications/LibreOffice.app/Contents/MacOS/soffice' : '',
    is.windows() ? 'C:\\Program Files\\LibreOffice\\program\\soffice.exe' : '',
    is.windows() ? 'C:\\Program Files (x86)\\LibreOffice\\program\\soffice.exe' : '',
    '/usr/bin/libreoffice',
    '/usr/local/bin/libreoffice',
    '/opt/homebrew/bin/libreoffice',
    '/usr/bin/soffice',
    '/usr/local/bin/soffice',
    '/opt/homebrew/bin/soffice'
  ].filter(Boolean)
  for (const candidate of candidates) {
    if (existsSync(candidate)) return candidate
  }
  const names = is.windows() ? ['soffice.exe', 'libreoffice.exe'] : ['soffice', 'libreoffice']
  const pathDirs = (process.env.PATH || '').split(path.delimiter).filter(Boolean)
  for (const dir of pathDirs) {
    for (const name of names) {
      const candidate = path.join(dir, name)
      if (existsSync(candidate)) return candidate
    }
  }
  return ''
}

function convertOfficeFileToPdf(soffice: string, inputPath: string, outDir: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const userInstall = path.join(outDir, 'lo-profile').replace(/\\/g, '/')
    const args = [
      '--headless',
      '--nologo',
      '--nofirststartwizard',
      `-env:UserInstallation=file://${userInstall}`,
      '--convert-to',
      'pdf',
      '--outdir',
      outDir,
      inputPath
    ]
    execFile(soffice, args, { windowsHide: true }, (err, stdout, stderr) => {
      if (err) {
        reject(new Error((stderr || stdout || err.message || '').trim() || 'LibreOffice 转换失败'))
      } else {
        resolve()
      }
    })
  })
}

async function runBundledCloudDriveCli(args: string[]) {
  const modulePath = path.join(app.getAppPath(), 'clouddrive-cli', 'core', 'commands.mjs')
  const mod = await import(pathToFileURL(modulePath).href)
  return mod.runBoxPlayerCli(args)
}

function pushCliOption(argv: string[], flag: string, value: unknown) {
  if (value === undefined || value === null || value === '' || value === false) return
  if (value === true) {
    argv.push(flag)
    return
  }
  argv.push(flag, String(value))
}

function buildOpenDataLoaderPdfArgv(data: any): string[] {
  const inputPath = String(data?.inputPath || '')
  const outputDir = String(data?.outputDir || '')
  if (!inputPath) throw new Error('请选择要转换的 PDF 文件或文件夹')
  // if (!outputDir) throw new Error('请选择输出文件夹')
  const argv = ['docs', 'convert', inputPath, '--output', outputDir, '--json']
  if (!data?.format) argv.push('--pdf-format', 'json,html,markdown,text')
  const optionMap: Array<[string, string]> = [
    ['format', '--pdf-format'],
    ['password', '--pdf-password'],
    ['contentSafetyOff', '--pdf-content-safety-off'],
    ['replaceInvalidChars', '--pdf-replace-invalid-chars'],
    ['tableMethod', '--pdf-table-method'],
    ['readingOrder', '--pdf-reading-order'],
    ['markdownPageSeparator', '--pdf-markdown-page-separator'],
    ['textPageSeparator', '--pdf-text-page-separator'],
    ['htmlPageSeparator', '--pdf-html-page-separator'],
    ['imageOutput', '--pdf-image-output'],
    ['imageFormat', '--pdf-image-format'],
    ['imageDir', '--pdf-image-dir'],
    ['pages', '--pdf-pages'],
    ['hybrid', '--pdf-hybrid'],
    ['hybridMode', '--pdf-hybrid-mode'],
    ['hybridUrl', '--pdf-hybrid-url'],
    ['hybridTimeout', '--pdf-hybrid-timeout'],
    ['hybridHancomAiRegionlistStrategy', '--pdf-hybrid-hancom-ai-regionlist-strategy'],
    ['hybridHancomAiOcrStrategy', '--pdf-hybrid-hancom-ai-ocr-strategy'],
    ['hybridHancomAiImageCache', '--pdf-hybrid-hancom-ai-image-cache'],
    ['threads', '--pdf-threads'],
  ]
  for (const [key, flag] of optionMap) pushCliOption(argv, flag, data?.[key])
  const booleanMap: Array<[string, string]> = [
    ['sanitize', '--pdf-sanitize'],
    ['keepLineBreaks', '--pdf-keep-line-breaks'],
    ['useStructTree', '--pdf-use-struct-tree'],
    ['markdownWithHtml', '--pdf-markdown-with-html'],
    ['includeHeaderFooter', '--pdf-include-header-footer'],
    ['detectStrikethrough', '--pdf-detect-strikethrough'],
    ['hybridFallback', '--pdf-hybrid-fallback'],
    ['toStdout', '--pdf-to-stdout'],
    ['verbose', '--pdf-verbose'],
  ]
  for (const [key, flag] of booleanMap) pushCliOption(argv, flag, data?.[key] === true)
  return argv
}

export default class ipcEvent {
  private constructor() {
  }

  static handleEvents() {
    this.handleWebToElectron()
    this.handleWebToElectronCB()
    this.handleShowContextMenu()
    this.handleWebShowOpenDialogSync()
    this.handleWebShowSaveDialogSync()
    this.handleWebShowItemInFolder()
    this.handleWebPlatformSync()
    this.handleWebSpawnSync()
    this.handleWebExecSync()
    this.handleWebSaveTheme()
    this.handleWebClearCookies()
    this.handleWebGetCookies()
    this.handleWebQuarkAccountInfo()
    this.handleWebQuarkDownloadUrl()
    this.handleWebSetCookies()
    this.handleWebClearCache()
    this.handleWebReload()
    this.handleWebRelaunch()
    this.handleWebRelaunchAria()
    this.handleWebSetProgressBar()
    this.handleWebShutDown()
    this.handleWebSetProxy()
    this.handlePanHubRequest()
    this.handleOfficePreviewConvertToPdf()
    this.handleOpenDataLoaderConvertPdf()
    this.handleWebOpenWindow()
    // this.handleWebOpenLyric()
    // this.handleWebSendLyric()
    // this.handleWebCloseLyric()
    this.handleWebOpenUrl()
    this.handleExportCliTokens()
    this.handleInstallCli()
    this.handlePowerSaveBlocker()
    if (app.isPackaged) {
      this.installCli(true).catch((err: any) => {
        console.warn('Auto install BoxPlayer CLI failed', err?.message || err)
      })
    }
    registerMediaImageCacheIpc()
    this.handleReedy()
  }

  private static handleWebToElectron() {
    ipcMain.on('WebToElectron', async (event, data) => {
      let mainWindow = AppWindow.mainWindow
      if (data.cmd && data.cmd === 'close') {
        if (mainWindow && !mainWindow.isDestroyed()) mainWindow.hide()
      } else if (data.cmd && data.cmd === 'relaunch') {
        if (mainWindow && !mainWindow.isDestroyed()) {
          mainWindow.destroy()
          mainWindow = undefined
        }
        try {
          app.relaunch({ args: process.argv.slice(1).concat(['--relaunch']) })
          app.exit(0)
        } catch {
        }
      } else if (data.cmd && data.cmd === 'exit') {
        if (mainWindow && !mainWindow.isDestroyed()) {
          mainWindow.destroy()
          mainWindow = undefined
        }
        try {
          app.exit(0)
        } catch {
        }
      } else if (data.cmd && data.cmd === 'minsize') {
        if (mainWindow && !mainWindow.isDestroyed()) mainWindow.minimize()
      } else if (data.cmd && data.cmd === 'open-reader-window') {
        createReaderWindow(data.bookData)
      } else if (data.cmd && data.cmd === 'maxsize') {
        if (mainWindow && !mainWindow.isDestroyed()) {
          if (mainWindow.isMaximized()) {
            mainWindow.unmaximize()
          } else {
            mainWindow.maximize()
          }
        }
      } else if (data.cmd && (Object.hasOwn(data.cmd, 'launchStart')
        || Object.hasOwn(data.cmd, 'launchStartShow'))) {
        const launchStart = data.cmd.launchStart
        const launchStartShow = data.cmd.launchStartShow
        const appName = path.basename(process.execPath)
        // 设置开机自启
        const settings: Electron.Settings = {
          openAtLogin: launchStart,
          path: process.execPath
        }
        // 显示主窗口
        if (is.macOS()) {
          settings.openAsHidden = !launchStartShow
        } else {
          settings.args = [
            '--processStart', `${appName}`,
            '--process-start-args', `"--hidden"`
          ]
          !launchStartShow && settings.args.push('--openAsHidden')
        }
        app.setLoginItemSettings(settings)
      } else if (data.cmd && data.cmd === 'preventSleep') {
        if (data.flag) {
          if (psbId && powerSaveBlocker.isStarted(psbId)) {
            return
          }
          psbId = powerSaveBlocker.start('prevent-app-suspension')
        } else {
          if (typeof psbId === 'undefined' || !powerSaveBlocker.isStarted(psbId)) {
            return
          }
          powerSaveBlocker.stop(psbId)
          psbId = undefined
        }
      } else if (data.cmd && data.cmd === 'downloadProgress') {
        const win = AppWindow.mainWindow
        if (win && !win.isDestroyed()) {
          const progress = typeof data.progress === 'number' ? data.progress : -1
          win.setProgressBar(progress)
        }
      } else if (data.cmd && data.cmd === 'downloadCompleted') {
        const { Notification } = require('electron') as typeof import('electron')
        if (Notification.isSupported()) {
          const n = new Notification({
            title: '下载完成',
            body: data.fileName ? `${data.fileName} 已下载完成` : '文件下载完成'
          })
          n.on('click', () => { AppWindow.mainWindow?.show(); AppWindow.mainWindow?.focus() })
          n.show()
        }
      } else {
        event.sender.send('ElectronToWeb', 'mainsenddata')
      }
    })
  }

  private static handleWebToElectronCB() {
    ipcMain.on('WebToElectronCB', (event, data) => {
      const mainWindow = AppWindow.mainWindow
      if (data.cmd && data.cmd === 'maxsize') {
        if (mainWindow && !mainWindow.isDestroyed()) {
          if (mainWindow.isMaximized()) {
            mainWindow.unmaximize()
            event.returnValue = 'unmaximize'
          } else {
            mainWindow.maximize()
            event.returnValue = 'maximize'
          }
        }
      } else {
        event.returnValue = 'backdata'
      }
    })
  }

  private static handleShowContextMenu() {
    ipcMain.on('show-context-menu', (event, params) => {
      const { showCut, showCopy, showPaste } = params
      const window = BrowserWindow.fromWebContents(event.sender)
      // 制作右键菜单
      let template: Array<Electron.MenuItemConstructorOptions> = [
        // 设置选项是否可见
        { role: 'selectAll', label: '全选' },
        { role: 'copy', label: '复制', visible: showCopy },
        { role: 'cut', label: '剪切', visible: showCut },
        { role: 'paste', label: '粘贴', visible: showPaste },
        { role: 'undo', label: '撤销' }
      ]
      // 显示菜单
      const contextMenu = Menu.buildFromTemplate(template)
      contextMenu.popup({ window })
    })
  }

  private static handleWebShowOpenDialogSync() {
    ipcMain.on('WebShowOpenDialogSync', (event, config) => {
      dialog.showOpenDialog(AppWindow.mainWindow!, config).then((result) => {
        event.returnValue = result.filePaths
      })
    })
  }

  private static handleWebShowSaveDialogSync() {
    ipcMain.on('WebShowSaveDialogSync', (event, config) => {
      dialog.showSaveDialog(AppWindow.mainWindow!, config).then((result) => {
        event.returnValue = result.filePath || ''
      })
    })
  }

  private static handleWebShowItemInFolder() {
    ipcMain.on('WebShowItemInFolder', (event, fullPath) => {
      for (let i = 0; i < 5; i++) {
        if (existsSync(fullPath)) break
        if (fullPath.lastIndexOf(path.sep) > 0) {
          fullPath = fullPath.substring(0, fullPath.lastIndexOf(path.sep))
        } else return
      }
      if (fullPath.length > 2) shell.showItemInFolder(fullPath)
    })
  }

  private static handleWebPlatformSync() {
    ipcMain.on('WebPlatformSync', (event) => {
      const asarPath = app.getAppPath()
      const appPath = app.getPath('userData')
      event.returnValue = {
        platform: process.platform,
        arch: process.arch,
        version: process.version,
        execPath: process.execPath,
        appPath: appPath,
        asarPath: asarPath,
        argv0: process.argv0
      }
    })
  }

  private static handleWebSpawnSync() {
    ipcMain.on('WebSpawnSync', (event, data) => {
      try {
        const options: SpawnOptions = {
          shell: true,
          stdio: 'ignore',
          windowsVerbatimArguments: true,
          ...data.options
        }
        const argsToStr = (args: string) => is.windows() ? `"${args}"` : `'${args}'`
        if ((is.windows() || is.macOS()) && !existsSync(data.command)) {
          event.returnValue = { error: '找不到文件' + data.command }
          ShowError('找不到文件', data.command)
        } else {
          let command
          if (is.macOS()) {
            command = `open -a ${argsToStr(data.command)} ${data.command.includes('mpv.app') ? '--args ' : ''}`
          } else {
            command = `${argsToStr(data.command)}`
          }
          const subProcess = spawn(command, data.args, options)
          subProcess.unref()
          event.returnValue = {
            pid: subProcess.pid,
            subProcess: subProcess,
            execCmd: data,
            options: options,
            exitCode: subProcess.exitCode
          }
        }
      } catch (err: any) {
        event.returnValue = { error: err }
      }
    })
  }

  private static handleWebExecSync() {
    ipcMain.on('WebExecSync', (event, data) => {
      try {
        const cmdArguments = []
        cmdArguments.push(data.command)
        if (data.args) cmdArguments.push(...data.args)
        const finalCmd = cmdArguments.join(' ')
        exec(finalCmd, (err: any) => {
          event.returnValue = err
        })
        event.returnValue = ''
      } catch (err: any) {
        event.returnValue = { error: err }
      }
    })
  }

  private static handleWebSaveTheme() {
    ipcMain.on('WebSaveTheme', (event, data) => {
      try {
        const themeJson = getUserDataPath('theme.json')
        writeFileSync(themeJson, `{"theme":"${data.theme || ''}"}`, 'utf-8')
      } catch {
      }
    })
  }

  private static handleWebClearCookies() {
    ipcMain.on('WebClearCookies', (event, data) => {
      session.defaultSession.clearStorageData(data)
    })
  }

  private static handleWebGetCookies() {
    ipcMain.handle('WebGetCookies', async (event, data) => {
      return await session.defaultSession.cookies.get(data)
    })
  }

  private static handleWebQuarkAccountInfo() {
    ipcMain.handle('WebQuarkAccountInfo', async (_event, data: { serviceTicket?: string }) => {
      const serviceTicket = String(data?.serviceTicket || '')
      if (!serviceTicket) return { ok: false, status: 0, body: '', cookies: [], error: '夸克登录凭证为空' }
      const params = new URLSearchParams({ st: serviceTicket, lw: 'scan' })
      const url = `https://pan.quark.cn/account/info?${params.toString()}`
      const existingCookies = await session.defaultSession.cookies.get({ domain: 'quark.cn' })
      const cookieHeader = existingCookies
        .filter((cookie) => cookie.name && cookie.value)
        .map((cookie) => `${cookie.name}=${cookie.value}`)
        .join('; ')

      return await new Promise((resolve) => {
        const request = net.request({
          method: 'GET',
          url,
          useSessionCookies: true
        } as any)
        request.setHeader('Accept', 'application/json, text/plain, */*')
        request.setHeader('Accept-Language', 'zh-CN,zh;q=0.9')
        request.setHeader('Cache-Control', 'no-cache')
        request.setHeader('Pragma', 'no-cache')
        request.setHeader('Origin', 'https://pan.quark.cn')
        request.setHeader('Referer', 'https://pan.quark.cn/')
        request.setHeader('User-Agent', 'Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/94.0.4606.71 Safari/537.36 Core/1.94.225.400 QQBrowser/12.2.5544.400')
        if (cookieHeader) request.setHeader('Cookie', cookieHeader)
        request.on('response', (response) => {
          const chunks: Buffer[] = []
          response.on('data', (chunk) => chunks.push(Buffer.from(chunk)))
          response.on('end', async () => {
            const body = Buffer.concat(chunks).toString('utf8')
            const setCookie = response.headers['set-cookie']
            const setCookieList = Array.isArray(setCookie) ? setCookie : (setCookie ? [String(setCookie)] : [])
            for (const rawCookie of setCookieList) {
              const cookie = ipcEvent.parseSetCookie(rawCookie, 'https://pan.quark.cn')
              if (cookie) {
                try {
                  await session.defaultSession.cookies.set(cookie)
                } catch (err) {
                  console.error(err)
                }
              }
            }
            const cookies = await session.defaultSession.cookies.get({ domain: 'quark.cn' })
            resolve({ ok: response.statusCode >= 200 && response.statusCode < 300, status: response.statusCode, body, cookies })
          })
        })
        request.on('error', (error) => resolve({ ok: false, status: 0, body: '', cookies: [], error: error.message }))
        request.end()
      })
    })
  }

  private static handleWebQuarkDownloadUrl() {
    ipcMain.handle('WebQuarkDownloadUrl', async (_event, data: { fileId?: string; cookie?: string }) => {
      const fileId = String(data?.fileId || '')
      if (!fileId) return { ok: false, status: 0, body: '', cookies: [], error: '夸克文件 ID 为空' }
      const params = new URLSearchParams({
        pr: 'ucpro',
        fr: 'pc',
        sys: 'win32',
        ve: '2.5.56',
        ut: '',
        guid: ''
      })
      const url = `https://drive-pc.quark.cn/1/clouddrive/file/download?${params.toString()}`
      const existingCookies = await session.defaultSession.cookies.get({ domain: 'quark.cn' })
      const cookieHeader = String(data?.cookie || '') || existingCookies
        .filter((cookie) => cookie.name && cookie.value)
        .map((cookie) => `${cookie.name}=${cookie.value}`)
        .join('; ')

      return await new Promise((resolve) => {
        const request = net.request({
          method: 'POST',
          url,
          useSessionCookies: true
        } as any)
        request.setHeader('Accept', 'application/json, text/plain, */*')
        request.setHeader('Accept-Language', 'zh-CN,zh;q=0.9')
        request.setHeader('Content-Type', 'application/json')
        request.setHeader('Origin', 'https://pan.quark.cn')
        request.setHeader('Referer', 'https://pan.quark.cn/')
        request.setHeader('User-Agent', QUARK_DOWNLOAD_AGENT)
        if (cookieHeader) request.setHeader('Cookie', cookieHeader)
        request.on('response', (response) => {
          const chunks: Buffer[] = []
          response.on('data', (chunk) => chunks.push(Buffer.from(chunk)))
          response.on('end', async () => {
            const body = Buffer.concat(chunks).toString('utf8')
            const setCookie = response.headers['set-cookie']
            const setCookieList = Array.isArray(setCookie) ? setCookie : (setCookie ? [String(setCookie)] : [])
            for (const rawCookie of setCookieList) {
              const cookie = ipcEvent.parseSetCookie(rawCookie, 'https://pan.quark.cn')
              if (cookie) {
                try {
                  await session.defaultSession.cookies.set(cookie)
                } catch (err) {
                  console.error(err)
                }
              }
            }
            const cookies = await session.defaultSession.cookies.get({ domain: 'quark.cn' })
            resolve({ ok: response.statusCode >= 200 && response.statusCode < 300, status: response.statusCode, body, cookies })
          })
        })
        request.on('error', (error) => resolve({ ok: false, status: 0, body: '', cookies: [], error: error.message }))
        request.write(JSON.stringify({ fids: [fileId] }))
        request.end()
      })
    })
  }

  private static parseSetCookie(rawCookie: string, defaultUrl: string) {
    const parts = rawCookie.split(';').map((part) => part.trim()).filter(Boolean)
    const [nameValue, ...attrs] = parts
    if (!nameValue || !nameValue.includes('=')) return undefined
    const [name, ...valueParts] = nameValue.split('=')
    const cookie: any = {
      url: defaultUrl,
      name,
      value: valueParts.join('='),
      path: '/',
      secure: true
    }
    for (const attr of attrs) {
      const [key, ...attrValueParts] = attr.split('=')
      const lowerKey = key.toLowerCase()
      const attrValue = attrValueParts.join('=')
      if (lowerKey === 'domain' && attrValue) cookie.domain = attrValue
      else if (lowerKey === 'path' && attrValue) cookie.path = attrValue
      else if (lowerKey === 'expires' && attrValue) {
        const expires = Date.parse(attrValue)
        if (Number.isFinite(expires)) cookie.expirationDate = Math.floor(expires / 1000)
      } else if (lowerKey === 'max-age' && attrValue) {
        const maxAge = Number(attrValue)
        if (Number.isFinite(maxAge)) cookie.expirationDate = Math.floor(Date.now() / 1000) + maxAge
      } else if (lowerKey === 'secure') cookie.secure = true
      else if (lowerKey === 'httponly') cookie.httpOnly = true
    }
    return cookie
  }

  private static handleWebSetCookies() {
    ipcMain.on('WebSetCookies', (event, data) => {
      for (let i = 0, maxi = data.length; i < maxi; i++) {
        const cookie = {
          url: data[i].url,
          name: data[i].name,
          value: data[i].value,
          domain: '.' + data[i].url.substring(data[i].url.lastIndexOf('/') + 1),
          secure: data[i].url.indexOf('https://') == 0,
          expirationDate: data[i].expirationDate
        }
        session.defaultSession.cookies.set(cookie).catch((err: any) => console.error(err))
      }
    })
  }

  private static handleWebClearCache() {
    ipcMain.on('WebClearCache', (event, data) => {
      if (data.cache) {
        session.defaultSession.clearCache()
        session.defaultSession.clearAuthCache()
      } else {
        session.defaultSession.clearStorageData(data)
      }
    })
  }

  private static handleWebReload() {
    ipcMain.on('WebReload', (event, data) => {
      if (AppWindow.mainWindow && !AppWindow.mainWindow.isDestroyed()) AppWindow.mainWindow.reload()
    })
  }

  private static handleWebRelaunch() {
    ipcMain.on('WebRelaunch', (event, data) => {
      app.relaunch()
      try {
        app.exit()
      } catch {
      }
    })
  }

  private static handleWebRelaunchAria() {
    ipcMain.handle('WebRelaunchAria', async (event, data) => {
      return getMotrixApplicationRpcPort()
    })
  }

  private static handleWebSetProgressBar() {
    ipcMain.on('WebSetProgressBar', (event, data) => {
      if (AppWindow.mainWindow && !AppWindow.mainWindow.isDestroyed()) {
        if (data.pro) {

          AppWindow.mainWindow.setProgressBar(data.pro, { mode: data.mode || 'normal' })
        } else AppWindow.mainWindow.setProgressBar(-1)
      }
    })
  }

  private static handleWebShutDown() {
    ipcMain.on('WebShutDown', (event, data) => {
      if (is.macOS()) {
        const shutdownCmd = 'osascript -e \'tell application "System Events" to shut down\''
        exec(shutdownCmd, (err: any) => {
          if (data.quitApp) {
            try {
              app.exit()
            } catch {
            }
          }
          if (err) {
            // donothing
          }
        })
      } else {
        const cmdArguments = ['shutdown']
        if (is.linux()) {
          if (data.sudo) {
            cmdArguments.unshift('sudo')
          }
          cmdArguments.push('-h')
          cmdArguments.push('now')
        }
        if (is.windows()) {
          cmdArguments.push('-s')
          cmdArguments.push('-f')
          cmdArguments.push('-t 0')
        }

        const finalcmd = cmdArguments.join(' ')

        exec(finalcmd, (err: any) => {
          if (data.quitApp) {
            try {
              app.exit()
            } catch {
            }
          }
          if (err) {
            // donothing
          }
        })
      }
    })
  }

  private static handleWebSetProxy() {
    ipcMain.on('WebSetProxy', (event, data) => {
      // if (data.proxyUrl) app.commandLine.appendSwitch('proxy-server', data.proxyUrl)
      // else app.commandLine.removeSwitch('proxy-server')
      console.log(JSON.stringify(data))
      if (data.proxyUrl) {
        session.defaultSession.setProxy({ proxyRules: data.proxyUrl })
      } else {
        session.defaultSession.setProxy({})
      }
    })
  }

  private static handlePanHubRequest() {
    ipcMain.handle('PanHub:request', async (_event, data) => requestPanHub(data))
  }

  private static handleOfficePreviewConvertToPdf() {
    ipcMain.handle('OfficePreview:convertToPdf', async (_event, data: { sourceUrl?: string; fileName?: string }) => {
      try {
        const sourceUrl = data?.sourceUrl || ''
        const fileName = path.basename(data?.fileName || 'document')
        if (!sourceUrl) return { ok: false, error: '文档预览地址为空' }
        const soffice = findSoffice()
        if (!soffice) {
          return {
            ok: false,
            error: '未找到 LibreOffice。请安装 LibreOffice 后重试，或继续使用网盘自带预览。'
          }
        }

        const hash = createHash('sha1').update(sourceUrl + fileName).digest('hex')
        const previewRoot = path.join(app.getPath('userData'), 'office-preview')
        const workDir = path.join(previewRoot, hash)
        mkdirSync(workDir, { recursive: true })
        const ext = path.extname(fileName) || '.office'
        const inputPath = path.join(workDir, `source${ext}`)
        const outputPath = path.join(workDir, 'source.pdf')
        if (!existsSync(outputPath)) {
          const response = await fetch(sourceUrl)
          if (!response.ok) return { ok: false, error: `文档下载失败：${response.status}` }
          const arrayBuffer = await response.arrayBuffer()
          writeFileSync(inputPath, Buffer.from(arrayBuffer))
          await convertOfficeFileToPdf(soffice, inputPath, workDir)
        }
        if (!existsSync(outputPath)) return { ok: false, error: 'LibreOffice 未生成 PDF 文件' }
        return { ok: true, pdfUrl: pathToFileUrl(outputPath) }
      } catch (err: any) {
        return { ok: false, error: err?.message || '文档转换 PDF 失败' }
      }
    })
  }

  private static handleOpenDataLoaderConvertPdf() {
    const handler = async (_event: any, data: any) => {
      try {
        const inputPath = String(data?.inputPath || '')
        if (!inputPath) return { ok: false, error: '请选择要转换的 PDF 文件' }
        const baseName = path.basename(inputPath, path.extname(inputPath))
        const hash = createHash('sha1').update(inputPath).digest('hex').slice(0, 12)
        const outputDir = path.join(app.getPath('userData'), 'pdf-tools-output', `${baseName}-${hash}`)
        try { rmSync(outputDir, { recursive: true, force: true }) } catch {}
        mkdirSync(outputDir, { recursive: true })
        const argv = buildOpenDataLoaderPdfArgv({ ...data, outputDir })
        const result = await runBundledCloudDriveCli(argv)
        const stdout = result.stdout || ''
        const stderr = result.stderr || ''
        let body: any = stdout.trim()
        try {
          body = stdout ? JSON.parse(stdout) : {}
        } catch {
        }
        if (result.exitCode !== 0) {
          const errMsg = typeof body === 'object' ? body?.error?.message || body?.error || stdout || stderr : (stdout || stderr)
          console.error('[opendataloader:convertPdf] CLI failed', { exitCode: result.exitCode, stdout, stderr })
          return { ok: false, error: errMsg, exitCode: result.exitCode, stderr }
        }

        const fmtMap: { [k: string]: 'json' | 'html' | 'md' | 'text' } = {
          '.json': 'json',
          '.html': 'html',
          '.htm': 'html',
          '.md': 'md',
          '.markdown': 'md',
          '.txt': 'text'
        }
        const contents: { json: any; html: string; md: string; text: string; files: any[] } = {
          json: null, html: '', md: '', text: '', files: []
        }
        const walk = (dir: string): string[] => {
          const out: string[] = []
          try {
            for (const name of readdirSync(dir)) {
              const fp = path.join(dir, name)
              try {
                const st = statSync(fp)
                if (st.isDirectory()) out.push(...walk(fp))
                else if (st.isFile()) out.push(fp)
              } catch {}
            }
          } catch {}
          return out
        }
        const allFiles = walk(outputDir)
        for (const filePath of allFiles) {
          const ext = path.extname(filePath).toLowerCase()
          const kind = fmtMap[ext]
          const entry: any = { path: filePath, name: path.basename(filePath), format: kind || ext.slice(1) || 'unknown' }
          contents.files.push(entry)
          if (!kind) continue
          try {
            const text = readFileSync(filePath, 'utf8')
            entry.size = text.length
            if (kind === 'json' && !contents.json) {
              try { contents.json = JSON.parse(text) } catch { contents.json = text }
            } else if (kind === 'html' && !contents.html) contents.html = text
            else if (kind === 'md' && !contents.md) contents.md = text
            else if (kind === 'text' && !contents.text) contents.text = text
          } catch (e: any) {
            console.warn('[opendataloader:convertPdf] read file failed', filePath, e?.message)
          }
        }

        const merged = (body && typeof body === 'object') ? { ...body, ...contents } : contents
        if (!contents.json && !contents.html && !contents.md && !contents.text) {
          console.warn('[opendataloader:convertPdf] empty output, files=', allFiles, 'stdout=', stdout.slice(0, 500))
        }
        return { ok: true, result: merged }
      } catch (err: any) {
        console.error('[opendataloader:convertPdf] error', err)
        return { ok: false, error: err?.message || 'OpenDataLoader PDF 转换失败' }
      }
    }
    ipcMain.handle('opendataloader:convertPdf', handler)
    ipcMain.handle('OpenDataLoader:convertPdf', handler)
  }

  private static handleWebOpenWindow() {
    let winWidth = AppWindow.winWidth
    if (winWidth < 1080) winWidth = 1080
    ipcMain.on('WebOpenWindow', (event, data) => {
      const win = createElectronWindow(winWidth, AppWindow.winHeight, true, 'main2', data.theme)
      win.on('ready-to-show', function() {
        win.webContents.send('setPage', data)
        win.setTitle('预览窗口')
        win.show()
        if (data.page === 'PageBookReader') {
          setTimeout(() => win.setFullScreen(true), 200)
        }
      })
    })
  }

  // private static lyricWin: BrowserWindow | null = null
  //
  // private static handleWebOpenLyric() {
  //   ipcMain.on('WebOpenLyric', () => {
  //     if (ipcEvent.lyricWin && !ipcEvent.lyricWin.isDestroyed()) {
  //       ipcEvent.lyricWin.show()
  //       return
  //     }
  //     const win = new BrowserWindow({
  //       show: false,
  //       width: 900,
  //       height: 180,
  //       x: undefined,
  //       y: undefined,
  //       center: true,
  //       frame: false,
  //       transparent: true,
  //       hasShadow: false,
  //       resizable: true,
  //       skipTaskbar: true,
  //       alwaysOnTop: true,
  //       backgroundColor: '#00000000',
  //       webPreferences: {
  //         nodeIntegration: true,
  //         contextIsolation: false,
  //         webSecurity: false,
  //         sandbox: false,
  //         preload: getAsarPath('dist/electron/preload/index.js'),
  //       },
  //     })
  //     win.removeMenu()
  //
  //     if (!app.isPackaged) {
  //       win.loadURL(process.env.VITE_DEV_SERVER_URL + '/mainLyric.html')
  //     } else {
  //       win.loadFile(getStaticPath('mainLyric.html'))
  //     }
  //     win.on('ready-to-show', () => {
  //       win.webContents.send('setPage', { page: 'PageLyric' })
  //       win.show()
  //     })
  //     win.on('closed', () => {
  //       ipcEvent.lyricWin = null
  //     })
  //     ipcEvent.lyricWin = win
  //   })
  // }
  //
  // private static handleWebSendLyric() {
  //   ipcMain.on('WebSendLyric', (_event, data) => {
  //     if (ipcEvent.lyricWin && !ipcEvent.lyricWin.isDestroyed()) {
  //       ipcEvent.lyricWin.webContents.send('lyricData', data)
  //     }
  //   })
  // }
  //
  // private static handleWebCloseLyric() {
  //   ipcMain.on('WebCloseLyric', () => {
  //     if (ipcEvent.lyricWin && !ipcEvent.lyricWin.isDestroyed()) {
  //       ipcEvent.lyricWin.close()
  //       ipcEvent.lyricWin = null
  //     }
  //   })
  // }

  private static handleWebOpenUrl() {
    ipcMain.on('WebOpenUrl', (event, data) => {
      const win = new BrowserWindow({
        show: false,
        width: AppWindow.winWidth,
        height: AppWindow.winHeight,
        center: true,
        minWidth: 680,
        minHeight: 500,
        icon: getStaticPath('icon_256x256.ico'),
        useContentSize: true,
        frame: true,
        hasShadow: true,
        autoHideMenuBar: true,
        backgroundColor: data.theme && data.theme == 'dark' ? '#23232e' : '#ffffff',
        webPreferences: {
          spellcheck: false,
          devTools: is.dev(),
          sandbox: false,
          webSecurity: false,
          allowRunningInsecureContent: true,
          backgroundThrottling: false,
          enableWebSQL: false,
          disableBlinkFeatures: 'OutOfBlinkCors,SameSiteByDefaultCookies,CookiesWithoutSameSiteMustBeSecure'
        }
      })

      win.on('ready-to-show', function() {
        win.setTitle('预览窗口')
        win.show()
      })

      win.loadURL(data.PageUrl, {
        userAgent: ua,
        httpReferrer: Referer
      })
    })
  }

  private static handleExportCliTokens() {
    ipcMain.handle('ExportCliTokens', async (_event, data: { accounts: any[] }) => {
      try {
        const cliDir = path.join(os.homedir(), '.clouddrive-cli')
        const tokensPath = path.join(cliDir, 'tokens.json')
        const configPath = path.join(cliDir, 'config.json')
        mkdirSync(cliDir, { recursive: true })

        let existing: { accounts: any[] } = { accounts: [] }
        try {
          const raw = require('fs').readFileSync(tokensPath, 'utf8')
          existing = JSON.parse(raw)
          if (!Array.isArray(existing.accounts)) existing.accounts = []
        } catch {
          existing = { accounts: [] }
        }

        existing.accounts = Array.isArray(data.accounts) ? data.accounts : []

        let config: { defaults: Record<string, string> } = { defaults: {} }
        try {
          const raw = readFileSync(configPath, 'utf8')
          config = JSON.parse(raw)
          if (!config.defaults || typeof config.defaults !== 'object') config.defaults = {}
        } catch {
          config = { defaults: {} }
        }

        const accountKeys = new Set(existing.accounts.map((account) => `${account.provider}/${account.accountId}`))
        for (const [provider, accountId] of Object.entries(config.defaults)) {
          if (!accountKeys.has(`${provider}/${accountId}`)) delete config.defaults[provider]
        }
        for (const account of existing.accounts) {
          if (account.provider && account.accountId && !config.defaults[account.provider]) {
            config.defaults[account.provider] = account.accountId
          }
        }

        writeFileSync(tokensPath, JSON.stringify(existing, null, 2) + '\n', { mode: 0o600 })
        writeFileSync(configPath, JSON.stringify(config, null, 2) + '\n', { mode: 0o600 })
        return { ok: true, exported: (data.accounts || []).length, path: tokensPath }
      } catch (e: any) {
        return { ok: false, error: e?.message || 'Export failed' }
      }
    })
  }

  private static handleInstallCli() {
    ipcMain.handle('InstallCli', async () => {
      return this.installCli(false)
    })
  }

  private static async installCli(silent: boolean) {
      try {
        const cliSrcDir = is.dev()
          ? path.join(app.getAppPath(), 'scripts')
          : path.join(process.resourcesPath, 'cli')
        const nodeExe = (() => {
          const p = process.execPath.toLowerCase()
          if (p.endsWith('electron') || p.endsWith('electron.exe') || p.includes('electron.app')) return null
          return process.execPath
        })()

        if (is.macOS() || is.linux()) {
          const homeDir = os.homedir()
          const installDir = path.join(homeDir, '.local', 'bin')
          mkdirSync(installDir, { recursive: true })

          for (const name of ['clouddrive-cli', 'clouddrive-mcp']) {
            const scriptFile = path.join(cliSrcDir, `${name}.mjs`)
            if (!existsSync(scriptFile)) {
              return { ok: false, error: `CLI script not found: ${scriptFile}` }
            }
            const linkPath = path.join(installDir, name)
            const wrapper = `#!/bin/sh\nexec node ${shellQuote(scriptFile)} "$@"\n`
            writeFileSync(linkPath, wrapper, { mode: 0o755 })
            try { chmodSync(linkPath, 0o755) } catch { /* ignore */ }
          }

          // 将 ~/.local/bin 写入 shell 配置（如尚未添加）
          const pathLine = `\n# BoxPlayer CLI\nexport PATH="$HOME/.local/bin:$PATH"\n`
          const profiles = ['.zshrc', '.bashrc', '.bash_profile'].map((f) => path.join(homeDir, f))
          let updatedProfile = ''
          for (const p of profiles) {
            try {
              if (existsSync(p)) {
                const content = readFileSync(p, 'utf8')
                if (!content.includes('.local/bin')) {
                  writeFileSync(p, content + pathLine)
                  updatedProfile = path.basename(p)
                  break
                } else {
                  updatedProfile = path.basename(p)
                  break
                }
              }
            } catch { /* ignore */ }
          }

          const hint = updatedProfile
            ? `重启终端（或运行 source ~/.${updatedProfile}）后执行：`
            : `请确保 ~/.local/bin 已加入 PATH，然后执行：`

          return {
            ok: true,
            auto: silent,
            message: `已安装到 ${installDir}\n${hint} clouddrive-cli auth list`,
            paths: [
              path.join(installDir, 'clouddrive-cli'),
              path.join(installDir, 'clouddrive-mcp'),
            ],
          }
        }

        if (is.windows()) {
          const installDir = path.join(os.homedir(), 'AppData', 'Local', 'BoxPlayer', 'bin')
          mkdirSync(installDir, { recursive: true })

          for (const name of ['clouddrive-cli', 'clouddrive-mcp']) {
            const scriptFile = path.join(cliSrcDir, `${name}.mjs`)
            if (!existsSync(scriptFile)) {
              return { ok: false, error: `CLI script not found: ${scriptFile}` }
            }
            const nodeCmd = nodeExe ? nodeExe : 'node'
            const batContent = `@echo off\n"${nodeCmd}" "${scriptFile}" %*\n`
            writeFileSync(path.join(installDir, `${name}.cmd`), batContent)
          }

          const pathKey = 'HKCU\\Environment'
          const currentPath = process.env.PATH || ''
          if (!currentPath.includes(installDir)) {
            const { execSync } = await import('child_process')
            try {
              execSync(`setx PATH "${currentPath};${installDir}"`)
            } catch {
            }
          }

          return {
            ok: true,
            auto: silent,
            message: `已安装到 ${installDir}\n重启终端后运行: clouddrive-cli auth list`,
            paths: [
              path.join(installDir, 'clouddrive-cli.cmd'),
              path.join(installDir, 'clouddrive-mcp.cmd'),
            ],
            note: '如命令不可用，请手动将该目录加入系统 PATH',
          }
        }

        return { ok: false, error: 'Unsupported platform' }
      } catch (e: any) {
        if (e?.code === 'EACCES' || e?.code === 'EPERM') {
          return {
            ok: false,
            error: '权限不足，无法写入命令行安装目录。\n请在设置页手动安装，或确认 ~/.local/bin / %LOCALAPPDATA% 可写。',
            needsElevation: true,
          }
        }
        return { ok: false, error: e?.message || 'Install failed' }
      }
  }

  private static powerSaveBlockerId: number | null = null

  private static handlePowerSaveBlocker() {
    ipcMain.on('setPowerSaveBlocker', (_event, enabled: boolean) => {
      if (enabled) {
        if (ipcEvent.powerSaveBlockerId === null) {
          ipcEvent.powerSaveBlockerId = powerSaveBlocker.start('prevent-display-sleep')
        }
      } else {
        if (ipcEvent.powerSaveBlockerId !== null) {
          powerSaveBlocker.stop(ipcEvent.powerSaveBlockerId)
          ipcEvent.powerSaveBlockerId = null
        }
      }
    })
  }

  // ---------------------------------------------------------------------------
  // Reedy IPC handlers
  // ---------------------------------------------------------------------------

  private static handleReedy() {
    // Request-response via ipcMain.handle
    ipcMain.handle('reedy:get-meta', (_event, bookHash: string) => {
      return getBookMeta(bookHash)
    })

    ipcMain.handle('reedy:is-indexed', (_event, bookHash: string) => {
      return isBookIndexed(bookHash)
    })

    ipcMain.handle('reedy:clear-book', (_event, bookHash: string) => {
      clearBookData(bookHash)
      return { ok: true }
    })

    ipcMain.handle('reedy:list-skills', () => {
      return listSkills()
    })

    ipcMain.handle('reedy:toggle-skill', (_event, skillId: string, enabled: boolean) => {
      setSkillEnabled(skillId, enabled)
      return { ok: true }
    })

    ipcMain.handle('reedy:write-memory', (_event, args: {
      scope: string; scope_key: string; key: string; summary: string
      source_message_id?: string; embedding?: number[]
    }) => {
      return writeMemory(args as any)
    })

    ipcMain.handle('reedy:search-memories', (_event, scope: string, scopeKey: string, queryEmbedding: number[] | null, topK: number, recencyWeight?: number) => {
      return searchMemories(scope as any, scopeKey, queryEmbedding, topK, recencyWeight)
    })

    ipcMain.handle('reedy:list-memories', (_event, scope: string, scopeKey: string, limit: number) => {
      return listMemories(scope as any, scopeKey, limit)
    })

    ipcMain.handle('reedy:delete-memory', (_event, id: string) => {
      return deleteMemory(id)
    })

    ipcMain.handle('reedy:export-metrics', (_event, since?: number) => {
      return getMetrics(since)
    })

    ipcMain.handle('reedy:wipe-all', () => {
      wipeAllData()
      return { ok: true }
    })

    ipcMain.handle('reedy:record-metric', (_event, evt: any) => {
      recordMetric(evt)
      return { ok: true }
    })

    // Indexing: store chunks + embeddings + meta
    ipcMain.handle('reedy:store-chunks', (_event, chunks: any[]) => {
      storeChunks(chunks)
      return { ok: true }
    })

    ipcMain.handle('reedy:store-embeddings', (_event, rows: any[]) => {
      storeEmbeddings(rows)
      return { ok: true }
    })

    ipcMain.handle('reedy:store-meta', (_event, meta: any) => {
      storeMeta(meta)
      return { ok: true }
    })

    // Search: hybrid search
    ipcMain.handle('reedy:search', (_event, bookHash: string, queryEmbedding: number[], queryText: string, topK: number, spoilerBound?: number) => {
      return hybridSearch(bookHash, queryEmbedding, queryText, topK, spoilerBound)
    })

    // Cleanup on app quit
    ipcMain.handle('reedy:destroy', () => {
      destroyDb()
      return { ok: true }
    })

    ipcMain.handle('payment:start-server', async () => {
      const { startPaymentServer } = await import('./oauthServer')
      const win = BrowserWindow.getAllWindows()[0]
      return startPaymentServer(win)
    })
    ipcMain.handle('payment:stop-server', async () => {
      const { stopPaymentServer } = await import('./oauthServer')
      stopPaymentServer()
    })

  }
}
