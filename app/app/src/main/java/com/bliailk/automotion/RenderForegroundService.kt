package com.bliailk.automotion

import android.app.*
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.Uri
import android.os.Build
import android.os.IBinder
import android.util.Log
import kotlinx.coroutines.*

/**
 * 前台服务：在批量渲染期间保持应用存活。
 *
 * Android 会杀死长时间后台运行的应用，而渲染可能持续 30+ 分钟。
 * 前台服务 + 通知确保系统不会杀死进程。
 */
class RenderForegroundService : Service() {

    companion object {
        private const val TAG = "RenderForegroundService"
        private const val NOTIFICATION_ID = 1001
        private const val CHANNEL_ID = "automotion_render"
        private const val CHANNEL_NAME = "渲染进度"

        @Volatile
        var isRunning = false
            private set
    }

    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())
    private var engine: RenderEngine? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        isRunning = true
        AppLog.log(TAG, "渲染服务已启动")
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val mode = intent?.getStringExtra("mode") ?: "render"
        val uris = intent?.getParcelableArrayListExtra<Uri>("file_uris")
        val filePaths = intent?.getStringArrayListExtra("file_paths")

        if (uris == null && filePaths == null) {
            stopSelf()
            return START_NOT_STICKY
        }

        val totalCount = uris?.size ?: filePaths?.size ?: 0

        // 启动前台通知
        val notification = buildNotification("准备中...", 0, totalCount)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }

        // 启动渲染引擎
        engine = RenderEngine(applicationContext).apply {
            onProgress = { current, total, message ->
                updateNotification(message, current, total)
                AppLog.log(TAG, "[$current/$total] $message")
                Log.i(TAG, "[$current/$total] $message")
            }
            onComplete = { success, failed ->
                AppLog.log(TAG, "批量渲染完成: 成功=$success, 失败=$failed")
                Log.i(TAG, "批量渲染完成: 成功=$success, 失败=$failed")
                updateNotification("完成: 成功 $success / 失败 $failed", success + failed, success + failed)
                scope.launch {
                    delay(3000)
                    stopSelf()
                }
            }
        }

        scope.launch {
            try {
                if (filePaths != null) {
                    // 本地文件模式（拆分后渲染）
                    val files = filePaths.map { java.io.File(it) }
                    engine?.batchRenderFiles(files)
                } else if (uris != null) {
                    engine?.batchRender(uris)
                }
            } catch (e: Exception) {
                Log.e(TAG, "批量渲染异常", e)
                AppLog.log(TAG, "❌ 批量渲染异常: ${e.message}")
                updateNotification("错误: ${e.message}", 0, 0)
                delay(3000)
                stopSelf()
            }
        }

        return START_NOT_STICKY
    }

    override fun onDestroy() {
        engine?.cancel()
        scope.cancel()
        isRunning = false
        super.onDestroy()
    }

    private fun createNotificationChannel() {
        val channel = NotificationChannel(
            CHANNEL_ID,
            CHANNEL_NAME,
            NotificationManager.IMPORTANCE_LOW
        ).apply {
            description = "显示 amproj 批量渲染进度"
        }
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(channel)
    }

    private fun buildNotification(text: String, progress: Int, max: Int): Notification {
        val pendingIntent = PendingIntent.getActivity(
            this, 0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        return Notification.Builder(this, CHANNEL_ID)
            .setContentTitle("Automotion 渲染中")
            .setContentText(text)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .setProgress(max, progress, max == 0)
            .build()
    }

    private fun updateNotification(text: String, progress: Int, max: Int) {
        val notification = buildNotification(text, progress, max)
        val nm = getSystemService(NotificationManager::class.java)
        nm.notify(NOTIFICATION_ID, notification)
    }
}
