# virtual-os — REA 기반 Parallels Desktop 27 정적 분석 보고서

> **분석 대상**: `/Applications/Parallels Desktop.app`
> **버전**: 27.0.3 (Build 58680) · CFMajorVersion 27
> **Bundle ID**: `com.parallels.desktop.console`
> **Team ID**: `4C6364ACXT` (Parallels International GmbH)
> **CDHash**: `4deab891e777de487f594f02e0d1c72461bbc81d`
> **최소 OS**: macOS 14.0 (Sonoma) · SDK 26.0 · arm64 전용
> **분석 도구**: REA(rea-agents) 6.5.0 — `native-macos` + `rea-artifact-graph` provider
> **분석일**: 2026-11-05

---

## 1. 요약 (Executive Summary)

Parallels Desktop 27.0.3은 **Apple Silicon 전용** 상용 하이퍼바이저다. 메인 UI부터
서비스, CLI까지 모든 1차 실행 파일이 **arm64 thin**이며, x86_64 슬라이스는 일부
프레임워크와 구형 변환 도구(`prl_disk_tool`, `prl_convert`, `libDaApiWrap`)에만 남아 있다.
따라서 "Intel Mac + Rosetta 2"로 동작한다는 통념은 이 버전에는 적용되지 않는다.

핵심 구조는 다음 네 층으로 정리된다.

| 계층 | 구성 요소 | 확인된 사실 |
|------|-----------|-------------|
| **UI 클라이언트** | `prl_client_app` (29.4 MB, arm64) | Qt **5.15.1** Widgets 기반 C++ 앱. WebKit/SafariServices(SSO), LocalAuthentication, PDFKit, QuickLookUI, Metal 연동 |
| **권한 상승 체인** | LaunchDaemon → `launchd_wrapper` → `launchd_user_wrapper` → `Parallels Service`(setuid 6755 root:accessibility) → `prl_disp_service` | 위치 파일(`parallels-desktop.loc`)과 번들 소유자/모드 검증 후 root 서비스 기동 |
| **가상화 서비스** | `prl_disp_service` (7.9 MB, arm64, `com.parallels.desktop.dispatcher`) | VM·디스크·네트워크·USB·라이선스를 총괄하는 상주 데몬. Hypervisor/vmnet/PCSC/CryptoTokenKit 링크 |
| **VM 실행기** | `prl_vm_app` (13.8 MB) · `prl_macvm_app` (2.7 MB) | `prl_vm_app`은 Hypervisor.framework + `libMonitorArm`(HVT) / `libMonitorX86Emu`(FEX JIT). `prl_macvm_app`은 Apple Virtualization.framework로 macOS 게스트 구동 |

**가장 중요한 정정**: 실제 VM을 실행하는 프로세스는 `prl_client_app`이 아니라
`prl_vm_app`이며, 하이퍼바이저 entitlement(`com.apple.security.hypervisor`,
`com.apple.vm.device-access`, `com.apple.vm.networking`,
`com.apple.security.cs.allow-unsigned-executable-memory`)를 보유한 것도 이 프로세스다.
UI 앱은 `com.apple.security.virtualization`만 갖는다.

---

## 2. 분석 방법과 한계

### 2.1 사용한 REA 연산

| 연산 | 목적 | Evidence |
|------|------|----------|
| `inspect_artifact` | 번들 전체 아티팩트 그래프(831 노드 / 972 항목 / 892 엣지) | `ev_a19f02f0…` |
| `list_architectures` | 번들 아키텍처 확인 (arm64 단일) | `ev_f834c8b8…` |
| `inspect_macho` ×17 | 세그먼트·의존성·import/export·빌드 메타데이터 | 부록 A |
| `inspect_signature` ×15+ | codesign 신원·CDHash·hardened runtime·entitlement | 부록 A |
| `inspect_plist` ×12 | 중첩 앱/익스텐션/프레임워크 메타데이터 | 부록 A |

전체 Evidence ID와 다이제스트는 [`analysis/evidence-index.md`](analysis/evidence-index.md)에 있다.

### 2.2 한계 (명시)

- **디스어셈블러 미설치**: Hopper/Ghidra/IDA가 없어 `decompile`·`search`·`xrefs`
  같은 심층 연산은 수행하지 않았다. 따라서 함수 수준 로직, DRM 알고리즘, IPC
  프로토콜 내부는 **미확인**이다.
- **문자열 기반 추론**: 아래에 `(문자열 추론)`으로 표시한 항목은 REA Evidence가 아니라
  바이너리 내 `strings` 스캔에서 얻은 단서다. 구조적 사실(파일·아키텍처·의존성·서명·plist)은
  REA Evidence로 확인했다.
- **런타임 미관찰**: 조사 시점에 Parallels 프로세스가 없었고, 시스템 확장 0개,
  `/Library/Parallels`에는 `launchd_wrapper`만 존재했다. 즉 **앱이 아직 1회 초기화되지 않은 상태**다.
  서비스 설치·VM 부팅·네트워크 동작은 실행 관찰로 검증하지 않았다.

---

## 3. 대상 식별과 무결성

| 항목 | 값 | 출처 |
|------|-----|------|
| 코드 서명 | Developer ID Application: Parallels International GmbH (4C6364ACXT) | `codesign`/REA |
| Hardened Runtime | 예 (메인 앱·서비스·CLI) | `ev_6ba40a4b…` |
| 공증(Notarization) | `spctl`: **accepted**, `source=Notarized Developer ID` | `spctl -a -vvv` |
| Stapled ticket | 없음 (온라인 공증 확인 방식) | `stapler validate` |
| Provisioning | `Parallels Desktop for Mac Distribution`, 만료 2044-07-03 | `embedded.provisionprofile` |
| 번들 크기 | 약 348 MB | `inspect_artifact` |
| Mach-O | 84개 (실행 파일 36 / 공유 라이브러리 47 / 로더블 번들 1) | 인벤토리 |
| PE | 15개 (게스트 도구·부트로더) | 인벤토리 |
| 중첩 앱 | 5개 `.app`, 1개 `.xpc`, 3개 `.appex` | 인벤토리 |

`inspect_artifact`는 심볼릭 링크 79개를 따라가지 않고 기록만 했으며(제한 사항에 명시),
중첩 ISO(게스트 도구)는 내부까지 확장하지 않았다.

---

## 4. 구성 요소 인벤토리

### 4.1 1차 실행 파일

| 파일 | 크기 | arch | 서명 ID | Hardened | 역할 |
|------|------|------|---------|----------|------|
| `Contents/MacOS/prl_client_app` | 29.4 MB | arm64 | `com.parallels.desktop.console` | ✔ | 메인 UI |
| `Parallels VM.app/…/prl_vm_app` | 13.8 MB | arm64 | `com.parallels.vm` | ✔ | **VM 실행기** |
| `Parallels Service.app/…/prl_disp_service` | 7.9 MB | arm64 | `com.parallels.desktop.dispatcher` | ✔ | 상주 서비스 |
| `Parallels Mac VM.app/…/prl_macvm_app` | 2.7 MB | arm64 | `com.parallels.macvm` | ✔ | macOS 게스트(가상화 프레임워크) |
| `prl_convert` | 1.7 MB | **x86_64** | `com.parallels.prl-convert` | ✔ | 타사 VM 변환 |
| `prlctl` / `prlsrvctl` | 687 KB | arm64 | `…console.prlctl` / `…prlsrvctl` | ✔ | CLI |
| `prl_naptd` | 537 KB | arm64 | `com.parallels.naptd` | ✔ | NAT/네트워크 |
| `prl_disk_tool` | 505 KB | **x86_64** | `com.parallels.disktool` | ✔ | 디스크 도구 |
| `prl_updater_ctl` | 292 KB | arm64 | — | ✔ | 업데이트 제어 |
| `prl_update_helper` | 90 KB | arm64 | `…console.common` | ✔ | **setuid root 업데이터** |
| `Parallels Service` | 88 KB | arm64 | `…console.common` | ✔ | **setuid 6755 런처 스텁** |
| `7z` + `7z.dylib` | 438 KB + 1.6 MB | arm64 | `…console.common` | — | 아카이브 |
| `prl_vmarchiver` / `prl_osimage_tool` / `prl_net_start` / `prl_bglaunch` / `prl_esd2iso` / `prl_ps2pdf` / `prl_perf_ctl` / `prl_switchspaces` / `prllog` / `prl_ls_users` | 70–532 KB | arm64 | 다양 | 대부분 ✔ | 보조 도구 |

### 4.2 중첩 번들

| 번들 | Bundle ID | 성격 |
|------|-----------|------|
| `Parallels VM.app` | `com.parallels.vm` | VM 워커 + `XPCMounter.xpc`(`com.parallels.xpcmounter`) |
| `Parallels Service.app` | `com.parallels.desktop.dispatcher` | 상주 데몬 |
| `Parallels Mac VM.app` | `com.parallels.macvm` | macOS 게스트 전용 |
| `Parallels Mounter.app` | `com.parallels.desktop.mounter` | 디스크 이미지 마운터. `*.*` 뷰어로 등록(Alternate) |
| `Parallels Link.app` | `com.parallels.SharedAppLink.Agent` | Windows `.lnk`/`plinkAg`/`plinkFe` 핸들러, LSUIElement |

### 4.3 확장(PlugIns)

| 확장 | 확장 포인트 | 확인된 동작 |
|------|-------------|-------------|
| `com.parallels.desktop.console.ExeQLPlugin` | Quick Look thumbnail | Windows 실행 파일(`com.microsoft.windows-executable`) 썸네일 |
| `com.parallels.desktop.console.OpenInIE` | Safari extension | 툴바 버튼 라벨 **"Open current page in Edge"**, 허용 도메인 `*.webkit.org` |
| `com.parallels.desktop.console.ParallelsMail` | Share services | 파일 URL 1개 공유 시 처리 |

### 4.4 프레임워크 / 라이브러리

- **ParallelsVirtualizationSDK** v11 (`com.parallels.virtualization.sdk`, universal): C API 1,268개 `Prl*` 심볼.
- `libPrlGui.3` (universal, export 3,994): Qt Widgets 기반 UI 클래스(`C…Window/Dialog/View`).
- `libPrlXmlModel.1` (universal, export 15,111): VM 설정 모델.
- `libPrlSdkCommunication.1`, `libAbstractTask.1`, `libDaApiWrap.1`(x86_64), `libprl_mspack`, `libwimlib.1`,
  `PDCocoaKit`, `PDControlCenter`, `ProxyAppCore`.
- **Qt 5.15.1** 17개 프레임워크(universal) + QtPlugIns(cocoa, qmacstyle, qsqlite, 이미지 포맷, Quick 계열).
- `libOSMesa.8`(14.9 MB, 소프트웨어 OpenGL), `libmetalirconverter.dylib`(86 MB, Metal IR 변환).
- `libMonitorArm.dylib`(1.0 MB), `libMonitorX86Emu.dylib`(3.6 MB).

---

## 5. 실행·권한 상승 체인

정적 증거(plist, setuid 모드, 문자열)로 재구성한 기동 경로다.

```
launchd (root)
  └─ /Library/LaunchDaemons/com.parallels.desktop.launchdaemon.plist   (문자열 추론)
       └─ /Library/Parallels/Parallels Desktop/launchd_wrapper          (root:wheel 0755)
            ├─ /Library/Preferences/Parallels/parallels-desktop.loc 읽기
            ├─ 번들 소유자/모드 검증 + /Applications 하위인지 확인
            ├─ console user로 setreuid/setregid/initgroups (권한 강하)
            └─ Contents/MacOS/launchd_user_wrapper
                 └─ Contents/MacOS/Parallels Service  "start"          (setuid 6755 root:accessibility)
                      ├─ 위치 파일과 실제 번들 경로 일치 검증
                      ├─ bootstrap_get_root / bootstrap_look_up_per_user
                      └─ /bin/bash service-wrap …
                           └─ <AppBundle>/Contents/MacOS/Parallels Service.app/Contents/MacOS/prl_disp_service
```

- `exceptions.list`(번들 리소스)는 setuid 대상 두 개를 명시한다:
  `Parallels Service` → `root:accessibility 6755`, `prl_update_helper` → `root:wheel 4755`.
- `watchdog`(bash)는 PID 파일 `/var/run/prl_watchdog-<md5>.pid`로 자식 프로세스를 재시작한다.
- 업데이트는 `prl_update_helper`가 `/Library/Parallels/Parallels Desktop/Updater`의
  소유자·권한을 검사하고, 코드 서명 요구사항(`anchor apple generic`,
  `certificate leaf[subject.OU] = "4C6364ACXT"`)을 만족하는 번들만 복사하며 다운그레이드를 거부한다.

> **평가**: 위치 파일 + 소유자/모드 + 코드서명 검증을 조합한 전형적인 방어적 setuid 설계다.
> 다만 root로 실행되는 `launchd_wrapper`가 사용자 영역 경로(`/Applications`)의 번들을
> 신뢰 경계로 사용하므로, 위치 파일과 번들 디렉터리의 무결성(소유자/권한)이 깨질 경우의
> 위협 모델은 별도 검토가 필요하다. (추론)

---

## 6. 가상화 스택

### 6.1 ARM 게스트 — `libMonitorArm`

- `Hypervisor.framework`에 직접 링크.
- 문자열에서 `HVT: hv_vm_map`, `hv_vm_unmap`, `hv_gic_get_icc_reg`, `ArmPl011`(UART),
  `ArmToolsGate`, `ArmVideo`, `VirtualPlatformArm` 확인 → **Apple HVF 기반 ARM64 VM 모니터**.
- GIC/PL011/도구 게이트를 자체 에뮬레이션.

### 6.2 x86_64 게스트 — `libMonitorX86Emu`

- `FEXJIT`, `FEXCore::CPU::InterpreterCore`, `FEX` 문자열 확인 → **FEX-Emu 계열 JIT**.
- 모듈 문자열: `Cpu-64/{Decode,Emulation,ArithmeticOps,ControlOps,CpuidOp,LocalAPIC,SegmentOps,StringOps,SwitchLevel}`,
  `Mainboard/{Cmos,Dma,Acpi/Nvs}`, `Comm/{Serial,ParPort}`, `Input/KbdMou` → PC 하드웨어 에뮬레이션 포함.
- `Hypervisor.framework`에도 링크되어 호스트 가상화 보조 기능을 사용하는 것으로 보인다(추론).

### 6.3 macOS 게스트 — `prl_macvm_app`

- `Virtualization.framework` + `vmnet.framework` 링크. entitlement: `virtualization`,
  `vm.device-access`, `vm.networking`. ARM 네이티브 macOS VM 전용 경로.

### 6.4 그래픽·주변기기

`prl_vm_app` 의존성: `Metal`, `OpenGL`, `IOSurface`, `CoreVideo`, `CoreMedia`, `AVFoundation`,
`AudioUnit`, `CoreAudio`, `GameController`, `CoreHaptics`, `PCSC`(스마트카드), `vmnet`.
`libOSMesa.8`은 3D 가속이 없을 때의 소프트웨어 GL 폴백으로 보인다(추론).

---

## 7. IPC·네트워크

### 7.1 내부 IPC (문자열 추론)

| 이름 | 사용처 |
|------|--------|
| `com.parallels.disp.net.ipc` | 디스패처 ↔ 네트워크 |
| `com.parallels.naptd.ipc` | NAT 데몬 |
| `com.parallels.vm.prl_naptd` | VM ↔ NAT |
| `com.parallels.prl_net_library` | 네트워크 라이브러리 |
| `com.parallels.desktop.managedprefs` | 관리형 환경설정(MDM) |
| `com.parallels.kext.{hypervisor,netbridge,vnic}` | 레거시 kext 식별자(현재 미설치) |

### 7.2 외부 엔드포인트 (문자열 추론)

`%1`은 지역별 도메인 치환자다(예: `parallels.com`, `prls.net`).

| 엔드포인트 | 추정 용도 |
|------------|-----------|
| `https://access.%1/license/api/v1`, `/agent_api/v1` | 라이선스/에이전트 API |
| `https://pda-api.%1/api` | Parallels Desktop Activation API |
| `https://account.%1/api/v1/webinstaller` | 계정·웹 설치 |
| `https://my.%1/support/%2`, `https://my.parallels.com` | 지원/SSO (`webcredentials:` 연동 도메인) |
| `https://reportus.%1/api`, `https://reportus.prls.net/webapp/reports/%1` | 텔레메트리/문제 보고 |
| `https://desktop.%1`, `https://static.parallels.com/i`, `https://update.parallels.com` | 배포·업데이트 |
| `https://kb/support/forum.parallels.com` | 지원 문서 |
| `https://fe3.delivery.mp.microsoft.com/UpdateMetadataService/...` | Windows 업데이트 메타데이터 조회 |
| `http://protexis.com/webservices/DownloadAuthorizationRequest` | 레거시 Protexis DRM 잔재 |

앱과 서비스 모두 **ATS 예외**(`NSAllowsArbitraryLoads = true`)를 선언한다.

### 7.3 등록 URL 스킴

메인 앱: `http`, `https`, `pdfm`(SSO 콜백). `http`/`https`를 문서 타입으로 등록하는 것은
Coherence/브라우저 연동용으로 보이나, 기본 브라우저 핸들러 등록 여부는 실행 관찰이 필요하다(미확인).

---

## 8. 라이선스·DRM

| 관찰 | 근거 |
|------|------|
| `License.ver` = `4.0.0.parallels` | 번들 리소스 |
| `CDspLicenseChecker`, `Task_DeactivateLicense`, `Task_DspLicenseHelper`, `N5Utils8Licenses16DeactivationTaskE` | 서비스 심볼/문자열 |
| `AuthorizationAccessKey`, `AuthorizationKaPdeKey`, `AuthorizationKaPdeInstanceKeyNumber`, `AuthorizationMasterLicKey`, `AuthorizationSubscriptionUuidKey`, `AuthorizationTemporaryLicKey` | 서비스 문자열 |
| `PLM_ROOT_ACCOUNT`, `Admin license monitor`, `Admin portal (app %s) sync VMs` | 관리형/기업 라이선스 모니터링 |
| `Protects`/`protexis.com` | 레거시 DRM 흔적 |

라이선스 토큰은 `access.*` API와 `pda-api.*`를 통해 교환되는 구조로 보이며,
키 종류가 Access/KaPde/Master/Subscription/Temporary로 분리되어 있다.
**알고리즘·서명 검증 방식은 미확인**(디컴파일 필요).

---

## 9. 보안 관점 정리

상세 내용은 [`analysis/security-review.md`](analysis/security-review.md) 참조.

1. **setuid 표면**: `Parallels Service`(6755) + `prl_update_helper`(4755). 둘 다 번들 위치/소유자/
   권한/서명 검증 로직을 포함하지만, 검증 실패 시 동작과 TOCTOU 가능성은 미확인.
2. **Hardened Runtime 예외**: 프레임워크·Qt·`libMonitor*`는 hardened runtime이 꺼져 있다.
   `prl_vm_app`만 `allow-unsigned-executable-memory`(JIT) 예외를 가진다.
3. **ATS 비활성**: 클라이언트와 디스패처 모두 `NSAllowsArbitraryLoads`.
4. **광범위한 entitlement**: 카메라, 마이크, 주소록, Apple Events 자동화, 키체인 그룹,
   associated domains(my.parallels.com).
5. **파일 핸들러 범위**: Mounter가 `*.*` 뷰어로 등록, 클라이언트가 `http`/`https` 등록.
6. **시스템 확장 없음**: 번들에 `.dext`/`.systemextension` 없음, 호스트에도 0개.
   가상화는 Hypervisor/Virtualization/vmnet 프레임워크에 의존한다.

---

## 10. 이전 보고서 대비 정정 사항

| 이전 기술 | 실제 (Evidence) |
|-----------|-----------------|
| Qt 6 / QuickControls2 기반 모던 UI | **Qt 5.15.1**. UI는 C++ Qt Widgets 중심, QML은 일부 컴포넌트(`CDeclarativeWizardPage`, `CQtViewController`)에만 사용. 번들 QML 105개는 전부 Qt 기본 모듈 |
| macOS 14 = Sequoia | macOS 14는 **Sonoma**. Sequoia는 15 |
| Intel Mac은 Rosetta 2로 동작 | 메인 실행 파일이 arm64 thin → **Intel 호스트 미지원**. x86_64는 `prl_disk_tool`, `prl_convert`, `libDaApiWrap` 등 일부만 |
| `prl_disp_service` 미기재 / "VM Dispatcher" | 7.9 MB 핵심 상주 데몬(`com.parallels.desktop.dispatcher`). VM뿐 아니라 라이선스·네트워크·USB·디스크를 총괄 |
| `prl_client_app`이 VM 컨테이너/디스플레이 담당 | VM 실행은 **`prl_vm_app`**. 하이퍼바이저 entitlement도 `prl_vm_app` 소유 |
| `Parallels Service`(setuid)가 디스패처 | setuid 바이너리는 **런처 스텁**이며, 검증 후 `prl_disp_service`를 기동 |
| Hot Migration, Chrome OS/Solaris 지원 | 정적 증거에서 확인되지 않음 → **미확인 항목으로 제외** |

---

## 11. 다음 단계 (권장)

정적 구조 분석은 완료 수준이나, 함수 로직은 아직 열리지 않았다.

1. **Ghidra 12.1.x 설치 후** `prl_disp_service`(라이선스/IPC), `prl_update_helper`(서명 검증),
   `Parallels Service`(위치 검증) 순으로 디컴파일.
2. `libMonitorX86Emu`에서 FEX 버전·JIT 캐시 정책·`allow-unsigned-executable-memory` 사용 범위 확인.
3. 앱 1회 초기화 후 `/Library/LaunchDaemons/com.parallels.desktop.launchdaemon.plist`,
   `/Library/Parallels/Parallels Service.app` 설치 결과와 실제 IPC를 런타임 관찰.
4. 라이선스 API(`access.*`, `pda-api.*`) 요청/응답을 네트워크 캡처로 확인.

---

## 12. 산출물

| 문서 | 내용 |
|------|------|
| [`architecture.md`](architecture.md) | 컴포넌트·기동 체인·데이터 경로 다이어그램 |
| [`sdk-api-reference.md`](sdk-api-reference.md) | 실제 export 1,268개 기반 SDK 레퍼런스 |
| [`analysis/evidence-index.md`](analysis/evidence-index.md) | Evidence ID·다이제스트·도구·한계 |
| [`analysis/security-review.md`](analysis/security-review.md) | 권한·서명·네트워크·프라이버시 상세 |

---

*정적 분석 결과이며, 명시되지 않은 런타임 동작은 확인되지 않았다. 문자열 기반 추론은 본문에 "(문자열 추론)"으로 표기했다.*
