package com.v2ray.ang.core

import android.content.Context
import com.v2ray.ang.AppConfig
import com.v2ray.ang.dto.CoreConfigContext
import com.v2ray.ang.dto.entities.ProfileItem
import com.v2ray.ang.enums.CoreResolvedType
import com.v2ray.ang.enums.EConfigType
import com.v2ray.ang.extension.isNotNullEmpty
import com.v2ray.ang.handler.MmkvManager
import com.v2ray.ang.handler.SettingsManager
import com.v2ray.ang.util.LogUtil
import com.v2ray.ang.util.Utils

/**
 * Builds [com.v2ray.ang.dto.CoreConfigContext] from the selected profile.
 * Keeps parsing and resolution logic out of [CoreConfigManager].
 */
object CoreConfigContextBuilder {

    /** Loads profile by guid and returns a resolved runtime context. */
    fun build(context: Context, guid: String): CoreConfigContext? {
        val config = MmkvManager.decodeServerConfig(guid) ?: return null
        if (config.configType == EConfigType.CUSTOM) {
            return CoreConfigContext(
                context = context,
                guid = guid,
                selectedProfile = config,
                resolvedProfiles = listOf(config),
                resolvedType = CoreResolvedType.CUSTOM,
            )
        }

        // Pre-resolve custom outbound profiles from routing rulesets
        val customOutbounds = resolveCustomOutbounds()

        // Determine resolved profiles and type based on config type
        val (resolvedProfiles, resolvedType) = when (config.configType) {
            EConfigType.POLICYGROUP -> {
                val profiles = resolvePolicyGroupProfiles(config)
                Pair(profiles, CoreResolvedType.POLICYGROUP)
            }

            EConfigType.PROXYCHAIN -> {
                val profiles = resolveProxyChainProfiles(config)
                Pair(profiles, CoreResolvedType.PROXYCHAIN)
            }

            else -> {
                val chainProfiles = resolveProxyChainProfilesFromGroup(config)
                val type = if (chainProfiles.size <= 1) CoreResolvedType.NORMAL else CoreResolvedType.PROXYCHAIN
                Pair(chainProfiles, type)
            }
        }

        // Create context with common fields
        return CoreConfigContext(
            context = context,
            guid = guid,
            selectedProfile = config,
            resolvedProfiles = resolvedProfiles,
            resolvedType = resolvedType,
            customOutboundProfiles = customOutbounds,
        )
    }

    /** Resolves policy-group members with the same filters as runtime build. */
    private fun resolvePolicyGroupProfiles(config: ProfileItem): List<ProfileItem> {
        try {
            val serverList = MmkvManager.decodeAllServerList()
            return serverList
                .asSequence()
                .mapNotNull { id -> MmkvManager.decodeServerConfig(id) }
                .filter { profile ->
                    val subscriptionId = config.policyGroupSubscriptionId
                    if (subscriptionId.isNullOrBlank()) {
                        true
                    } else {
                        profile.subscriptionId == subscriptionId
                    }
                }
                .filter { profile ->
                    val filter = config.policyGroupFilter
                    if (filter.isNullOrBlank()) {
                        true
                    } else {
                        try {
                            Regex(filter).containsMatchIn(profile.remarks)
                        } catch (_: Exception) {
                            profile.remarks.contains(filter)
                        }
                    }
                }
                .filter { it.server.isNotNullEmpty() }
                .filter { !Utils.isPureIpAddress(it.server!!) || Utils.isValidUrl(it.server!!) }
                .filter { it.configType != EConfigType.CUSTOM }
                .filter { it.configType != EConfigType.POLICYGROUP }
                .filter { it.configType != EConfigType.PROXYCHAIN }
                .toList()
        } catch (e: Exception) {
            LogUtil.e(AppConfig.TAG, "Failed to resolve policy group profiles for config '${config.remarks}'", e)
            return listOf(config)
        }
    }

    /** Resolves proxy-chain members with the same filters as runtime build. */
    private fun resolveProxyChainProfiles(config: ProfileItem): List<ProfileItem> {
        if (config.proxyChainProfiles.isNullOrBlank()) {
            return listOf(config)
        }

        try {
            return config.proxyChainProfiles.orEmpty().split(",")
                .asSequence()
                .mapNotNull { remark -> SettingsManager.getServerViaRemarks(remark) }
                .filter { it.server.isNotNullEmpty() }
                .filter { !Utils.isPureIpAddress(it.server!!) || Utils.isValidUrl(it.server!!) }
                .filter { it.configType != EConfigType.CUSTOM }
                .filter { it.configType != EConfigType.POLICYGROUP }
                .filter { it.configType != EConfigType.PROXYCHAIN }
                .toList()
                .reversed()
        } catch (e: Exception) {
            LogUtil.e(AppConfig.TAG, "Failed to resolve proxy chain profiles for config '${config.remarks}'", e)
            return listOf(config)
        }
    }

    /**
     * Resolves chain nodes in fixed order: next -> current -> prev.
     * If chain cannot be built, caller treats result as normal mode.
     */
    private fun resolveProxyChainProfilesFromGroup(config: ProfileItem): List<ProfileItem> {
        if (MmkvManager.decodeSettingsBool(AppConfig.PREF_FRAGMENT_ENABLED, false) == true) {
            return listOf(config)
        }
        if (config.subscriptionId.isEmpty()) {
            return listOf(config)
        }

        try {
            val subItem = MmkvManager.decodeSubscription(config.subscriptionId) ?: return listOf(config)
            val resolved = mutableListOf<ProfileItem>()

            // Keep the same practical chain order as current runtime assembly:
            // next -> current -> prev
            SettingsManager.getServerViaRemarks(subItem.nextProfile)?.let { resolved.add(it) }
            resolved.add(config)
            SettingsManager.getServerViaRemarks(subItem.prevProfile)?.let { resolved.add(it) }

            return resolved
        } catch (e: Exception) {
            LogUtil.e(AppConfig.TAG, "Failed to resolve proxy chain profiles from group for config '${config.remarks}'", e)
            return listOf(config)
        }
    }

    /**
     * Resolves custom outbound profiles from routing rulesets.
     * Scans rulesets for non-builtin outbound tags and looks up matching profiles by remarks.
     * Returns a map of tag -> ProfileItem for independent processing.
     */
    private fun resolveCustomOutbounds(): Map<String, ProfileItem> {
        val customMap = mutableMapOf<String, ProfileItem>()
        val rulesetItems = MmkvManager.decodeRoutingRulesets() ?: return customMap

        try {
            val processedTags = mutableSetOf<String>()

            rulesetItems
                .filter { it.enabled }
                .mapNotNull { it.outboundTag.takeIf { tag -> tag.isNotBlank() } }
                .filter { tag -> tag !in AppConfig.BUILTIN_OUTBOUND_TAGS }
                .distinct()
                .forEach { tag ->
                    if (tag in processedTags) return@forEach
                    processedTags.add(tag)

                    try {
                        val profile = SettingsManager.getServerViaRemarks(tag) ?: run {
                            return@forEach
                        }

                        customMap[tag] = profile
                    } catch (e: Exception) {
                        LogUtil.e(AppConfig.TAG, "Failed to resolve custom outbound for tag '$tag', skipping", e)
                    }
                }
        } catch (e: Exception) {
            LogUtil.e(AppConfig.TAG, "Failed to resolve custom outbound profiles", e)
        }

        return customMap
    }
}

