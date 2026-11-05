# virtual-os — 아키텍처 다이어그램

## 전체 시스템 아키텍처

```
┌─────────────────────────────────────────────────────────────────────┐
│                        macOS 14+ (Sequoia)                          │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    Parallels Desktop.app                      │  │
│  │                                                               │  │
│  │  ┌─────────────────┐    ┌──────────────────────────────────┐  │  │
│  │  │  prl_client_app │    │         Parallels Service        │  │  │
│  │  │  (arm64, 29MB)  │    │   (setuid/setgid, root)         │  │  │
│  │  │                 │    │                                  │  │  │
│  │  │  ┌───────────┐  │    │  ┌────────────┐  ┌────────────┐  │  │  │
│  │  │  │ Qt 6 UI   │  │    │  │ prl_disp_  │  │ prl_naptd  │  │  │  │
│  │  │  │ (QML +    │  │    │  │ service    │  │ (NAT)      │  │  │  │
│  │  │  │ QuickCtrl2)│  │    │  │ (VM Disp.) │  │            │  │  │  │
│  │  │  └─────┬─────┘  │    │  └─────┬──────┘  └────┬──────┘  │  │  │
│  │  │        │       │    │        │              │           │  │  │
│  │  │  ┌─────▼─────┐  │    │        │              │           │  │  │
│  │  │  │ Virtual-  │  │    │        │              │           │  │  │
│  │  │  │ ization   │◄─┼────┼────────┼──────────────┼───────────┐  │  │
│  │  │  │   SDK     │  │    │        │              │           │  │  │
│  │  │  └─────┬─────┘  │    │        │              │           │  │  │
│  │  └─────────┬───────┘    └────────┴──────────────┴───────────┘  │  │
│  │            │                                                   │  │
│  │  ┌─────────▼──────────────────────────────────────────────┐   │  │
│  │  │              모니터링 라이브러리                        │   │  │
│  │  │                                                        │   │  │
│  │  │  libMonitorArm.dylib          libMonitorX86Emu.dylib   │   │  │
│  │  │  (ARM64 네이티브 HVT)         (x86→ARM JIT 에뮬)      │   │  │
│  │  └───────────────────────────────┬────────────────────────┘   │  │
│  │                                  │                             │  │
│  └──────────────────────────────────┼─────────────────────────────┘  │
│                                      │                               │
│              ┌───────────────────────▼───────────────────────┐       │
│              │         Apple Hypervisor.framework             │       │
│              │         (커널 레벨 하드웨어 가상화)            │       │
│              └───────────────────────┬───────────────────────┘       │
│                                      │                               │
│              ┌───────────────────────▼───────────────────────┐       │
│              │                Guest VM(s)                     │       │
│              │                                                │       │
│              │  Windows 11  │  Linux  │  macOS  │  Chrome OS  │       │
│              └────────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────────────────┘
```

## 데이터 흐름: VM 시작 시

```
사용자 클릭 "Start"
       │
       ▼
┌─────────────────┐
│ prl_client_app  │  ← Qt UI 이벤트
│ (arm64)         │
└────────┬────────┘
         │ PrlApi_CreateVm / Start()
         ▼
┌─────────────────┐
│ Parallels SDK   │  ← libPrlSdkCommunication
│ (Framework)     │
└────────┬────────┘
         │ IPC / Shared Memory
         ▼
┌─────────────────┐          ┌─────────────────┐
│ prl_disp_service│◄────────►│  prl_naptd      │
│ (디스패처)       │          │ (NAT 데몬)       │
└────────┬────────┘          └─────────────────┘
         │
         ▼ (HVT / 에뮬레이션)
┌─────────────────┐
│ libMonitorArm   │  ← Apple Silicon 네이티브 VM
│ (Hypervisor.fw) │
└─────────────────┘
         │
         ▼ (FEX-JIT 재컴파일)
┌─────────────────┐
│ libMonitorX86Emu│  ← x86_64 → ARM64 번역
└─────────────────┘
         │
         ▼
   Guest OS 실행 중...
```

## CLI 도구 계층

```
┌─────────────────────────────────────────────┐
│  사용자 터미널                               │
│                                             │
│  prlctl create myvm --ram 4096 --cpus 4     │
│  prlctl start myvm                          │
│  prlsrvctl status                           │
└──────────────┬──────────────────────────────┘
               │ arm64 바이너리
               ▼
┌─────────────────┐    ┌───────────────────────┐
│   prlctl        │    │   prlsrvctl           │
│  (CLI 도구)     │    │  (서버 컨트롤)         │
└────────┬────────┘    └───────────┬───────────┘
         │                        │
         ▼                        ▼
┌─────────────────┐    ┌───────────────────────┐
│ Parallels SDK   │    │ Parallels Service      │
│ (C API 호출)    │    │ (루트 권한 서비스)      │
└────────┬────────┘    └───────────┬───────────┘
         │                        │
         ▼                        ▼
   Hypervisor.framework    네트워크/장치 접근
```

## 파일 포맷 변환 파이프라인

```
  타사 VM 포맷                    Parallels 네이티브 포맷
┌──────────────┐                 ┌─────────────────┐
│ .vmdk (VMware)│                 │  .pvm           │
│ .vdi (VB)    │── prl_convert ──►│  .pvmz (압축)   │
│ .vhdx (HV)   │   (x86_64)      │  .pvmp (패킹)   │
│ .vmx         │                 └─────────────────┘
│ .vbox        │                         │
└──────────────┘                         ▼
                                  ┌───────────────┐
                                  │  .pvs (스냅)   │
                                  │  .pvsz (압축)  │
                                  └───────────────┘
```

## 권한 분리 모델

```
┌─────────────────────────────────────────────┐
│  일반 사용자 세션                            │
│                                             │
│  prl_client_app (29MB, arm64)               │
│    ├─ Qt 6 UI (QML + QuickControls2)        │
│    ├─ VM 컨테이너 관리                      │
│    └─ 사용자 설정/구성                       │
│                                             │
│  prlctl / prlsrvctl (arm64)                 │
│    └─ 명령줄 인터페이스                      │
└──────────────┬──────────────────────────────┘
               │ (sudo 시)
               ▼
┌─────────────────────────────────────────────┐
│  루트 권한 세션                              │
│                                             │
│  Parallels Service (setuid+setgid)           │
│    ├─ prl_disp_service (VM 디스패처)         │
│    ├─ prl_naptd (NAT 데몬)                   │
│    └─ 네트워크 인터페이스 관리               │
│                                             │
│  prl_update_helper (setuid)                 │
│    └─ 업데이트 권한 승인                     │
└─────────────────────────────────────────────┘
```

## 서브 애플리케이션 구조

```
Parallels Desktop.app/Contents/MacOS/
│
├── prl_client_app              ← 메인 UI (arm64)
├── Parallels Service            ← 시스템 서비스 (setuid arm64)
│
├── Parallels VM.app/            ← VM 컨테이너 (Windows/Linux)
│   └── MacOS/[binary]
├── Parallels Mac VM.app/        ← macOS VM 전용 컨테이너 (Apple Silicon)
│   └── MacOS/[binary]
├── Parallels Mounter.app/       ← 디스크 마운터
│   └── MacOS/[binary]
├── Parallels Link.app/          ← 호스트-Guest 연결 관리
│   └── MacOS/[binary]
├── Parallels Service.app/       ← 서비스 서브 앱
│   └── MacOS/[binary]
```

## 플러그인 확장 구조

```
PlugIns/
│
├── com.parallels.desktop.console.ParallelsMail.appex
│   └─ VM 내 메일 클라이언트 연동 (Outlook, Thunderbird)
│
├── com.parallels.desktop.console.ExeQLPlugin.appex
│   └─ Windows .exe 파일을 macOS에서 직접 실행 (Coherence)
│
├── com.parallels.desktop.console.OpenInIE.appex
│   └─ Internet Explorer 연동
│
└── QtPlugIns/
    ├── platforms/libqcocoa.dylib       ← macOS 플랫폼 통합
    ├── styles/libqmacstyle.dylib       ← macOS 스타일
    ├── sqldrivers/libqsqlite.dylib     ← SQLite
    ├── imageformats/                   ← GIF, JPEG, TIFF
    └── quick/                          ← QML 그래픽 효과
```
