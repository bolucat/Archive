package common

import (
	"fmt"
	"os"
	"runtime"
	"runtime/pprof"
	"sync"

	"github.com/enfein/mieru/v3/pkg/stderror"
)

var (
	cpuProfileFile *os.File
	debugMutex     sync.Mutex
)

// GetAllStackTrace returns the stack trace of all goroutines.
func GetAllStackTrace() string {
	debugMutex.Lock()
	defer debugMutex.Unlock()
	buf := make([]byte, 16384)
	for {
		n := runtime.Stack(buf, true)
		if n < len(buf) {
			buf = buf[:n]
			break
		}
		buf = make([]byte, 4*len(buf))
	}
	return string(buf)
}

// GetStackTrace returns the stack trace of this goroutine.
func GetStackTrace() string {
	debugMutex.Lock()
	defer debugMutex.Unlock()
	buf := make([]byte, 4096) // maximum length
	n := runtime.Stack(buf, false)
	if n < len(buf) {
		buf = buf[:n]
	}
	return string(buf)
}

// StartCPUProfile starts CPU profile and writes result to the file.
func StartCPUProfile(filePath string) error {
	debugMutex.Lock()
	defer debugMutex.Unlock()
	if filePath == "" {
		return fmt.Errorf("file path is empty")
	}
	if cpuProfileFile != nil {
		return stderror.ErrAlreadyStarted
	}
	cpuProfileFile, err := os.Create(filePath)
	if err != nil {
		return fmt.Errorf("os.Create() failed: %w", err)
	}
	if err := pprof.StartCPUProfile(cpuProfileFile); err != nil {
		return fmt.Errorf("pprof.StartCPUProfile() failed: %w", err)
	}
	return nil
}

// StopCPUProfile stops CPU profile.
func StopCPUProfile() {
	debugMutex.Lock()
	defer debugMutex.Unlock()
	pprof.StopCPUProfile()
	if cpuProfileFile != nil {
		cpuProfileFile.Close()
	}
	cpuProfileFile = nil
}

// GetHeapProfile generates a heap profile file.
func GetHeapProfile(filePath string) error {
	debugMutex.Lock()
	defer debugMutex.Unlock()
	if filePath == "" {
		return fmt.Errorf("file path is empty")
	}
	heapFile, err := os.Create(filePath)
	if err != nil {
		return fmt.Errorf("os.Create() failed: %w", err)
	}
	defer heapFile.Close()
	runtime.GC()
	if err := pprof.WriteHeapProfile(heapFile); err != nil {
		return fmt.Errorf("pprof.WriteHeapProfile() failed: %w", err)
	}
	return nil
}

// GetMemoryStats runs GC and returns memory statistics.
func GetMemoryStats() *runtime.MemStats {
	debugMutex.Lock()
	defer debugMutex.Unlock()
	runtime.GC()
	ms := &runtime.MemStats{}
	runtime.ReadMemStats(ms)
	return ms
}
