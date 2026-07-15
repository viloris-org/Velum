# Velum Native Runtime Libraries

Place the Android builds of `libvelum_client_ffi.so` in ABI-specific directories
before building a distributable APK or App Bundle:

```text
jniLibs/arm64-v8a/libvelum_client_ffi.so
jniLibs/armeabi-v7a/libvelum_client_ffi.so
jniLibs/x86_64/libvelum_client_ffi.so
```

Build arm64 locally with:

```text
scripts/build-android-client.sh
```

The Flutter Android host loads this library through the existing runtime ABI v1.
The runner still provides control only: a bounded UDP engine exists in Rust, but
it has no JNI fd pump and no TCP/DNS engine, so it does not establish a TUN
device until ADR-0014 is complete and device-tested.
