import { spawn } from 'node:child_process'
import { existsSync, rmSync, writeFileSync } from 'node:fs'
import http from 'node:http'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import JSZip from 'jszip'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const rootDir = path.resolve(__dirname, '..')
const smokeHtmlPath = path.join(rootDir, 'koodo-smoke.html')
const modalSmokeHtmlPath = path.join(rootDir, 'koodo-modal-smoke.html')
const smokeBookPath = path.join(rootDir, 'koodo-smoke-book-content.html')
const smokeEpubPath = path.join(rootDir, 'koodo-smoke-book.epub')
const smokeDriverPath = path.join(rootDir, '.koodo-smoke-electron.cjs')
const viteUrl = process.env.KOODO_SMOKE_VITE_URL || 'http://127.0.0.1:5173'
const modalBookUrl = process.env.KOODO_SMOKE_MODAL_BOOK_URL || '/koodo-smoke-book.epub'
const modalBookExt = process.env.KOODO_SMOKE_MODAL_BOOK_EXT || path.extname(modalBookUrl).replace('.', '') || 'epub'
const modalBookTitle = process.env.KOODO_SMOKE_MODAL_BOOK_TITLE || (modalBookUrl.split('/').pop() || 'Koodo Scroll Smoke Book')
const modalExpectedText = process.env.KOODO_SMOKE_MODAL_EXPECTED_TEXT || modalBookTitle
const modalLayoutMode = process.env.KOODO_SMOKE_MODAL_LAYOUT || 'scroll'
const modalFullTranslationMode = process.env.KOODO_SMOKE_MODAL_TRANSLATION || 'no'
const modalTranslationStub = process.env.KOODO_SMOKE_TRANSLATION_STUB === '1'
const modalRightPanelCheck = process.env.KOODO_SMOKE_MODAL_RIGHT_PANEL === '1'
const directSmokeUrl = `${viteUrl}/koodo-smoke.html`
const modalSmokeUrl = `${viteUrl}/koodo-modal-smoke.html`
const childTimeoutMs = Number(process.env.KOODO_SMOKE_CHILD_TIMEOUT_MS || 30000)

function log(message) {
  process.stderr.write(`[koodo-smoke] ${message}\n`)
}

function wait(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

function request(url) {
  return new Promise((resolve, reject) => {
    const req = http.get(url, (res) => {
      res.resume()
      res.on('end', () => resolve(res.statusCode || 0))
    })
    req.on('error', reject)
    req.setTimeout(2000, () => {
      req.destroy(new Error(`Timeout requesting ${url}`))
    })
  })
}

async function waitForServer(url, timeoutMs = 20000) {
  const started = Date.now()
  while (Date.now() - started < timeoutMs) {
    try {
      const status = await request(url)
      if (status >= 200 && status < 500) return
    } catch {
    }
    await wait(250)
  }
  throw new Error(`Vite server did not become ready at ${url}`)
}

function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: rootDir,
      stdio: ['ignore', 'pipe', 'pipe'],
      ...options,
      env: {
        ...process.env,
        ...options.env
      }
    })
    let stdout = ''
    let stderr = ''
    const timer = setTimeout(() => {
      child.kill('SIGTERM')
      reject(new Error(`${command} ${args.join(' ')} timed out after ${childTimeoutMs}ms\n${stdout}\n${stderr}`))
    }, childTimeoutMs)
    child.stdout.on('data', (chunk) => {
      stdout += chunk.toString()
      if (stdout.length > 20000) stdout = stdout.slice(-20000)
    })
    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString()
      if (stderr.length > 20000) stderr = stderr.slice(-20000)
    })
    child.on('error', reject)
    child.on('exit', (code, signal) => {
      clearTimeout(timer)
      if (code === 0) resolve({ stdout, stderr })
      else reject(new Error(`${command} ${args.join(' ')} failed with ${code ?? signal}\n${stdout}\n${stderr}`))
    })
  })
}

async function writeSmokeFiles() {
  const longChapter = Array.from({ length: 120 }, (_, index) => `<p>Scroll verification paragraph ${index + 1}: this EPUB line makes the modal reader taller than the scroll viewport so page buttons and keyboard navigation must move the page-area scrollTop.</p>`).join('\n')
  const zip = new JSZip()
  zip.file('mimetype', 'application/epub+zip', { compression: 'STORE' })
  zip.file('META-INF/container.xml', `<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>`)
  zip.file('OEBPS/content.opf', `<?xml version="1.0" encoding="UTF-8"?>
<package version="3.0" unique-identifier="bookid" xmlns="http://www.idpf.org/2007/opf">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">koodo-scroll-smoke</dc:identifier>
    <dc:title>Koodo Scroll Smoke Book</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
  </spine>
</package>`)
  zip.file('OEBPS/nav.xhtml', `<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
  <head><title>Navigation</title></head>
  <body><nav epub:type="toc"><ol><li><a href="chapter1.xhtml">Chapter 1</a></li></ol></nav></body>
</html>`)
  zip.file('OEBPS/chapter1.xhtml', `<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <head><title>Chapter 1</title></head>
  <body>
    <h1>Koodo Scroll Smoke Book</h1>
    ${longChapter}
    <p>The final paragraph contains the keyword nebula for search verification.</p>
  </body>
</html>`)
  writeFileSync(smokeEpubPath, await zip.generateAsync({ type: 'nodebuffer' }))

  writeFileSync(smokeBookPath, `<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <title>Koodo Smoke Book</title>
  </head>
  <body>
    <h1>Koodo Smoke Book</h1>
    <h2>Chapter 1</h2>
    <p>This local smoke-test book verifies that Koodo can render through the integrated reader wrapper.</p>
    ${Array.from({ length: 80 }, (_, index) => `<p>Scroll verification paragraph ${index + 1}: this line makes the modal reader taller than the scroll viewport so page buttons and keyboard navigation must move the page-area scrollTop.</p>`).join('\n')}
    <h2>Chapter 2</h2>
    <p>The second chapter contains the keyword nebula for search verification.</p>
    ${Array.from({ length: 20 }, (_, index) => `<p>Second chapter filler ${index + 1}: nebula remains searchable while the first chapter is long enough for scroll mode.</p>`).join('\n')}
  </body>
</html>
`, 'utf8')

  writeFileSync(smokeHtmlPath, `<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <title>Koodo Reader Smoke</title>
    <style>
      html, body { margin: 0; width: 100%; height: 100%; background: #f5f0e8; }
      #page-area { width: 960px; height: 680px; margin: 24px auto; background: #fff; }
      #status { position: fixed; left: 8px; top: 8px; font: 12px sans-serif; }
    </style>
  </head>
  <body>
    <div id="status">loading</div>
    <div id="page-area"></div>
    <script type="module">
      import { createBookReader } from '/src/utils/bookReader.ts'

      window.__koodoSmokeErrors = []
      window.addEventListener('error', (event) => {
        window.__koodoSmokeErrors.push(String(event.error?.message || event.message || 'error'))
      })
      window.addEventListener('unhandledrejection', (event) => {
        window.__koodoSmokeErrors.push(String(event.reason?.message || event.reason || 'rejection'))
      })

      async function run() {
        const container = document.getElementById('page-area')
        const status = document.getElementById('status')
        const reader = await createBookReader({
          sourceUrl: '/koodo-smoke-book-content.html',
          ext: 'html',
          container,
          readerMode: 'single',
          fontSize: 18
        })
        const started = Date.now()
        let visibleText = ''
        let bodyText = ''
        let iframeText = ''
        let combinedText = ''
        while (Date.now() - started < 8000) {
          visibleText = await reader.getVisibleText()
          bodyText = document.body.innerText || ''
          iframeText = Array.from(container.querySelectorAll('iframe'))
            .map((iframe) => iframe.contentDocument?.body?.innerText || '')
            .join('\\n')
          combinedText = [visibleText, bodyText, iframeText].join('\\n')
	          if (combinedText.includes('Koodo Smoke Book') || combinedText.includes('nebula')) break
          await new Promise((resolve) => setTimeout(resolve, 250))
        }
        const result = {
          ok: combinedText.includes('Koodo Smoke Book') || combinedText.includes('nebula'),
          childCount: container.children.length,
          iframeCount: container.querySelectorAll('iframe').length,
          iframeDetails: Array.from(container.querySelectorAll('iframe')).map((iframe) => ({
            src: iframe.getAttribute('src') || '',
            readyState: iframe.contentDocument?.readyState || '',
            bodyText: iframe.contentDocument?.body?.innerText?.slice(0, 200) || ''
          })),
          progressText: reader.getProgressText(),
          textSample: combinedText.slice(0, 500),
          containerHtml: container.innerHTML.slice(0, 1000),
          errors: window.__koodoSmokeErrors
        }
        status.textContent = result.ok ? 'ok' : 'failed'
        window.__koodoSmokeResult = result
        reader.destroy()
      }

      run().catch((error) => {
        window.__koodoSmokeResult = {
          ok: false,
          childCount: document.getElementById('page-area')?.children.length || 0,
          iframeCount: document.querySelectorAll('#page-area iframe').length,
          iframeDetails: Array.from(document.querySelectorAll('#page-area iframe')).map((iframe) => ({
            src: iframe.getAttribute('src') || '',
            readyState: iframe.contentDocument?.readyState || '',
            bodyText: iframe.contentDocument?.body?.innerText?.slice(0, 200) || ''
          })),
          progressText: '-',
          textSample: '',
          containerHtml: document.getElementById('page-area')?.innerHTML.slice(0, 1000) || '',
          errors: [...window.__koodoSmokeErrors, String(error?.stack || error?.message || error)]
        }
      })
    </script>
  </body>
</html>
`, 'utf8')

  writeFileSync(modalSmokeHtmlPath, `<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <title>Koodo Modal Smoke</title>
    <style>
      html, body, #app { margin: 0; width: 100%; height: 100%; background: #f5f0e8; }
      #status { position: fixed; left: 8px; top: 8px; z-index: 9999; font: 12px sans-serif; }
    </style>
  </head>
  <body>
	    <div id="status">loading</div>
	    <div id="app"></div>
	    <script type="module">
	      if (${JSON.stringify(modalTranslationStub)}) {
	        const originalFetch = window.fetch.bind(window)
	        window.fetch = async (input, init) => {
	          const url = String(input?.url || input)
	          if (url.includes('translate.googleapis.com/translate_a/single')) {
	            const source = new URL(url).searchParams.get('q') || ''
	            return new Response(JSON.stringify([[[\`译文 \${source.slice(0, 80)}\`, source, null, null]]]), { headers: { 'content-type': 'application/json' } })
	          }
	          return originalFetch(input, init)
	        }
	      }
	      window.__koodoSmokeErrors = []
	      window.addEventListener('error', (event) => {
	        window.__koodoSmokeErrors.push(String(event.error?.message || event.message || 'error'))
	      })
	      window.addEventListener('unhandledrejection', (event) => {
	        window.__koodoSmokeErrors.push(String(event.reason?.message || event.reason || 'rejection'))
	      })
	      localStorage.setItem('bookReader.preferences', JSON.stringify({ readerLayoutMode: ${JSON.stringify(modalLayoutMode)}, readerFullTranslationMode: ${JSON.stringify(modalFullTranslationMode)}, readerIsHidePageButton: false }))
	      localStorage.setItem('bookReader.translator', 'google')
	      if (${JSON.stringify(modalFullTranslationMode)} !== 'no') {
	        localStorage.setItem('app_user_authed', '1')
	        localStorage.setItem(\`usage_\${new Date().toISOString().slice(0, 10)}_readerTranslation\`, '-100000')
	      }

      const smokeBook = {
        id: 'koodo-modal-smoke-book',
        user_id: 'smoke-user',
        drive_id: 'smoke-drive',
        file_id: 'smoke-file',
        parent_file_id: '',
        name: ${JSON.stringify(modalBookTitle)},
        file_name: ${JSON.stringify(modalBookTitle)},
        title: ${JSON.stringify(modalBookTitle)},
        ext: ${JSON.stringify(modalBookExt)},
        category: 'book',
        description: '',
        size: 512,
        starred: false,
        updated_at: Date.now(),
        created_at: Date.now(),
        thumbnail: '',
        cover: '',
        reading_progress: 0,
        reading_position: undefined,
        last_read_at: 0,
        source: 'smoke'
      }

      async function run() {
        const [{ createApp, h, ref }, { createPinia }, arcoModule, modalModule] = await Promise.all([
          import('vue'),
          import('pinia'),
          import('@arco-design/web-vue'),
          import('/src/layout/BookReaderModal.vue'),
          import('@arco-themes/vue-gi-demo/css/arco.css')
        ]).then(([vue, piniaModule, arco, modal]) => [vue, piniaModule, arco, modal])
        const ArcoVue = arcoModule.default
        const pinia = createPinia()
        const BookReaderModal = modalModule.default
        const visible = ref(true)
        createApp({
          setup() {
            return () => h(BookReaderModal, {
              visible: visible.value,
              book: smokeBook,
              sourceUrlOverride: ${JSON.stringify(modalBookUrl)},
              'onUpdate:visible': (value) => {
                visible.value = value
              }
            })
          }
	        }).use(pinia).use(ArcoVue).mount('#app')
	
	        const status = document.getElementById('status')
	        const collectTranslationHosts = () =>
	          Array.from(document.querySelectorAll('#page-area iframe')).flatMap((iframe) =>
	            Array.from(iframe.contentDocument?.querySelectorAll('.kookit-translation-host') || []).map((node) => ({
	              text: node.textContent?.slice(0, 120) || '',
	              translation: node.getAttribute('data-kookit-translation') || '',
	              className: node.getAttribute('class') || '',
	              style: node.getAttribute('style') || ''
	            }))
	          )
	        const waitForTranslationHosts = async () => {
	          if (${JSON.stringify(modalFullTranslationMode)} === 'no') return collectTranslationHosts()
	          const translationStarted = Date.now()
	          let hosts = collectTranslationHosts()
	          while (Date.now() - translationStarted < 15000) {
	            hosts = collectTranslationHosts()
	            if (hosts.some((host) => host.translation.includes('译文'))) return hosts
	            await new Promise((resolve) => setTimeout(resolve, 300))
	          }
	          return hosts
	        }
	        const started = Date.now()
        let combinedText = ''
        let stage = null
        while (Date.now() - started < 10000) {
          stage = document.querySelector('#page-area')
          const iframeText = Array.from(document.querySelectorAll('#page-area iframe'))
            .map((iframe) => iframe.contentDocument?.body?.innerText || '')
            .join('\\n')
          combinedText = [document.body.innerText || '', iframeText].join('\\n')
	          if (combinedText.includes(${JSON.stringify(modalExpectedText)}) || combinedText.includes('nebula')) break
          await new Promise((resolve) => setTimeout(resolve, 250))
	        }
		        await new Promise((resolve) => setTimeout(resolve, 600))
		        stage = document.querySelector('#page-area')
		        const checkRightPanel = async () => {
		          if (!${JSON.stringify(modalRightPanelCheck)}) return { requested: false, ok: true }
		          const trigger = document.querySelector('.trigger-right')
		          trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, clientX: window.innerWidth - 2, clientY: Math.round(window.innerHeight / 2) }))
		          await new Promise((resolve) => setTimeout(resolve, 350))
		          const panel = document.querySelector('.panel-right.setting-panel')
		          const lang = document.querySelector('.panel-right.setting-panel .lang-toggle-btn')
		          const topControls = document.querySelector('.reader-topright-controls')
		          const rectOf = (el) => {
		            if (!el) return null
		            const rect = el.getBoundingClientRect()
		            return { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom, width: rect.width, height: rect.height }
		          }
		          const intersects = (a, b) => !!a && !!b && a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top
		          const panelRectBefore = panel?.getBoundingClientRect()
		          const widthBefore = panelRectBefore?.width || 0
		          const handle = document.querySelector('.panel-right.setting-panel .panel-resize-left')
		          handle?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true, clientX: Math.round((panelRectBefore?.left || 0) + 2), clientY: 120 }))
		          await new Promise((resolve) => setTimeout(resolve, 50))
		          const shield = document.querySelector('.panel-resize-shield')
		          const move = new MouseEvent('mousemove', { bubbles: true, cancelable: true, clientX: Math.round((panelRectBefore?.left || 0) - 140), clientY: 120 })
		          ;(shield || document).dispatchEvent(move)
		          const up = new MouseEvent('mouseup', { bubbles: true, cancelable: true, clientX: Math.round((panelRectBefore?.left || 0) - 140), clientY: 120 })
		          ;(shield || document).dispatchEvent(up)
		          await new Promise((resolve) => setTimeout(resolve, 80))
		          const panelRectAfter = panel?.getBoundingClientRect()
		          const widthAfter = panelRectAfter?.width || 0
		          const langRect = rectOf(lang)
		          const topControlsRect = rectOf(topControls)
		          const triggerRect = rectOf(trigger)
		          const triggerStyle = trigger ? getComputedStyle(trigger) : null
		          const triggerVisible = !!trigger && triggerStyle?.display !== 'none' && triggerStyle?.visibility !== 'hidden'
		          const topControlsOverlapLang = intersects(langRect, topControlsRect)
		          const triggerOverlapLang = triggerVisible && intersects(langRect, triggerRect)
		          const resized = widthAfter > widthBefore + 80
		          const ok = !!panel?.classList.contains('open') && !!lang && !!topControls && !topControlsOverlapLang && !triggerOverlapLang && !!shield && resized
		          return {
		            requested: true,
		            ok,
		            panelOpen: !!panel?.classList.contains('open'),
		            shieldSeen: !!shield,
		            resized,
		            widthBefore,
		            widthAfter,
		            topControlsOverlapLang,
		            triggerOverlapLang,
		            triggerVisible,
		            langRect,
		            topControlsRect,
		            triggerRect
		          }
		        }
		        const rightPanelCheck = await checkRightPanel()
		        const beforeTranslationHosts = await waitForTranslationHosts()
		        const beforeScrollTop = stage?.scrollTop ?? -1
        const scrollHeight = stage?.scrollHeight ?? 0
        const clientHeight = stage?.clientHeight ?? 0
        const iframeMetricsBefore = Array.from(document.querySelectorAll('#page-area iframe')).map((iframe) => ({
          inlineHeight: iframe.style.height,
          attrHeight: iframe.getAttribute('height') || '',
          offsetHeight: iframe.offsetHeight,
          clientHeight: iframe.clientHeight,
          scrollHeight: iframe.scrollHeight,
          docBodyScrollHeight: iframe.contentDocument?.body?.scrollHeight || 0,
          docElementScrollHeight: iframe.contentDocument?.documentElement?.scrollHeight || 0,
          docBodyTextLength: iframe.contentDocument?.body?.innerText?.length || 0
        }))
	        const beforeReaderText = Array.from(document.querySelectorAll('#page-area iframe')).map((iframe) => iframe.contentDocument?.body?.innerText || '').join('\\n')
	        const beforeBodyText = document.body.innerText || ''
	        const beforeFooterText = document.querySelector('.pw-footer')?.textContent || ''
	        const modalTranslationRequested = ${JSON.stringify(modalFullTranslationMode)} !== 'no'
		        document.querySelector('.page-turn-next')?.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, cancelable: true, pointerType: 'mouse' }))
		        await new Promise((resolve) => setTimeout(resolve, 1800))
		        const afterClickTranslationHosts = await waitForTranslationHosts()
		        const afterClickScrollTop = stage?.scrollTop ?? -1
	        const afterClickReaderText = Array.from(document.querySelectorAll('#page-area iframe')).map((iframe) => iframe.contentDocument?.body?.innerText || '').join('\\n')
	        const afterClickBodyText = document.body.innerText || ''
	        const afterClickFooterText = document.querySelector('.pw-footer')?.textContent || ''
	        let afterKeyboardTranslationHosts = afterClickTranslationHosts
	        let afterKeyboardScrollTop = afterClickScrollTop
	        let afterKeyboardReaderText = afterClickReaderText
	        let afterKeyboardBodyText = afterClickBodyText
	        let afterKeyboardFooterText = afterClickFooterText
	        let arrowDownBefore = stage?.scrollTop ?? -1
	        let arrowDownAfter = arrowDownBefore
	        if (!modalTranslationRequested) {
	          window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true, cancelable: true }))
	          await new Promise((resolve) => setTimeout(resolve, 1800))
	          afterKeyboardTranslationHosts = await waitForTranslationHosts()
	          afterKeyboardScrollTop = stage?.scrollTop ?? -1
	          afterKeyboardReaderText = Array.from(document.querySelectorAll('#page-area iframe')).map((iframe) => iframe.contentDocument?.body?.innerText || '').join('\\n')
	          afterKeyboardBodyText = document.body.innerText || ''
	          afterKeyboardFooterText = document.querySelector('.pw-footer')?.textContent || ''
	          arrowDownBefore = stage?.scrollTop ?? -1
	          window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }))
	          await new Promise((resolve) => setTimeout(resolve, 500))
	          arrowDownAfter = stage?.scrollTop ?? -1
	        }
	        const arrowDownDelta = arrowDownAfter - arrowDownBefore
		        const footerShowsPageNumber = /Page\\s+\\d+/.test([beforeFooterText, afterClickFooterText, afterKeyboardFooterText].join(' '))
		        const isScrollLayout = ${JSON.stringify(modalLayoutMode)} === 'scroll'
			        const translationHosts = [...beforeTranslationHosts, ...afterClickTranslationHosts, ...afterKeyboardTranslationHosts]
	        const translationOk =
	          ${JSON.stringify(modalFullTranslationMode)} === 'no' ||
	          (translationHosts.some((host) => host.translation.includes('译文')) &&
	            (${JSON.stringify(modalFullTranslationMode)} !== 'target' || translationHosts.some((host) => /font-size:\\s*0px/i.test(host.style))))
		        const scrollNavigationWorked = scrollHeight > clientHeight && (afterClickScrollTop > beforeScrollTop || afterClickReaderText !== beforeReaderText || afterClickBodyText !== beforeBodyText) && (afterKeyboardScrollTop > afterClickScrollTop || afterKeyboardReaderText !== afterClickReaderText || afterKeyboardBodyText !== afterClickBodyText)
		        const paginatedNavigationWorked = scrollHeight <= clientHeight + 2 && afterClickFooterText !== beforeFooterText && afterKeyboardFooterText !== afterClickFooterText
		        const navigationWorked = isScrollLayout ? scrollNavigationWorked : paginatedNavigationWorked
		        const result = {
		          ok: combinedText.includes(${JSON.stringify(modalExpectedText)}) && footerShowsPageNumber && translationOk && (modalTranslationRequested || navigationWorked) && rightPanelCheck.ok,
	          hasModal: !!document.querySelector('.reader-modal'),
	          hasStage: !!stage,
	          layoutMode: ${JSON.stringify(modalLayoutMode)},
	          fullTranslationMode: ${JSON.stringify(modalFullTranslationMode)},
	          navigationWorked,
		          translationOk,
		          rightPanelCheck,
	          translationHosts: translationHosts.slice(0, 5),
          scrollNavigationWorked,
          paginatedNavigationWorked,
          footerShowsPageNumber,
          beforeFooterText,
          afterClickFooterText,
          afterKeyboardFooterText,
          beforeScrollTop,
          afterClickScrollTop,
          afterKeyboardScrollTop,
          arrowDownBefore,
          arrowDownAfter,
          arrowDownDelta,
          scrollHeight,
          clientHeight,
          iframeMetricsBefore,
          beforeReaderText: beforeReaderText.slice(0, 240),
          afterClickReaderText: afterClickReaderText.slice(0, 240),
          afterKeyboardReaderText: afterKeyboardReaderText.slice(0, 240),
          afterClickBodySample: afterClickBodyText.slice(0, 500),
          afterKeyboardBodySample: afterKeyboardBodyText.slice(0, 500),
          loadingTextPresent: (document.body.innerText || '').includes('加载中'),
          iframeCount: document.querySelectorAll('#page-area iframe').length,
          iframeDetails: Array.from(document.querySelectorAll('#page-area iframe')).map((iframe) => ({
            src: iframe.getAttribute('src') || '',
            readyState: iframe.contentDocument?.readyState || '',
            bodyText: iframe.contentDocument?.body?.innerText?.slice(0, 200) || ''
          })),
          textSample: combinedText.slice(0, 500),
          bodySample: (document.body.innerText || '').slice(0, 500),
          errors: window.__koodoSmokeErrors
        }
        status.textContent = result.ok ? 'ok' : 'failed'
        window.__koodoSmokeResult = result
      }

      run().catch((error) => {
        window.__koodoSmokeResult = {
          ok: false,
          hasModal: !!document.querySelector('.reader-modal'),
          hasStage: !!document.querySelector('#page-area'),
          loadingTextPresent: (document.body.innerText || '').includes('加载中'),
          iframeCount: document.querySelectorAll('#page-area iframe').length,
          iframeDetails: Array.from(document.querySelectorAll('#page-area iframe')).map((iframe) => ({
            src: iframe.getAttribute('src') || '',
            readyState: iframe.contentDocument?.readyState || '',
            bodyText: iframe.contentDocument?.body?.innerText?.slice(0, 200) || ''
          })),
          textSample: '',
          bodySample: (document.body.innerText || '').slice(0, 500),
          errors: [...window.__koodoSmokeErrors, String(error?.stack || error?.message || error)]
        }
      })
    </script>
  </body>
</html>
`, 'utf8')

  writeFileSync(smokeDriverPath, `
const { app, BrowserWindow } = require('electron')

const url = process.env.KOODO_SMOKE_URL
const timeoutMs = Number(process.env.KOODO_SMOKE_TIMEOUT_MS || 20000)
const screenshotPath = process.env.KOODO_SMOKE_SCREENSHOT_PATH || ''

app.commandLine.appendSwitch('no-sandbox')
app.commandLine.appendSwitch('disable-web-security')

async function wait(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

async function main() {
  await app.whenReady()
  const win = new BrowserWindow({
    show: true,
    width: 1280,
    height: 900,
    webPreferences: {
      sandbox: false,
      webSecurity: false,
      nodeIntegration: true,
      nodeIntegrationInWorker: true,
      allowRunningInsecureContent: true,
      contextIsolation: false,
      backgroundThrottling: false
    }
  })
  const consoleMessages = []
  win.webContents.on('console-message', (_event, level, message, line, sourceId) => {
    consoleMessages.push({ level, message: String(message).slice(0, 500), line, sourceId })
    if (consoleMessages.length > 50) consoleMessages.shift()
  })
  win.webContents.on('did-fail-load', (_event, errorCode, errorDescription, validatedURL) => {
    consoleMessages.push({ level: -1, message: errorDescription, line: errorCode, sourceId: validatedURL })
  })
  await win.loadURL(url)
  const started = Date.now()
  while (Date.now() - started < timeoutMs) {
    const result = await win.webContents.executeJavaScript('window.__koodoSmokeResult || null', true)
    if (result) {
      if (screenshotPath) {
        const image = await win.webContents.capturePage()
        require('fs').writeFileSync(screenshotPath, image.toPNG())
        result.screenshotPath = screenshotPath
      }
      result.consoleMessages = consoleMessages
      console.log(JSON.stringify(result, null, 2))
      app.exit(result.ok ? 0 : 1)
      return
    }
    await wait(250)
  }
  const snapshot = await win.webContents.executeJavaScript('({ title: document.title, text: document.body.innerText, html: document.body.innerHTML.slice(0, 2000), errors: window.__koodoSmokeErrors || [] })', true)
  console.error(JSON.stringify({ ok: false, timeout: true, snapshot, consoleMessages }, null, 2))
  app.exit(1)
}

main().catch((error) => {
  console.error(error)
  app.exit(1)
})
`, 'utf8')
}

function cleanup(viteProcess) {
  if (viteProcess && !viteProcess.killed) viteProcess.kill('SIGTERM')
  for (const file of [smokeHtmlPath, modalSmokeHtmlPath, smokeBookPath, smokeEpubPath, smokeDriverPath]) {
    if (existsSync(file)) rmSync(file, { force: true })
  }
}

async function main() {
  log('writing temporary smoke page')
  await writeSmokeFiles()
  log('starting Vite dev server without plugin-started Electron')
  const viteProcess = spawn('pnpm', ['exec', 'vite', '--host', '127.0.0.1'], {
    cwd: rootDir,
    stdio: ['ignore', 'pipe', 'pipe'],
    env: {
      ...process.env,
      VSCODE_DEBUG: '1',
      VITE_DEV_SERVER_URL: viteUrl
    }
  })
  let viteStdout = ''
  let viteStderr = ''
  viteProcess.stdout.on('data', (chunk) => {
    viteStdout += chunk.toString()
    if (viteStdout.length > 12000) viteStdout = viteStdout.slice(-12000)
  })
  viteProcess.stderr.on('data', (chunk) => {
    viteStderr += chunk.toString()
    if (viteStderr.length > 12000) viteStderr = viteStderr.slice(-12000)
  })
  viteProcess.on('exit', (code, signal) => {
    log(`Vite exited with ${code ?? signal}`)
  })

  try {
    log(`waiting for ${viteUrl}`)
    try {
      await waitForServer(viteUrl)
    } catch (error) {
      process.stderr.write(`\n[koodo-smoke] Vite stdout:\n${viteStdout}\n`)
      process.stderr.write(`\n[koodo-smoke] Vite stderr:\n${viteStderr}\n`)
      throw error
    }
    log(`opening ${directSmokeUrl} in Electron`)
    const directResult = await run('pnpm', ['exec', 'electron', smokeDriverPath], {
      env: {
        KOODO_SMOKE_URL: directSmokeUrl
      }
    })
    log('direct Electron smoke finished')
    process.stdout.write(directResult.stdout)
    process.stderr.write(directResult.stderr)

    log(`opening ${modalSmokeUrl} in Electron`)
    const modalResult = await run('pnpm', ['exec', 'electron', smokeDriverPath], {
      env: {
        KOODO_SMOKE_URL: modalSmokeUrl
      }
    })
    log('modal Electron smoke finished')
    process.stdout.write(modalResult.stdout)
    process.stderr.write(modalResult.stderr)
  } finally {
    log('cleaning up')
    cleanup(viteProcess)
  }
}

main().catch((error) => {
  cleanup()
  console.error(error)
  process.exit(1)
})
