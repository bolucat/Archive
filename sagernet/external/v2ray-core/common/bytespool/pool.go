package bytespool

import "sync"

// The following parameters controls the size of buffer pools.
// There are numPools pools. Starting from 2k size, the size of each Pool is sizeMulti of the previous one.
// Package buf is guaranteed to not use buffers larger than the largest Pool.
// Other packets may use larger buffers.
const (
	poolSize = 20 * 1024
)

var pool = sync.Pool{
	New: func() any {
		return make([]byte, poolSize)
	},
}

// GetPool returns a sync.Pool that generates bytes array with at least the given size.
// It may return nil if no such Pool exists.
//
// v2ray:api:stable
func GetPool(size int32) *sync.Pool {
	if size <= poolSize {
		return &pool
	}
	return nil
}
