# 보안 검토 — Parallels Desktop 27.0.3

> **대상**: `/Applications/Parallels Desktop.app` (27.0.3, build 58680, arm64)
> **방법**: REA 정적 분석(`inspect_macho`, `inspect_signature`, `inspect_plist`) + 번들 리소스 직접 확인 + `strings`
> **기준일**: 2026-11-05
> **표기 규칙**
> - **관찰**: REA Evidence 또는 파일에서 직접 확인한 사실
> - **추론**: 관찰된 사실에서 도출한 해석·위험 평가
> - **미확인**: 정적 증거만으로 판단할 수 없어 런타임/디컴파일이 필요한 항목
>
> 상세 Evidence ID는 [`analysis/evidence-index.md`](evidence-index.md)에 있다.

---

## 1. 요약

| # | 항목 | 판정 | 근거 유형 |
|---|------|------|-----------|
| 1 | 코드 서명·공증 | Developer ID + 공증 accepted, 단 **stapled ticket 없음** | 관찰 |
| 2 | setuid 바이너리 | 2개 (`Parallels Service` 6755, `prl_update_helper` 4755) | 관찰 |
| 3 | 권한 상승 경로 | 위치 파일 + 번들 소유자/모드 + 코드서명 요구사항의 3중 검증 | 관찰(문자열) |
| 4 | Hardened Runtime | 주요 실행 파일은 적용, **프레임워크/모니터 dylib는 미적용** | 관찰 |
| 5 | JIT 예외 | `prl_vm_app`만 `allow-unsigned-executable-memory` | 관찰 |
| 6 | ATS | 메인 앱·디스패처 모두 `NSAllowsArbitraryLoads = true` | 관찰 |
| 7 | entitlement 범위 | 카메라·마이크·주소록·Apple Events·키체인·앱 그룹 | 관찰 |
| 8 | 파일 핸들러 | Mounter가 `*.*`를 Alternate 뷰어로 등록 | 관찰 |
| 9 | 시스템 확장 | `.dext`/`.systemextension` **0개** | 관찰 |
| 10 | 런타임 검증 | 서비스 설치·VM 부팅·네트워크 요청 **미관찰** | 미확인 |

전체적으로 **방어적 setuid 설계**(위치 고정 + 소유자/모드 + 서명 요구사항)를 갖추고 있으나,
검증 실패 시의 동작과 TOCTOU, ATS 비활성, 광범위한 entitlement는 별도 확인이 필요하다.

---

## 2. 서명·공증 기준선

### 2.1 서명 identity (관찰)

| 항목 | 값 | Evidence |
|------|-----|----------|
| Authority | `Developer ID Application: Parallels International GmbH (4C6364ACXT)` | `ev_6ba40a4b…` |
| 중간 CA | `Developer ID Certification Authority` → `Apple Root CA` | 동일 |
| Team ID | `4C6364ACXT` | 동일 |
| 메인 앱 CDHash | `4deab891e777de487f594f02e0d1c72461bbc81d` | `ev_6ba40a4b…` |
| Designated Requirement | `identifier "…" and anchor apple generic and certificate leaf[subject.OU] = "4C6364ACXT"` | 동일 |

모든 1차 실행 파일과 번들 라이브러리가 동일 Team ID로 서명되어 있다.
`libmetalirconverter.dylib`만 identifier가 `com.apple.libmetalirconverter`이지만
서명 authority는 동일하다(관찰, `ev_58b0f493…`).

### 2.2 공증 (직접 확인 — REA Evidence 아님)

| 항목 | 결과 |
|------|------|
| `spctl -a -vvv` | `accepted`, `source=Notarized Developer ID` |
| `stapler validate` | stapled ticket 없음 |
| Provisioning Profile | `Parallels Desktop for Mac Distribution`, 만료 **2044-07-03**, App ID `4C6364ACXT.com.parallels.desktop.console` |

**추론**: stapled ticket이 없으므로 최초 실행 시 온라인 공증 확인(Gatekeeper)에 의존한다.
오프라인 환경에서 첫 실행이 차단될 수 있다.

### 2.3 Hardened Runtime 분포 (관찰)

`inspect_signature` 79건 기준:

| 그룹 | Hardened | 비고 |
|------|----------|------|
| 1차 실행 파일 (`prl_client_app`, `prl_disp_service`, `prl_vm_app`, CLI, setuid 2종) | ✔ | `ev_6ba40a4b…`, `ev_00f90db3…`, `ev_376defa0…` |
| 앱 확장 3종 | ✔ | 샌드박스 entitlement 포함 |
| Parallels 프레임워크·dylib (`libPrlGui`, `libPrlXmlModel`, `libMonitor*` 등) | ✘ | 로더 프로세스의 정책을 따름 |
| Qt 프레임워크 17종 + QtPlugIns | ✘ | 서드파티 바이너리 |

**추론**: dylib 자체에 hardened runtime이 없어도, 이를 로드하는 프로세스가 hardened이면
프로세스 수준 보호는 유지된다. 다만 `prl_vm_app`이 `allow-unsigned-executable-memory`를
갖고 `libMonitorX86Emu`(FEX JIT)를 로드하므로, **JIT 코드 페이지 생성 경로**가 이 앱의
주요 메모리 무결성 예외다.

---

## 3. 권한 상승 표면

### 3.1 setuid/setgid 목록 (관찰 — `Contents/Resources/exceptions.list` 직접 확인)

```
( "Contents/MacOS/Parallels Service"  "root"  "accessibility"  "6755" )
( "Contents/MacOS/prl_update_helper"  "root"  "wheel"          "4755" )
```

실제 파일 모드도 `find`로 확인했다: `Parallels Service`는 `setuid,setgid` arm64,
`prl_update_helper`는 `setuid` arm64 (관찰).

### 3.2 기동 체인 (관찰 + 문자열 추론)

```
launchd (root)
  └─ /Library/LaunchDaemons/com.parallels.desktop.launchdaemon.plist
       ProgramArguments = /Library/Parallels/Parallels Desktop/launchd_wrapper
       RunAtLoad = true, ProcessType = Interactive, ExitTimeOut = 150
       └─ launchd_wrapper (root:wheel 0755)
            ├─ /Library/Preferences/Parallels/parallels-desktop.loc 읽기
            ├─ "Malformed location", "Unsupported location", "Locations mismatch" 검사
            ├─ 번들 stat: "Bundle has incorrect owner", "Bundle has incorrect mode"
            ├─ console user 확인: "No user logged in - running as root"
            ├─ setreuid() / setregid() / initgroups()
            └─ Contents/MacOS/launchd_user_wrapper
                 └─ Contents/MacOS/Parallels Service "start"   (6755 root:accessibility)
                      ├─ 위치 파일과 실제 경로 일치 검증
                      ├─ /var/run/prl_desktop_services.lock (shlock)
                      ├─ bootstrap_get_root / bootstrap_look_up_per_user
                      └─ prl_disp_service --pidfile /var/run/prl_disp_service.pid
```

- launchd plist 템플릿은 번들 내부(`Parallels Service.app/Contents/Resources/`)에 존재하며,
  설치 시 `/Library/LaunchDaemons/`로 복사되는 구조다(관찰: 템플릿 파일, 추론: 복사 시점).
- `launchd_wrapper`는 **/Applications 하위 번들을 신뢰 경계로 사용**한다.
  위치 파일과 번들 디렉터리의 소유자/모드가 검증되지만, `/Applications`는 관리자 권한으로
  쓰기 가능하므로 **로컬 관리자 → root** 경로가 성립할 수 있다(추론, 설계상 허용된 범위).
- `Parallels Service` 스텁은 `prl_disp_service`를 직접 exec하지 않고, 내장 셸 로직으로
  포그라운드 락/launchd 모드를 판별한 뒤 기동한다(문자열 관찰).
- `watchdog`(bash)는 `/var/run/prl_watchdog-<md5>.pid`로 자식을 재시작하며,
  NAT 데몬은 `NatWatchdogTimeout=60`, `NatWatchdogLimit=20`으로 감시된다(직접 확인).

### 3.3 업데이터 (관찰 — 문자열)

`prl_update_helper`에서 확인된 문자열:

- `/Library/Parallels/Parallels Desktop/Updater`
- `Updater dir permissions check failed`
- `Failed to remove updater dir`
- `Downgrade not allowed: %d => %d`
- `anchor apple generic`
- `codesign` 요구사항의 OU(`4C6364ACXT`) 검증

**추론**: 업데이트는 (1) Updater 디렉터리 소유자/권한 확인, (2) 번들 코드서명 요구사항 확인,
(3) 버전 다운그레이드 거부의 3단계를 거친다. 이는 전형적인 방어적 설계다.
다만 서명 검증이 `codesign --verify` 수준인지, 공증/팀 ID/식별자까지 결합하는지는
**미확인**(디컴파일 필요).

### 3.4 위협 모델 메모

| 시나리오 | 관찰된 방어 | 남는 질문 |
|----------|-------------|-----------|
| 일반 사용자가 setuid 스텁을 직접 실행 | 위치 파일 일치 + 번들 소유자/모드 검증 | 검증 실패 시 exit code/로그, 우회 가능성 |
| 번들 디렉터리 교체(관리자 권한) | 소유자/모드 + 서명 요구사항 | TOCTOU(검증 후 exec 사이 교체) |
| 업데이터 디렉터리 사전 생성 | 권한 검사 + 서명 검증 | symlink/race 처리 |
| 악성 `.pvm` 번들 열기 | 코드 서명은 앱에만 적용 | VM 설정 파서의 메모리 안전성 |

TOCTOU와 파서 안전성은 정적 문자열만으로 판단할 수 없다 → **미확인**.

---

## 4. Entitlement 최소권한 분석 (관찰)

### 4.1 메인 앱 `prl_client_app` — `ev_6ba40a4b…`

| Entitlement | 값 | 평가 |
|-------------|-----|------|
| `com.apple.security.virtualization` | true | UI 앱이 Virtualization.framework 사용 |
| `com.apple.security.automation.apple-events` | true | 다른 앱 자동화 — **권한 큼** |
| `com.apple.security.device.camera` | true | Coherence/비디오 회의 연동 |
| `com.apple.security.device.audio-input` | true | 마이크 전달 |
| `com.apple.security.personal-information.addressbook` | true | 주소록 접근 |
| `com.apple.security.application-groups` | `…appstore`, `…Desktop`, `…appbundles` | 확장/헬퍼와 데이터 공유 |
| `keychain-access-groups` | `4C6364ACXT.com.parallels.desktop.passwords` | VM/계정 자격증명 저장 |
| `com.apple.developer.associated-domains` | `webcredentials:my.parallels.com`, `…myparallels.com` | SSO 웹 자격증명 공유 |

**추론**: 단일 UI 프로세스에 카메라·마이크·주소록·Apple Events가 모두 부여되어 있다.
기능상 필요하지만, UI 프로세스가 침해될 경우 **권한 집중** 위험이 있다.
Apple Events 자동화는 TCC 프롬프트로 사용자 동의를 받지만, 한 번 승인되면 범위가 넓다.

### 4.2 VM 실행기 `prl_vm_app` — `ev_376defa0…`

| Entitlement | 값 | 평가 |
|-------------|-----|------|
| `com.apple.security.hypervisor` | true | HVF 사용 |
| `com.apple.security.cs.allow-unsigned-executable-memory` | true | **JIT/FEX용**, 메모리 무결성 예외 |
| `com.apple.vm.device-access` | true | PCI/USB 등 장치 패스스루 |
| `com.apple.vm.networking` | true | vmnet |
| `com.apple.security.application-groups` | `…appbundles` | 앱 번들 공유 |
| `com.apple.application-identifier` | `4C6364ACXT.com.parallels.desktop.console` | **번들 ID와 불일치**(관찰) |

**관찰**: `prl_vm_app`의 서명 identifier는 `com.parallels.vm`인데,
`application-identifier` entitlement는 메인 앱 ID(`…console`)를 사용한다.
**추론**: 앱 그룹/컨테이너 공유를 위한 의도적 설정으로 보이나, 검증 로직이
entitlement의 application-identifier를 신뢰한다면 혼동 가능성이 있다(미확인).

### 4.3 macOS 게스트 `prl_macvm_app` — `ev_cf5cf98c…`

```
com.apple.security.app-sandbox            = false   (명시적 false)
com.apple.security.virtualization         = true
com.apple.vm.device-access                = true
com.apple.vm.networking                   = true
```

**관찰**: `app-sandbox`를 명시적으로 `false`로 선언한다. Virtualization.framework 기반
macOS 게스트는 샌드박스 없이 동작한다.

### 4.4 디스패처 `prl_disp_service` — `ev_00f90db3…`

```
com.apple.security.smartcard = true
```

**관찰**: root 상주 데몬임에도 entitlement는 스마트카드 하나뿐이다.
파일·네트워크·USB 접근은 entitlement가 아니라 **root 권한과 프레임워크**로 수행한다.
따라서 이 프로세스의 공격면은 entitlement가 아니라 **root로 실행되는 코드 자체**다.

### 4.5 앱 확장 3종 (관찰)

| 확장 | Sandbox | 추가 entitlement |
|------|---------|------------------|
| `ExeQLPlugin` | true | `files.user-selected.read-only` |
| `OpenInIE` | true | `application-groups(…Desktop)`, `files.user-selected.read-only` |
| `ParallelsMail` | true | `files.user-selected.read-only` |

확장은 모두 샌드박스화되어 있어 최소권한 원칙에 부합한다(관찰).

---

## 5. 네트워크 보안

### 5.1 ATS (관찰)

| plist | `NSAppTransportSecurity` | Evidence |
|-------|--------------------------|----------|
| 메인 앱 | `NSAllowsArbitraryLoads = true` | `ev_80877cc6…` |
| 디스패처 | `NSAllowsArbitraryLoads = true` | `ev_fe534773…` |

**추론**: HTTPS 강제가 해제되어 있다. 라이선스/업데이트 API가 HTTP로 폴백하거나
사내 프록시 인증서를 허용하기 위한 것으로 보이나, **평문 통신·약한 TLS 허용 가능성**은
런타임 캡처 없이 확정할 수 없다(미확인).

### 5.2 외부 엔드포인트 (문자열 추론)

| 엔드포인트 | 추정 용도 | 민감도 |
|------------|-----------|--------|
| `https://access.%1/license/api/v1`, `/agent_api/v1` | 라이선스 교환 | 높음 (토큰) |
| `https://pda-api.%1/api` | Desktop Activation | 높음 |
| `https://account.%1/api/v1/webinstaller` | 계정·웹 설치 | 중간 |
| `https://my.%1/support/%2`, `https://my.parallels.com` | 지원/SSO | 중간 |
| `https://reportus.%1/api`, `https://reportus.prls.net/webapp/reports/%1` | 텔레메트리/문제 보고 | 중간 (시스템 정보) |
| `https://desktop.%1`, `https://static.parallels.com/i` | 배포/정적 리소스 | 낮음 |
| `https://update.parallels.com` | 업데이트 | 높음 (무결성) |
| `https://fe3.delivery.mp.microsoft.com/UpdateMetadataService/...` | Windows Update 메타데이터 | 낮음 |
| `http://protexis.com/webservices/DownloadAuthorizationRequest` | 레거시 DRM | **평문 HTTP** |

**관찰**: `protexis.com` 항목만 `http://`다. **추론**: 레거시 코드 경로이며 실제 호출 여부는
미확인. 만약 활성화되어 있다면 다운로드 권한 토큰이 평문으로 오갈 수 있다.

### 5.3 인증서 검증

- TLS 피닝, 공개키 고정, 서버 인증서 검증 커스터마이즈 여부: **미확인**(디컴파일 필요).
- ATS 예외가 선언되어 있으므로, 라이브러리(`CFNetwork`/Qt Network)가 기본 검증을 수행하더라도
  정책상 제약이 약하다.

---

## 6. 프라이버시·데이터 접근

### 6.1 Usage description (관찰 — `Info.plist`)

| 키 | 문구 요지 |
|----|-----------|
| `NSCameraUsageDescription` | 가상 머신에서 카메라 사용 |
| `NSMicrophoneUsageDescription` | 가상 머신에서 마이크 사용 |
| `NSAddressBookUsageDescription` | 연락처 연동 |
| `NSDocumentsFolderUsageDescription` | Windows 앱에서 문서 열기/저장 |
| `NSDownloadsFolderUsageDescription` | ISO/업데이트 저장 |
| `NSDesktopFolderUsageDescription` | 데스크톱 파일 공유 |
| `NSRemovableVolumesUsageDescription` | 외부 디스크 부팅/사용 |
| `NSNetworkVolumesUsageDescription` | 네트워크 볼륨 접근 |
| `NSBluetoothAlwaysUsageDescription` | Bluetooth 장치 전달 |

**추론**: Coherence/공유 폴더 기능을 위해 파일 시스템·주변기기 접근 범위가 넓다.
TCC 프롬프트는 최초 사용 시 발생하며, 사용자가 거부하면 해당 기능만 비활성화된다.

### 6.2 저장 위치 (문자열 추론)

- 키체인 그룹: `4C6364ACXT.com.parallels.desktop.passwords`
- 앱 그룹: `…appstore`, `…Desktop`, `…appbundles`
- 라이선스 파일: `licenses.json`, `License.ver = 4.0.0.parallels`
- 서비스 락/pid: `/var/run/prl_desktop_services.lock`, `/var/run/prl_disp_service.pid`

**미확인**: 키체인 항목의 접근 제어(ACL), 라이선스 파일 암호화 방식.

---

## 7. 파일·URL 핸들러 공격면 (관찰)

| 등록자 | 범위 | 근거 |
|--------|------|------|
| 메인 앱 | URL 스킴 `http`, `https`, `pdfm` | `ev_80877cc6…` |
| 메인 앱 | 문서 타입 `pvm`, `pvmz`, `pvmp`, `macvm`, `vmconfig` (Owner) | 동일 |
| 메인 앱 | Services: `Reveal in Windows`, `Open in Windows` (`public.item`) | 동일 |
| Mounter | `public.directory` + `*` / `*/*` / `****` (Alternate Viewer) | `ev_be347521…` |
| Link | `.lnk`, `.plinkAg`, `.plinkFe`, `.plinkPd` (Shell), LSUIElement | `ev_0322a434…` |
| ExeQLPlugin | Quick Look thumbnail, `com.microsoft.windows-executable` | `ev_55d25f7c…` |
| OpenInIE | Safari extension, 툴바 **"Open current page in Edge"**, 허용 도메인 `*.webkit.org` | `ev_9fcdbd4d…` |
| ParallelsMail | Share extension, 파일 URL 1개일 때 활성 | `ev_714ab174…` |

**추론**:
- `http`/`https` 스킴 등록은 기본 브라우저를 탈취하지 않도록 `LSHandlerRank`가 낮게 설정됐을
  가능성이 있으나, plist에 rank가 없어 **미확인**.
- Mounter의 `*.*` Alternate 등록은 사용자가 "다음으로 열기"를 선택할 때만 동작한다.
  악성 디스크 이미지 파서가 공격면이 될 수 있다.
- Safari 확장의 허용 도메인이 `*.webkit.org`로 제한되어 있어 웹 페이지 콘텐츠 접근 범위는 좁다.

---

## 8. 커널·시스템 확장

| 항목 | 관찰 | Evidence |
|------|------|----------|
| `.dext` / `.systemextension` | 번들 내 **0개** | 인벤토리 |
| 호스트 설치 확장 | **0개** (`systemextensionsctl list`) | 직접 확인 |
| 레거시 kext 문자열 | `com.parallels.kext.{hypervisor,netbridge,vnic}` | 문자열 |
| 가상화 의존 | `Hypervisor.framework`, `Virtualization.framework`, `vmnet.framework` | `ev_b0c4eabf…`, `ev_7a60ef9c…` |

**추론**: 최신 버전은 커널 확장 없이 Apple 프레임워크만 사용한다. 레거시 kext 문자열은
마이그레이션/감지 코드의 잔재로 보인다. 커널 공격면은 크게 줄었지만, **root 데몬과
장치 패스스루 entitlement**가 실질적 경계다.

---

## 9. 라이선스·DRM 관찰 (문자열 추론)

| 항목 | 값 |
|------|-----|
| 라이선스 포맷 | `License.ver = 4.0.0.parallels` |
| 검사기 | `CDspLicenseChecker`, `DspLicense`, `LicenseOperation` |
| 작업 | `Task_DeactivateLicense`, `Task_DspLicenseHelper`, `DspCmdCtlLicenseChange` |
| 토큰 키 | `AuthorizationAccessKey`, `AuthorizationKaPdeKey`, `AuthorizationKaPdeInstanceKeyNumber`, `AuthorizationMasterLicKey`, `AuthorizationSubscriptionUuidKey`, `AuthorizationTemporaryLicKey` |
| 관리형 | `PLM_ROOT_ACCOUNT`, `Admin license monitor`, `Admin::LicenseMonitor` |
| 레거시 | `protexis.com`, `Protects` |

**추론**: Access/KaPde/Master/Subscription/Temporary 5종 토큰을 분리해
오프라인/구독/기업 라이선스를 처리한다. 서명 검증·만료 처리·오프라인 유예 로직은
**미확인**(디컴파일 필요).

---

## 10. 위험 등록부

| ID | 항목 | 관찰 | 평가(추론) | 등급 | 검증 방법 |
|----|------|------|-----------|------|-----------|
| R1 | setuid 2종의 TOCTOU | 위치/소유자/모드/서명 검증 | 검증-실행 사이 교체 가능성 | 중 | 디컴파일 + 레이스 테스트 |
| R2 | `prl_disp_service` root 실행 | entitlement 최소, root 권한 | 코드 취약점 시 즉시 root | 높음 | 디컴파일/퍼징 |
| R3 | ATS 임의 로드 | 양 앱 모두 true | 평문/약한 TLS 가능 | 중 | 네트워크 캡처 |
| R4 | 광범위 entitlement | 카메라/마이크/주소록/Apple Events | UI 침해 시 권한 집중 | 중 | TCC 로그, 런타임 |
| R5 | JIT 예외 | `allow-unsigned-executable-memory` | JIT 버그 시 코드 실행 | 중 | FEX 버전/CVE 추적 |
| R6 | 레거시 평문 DRM | `http://protexis.com` | 토큰 노출 가능 | 낮음~중 | 호출 여부 확인 |
| R7 | 파일 핸들러 `*.*` | Mounter Alternate | 악성 이미지 파서 공격면 | 중 | 퍼징 |
| R8 | `application-identifier` 불일치 | `prl_vm_app`이 메인 앱 ID 사용 | 검증 혼동 가능성 | 낮음 | 런타임/디컴파일 |
| R9 | 공증 ticket 미스테이플 | `stapler validate` 실패 | 오프라인 첫 실행 차단 | 낮음 | 오프라인 실행 |
| R10 | 시스템 확장 없음 | `.dext` 0 | 커널 공격면 감소 | 정보 | — |

> 등급은 정적 증거 기반의 **예비 평가**다. 런타임 검증 전에는 확정하지 않는다.

---

## 11. 런타임 검증 체크리스트

1. 앱 1회 초기화 후 설치 산출물 확인
   - `/Library/LaunchDaemons/com.parallels.desktop.launchdaemon.plist`
   - `/Library/Parallels/Parallels Desktop/launchd_wrapper`
   - `/Library/Parallels/Parallels Service.app`
   - `exceptions.list`의 setuid 모드가 실제로 적용됐는지
2. `launchd_wrapper` 검증 실패 케이스 재현
   - 위치 파일 변조, 번들 소유자/모드 변경, symlink 교체
3. `prl_update_helper` 서명 검증 범위 확인
   - 잘못된 Team ID / 변조된 번들 / 다운그레이드 시도
4. 네트워크 캡처
   - 라이선스 활성화/갱신, 텔레메트리, 업데이트
   - ATS 예외가 실제 HTTP 폴백을 허용하는지
5. TCC 이벤트 확인
   - 카메라/마이크/주소록/Apple Events 최초 요청 시점
6. XPC/IPC 관찰
   - `com.parallels.disp.net.ipc`, `com.parallels.naptd.ipc`, `XPCMounter`
   - 클라이언트→디스패처 인증(코드 서명/감사 토큰) 여부
7. VM 부팅 경로별 확인
   - ARM: HVF, x86: FEX JIT, macOS: Virtualization.framework

---

## 12. 결론

- **긍정적**: setuid 2종 모두 위치·소유자/모드·서명 요구사항을 조합한 방어적 설계를 갖췄고,
  앱 확장은 샌드박스화되어 있으며, 커널 확장은 제거되었다.
- **주의**: root 상주 데몬(`prl_disp_service`)의 코드 자체가 최대 공격면이며,
  ATS 비활성·광범위 entitlement·JIT 예외가 이를 보완하거나 확대한다.
- **미확인**: TOCTOU, 서명 검증 범위, IPC 인증, TLS 정책, 라이선스 검증 알고리즘.
  이 항목들은 디컴파일러(Ghidra/Hopper)와 런타임 관찰로만 확정할 수 있다.

---

*본 문서는 정적 분석 결과다. "추론"으로 표기한 항목은 실행 관찰로 검증되지 않았다.*
