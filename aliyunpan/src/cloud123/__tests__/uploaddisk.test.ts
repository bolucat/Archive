import nodehttp from 'http'
import nodehttps from 'https'
import { describe, expect, it } from 'vitest'
import { getCloud123SliceRequest } from '../http'

describe('Cloud123UploadDisk', () => {
  it('uses the request client matching the upload server protocol', () => {
    expect(getCloud123SliceRequest('http:')).toBe(nodehttp.request)
    expect(getCloud123SliceRequest('https:')).toBe(nodehttps.request)
  })
})
