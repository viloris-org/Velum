package org.velum.velum_client

import android.content.Intent
import android.net.VpnService

/**
 * Android owns consent and the TUN descriptor lifecycle. The descriptor is
 * deliberately not established until the native packet engine is attached;
 * establishing a default route earlier would blackhole device traffic.
 */
class VelumVpnService : VpnService() {
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        stopSelf(startId)
        return START_NOT_STICKY
    }
}
