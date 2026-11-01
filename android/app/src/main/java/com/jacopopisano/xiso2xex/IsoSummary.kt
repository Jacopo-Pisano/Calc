package com.jacopopisano.xiso2xex

import org.json.JSONObject

data class IsoEntry(val path: String, val offset: Long, val size: Long, val isDir: Boolean)

data class ExeInfo(
    val format: String,
    val titleId: String?,
    val titleName: String?,
    val mediaId: String?,
    val version: String?,
    val discNumber: Int?,
    val discCount: Int?,
)

data class IsoSummary(
    val layoutLabel: String,
    val totalSize: Long,
    val fileCount: Int,
    val executable: ExeInfo?,
    val entries: List<IsoEntry>,
) {
    companion object {
        fun fromJson(json: String): IsoSummary {
            val o = JSONObject(json)
            val arr = o.getJSONArray("entries")
            val entries = List(arr.length()) { i ->
                val e = arr.getJSONObject(i)
                IsoEntry(
                    path = e.getString("path"),
                    offset = e.getLong("offset"),
                    size = e.getLong("size"),
                    isDir = e.getBoolean("is_dir"),
                )
            }
            val exe = o.optJSONObject("executable")?.let { x ->
                ExeInfo(
                    format = x.getString("format"),
                    titleId = x.optStringOrNull("titleId"),
                    titleName = x.optStringOrNull("titleName"),
                    mediaId = x.optStringOrNull("mediaId"),
                    version = x.optStringOrNull("version"),
                    discNumber = x.optIntOrNull("discNumber"),
                    discCount = x.optIntOrNull("discCount"),
                )
            }
            return IsoSummary(
                layoutLabel = o.getString("layoutLabel"),
                totalSize = o.getLong("totalSize"),
                fileCount = o.getInt("fileCount"),
                executable = exe,
                entries = entries,
            )
        }

        private fun JSONObject.optStringOrNull(key: String): String? =
            if (isNull(key)) null else optString(key)

        private fun JSONObject.optIntOrNull(key: String): Int? =
            if (isNull(key)) null else optInt(key)
    }
}

fun formatBytes(bytes: Long): String {
    val units = listOf("B", "KB", "MB", "GB", "TB")
    var v = bytes.toDouble()
    var u = 0
    while (v >= 1024 && u < units.lastIndex) {
        v /= 1024
        u++
    }
    return if (u == 0) "$bytes B" else String.format("%.2f %s", v, units[u])
}
