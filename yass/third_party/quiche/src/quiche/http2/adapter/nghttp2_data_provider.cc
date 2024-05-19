#include "quiche/http2/adapter/nghttp2_data_provider.h"

#include <memory>

#include "quiche/http2/adapter/http2_visitor_interface.h"
#include "quiche/http2/adapter/nghttp2_util.h"

namespace http2 {
namespace adapter {
namespace callbacks {

namespace {
const size_t kFrameHeaderSize = 9;
}

#if NGHTTP2_VERSION_NUM >= 0x013c00
nghttp2_ssize
#else
ssize_t
#endif
DataFrameSourceReadCallback(nghttp2_session* /* session */,
                            int32_t /* stream_id */, uint8_t* /* buf */,
                            size_t length, uint32_t* data_flags,
                            nghttp2_data_source* source,
                            void* /* user_data */) {
  *data_flags |= NGHTTP2_DATA_FLAG_NO_COPY;
  auto* frame_source = static_cast<DataFrameSource*>(source->ptr);
  auto [result_length, done] = frame_source->SelectPayloadLength(length);
  if (result_length == 0 && !done) {
    return NGHTTP2_ERR_DEFERRED;
  } else if (result_length == DataFrameSource::kError) {
    return NGHTTP2_ERR_TEMPORAL_CALLBACK_FAILURE;
  }
  if (done) {
    *data_flags |= NGHTTP2_DATA_FLAG_EOF;
  }
  if (!frame_source->send_fin()) {
    *data_flags |= NGHTTP2_DATA_FLAG_NO_END_STREAM;
  }
  return result_length;
}

int DataFrameSourceSendCallback(nghttp2_session* /* session */,
                                nghttp2_frame* /* frame */,
                                const uint8_t* framehd, size_t length,
                                nghttp2_data_source* source,
                                void* /* user_data */) {
  auto* frame_source = static_cast<DataFrameSource*>(source->ptr);
  frame_source->Send(ToStringView(framehd, kFrameHeaderSize), length);
  return 0;
}

}  // namespace callbacks

#if NGHTTP2_VERSION_NUM >= 0x013c00
std::unique_ptr<nghttp2_data_provider2> MakeDataProvider(
#else
std::unique_ptr<nghttp2_data_provider> MakeDataProvider(
#endif
    DataFrameSource* source) {
  if (source == nullptr) {
    return nullptr;
  }
#if NGHTTP2_VERSION_NUM >= 0x013c00
  auto provider = std::make_unique<nghttp2_data_provider2>();
#else
  auto provider = std::make_unique<nghttp2_data_provider>();
#endif
  provider->source.ptr = source;
  provider->read_callback = &callbacks::DataFrameSourceReadCallback;
  return provider;
}

}  // namespace adapter
}  // namespace http2
