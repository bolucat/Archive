package vmess

import (
	protocol "github.com/v2fly/v2ray-core/v5/common/protocol"
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

type Account struct {
	state protoimpl.MessageState `protogen:"open.v1"`
	// ID of the account, in the form of a UUID, e.g.,
	// "66ad4540-b58c-4ad2-9926-ea63445a9b57".
	Id string `protobuf:"bytes,1,opt,name=id,proto3" json:"id,omitempty"`
	// Number of alternative IDs. Client and server must share the same number.
	AlterId uint32 `protobuf:"varint,2,opt,name=alter_id,json=alterId,proto3" json:"alter_id,omitempty"`
	// Security settings. Only applies to client side.
	SecuritySettings *protocol.SecurityConfig `protobuf:"bytes,3,opt,name=security_settings,json=securitySettings,proto3" json:"security_settings,omitempty"`
	// Define tests enabled for this account
	TestsEnabled  string `protobuf:"bytes,4,opt,name=tests_enabled,json=testsEnabled,proto3" json:"tests_enabled,omitempty"`
	unknownFields protoimpl.UnknownFields
	sizeCache     protoimpl.SizeCache
}

func (x *Account) Reset() {
	*x = Account{}
	mi := &file_proxy_vmess_account_proto_msgTypes[0]
	ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
	ms.StoreMessageInfo(mi)
}

func (x *Account) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*Account) ProtoMessage() {}

func (x *Account) ProtoReflect() protoreflect.Message {
	mi := &file_proxy_vmess_account_proto_msgTypes[0]
	if x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use Account.ProtoReflect.Descriptor instead.
func (*Account) Descriptor() ([]byte, []int) {
	return file_proxy_vmess_account_proto_rawDescGZIP(), []int{0}
}

func (x *Account) GetId() string {
	if x != nil {
		return x.Id
	}
	return ""
}

func (x *Account) GetAlterId() uint32 {
	if x != nil {
		return x.AlterId
	}
	return 0
}

func (x *Account) GetSecuritySettings() *protocol.SecurityConfig {
	if x != nil {
		return x.SecuritySettings
	}
	return nil
}

func (x *Account) GetTestsEnabled() string {
	if x != nil {
		return x.TestsEnabled
	}
	return ""
}

var File_proxy_vmess_account_proto protoreflect.FileDescriptor

const file_proxy_vmess_account_proto_rawDesc = "" +
	"\n" +
	"\x19proxy/vmess/account.proto\x12\x16v2ray.core.proxy.vmess\x1a\x1dcommon/protocol/headers.proto\"\xb2\x01\n" +
	"\aAccount\x12\x0e\n" +
	"\x02id\x18\x01 \x01(\tR\x02id\x12\x19\n" +
	"\balter_id\x18\x02 \x01(\rR\aalterId\x12W\n" +
	"\x11security_settings\x18\x03 \x01(\v2*.v2ray.core.common.protocol.SecurityConfigR\x10securitySettings\x12#\n" +
	"\rtests_enabled\x18\x04 \x01(\tR\ftestsEnabledBc\n" +
	"\x1acom.v2ray.core.proxy.vmessP\x01Z*github.com/v2fly/v2ray-core/v5/proxy/vmess\xaa\x02\x16V2Ray.Core.Proxy.Vmessb\x06proto3"

var (
	file_proxy_vmess_account_proto_rawDescOnce sync.Once
	file_proxy_vmess_account_proto_rawDescData []byte
)

func file_proxy_vmess_account_proto_rawDescGZIP() []byte {
	file_proxy_vmess_account_proto_rawDescOnce.Do(func() {
		file_proxy_vmess_account_proto_rawDescData = protoimpl.X.CompressGZIP(unsafe.Slice(unsafe.StringData(file_proxy_vmess_account_proto_rawDesc), len(file_proxy_vmess_account_proto_rawDesc)))
	})
	return file_proxy_vmess_account_proto_rawDescData
}

var file_proxy_vmess_account_proto_msgTypes = make([]protoimpl.MessageInfo, 1)
var file_proxy_vmess_account_proto_goTypes = []any{
	(*Account)(nil),                 // 0: v2ray.core.proxy.vmess.Account
	(*protocol.SecurityConfig)(nil), // 1: v2ray.core.common.protocol.SecurityConfig
}
var file_proxy_vmess_account_proto_depIdxs = []int32{
	1, // 0: v2ray.core.proxy.vmess.Account.security_settings:type_name -> v2ray.core.common.protocol.SecurityConfig
	1, // [1:1] is the sub-list for method output_type
	1, // [1:1] is the sub-list for method input_type
	1, // [1:1] is the sub-list for extension type_name
	1, // [1:1] is the sub-list for extension extendee
	0, // [0:1] is the sub-list for field type_name
}

func init() { file_proxy_vmess_account_proto_init() }
func file_proxy_vmess_account_proto_init() {
	if File_proxy_vmess_account_proto != nil {
		return
	}
	type x struct{}
	out := protoimpl.TypeBuilder{
		File: protoimpl.DescBuilder{
			GoPackagePath: reflect.TypeOf(x{}).PkgPath(),
			RawDescriptor: unsafe.Slice(unsafe.StringData(file_proxy_vmess_account_proto_rawDesc), len(file_proxy_vmess_account_proto_rawDesc)),
			NumEnums:      0,
			NumMessages:   1,
			NumExtensions: 0,
			NumServices:   0,
		},
		GoTypes:           file_proxy_vmess_account_proto_goTypes,
		DependencyIndexes: file_proxy_vmess_account_proto_depIdxs,
		MessageInfos:      file_proxy_vmess_account_proto_msgTypes,
	}.Build()
	File_proxy_vmess_account_proto = out.File
	file_proxy_vmess_account_proto_goTypes = nil
	file_proxy_vmess_account_proto_depIdxs = nil
}
