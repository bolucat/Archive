package com.v2ray.ang.dto

import android.content.Context
import com.v2ray.ang.enums.CoreResolvedType

/**
 * Runtime context produced by the builder and consumed by CoreConfigManager.
 */
data class CoreConfigContext(
    val context: Context,
    val guid: String,
    val selectedProfile: ProfileItem,
    val resolvedProfiles: List<ProfileItem>,
    val resolvedType: CoreResolvedType,
    val customOutboundProfiles: Map<String, ProfileItem> = emptyMap(),
)