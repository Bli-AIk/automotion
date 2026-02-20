package com.bliailk.automotion

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // 请求文件管理权限（Android 11+）
        if (!Environment.isExternalStorageManager()) {
            try {
                startActivity(Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION))
            } catch (_: Exception) {
                startActivity(Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
                    data = Uri.parse("package:$packageName")
                })
            }
        }

        // 请求通知权限（Android 13+），前台服务必需
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS)
                != PackageManager.PERMISSION_GRANTED
            ) {
                requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 100)
            }
        }

        setContent {
            MaterialTheme {
                MainScreen()
            }
        }
    }
}

@Composable
fun MainScreen() {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    var isServiceEnabled by remember { mutableStateOf(false) }
    var selectedFiles by remember { mutableStateOf<List<Uri>>(emptyList()) }
    var isRunning by remember { mutableStateOf(false) }
    var showLog by remember { mutableStateOf(false) }
    // 0 = 渲染, 1 = 拆分, 2 = 拆分并渲染
    var mode by remember { mutableIntStateOf(0) }
    val modeLabels = listOf("批量渲染", "拆分", "拆分并渲染")
    val coroutineScope = rememberCoroutineScope()

    // 日志收集
    val logLines = remember { mutableStateListOf<String>() }
    LaunchedEffect(Unit) {
        // 加载已有日志
        logLines.addAll(AppLog.logs)
        // 监听新日志
        AppLog.flow.collect { line ->
            logLines.add(line)
            if (logLines.size > 500) logLines.removeAt(0)
        }
    }

    // 每次 onResume 时刷新无障碍状态 + 定时轮询
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                isServiceEnabled = isAccessibilityServiceEnabled(context)
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    // 补充定时轮询（每 2 秒），处理后台切换等边缘情况
    LaunchedEffect(Unit) {
        while (true) {
            isServiceEnabled = isAccessibilityServiceEnabled(context)
            delay(2000)
        }
    }

    // 文件选择器
    val filePickerLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenMultipleDocuments()
    ) { uris ->
        selectedFiles = uris.filter { uri ->
            val name = getFilenameFromUri(context, uri)
            name?.endsWith(".amproj") == true
        }
    }

    Surface(
        modifier = Modifier.fillMaxSize(),
        color = MaterialTheme.colorScheme.background
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            // 标题
            Text(
                text = "Automotion",
                style = MaterialTheme.typography.headlineLarge
            )
            Text(
                text = "Alemon 批量渲染自动化",
                style = MaterialTheme.typography.bodyLarge
            )

            Spacer(modifier = Modifier.height(16.dp))

            // 无障碍服务状态
            if (!isServiceEnabled) {
                Card(
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.errorContainer
                    ),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text(
                            "⚠️ 请先开启无障碍服务",
                            color = MaterialTheme.colorScheme.onErrorContainer,
                            style = MaterialTheme.typography.titleMedium
                        )
                        Spacer(modifier = Modifier.height(4.dp))
                        Text(
                            "提示：清除后台可能导致系统关闭无障碍权限。请在设置中锁定此应用的后台运行。",
                            color = MaterialTheme.colorScheme.onErrorContainer,
                            style = MaterialTheme.typography.bodySmall
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Button(onClick = {
                            context.startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS))
                        }) {
                            Text("前往设置")
                        }
                    }
                }
            } else {
                Card(
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.primaryContainer
                    ),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text(
                        "✅ 无障碍服务已开启",
                        modifier = Modifier.padding(16.dp),
                        color = MaterialTheme.colorScheme.onPrimaryContainer
                    )
                }
            }

            Spacer(modifier = Modifier.height(12.dp))

            // 文件选择
            Button(
                onClick = {
                    filePickerLauncher.launch(arrayOf("application/zip", "application/octet-stream"))
                },
                enabled = isServiceEnabled && !isRunning,
                modifier = Modifier.fillMaxWidth()
            ) {
                Text("选择 amproj 文件")
            }

            // 中间区域：文件列表 或 日志
            if (showLog) {
                // 日志视图
                Row(
                    modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("📋 运行日志", style = MaterialTheme.typography.titleSmall)
                    Row {
                        TextButton(onClick = {
                            val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                            cm.setPrimaryClip(ClipData.newPlainText("automotion_log", AppLog.getAllText()))
                            Toast.makeText(context, "日志已复制", Toast.LENGTH_SHORT).show()
                        }) { Text("复制") }
                        TextButton(onClick = {
                            AppLog.clear()
                            logLines.clear()
                        }) { Text("清空") }
                        TextButton(onClick = { showLog = false }) { Text("返回") }
                    }
                }
                Card(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth(),
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.surfaceVariant
                    )
                ) {
                    val listState = rememberLazyListState()
                    // 新日志自动滚动到底部
                    LaunchedEffect(logLines.size) {
                        if (logLines.isNotEmpty()) {
                            listState.animateScrollToItem(logLines.size - 1)
                        }
                    }
                    LazyColumn(
                        state = listState,
                        modifier = Modifier.padding(8.dp)
                    ) {
                        items(logLines.size) { i ->
                            Text(
                                logLines[i],
                                style = MaterialTheme.typography.bodySmall,
                                modifier = Modifier.padding(vertical = 1.dp)
                            )
                        }
                    }
                }
            } else {
                // 文件列表视图
                if (selectedFiles.isNotEmpty()) {
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        "已选择 ${selectedFiles.size} 个文件：",
                        style = MaterialTheme.typography.titleSmall
                    )
                    LazyColumn(
                        modifier = Modifier
                            .weight(1f)
                            .fillMaxWidth()
                            .padding(vertical = 8.dp)
                    ) {
                        items(selectedFiles) { uri ->
                            val name = getFilenameFromUri(context, uri) ?: "未知文件"
                            Text(
                                "  • $name",
                                style = MaterialTheme.typography.bodyMedium,
                                modifier = Modifier.padding(vertical = 2.dp)
                            )
                        }
                    }
                } else {
                    Spacer(modifier = Modifier.weight(1f))
                }
            }

            // 底部按钮区域
            Column(modifier = Modifier.fillMaxWidth()) {
                // 模式选择
                if (!isRunning) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 4.dp),
                        horizontalArrangement = Arrangement.spacedBy(4.dp)
                    ) {
                        modeLabels.forEachIndexed { index, label ->
                            FilterChip(
                                selected = mode == index,
                                onClick = { mode = index },
                                label = { Text(label, style = MaterialTheme.typography.labelSmall) },
                                modifier = Modifier.weight(1f)
                            )
                        }
                    }
                }

                // 主操作按钮
                if (selectedFiles.isNotEmpty()) {
                    Button(
                        onClick = {
                            if (isRunning) {
                                isRunning = false
                                context.stopService(Intent(context, RenderForegroundService::class.java))
                                AppLog.log("UI", "用户停止操作")
                                Toast.makeText(context, "已停止", Toast.LENGTH_SHORT).show()
                                return@Button
                            }

                            when (mode) {
                                0 -> {
                                    // 批量渲染
                                    try {
                                        isRunning = true
                                        showLog = true
                                        AppLog.log("UI", "开始渲染 ${selectedFiles.size} 个文件")
                                        val intent = Intent(context, RenderForegroundService::class.java).apply {
                                            putExtra("mode", "render")
                                            putParcelableArrayListExtra("file_uris", ArrayList(selectedFiles))
                                        }
                                        context.startForegroundService(intent)
                                        Toast.makeText(context, "渲染服务已启动", Toast.LENGTH_SHORT).show()
                                    } catch (e: Exception) {
                                        isRunning = false
                                        AppLog.log("UI", "❌ 启动失败: ${e.message}")
                                        Toast.makeText(context, "启动失败: ${e.message}", Toast.LENGTH_LONG).show()
                                    }
                                }
                                1 -> {
                                    // 仅拆分（不需要无障碍服务）
                                    isRunning = true
                                    showLog = true
                                    coroutineScope.launch(Dispatchers.IO) {
                                        try {
                                            performSplit(context, selectedFiles)
                                        } catch (e: Exception) {
                                            AppLog.log("UI", "❌ 拆分失败: ${e.message}")
                                        } finally {
                                            isRunning = false
                                        }
                                    }
                                }
                                2 -> {
                                    // 拆分并渲染
                                    try {
                                        isRunning = true
                                        showLog = true
                                        coroutineScope.launch(Dispatchers.IO) {
                                            try {
                                                val splitFiles = performSplit(context, selectedFiles)
                                                if (splitFiles.isNotEmpty()) {
                                                    AppLog.log("UI", "开始渲染 ${splitFiles.size} 个拆分文件")
                                                    val intent = Intent(context, RenderForegroundService::class.java).apply {
                                                        putExtra("mode", "split-render")
                                                        putStringArrayListExtra("file_paths", ArrayList(splitFiles.map { it.absolutePath }))
                                                    }
                                                    context.startForegroundService(intent)
                                                } else {
                                                    AppLog.log("UI", "⚠️ 拆分结果为空，无文件可渲染")
                                                    isRunning = false
                                                }
                                            } catch (e: Exception) {
                                                AppLog.log("UI", "❌ 拆分并渲染失败: ${e.message}")
                                                isRunning = false
                                            }
                                        }
                                    } catch (e: Exception) {
                                        isRunning = false
                                        AppLog.log("UI", "❌ 启动失败: ${e.message}")
                                    }
                                }
                            }
                        },
                        enabled = if (mode == 1) !isRunning else isServiceEnabled && !isRunning || isRunning,
                        colors = if (isRunning) {
                            ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error)
                        } else {
                            ButtonDefaults.buttonColors()
                        },
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Text(if (isRunning) "停止" else modeLabels[mode])
                    }
                }

                // 查看日志按钮
                TextButton(
                    onClick = { showLog = !showLog },
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text(if (showLog) "隐藏日志" else "查看日志")
                }
            }
        }
    }
}

fun isAccessibilityServiceEnabled(context: android.content.Context): Boolean {
    val serviceName = "${context.packageName}/${AutomationService::class.java.canonicalName}"
    val enabledServices = Settings.Secure.getString(
        context.contentResolver,
        Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES
    ) ?: return false
    return enabledServices.contains(serviceName)
}

private fun getFilenameFromUri(context: android.content.Context, uri: Uri): String? {
    return context.contentResolver.query(uri, null, null, null, null)?.use { cursor ->
        val nameIndex = cursor.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
        cursor.moveToFirst()
        if (nameIndex >= 0) cursor.getString(nameIndex) else null
    }
}

/**
 * 执行 amproj 拆分（复用 Rust 核心库的 split_amproj_to_dir）
 * 返回拆分输出的文件列表
 */
private fun performSplit(context: android.content.Context, uris: List<Uri>): List<java.io.File> {
    val allOutputs = mutableListOf<java.io.File>()
    val downloadDir = android.os.Environment.getExternalStoragePublicDirectory(
        android.os.Environment.DIRECTORY_DOWNLOADS
    )
    val splitOutputDir = java.io.File(
        context.getExternalFilesDir(null), "split_output"
    )
    splitOutputDir.mkdirs()

    for (uri in uris) {
        val filename = getFilenameFromUri(context, uri) ?: continue
        AppLog.log("Split", "▶ 拆分: $filename")

        // 复制到临时位置供 Rust 读取
        val tempFile = java.io.File(downloadDir, filename)
        try {
            val inputStream = context.contentResolver.openInputStream(uri)
            if (inputStream == null) {
                AppLog.log("Split", "❌ 无法读取文件: $filename")
                continue
            }
            inputStream.use { input ->
                tempFile.outputStream().use { output ->
                    input.copyTo(output)
                }
            }

            // 调用 Rust 核心库拆分
            val outputPaths = uniffi.automotion_core.splitAmprojToDir(
                tempFile.absolutePath,
                splitOutputDir.absolutePath
            )

            AppLog.log("Split", "✅ $filename → ${outputPaths.size} 个元素")
            allOutputs.addAll(outputPaths.map { java.io.File(it) })
        } catch (e: Exception) {
            AppLog.log("Split", "❌ 拆分失败: ${e.message}")
        } finally {
            tempFile.delete()
        }
    }

    AppLog.log("Split", "拆分完成，共 ${allOutputs.size} 个文件")
    return allOutputs
}
