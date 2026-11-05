# virtual-os — REA 기반 역분석 및 아키텍처 보고서

> **프로젝트**: virtual-os  
> **저장소**: `https://github.com/uulab-official/virtual-os`  
> **분석 대상**: Parallels Desktop.app (v27.0.3, Build 58680)  
> **Copyright**: 2026 Parallels International GmbH  
> **Bundle ID**: `com.parallels.desktop.console`  
> **Minimum System**: macOS 14.0 (Sequoia)  
> **분석일**: 2026-11-05  

---

## 1. 개요 (Executive Summary)

Parallels Desktop는 macOS에서 Windows, Linux 등 타 OS를 가상머신(VM)으로 구동하는 **상용 하이퍼바이저 솔루션**입니다. macOS 14+ 전용으로 재설계된 v27은 Apple Silicon (ARM64) 네이티브 컴퓨팅과 Intel x86_64 에뮬레이션을 모두 지원합니다.

본 보고서는 REA (Reverse Engineer Anything) 프로젝트의 Desktop Application Analysis 방법론을 적용하여 Parallels Desktop.app의 정적 분석 결과를 정리한 것입니다.

### 핵심 아키텍처 요약
| 레이어 | 기술 스택 | 역할 |
|--------|-----------|------|
| **UI** | Qt 6 (QuickControls2 + QML) | 데스크톱 UI, VM 컨테이너 관리 |
| **Virtualization SDK** | ParallelsVirtualizationSDK (C API) | VM 생명주기, 디스크, 네트워크, USB 관리 |
| **Hypervisor** | Apple Hypervisor.framework | 커널 레벨 가상화 |
| **x86 에뮬레이션** | libMonitorX86Emu (JIT 재컴파일) | x86_64 → ARM64 실시간 번역 |
| **ARM 가상화** | libMonitorArm (HVT) | Apple Silicon 네이티브 ARM64 실행 |
| **CLI** | prlctl / prlsrvctl (arm64) | 명령줄 VM 관리, 서버 컨트롤 |
| **네트워크** | prl_naptd (NAT daemon) | 가상 네트워크, NAT, 브리징 |
| **서비스 계층** | Parallels Service (setuid) | 루트 권한으로 VM 디스패처, 네트워킹 관리 |

---

## 2. 바이너리 아키텍처 (Binary Architecture)

### 2.1 메인 실행 파일

| 바이너리 | 아키텍처 | 크기 | 역할 |
|----------|----------|------|------|
| `prl_client_app` | **arm64** | 29.4 MB | **메인 UI 클라이언트**. VM 컨테이너, 디스플레이, 사용자 인터페이스 담당 |
| `Parallels Service` | **arm64** (setuid/setgid) | 87.9 KB | **시스템 서비스**. 루트 권한으로 VM 디스패처, 네트워킹, 장치 매니저 |
| `prlctl` | **arm64** | 686.9 KB | **CLI 도구**. VM 생성/설정/제어 명령줄 인터페이스 |
| `prlsrvctl` | **arm64** | 686.9 KB | **서버 컨트롤**. Parallels 서비스 상태 관리, 구성 |

### 2.2 x86_64 바이너리 (Rosetta 2 의존)

| 바이너리 | 아키텍처 | 크기 | 역할 |
|----------|----------|------|------|
| `prl_disk_tool` | **x86_64** | 505.1 KB | 가상 디스크 변환/관리 (VMDK, VHD, VHDX 등) |
| `prl_convert` | **x86_64** | 1.7 MB | 타사 VM 포맷 변환 (VMware, VirtualBox, Hyper-V 등) |

### 2.3 기타 핵심 바이너리 (전부 arm64)

| 바이너리 | 크기 | 역할 |
|----------|------|------|
| `prl_vmarchiver` | 69.8 KB | VM 패키징/압축 (.pvm, .pvmz) |
| `prl_osimage_tool` | 531.9 KB | OS 이미지 관리 (부트캣프, 리커버리 파티션) |
| `prl_naptd` | 234.3 KB | NAT 프로토콜 데몬 (가상 네트워크) |
| `prl_net_start` | 234.3 KB | 네트워크 시작/정지 스크립트 |
| `prl_bglaunch` | 69.5 KB | 백그라운드 VM 시작 |
| `prl_update_helper` | 89.8 KB (setuid) | 업데이트 권한 승인 |
| `prl_updater_ctl` | 291.5 KB | 자동 업데이트 컨트롤러 |
| `prl_esd2iso` | 69.6 KB | ESD → ISO 변환 (Windows 설치 미디어) |
| `7z` / `7z.dylib` | 437.6 KB + 1.6 MB | 7-Zip 아카이브 (VM 압축/백업) |
| `watchdog` | 2.7 KB (bash) | 서비스 감시/재시작 스크립트 |

### 2.4 모니터링 라이브러리 (가상화 핵심)

| 라이브러리 | 크기 | 역할 |
|------------|------|------|
| `libMonitorArm.dylib` | 1.0 MB | **Apple Silicon 네이티브 ARM64 가상화**. Apple Hypervisor.framework 활용 (HVT: Hardware Virtualization Technology) |
| `libMonitorX86Emu.dylib` | 3.6 MB | **x86_64 → ARM64 실시간 JIT 에뮬레이션**. FEX-JIT 기반 재컴파일러. x86 명령어 번역, CMOS/ PIT / 시리얼 / 병렬 포트 에뮬레이션 |

---

## 3. 프레임워크 분석 (Framework Analysis)

### 3.1 ParallelsVirtualizationSDK (핵심 가상화 SDK)

**역할**: 모든 VM 관리 기능의 핵심 C API 라이브러리. UI, CLI, 서비스 모두 이 SDK를 호출합니다.

**주요 API 카테고리**:

| 카테고리 | 주요 함수 | 설명 |
|-----------|-----------|------|
| **VM 생명주기** | `CreateVm`, `Start`, `Stop`, `ShutdownEx`, `Suspend` | VM 생성, 시작/정지, 서스펜드 |
| **디스플레이** | `ConnectToVm`, `AsyncCaptureScaledScreenRegionToBuffer`, `GetAvailableDisplaysCount` | VM 화면 캡처, 다중 디스플레이, 동적 해상도 |
| **디스크** | `CreateDisk`, `ConvertDisks`, `Compact`, `StartConvertHdd` | 가상 디스크 생성/변환/압축 |
| **네트워크** | `AddVirtualNetwork`, `DeleteVirtualNetwork`, `UpdateVirtualNetwork` | 가상 네트워크 관리 (NAT, 브리지, 호스트 전용) |
| **USB** | `SetUsbIdentAssociation`, `SetUsbAutoConnectOption` | USB 장치 연결/자동 연결 |
| **백업** | `PrlBackup_Begin`, `PrlBackup_Commit`, `PrlBackup_Rollback` | VM 스냅샷/백업 시스템 (增量 백업) |
| **ACL/권한** | `PrlAcl_IsAllowed`, `PrlAcl_SetAccessForOthers` | VM 접근 제어 목록 |
| **Boot 장치** | `CreateBootDev`, `GetBootDevType` | 부트 순서, ISO/디스크/네트워크 부팅 |
| **TPM** | `SetTpmType` | 가상 TPM 2.0 지원 |
| **3D 가속** | `Get3DAccelerationMode` | GPU 가상화 (Metal) |
| **VNC** | `StartVncServer`, `StopVncServer` | 원격 접속 서버 |
| **에러/로깅** | `PrlApi_Init`, `PrlApi_CreateProblemReport` | 문제 보고, 크래시 덤프, 로깅 |

### 3.2 Qt 프레임워크 (UI 레이어)

Qt 6 기반의 모던 UI. QuickControls2 + QML로 구축된 현대적 인터페이스:

| Qt 모듈 | 용도 |
|---------|------|
| `QtQuick` / `QtQml` | QML UI 프레임워크 |
| `QtQuickControls2` / `QtQuickTemplates2` | 컨트롤 (버튼, 입력창, 드로어 등) |
| `QtWidgets` | 레거시 위젯 호환 |
| `QtNetwork` | 네트워크 통신 (VM 상태, 업데이트) |
| `QtSql` | 로컬 데이터베이스 (SQLite) |
| `QtMacExtras` | macOS 네이티브 기능 (Dock, Menu Bar, Services) |
| `QtConcurrent` | 비동기 작업 (VM 시작/정지 등) |

**Qt 플러그인**:
- `libqcocoa.dylib` — macOS 플랫폼 통합 (네이티브 느낌)
- `libqmacstyle.dylib` — macOS 네이티브 스타일
- `libqtgraphicaleffectsplugin.dylib` — 그래픽 효과 (그림자, 블러 등)
- `libqsqlite.dylib` — SQLite 데이터베이스

### 3.3 기타 프레임워크

| 프레임워크 | 용도 |
|------------|------|
| `PDCocoaKit.framework` | Parallels 전용 Cocoa 유틸리티 |
| `PDControlCenter.framework` | 제어센터 (音量, 밝기 등) 통합 |
| `ProxyAppCore.framework` | 앱 프록시/연결 관리 |
| `ParallelsVirtualizationSDK.framework` | 핵심 가상화 SDK (위 참조) |
| `libprl_mspack.dylib` | 아카이브 압축/압축 해제 (ZIP, CAB 등) |
| `libwimlib.1.dylib` | WIM (Windows Imaging Format) 지원 |
| `libPrlGui.3.dylib` | GUI 헬퍼 라이브러리 |
| `libPrlSdkCommunication.1.dylib` | SDK-서비스 간 통신 |
| `libPrlXmlModel.1.dylib` | VM 설정 XML 모델링 |
| `7z.dylib` | 7-Zip 아카이브 지원 |

---

## 4. 서브 애플리케이션 (Sub-Applications)

MacOS 디렉토리에 중첩된 5개의 서브 애플리케이션:

| 서브 앱 | 위치 | 역할 |
|---------|------|------|
| `Parallels VM.app` | `Contents/MacOS/` | **VM 컨테이너**. 실제 가상머신 실행 UI |
| `Parallels Service.app` | `Contents/MacOS/` | **백그라운드 서비스**. 디스패처, 네트워킹, 장치 관리 |
| `Parallels Mac VM.app` | `Contents/MacOS/` | **macOS VM 전용 컨테이너**. Apple Silicon 네이티브 macOS 실행 |
| `Parallels Mounter.app` | `Contents/MacOS/` | **디스크 마운터**. ISO, 가상 디스크 마운트 관리 |
| `Parallels Link.app` | `Contents/MacOS/` | **연결 관리**. 호스트-Guest 간 연결 (클립보드, 파일 공유) |

---

## 5. 플러그인 확장 (PlugIns / AppEx)

### 5.1 앱 익스텐션 (Appex)

| 플러그인 | 용도 |
|----------|------|
| `ParallelsMail.appex` | **메일 통합**. VM 내 Outlook/Thunderbird 연동 |
| `ExeQLPlugin.appex` | **Windows 앱 실행**. Windows .exe 파일을 macOS에서 직접 실행 (Coherence 모드) |
| `OpenInIE.appex` | **Internet Explorer**. VM 내 IE 브라우저 연동 |

### 5.2 Qt 플러그인 구조

```
QtPlugIns/
├── platforms/libqcocoa.dylib       # macOS 네이티브 플랫폼
├── styles/libqmacstyle.dylib       # macOS 스타일
├── sqldrivers/libqsqlite.dylib     # SQLite 드라이버
├── imageformats/                   # 이미지 포맷 (GIF, JPEG, TIFF)
└── quick/                          # QML 그래픽 효과 플러그인
```

---

## 6. 파일 포맷 및 호환성

### 6.1 Parallels 네이티브 포맷

| 확장자 | 설명 |
|--------|------|
| `.pvm` | Parallels VM 설정 (XML 기반) |
| `.pvmz` | 압축된 Parallels VM |
| `.pvmp` | 패킹된 Parallels VM |
| `.pvs` / `.pvsz` | 스냅샷 파일 |

### 6.2 타사 포맷 지원 (prl_convert로 변환 가능)

| 포맷 | 출처 |
|------|------|
| `.vmdk` | VMware |
| `.vhd` / `.vhdx` | Hyper-V, VirtualBox |
| `.vdi` | VirtualBox |
| `.vmx` / `.vmc` | VMware Workstation/Fusion |
| `.vbox` | VirtualBox |
| `.vmwarevm/` | VMware Fusion 폴더 구조 |

---

## 7. 네트워크 아키텍처

```
┌─────────────────────────────────────────────┐
│  macOS Host                                  │
│                                              │
│  ┌──────────────┐    ┌───────────────────┐  │
│  │ prl_naptd    │    │ Parallels Service  │  │
│  │ (NAT Daemon) │    │ (setuid, root)     │  │
│  └──────┬───────┘    └────────┬──────────┘  │
│         │                     │              │
│  ┌──────▼───────┐    ┌───────▼──────────┐   │
│  │ Virtual NIC  │    │ prl_disp_service │   │
│  │ (bridge/NAT) │    │ (VM Dispatcher)  │   │
│  └──────┬───────┘    └───────┬──────────┘   │
│         │                     │              │
│  ┌──────▼─────────────────────▼──────────┐   │
│  │         Guest VM(s)                    │   │
│  └───────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
```

**네트워크 모드**: NAT, 브리지, 호스트 전용, 사용자 정의

---

## 8. 권한 모델 (Privilege Model)

| 프로세스 | UID/GID | 설명 |
|----------|---------|------|
| `Parallels Service` | root/wheel (setuid+setgid) | 커널 장치 접근, 네트워크 인터페이스 생성/삭제 |
| `prl_update_helper` | root/wheel (setuid) | 업데이트 시 권한 승인 |
| `prl_client_app` | 일반 사용자 | UI, VM 컨테이너 관리 |
| `prlctl` / `prlsrvctl` | 일반 사용자 (sudo 시 root) | CLI 관리 도구 |

**exceptions.list**: `Parallels Service`와 `prl_update_helper`만 루트 권한으로 실행됨.

---

## 9. 지원 OS 및 기능

### 9.1 호스트 지원
- **macOS 14.0 (Sequoia)** 이상 (LSMinimumSystemVersion)
- **Apple Silicon** (ARM64 네이티브) — 메인 UI, CLI, 서비스 모두 arm64
- **Intel Mac** (Rosetta 2 필요) — `prl_disk_tool`, `prl_convert`는 x86_64

### 9.2 게스트 OS 지원
- **Windows** (10, 11 — x86_64 / ARM64)
- **Linux** (주요 배포판 전체)
- **macOS** (Apple Silicon 네이티브 — Boot Camp 불필요)
- **Chrome OS**
- **FreeBSD**, **Solaris** (레거시)

### 9.3 핵심 기능
- **Coherence 모드**: Windows/Linux 앱을 macOS 데스크톱에 자연스럽게 통합
- **Unity 모드**: VM 창을 macOS 창처럼 표시
- **Hot Migration**: 실행 중 VM 이동 (서버 에디션)
- **스냅샷/백업**: 增量 백업 시스템 (PrlBackup API)
- **3D 가속**: Metal 기반 GPU 가상화
- **TPM 2.0**: 가상 TPM 지원 (Windows 11 요구)
- **USB 3.0/3.1**: USB 장치 호스팅

---

## 10. REA 분석 결론 (REA Analysis Summary)

### 분석 방법론 적용
REA의 Desktop Application Analysis 프레임워크를 다음과 같이 적용했습니다:

| REA 분석 항목 | Parallels Desktop 결과 |
|---------------|----------------------|
| **Runtime candidates** | `prl_client_app` (메인 UI), `Parallels Service` (백그라운드 서비스) |
| **Supporting files** | Qt 프레임워크, ParallelsVirtualizationSDK, 모니터링 라이브러리 |
| **Component markers** | `.pvm`, `.pvmz`, `.vmdk` 등 VM 관련 파일 핸들링 |
| **Next probes** | `libMonitorX86Emu`의 JIT 재컴파일러 상세 역분석, Apple Hypervisor.framework 통합 방식 |

### 주요 발견
1. **Apple Silicon 최적화**: v27은 완전히 arm64로 재컴파일됨 (UI, CLI, 서비스 모두)
2. **이중 가상화 아키텍처**: ARM 네이티브 (libMonitorArm) + x86 에뮬레이션 (libMonitorX86Emu/FEX-JIT)
3. **Qt 6 기반 모던 UI**: QML + QuickControls2로 재설계된 현대적 인터페이스
4. **포괄적인 타사 호환성**: VMware, VirtualBox, Hyper-V VM 직접 변환 지원
5. **서비스 계층의 분리**: setuid Parallels Service가 루트 권한을 담당, 일반 사용자는 prl_client_app으로 UI 상호작용

### 제한사항
- `Parallels Service` 내부의 디스패처 로직은 바이너리 역분석 필요 (Hopper/Ghidra 권장)
- `libMonitorX86Emu`의 JIT 재컴파일 알고리즘 상세 역분석 필요
- `libMonitorArm`의 Hypervisor.framework 통합 방식 분석 가능

---

*본 보고서는 REA (Reverse Engineer Anything) 프로젝트의 Desktop Application Analysis 방법론을 기반으로 작성되었습니다.*
