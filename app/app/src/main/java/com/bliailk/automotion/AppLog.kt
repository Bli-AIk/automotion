package com.bliailk.automotion

import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import java.io.File
import java.text.SimpleDateFormat
import java.util.*

/**
 * 应用内日志收集器，替代 Logcat 查看。
 * 所有关键操作通过此对象记录，在 UI 中实时显示。
 * 同时写入 /sdcard/Download/automotion_log.txt 便于 ADB 拉取。
 */
object AppLog {
    private val _logs = mutableListOf<String>()
    private val _flow = MutableSharedFlow<String>(extraBufferCapacity = 100)
    private val logFile = File("/sdcard/Download/automotion_log.txt")

    val flow = _flow.asSharedFlow()

    val logs: List<String> get() = synchronized(_logs) { _logs.toList() }

    private val timeFormat = SimpleDateFormat("HH:mm:ss", Locale.getDefault())

    fun log(tag: String, message: String) {
        val line = "[${timeFormat.format(Date())}] $tag: $message"
        synchronized(_logs) {
            _logs.add(line)
            if (_logs.size > 500) _logs.removeAt(0)
        }
        _flow.tryEmit(line)
        android.util.Log.i(tag, message)
        // 追加写入文件
        try { logFile.appendText(line + "\n") } catch (_: Exception) {}
    }

    fun clear() {
        synchronized(_logs) { _logs.clear() }
        try { logFile.writeText("") } catch (_: Exception) {}
    }

    fun getAllText(): String = synchronized(_logs) { _logs.joinToString("\n") }
}
