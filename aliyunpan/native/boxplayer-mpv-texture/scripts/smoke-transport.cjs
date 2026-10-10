// Native transport regression only; no Electron process or user profile.
const fs = require('node:fs')
const path = require('node:path')
const os = require('node:os')
const { spawnSync } = require('node:child_process')
const assert = require('node:assert/strict')
const addonPath = path.resolve(process.argv[2] || path.join(__dirname, '../build/Release/mpv_transport.node'))
const transport = require(addonPath)
const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'mpv-fd-'))
const socket = path.join(directory, 'frames')
const file = path.join(directory, 'payload')
fs.writeFileSync(file, 'shared resource')
let deadline
const receiver = new transport.Receiver(socket, (metadata, fds) => {
  try {
    assert.equal(metadata, 'frame')
    assert.equal(fds.length, 1)
    const bytes = Buffer.alloc(15)
    assert.equal(fs.readSync(fds[0], bytes, 0, 15, 0), 15)
    assert.equal(bytes.toString(), 'shared resource')
    console.log('SCM_RIGHTS cross-process receive/read/close: OK')
  } finally {
    transport.closeDescriptors(fds)
    receiver.stop()
    fs.rmSync(directory, { recursive: true, force: true })
    clearTimeout(deadline)
  }
})
const sender = spawnSync(process.execPath, ['-e', `
  const fs = require('node:fs')
  const transport = require(process.argv[1])
  const fd = fs.openSync(process.argv[3], 'r')
  const sent = transport.send(process.argv[2], 'frame', [fd])
  fs.closeSync(fd)
  process.exit(sent ? 0 : 1)
`, addonPath, socket, file], { timeout: 3000 })
assert.equal(sender.status, 0, sender.stderr?.toString())
deadline = setTimeout(() => {
  receiver.stop()
  fs.rmSync(directory, { recursive: true, force: true })
  throw new Error('FD transport timed out')
}, 3000)
