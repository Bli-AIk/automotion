package com.bliailk.automotion

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Environment
import android.util.Log
import android.view.accessibility.AccessibilityNodeInfo
import kotlinx.coroutines.*
import java.io.File

/**
 * 渲染引擎：批量自动化状态机。
 *
 * 对应 CLI 的 process_single_project 9 阶段流程，
 * 但使用 AccessibilityService 替代 ADB + uiautomator。
 */
class RenderEngine(private val context: Context) {

    companion object {
        private const val TAG = "RenderEngine"
        const val ALEMON_PACKAGE = "com.taffy.alemon"
        const val PHONE_VIDEO_DIR = "/sdcard/Movies/Alemon"

        // 弹窗关键词（从 Rust 核心库通过 UniFFI 获取）
        val POPUP_DISMISS_TEXTS: List<String> by lazy {
            try {
                uniffi.automotion_core.getPopupDismissTexts()
            } catch (_: Exception) {
                // 降级：硬编码列表（与 Rust config::POPUP_DISMISS_TEXTS 一致）
                listOf("仍要导出", "跳过", "稍后", "不了", "跳过广告", "确定", "确认", "好")
            }
        }

        // 时序参数
        const val UI_POLL_INTERVAL_MS = 500L
        const val UI_WAIT_TIMEOUT_MS = 10_000L
        const val SHARE_WAIT_TIMEOUT_MS = 30_000L
        const val MAX_RENDER_WAIT_MS = 1_800_000L // 30分钟
        const val STABLE_THRESHOLD_MS = 3_000L    // 文件大小稳定阈值（本地写入很快）
        const val POLL_INTERVAL_MS = 1_000L       // 文件轮询间隔
    }

    // 状态回调
    var onProgress: ((Int, Int, String) -> Unit)? = null  // (current, total, message)
    var onComplete: ((Int, Int) -> Unit)? = null           // (success, failed)

    private var cancelled = false

    fun cancel() {
        cancelled = true
    }

    /**
     * 批量处理 amproj 文件列表（从 Uri，如文件选择器）
     */
    suspend fun batchRender(files: List<Uri>) = withContext(Dispatchers.IO) {
        val total = files.size
        var success = 0
        var failed = 0

        for ((index, uri) in files.withIndex()) {
            if (cancelled) break

            val filename = getFilenameFromUri(uri) ?: "unknown_${index}.amproj"
            onProgress?.invoke(index + 1, total, "处理中: $filename")
            AppLog.log(TAG, "▶ 开始处理 [${ index + 1 }/$total]: $filename")

            try {
                val downloadFile = copyToDownload(uri, filename)
                processSingleProject(downloadFile, filename)
                success++
            } catch (e: Exception) {
                failed++
                Log.e(TAG, "工程处理失败: $filename", e)
                AppLog.log(TAG, "❌ $filename 失败: ${e.message}")
                onProgress?.invoke(index + 1, total, "❌ $filename: ${e.message}")
                forceStopAlemon()
            }
        }

        onComplete?.invoke(success, failed)
    }

    /**
     * 批量处理本地文件列表（用于拆分后渲染，文件已在设备上）
     */
    suspend fun batchRenderFiles(files: List<File>) = withContext(Dispatchers.IO) {
        val total = files.size
        var success = 0
        var failed = 0

        for ((index, file) in files.withIndex()) {
            if (cancelled) break

            val filename = file.name
            onProgress?.invoke(index + 1, total, "处理中: $filename")
            AppLog.log(TAG, "▶ 开始处理 [${ index + 1 }/$total]: $filename")

            try {
                // 复制到 Download 以便 FileProvider 共享给 Alemon
                val downloadFile = File(
                    Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS),
                    filename
                )
                file.copyTo(downloadFile, overwrite = true)
                processSingleProject(downloadFile, filename)
                success++
            } catch (e: Exception) {
                failed++
                Log.e(TAG, "工程处理失败: $filename", e)
                AppLog.log(TAG, "❌ $filename 失败: ${e.message}")
                onProgress?.invoke(index + 1, total, "❌ $filename: ${e.message}")
                forceStopAlemon()
            }
        }

        onComplete?.invoke(success, failed)
    }

    /**
     * 处理单个 amproj 工程（对应 CLI 的 9 阶段流程）
     * @param downloadFile 已在 Download 目录中的文件
     */
    private suspend fun processSingleProject(downloadFile: File, filename: String) {
        val service = AutomationService.instance
            ?: throw IllegalStateException("无障碍服务未运行")

        // 阶段 1: 分析文件
        AppLog.log(TAG, "[1/9] 分析文件: $filename")

        // 分析 amproj 类型和标题（在导入前完成）
        val (projType, projTitle) = try {
            val info = uniffi.automotion_core.analyzeAmproj(downloadFile.absolutePath)
            val type = when (info.projType) {
                uniffi.automotion_core.FfiAmprojType.PROJECT -> "project"
                uniffi.automotion_core.FfiAmprojType.ELEMENT -> "element"
            }
            Pair(type, info.title)
        } catch (e: Exception) {
            AppLog.log(TAG, "⚠️ amproj 分析失败: ${e.message}，使用文件名作为标题")
            Pair("project", filename.removeSuffix(".amproj"))
        }
        AppLog.log(TAG, "工程类型: $projType, 标题: \"$projTitle\"")

        Log.i(TAG, "[2/9] 触发 Alemon 导入")
        AppLog.log(TAG, "[2/9] 触发 Alemon 导入")
        launchAlemonWithFile(downloadFile)
        delay(3000) // CLEAR_TASK 导致冷启动，需要更长加载时间

        // 阶段 3: 等待"导入"按钮并点击
        Log.i(TAG, "[3/9] 确认导入对话框")
        AppLog.log(TAG, "[3/9] 确认导入对话框")
        if (!waitAndTapText(service, "导入", UI_WAIT_TIMEOUT_MS)) {
            // 重试：强制停止后重新打开
            Log.w(TAG, "导入对话框未出现，重试中...")
            AppLog.log(TAG, "⚠️ 导入对话框未出现，重试...")
            forceStopAlemon()
            delay(2000)
            launchAlemonWithFile(downloadFile)
            delay(3000)
            if (!waitAndTapText(service, "导入", 15_000L)) {
                forceStopAlemon()
                downloadFile.delete()
                throw RuntimeException("无法找到导入确认按钮")
            }
        }
        delay(500)

        // 阶段 4: 等待"完成"按钮
        Log.i(TAG, "[4/9] 等待导入完成")
        AppLog.log(TAG, "[4/9] 等待导入完成")
        if (!waitAndTapText(service, "完成", UI_WAIT_TIMEOUT_MS)) {
            Log.w(TAG, "未找到完成按钮，尝试关闭弹窗...")
            AppLog.log(TAG, "⚠️ 未找到完成按钮，尝试关闭弹窗")
            service.dismissPopups()
        }
        delay(500)

        // 阶段 5: 启动 Alemon 主界面并根据类型导航到对应标签页
        val (tabId, tabText) = if (projType == "element") {
            Pair("tab_button_elements", "元素")
        } else {
            Pair("tab_button_projects", "项目")
        }
        Log.i(TAG, "[5/9] 打开 Alemon 并导航到「$tabText」标签页")
        AppLog.log(TAG, "[5/9] 导航到「$tabText」标签页")
        launchAlemonMain()
        delay(1000) // 等待主界面加载

        // 尝试通过 resource-id 点击标签
        if (!waitAndTapById(service, tabId, UI_WAIT_TIMEOUT_MS)) {
            // 降级：通过文本查找
            AppLog.log(TAG, "⚠️ 未找到 $tabId，尝试文本匹配「$tabText」")
            if (!waitAndTapText(service, tabText, UI_WAIT_TIMEOUT_MS)) {
                forceStopAlemon()
                downloadFile.delete()
                throw RuntimeException("无法导航到「$tabText」标签页")
            }
        }
        delay(1000) // 等待列表加载

        // 阶段 6: 在列表中找到并打开工程
        Log.i(TAG, "[6/9] 打开导入的工程: $projTitle")
        AppLog.log(TAG, "[6/9] 查找工程: \"$projTitle\"")
        if (!waitAndTapText(service, projTitle, 15_000L)) {
            // 诊断：记录当前可见的节点
            AppLog.log(TAG, "⚠️ 未找到 \"$projTitle\"，尝试滚动列表...")
            // 尝试滚动列表后重试
            service.scrollDown()
            delay(1000)
            if (!waitAndTapText(service, projTitle, 10_000L)) {
                AppLog.log(TAG, "❌ 滚动后仍未找到 \"$projTitle\"")
                forceStopAlemon()
                downloadFile.delete()
                throw RuntimeException("在列表中未找到工程: $projTitle")
            }
        }
        delay(500)

        // 阶段 7: 等待 share 按钮（处理弹窗）→ 打开导出菜单 → 点击 exportButton
        Log.i(TAG, "[7/9] 打开导出菜单并开始渲染")
        AppLog.log(TAG, "[7/9] 打开导出菜单")

        val shareDeadline = System.currentTimeMillis() + SHARE_WAIT_TIMEOUT_MS
        var foundShare = false
        while (System.currentTimeMillis() < shareDeadline && !cancelled) {
            service.dismissPopups()
            val shareNode = service.findNodeById("$ALEMON_PACKAGE:id/share")
            if (shareNode != null) {
                service.clickNode(shareNode)
                foundShare = true
                break
            }
            delay(UI_POLL_INTERVAL_MS)
        }
        if (!foundShare) {
            forceStopAlemon()
            downloadFile.delete()
            throw RuntimeException("无法找到分享/导出按钮")
        }
        delay(500)

        // 等待导出菜单 → 点击 exportButton
        if (!waitForTextVisible(service, "视频", UI_WAIT_TIMEOUT_MS)) {
            forceStopAlemon()
            downloadFile.delete()
            throw RuntimeException("导出菜单未正确加载")
        }

        if (!waitAndTapById(service, "exportButton", UI_WAIT_TIMEOUT_MS)) {
            forceStopAlemon()
            downloadFile.delete()
            throw RuntimeException("无法找到导出按钮")
        }

        // 阶段 8: 等待渲染完成（saveButton 出现 = 渲染完成）
        Log.i(TAG, "[8/9] 等待渲染完成")
        AppLog.log(TAG, "[8/9] 渲染中...")
        val renderDeadline = System.currentTimeMillis() + MAX_RENDER_WAIT_MS
        var foundSave = false

        while (System.currentTimeMillis() < renderDeadline && !cancelled) {
            service.dismissPopups()
            val saveNode = service.findNodeById("$ALEMON_PACKAGE:id/saveButton")
            if (saveNode != null) {
                Log.i(TAG, "渲染完成，预览页已出现")
                service.clickNode(saveNode)
                foundSave = true
                break
            }
            delay(UI_POLL_INTERVAL_MS)
        }

        if (!foundSave) {
            forceStopAlemon()
            downloadFile.delete()
            throw RuntimeException("渲染超时")
        }

        // 等待文件保存完成（轮询文件大小）
        Log.i(TAG, "[8/9] 等待文件保存完成")
        val videoFile = waitForVideoFile(projTitle) ?: throw RuntimeException("视频文件未生成")
        Log.i(TAG, "视频保存完成: ${videoFile.absolutePath}")

        // 阶段 9: 清理
        Log.i(TAG, "[9/9] 清理")
        AppLog.log(TAG, "[9/9] 清理")
        forceStopAlemon()
        downloadFile.delete()

        Log.i(TAG, "工程 [$filename] 处理完成 ✓")
        AppLog.log(TAG, "✅ $filename 处理完成")
    }

    // ── 辅助方法 ──────────────────────────────────────────────────────────

    private fun copyToDownload(uri: Uri, filename: String): File {
        val downloadDir = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS)
        val target = File(downloadDir, filename)

        context.contentResolver.openInputStream(uri)?.use { input ->
            target.outputStream().use { output ->
                input.copyTo(output)
            }
        } ?: throw RuntimeException("无法读取文件: $uri")

        return target
    }

    private fun launchAlemonWithFile(file: File) {
        // 使用 am start 命令（类似 CLI，绕过 Android 12+ 后台 Activity 启动限制）
        try {
            val proc = Runtime.getRuntime().exec(arrayOf(
                "am", "start",
                "--activity-new-task", "--activity-clear-task",
                "-a", "android.intent.action.VIEW",
                "-d", "file://${file.absolutePath}",
                "-t", "application/zip",
                "-p", ALEMON_PACKAGE
            ))
            proc.waitFor(5, java.util.concurrent.TimeUnit.SECONDS)
        } catch (e: Exception) {
            Log.w(TAG, "am start 失败，回退到 startActivity: ${e.message}")
            val uri = androidx.core.content.FileProvider.getUriForFile(
                context, "${context.packageName}.fileprovider", file
            )
            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, "application/zip")
                setPackage(ALEMON_PACKAGE)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            context.startActivity(intent)
        }
    }

    private fun launchAlemonMain() {
        val intent = context.packageManager.getLaunchIntentForPackage(ALEMON_PACKAGE)
            ?: throw RuntimeException("Alemon 未安装")
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(intent)
    }

    private fun forceStopAlemon() {
        try {
            // 直接 am force-stop（不按 HOME，避免触发后台 Activity 启动限制）
            try {
                val proc = Runtime.getRuntime().exec(arrayOf("am", "force-stop", ALEMON_PACKAGE))
                proc.waitFor(3, java.util.concurrent.TimeUnit.SECONDS)
            } catch (_: Exception) {}

            // 补充：killBackgroundProcesses
            val am = context.getSystemService(android.content.Context.ACTIVITY_SERVICE) as android.app.ActivityManager
            am.killBackgroundProcesses(ALEMON_PACKAGE)

            Thread.sleep(1000)
            AppLog.log(TAG, "已清理 Alemon 进程")
        } catch (e: Exception) {
            Log.w(TAG, "退出 Alemon 失败: ${e.message}")
        }
    }

    private suspend fun waitAndTapText(
        service: AutomationService,
        text: String,
        timeoutMs: Long
    ): Boolean {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline && !cancelled) {
            val node = service.findNodeByText(text)
            if (node != null) {
                service.clickNode(node)
                return true
            }
            delay(UI_POLL_INTERVAL_MS)
        }
        return false
    }

    private suspend fun waitAndTapById(
        service: AutomationService,
        id: String,
        timeoutMs: Long
    ): Boolean {
        val deadline = System.currentTimeMillis() + timeoutMs
        val fullId = "$ALEMON_PACKAGE:id/$id"
        while (System.currentTimeMillis() < deadline && !cancelled) {
            val node = service.findNodeById(fullId)
            if (node != null) {
                service.clickNode(node)
                return true
            }
            delay(UI_POLL_INTERVAL_MS)
        }
        return false
    }

    private suspend fun waitForTextVisible(
        service: AutomationService,
        text: String,
        timeoutMs: Long
    ): Boolean {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline && !cancelled) {
            if (service.findNodeByText(text) != null) return true
            delay(UI_POLL_INTERVAL_MS)
        }
        return false
    }

    /**
     * 轮询等待视频文件出现并稳定
     */
    private suspend fun waitForVideoFile(projTitle: String): File? {
        val videoDir = File(PHONE_VIDEO_DIR)
        val deadline = System.currentTimeMillis() + MAX_RENDER_WAIT_MS

        // 等待匹配文件出现
        var videoFile: File? = null
        while (System.currentTimeMillis() < deadline && !cancelled) {
            videoFile = videoDir.listFiles()
                ?.filter { it.name.endsWith(".mp4") && it.name.contains(projTitle) }
                ?.maxByOrNull { it.lastModified() }

            if (videoFile != null) break
            delay(POLL_INTERVAL_MS)
        }

        videoFile ?: return null

        // 等待文件大小稳定
        val stableNeeded = (STABLE_THRESHOLD_MS / POLL_INTERVAL_MS).toInt()
        var stableCount = 0
        var lastSize = -1L

        while (System.currentTimeMillis() < deadline && !cancelled) {
            val currentSize = videoFile.length()
            if (currentSize == lastSize && currentSize > 0) {
                stableCount++
                if (stableCount >= stableNeeded) return videoFile
            } else {
                stableCount = 0
            }
            lastSize = currentSize
            delay(POLL_INTERVAL_MS)
        }

        return null
    }

    private fun getFilenameFromUri(uri: Uri): String? {
        return context.contentResolver.query(uri, null, null, null, null)?.use { cursor ->
            val nameIndex = cursor.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
            cursor.moveToFirst()
            if (nameIndex >= 0) cursor.getString(nameIndex) else null
        }
    }
}
