#ifndef QUICHE_HTTP2_ADAPTER_NGHTTP2_DATA_PROVIDER_H_
#define QUICHE_HTTP2_ADAPTER_NGHTTP2_DATA_PROVIDER_H_

#include <cstdint>
#include <memory>

#include "quiche/http2/adapter/data_source.h"
#include "quiche/http2/adapter/nghttp2.h"

namespace http2 {
namespace adapter {
namespace callbacks {

// Assumes |source| is a DataFrameSource.
#if NGHTTP2_VERSION_NUM >= 0x013c00
nghttp2_ssize
#else
ssize_t
#endif
DataFrameSourceReadCallback(nghttp2_session* /*session */,
                            int32_t /* stream_id */, uint8_t* /* buf */,
                            size_t length, uint32_t* data_flags,
                            nghttp2_data_source* source,
                            void* /* user_data */);

int DataFrameSourceSendCallback(nghttp2_session* /* session */,
                                nghttp2_frame* /* frame */,
                                const uint8_t* framehd, size_t length,
                                nghttp2_data_source* source,
                                void* /* user_data */);

}  // namespace callbacks

#if NGHTTP2_VERSION_NUM >= 0x013c00
// Transforms a DataFrameSource into a nghttp2_data_provider2. Does not take
// ownership of |source|. Returns nullptr if |source| is nullptr.
std::unique_ptr<nghttp2_data_provider2> MakeDataProvider(
    DataFrameSource* source);
#else
// Transforms a DataFrameSource into a nghttp2_data_provider. Does not take
// ownership of |source|. Returns nullptr if |source| is nullptr.
std::unique_ptr<nghttp2_data_provider> MakeDataProvider(
    DataFrameSource* source);
#endif

}  // namespace adapter
}  // namespace http2

#endif  // QUICHE_HTTP2_ADAPTER_NGHTTP2_DATA_PROVIDER_H_
