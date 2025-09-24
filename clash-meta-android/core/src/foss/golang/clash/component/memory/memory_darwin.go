package memory

import (
	"unsafe"

	"github.com/ebitengine/purego"
)

const PROC_PIDTASKINFO = 4

type ProcTaskInfo struct {
	Virtual_size      uint64
	Resident_size     uint64
	Total_user        uint64
	Total_system      uint64
	Threads_user      uint64
	Threads_system    uint64
	Policy            int32
	Faults            int32
	Pageins           int32
	Cow_faults        int32
	Messages_sent     int32
	Messages_received int32
	Syscalls_mach     int32
	Syscalls_unix     int32
	Csw               int32
	Threadnum         int32
	Numrunning        int32
	Priority          int32
}

const System = "/usr/lib/libSystem.B.dylib"

type ProcPidInfoFunc func(pid, flavor int32, arg uint64, buffer uintptr, bufferSize int32) int32

const ProcPidInfoSym = "proc_pidinfo"

func GetMemoryInfo(pid int32) (*MemoryInfoStat, error) {
	lib, err := purego.Dlopen(System, purego.RTLD_LAZY|purego.RTLD_GLOBAL)
	if err != nil {
		return nil, err
	}
	defer purego.Dlclose(lib)

	var procPidInfo ProcPidInfoFunc
	purego.RegisterLibFunc(&procPidInfo, lib, ProcPidInfoSym)

	var ti ProcTaskInfo
	procPidInfo(pid, PROC_PIDTASKINFO, 0, uintptr(unsafe.Pointer(&ti)), int32(unsafe.Sizeof(ti)))

	ret := &MemoryInfoStat{
		RSS: uint64(ti.Resident_size),
		VMS: uint64(ti.Virtual_size),
	}
	return ret, nil
}
