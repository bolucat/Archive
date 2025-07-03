package dispatcher

import (
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

type SessionConfig struct {
	state         protoimpl.MessageState `protogen:"open.v1"`
	unknownFields protoimpl.UnknownFields
	sizeCache     protoimpl.SizeCache
}

func (x *SessionConfig) Reset() {
	*x = SessionConfig{}
	mi := &file_app_dispatcher_config_proto_msgTypes[0]
	ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
	ms.StoreMessageInfo(mi)
}

func (x *SessionConfig) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*SessionConfig) ProtoMessage() {}

func (x *SessionConfig) ProtoReflect() protoreflect.Message {
	mi := &file_app_dispatcher_config_proto_msgTypes[0]
	if x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use SessionConfig.ProtoReflect.Descriptor instead.
func (*SessionConfig) Descriptor() ([]byte, []int) {
	return file_app_dispatcher_config_proto_rawDescGZIP(), []int{0}
}

type Config struct {
	state         protoimpl.MessageState `protogen:"open.v1"`
	Settings      *SessionConfig         `protobuf:"bytes,1,opt,name=settings,proto3" json:"settings,omitempty"`
	unknownFields protoimpl.UnknownFields
	sizeCache     protoimpl.SizeCache
}

func (x *Config) Reset() {
	*x = Config{}
	mi := &file_app_dispatcher_config_proto_msgTypes[1]
	ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
	ms.StoreMessageInfo(mi)
}

func (x *Config) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*Config) ProtoMessage() {}

func (x *Config) ProtoReflect() protoreflect.Message {
	mi := &file_app_dispatcher_config_proto_msgTypes[1]
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
	return file_app_dispatcher_config_proto_rawDescGZIP(), []int{1}
}

func (x *Config) GetSettings() *SessionConfig {
	if x != nil {
		return x.Settings
	}
	return nil
}

var File_app_dispatcher_config_proto protoreflect.FileDescriptor

const file_app_dispatcher_config_proto_rawDesc = "" +
	"\n" +
	"\x1bapp/dispatcher/config.proto\x12\x19v2ray.core.app.dispatcher\"\x15\n" +
	"\rSessionConfigJ\x04\b\x01\x10\x02\"N\n" +
	"\x06Config\x12D\n" +
	"\bsettings\x18\x01 \x01(\v2(.v2ray.core.app.dispatcher.SessionConfigR\bsettingsBl\n" +
	"\x1dcom.v2ray.core.app.dispatcherP\x01Z-github.com/v2fly/v2ray-core/v5/app/dispatcher\xaa\x02\x19V2Ray.Core.App.Dispatcherb\x06proto3"

var (
	file_app_dispatcher_config_proto_rawDescOnce sync.Once
	file_app_dispatcher_config_proto_rawDescData []byte
)

func file_app_dispatcher_config_proto_rawDescGZIP() []byte {
	file_app_dispatcher_config_proto_rawDescOnce.Do(func() {
		file_app_dispatcher_config_proto_rawDescData = protoimpl.X.CompressGZIP(unsafe.Slice(unsafe.StringData(file_app_dispatcher_config_proto_rawDesc), len(file_app_dispatcher_config_proto_rawDesc)))
	})
	return file_app_dispatcher_config_proto_rawDescData
}

var file_app_dispatcher_config_proto_msgTypes = make([]protoimpl.MessageInfo, 2)
var file_app_dispatcher_config_proto_goTypes = []any{
	(*SessionConfig)(nil), // 0: v2ray.core.app.dispatcher.SessionConfig
	(*Config)(nil),        // 1: v2ray.core.app.dispatcher.Config
}
var file_app_dispatcher_config_proto_depIdxs = []int32{
	0, // 0: v2ray.core.app.dispatcher.Config.settings:type_name -> v2ray.core.app.dispatcher.SessionConfig
	1, // [1:1] is the sub-list for method output_type
	1, // [1:1] is the sub-list for method input_type
	1, // [1:1] is the sub-list for extension type_name
	1, // [1:1] is the sub-list for extension extendee
	0, // [0:1] is the sub-list for field type_name
}

func init() { file_app_dispatcher_config_proto_init() }
func file_app_dispatcher_config_proto_init() {
	if File_app_dispatcher_config_proto != nil {
		return
	}
	type x struct{}
	out := protoimpl.TypeBuilder{
		File: protoimpl.DescBuilder{
			GoPackagePath: reflect.TypeOf(x{}).PkgPath(),
			RawDescriptor: unsafe.Slice(unsafe.StringData(file_app_dispatcher_config_proto_rawDesc), len(file_app_dispatcher_config_proto_rawDesc)),
			NumEnums:      0,
			NumMessages:   2,
			NumExtensions: 0,
			NumServices:   0,
		},
		GoTypes:           file_app_dispatcher_config_proto_goTypes,
		DependencyIndexes: file_app_dispatcher_config_proto_depIdxs,
		MessageInfos:      file_app_dispatcher_config_proto_msgTypes,
	}.Build()
	File_app_dispatcher_config_proto = out.File
	file_app_dispatcher_config_proto_goTypes = nil
	file_app_dispatcher_config_proto_depIdxs = nil
}
