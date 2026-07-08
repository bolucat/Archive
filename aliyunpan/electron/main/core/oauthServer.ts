import { createServer } from 'http'
import type { BrowserWindow } from 'electron'

let oauthServer: ReturnType<typeof createServer> | null = null
let paymentServer: ReturnType<typeof createServer> | null = null

export function startOAuthServer(win: BrowserWindow): Promise<number> {
  return new Promise((resolve) => {
    stopOAuthServer()
    oauthServer = createServer((req, res) => {
      const url = req.url || ''
      if (url.startsWith('/callback')) {
        res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' })
        res.end(`<!DOCTYPE html><html><head><meta charset="utf-8"></head><body>
          <script>
            if (location.hash) {
              var p = new URLSearchParams(location.hash.slice(1));
              fetch('http://localhost:3003/token?access_token=' + encodeURIComponent(p.get('access_token') || '') + '&refresh_token=' + encodeURIComponent(p.get('refresh_token') || ''));
            } else {
              var q = new URLSearchParams(location.search);
              fetch('http://localhost:3003/token?access_token=' + encodeURIComponent(q.get('access_token') || '') + '&refresh_token=' + encodeURIComponent(q.get('refresh_token') || ''));
            }
          </script>
          登录成功，正在返回 App...
        </body></html>`)
      } else if (url.startsWith('/token')) {
        const params = new URLSearchParams(url.split('?')[1] || '')
        win.webContents.send('auth-callback', { access_token: params.get('access_token') || '', refresh_token: params.get('refresh_token') || '' })
        res.writeHead(200).end('ok')
      }
    })
    oauthServer.listen(3003, '127.0.0.1', () => resolve(3003))
  })
}

export function startPaymentServer(win: BrowserWindow): Promise<number> {
  return new Promise((resolve) => {
    stopPaymentServer()
    paymentServer = createServer((req, res) => {
      const url = req.url || ''
      if (url.startsWith('/payment-success') || url.startsWith('/payment-cancelled') || url.startsWith('/payment-canceled') || url.startsWith('/payment-failed') || url.startsWith('/payment-failure')) {
        const params = new URLSearchParams((req.url || '').split('?')[1] || '')
        const status = url.startsWith('/payment-cancelled') || url.startsWith('/payment-canceled')
          ? 'cancelled'
          : url.startsWith('/payment-failed') || url.startsWith('/payment-failure')
            ? 'failed'
            : 'success'
        win.webContents.send('payment-callback', { status, checkout_id: params.get('checkout_id') || '', reason: params.get('reason') || '' })
        res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' })
        const title = status === 'success' ? '支付完成 ✓' : status === 'cancelled' ? '已取消购买' : '支付未完成'
        res.end(`<!DOCTYPE html><html><head><meta charset="utf-8"></head><body style="text-align:center;padding-top:40px;font-family:sans-serif"><h2>${title}</h2><p>正在返回 App…</p></body></html>`)
      }
    })
    paymentServer.listen(3004, '127.0.0.1', () => resolve(3004))
  })
}

export function stopOAuthServer() {
  if (oauthServer) { oauthServer.close(); oauthServer = null }
}

export function stopPaymentServer() {
  if (paymentServer) { paymentServer.close(); paymentServer = null }
}
