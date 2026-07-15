# Velum Native Runtime Libraries

Place the Android builds of `libvelum_client_ffi.so` in ABI-specific directories
before building a distributable APK or App Bundle:

```text
jniLibs/arm64-v8a/libvelum_client_ffi.so
jniLibs/armeabi-v7a/libvelum_client_ffi.so
jniLibs/x86_64/libvelum_client_ffi.so
```

Build all supported Android ABIs locally with:

```text
scripts/build-android-client.sh
```

The Flutter Android host loads this library through the runtime control ABI. The
same artifact contains the pinned userspace TCP/UDP engine described by ADR-0016;
the VPN service passes its raw descriptor to that engine without moving packet
content through Flutter platform channels.
