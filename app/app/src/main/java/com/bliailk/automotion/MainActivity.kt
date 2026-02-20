package com.bliailk.automotion

import android.Manifest
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
import kotlinx.coroutines.delay

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

            Spacer(modifier = Modifier.height(24.dp))

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

            Spacer(modifier = Modifier.height(16.dp))

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

            // 已选文件列表
            if (selectedFiles.isNotEmpty()) {
                Spacer(modifier = Modifier.height(12.dp))
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

            // 开始/停止按钮
            if (selectedFiles.isNotEmpty()) {
                Button(
                    onClick = {
                        if (!isRunning) {
                            try {
                                isRunning = true
                                val intent = Intent(context, RenderForegroundService::class.java).apply {
                                    putParcelableArrayListExtra("file_uris", ArrayList(selectedFiles))
                                }
                                context.startForegroundService(intent)
                                Toast.makeText(context, "渲染服务已启动", Toast.LENGTH_SHORT).show()
                            } catch (e: Exception) {
                                isRunning = false
                                Toast.makeText(context, "启动失败: ${e.message}", Toast.LENGTH_LONG).show()
                            }
                        } else {
                            isRunning = false
                            context.stopService(Intent(context, RenderForegroundService::class.java))
                            Toast.makeText(context, "渲染已停止", Toast.LENGTH_SHORT).show()
                        }
                    },
                    enabled = isServiceEnabled,
                    colors = if (isRunning) {
                        ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error)
                    } else {
                        ButtonDefaults.buttonColors()
                    },
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text(if (isRunning) "停止渲染" else "开始批量渲染")
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
