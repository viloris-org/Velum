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
import java.net.InetAddress

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
            ACTION_START -> startTunnel(intent)
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

    private fun startTunnel(intent: Intent) {
        if (running) {
            completePendingError("busy", "The VPN is already running.")
            return
        }

        startForeground(NOTIFICATION_ID, notification())
        try {
            val configuration = VpnConfiguration.fromIntent(intent)
            val descriptor = Builder()
                .setSession("Velum")
                .setMtu(configuration.mtu)
                .addDisallowedApplication(packageName)
                .apply {
                    configuration.addresses.forEach { address ->
                        addAddress(address.address, address.prefixLength)
                    }
                    configuration.routes.forEach { route ->
                        addRoute(route.address, route.prefixLength)
                    }
                    configuration.dnsServers.forEach(::addDnsServer)
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
        private const val EXTRA_ADDRESS = "address"
        private const val EXTRA_PREFIX_LENGTH = "prefixLength"
        private const val EXTRA_IPV6_ADDRESS = "ipv6Address"
        private const val EXTRA_IPV6_PREFIX_LENGTH = "ipv6PrefixLength"
        private const val EXTRA_MTU = "mtu"
        private const val EXTRA_DNS_SERVERS = "dnsServers"
        private const val EXTRA_ROUTE_ADDRESSES = "routeAddresses"
        private const val EXTRA_ROUTE_PREFIXES = "routePrefixes"

        @Volatile
        private var pendingResult: MethodChannel.Result? = null

        fun start(context: Context, result: MethodChannel.Result, arguments: Any?) {
            if (pendingResult != null) {
                result.error("busy", "A VPN start command is already active.", null)
                return
            }
            val configuration = runCatching {
                VpnConfiguration.fromMethodArguments(arguments)
            }.getOrElse { error ->
                result.error("invalid_config", error.message ?: "Invalid VPN configuration.", null)
                return
            }
            pendingResult = result
            val intent = Intent(context, VelumVpnService::class.java)
                .setAction(ACTION_START)
                .putExtra(EXTRA_ADDRESS, configuration.address)
                .putExtra(EXTRA_PREFIX_LENGTH, configuration.prefixLength)
                .putExtra(EXTRA_IPV6_ADDRESS, configuration.ipv6Address)
                .putExtra(EXTRA_IPV6_PREFIX_LENGTH, configuration.ipv6PrefixLength)
                .putExtra(EXTRA_MTU, configuration.mtu)
                .putStringArrayListExtra(
                    EXTRA_DNS_SERVERS,
                    ArrayList(configuration.dnsServers),
                )
                .putStringArrayListExtra(
                    EXTRA_ROUTE_ADDRESSES,
                    ArrayList(configuration.routes.map(VpnRoute::address)),
                )
                .putIntegerArrayListExtra(
                    EXTRA_ROUTE_PREFIXES,
                    ArrayList(configuration.routes.map(VpnRoute::prefixLength)),
                )
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

    private data class VpnRoute(val address: String, val prefixLength: Int) {
        init {
            require(address.isNotBlank()) { "VPN route address must not be empty." }
            val maxPrefix = if (InetAddress.getByName(address).address.size == 16) 128 else 32
            require(prefixLength in 0..maxPrefix) {
                "VPN route prefix is invalid for its address family."
            }
        }
    }

    private data class VpnAddress(val address: String, val prefixLength: Int)

    private data class VpnConfiguration(
        val address: String,
        val prefixLength: Int,
        val ipv6Address: String,
        val ipv6PrefixLength: Int,
        val mtu: Int,
        val dnsServers: List<String>,
        val routes: List<VpnRoute>,
    ) {
        init {
            require(address.isNotBlank()) { "VPN address must not be empty." }
            require(prefixLength in 0..32) { "VPN prefix must be between 0 and 32." }
            require(ipv6Address.isNotBlank()) { "VPN IPv6 address must not be empty." }
            require(ipv6PrefixLength in 0..128) {
                "VPN IPv6 prefix must be between 0 and 128."
            }
            require(mtu in 576..65535) { "VPN MTU must be between 576 and 65535." }
            require(dnsServers.none(String::isBlank)) { "VPN DNS server must not be empty." }
        }

        val addresses: List<VpnAddress>
            get() = listOf(
                VpnAddress(address, prefixLength),
                VpnAddress(ipv6Address, ipv6PrefixLength),
            )

        companion object {
            fun fromMethodArguments(arguments: Any?): VpnConfiguration {
                val values = arguments as? Map<*, *>
                    ?: error("VPN configuration is required.")
                val routeValues = values[EXTRA_ROUTE_ADDRESSES] ?: values["routes"]
                val routes = (routeValues as? List<*>)?.mapIndexed { index, value ->
                    val route = value as? Map<*, *>
                        ?: error("VPN route $index is invalid.")
                    VpnRoute(
                        route[EXTRA_ADDRESS] as? String
                            ?: error("VPN route $index address is required."),
                        (route[EXTRA_PREFIX_LENGTH] as? Number)?.toInt()
                            ?: error("VPN route $index prefix is required."),
                    )
                } ?: error("VPN routes are required.")
                return VpnConfiguration(
                    values[EXTRA_ADDRESS] as? String
                        ?: error("VPN address is required."),
                    (values[EXTRA_PREFIX_LENGTH] as? Number)?.toInt()
                        ?: error("VPN prefix is required."),
                    values[EXTRA_IPV6_ADDRESS] as? String
                        ?: error("VPN IPv6 address is required."),
                    (values[EXTRA_IPV6_PREFIX_LENGTH] as? Number)?.toInt()
                        ?: error("VPN IPv6 prefix is required."),
                    (values[EXTRA_MTU] as? Number)?.toInt()
                        ?: error("VPN MTU is required."),
                    (values[EXTRA_DNS_SERVERS] as? List<*>)?.mapIndexed { index, value ->
                        value as? String ?: error("VPN DNS server $index is invalid.")
                    } ?: error("VPN DNS servers are required."),
                    routes,
                )
            }

            fun fromIntent(intent: Intent): VpnConfiguration {
                val routeAddresses = intent.getStringArrayListExtra(EXTRA_ROUTE_ADDRESSES)
                    ?: error("VPN routes are missing.")
                val routePrefixes = intent.getIntegerArrayListExtra(EXTRA_ROUTE_PREFIXES)
                    ?: error("VPN route prefixes are missing.")
                require(routeAddresses.size == routePrefixes.size) {
                    "VPN route configuration is inconsistent."
                }
                return VpnConfiguration(
                    intent.getStringExtra(EXTRA_ADDRESS)
                        ?: error("VPN address is missing."),
                    intent.getIntExtra(EXTRA_PREFIX_LENGTH, -1),
                    intent.getStringExtra(EXTRA_IPV6_ADDRESS)
                        ?: error("VPN IPv6 address is missing."),
                    intent.getIntExtra(EXTRA_IPV6_PREFIX_LENGTH, -1),
                    intent.getIntExtra(EXTRA_MTU, -1),
                    intent.getStringArrayListExtra(EXTRA_DNS_SERVERS)
                        ?: error("VPN DNS servers are missing."),
                    routeAddresses.indices.map { index ->
                        VpnRoute(routeAddresses[index], routePrefixes[index])
                    },
                )
            }
        }
    }
}
