package loopback

import (
	_ "github.com/v2fly/v2ray-core/v5/common/protoext"
	protoreflect "google.golang.org/protobuf/reflect/protoreflect"
	protoimpl "google.golang.org/protobuf/runtime/protoimpl"
	reflect "reflect"
	sync "sync"
	unsafe "unsafe"
)

const (
	// Verify that this generated code is sufficiently up-to-date.
	_ = protoimpl.EnforceVersion(20 - protoimpl.MinVersion)
	// Verify that runtime/protoimpl is sufficiently up-to-date.
	_ = protoimpl.EnforceVersion(protoimpl.MaxVersion - 20)
)

type Config struct {
	state         protoimpl.MessageState `protogen:"open.v1"`
	InboundTag    string                 `protobuf:"bytes,1,opt,name=inbound_tag,json=inboundTag,proto3" json:"inbound_tag,omitempty"`
	unknownFields protoimpl.UnknownFields
	sizeCache     protoimpl.SizeCache
}

func (x *Config) Reset() {
	*x = Config{}
	mi := &file_proxy_loopback_config_proto_msgTypes[0]
	ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
	ms.StoreMessageInfo(mi)
}

func (x *Config) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*Config) ProtoMessage() {}

func (x *Config) ProtoReflect() protoreflect.Message {
	mi := &file_proxy_loopback_config_proto_msgTypes[0]
	if x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use Config.ProtoReflect.Descriptor instead.
func (*Config) Descriptor() ([]byte, []int) {
	return file_proxy_loopback_config_proto_rawDescGZIP(), []int{0}
}

func (x *Config) GetInboundTag() string {
	if x != nil {
		return x.InboundTag
	}
	return ""
}

var File_proxy_loopback_config_proto protoreflect.FileDescriptor

const file_proxy_loopback_config_proto_rawDesc = "" +
	"\n" +
	"\x1bproxy/loopback/config.proto\x12\x19v2ray.core.proxy.loopback\x1a common/protoext/extensions.proto\"C\n" +
	"\x06Config\x12\x1f\n" +
	"\vinbound_tag\x18\x01 \x01(\tR\n" +
	"inboundTag:\x18\x82\xb5\x18\x14\n" +
	"\boutbound\x12\bloopbackBl\n" +
	"\x1dcom.v2ray.core.proxy.loopbackP\x01Z-github.com/v2fly/v2ray-core/v5/proxy/loopback\xaa\x02\x19V2Ray.Core.Proxy.Loopbackb\x06proto3"

var (
	file_proxy_loopback_config_proto_rawDescOnce sync.Once
	file_proxy_loopback_config_proto_rawDescData []byte
)

func file_proxy_loopback_config_proto_rawDescGZIP() []byte {
	file_proxy_loopback_config_proto_rawDescOnce.Do(func() {
		file_proxy_loopback_config_proto_rawDescData = protoimpl.X.CompressGZIP(unsafe.Slice(unsafe.StringData(file_proxy_loopback_config_proto_rawDesc), len(file_proxy_loopback_config_proto_rawDesc)))
	})
	return file_proxy_loopback_config_proto_rawDescData
}

var file_proxy_loopback_config_proto_msgTypes = make([]protoimpl.MessageInfo, 1)
var file_proxy_loopback_config_proto_goTypes = []any{
	(*Config)(nil), // 0: v2ray.core.proxy.loopback.Config
}
var file_proxy_loopback_config_proto_depIdxs = []int32{
	0, // [0:0] is the sub-list for method output_type
	0, // [0:0] is the sub-list for method input_type
	0, // [0:0] is the sub-list for extension type_name
	0, // [0:0] is the sub-list for extension extendee
	0, // [0:0] is the sub-list for field type_name
}

func init() { file_proxy_loopback_config_proto_init() }
func file_proxy_loopback_config_proto_init() {
	if File_proxy_loopback_config_proto != nil {
		return
	}
	type x struct{}
	out := protoimpl.TypeBuilder{
		File: protoimpl.DescBuilder{
			GoPackagePath: reflect.TypeOf(x{}).PkgPath(),
			RawDescriptor: unsafe.Slice(unsafe.StringData(file_proxy_loopback_config_proto_rawDesc), len(file_proxy_loopback_config_proto_rawDesc)),
			NumEnums:      0,
			NumMessages:   1,
			NumExtensions: 0,
			NumServices:   0,
		},
		GoTypes:           file_proxy_loopback_config_proto_goTypes,
		DependencyIndexes: file_proxy_loopback_config_proto_depIdxs,
		MessageInfos:      file_proxy_loopback_config_proto_msgTypes,
	}.Build()
	File_proxy_loopback_config_proto = out.File
	file_proxy_loopback_config_proto_goTypes = nil
	file_proxy_loopback_config_proto_depIdxs = nil
}
