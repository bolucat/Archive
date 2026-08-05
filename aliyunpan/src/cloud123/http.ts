import nodehttp from 'http'
import nodehttps from 'https'

export const getCloud123SliceRequest = (protocol: string) => (protocol === 'http:' ? nodehttp.request : nodehttps.request)
