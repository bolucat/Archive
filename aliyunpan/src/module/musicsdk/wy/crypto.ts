const { createCipheriv, publicEncrypt, randomBytes, createHash, constants } = require('crypto')

const iv = Buffer.from('0102030405060708')
const presetKey = Buffer.from('0CoJUm6Qyw8W8jud')
const base62 = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789'
const publicKey = '-----BEGIN PUBLIC KEY-----\nMIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDgtQn2JZ34ZC28NWYpAUd98iZ37BUrX/aKzmFbt7clFSs6sXqHauqKWqdtLkF2KexO40H1YTX8z2lSgBBOAxLsvaklV8k4cBFK9snQXE9/DDaFt6Rr7iVZMldczhC0JNgTz+SHXT6CBHuX3e9SdB1Ua44oncaTWz7OBGLbCiK45wIDAQAB\n-----END PUBLIC KEY-----'
const eapiKey = ''

function aesEncrypt(buffer: Buffer, mode: string, key: Buffer, iv: Buffer | string): Buffer {
  const cipher = createCipheriv(mode, key, iv)
  return Buffer.concat([cipher.update(buffer), cipher.final()])
}

function rsaEncrypt(buffer: Buffer, key: string): Buffer {
  buffer = Buffer.concat([Buffer.alloc(128 - buffer.length), buffer])
  return publicEncrypt({ key, padding: constants.RSA_NO_PADDING }, buffer)
}

export function weapi(object: Record<string, any>) {
  const text = JSON.stringify(object)
  const secretKey = Buffer.from(randomBytes(16).map((n: number) => base62.charAt(n % 62).charCodeAt(0)))
  return {
    params: aesEncrypt(
      Buffer.from(aesEncrypt(Buffer.from(text), 'aes-128-cbc', presetKey, iv).toString('base64')),
      'aes-128-cbc',
      secretKey,
      iv
    ).toString('base64'),
    encSecKey: rsaEncrypt(Buffer.from(secretKey.reverse()), publicKey).toString('hex'),
  }
}

export function eapi(url: string, object: Record<string, any>) {
  const text = JSON.stringify(object)
  const message = `nobody${url}use${text}md5forencrypt`
  const digest = createHash('md5').update(message).digest('hex')
  const data = `${url}-36cd479b6b5-${text}-36cd479b6b5-${digest}`
  return {
    params: aesEncrypt(Buffer.from(data), 'aes-128-ecb', Buffer.from(eapiKey), '').toString(
      'hex'
    ).toUpperCase(),
  }
}
