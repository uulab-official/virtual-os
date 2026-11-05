# ParallelsVirtualizationSDK — Export 기반 API 레퍼런스

> **대상**: `/Applications/Parallels Desktop.app/Contents/Frameworks/ParallelsVirtualizationSDK.framework`
> **Bundle ID**: `com.parallels.virtualization.sdk` · **버전**: 27.0.3 (build 58680, SDK 11)
> **바이너리**: `Versions/11/ParallelsVirtualizationSDK` (universal: x86_64 + arm64)
> **Export**: **1,268개** `Prl*` 심볼 (+ C++ 심볼 1개)
> **검증**: REA `inspect_macho` — `ev_00b1ae26…`, `inspect_signature` — `ev_e4419c0d…`
> **전체 심볼 목록**: [`analysis/sdk-symbols.txt`](analysis/sdk-symbols.txt)

---

## 1. 이 문서의 성격과 한계

이 문서는 **export된 심볼 이름**을 근거로 작성했다. 다음은 확인된 사실과 확인되지 않은 것을 구분한다.

| 구분 | 내용 |
|------|------|
| **관찰** | 심볼 이름, 개수, 패밀리 분포, 프레임워크 메타데이터, 의존성 |
| **추론** | 이름 규칙(`동사_목적어`)에서 도출한 기능 분류, 핸들 기반 수명주기 |
| **미확인** | 함수 시그니처(인자/반환형), 구조체 레이아웃, 오류 코드 값, 스레드 안전성, ABI 안정성 |

> ⚠️ 이 문서의 의사코드는 **이름 기반 구조 예시**이며, 실제 프로토타입이 아니다.
> 헤더 파일이 번들에 포함되어 있지 않고 디컴파일도 수행하지 않았으므로,
> 인자 타입·순서·반환값은 **미확인**이다. 실제 사용 시 공식 SDK 헤더로 검증해야 한다.

프레임워크에는 `Headers/`가 없고 바이너리와 `Resources/Info.plist`만 있다(관찰).
즉 이 프레임워크는 **앱 내부 소비용**으로 배포되며, 공개 SDK 헤더는 별도 배포된다(추론).

---

## 2. Export 규모와 패밀리 분포

총 **1,268개**의 `Prl*` 심볼이 export된다. 상위 패밀리는 다음과 같다.

| 패밀리 | 개수 | 대표 심볼 |
|--------|-----:|-----------|
| `PrlVmCfg` | 390 | `PrlVmCfg_SetCpuCount`, `PrlVmCfg_GetRamSize`, `PrlVmCfg_CreateVmDev` |
| `PrlVm` | 103 | `PrlVm_Start`, `PrlVm_Stop`, `PrlVm_GetState`, `PrlVm_Clone` |
| `PrlSrv` | 73 | `PrlSrv_Create`, `PrlSrv_LoginLocal`, `PrlSrv_GetVmList` |
| `PrlDisk` | 58 | `PrlDisk_CreateDisk`, `PrlDisk_Compact`, `PrlDisk_Encrypt` |
| `PrlVirtNet` | 50 | `PrlVirtNet_Create`, `PrlVirtNet_GetAdapterName` |
| `PrlDispCfg` | 45 | `PrlDispCfg_GetDefaultVmDir`, `PrlDispCfg_GetMaxVmMem` |
| `PrlVmDevNet` | 43 | `PrlVmDevNet_GenerateMacAddr`, `PrlVmDevNet_GetAdapterType` |
| `PrlSrvCfg` | 41 | `PrlSrvCfg_GetCpuCount`, `PrlSrvCfg_GetCpuModel` |
| `PrlVmDev` | 40 | `PrlVmDev_Create`, `PrlVmDev_Connect`, `PrlVmDev_SetImagePath` |
| `PrlReport` | 21 | `PrlReport_AsString`, `PrlReport_GetData` |
| `PrlDevDisplay` | 21 | `PrlDevDisplay_ConnectToVm`, `PrlDevDisplay_GetScreenSize` |
| `PrlVmDevHd` | 19 | `PrlVmDevHd_GetDiskSize`, `PrlVmDevHd_AddPartition` |
| `PrlLoginParams` | 19 | `PrlLoginParams_Create`, `PrlLoginParams_GetHost` |
| `PrlDiskParams` | 18 | `PrlDiskParams_GetBlockSize`, `PrlDiskParams_GetCHS` |
| `PrlApi` | 18 | `PrlApi_Init`, `PrlApi_Deinit`, `PrlApi_GetVersion` |
| `PrlVmGuestOsInfo` | 15 | `PrlVmGuestOsInfo_GetArch`, `PrlVmGuestOsInfo_GetBuildVersion` |
| `PrlEvent` | 13 | `PrlEvent_CreateAnswerEvent`, `PrlEvent_GetErrCode` |
| `PrlSrvCfgNet` | 12 | `PrlSrvCfgNet_GetDnsServers`, `PrlSrvCfgNet_GetMacAddress` |
| `PrlShare` | 11 | `PrlShare_GetPath`, `PrlShare_IsReadOnly` |
| `PrlPortFwd` | 11 | `PrlPortFwd_GetIncomingPort`, `PrlPortFwd_GetRedirectVm` |
| `PrlDiskStorageParams` | 11 | `PrlDiskStorageParams_GetImageName` |
| `PrlUsrCfg` | 10 | `PrlUsrCfg_GetDefaultVmFolder`, `PrlUsrCfg_AddProxy` |
| `PrlEvtPrm` | 10 | `PrlEvtPrm_ToInt32`, `PrlEvtPrm_ToHandle` |
| `PrlBackupParam` | 10 | `PrlBackupParam_GetLevel`, `PrlBackupParam_GetLocationUuid` |
| `PrlVmRegisterParams` | 9 | `PrlVmRegisterParams_GetVmPath` |
| `PrlSrvCfgDev` | 9 | `PrlSrvCfgDev_GetDeviceState` |
| `PrlJob` | 9 | `PrlJob_GetResult`, `PrlJob_GetOpCode` |
| `PrlDiskOpParams` | 9 | `PrlDiskOpParams_GetDiskFilePath` |
| `PrlBootDev` | 9 | `PrlBootDev_GetType`, `PrlBootDev_GetSequenceIndex` |
| `PrlBackup` | 9 | `PrlBackup_Begin`, `PrlBackup_Commit` |
| `PrlSrvCfgHddPart` | 8 | `PrlSrvCfgHddPart_GetSize` |
| `PrlTisRecord` | 7 | `PrlTisRecord_GetName`, `PrlTisRecord_GetData` |
| `PrlThirdPartyVmConvertParams` | 7 | `PrlThirdPartyVmConvertParams_GetSourcePath` |
| `PrlScrRes` | 7 | `PrlScrRes_GetWidth`, `PrlScrRes_SetHeight` |
| `PrlHandle` | 7 | `PrlHandle_AddRef`, `PrlHandle_Free`, `PrlHandle_ToString` |
| `PrlBackupFile` | 7 | `PrlBackupFile_GetPath`, `PrlBackupFile_GetKind` |
| `PrlVmDevSound` | 6 | `PrlVmDevSound_SetOutputDev` |
| `PrlStat` | 6 | `PrlStat_GetCpuStat`, `PrlStat_GetUsageRamSize` |
| `PrlSrvCfgHdd` | 6 | `PrlSrvCfgHdd_GetDevName`, `PrlSrvCfgHdd_GetPart` |
| `PrlUsrInfo` | 5 | `PrlUsrInfo_GetName`, `PrlUsrInfo_GetSessionCount` |
| `PrlSrvInfo` | 5 | `PrlSrvInfo_GetHostName`, `PrlSrvInfo_GetProductVersion` |
| `PrlResult` | 5 | `PrlResult_GetParam`, `PrlResult_GetParamsCount` |
| `PrlFoundVmInfo` | 5 | `PrlFoundVmInfo_GetConfigPath`, `PrlFoundVmInfo_IsTemplate` |
| `PrlDevMouse` | 5 | `PrlDevMouse_Move`, `PrlDevMouse_SetPos` |
| `PrlAcl` | 5 | `PrlAcl_IsAllowed`, `PrlAcl_GetOwnerName` |
| `PrlVmInfo` | 4 | `PrlVmInfo_GetState`, `PrlVmInfo_GetVmProcessId` |
| `PrlUsbIdent` | 4 | `PrlUsbIdent_GetFriendlyName`, `PrlUsbIdent_GetVmUuidAssociation` |
| `PrlTool` | 4 | `PrlTool_AllocBuffer`, `PrlTool_SendDataAsync` |
| `PrlThirdPartyVmConvertResult` | 4 | `PrlThirdPartyVmConvertResult_GetVmUuid` |
| `PrlStrList` | 4 | `PrlStrList_AddItem`, `PrlStrList_GetItem` |
| `PrlHndlList` | 4 | `PrlHndlList_AddItem`, `PrlHndlList_GetItem` |
| `PrlDevSecondaryDisplay` | 4 | `PrlDevSecondaryDisplay_SyncCaptureScaledScreenRegionToBuffer` |
| `PrlDbg` | 4 | `PrlDbg_GetHandlesNum`, `PrlDbg_PrlResultToString` |
| `PrlVmGuest` | 3 | `PrlVmGuest_RunProgram`, `PrlVmGuest_SetUserPasswd` |
| `PrlVmDevHdPart` | 3 | `PrlVmDevHdPart_GetSysName` |
| `PrlTisEmitter` | 3 | `PrlTisEmitter_RegCallback` |
| `PrlBackupInfo` | 3 | `PrlBackupInfo_GetState` |
| `PrlVmDevUsb` | 2 | `PrlVmDevUsb_SetAutoconnectOption` |
| `PrlVmDevSerial` | 2 | `PrlVmDevSerial_SetSocketMode` |
| `PrlDiskStateParams` | 2 | `PrlDiskStateParams_GetUid` |
| `PrlDevKeyboard` | 2 | `PrlDevKeyboard_SendKeyEventEx` |
| `PrlBackupFileDiff` | 2 | `PrlBackupFileDiff_GetOffset` |
| `PrlStatVmData` / `PrlStatCpu` | 1 / 1 | `PrlStatVmData_GetSegmentCapacity`, `PrlStatCpu_GetCpuUsage` |
| `PrlSrvConfig` / `PrlSessionInfo` | 1 / 1 | `PrlSrvConfig_FromString`, `PrlSessionInfo_Create` |
| `PrlDevSIA` / `PrlBuffer` / `PrlAppliance` | 1 / 1 / 1 | `PrlDevSIA_SendSIAData`, `PrlBuffer_Free`, `PrlAppliance_Create` |

> 전체 1,268개 목록은 [`analysis/sdk-symbols.txt`](analysis/sdk-symbols.txt)에 있다.

---

## 3. 기능 영역별 API

### 3.1 초기화·버전·진단 (`PrlApi`, 18)

| 함수 | 이름 기반 역할 |
|------|----------------|
| `PrlApi_Init` / `PrlApi_InitEx` | SDK 초기화 (Ex = 확장 옵션) |
| `PrlApi_Deinit` | 정리 |
| `PrlApi_GetVersion` | SDK 버전 |
| `PrlApi_SetLogPath` / `PrlApi_SwitchConsoleLogging` | 로그 설정 |
| `PrlApi_InitCrashHandler` / `PrlApi_GetCrashDumpsPath` | 크래시 처리 |
| `PrlApi_GetResultDescription` / `PrlApi_GetMessageType` | 결과/메시지 조회 |
| `PrlApi_CreateHandlesList` / `PrlApi_CreateStringsList` | 컬렉션 생성 |
| `PrlApi_CreateProblemReport` / `PrlApi_CreatePackedProblemReport` | 문제 보고서 |
| `PrlApi_SendPackedProblemReport` | 보고서 전송 |
| `PrlApi_GetMainServerVersion` / `PrlApi_SetMainServerVersion` | 서버 버전 협상 |
| `PrlApi_MsgCanBeIgnored` | 메시지 무시 가능 여부 |

### 3.2 서버·세션 (`PrlSrv`, 73)

| 함수 | 역할 |
|------|------|
| `PrlSrv_Create` | 서버 핸들 생성 |
| `PrlSrv_LoginLocal` / `LoginLocalEx` / `LoginLocalWithParams` | 로컬 서비스 로그인 |
| `PrlSrv_IsConnected` / `PrlSrv_Logoff` | 연결 상태/로그아웃 |
| `PrlSrv_GetVmList` / `GetVmListEx` | VM 목록 |
| `PrlSrv_GetVmConfig` | VM 설정 조회 |
| `PrlSrv_GetServerInfo` / `RefreshServerInfo` | 서버 정보 |
| `PrlSrv_GetSrvConfig` | 서버 설정 |
| `PrlSrv_GetUserInfo` / `GetUserInfoList` / `GetUserProfile` | 사용자 정보 |
| `PrlSrv_GetCommonPrefs` / `CommonPrefsBeginEdit` / `CommonPrefsCommit` | 공통 환경설정 |
| `PrlSrv_RegisterVm` / `RegisterVmEx` / `RegisterVmWithParams` / `RegisterVmWithUuid` | VM 등록 |
| `PrlSrv_StartConvertHdd` / `ConvertThirdPartyVm` / `EstimateConvertThirdPartyVm` | 디스크/VM 변환 |
| `PrlSrv_CheckParallelsServerAlive` / `CheckParallelsServerAliveEx` | 헬스 체크 |
| `PrlSrv_SubscribeToHostStatistics` / `UnsubscribeFromHostStatistics` | 호스트 통계 |
| `PrlSrv_SendProblemReport` / `SendClientStatistics` / `SendITAdminReport` | 텔레메트리 |
| `PrlSrv_GetLicenseInfo` / `UpdateLicenseEx` | 라이선스 |
| `PrlSrv_NetServiceStart` / `NetServiceStop` / `NetServiceRestart` | 네트워크 서비스 제어 |
| `PrlSrv_AddVirtualNetwork` / `DeleteVirtualNetwork` / `UpdateVirtualNetwork` | 가상 네트워크 |
| `PrlSrv_InstallAppliance` / `InstallDeclarativeVmAppliance` / `InstallGoldenImageVmAppliance` | 어플라이언스 |
| `PrlSrv_GetITAdminPoliciesSettings` / `UpdateITAdminAllVmSettings` | 기업 관리(MDM) |

**추론**: `PrlSrv_LoginLocal`은 로컬 디스패처에 대한 세션을 만들고, 이후 모든 VM 작업은
이 세션 핸들을 통해 수행된다. 원격 서버용 `Login` 계열은 export 목록에 없다.

### 3.3 VM 생명주기 (`PrlVm`, 103)

| 그룹 | 함수 |
|------|------|
| 생성/등록 | `PrlVm_Reg`, `PrlVm_RegEx`, `PrlVm_Clone`, `PrlVm_CloneEx`, `PrlVm_Delete` |
| 시작/정지 | `PrlVm_Start`, `StartEx`, `Stop`, `StopEx`, `Pause`, `Suspend`, `Resume`, `Reset`, `Restart` |
| 상태 | `PrlVm_GetState`, `GetConfig`, `RefreshConfig`, `GetStatistics`, `GetStatisticsEx` |
| 편집 | `PrlVm_BeginEdit`, `Commit`, `CommitEx`, `SetConfig`, `ValidateConfig` |
| 스냅샷 | `PrlVm_CreateSnapshot`, `DeleteSnapshot`, `SwitchToSnapshot`, `SwitchToSnapshotEx`, `GetSnapshotsTree`, `UpdateSnapshotData` |
| 디스크 | `PrlVm_Compact`, `CancelCompact`, `ConvertDisks`, `CancelConvertDisks` |
| 게스트 도구 | `PrlVm_InstallTools`, `UpdateToolsSection`, `GetToolsState`, `ToolsOpenFileBrowser`, `ToolsShowRecycleBin` |
| 게스트 제어 | `PrlVm_LoginInGuest`, `PrlVmGuest_RunProgram`, `PrlVmGuest_SetUserPasswd`, `PrlVmGuest_Logout` |
| 콘솔 | `PrlVm_TerminalConnect`, `TerminalDisconnect`, `StartVncServer`, `StopVncServer` |
| 보안 | `PrlVm_Encrypt`, `Decrypt`, `Authorise`, `ChangePassword`, `SetProtection`, `RemoveProtection` |
| 기타 | `PrlVm_Archive`, `Unarchive`, `Pack`, `Unpack`, `Move`, `LinkedClone` |
| 이벤트 | `PrlVm_RegEventHandler`, `UnregEventHandler`, `CreateEvent`, `SendClipboardRequest` |

### 3.4 VM 설정 (`PrlVmCfg`, 390 — 최대 패밀리)

`PrlVmCfg`는 VM의 거의 모든 설정을 `Get`/`Set` 쌍으로 노출한다.
이름에서 확인되는 주요 범주:

| 범주 | 예시 |
|------|------|
| 식별 | `GetName`/`SetName`, `GetUuid`/`SetUuid`, `GetVmType` |
| OS | `GetOsType`, `GetOsVersion`/`SetOsVersion`, `GetArch` |
| CPU | `GetCpuCount`/`SetCpuCount`, `GetCpuType`/`SetCpuType`, `SetCpuHotplugEnabled`, `SetCpuAccelLevel` |
| 메모리 | `GetRamSize`/`SetRamSize`, `SetRamAutoSizeEnabled` |
| 칩셋/BIOS | `SetChipsetType`, `SetChipsetVersion`, `SetBiosType` |
| 디스플레이 | `Set3DAccelerationEnabled`, `Set3DAccelerationMode`, `SetVideoRamSize`, `SetVideoAdapterType` |
| 부팅 | `CreateBootDev`, `SetAllowSelectBootDevice`, `PrlBootDev_*` |
| 장치 | `CreateVmDev`, `CreateShare`, `CreateScrRes` |
| 전원/세션 | `SetActionOnStopMode`, `SetActionOnWindowClose`, `SetAutoStart`, `SetAutoStop` |
| 통합 | `SetClipboardSyncMode`, `SetAppInDockMode`, `SetAutoCaptureReleaseMouse` |
| 네트워크 | `SetDefaultGateway`, `SetDefaultGatewayIPv6`, `SetHostname` |

**추론**: 390개라는 규모는 VM 설정 스키마가 매우 세분화되어 있음을 의미한다.
`PrlVm_BeginEdit` → `PrlVmCfg_Set*` → `PrlVm_Commit`의 트랜잭션 패턴이 이름상 자연스럽다
(실제 호출 규약은 **미확인**).

### 3.5 하드웨어·장치

| 패밀리 | 개수 | 대표 함수 |
|--------|-----:|-----------|
| `PrlVmDev` | 40 | `Create`, `Connect`, `Disconnect`, `CreateImage`, `SetImagePath`, `SetPassthrough` |
| `PrlVmDevHd` | 19 | `GetDiskSize`, `GetDiskType`, `AddPartition`, `CheckPassword`, `GetOnlineCompactMode` |
| `PrlVmDevNet` | 43 | `GenerateMacAddr`, `GetAdapterType`, `GetBoundAdapterName`, `GetDefaultGatewayIPv6` |
| `PrlVmDevSound` | 6 | `SetOutputDev`, `SetMixerDev`, `SetAdvancedType` |
| `PrlVmDevUsb` | 2 | `SetAutoconnectOption`, `GetAutoconnectOption` |
| `PrlVmDevSerial` | 2 | `SetSocketMode`, `GetSocketMode` |
| `PrlDevKeyboard` | 2 | `SendKeyEventEx`, `SendOptimizationMode` |
| `PrlDevMouse` | 5 | `Move`, `MoveEx`, `SetPos`, `SetPosEx`, `Event` |
| `PrlUsbIdent` | 4 | `GetFriendlyName`, `GetSystemName`, `GetVmUuidAssociation` |

### 3.6 디스크 (`PrlDisk`, 58 + `PrlDiskParams` 18 + `PrlDiskOpParams` 9)

| 그룹 | 함수 |
|------|------|
| 생성/열기 | `PrlDisk_CreateDisk`, `CreateDiskEx`, `CreateDisk_Local`, `OpenDisk`, `OpenDiskEx`, `OpenDisk_Local` |
| 용량 | `Resize`, `IncreaseCapacity`, `DecreaseCapacity`, `CalcDefaultBlockSize`, `CalcSplitPartSize` |
| 압축 | `Compact`, `CompactByBitmap`, `EstimateCompactByBitmap`, `IsCompactable` |
| 암호화 | `Encrypt`, `EncryptEx`, `Decrypt`, `ChangeKey`, `ChangeKeyEx`, `SetKey`, `IsEncrypted` |
| 스냅샷/상태 | `CreateState`, `DeleteState`, `SwitchToState`, `GetStatesTree`, `GetSnapshotsTree`, `ReopenWithSnapshot` |
| I/O | `Read`, `Write`, `FlushCache`, `SetCachingPolicy`, `WaitForCompletion` |
| 변환 | `ConvertToCHS`, `ConvertToCurrent`, `ConvertToExtendFormat`, `ConvertToCurrent_Local` |
| 무결성 | `FixConsistency`, `IsBootable_Local`, `GetIdentify`, `GetDiskInfo` |

### 3.7 네트워크

| 패밀리 | 개수 | 대표 함수 |
|--------|-----:|-----------|
| `PrlVirtNet` | 50 | `Create`, `GetAdapterName`, `GetAdapterIndex`, `GetBoundAdapterInfo`, `GetDescription` |
| `PrlVmDevNet` | 43 | `GenerateMacAddr`, `GetAdapterType`, `GetBoundCardMac`, `GetDefaultGateway` |
| `PrlSrvCfgNet` | 12 | `GetDnsServers`, `GetMacAddress`, `GetNetAdapterType`, `GetNetAddresses` |
| `PrlPortFwd` | 11 | `GetIncomingPort`, `GetRedirectIPAddress`, `GetRedirectPort`, `GetRuleName` |
| `PrlShare` | 11 | `GetPath`, `IsEnabled`, `IsReadOnly`, `GetName`, `Remove` |

### 3.8 디스플레이·화면

| 패밀리 | 개수 | 대표 함수 |
|--------|-----:|-----------|
| `PrlDevDisplay` | 21 | `ConnectToVm`, `DisconnectFromVm`, `GetScreenSize`, `GetScreenBuffer`, `AsyncEnableUpdates` |
| `PrlScrRes` | 7 | `GetWidth`, `GetHeight`, `SetWidth`, `SetHeight`, `SetEnabled` |
| `PrlDevSecondaryDisplay` | 4 | `SyncCaptureScaledScreenRegionToBuffer`, `AsyncCapture...` |

캡처 계열은 `Async*`/`Sync*` × `ScreenRegion`/`ScaledScreenRegion` × `ToFile`/`ToBuffer`
조합으로 세분화되어 있다(관찰).
`PrlDevDisplay_LockForRead` / `Unlock`은 화면 버퍼 동시 접근 제어용으로 보인다(추론).

### 3.9 백업·복원

| 패밀리 | 개수 | 대표 함수 |
|--------|-----:|-----------|
| `PrlBackup` | 9 | `Begin`, `Commit`, `GetFilesList`, `GetInfo`, `GetParams`, `GetSessionUuid` |
| `PrlBackupParam` | 10 | `GetLevel`, `GetLocationUuid`, `GetChangeMonitoring`, `GetOptions` |
| `PrlBackupFile` | 7 | `GetPath`, `GetRestorePath`, `GetKind`, `GetLevel`, `GetSize` |
| `PrlBackupFileDiff` | 2 | `GetOffset`, `GetSize` |
| `PrlBackupInfo` | 3 | `GetState`, `GetVmPath`, `IsBootcamp` |

**추론**: 증분/차등 백업을 `Level`과 `Diff`로 표현하는 구조다.
`IsBootcamp`는 레거시 Boot Camp VM 호환 흔적으로 보인다.

### 3.10 게스트·통계·이벤트

| 패밀리 | 개수 | 대표 함수 |
|--------|-----:|-----------|
| `PrlVmGuestOsInfo` | 15 | `GetArch`, `GetBuildVersion`, `GetMajorVersion`, `GetMinorVersion` |
| `PrlStat` | 6 | `GetCpuStat`, `GetUsageRamSize`, `GetTotalRamSize`, `GetVmDataStat` |
| `PrlStatCpu` / `PrlStatVmData` | 1 / 1 | `GetCpuUsage` / `GetSegmentCapacity` |
| `PrlVmInfo` | 4 | `GetState`, `GetVmProcessId`, `GetAccessRights`, `IsInvalid` |
| `PrlEvent` | 13 | `CreateAnswerEvent`, `CreateResponse`, `GetErrCode`, `GetDataPtr`, `CanBeIgnored` |
| `PrlEvtPrm` | 10 | `ToInt32`, `ToUint64`, `ToBoolean`, `ToHandle`, `GetName` |
| `PrlJob` | 9 | `GetResult`, `GetOpCode`, `GetError`, `GetEvent`, `Cancel` |
| `PrlResult` | 5 | `GetParam`, `GetParamAsString`, `GetParamsCount` |
| `PrlTisRecord` / `PrlTisEmitter` | 7 / 3 | `GetName`, `GetData`, `RegCallback`, `UnregCallback` |

**추론**: 비동기 작업은 `PrlJob` + `PrlEvent` + `PrlResult` 조합으로 표현된다.
`PrlEvent_CreateAnswerEvent`/`CreateResponse`는 요청-응답 이벤트 모델을 시사한다.

### 3.11 인증·ACL·라이선스

| 패밀리 | 개수 | 대표 함수 |
|--------|-----:|-----------|
| `PrlLoginParams` | 19 | `Create`, `GetHost`, `GetPortNumber`, `GetPasswd`, `GetFlags`, `GetPrevSessionUuid` |
| `PrlAcl` | 5 | `IsAllowed`, `GetAccessForOthers`, `SetAccessForOthers`, `GetOwnerName`, `IsCurrentSessionOwner` |
| `PrlUsrCfg` | 10 | `GetDefaultVmFolder`, `AddProxy`, `CanChangeSrvSets`, `CanUseMngConsole` |
| `PrlUsrInfo` | 5 | `GetName`, `GetUuid`, `GetSessionCount`, `CanChangeSrvSets` |
| `PrlDispCfg` | 45 | `GetDefaultVmDir`, `GetMaxVmMem`, `GetMaxReservMemLimit`, `GetLicenseRequestFlags` |

- `PrlSrv_GetLicenseInfo` / `UpdateLicenseEx`가 라이선스 조회/갱신을 담당한다(관찰).
- 실제 서명 검증·만료 판정은 디스패처 내부(`CDspLicenseChecker`)에서 수행된다(문자열 추론).

### 3.12 변환·어플라이언스·기타

| 패밀리 | 개수 | 대표 함수 |
|--------|-----:|-----------|
| `PrlThirdPartyVmConvertParams` | 7 | `GetSourcePath`, `SetDestinationPath`, `GetFlags` |
| `PrlThirdPartyVmConvertResult` | 4 | `GetVmUuid`, `GetResultPath`, `GetRequiredDiskSpace` |
| `PrlVmRegisterParams` | 9 | `GetVmPath`, `GetVmName`, `GetFlags` |
| `PrlFoundVmInfo` | 5 | `GetConfigPath`, `GetName`, `IsTemplate` |
| `PrlReport` | 21 | `AsString`, `GetData`, `GetArchiveFileName`, `GetClientVersion` |
| `PrlHandle` | 7 | `AddRef`, `Free`, `GetType`, `ToString`, `RegEventHandler` |
| `PrlHndlList` / `PrlStrList` | 4 / 4 | `AddItem`, `GetItem`, `GetItemsCount`, `RemoveItem` |
| `PrlDbg` | 4 | `GetHandlesNum`, `HandleTypeToString`, `PrlResultToString` |
| `PrlTool` | 4 | `AllocBuffer`, `FreeBuffer`, `SendDataAsync`, `Unregister` |
| `PrlBuffer` | 1 | `Free` |
| `PrlAppliance` | 1 | `Create` |
| `PrlDevSIA` / `PrlSessionInfo` / `PrlSrvConfig` | 1 / 1 / 1 | `SendSIAData` / `Create` / `FromString` |

---

## 4. 핸들·수명주기 모델 (추론)

이름 규칙에서 확인되는 공통 패턴:

```
PrlApi_Init
   │
   ├─ PrlSrv_Create              → 서버 핸들
   │    └─ PrlSrv_LoginLocal     → 세션
   │         ├─ PrlSrv_GetVmList → VM 목록
   │         └─ PrlVm_GetConfig  → PrlVmCfg 핸들
   │              ├─ PrlVmCfg_Set*  (설정 편집)
   │              └─ PrlVm_Commit
   │
   ├─ PrlHandle_AddRef / PrlHandle_Free
   ├─ PrlHndlList_* / PrlStrList_*  (컬렉션)
   └─ PrlApi_Deinit
```

| 관찰된 규칙 | 의미 |
|-------------|------|
| `*_Create` / `*_Free` | 핸들 생성/해제 |
| `*_Get*` / `*_Set*` | 속성 접근 |
| `*_BeginEdit` / `*_Commit` | 트랜잭션 편집 |
| `*_Async*` / `*_Sync*` | 비동기/동기 변형 |
| `*_Ex` | 확장 옵션 버전 |
| `*_Local` | 로컬 전용 변형 |
| `*_FromString` / `*_ToString` | 직렬화 |
| `PrlJob_*` + `PrlEvent_*` | 비동기 작업/이벤트 |

**미확인**: 참조 카운팅 규칙, 핸들 소유권, 스레드 안전성, 오류 전달 방식(반환 코드 vs out-param).

---

## 5. 호출 시퀀스 (이름 기반 구조 예시)

> 아래는 **의사코드**다. 실제 시그니처가 아니다.

```c
/* 1. 초기화 */
PrlApi_Init();

/* 2. 로컬 서비스 세션 */
HANDLE hSrv;
PrlSrv_Create(&hSrv);
PrlSrv_LoginLocal(hSrv, /* user, password, flags ... */);

/* 3. 기존 VM 설정 조회 */
HANDLE hVm;
PrlSrv_GetVmList(hSrv, &hVmList);
/* 목록에서 VM 핸들 획득 */
PrlVm_GetConfig(hVm, &hVmCfg);

/* 4. 설정 편집 */
PrlVm_BeginEdit(hVm);
PrlVmCfg_SetCpuCount(hVmCfg, 4);
PrlVmCfg_SetRamSize(hVmCfg, 4096);
PrlVm_Commit(hVm);

/* 5. 시작 */
PrlVm_Start(hVm);

/* 6. 화면 캡처 */
PrlDevDisplay_ConnectToVm(hVm, ...);
PrlDevDisplay_GetScreenSize(hVm, &w, &h);
PrlDevDisplay_SyncCaptureScreenRegionToFile(hVm, ...);

/* 7. 정리 */
PrlVm_Stop(hVm, ...);
PrlHandle_Free(hVmCfg);
PrlHandle_Free(hVm);
PrlSrv_Logoff(hSrv);
PrlApi_Deinit();
```

**주의**: `PrlSrv_GetVmList`가 반환하는 목록에서 VM 핸들을 얻는 정확한 함수
(`PrlHndlList_GetItem` 등)와 각 인자의 타입은 **미확인**이다.

---

## 6. SDK 소비자

| 소비자 | 사용 방식 | 근거 |
|--------|-----------|------|
| `prl_client_app` | 프레임워크를 직접 링크, UI에서 VM 관리 | `ev_f7534839…` (의존성에 `libprl_sdk.11.dylib`) |
| `prlctl` | CLI에서 동일 SDK 사용 | `ev_1a174d3b…` |
| `prlsrvctl` | 서버 설정/상태 | `ev_b0bc1860…` |
| `libPrlSdkCommunication.1` | IPC/전송 계층 | 프레임워크 의존성 |
| `libPrlXmlModel.1` | VM 설정 모델 | 프레임워크 의존성 |

프레임워크의 install name은 `@rpath/ParallelsVirtualizationSDK.framework/Versions/11/libprl_sdk.11.dylib`이며,
`sdk-11`이 ABI 버전임을 시사한다(관찰).

---

## 7. 안정성·호환성 메모

| 항목 | 관찰/추론 |
|------|-----------|
| 아키텍처 | universal(x86_64 + arm64) — 호스트 앱은 arm64만 사용 |
| 최소 OS | 14.0 (Sonoma) |
| Hardened Runtime | 프레임워크 자체는 미적용, 로더 정책을 따름 |
| SDK 버전 표기 | 디렉터리 `Versions/11`, install name `libprl_sdk.11.dylib` |
| 심볼 노출 | 1,268개 전체 export — stripped 아님 |
| ABI | `_Ex`/`_Local` 변형이 많아 버전 간 호환성 관리 흔적으로 보임(추론) |

---

## 8. 참고

- 전체 심볼: [`analysis/sdk-symbols.txt`](analysis/sdk-symbols.txt) (1,268줄)
- 프레임워크 Evidence: `ev_00b1ae26…` (inspect_macho), `ev_e4419c0d…` (inspect_signature)
- 이 문서의 기능 분류는 **심볼 이름 기반**이며, 공식 문서/헤더와 다를 수 있다.
