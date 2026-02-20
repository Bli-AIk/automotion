package com.bliailk.automotion

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.graphics.Path
import android.util.Log
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo

/**
 * 核心自动化引擎：通过 AccessibilityService 操控 Alemon UI。
 *
 * 职责：
 * - 接收 UI 事件，遍历节点树查找目标元素
 * - 执行点击（performAction / dispatchGesture）
 * - 处理弹窗（使用 Rust 核心库提供的 POPUP_DISMISS_TEXTS）
 * - 监控渲染状态（exportButton → saveButton 流程）
 *
 * 所有 amproj 解析、拆分、配置常量均通过 UniFFI 从 Rust 核心库获取。
 */
class AutomationService : AccessibilityService() {

    companion object {
        private const val TAG = "AutomationService"
        const val ALEMON_PACKAGE = "com.taffy.alemon"

        // 单例引用，供 MainActivity 通信
        @Volatile
        var instance: AutomationService? = null
            private set
    }

    override fun onServiceConnected() {
        super.onServiceConnected()
        instance = this
        Log.i(TAG, "AutomationService 已连接")
        AppLog.log(TAG, "无障碍服务已连接")
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        event ?: return
        if (event.packageName?.toString() != ALEMON_PACKAGE) return

        // TODO: 根据当前自动化阶段处理事件
        //  - 事件驱动比轮询更高效
        //  - 使用 rootInActiveWindow 获取完整节点树
    }

    override fun onInterrupt() {
        Log.w(TAG, "AutomationService 被中断")
    }

    override fun onDestroy() {
        instance = null
        AppLog.log(TAG, "无障碍服务已断开")
        super.onDestroy()
    }

    // ── 节点查找 ──────────────────────────────────────────────────────────

    /**
     * 在当前窗口中查找包含指定文本的节点
     */
    fun findNodeByText(text: String): AccessibilityNodeInfo? {
        val root = rootInActiveWindow ?: return null
        val nodes = root.findAccessibilityNodeInfosByText(text)
        return nodes?.firstOrNull { it.isClickable }
            ?: nodes?.firstOrNull()
    }

    /**
     * 在当前窗口中通过 resource-id 查找节点
     */
    fun findNodeById(resourceId: String): AccessibilityNodeInfo? {
        val root = rootInActiveWindow ?: return null
        val nodes = root.findAccessibilityNodeInfosByViewId(resourceId)
        return nodes?.firstOrNull()
    }

    // ── 点击操作 ──────────────────────────────────────────────────────────

    /**
     * 点击指定节点
     */
    fun clickNode(node: AccessibilityNodeInfo): Boolean {
        // 优先尝试直接 performAction
        if (node.isClickable) {
            return node.performAction(AccessibilityNodeInfo.ACTION_CLICK)
        }
        // 节点不可点击时，尝试点击其父节点
        var parent = node.parent
        while (parent != null) {
            if (parent.isClickable) {
                return parent.performAction(AccessibilityNodeInfo.ACTION_CLICK)
            }
            parent = parent.parent
        }
        // 最后手段：通过手势在节点中心点击
        return clickAtCenter(node)
    }

    /**
     * 通过手势在节点中心坐标点击（最可靠的方式）
     */
    private fun clickAtCenter(node: AccessibilityNodeInfo): Boolean {
        val rect = android.graphics.Rect()
        node.getBoundsInScreen(rect)
        val x = rect.centerX().toFloat()
        val y = rect.centerY().toFloat()

        val path = Path().apply { moveTo(x, y) }
        val gesture = GestureDescription.Builder()
            .addStroke(GestureDescription.StrokeDescription(path, 0, 100))
            .build()

        return dispatchGesture(gesture, null, null)
    }

    // ── 弹窗处理 ──────────────────────────────────────────────────────────

    /**
     * 尝试关闭弹窗，使用 Rust 核心库提供的弹窗关键词列表（通过 UniFFI）
     * 返回是否成功关闭了弹窗
     */
    fun dismissPopups(): Boolean {
        val popupTexts = try {
            uniffi.automotion_core.getPopupDismissTexts()
        } catch (_: Exception) {
            listOf("仍要导出", "跳过", "稍后", "不了", "跳过广告", "确定", "确认", "好")
        }

        for (text in popupTexts) {
            val node = findNodeByText(text)
            if (node != null) {
                Log.i(TAG, "检测到弹窗 [$text]，点击关闭")
                clickNode(node)
                return true
            }
        }
        return false
    }
}
