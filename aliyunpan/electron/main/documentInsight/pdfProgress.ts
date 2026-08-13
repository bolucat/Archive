export interface PdfProgressSender {
  isDestroyed(): boolean
  send(channel: string, requestId: string, progress: unknown): void
}

export function sendPdfProgress(sender: PdfProgressSender, requestId: string, progress: unknown): void {
  if (sender.isDestroyed()) return
  try {
    sender.send('documentReading:pdfProgress', requestId, progress)
  } catch (error) {
    if (sender.isDestroyed() || error instanceof Error && error.message.includes('Object has been destroyed')) return
    throw error
  }
}
