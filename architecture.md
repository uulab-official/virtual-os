# 아키텍처 — Parallels Desktop 27.0.3

> **대상**: `/Applications/Parallels Desktop.app` (27.0.3, build 58680)
> **플랫폼**: macOS 14.0+ (Sonoma), **arm64 전용**
> **방법**: REA 정적 분석 + 번들 리소스/문자열 확인
> **표기**: `관찰` = REA Evidence 또는 파일에서 직접 확인, `추론` = 관찰 기반 해석, `미확인` = 런타임/디컴파일 필요
>
> Evidence ID 전체 목록은 [`analysis/evidence-index.md`](analysis/evidence-index.md) 참조.

---

## 1. 한눈에 보는 구조

Parallels Desktop은 **UI 클라이언트 / root 상주 서비스 / VM별 실행기**의 3계층으로 나뉜다.
가상화 자체는 VM 실행기(`prl_vm_app`)가 담당하고, 디스패처(`prl_disp_service`)는
VM 생명주기·디스크·네트워크·USB·라이선스를 조정한다.

| 계층 | 프로세스 | 권한 | 역할 | Evidence |
|------|----------|------|------|----------|
| UI | `prl_client_app` (29.4 MB) | 사용자 | Qt 5.15.1 Widgets UI, VM 목록/설정/콘솔 | `ev_f7534839…`, `ev_6ba40a4b…` |
| 서비스 | `prl_disp_service` (7.9 MB) | **root** | 상주 데몬, VM/디스크/네트워크/USB/라이선스 조정 | `ev_a8b166b3…`, `ev_00f90db3…` |
| 실행기 | `prl_vm_app` (13.8 MB) | 사용자(추론) | 실제 VM 실행, HVF/FEX/장치 에뮬레이션 | `ev_b0c4eabf…`, `ev_376defa0…` |
| 실행기 | `prl_macvm_app` (2.7 MB) | 사용자(추론) | macOS 게스트, Virtualization.framework | `ev_7a60ef9c…`, `ev_cf5cf98c…` |
| 네트워크 | `prl_naptd` (537 KB) | root(추론) | NAT/DHCP/DNS 보조 데몬 | `ev_36a22813…` |
| 마운트 | `XPCMounter.xpc` | 사용자 | 디스크 이미지 마운트 XPC | `ev_e05b6d29…` |
| 런처 | `Parallels Service` (88 KB) | **setuid 6755** | 검증 후 디스패처 기동 | `ev_e3120afb…`, `ev_6ec439c2…` |
| 업데이터 | `prl_update_helper` (90 KB) | **setuid 4755** | 서명 검증 후 업데이트 적용 | `ev_c8633c1a…`, `ev_efef5c12…` |
| CLI | `prlctl`, `prlsrvctl` | 사용자 | SDK/서비스 제어 | `ev_1a174d3b…`, `ev_b0bc1860…` |
| CLI | `prl_disk_tool`, `prl_convert` | 사용자 | 디스크/변환 (**x86_64**, Rosetta) | `ev_73267ba1…`, `ev_5d3d4af3…` |

---

## 2. 기동·권한 상승 체인

### 2.1 다이어그램

```
launchd (PID 1, root)
 │
 │  /Library/LaunchDaemons/com.parallels.desktop.launchdaemon.plist
 │    ProgramArguments = /Library/Parallels/Parallels Desktop/launchd_wrapper
 │    RunAtLoad = true, ProcessType = Interactive, ExitTimeOut = 150
 │
 ▼
launchd_wrapper                       (root:wheel 0755, hardened ✔)
 │  ├─ /Library/Preferences/Parallels/parallels-desktop.loc 읽기
 │  ├─ 위치 검증: Malformed / Unsupported / Locations mismatch
 │  ├─ 번들 검증: "Bundle has incorrect owner/mode", stat 실패
 │  ├─ console user 확인
 │  ├─ setreuid() / setregid() / initgroups()   ← 권한 강하
 │  └─ exec Contents/MacOS/launchd_user_wrapper
 │
 ▼
launchd_user_wrapper                  (사용자 권한, hardened ✔)
 │
 ▼
Contents/MacOS/Parallels Service "start"   (setuid 6755, root:accessibility)
 │  ├─ 위치 파일 ↔ 실제 번들 경로 일치 검증
 │  ├─ /var/run/prl_desktop_services.lock  (shlock)
 │  ├─ foreground lock: /var/run/prl_desktop_services_foreground.lock
 │  ├─ bootstrap_get_root / bootstrap_look_up_per_user
 │  └─ 셸 로직으로 서비스 기동
 │
 ├──────────────► prl_disp_service --pidfile /var/run/prl_disp_service.pid
 │                  (root, hardened ✔)
 │
 └──────────────► watchdog start 60 20 prl_naptd ...
                    └─ /var/run/prl_watchdog-<md5>.pid
```

### 2.2 관찰 근거

| 사실 | 근거 |
|------|------|
| launchd가 `launchd_wrapper`를 직접 실행 | 번들 내 launchd plist 템플릿(직접 확인) |
| 위치 파일 경로 `parallels-desktop.loc` | `launchd_wrapper`·`Parallels Service` 문자열 |
| 소유자/모드 검증 문자열 | `Bundle has incorrect owner - %u`, `Bundle has incorrect mode - %o` |
| 권한 강하 | `setreuid()`, `setregid()`, `initgroups()` 문자열 |
| setuid 모드 | `Contents/Resources/exceptions.list` (6755 / 4755) |
| 디스패처 PID 파일 | `/var/run/prl_disp_service.pid` |
| watchdog 재시작 | `watchdog` 스크립트(`NatWatchdogTimeout=60`, `NatWatchdogLimit=20`) |
| launchd 모드 | `--launchd-mode`, `PRL_DISP_LAUNCHD_MODE`, `Dsp::Launchd::*` 문자열 |

### 2.3 서비스 설치 위치 (추론)

```
/Applications/Parallels Desktop.app/          ← 사용자 영역 번들 (실행 파일·프레임워크·확장)
/Library/Parallels/Parallels Desktop/
  ├─ launchd_wrapper                          ← root:wheel, 관찰
  └─ Updater/                                 ← 업데이트 스테이징
/Library/LaunchDaemons/com.parallels.desktop.launchdaemon.plist
/Library/Preferences/Parallels/parallels-desktop.loc
```

조사 시점에는 `launchd_wrapper`만 존재했고 LaunchDaemon plist는 없었다.
즉 **앱이 아직 1회 초기화되지 않은 상태**였다(관찰).

### 2.4 평가

- **관찰**: 위치 고정 + 번들 소유자/모드 + 코드서명 요구사항을 조합한 방어적 setuid 설계.
- **추론**: root 래퍼가 `/Applications` 하위 사용자 쓰기 가능 번들을 신뢰 경계로 사용하므로,
  관리자 권한을 가진 로컬 공격자는 번들 교체로 root 실행을 유도할 수 있다. 다만 이는
  macOS의 일반적인 설치 모델과 동일한 트레이드오프다.
- **미확인**: 검증과 exec 사이의 TOCTOU, 검증 실패 시 종료 코드/로그, 서명 요구사항의 정확한 범위.

---

## 3. 프로세스·IPC 토폴로지

### 3.1 다이어그램

```
┌──────────────────────────── 사용자 세션 ────────────────────────────┐
│                                                                    │
│  prl_client_app                                                    │
│   ├─ Qt 5.15.1 Widgets (libPrlGui.3)                               │
│   ├─ VM 설정 모델 (libPrlXmlModel.1)                               │
│   ├─ SDK 클라이언트 (libPrlSdkCommunication.1)                     │
│   ├─ PDCocoaKit / PDControlCenter                                  │
│   └─ WebKit / SafariServices (SSO), LocalAuthentication           │
│            │                                                       │
│            │  SDK C API (PrlSrv_* / PrlVm_* / PrlVmCfg_*)          │
│            ▼                                                       │
│  ParallelsVirtualizationSDK.framework (1,268 exports)              │
│            │                                                       │
│            │  로컬 IPC (정확한 전송 계층 미확인)                    │
│            ▼                                                       │
└────────────┼───────────────────────────────────────────────────────┘
             │
┌────────────▼──────────── root ─────────────────────────────────────┐
│  prl_disp_service (com.parallels.desktop.dispatcher)               │
│   ├─ VM 생명주기 (posix_spawn)                                      │
│   ├─ 디스크/스냅샷/백업                                             │
│   ├─ 네트워크 (vmnet, naptd, port forwarding)                      │
│   ├─ USB/스마트카드 (IOKit, PCSC, CryptoTokenKit)                  │
│   ├─ 라이선스 (CDspLicenseChecker, access/pda-api)                 │
│   └─ launchd 관리 (Dsp::Launchd::Manager)                          │
│            │                                                       │
│            ├──────────────► prl_vm_app  (VM별 1개)                 │
│            │                  ├─ libMonitorArm.dylib  (ARM/HVF)    │
│            │                  ├─ libMonitorX86Emu.dylib (FEX JIT)  │
│            │                  └─ Metal / IOSurface / CoreAudio     │
│            │                                                       │
│            ├──────────────► prl_macvm_app (macOS 게스트)           │
│            │                  └─ Virtualization.framework          │
│            │                                                       │
│            └──────────────► prl_naptd (NAT/DHCP/DNS)               │
│                               └─ launchd job com.parallels.vm.prl_naptd │
└────────────────────────────────────────────────────────────────────┘
```

### 3.2 IPC 식별자 (문자열 추론)

| 이름 | 추정 용도 |
|------|-----------|
| `com.parallels.disp.net.ipc` | 디스패처 ↔ 네트워크 |
| `com.parallels.naptd.ipc` | NAT 데몬 제어 |
| `com.parallels.vm.prl_naptd` | VM ↔ NAT launchd job |
| `com.parallels.prl_net_library` | 네트워크 라이브러리 |
| `com.parallels.desktop.managedprefs` | MDM 관리형 환경설정 |
| `com.parallels.xpcmounter` | XPC 마운트 서비스 |
| `/var/run/prl_disp_service.pid` | 디스패처 PID |
| `/var/run/prl_desktop_services.lock` | 서비스 중복 기동 방지 |

**미확인**: 클라이언트→디스패처 메시지의 실제 전송 방식(XPC/Mach port/Unix socket),
인증(코드 서명 감사 토큰/UID 검사), 메시지 스키마. `strings`에 `Connecting socket: %s`,
`unix` 관련 심볼이 있으나 단정할 수 없다.

### 3.3 프로세스 수명

- 디스패처는 **상주**한다. `Restart services - concurent prl_disp_service is running`,
  `Restart services - WindowServer started after prl_disp_service` 문자열로 보아
  WindowServer 재시작/중복 기동을 감지해 서비스를 재기동한다(문자열 추론).
- VM 실행기는 **VM별로 spawn**된다. 디스패처 문자열에
  `Failed posix_spawn() = %d - Parallels VM app fails to start`가 있다(관찰).
- `prl_naptd`는 watchdog이 감시하며, 60초 간격·최대 20회 재시작 정책을 갖는다(직접 확인).

---

## 4. 가상화 스택

### 4.1 ARM 게스트 — `libMonitorArm` (관찰)

```
prl_vm_app
  └─ libMonitorArm.dylib
       ├─ Hypervisor.framework (hv_vm_map, hv_vm_unmap, hv_gic_get_icc_reg)
       ├─ HVT (hvt.c, hvt_gic.c)
       ├─ PL011 UART (ArmPl011.cpp)
       ├─ GIC distributor/redistributor/CPU interface
       └─ VirtualPlatformArm (AHCI, DMA, virtio 계열)
```

- `Hypervisor.framework`에 직접 링크(관찰, `ev_6b7f8aa0…`).
- 문자열에 `HVT: hv_vm_map`, `ArmPl011`, `GIC: hv_gic_get_icc_reg`, `VirtualPlatformArm` 확인.
- **추론**: Apple Silicon의 EL2/HVF를 사용해 ARM64 게스트를 네이티브 속도로 실행하고,
  GIC/PL011/AHCI 등 주변장치는 자체 에뮬레이션한다.

### 4.2 x86_64 게스트 — `libMonitorX86Emu` (관찰)

```
prl_vm_app
  └─ libMonitorX86Emu.dylib
       ├─ FEX (FEXCore::CPU::InterpreterCore, FEXJIT)
       ├─ x86 디코더/실행기 (Cpu-64/*)
       ├─ PC 하드웨어 (Mainboard/Cmos, Dma, Acpi, Comm/Serial, Input/KbdMou)
       └─ Hypervisor.framework (보조)
```

- `FEX` 계열 JIT 및 인터프리터 심볼 확인(관찰, `ev_96e1089a…`).
- `FEX_*` 환경변수(코드 캐시, IR 덤프, GDB 서버 등)가 다수 존재.
- **추론**: x86 명령을 ARM64로 재컴파일하며, JIT 코드 페이지 때문에
  `prl_vm_app`이 `allow-unsigned-executable-memory`를 갖는다.

### 4.3 macOS 게스트 — `prl_macvm_app` (관찰)

- `Virtualization.framework` + `vmnet.framework` 링크(관찰).
- entitlement: `virtualization`, `vm.device-access`, `vm.networking`,
  `app-sandbox = false`(관찰, `ev_cf5cf98c…`).
- **추론**: Apple의 공식 가상화 프레임워크를 사용해 macOS 게스트를 구동하는 별도 경로다.
  HVF를 직접 쓰는 `prl_vm_app` 경로와 분리되어 있다.

### 4.4 그래픽·주변기기 (관찰)

| 영역 | 구성 |
|------|------|
| 그래픽 | `Metal`, `OpenGL`, `IOSurface`, `CoreVideo`, `QuartzCore` |
| 미디어 | `CoreMedia`, `AVFoundation`, `AudioToolbox`, `AudioUnit`, `CoreAudio` |
| 입력 | `GameController`, `CoreHaptics`, `IOKit` |
| 스마트카드 | `PCSC`, `CryptoTokenKit` |
| 네트워크 | `vmnet` |
| 소프트웨어 GL | `libOSMesa.8.dylib` (14.9 MB) |
| Metal IR | `libmetalirconverter.dylib` (86 MB) |

`libOSMesa`는 3D 가속이 불가능한 환경의 소프트웨어 OpenGL 폴백으로 보인다(추론).

---

## 5. VM 데이터 경로

### 5.1 번들 포맷

```
myvm.pvm/                         ← VM 패키지 (directory)
 ├─ config.pvs                    ← XML 설정 (libPrlXmlModel)
 ├─ *.hdd / *.hds                 ← 가상 디스크 (스냅샷 체인)
 ├─ Snapshots/                    ← .pvs 스냅샷
 ├─ *.NVRAM                       ← 펌웨어 변수
 └─ Windows 11.pvm/ ...           ← 중첩 데이터

myvm.pvmz                         ← 압축 VM (z)
myvm.pvmp                         ← 패킹 VM (p)
myvm.macvm                        ← Virtualization.framework 기반 macOS VM
```

문서 타입 등록(관찰, `ev_80877cc6…`):

| UTI | 확장자 | 역할 |
|-----|--------|------|
| `com.parallels.vm.vmconfig` | — | VM 설정 |
| `com.parallels.vm.vmpackage` | `.pvm` | VM 패키지 (Owner) |
| `com.parallels.vm.vmpackage.compressed` | `.pvmz` | 압축 VM |
| `com.parallels.vm.vmpackage.packed` | `.pvmp` | 패킹 VM |
| `com.parallels.vm.macvmpackage` | `.macvm` | macOS VM |

### 5.2 디스크 I/O (추론)

```
prl_client_app ──► prl_disp_service ──► prl_disk_tool (x86_64, Rosetta)
                              │              ├─ create / convert / compact
                              │              └─ snapshot chain
                              ▼
                        .hdd / .hds 파일
```

- 디스크 도구와 변환기는 **x86_64 thin**이므로 Apple Silicon에서 Rosetta 2로 실행된다(관찰).
- 디스크 포맷/압축 알고리즘 세부는 **미확인**(디컴파일 필요).

### 5.3 네트워크 경로 (관찰 + 추론)

```
VM (prl_vm_app)
  │  virtio-net / e1000 에뮬레이션
  ▼
vmnet.framework  ◄──►  prl_naptd (NAT/DHCP/DNS)
  │                        └─ com.parallels.naptd.ipc
  ├─ Shared (NAT)
  ├─ Bridged
  └─ Host-only

포트 포워딩: PrlPortFwd_* API → 디스패처 → naptd 규칙
가상 네트워크: PrlVirtNet_* API (Create/GetAdapterName/...)
```

- `vmnet.framework` 링크는 `prl_client_app`, `prl_disp_service`, `prl_vm_app`,
  `prl_macvm_app` 모두에서 확인(관찰).
- NAT 데몬은 launchd job `com.parallels.vm.prl_naptd`로 관리된다(문자열).
- 실제 패킷 경로·방화벽 규칙 적용 방식은 **미확인**.

### 5.4 공유 폴더·USB (추론)

- 공유 폴더: `PrlShare_*` API(11개), `libprl_shared_apps.dylib`.
- USB: `PrlVmDevUsb_*`, `PrlUsbIdent_*`, `com.apple.vm.device-access`.
- 스마트카드: `PrlVmDev_*` + PCSC/CryptoTokenKit, 디스패처의 `com.apple.security.smartcard`.
- 장치 핫플러그/재연결 정책은 **미확인**.

---

## 6. CLI·도구 계층

```
사용자 터미널
 │
 ├─ prlctl (arm64)
 │    └─ ParallelsVirtualizationSDK → prl_disp_service
 │
 ├─ prlsrvctl (arm64)
 │    └─ 서비스 설정/상태 (SDK 경유)
 │
 ├─ prl_disk_tool (x86_64, Rosetta)
 │    └─ 디스크 생성/변환/압축
 │
 ├─ prl_convert (x86_64, Rosetta)
 │    └─ 타사 VM → Parallels 포맷
 │
 ├─ prl_vmarchiver / prl_osimage_tool / prl_esd2iso (arm64)
 │    └─ 아카이브·OS 이미지 변환
 │
 ├─ prl_updater_ctl (arm64)
 │    └─ 업데이트 제어 → prl_update_helper (setuid)
 │
 └─ prl_naptd / prl_net_start (arm64)
      └─ 네트워크 보조
```

- `prlctl`/`prlsrvctl`은 의존성이 3개뿐인 얇은 CLI로, 실제 작업은 SDK/서비스에 위임한다(관찰).
- `7z` + `7z.dylib`은 아카이브 처리용으로 번들에 포함되어 있다(관찰).

---

## 7. 번들 레이아웃

```
Parallels Desktop.app/Contents/
├─ MacOS/
│  ├─ prl_client_app                     ← 메인 UI
│  ├─ launchd_user_wrapper               ← 사용자 권한 래퍼
│  ├─ Parallels Service                  ← setuid 6755 런처
│  ├─ prl_update_helper                  ← setuid 4755 업데이터
│  ├─ prlctl / prlsrvctl / prl_naptd
│  ├─ prl_disk_tool / prl_convert        ← x86_64
│  ├─ watchdog (bash)
│  ├─ libMonitorArm.dylib
│  ├─ libMonitorX86Emu.dylib
│  ├─ Parallels VM.app/                  ← prl_vm_app + XPCMounter.xpc
│  ├─ Parallels Service.app/             ← prl_disp_service + launchd plist 템플릿
│  ├─ Parallels Mac VM.app/              ← prl_macvm_app
│  ├─ Parallels Mounter.app/             ← 디스크 이미지 마운터
│  └─ Parallels Link.app/                ← Windows 바로가기 핸들러
├─ Frameworks/
│  ├─ ParallelsVirtualizationSDK.framework/   (1,268 Prl* exports)
│  ├─ libPrlGui.3.dylib / libPrlXmlModel.1.dylib
│  ├─ libPrlSdkCommunication.1.dylib / libAbstractTask.1.dylib
│  ├─ PDCocoaKit / PDControlCenter / ProxyAppCore
│  ├─ Qt*.framework ×17 (5.15.1)
│  └─ QtPlugIns/{platforms,styles,sqldrivers,imageformats,quick}
├─ PlugIns/
│  ├─ …ExeQLPlugin.appex                ← .exe 썸네일
│  ├─ …OpenInIE.appex                   ← Safari 확장
│  └─ …ParallelsMail.appex              ← 공유 확장
└─ Resources/
   ├─ exceptions.list                   ← setuid 대상/모드
   ├─ License.ver                       ← 4.0.0.parallels
   ├─ launchd_wrapper                   ← LaunchDaemon이 실행하는 사본
   ├─ qml/ (105개, Qt 기본 모듈)
   └─ prl_boot_{32,64}* / EFI/BIOS 바이너리
```

**규모** (관찰): 약 348 MB, Mach-O 84개(실행 파일 36 / 공유 라이브러리 47 / 번들 1),
PE 15개, 중첩 `.app` 5개, `.xpc` 1개, `.appex` 3개, framework 21개, 시스템 확장 0개.

---

## 8. 확장·통합 지점

| 지점 | 식별자 | 동작 |
|------|--------|------|
| Quick Look | `com.parallels.desktop.console.ExeQLPlugin` | Windows `.exe` 썸네일 (`com.microsoft.windows-executable`) |
| Safari | `com.parallels.desktop.console.OpenInIE` | 툴바 "Open current page in Edge", 허용 도메인 `*.webkit.org` |
| Share | `com.parallels.desktop.console.ParallelsMail` | 파일 URL 1개 공유 |
| Services | `Reveal in Windows`, `Open in Windows` | `public.item`/`public.data` |
| URL 스킴 | `http`, `https`, `pdfm` | SSO 콜백 포함 |
| 문서 | `.pvm`, `.pvmz`, `.pvmp`, `.macvm` | Owner |
| 링크 | `.lnk`, `.plinkAg`, `.plinkFe`, `.plinkPd` | Parallels Link (LSUIElement) |

---

## 9. 설계상 특징과 한계

### 9.1 특징

1. **명확한 권한 분리**: UI(사용자) / 디스패처(root) / VM(사용자)이 분리되어 있다.
2. **VM별 프로세스**: VM 하나가 죽어도 디스패처와 다른 VM은 유지된다(추론).
3. **이중 하이퍼바이저 경로**: HVF 직접 사용(ARM/x86 에뮬)과 Virtualization.framework(macOS) 병존.
4. **플러그 가능한 모니터**: `libMonitorArm`/`libMonitorX86Emu`를 런타임에 로드한다.
5. **launchd 통합**: NAT/서비스/업데이트가 launchd job으로 관리된다.

### 9.2 한계·미확인

- 클라이언트↔디스패처 IPC의 전송 계층·인증·스키마.
- 디스패처의 라이선스 검증 알고리즘과 오프라인 정책.
- FEX 버전, JIT 캐시 정책, 코드 페이지 보호 방식.
- VM 설정(`config.pvs`) 파서의 메모리 안전성.
- 서비스 설치/업데이트의 실제 파일 작업과 롤백.
- Intel 호스트 지원: 1차 실행 파일이 arm64 thin이므로 **미지원**(관찰).

---

## 10. Evidence 대응표

| 다이어그램/주장 | Evidence |
|-----------------|----------|
| 번들 그래프·규모 | `ev_a19f02f0…`, `ev_7372cb8a…` |
| 메인 UI 구조/서명 | `ev_f7534839…`, `ev_6ba40a4b…` |
| 디스패처 구조/서명 | `ev_a8b166b3…`, `ev_00f90db3…` |
| VM 실행기 구조/서명 | `ev_b0c4eabf…`, `ev_376defa0…` |
| macOS VM 구조/서명 | `ev_7a60ef9c…`, `ev_cf5cf98c…` |
| setuid 스텁 구조/서명 | `ev_e3120afb…`, `ev_6ec439c2…` |
| 업데이터 구조/서명 | `ev_c8633c1a…`, `ev_efef5c12…` |
| launchd 래퍼 서명 | `ev_32d51458…` |
| ARM 모니터 구조 | `ev_6b7f8aa0…` |
| x86 모니터 구조 | `ev_96e1089a…` |
| SDK 구조 | `ev_00b1ae26…` |
| 아키텍처(arm64 thin) | `ev_f834c8b8…` |
| Info.plist(URL/문서/ATS) | `ev_80877cc6…` |
| 디스패처 plist(백그라운드) | `ev_fe534773…` |
| 중첩 앱 plist | `ev_4e4ed519…`, `ev_99106043…`, `ev_be347521…`, `ev_0322a434…` |
| 확장 plist | `ev_55d25f7c…`, `ev_9fcdbd4d…`, `ev_714ab174…` |
| XPC plist | `ev_e05b6d29…` |

---

*문자열에서 얻은 결론은 본문에 `(문자열 추론)`으로 표기했다. 런타임 동작은 검증되지 않았다.*
