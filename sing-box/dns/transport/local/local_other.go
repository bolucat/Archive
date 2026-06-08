//go:build !darwin

package local

func newSystemResolver() systemResolver {
	return nil
}
