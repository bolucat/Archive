package com.v2ray.ang.util

import android.content.Context
import android.os.SystemClock
import android.util.Log
import com.v2ray.ang.AppConfig.ANG_PACKAGE
import com.v2ray.ang.dto.EConfigType
import com.v2ray.ang.dto.ProfileItem
import com.v2ray.ang.fmt.Hysteria2Fmt
import com.v2ray.ang.handler.SpeedtestManager
import com.v2ray.ang.service.ProcessService
import java.io.File

object PluginUtil {
    private const val HYSTERIA2 = "libhysteria2.so"
    private const val TAG = ANG_PACKAGE
    private val procService: ProcessService by lazy {
        ProcessService()
    }

    /**
     * Run the plugin based on the provided configuration.
     *
     * @param context The context to use.
     * @param config The profile configuration.
     * @param domainPort The domain and port information.
     */
    fun runPlugin(context: Context, config: ProfileItem?, domainPort: String?) {
        Log.d(TAG, "runPlugin")

        if (config?.configType?.equals(EConfigType.HYSTERIA2) == true) {
            val configFile = genConfigHy2(context, config, domainPort) ?: return
            val cmd = genCmdHy2(context, configFile)

            procService.runProcess(context, cmd)
        }
    }

    /**
     * Stop the running plugin.
     */
    fun stopPlugin() {
        stopHy2()
    }

    /**
     * Perform a real ping using Hysteria2.
     *
     * @param context The context to use.
     * @param config The profile configuration.
     * @return The ping delay in milliseconds, or -1 if it fails.
     */
    fun realPingHy2(context: Context, config: ProfileItem?): Long {
        Log.d(TAG, "realPingHy2")
        val retFailure = -1L

        if (config?.configType?.equals(EConfigType.HYSTERIA2) == true) {
            val socksPort = Utils.findFreePort(listOf(0))
            val configFile = genConfigHy2(context, config, "0:${socksPort}") ?: return retFailure
            val cmd = genCmdHy2(context, configFile)

            val proc = ProcessService()
            proc.runProcess(context, cmd)
            Thread.sleep(1000L)
            val delay = SpeedtestManager.testConnection(context, socksPort)
            proc.stopProcess()

            return delay.first
        }
        return retFailure
    }

    /**
     * Generate the configuration file for Hysteria2.
     *
     * @param context The context to use.
     * @param config The profile configuration.
     * @param domainPort The domain and port information.
     * @return The generated configuration file.
     */
    private fun genConfigHy2(context: Context, config: ProfileItem, domainPort: String?): File? {
        Log.d(TAG, "runPlugin $HYSTERIA2")

        val socksPort = domainPort?.split(":")?.last()
            .let { if (it.isNullOrEmpty()) return null else it.toInt() }
        val hy2Config = Hysteria2Fmt.toNativeConfig(config, socksPort) ?: return null

        val configFile = File(context.noBackupFilesDir, "hy2_${SystemClock.elapsedRealtime()}.json")
        Log.d(TAG, "runPlugin ${configFile.absolutePath}")

        configFile.parentFile?.mkdirs()
        configFile.writeText(JsonUtil.toJson(hy2Config))
        Log.d(TAG, JsonUtil.toJson(hy2Config))

        return configFile
    }

    /**
     * Generate the command to run Hysteria2.
     *
     * @param context The context to use.
     * @param configFile The configuration file.
     * @return The command to run Hysteria2.
     */
    private fun genCmdHy2(context: Context, configFile: File): MutableList<String> {
        return mutableListOf(
            File(context.applicationInfo.nativeLibraryDir, HYSTERIA2).absolutePath,
            "--disable-update-check",
            "--config",
            configFile.absolutePath,
            "--log-level",
            "warn",
            "client"
        )
    }

    /**
     * Stop the Hysteria2 process.
     */
    private fun stopHy2() {
        try {
            Log.d(TAG, "$HYSTERIA2 destroy")
            procService?.stopProcess()
        } catch (e: Exception) {
            Log.d(TAG, e.toString())
        }
    }
}
