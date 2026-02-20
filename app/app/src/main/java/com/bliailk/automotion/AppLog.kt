package com.bliailk.automotion

import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import java.text.SimpleDateFormat
import java.util.*

/**
 * 应用内日志收集器，替代 Logcat 查看。
 * 所有关键操作通过此对象记录，在 UI 中实时显示。
 */
object AppLog {
    private val _logs = mutableListOf<String>()
    private val _flow = MutableSharedFlow<String>(extraBufferCapacity = 100)

    val flow = _flow.asSharedFlow()

    val logs: List<String> get() = synchronized(_logs) { _logs.toList() }

    private val timeFormat = SimpleDateFormat("HH:mm:ss", Locale.getDefault())

    fun log(tag: String, message: String) {
        val line = "[${timeFormat.format(Date())}] $tag: $message"
        synchronized(_logs) {
            _logs.add(line)
            // 最多保留 500 条
            if (_logs.size > 500) _logs.removeAt(0)
        }
        _flow.tryEmit(line)
        android.util.Log.i(tag, message)
    }

    fun clear() {
        synchronized(_logs) { _logs.clear() }
    }
}
