//go:build disable_debug

package libcore

type DebugInstance struct{}

func NewDebugInstance() *DebugInstance {
	return new(DebugInstance)
}

func (*DebugInstance) Close() {
}
