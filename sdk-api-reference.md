# ParallelsVirtualizationSDK API 레퍼런스

> **출처**: `/Applications/Parallels Desktop.app/Contents/Frameworks/ParallelsVirtualizationSDK.framework`
> **타입**: C API (Objective-C 래핑 가능)
> **버전**: SDK 11 (Parallels Desktop 27.0.3)

---

## API 카테고리별 함수 목록

### 1. 초기화 / 종료 (Initialization)

| 함수 | 설명 |
|------|------|
| `PrlApi_Init()` | SDK 초기화. 모든 API 호출 전 필수 |
| `PrlApi_InitEx()` | 확장 초기화 (로그 경로, 옵션 등) |
| `PrlApi_Deinit()` | SDK 정리. 프로세스 종료 시 호출 |
| `PrlApi_SwitchConsoleLogging()` | 콘솔 로깅 전환 |
| `PrlApi_SetLogPath()` | 로그 파일 경로 설정 |
| `PrlApi_InitCrashHandler()` | 크래시 핸들러 초기화 |
| `PrlApi_GetCrashDumpsPath()` | 크래시 덤프 경로 반환 |
| `PrlApi_GetMainServerVersion()` | 메인 서버 버전 확인 |
| `PrlApi_SetMainServerVersion()` | 메인 서버 버전 설정 |

### 2. VM 생명주기 (VM Lifecycle)

| 함수 | 설명 |
|------|------|
| `PrlApi_CreateVm()` | 새 VM 생성 |
| `PrlApi_Start()` | VM 시작 |
| `PrlApi_StartEx()` | 확장 옵션으로 VM 시작 |
| `PrlApi_Stop()` | VM 정지 |
| `PrlApi_StopEx()` | 확장 옵션으로 VM 정지 (ACPI 등) |
| `PrlApi_ShutdownEx()` | 게스트 OSシャット다운 요청 |
| `PrlApi_Suspend()` | VM 서스펜드 (상태 저장) |
| `PrlApi_DropSuspendedState()` | 서스펜드 상태 삭제 |
| `PrlApi_Cancel()` | 작업 취소 |

### 3. VM 설정 (VM Configuration)

| 함수 | 설명 |
|------|------|
| `PrlCommonPrefsBeginEdit()` | 설정 편집 시작 (트랜잭션) |
| `PrlCommonPrefsCommit()` | 설정 커밋 |
| `PrlUserProfileBeginEdit()` | 사용자 프로필 편집 시작 |
| `PrlUserProfileCommit()` | 사용자 프로필 커밋 |

**설정 항목 (Get/Set)**:
- `SetVmName()` / `GetVmName()` — VM 이름
- `SetUuid()` / `GetUuid()` — VM UUID
- `SetVmDirPath()` / `GetVmDirPath()` — VM 저장 경로
- `SetVmDirUuid()` / `GetVmDirUuid()` — VM 디렉토리 UUID
- `SetArch()` / `GetArch()` — 아키텍처 (x86_64, ARM64)
- `SetType()` / `GetAdvancedType()` — VM 유형 (Windows, Linux, macOS 등)
- `SetOs()` / `GetOs()` — 게스트 OS 식별

### 4. 하드웨어 설정 (Hardware Configuration)

| 카테고리 | 주요 함수 |
|------------|-----------|
| **CPU** | `SetCpus()`, `GetCpus()` — 코어 수 설정 |
| **메모리** | `SetMemSize()`, `GetMemSize()` — RAM (MB) 설정 |
| **디스플레이** | `SetVideoRamSize()`, `GetVideoRamSize()` — 비디오 RAM |
| **3D 가속** | `Set3DAccelerationMode()`, `Get3DAccelerationMode()` — Metal 3D |
| **비디오 어댑터** | `SetVideoAdapterType()`, `GetVideoAdapterType()` — 가상 그래픽 카드 |
| **수직 동기화** | `SetVerticalSynchronizationEnabled()` — VSync |

### 5. 부트 장치 (Boot Devices)

| 함수 | 설명 |
|------|------|
| `CreateBootDev()` | 부트 장치 생성 (ISO, 디스크, 네트워크) |
| `GetBootDevType()` | 부트 장치 유형 확인 (CD, HDD, Network) |
| `GetBootDevIndex()` / `SetBootDevIndex()` | 부트 순서 설정 |
| `GetBootDevSequenceIndex()` | 시퀀스 인덱스 |
| `IsBootDevInUse()` | 장치 사용 여부 확인 |

### 6. 디스크 관리 (Disk Management)

| 함수 | 설명 |
|------|------|
| `CreateDisk()` / `CreateDiskEx()` | 가상 디스크 생성 |
| `ConvertDisks()` / `StartConvertHdd()` | 디스크 변환 (VMDK → PVM 등) |
| `CancelConvertDisks()` | 디스크 변환 취소 |
| `Compact()` / `CompactByBitmap()` | 디스크 압축 (공간 회수) |
| `CancelCompact()` | 압축 취소 |
| `EstimateCompactByBitmap()` | 압축 예상량 계산 |

### 7. 네트워크 (Networking)

| 함수 | 설명 |
|------|------|
| `AddVirtualNetwork()` / `DeleteVirtualNetwork()` | 가상 네트워크 추가/삭제 |
| `UpdateVirtualNetwork()` | 가상 네트워크 업데이트 |
| `GetAdapterType()` / `GetAdapterName()` / `GetAdapterIndex()` | 어댑터 정보 |
| `SetVlanTag()` — VLAN 태그 설정 |

### 8. USB 장치 (USB Devices)

| 함수 | 설명 |
|------|------|
| `SetUsbIdentAssociation()` — USB 장치 연결 |
| `SetUsbAutoConnectOption()` — 자동 연결 옵션 |

### 9. 백업 / 스냅샷 (Backup & Snapshots)

| 함수 | 설명 |
|------|------|
| `PrlBackup_Begin()` — 백업 세션 시작 |
| `PrlBackup_Commit()` — 백업 커밋 |
| `PrlBackup_Rollback()` — 백업 롤백 |
| `PrlBackup_GetInfo()` — 백업 정보 조회 |
| `PrlBackup_GetParams()` — 백업 파라미터 |
| `PrlBackup_SetParams()` — 백업 파라미터 설정 |
| `PrlBackup_GetFilesList()` — 백업 파일 목록 |
| `PrlBackup_SendProgress()` — 진행률 보고 |

**스냅샷**:
- `CreateSnapshot()` / `DeleteSnapshot()` — 스냅샷 생성/삭제
- `SwitchToSnapshot()` / `SwitchToSnapshotEx()` — 스냅샷으로 전환
- `UpdateSnapshotData()` — 스냅샷 데이터 업데이트

### 10. 디스플레이 / 화면 (Display)

| 함수 | 설명 |
|------|------|
| `ConnectToVm()` / `DisconnectFromVm()` — VM 연결/해제 |
| `GetAvailableDisplaysCount()` — 사용 가능한 디스플레이 수 |
| `AsyncCaptureScreenRegionToFile()` — 비동기 화면 캡처 (파일) |
| `AsyncCaptureScaledScreenRegionToBuffer()` — 비동기 캡처 (버퍼) |
| `SyncCaptureScreenRegionToFile()` — 동기 화면 캡처 |
| `GetScreenSize()` / `GetScreenBuffer()` — 화면 크기/버퍼 |
| `AsyncEnableUpdates()` — 업데이트 활성화 |
| `GetDynResToolStatus()` — 동적 해상도 상태 |

### 11. ACL / 권한 (Access Control)

| 함수 | 설명 |
|------|------|
| `PrlAcl_IsAllowed()` — 권한 확인 |
| `PrlAcl_SetAccessForOthers()` — 타인 접근 설정 |
| `PrlAcl_GetAccessForOthers()` — 타인 접근 조회 |
| `PrlAcl_SetOwnerName()` / `GetOwnerName()` — 소유자 설정 |
| `PrlAcl_IsCurrentSessionOwner()` — 현재 세션 소유자 확인 |

### 12. 기타 기능 (Miscellaneous)

| 함수 | 설명 |
|------|------|
| `CreateSnapshot()` — 스냅샷 생성 |
| `SwitchToSnapshot()` — 스냅샷으로 전환 |
| `StartVncServer()` / `StopVncServer()` — VNC 서버 관리 |
| `SubscribeToGuestStatistics()` / `UnsubscribeFromGuestStatistics()` — 게스트 통계 구독 |
| `SubscribeToHostStatistics()` / `UnsubscribeFromHostStatistics()` — 호스트 통계 구독 |
| `SetTpmType()` — TPM 2.0 설정 |
| `AreNaptDependentFeaturesAvailable()` — NAT 의존 기능 확인 |
| `CanBeCapturedByVirtualizationFramework()` — 가상화 프레임워크 캡처 가능 여부 |
| `TisSubscribe()` / `TisUnsubscribe()` — TIS (Telemetry/Inspection) 구독 |

---

## 데이터 타입 및 핸들링

| 함수 | 설명 |
|------|------|
| `PrlApi_CreateHandlesList()` — 핸들 목록 생성 |
| `PrlDbg_GetHandlesNum()` — 핸들 수 조회 |
| `PrlDbg_HandleTypeToString()` — 핸들 타입 문자열화 |
| `ToHandle()` — 값을 핸들로 변환 |
| `PrlDbg_EventTypeToString()` — 이벤트 타입 문자열화 |
| `PrlDbg_PrlResultToString()` — 결과 코드 문자열화 |

## 문자열 / 값 변환

| 함수 | 설명 |
|------|------|
| `PrlApi_CreateStringsList()` — 문자열 목록 생성 |
| `AsString()` / `FromString()` — 문자열 변환 |
| `ToBoolean()` / `ToInt32()` / `ToInt64()` — 타입 변환 |
| `ToUint32()` / `ToUint64()` — 부호 없는 타입 변환 |
| `ToCData()` — CDATA 변환 |

## 메모리 관리

| 함수 | 설명 |
|------|------|
| `AllocBuffer()` — 버퍼 할당 |
| `PrlBuffer_Free()` — 버퍼 해제 |

## 암호화 / 보안

| 함수 | 설명 |
|------|------|
| `Encrypt()` / `EncryptEx()` — 데이터 암호화 |
| `Decrypt()` — 데이터 복호화 |
| `Authorise()` — 인증 |
| `CheckPassword()` / `ChangePassword()` — 비밀번호 관리 |

## 아카이브 / 압축

| 함수 | 설명 |
|------|------|
| `Archive()` — 아카이브 생성 |
| `Unarchive()` — 아카이브 해제 |
| `Pack()` / `Unpack()` — 패킹/언패킹 |

---

## SDK 사용 패턴 (예시)

```c
// 1. 초기화
PrlApi_Init();

// 2. VM 생성
HANDLE hVm;
PrlApi_CreateVm(&hVm);

// 3. 설정
PrlCommonPrefsBeginEdit(hVm);
PrlApi_SetCpus(hVm, 4);
PrlApi_SetMemSize(hVm, 4096);
PrlCommonPrefsCommit(hVm);

// 4. 시작
PrlApi_Start(hVm);

// 5. 화면 캡처 (비동기)
PrlDevDisplay_AsyncCaptureScaledScreenRegionToBuffer(hVm, ...);

// 6. 정리
PrlApi_StopEx(hVm, PRL_STOP_ACPI);
PrlApi_Deinit();
```

---

*본 레퍼런스는 REA의 Native Binary Analysis 방법론을 기반으로 ParallelsVirtualizationSDK.framework의 심볼 테이블에서 추출되었습니다.*
