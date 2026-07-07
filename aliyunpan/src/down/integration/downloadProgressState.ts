export interface AriaProgressErrorInput {
  status: string
  errorCode?: string
  errorMessage?: string
}

export interface DownloadErrorState {
  isFailed: boolean
  failedCode: number
  failedMessage: string
}

export const resolveAriaProgressErrorState = (
  input: AriaProgressErrorInput,
  formatError: (errorCode: string, errorMessage: string) => string
): DownloadErrorState => {
  if (input.status !== 'error') {
    return {
      isFailed: false,
      failedCode: 0,
      failedMessage: ''
    }
  }

  if (input.errorCode && input.errorCode !== '0') {
    return {
      isFailed: true,
      failedCode: parseInt(input.errorCode) || 0,
      failedMessage: formatError(input.errorCode, input.errorMessage || '')
    }
  }

  return {
    isFailed: true,
    failedCode: 0,
    failedMessage: '下载失败'
  }
}
