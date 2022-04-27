package pipe

import "github.com/v2fly/v2ray-core/v5/common/buf"

type IPipe interface {
	IsPipe() bool
}

func IsPipe(pipe any) bool {
	if p, ok := pipe.(IPipe); ok {
		return p.IsPipe()
	}
	return false
}

type CachedReader interface {
	ReadMultiBufferCached() (buf.MultiBuffer, error)
}
