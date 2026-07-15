package org.velum.velum_client

/** Emergency cancellation path owned by the Android service lifecycle. */
object NativeTun {
    init {
        System.loadLibrary("velum_client_ffi")
    }

    external fun stop(): Int
}
