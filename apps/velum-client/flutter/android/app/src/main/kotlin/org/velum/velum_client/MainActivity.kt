package org.velum.velum_client

import android.app.Activity
import android.content.Intent
import android.net.VpnService
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    private var permissionResult: MethodChannel.Result? = null

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, VPN_CHANNEL).setMethodCallHandler { call, result ->
            if (call.method != "requestPermission") {
                result.notImplemented()
                return@setMethodCallHandler
            }
            if (permissionResult != null) {
                result.error("busy", "A VPN permission request is already active.", null)
                return@setMethodCallHandler
            }
            val consent = VpnService.prepare(this)
            if (consent == null) {
                result.success(true)
            } else {
                permissionResult = result
                startActivityForResult(consent, VPN_PERMISSION_REQUEST)
            }
        }
    }

    @Deprecated("Deprecated in Java")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != VPN_PERMISSION_REQUEST) return
        permissionResult?.success(resultCode == Activity.RESULT_OK)
        permissionResult = null
    }

    override fun onDestroy() {
        permissionResult?.error("cancelled", "The VPN permission request was cancelled.", null)
        permissionResult = null
        super.onDestroy()
    }

    private companion object {
        const val VPN_CHANNEL = "org.velum.velum_client/vpn"
        const val VPN_PERMISSION_REQUEST = 3001
    }
}
