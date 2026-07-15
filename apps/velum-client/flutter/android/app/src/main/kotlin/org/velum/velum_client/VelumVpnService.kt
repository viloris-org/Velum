package org.velum.velum_client

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.net.VpnService
import android.os.Build
import io.flutter.plugin.common.MethodChannel

/** Owns Android consent, foreground lifetime, routes, and the TUN descriptor. */
class VelumVpnService : VpnService() {
    private var running = false

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> stopTunnel()
            ACTION_START -> startTunnel()
        }
        return Service.START_NOT_STICKY
    }

    override fun onRevoke() {
        stopTunnel()
        super.onRevoke()
    }

    override fun onDestroy() {
        runCatching { NativeTun.stop() }
        completePendingError("stopped", "The VPN service stopped before it became ready.")
        super.onDestroy()
    }

    private fun startTunnel() {
        if (running) {
            completePendingError("busy", "The VPN is already running.")
            return
        }

        startForeground(NOTIFICATION_ID, notification())
        try {
            val descriptor = Builder()
                .setSession("Velum")
                .setMtu(TUN_MTU)
                .addAddress(TUN_ADDRESS, TUN_PREFIX)
                .addRoute("0.0.0.0", 0)
                .addDnsServer(VIRTUAL_DNS)
                .addDisallowedApplication(packageName)
                .apply {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) setMetered(false)
                    setBlocking(true)
                }
                .establish()
                ?: error("Android rejected the VPN interface")
            running = true
            completePendingSuccess(descriptor.detachFd())
        } catch (error: Exception) {
            running = false
            completePendingError("start_failed", error.message ?: "VPN start failed.")
            stopSelf()
        }
    }

    private fun stopTunnel() {
        running = false
        runCatching { NativeTun.stop() }
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    @Suppress("DEPRECATION")
    private fun notification() = (if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
        Notification.Builder(this, CHANNEL_ID)
    } else {
        Notification.Builder(this)
    })
        .setSmallIcon(applicationInfo.icon)
        .setContentTitle("Velum VPN")
        .setContentText("Routing device traffic through the active Velum relay")
        .setOngoing(true)
        .setCategory(Notification.CATEGORY_SERVICE)
        .setContentIntent(
            PendingIntent.getActivity(
                this,
                0,
                packageManager.getLaunchIntentForPackage(packageName),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            ),
        )
        .build()

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(
                CHANNEL_ID,
                "VPN status",
                NotificationManager.IMPORTANCE_LOW,
            ),
        )
    }

    companion object {
        private const val ACTION_START = "org.velum.velum_client.START_VPN"
        private const val ACTION_STOP = "org.velum.velum_client.STOP_VPN"
        private const val CHANNEL_ID = "velum_vpn"
        private const val NOTIFICATION_ID = 4101
        private const val TUN_ADDRESS = "172.19.0.1"
        private const val TUN_PREFIX = 30
        private const val VIRTUAL_DNS = "8.8.8.8"
        private const val TUN_MTU = 1500

        @Volatile
        private var pendingResult: MethodChannel.Result? = null

        fun start(context: Context, result: MethodChannel.Result) {
            if (pendingResult != null) {
                result.error("busy", "A VPN start command is already active.", null)
                return
            }
            pendingResult = result
            val intent = Intent(context, VelumVpnService::class.java)
                .setAction(ACTION_START)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        fun stop(context: Context) {
            context.startService(
                Intent(context, VelumVpnService::class.java).setAction(ACTION_STOP),
            )
        }

        private fun completePendingSuccess(tunFd: Int) {
            pendingResult?.success(tunFd)
            pendingResult = null
        }

        private fun completePendingError(code: String, message: String) {
            pendingResult?.error(code, message, null)
            pendingResult = null
        }
    }
}
