#ifndef VELUM_CLIENT_H
#define VELUM_CLIENT_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef int32_t VelumStatus;
typedef int32_t VelumControlStatus;

#define VELUM_STATUS_OK 0
#define VELUM_STATUS_INVALID_ARGUMENT 1
#define VELUM_STATUS_INVALID_HANDLE 2
#define VELUM_STATUS_CONFIGURATION 3
#define VELUM_STATUS_CERTIFICATE 4
#define VELUM_STATUS_CONNECT_TIMEOUT 5
#define VELUM_STATUS_CONNECTION 6
#define VELUM_STATUS_CONTROL_TOO_LARGE 7
#define VELUM_STATUS_TRANSPORT 8
#define VELUM_STATUS_DATAGRAM_TOO_LARGE 9
#define VELUM_STATUS_DATAGRAM_UNAVAILABLE 10
#define VELUM_STATUS_PROTOCOL 11

#define VELUM_CONTROL_OK 0
#define VELUM_CONTROL_INVALID_ARGUMENT 1
#define VELUM_CONTROL_INVALID_HANDLE 2
#define VELUM_CONTROL_CONFIGURATION 3
#define VELUM_CONTROL_CERTIFICATE 4
#define VELUM_CONTROL_BUSY 5
#define VELUM_CONTROL_INTERNAL 6

#define VELUM_RUNTIME_STOPPED 0U
#define VELUM_RUNTIME_CONNECTING 1U
#define VELUM_RUNTIME_ONLINE 2U
#define VELUM_RUNTIME_STOPPING 3U
#define VELUM_RUNTIME_FAILED 4U

#define VELUM_RUNTIME_FAILURE_NONE 0U
#define VELUM_RUNTIME_FAILURE_CERTIFICATE 1U
#define VELUM_RUNTIME_FAILURE_CONNECT_TIMEOUT 2U
#define VELUM_RUNTIME_FAILURE_CONNECTION 3U
#define VELUM_RUNTIME_FAILURE_CONTROL_TOO_LARGE 4U
#define VELUM_RUNTIME_FAILURE_DATAGRAM_TOO_LARGE 5U
#define VELUM_RUNTIME_FAILURE_DATAGRAM_UNAVAILABLE 6U
#define VELUM_RUNTIME_FAILURE_PROTOCOL 7U
#define VELUM_RUNTIME_FAILURE_TRANSPORT 8U

#define VELUM_TRUST_SYSTEM 0U
#define VELUM_TRUST_CUSTOM_CA 1U
#define VELUM_TRUST_INSECURE 2U

typedef struct VelumByteSlice {
    const uint8_t *pointer;
    size_t length;
} VelumByteSlice;

typedef struct VelumMutableByteSlice {
    uint8_t *pointer;
    size_t length;
} VelumMutableByteSlice;

typedef struct VelumClientConfigInput {
    VelumByteSlice relay_address;
    VelumByteSlice server_name;
    VelumByteSlice credential;
    VelumByteSlice certificate_pem;
    uint64_t connect_timeout_millis;
    uint32_t trust_mode;
} VelumClientConfigInput;

typedef struct VelumRuntimeSnapshotV1 {
    uint64_t revision;
    uint64_t generation;
    uint32_t phase;
    uint32_t failure;
} VelumRuntimeSnapshotV1;

uint16_t velum_client_abi_version(void);
uint16_t velum_client_runtime_abi_version(void);

VelumStatus velum_client_connect(
    const VelumClientConfigInput *input,
    uint64_t *out_client_handle);
VelumStatus velum_client_open_stream(
    uint64_t client_handle,
    VelumByteSlice target_address,
    uint64_t *out_stream_handle);
VelumStatus velum_client_stream_write(
    uint64_t stream_handle,
    VelumByteSlice input);
VelumStatus velum_client_stream_read(
    uint64_t stream_handle,
    VelumMutableByteSlice output,
    size_t *out_read);
VelumStatus velum_client_stream_finish(uint64_t stream_handle);
VelumStatus velum_client_stream_close(uint64_t stream_handle);
VelumStatus velum_client_close(uint64_t client_handle);

VelumControlStatus velum_client_runtime_create(uint64_t *out_runtime_handle);
VelumControlStatus velum_client_runtime_start_v1(
    uint64_t runtime_handle,
    const VelumClientConfigInput *input,
    uint64_t *out_generation);
VelumControlStatus velum_client_runtime_snapshot_v1(
    uint64_t runtime_handle,
    VelumRuntimeSnapshotV1 *out_snapshot);
VelumControlStatus velum_client_runtime_stop(
    uint64_t runtime_handle,
    uint64_t *out_generation);
VelumControlStatus velum_client_runtime_proxy_start(
    uint64_t runtime_handle,
    uint16_t requested_port,
    uint16_t *out_port);
VelumControlStatus velum_client_runtime_proxy_stop(uint64_t runtime_handle);
VelumControlStatus velum_client_runtime_destroy(uint64_t runtime_handle);

#if defined(__ANDROID__)
int32_t velum_client_android_tun_run(uint64_t runtime_handle, int32_t tun_fd);
int32_t velum_client_android_tun_stop(void);
#endif

#ifdef __cplusplus
}
#endif

#endif
