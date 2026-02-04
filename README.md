## bb (Benowin Blanc)

> **Benowin Blanc:** Windows through a detective's lens...

Hm... today I want to see the layout of `PEB`.

<details>

```powershell
> target\debug\bb.exe --struct _PEB --depth 3

_PEB winternl.h:178:16
├─ +0x000 [  2] Reserved1  BYTE[2]
├─ +0x002 [  1] BeingDebugged  BYTE
├─ +0x003 [  1] Reserved2  BYTE[1]
├─ +0x008 [ 16] Reserved3  PVOID[2]
├─ +0x018 [  8] Ldr  PPEB_LDR_DATA
│  ├─ +0x000 [  8] Reserved1  BYTE[8]
│  ├─ +0x008 [ 24] Reserved2  PVOID[3]
│  ╰─ +0x020 [ 16] InMemoryOrderModuleList  LIST_ENTRY
│     ├─ +0x000 [  8] Flink  struct _LIST_ENTRY *
│     ╰─ +0x008 [  8] Blink  struct _LIST_ENTRY *
├─ +0x020 [  8] ProcessParameters  PRTL_USER_PROCESS_PARAMETERS
│  ├─ +0x000 [ 16] Reserved1  BYTE[16]
│  ├─ +0x010 [ 80] Reserved2  PVOID[10]
│  ├─ +0x060 [ 16] ImagePathName  UNICODE_STRING
│  │  ├─ +0x000 [  2] Length  USHORT
│  │  ├─ +0x002 [  2] MaximumLength  USHORT
│  │  ╰─ +0x008 [  8] Buffer  PWSTR
│  ╰─ +0x070 [ 16] CommandLine  UNICODE_STRING
│     ├─ +0x000 [  2] Length  USHORT
│     ├─ +0x002 [  2] MaximumLength  USHORT
│     ╰─ +0x008 [  8] Buffer  PWSTR
├─ +0x028 [ 24] Reserved4  PVOID[3]
├─ +0x040 [  8] AtlThunkSListPtr  PVOID
├─ +0x048 [  8] Reserved5  PVOID
├─ +0x050 [  4] Reserved6  ULONG
├─ +0x058 [  8] Reserved7  PVOID
├─ +0x060 [  4] Reserved8  ULONG
├─ +0x064 [  4] AtlThunkSListPtr32  ULONG
├─ +0x068 [360] Reserved9  PVOID[45]
├─ +0x1d0 [ 96] Reserved10  BYTE[96]
├─ +0x230 [  8] PostProcessInitRoutine  PPS_POST_PROCESS_INIT_ROUTINE
├─ +0x238 [128] Reserved11  BYTE[128]
├─ +0x2b8 [  8] Reserved12  PVOID[1]
╰─ +0x2c0 [  4] SessionId  ULONG
╰─ 712 bytes
```

Ok... ok, great! It even expanded the structures inline for me! But what is all this `Reserved` junk? I want to see... the real `PEB`!


```powershell
target\debug\bb.exe --phnt --struct _PEB --depth 3

_PEB __bb_phnt_synthetic.h:14347:20
├─ +0x000 [  1] InheritedAddressSpace  BOOLEAN
├─ +0x001 [  1] ReadImageFileExecOptions  BOOLEAN
├─ +0x002 [  1] BeingDebugged  BOOLEAN
├─ +0x008 [  8] Mutant  HANDLE
├─ +0x010 [  8] ImageBaseAddress  PVOID
├─ +0x018 [  8] Ldr  PPEB_LDR_DATA
│  ├─ +0x000 [  4] Length  ULONG
│  ├─ +0x004 [  1] Initialized  BOOLEAN
│  ├─ +0x008 [  8] SsHandle  HANDLE
│  ├─ +0x010 [ 16] InLoadOrderModuleList  LIST_ENTRY
│  │  ├─ +0x000 [  8] Flink  struct _LIST_ENTRY *
│  │  ╰─ +0x008 [  8] Blink  struct _LIST_ENTRY *
│  ├─ +0x020 [ 16] InMemoryOrderModuleList  LIST_ENTRY
│  │  ├─ +0x000 [  8] Flink  struct _LIST_ENTRY *
│  │  ╰─ +0x008 [  8] Blink  struct _LIST_ENTRY *
│  ├─ +0x030 [ 16] InInitializationOrderModuleList  LIST_ENTRY
│  │  ├─ +0x000 [  8] Flink  struct _LIST_ENTRY *
│  │  ╰─ +0x008 [  8] Blink  struct _LIST_ENTRY *
│  ├─ +0x040 [  8] EntryInProgress  PVOID
│  ├─ +0x048 [  1] ShutdownInProgress  BOOLEAN
│  ╰─ +0x050 [  8] ShutdownThreadId  HANDLE
├─ +0x020 [  8] ProcessParameters  PRTL_USER_PROCESS_PARAMETERS
│  ├─ +0x000 [  4] MaximumLength  ULONG
│  ├─ +0x004 [  4] Length  ULONG
│  ├─ +0x008 [  4] Flags  ULONG
│  ├─ +0x00c [  4] DebugFlags  ULONG
│  ├─ +0x010 [  8] ConsoleHandle  HANDLE
│  ├─ +0x018 [  4] ConsoleFlags  ULONG
│  ├─ +0x020 [  8] StandardInput  HANDLE
│  ├─ +0x028 [  8] StandardOutput  HANDLE
│  ├─ +0x030 [  8] StandardError  HANDLE
│  ├─ +0x038 [ 24] CurrentDirectory  CURDIR
│  │  ├─ +0x000 [ 16] DosPath  UNICODE_STRING
│  │  │  ├─ +0x000 [  2] Length  USHORT
│  │  │  ├─ +0x002 [  2] MaximumLength  USHORT
│  │  │  ╰─ +0x008 [  8] Buffer  PWCH
│  │  ╰─ +0x010 [  8] Handle  HANDLE
│  ├─ +0x050 [ 16] DllPath  UNICODE_STRING
│  │  ├─ +0x000 [  2] Length  USHORT
│  │  ├─ +0x002 [  2] MaximumLength  USHORT
│  │  ╰─ +0x008 [  8] Buffer  PWCH
│  ├─ +0x060 [ 16] ImagePathName  UNICODE_STRING
│  │  ├─ +0x000 [  2] Length  USHORT
│  │  ├─ +0x002 [  2] MaximumLength  USHORT
│  │  ╰─ +0x008 [  8] Buffer  PWCH
│  ├─ +0x070 [ 16] CommandLine  UNICODE_STRING
│  │  ├─ +0x000 [  2] Length  USHORT
│  │  ├─ +0x002 [  2] MaximumLength  USHORT
│  │  ╰─ +0x008 [  8] Buffer  PWCH
│  ├─ +0x080 [  8] Environment  PVOID
│  ├─ +0x088 [  4] StartingX  ULONG
│  ├─ +0x08c [  4] StartingY  ULONG
│  ├─ +0x090 [  4] CountX  ULONG
│  ├─ +0x094 [  4] CountY  ULONG
│  ├─ +0x098 [  4] CountCharsX  ULONG
│  ├─ +0x09c [  4] CountCharsY  ULONG
│  ├─ +0x0a0 [  4] FillAttribute  ULONG
│  ├─ +0x0a4 [  4] WindowFlags  ULONG
│  ├─ +0x0a8 [  4] ShowWindowFlags  ULONG
│  ├─ +0x0b0 [ 16] WindowTitle  UNICODE_STRING
│  │  ├─ +0x000 [  2] Length  USHORT
│  │  ├─ +0x002 [  2] MaximumLength  USHORT
│  │  ╰─ +0x008 [  8] Buffer  PWCH
│  ├─ +0x0c0 [ 16] DesktopInfo  UNICODE_STRING
│  │  ├─ +0x000 [  2] Length  USHORT
│  │  ├─ +0x002 [  2] MaximumLength  USHORT
│  │  ╰─ +0x008 [  8] Buffer  PWCH
│  ├─ +0x0d0 [ 16] ShellInfo  UNICODE_STRING
│  │  ├─ +0x000 [  2] Length  USHORT
│  │  ├─ +0x002 [  2] MaximumLength  USHORT
│  │  ╰─ +0x008 [  8] Buffer  PWCH
│  ├─ +0x0e0 [ 16] RuntimeData  UNICODE_STRING
│  │  ├─ +0x000 [  2] Length  USHORT
│  │  ├─ +0x002 [  2] MaximumLength  USHORT
│  │  ╰─ +0x008 [  8] Buffer  PWCH
│  ├─ +0x0f0 [768] CurrentDirectories  RTL_DRIVE_LETTER_CURDIR[32]
│  │  ├─ +0x000 [  2] Flags  USHORT
│  │  ├─ +0x002 [  2] Length  USHORT
│  │  ├─ +0x004 [  4] TimeStamp  ULONG
│  │  ╰─ +0x008 [ 16] DosPath  STRING
│  │     ├─ +0x000 [  2] Length  USHORT
│  │     ├─ +0x002 [  2] MaximumLength  USHORT
│  │     ╰─ +0x008 [  8] Buffer  PCHAR
│  ├─ +0x3f0 [  8] EnvironmentSize  ULONG_PTR
│  ├─ +0x3f8 [  8] EnvironmentVersion  ULONG_PTR
│  ├─ +0x400 [  8] PackageDependencyData  PVOID
│  ├─ +0x408 [  4] ProcessGroupId  ULONG
│  ├─ +0x40c [  4] LoaderThreads  ULONG
│  ├─ +0x410 [ 16] RedirectionDllName  UNICODE_STRING
│  │  ├─ +0x000 [  2] Length  USHORT
│  │  ├─ +0x002 [  2] MaximumLength  USHORT
│  │  ╰─ +0x008 [  8] Buffer  PWCH
│  ├─ +0x420 [ 16] HeapPartitionName  UNICODE_STRING
│  │  ├─ +0x000 [  2] Length  USHORT
│  │  ├─ +0x002 [  2] MaximumLength  USHORT
│  │  ╰─ +0x008 [  8] Buffer  PWCH
│  ├─ +0x430 [  8] DefaultThreadpoolCpuSetMasks  PULONGLONG
│  ├─ +0x438 [  4] DefaultThreadpoolCpuSetMaskCount  ULONG
│  ├─ +0x43c [  4] DefaultThreadpoolThreadMaximum  ULONG
│  ╰─ +0x440 [  4] HeapMemoryTypeMask  ULONG
├─ +0x028 [  8] SubSystemData  PVOID
├─ +0x030 [  8] ProcessHeap  PVOID
├─ +0x038 [  8] FastPebLock  PRTL_CRITICAL_SECTION
│  ├─ +0x000 [  8] DebugInfo  PRTL_CRITICAL_SECTION_DEBUG
│  │  ├─ +0x000 [  2] Type  WORD
│  │  ├─ +0x002 [  2] CreatorBackTraceIndex  WORD
│  │  ├─ +0x008 [  8] CriticalSection  struct _RTL_CRITICAL_SECTION *
│  │  ├─ +0x010 [ 16] ProcessLocksList  LIST_ENTRY
│  │  │  ├─ +0x000 [  8] Flink  struct _LIST_ENTRY *
│  │  │  ╰─ +0x008 [  8] Blink  struct _LIST_ENTRY *
│  │  ├─ +0x020 [  4] EntryCount  DWORD
│  │  ├─ +0x024 [  4] ContentionCount  DWORD
│  │  ├─ +0x028 [  4] Flags  DWORD
│  │  ├─ +0x02c [  2] CreatorBackTraceIndexHigh  WORD
│  │  ╰─ +0x02e [  2] Identifier  WORD
│  ├─ +0x008 [  4] LockCount  LONG
│  ├─ +0x00c [  4] RecursionCount  LONG
│  ├─ +0x010 [  8] OwningThread  HANDLE
│  ├─ +0x018 [  8] LockSemaphore  HANDLE
│  ╰─ +0x020 [  8] SpinCount  ULONG_PTR
├─ +0x040 [  8] AtlThunkSListPtr  PSLIST_HEADER
│  ╰─ +0x000 [ 16] HeaderX64  <anonymous "struct">
│     ├─ +0x000 [  8] Depth  ULONGLONG
│     ├─ +0x002 [  8] Sequence  ULONGLONG
│     ├─ +0x008 [  8] Reserved  ULONGLONG
│     ╰─ +0x008 [  8] NextEntry  ULONGLONG
├─ +0x048 [  8] IFEOKey  PVOID
├─ +0x060 [  4] SystemReserved  ULONG
├─ +0x064 [  4] AtlThunkSListPtr32  ULONG
├─ +0x068 [  8] ApiSetMap  PAPI_SET_NAMESPACE
│  ├─ +0x000 [  4] Version  ULONG
│  ├─ +0x004 [  4] Size  ULONG
│  ├─ +0x008 [  4] Flags  ULONG
│  ├─ +0x00c [  4] Count  ULONG
│  ├─ +0x010 [  4] EntryOffset  ULONG
│  ├─ +0x014 [  4] HashOffset  ULONG
│  ╰─ +0x018 [  4] HashFactor  ULONG
├─ +0x070 [  4] TlsExpansionCounter  ULONG
├─ +0x078 [  8] TlsBitmap  PRTL_BITMAP
│  ├─ +0x000 [  4] SizeOfBitMap  ULONG
│  ╰─ +0x008 [  8] Buffer  PULONG
├─ +0x080 [  8] TlsBitmapBits  ULONG[2]
├─ +0x088 [  8] ReadOnlySharedMemoryBase  PVOID
├─ +0x090 [  8] SharedData  PSILO_USER_SHARED_DATA
│  ├─ +0x000 [  4] ServiceSessionId  ULONG
│  ├─ +0x004 [  4] ActiveConsoleId  ULONG
│  ├─ +0x008 [  8] ConsoleSessionForegroundProcessId  LONGLONG
│  ├─ +0x010 [  4] NtProductType  NT_PRODUCT_TYPE
│  ├─ +0x014 [  4] SuiteMask  ULONG
│  ├─ +0x018 [  4] SharedUserSessionId  ULONG
│  ├─ +0x01c [  1] IsMultiSessionSku  BOOLEAN
│  ├─ +0x01d [  1] IsStateSeparationEnabled  BOOLEAN
│  ├─ +0x01e [520] NtSystemRoot  WCHAR[260]
│  ├─ +0x226 [ 32] UserModeGlobalLogger  USHORT[16]
│  ├─ +0x248 [  4] TimeZoneId  ULONG
│  ├─ +0x24c [  4] TimeZoneBiasStamp  LONG
│  ├─ +0x250 [ 12] TimeZoneBias  KSYSTEM_TIME
│  │  ├─ +0x000 [  4] LowPart  ULONG
│  │  ├─ +0x004 [  4] High1Time  LONG
│  │  ╰─ +0x008 [  4] High2Time  LONG
│  ├─ +0x260 [  8] TimeZoneBiasEffectiveStart  LARGE_INTEGER
│  │  ├─ +0x000 [  8] u  <anonymous "struct">
│  │  │  ├─ +0x000 [  4] LowPart  DWORD
│  │  │  ╰─ +0x004 [  4] HighPart  LONG
│  │  ╰─ +0x000 [  8] QuadPart  LONGLONG
│  ╰─ +0x268 [  8] TimeZoneBiasEffectiveEnd  LARGE_INTEGER
│     ├─ +0x000 [  8] u  <anonymous "struct">
│     │  ├─ +0x000 [  4] LowPart  DWORD
│     │  ╰─ +0x004 [  4] HighPart  LONG
│     ╰─ +0x000 [  8] QuadPart  LONGLONG
├─ +0x098 [  8] ReadOnlyStaticServerData  PVOID *
├─ +0x0a0 [  8] AnsiCodePageData  PVOID
├─ +0x0a8 [  8] OemCodePageData  PVOID
├─ +0x0b0 [  8] UnicodeCaseTableData  PVOID
├─ +0x0b8 [  4] NumberOfProcessors  ULONG
├─ +0x0bc [  4] NtGlobalFlag  ULONG
├─ +0x0c0 [  8] CriticalSectionTimeout  LARGE_INTEGER
│  ├─ +0x000 [  8] u  <anonymous "struct">
│  │  ├─ +0x000 [  4] LowPart  DWORD
│  │  ╰─ +0x004 [  4] HighPart  LONG
│  ╰─ +0x000 [  8] QuadPart  LONGLONG
├─ +0x0c8 [  8] HeapSegmentReserve  SIZE_T
├─ +0x0d0 [  8] HeapSegmentCommit  SIZE_T
├─ +0x0d8 [  8] HeapDeCommitTotalFreeThreshold  SIZE_T
├─ +0x0e0 [  8] HeapDeCommitFreeBlockThreshold  SIZE_T
├─ +0x0e8 [  4] NumberOfHeaps  ULONG
├─ +0x0ec [  4] MaximumNumberOfHeaps  ULONG
├─ +0x0f0 [  8] ProcessHeaps  PVOID *
├─ +0x0f8 [  8] GdiSharedHandleTable  PVOID
├─ +0x100 [  8] ProcessStarterHelper  PVOID
├─ +0x108 [  4] GdiDCAttributeList  ULONG
├─ +0x110 [  8] LoaderLock  PRTL_CRITICAL_SECTION
│  ├─ +0x000 [  8] DebugInfo  PRTL_CRITICAL_SECTION_DEBUG
│  │  ├─ +0x000 [  2] Type  WORD
│  │  ├─ +0x002 [  2] CreatorBackTraceIndex  WORD
│  │  ├─ +0x008 [  8] CriticalSection  struct _RTL_CRITICAL_SECTION *
│  │  ├─ +0x010 [ 16] ProcessLocksList  LIST_ENTRY
│  │  │  ├─ +0x000 [  8] Flink  struct _LIST_ENTRY *
│  │  │  ╰─ +0x008 [  8] Blink  struct _LIST_ENTRY *
│  │  ├─ +0x020 [  4] EntryCount  DWORD
│  │  ├─ +0x024 [  4] ContentionCount  DWORD
│  │  ├─ +0x028 [  4] Flags  DWORD
│  │  ├─ +0x02c [  2] CreatorBackTraceIndexHigh  WORD
│  │  ╰─ +0x02e [  2] Identifier  WORD
│  ├─ +0x008 [  4] LockCount  LONG
│  ├─ +0x00c [  4] RecursionCount  LONG
│  ├─ +0x010 [  8] OwningThread  HANDLE
│  ├─ +0x018 [  8] LockSemaphore  HANDLE
│  ╰─ +0x020 [  8] SpinCount  ULONG_PTR
├─ +0x118 [  4] OSMajorVersion  ULONG
├─ +0x11c [  4] OSMinorVersion  ULONG
├─ +0x120 [  2] OSBuildNumber  USHORT
├─ +0x122 [  2] OSCSDVersion  USHORT
├─ +0x124 [  4] OSPlatformId  ULONG
├─ +0x128 [  4] ImageSubsystem  ULONG
├─ +0x12c [  4] ImageSubsystemMajorVersion  ULONG
├─ +0x130 [  4] ImageSubsystemMinorVersion  ULONG
├─ +0x138 [  8] ActiveProcessAffinityMask  KAFFINITY
├─ +0x140 [240] GdiHandleBuffer  GDI_HANDLE_BUFFER
├─ +0x230 [  8] PostProcessInitRoutine  PPS_POST_PROCESS_INIT_ROUTINE
├─ +0x238 [  8] TlsExpansionBitmap  PRTL_BITMAP
│  ├─ +0x000 [  4] SizeOfBitMap  ULONG
│  ╰─ +0x008 [  8] Buffer  PULONG
├─ +0x240 [128] TlsExpansionBitmapBits  ULONG[32]
├─ +0x2c0 [  4] SessionId  ULONG
├─ +0x2c8 [  8] AppCompatFlags  ULARGE_INTEGER
│  ├─ +0x000 [  8] u  <anonymous "struct">
│  │  ├─ +0x000 [  4] LowPart  DWORD
│  │  ╰─ +0x004 [  4] HighPart  DWORD
│  ╰─ +0x000 [  8] QuadPart  ULONGLONG
├─ +0x2d0 [  8] AppCompatFlagsUser  ULARGE_INTEGER
│  ├─ +0x000 [  8] u  <anonymous "struct">
│  │  ├─ +0x000 [  4] LowPart  DWORD
│  │  ╰─ +0x004 [  4] HighPart  DWORD
│  ╰─ +0x000 [  8] QuadPart  ULONGLONG
├─ +0x2d8 [  8] pShimData  PVOID
├─ +0x2e0 [  8] AppCompatInfo  PVOID
├─ +0x2e8 [ 16] CSDVersion  UNICODE_STRING
│  ├─ +0x000 [  2] Length  USHORT
│  ├─ +0x002 [  2] MaximumLength  USHORT
│  ╰─ +0x008 [  8] Buffer  PWCH
├─ +0x2f8 [  8] ActivationContextData  PACTIVATION_CONTEXT_DATA
│  ├─ +0x000 [  4] Magic  ULONG
│  ├─ +0x004 [  4] HeaderSize  ULONG
│  ├─ +0x008 [  4] FormatVersion  ULONG
│  ├─ +0x00c [  4] TotalSize  ULONG
│  ├─ +0x010 [  4] DefaultTocOffset  ULONG
│  ├─ +0x014 [  4] ExtendedTocOffset  ULONG
│  ├─ +0x018 [  4] AssemblyRosterOffset  ULONG
│  ╰─ +0x01c [  4] Flags  ULONG
├─ +0x300 [  8] ProcessAssemblyStorageMap  PASSEMBLY_STORAGE_MAP
│  ├─ +0x000 [  4] Flags  ULONG
│  ├─ +0x004 [  4] AssemblyCount  ULONG
│  ╰─ +0x008 [  8] AssemblyArray  PASSEMBLY_STORAGE_MAP_ENTRY *
├─ +0x308 [  8] SystemDefaultActivationContextData  PACTIVATION_CONTEXT_DATA
│  ├─ +0x000 [  4] Magic  ULONG
│  ├─ +0x004 [  4] HeaderSize  ULONG
│  ├─ +0x008 [  4] FormatVersion  ULONG
│  ├─ +0x00c [  4] TotalSize  ULONG
│  ├─ +0x010 [  4] DefaultTocOffset  ULONG
│  ├─ +0x014 [  4] ExtendedTocOffset  ULONG
│  ├─ +0x018 [  4] AssemblyRosterOffset  ULONG
│  ╰─ +0x01c [  4] Flags  ULONG
├─ +0x310 [  8] SystemAssemblyStorageMap  PASSEMBLY_STORAGE_MAP
│  ├─ +0x000 [  4] Flags  ULONG
│  ├─ +0x004 [  4] AssemblyCount  ULONG
│  ╰─ +0x008 [  8] AssemblyArray  PASSEMBLY_STORAGE_MAP_ENTRY *
├─ +0x318 [  8] MinimumStackCommit  SIZE_T
├─ +0x320 [ 16] SparePointers  PVOID[2]
├─ +0x330 [  8] PatchLoaderData  PVOID
├─ +0x338 [  8] ChpeV2ProcessInfo  PVOID
├─ +0x344 [  8] SpareUlongs  ULONG[2]
├─ +0x34c [  2] ActiveCodePage  USHORT
├─ +0x34e [  2] OemCodePage  USHORT
├─ +0x350 [  2] UseCaseMapping  USHORT
├─ +0x352 [  2] UnusedNlsField  USHORT
├─ +0x358 [  8] WerRegistrationData  PWER_PEB_HEADER_BLOCK
│  ├─ +0x000 [  4] Length  LONG
│  ├─ +0x004 [ 32] Signature  WCHAR[16]
│  ├─ +0x024 [128] AppDataRelativePath  WCHAR[64]
│  ├─ +0x0a4 [2048] RestartCommandLine  WCHAR[1024]
│  ├─ +0x8a8 [ 64] RecoveryInfo  WER_RECOVERY_INFO
│  │  ├─ +0x000 [  4] Length  ULONG
│  │  ├─ +0x008 [  8] Callback  PVOID
│  │  ├─ +0x010 [  8] Parameter  PVOID
│  │  ├─ +0x018 [  8] Started  HANDLE
│  │  ├─ +0x020 [  8] Finished  HANDLE
│  │  ├─ +0x028 [  8] InProgress  HANDLE
│  │  ├─ +0x030 [  4] LastError  LONG
│  │  ├─ +0x034 [  4] Successful  BOOL
│  │  ├─ +0x038 [  4] PingInterval  ULONG
│  │  ╰─ +0x03c [  4] Flags  ULONG
│  ├─ +0x8e8 [  8] Gather  PWER_GATHER
│  │  ├─ +0x000 [  8] Next  PVOID
│  │  ├─ +0x008 [  2] Flags  USHORT
│  │  ╰─ +0x010 [528] v  <anonymous "union">
│  │     ├─ +0x000 [522] File  WER_FILE
│  │     ╰─ +0x000 [ 16] Memory  WER_MEMORY
│  ├─ +0x8f0 [  8] MetaData  PWER_METADATA
│  │  ├─ +0x000 [  8] Next  PVOID
│  │  ├─ +0x008 [128] Key  WCHAR[64]
│  │  ╰─ +0x088 [256] Value  WCHAR[128]
│  ├─ +0x8f8 [  8] RuntimeDll  PWER_RUNTIME_DLL
│  │  ├─ +0x000 [  8] Next  PVOID
│  │  ├─ +0x008 [  4] Length  ULONG
│  │  ├─ +0x010 [  8] Context  PVOID
│  │  ╰─ +0x018 [520] CallbackDllPath  WCHAR[260]
│  ├─ +0x900 [  8] DumpCollection  PWER_DUMP_COLLECTION
│  │  ├─ +0x000 [  8] Next  PVOID
│  │  ├─ +0x008 [  4] ProcessId  ULONG
│  │  ╰─ +0x00c [  4] ThreadId  ULONG
│  ├─ +0x908 [  4] GatherCount  LONG
│  ├─ +0x90c [  4] MetaDataCount  LONG
│  ├─ +0x910 [  4] DumpCount  LONG
│  ├─ +0x914 [  4] Flags  LONG
│  ├─ +0x918 [ 72] MainHeader  WER_HEAP_MAIN_HEADER
│  │  ├─ +0x000 [ 32] Signature  WCHAR[16]
│  │  ├─ +0x020 [ 16] Links  LIST_ENTRY
│  │  │  ├─ +0x000 [  8] Flink  struct _LIST_ENTRY *
│  │  │  ╰─ +0x008 [  8] Blink  struct _LIST_ENTRY *
│  │  ├─ +0x030 [  8] Mutex  HANDLE
│  │  ├─ +0x038 [  8] FreeHeap  PVOID
│  │  ╰─ +0x040 [  4] FreeCount  ULONG
│  ╰─ +0x960 [  8] Reserved  PVOID
├─ +0x360 [  8] WerShipAssertPtr  PVOID
├─ +0x370 [  8] pImageHeaderHash  PVOID
├─ +0x380 [  8] CsrServerReadOnlySharedMemoryBase  ULONGLONG
├─ +0x388 [  8] TppWorkerpListLock  PRTL_CRITICAL_SECTION
│  ├─ +0x000 [  8] DebugInfo  PRTL_CRITICAL_SECTION_DEBUG
│  │  ├─ +0x000 [  2] Type  WORD
│  │  ├─ +0x002 [  2] CreatorBackTraceIndex  WORD
│  │  ├─ +0x008 [  8] CriticalSection  struct _RTL_CRITICAL_SECTION *
│  │  ├─ +0x010 [ 16] ProcessLocksList  LIST_ENTRY
│  │  │  ├─ +0x000 [  8] Flink  struct _LIST_ENTRY *
│  │  │  ╰─ +0x008 [  8] Blink  struct _LIST_ENTRY *
│  │  ├─ +0x020 [  4] EntryCount  DWORD
│  │  ├─ +0x024 [  4] ContentionCount  DWORD
│  │  ├─ +0x028 [  4] Flags  DWORD
│  │  ├─ +0x02c [  2] CreatorBackTraceIndexHigh  WORD
│  │  ╰─ +0x02e [  2] Identifier  WORD
│  ├─ +0x008 [  4] LockCount  LONG
│  ├─ +0x00c [  4] RecursionCount  LONG
│  ├─ +0x010 [  8] OwningThread  HANDLE
│  ├─ +0x018 [  8] LockSemaphore  HANDLE
│  ╰─ +0x020 [  8] SpinCount  ULONG_PTR
├─ +0x390 [ 16] TppWorkerpList  LIST_ENTRY
│  ├─ +0x000 [  8] Flink  struct _LIST_ENTRY *
│  ╰─ +0x008 [  8] Blink  struct _LIST_ENTRY *
├─ +0x3a0 [1024] WaitOnAddressHashTable  PVOID[128]
├─ +0x7a0 [  8] TelemetryCoverageHeader  PTELEMETRY_COVERAGE_HEADER
│  ├─ +0x000 [  1] MajorVersion  UCHAR
│  ├─ +0x001 [  1] MinorVersion  UCHAR
│  ├─ +0x004 [  4] HashTableEntries  ULONG
│  ├─ +0x008 [  4] HashIndexMask  ULONG
│  ├─ +0x00c [  4] TableUpdateVersion  ULONG
│  ├─ +0x010 [  4] TableSizeInBytes  ULONG
│  ├─ +0x014 [  4] LastResetTick  ULONG
│  ├─ +0x018 [  4] ResetRound  ULONG
│  ├─ +0x01c [  4] Reserved2  ULONG
│  ├─ +0x020 [  4] RecordedCount  ULONG
│  ├─ +0x024 [ 16] Reserved3  ULONG[4]
│  ╰─ +0x034 [  4] HashTable  ULONG[1]
├─ +0x7a8 [  4] CloudFileFlags  ULONG
├─ +0x7ac [  4] CloudFileDiagFlags  ULONG
├─ +0x7b0 [  1] PlaceholderCompatibilityMode  CHAR
├─ +0x7b1 [  7] PlaceholderCompatibilityModeReserved  CHAR[7]
├─ +0x7b8 [  8] LeapSecondData  PLEAP_SECOND_DATA
├─ +0x7c4 [  4] NtGlobalFlag2  ULONG
╰─ +0x7c8 [  8] ExtendedFeatureDisableMask  ULONGLONG
╰─ 2000 bytes
```

Well, this is... glorious! Or it would be, if it weren't for the fact that it's ***so long!*** I just care about the general layout of `PEB::ProcessParameters`! ...or was it `ProcessParameters`?

```powershell
> target\debug\bb.exe --phnt --struct _PEB --field *proc*param* --depth 1

_PEB __bb_phnt_synthetic.h:14347:20
╰─ +0x020 [  8] ProcessParameters  PRTL_USER_PROCESS_PARAMETERS
   ├─ +0x000 [  4] MaximumLength  ULONG
   ├─ +0x004 [  4] Length  ULONG
   ├─ +0x008 [  4] Flags  ULONG
   ├─ +0x00c [  4] DebugFlags  ULONG
   ├─ +0x010 [  8] ConsoleHandle  HANDLE
   ├─ +0x018 [  4] ConsoleFlags  ULONG
   ├─ +0x020 [  8] StandardInput  HANDLE
   ├─ +0x028 [  8] StandardOutput  HANDLE
   ├─ +0x030 [  8] StandardError  HANDLE
   ├─ +0x038 [ 24] CurrentDirectory  CURDIR
   ├─ +0x050 [ 16] DllPath  UNICODE_STRING
   ├─ +0x060 [ 16] ImagePathName  UNICODE_STRING
   ├─ +0x070 [ 16] CommandLine  UNICODE_STRING
   ├─ +0x080 [  8] Environment  PVOID
   ├─ +0x088 [  4] StartingX  ULONG
   ├─ +0x08c [  4] StartingY  ULONG
   ├─ +0x090 [  4] CountX  ULONG
   ├─ +0x094 [  4] CountY  ULONG
   ├─ +0x098 [  4] CountCharsX  ULONG
   ├─ +0x09c [  4] CountCharsY  ULONG
   ├─ +0x0a0 [  4] FillAttribute  ULONG
   ├─ +0x0a4 [  4] WindowFlags  ULONG
   ├─ +0x0a8 [  4] ShowWindowFlags  ULONG
   ├─ +0x0b0 [ 16] WindowTitle  UNICODE_STRING
   ├─ +0x0c0 [ 16] DesktopInfo  UNICODE_STRING
   ├─ +0x0d0 [ 16] ShellInfo  UNICODE_STRING
   ├─ +0x0e0 [ 16] RuntimeData  UNICODE_STRING
   ├─ +0x0f0 [768] CurrentDirectories  RTL_DRIVE_LETTER_CURDIR[32]
   ├─ +0x3f0 [  8] EnvironmentSize  ULONG_PTR
   ├─ +0x3f8 [  8] EnvironmentVersion  ULONG_PTR
   ├─ +0x400 [  8] PackageDependencyData  PVOID
   ├─ +0x408 [  4] ProcessGroupId  ULONG
   ├─ +0x40c [  4] LoaderThreads  ULONG
   ├─ +0x410 [ 16] RedirectionDllName  UNICODE_STRING
   ├─ +0x420 [ 16] HeapPartitionName  UNICODE_STRING
   ├─ +0x430 [  8] DefaultThreadpoolCpuSetMasks  PULONGLONG
   ├─ +0x438 [  4] DefaultThreadpoolCpuSetMaskCount  ULONG
   ├─ +0x43c [  4] DefaultThreadpoolThreadMaximum  ULONG
   ╰─ +0x440 [  4] HeapMemoryTypeMask  ULONG
╰─ 2000 byte
```

</details>

---

Okay... that one was a softball... try this one, robot! What about... I'm on an `AMD64` host and I wanna see `_CONTEXT` for `ARM64`?

<details>

```powershell
> target\debug\bb.exe --phnt --arch arm64 --struct _CONTEXT --depth 1

_CONTEXT excpt.h:49:16
╰─ 912 bytes
_CONTEXT winnt.h:6796:54
├─ +0x000 [  4] ContextFlags  DWORD
├─ +0x004 [  4] Cpsr  DWORD
├─ +0x100 [  8] Sp  DWORD64
├─ +0x108 [  8] Pc  DWORD64
├─ +0x110 [512] V  NEON128[32]
│  ├─ +0x000 [ 16] D  double[2]
│  ├─ +0x000 [ 16] S  float[4]
│  ├─ +0x000 [ 16] H  WORD[8]
│  ╰─ +0x000 [ 16] B  BYTE[16]
├─ +0x310 [  4] Fpcr  DWORD
├─ +0x314 [  4] Fpsr  DWORD
├─ +0x318 [ 32] Bcr  DWORD[8]
├─ +0x338 [ 64] Bvr  DWORD64[8]
├─ +0x378 [  8] Wcr  DWORD[2]
╰─ +0x380 [ 16] Wvr  DWORD64[2]
╰─ 912 bytes
```

... wait, really? Can you also do x86?

```powershell
> target\debug\bb.exe --phnt --arch x86 --struct _CONTEXT --depth 1

_CONTEXT excpt.h:36:12
╰─ 716 bytes
_CONTEXT winnt.h:8506:35
├─ +0x000 [  4] ContextFlags  DWORD
├─ +0x004 [  4] Dr0  DWORD
├─ +0x008 [  4] Dr1  DWORD
├─ +0x00c [  4] Dr2  DWORD
├─ +0x010 [  4] Dr3  DWORD
├─ +0x014 [  4] Dr6  DWORD
├─ +0x018 [  4] Dr7  DWORD
├─ +0x01c [112] FloatSave  FLOATING_SAVE_AREA
│  ├─ +0x000 [  4] ControlWord  DWORD
│  ├─ +0x004 [  4] StatusWord  DWORD
│  ├─ +0x008 [  4] TagWord  DWORD
│  ├─ +0x00c [  4] ErrorOffset  DWORD
│  ├─ +0x010 [  4] ErrorSelector  DWORD
│  ├─ +0x014 [  4] DataOffset  DWORD
│  ├─ +0x018 [  4] DataSelector  DWORD
│  ├─ +0x01c [ 80] RegisterArea  BYTE[80]
│  ╰─ +0x06c [  4] Spare0  DWORD
├─ +0x08c [  4] SegGs  DWORD
├─ +0x090 [  4] SegFs  DWORD
├─ +0x094 [  4] SegEs  DWORD
├─ +0x098 [  4] SegDs  DWORD
├─ +0x09c [  4] Edi  DWORD
├─ +0x0a0 [  4] Esi  DWORD
├─ +0x0a4 [  4] Ebx  DWORD
├─ +0x0a8 [  4] Edx  DWORD
├─ +0x0ac [  4] Ecx  DWORD
├─ +0x0b0 [  4] Eax  DWORD
├─ +0x0b4 [  4] Ebp  DWORD
├─ +0x0b8 [  4] Eip  DWORD
├─ +0x0bc [  4] SegCs  DWORD
├─ +0x0c0 [  4] EFlags  DWORD
├─ +0x0c4 [  4] Esp  DWORD
├─ +0x0c8 [  4] SegSs  DWORD
╰─ +0x0cc [512] ExtendedRegisters  BYTE[512]
╰─ 716 bytes
```

Oh Sweet Jesus...

</details>

---

Okay, what else can you do, robot? Maybe you can help me store some of this data in an ***actually useful*** format? I don't know... JSON?

<details>

```json
> target\debug\bb.exe --phnt --struct _UNICODE_STRING --json

[
  {
    "name": "_UNICODE_STRING",
    "location": {
      "file": "__bb_phnt_synthetic.h",
      "line": 482,
      "column": 20
    },
    "size": 16,
    "fields": [
      {
        "name": "Length",
        "type": "USHORT",
        "offset": 0,
        "offset_bytes": 0,
        "size": 2,
        "alignment": 2
      },
      {
        "name": "MaximumLength",
        "type": "USHORT",
        "offset": 16,
        "offset_bytes": 2,
        "size": 2,
        "alignment": 2
      },
      {
        "name": "Buffer",
        "type": "PWCH",
        "offset": 64,
        "offset_bytes": 8,
        "size": 8,
        "alignment": 8
      }
    ]
  }
]
```

Well tickle me pink!

</details>

---

What is it all that you do, robot?

```text
Benowin Blanc (bb): Windows through a detective's lens...

Parse Windows SDK or PHNT embedded headers and extract struct information.

Usage: bb.exe [OPTIONS]

Options:
      --winsdk [<WINSDK>]     Use Windows SDK headers (optionally specify version)
      --phnt [<PHNT>]         Use PHNT headers with specified version [possible values: win2k, win-xp, ws03, vista, win7, win8, win-blue, threshold, threshold2, redstone, redstone2, redstone3, redstone4, redstone5, 19H1, 19H2, 20H1, 20H2, 21H1, Win10-21H2, Win10-22H2, win11, Win11-22H2]
  -m, --mode <MODE>           Mode: user or kernel (defines _KERNEL_MODE for kernel) [default: user] [possible values: user, kernel]
      --json                  Output as JSON
  -a, --arch <ARCH>           Architecture to target (supports cross-compilation) [default: amd64] [possible values: x86, amd64, arm, arm64]
  -H, --filter <FILTER>       Filter by header file (e.g., winternl.h)
  -s, --struct <STRUCT_NAME>  Struct name pattern (supports * wildcard)
  -f, --field <FIELD_NAME>    Field name pattern (supports * wildcard)
  -c, --case-sensitive        Case-sensitive matching
  -d, --depth <DEPTH>         Recursion depth for nested types [default: 0]
      --diagnostics           Show clang diagnostics
  -h, --help                  Print help
```