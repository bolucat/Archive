package core

import (
	"context"
)

// V2rayKey is the key type of Instance in Context, exported for test.
type v2rayKeyType int

const v2rayKey v2rayKeyType = 1

// FromContext returns an Instance from the given context, or nil if the context doesn't contain one.
func FromContext(ctx context.Context) *Instance {
	if s, ok := ctx.Value(v2rayKey).(*Instance); ok {
		return s
	}
	return nil
}

// MustFromContext returns an Instance from the given context, or panics if not present.
func MustFromContext(ctx context.Context) *Instance {
	v := FromContext(ctx)
	if v == nil {
		panic("V is not in context.")
	}
	return v
}

func WithContext(ctx context.Context, v *Instance) context.Context {
	if FromContext(ctx) != v {
		ctx = context.WithValue(ctx, v2rayKey, v)
	}
	return ctx
}

/*ToBackgroundDetachedContext create a detached context from another context
Internal API
*/
func ToBackgroundDetachedContext(ctx context.Context) context.Context {
	return &temporaryValueDelegationFix{context.Background(), ctx}
}

type temporaryValueDelegationFix struct {
	context.Context
	value context.Context
}

func (t *temporaryValueDelegationFix) Value(key interface{}) interface{} {
	return t.value.Value(key)
}
