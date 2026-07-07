import { powerSaveBlocker } from 'electron'
import logger from './Logger'

export default class EnergyManager {
  private psbId: number | null = null

  startPowerSaveBlocker (): void {
    if (this.psbId !== null && powerSaveBlocker.isStarted(this.psbId)) return
    this.psbId = powerSaveBlocker.start('prevent-app-suspension')
    logger.info('[motrix] EnergyManager started id=' + this.psbId)
  }

  stopPowerSaveBlocker (): void {
    if (this.psbId === null) return
    if (powerSaveBlocker.isStarted(this.psbId)) powerSaveBlocker.stop(this.psbId)
    this.psbId = null
  }
}
