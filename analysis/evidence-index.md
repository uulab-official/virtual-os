# Evidence Index — Parallels Desktop 27.0.3

> **목적**: `virtual-os` 보고서의 모든 구조적 결론에 대응하는 REA Evidence ID와 다이제스트를 기록한다.
> **대상**: `/Applications/Parallels Desktop.app` (27.0.3, build 58680)
> **도구**: REA `rea-agents@6.5.0` — provider `native-macos`, `rea-artifact-graph`
> **수집일**: 2026-11-05
> **주의**: Evidence ID는 REA가 반환한 값을 그대로 기록했다. ID가 없는 항목은 문서 마지막의 "보조 확인" 절에 별도로 분류했다.

---

## 1. 무결성 루트

| 항목 | 값 |
|---|---|
| 대상 | `/Applications/Parallels Desktop.app` (directory) |
| Root SHA-256 | `ff7df63fe3363e193be921184cc253461eb771dc840f3e9daf9f6357f32d26ce` |
| Manifest ID | `agm_9bf7414575a96534af917180c4d82259e35e7e789ca2c15d2b98a4d33a93aff1` |
| Graph SHA-256 | `b9d3a9275d6b40c91498350b2d284b8295c0215ca527b2caa53faf53978f8c92` |
| Root Artifact ID | `art_ee41564d28006836f2c76179c88c0d8ec9285d91c9cb598aa94317f4c83dcd79` |
| 그래프 규모 | 831 nodes / 972 occurrences / 892 edges |
| Coverage | `complete-within-substeps` (관찰 1, 가설 0, 미탐색 분기 3) |
| Evidence | `ev_a19f02f0ccbf437e1e566741deb3d307d37f3ef144462bd614610c750c0badfa` (inspect_artifact), `ev_7372cb8a0d6ddf6c8d6bdb8ce6a4f9810975b5bf6c5f552193c67b6a6101157e` (inventory_artifact) |

미탐색 분기 3개는 provider 중립 검사가 수행하지 않은 `open_binary`, `inspect_managed_artifact`, `inspect_plist`다. 이 중 `inspect_plist`는 이후 별도 연산으로 수행했고, `open_binary`는 디스어셈블러 부재로 수행하지 못했다.

---

## 2. 핵심 Evidence 요약

| # | 확인 내용 | 연산 | Evidence ID | 핵심 값 |
|---|---|---|---|---|
| 1 | 전체 번들 아티팩트 그래프 | `inspect_artifact` | `ev_a19f02f0ccbf437e1e566741deb3d307d37f3ef144462bd614610c750c0badfa` | 831 노드 / 972 항목 / 892 엣지, root_sha256 `ff7df63f…` |
| 2 | 번들 인벤토리(하위 단계) | `inventory_artifact` | `ev_7372cb8a0d6ddf6c8d6bdb8ce6a4f9810975b5bf6c5f552193c67b6a6101157e` | manifest `agm_9bf74145…`, graph_sha256 `b9d3a927…` |
| 3 | 메인 앱 아키텍처 | `list_architectures` | `ev_f834c8b81984950b0a6c7d2ed6f728a1ba1ae577a240cc76cfa8b7894aba49c9` | arm64 단일(thin) |
| 4 | 메인 UI `prl_client_app` 구조 | `inspect_macho` | `ev_f75348391b69e77a08a8e34f40abd01b33f09de34bf0f68ee1cd6f4233f6e745` | arm64, minOS 14.0, SDK 26.0, 의존성 52 |
| 5 | 메인 UI 서명·entitlement | `inspect_signature` | `ev_6ba40a4bbb1bb934b92f836e5bdaa148437a2f04211f26ffd81411e91523303f` | Hardened ✔, entitlement 10종 |
| 6 | 상주 데몬 `prl_disp_service` 구조 | `inspect_macho` | `ev_a8b166b3d886172b926de43cfada520f4197aae3aae44360ba5c4241ae3ff113` | arm64, 의존성 30 (Hypervisor/vmnet/PCSC/CryptoTokenKit) |
| 7 | 상주 데몬 서명 | `inspect_signature` | `ev_00f90db3a1f3a3e762727f94caf377a093dc777f75b8ebc7be22b3afe3ca039d` | Hardened ✔, `com.apple.security.smartcard` |
| 8 | VM 실행기 `prl_vm_app` 구조 | `inspect_macho` | `ev_b0c4eabf8d84bbe33722ce84fb084049a0622a2ff0d283b08cc8f5f50fa76b7a` | arm64, 의존성 36 |
| 9 | VM 실행기 서명·entitlement | `inspect_signature` | `ev_376defa00eeb5cd4704ab0e7c5e47d4e6fb8c18104368cbcdd26325dd7d2877b` | hypervisor / JIT / device-access / networking |
| 10 | macOS 게스트 `prl_macvm_app` 구조 | `inspect_macho` | `ev_7a60ef9c7261517ab78f1f49c4f0f1c35e0ac8822be03c90a45849a6da89dead` | arm64, Virtualization.framework 링크 |
| 11 | macOS 게스트 서명 | `inspect_signature` | `ev_cf5cf98ca125f766da66ec2cfe6798a8026e6229f8cc1968da96fbbbbef17c1a` | virtualization / device-access / networking |
| 12 | setuid 스텁 `Parallels Service` 구조 | `inspect_macho` | `ev_e3120afb897f6057fd7d8b090e532780df04ec1c3f8eab4c655ccdcad1767ab3` | arm64, 의존성 5 (경량 런처) |
| 13 | setuid 스텁 서명 | `inspect_signature` | `ev_6ec439c21673b2853ed99890fd1ff2f0536152d0e62477d95555f398d8b5d169` | Hardened ✔, entitlement 없음 |
| 14 | `prl_update_helper` 서명 | `inspect_signature` | `ev_efef5c12d04590b6875c28a156c99601d8dfee5f790ab1bec2d52ea780bba343` | Hardened ✔, entitlement 없음 |
| 15 | `launchd_wrapper` 서명 | `inspect_signature` | `ev_32d514586dc5463a301e1b48936ffc2af0235c25b2833d7a746ae24574880830` | Hardened ✔ |
| 16 | `libMonitorArm` 구조 | `inspect_macho` | `ev_6b7f8aa0c9f8c65ba8199d5da80b7dadced25d51d327f9252e1ccd7175a6107d` | arm64, Hypervisor.framework 의존 |
| 17 | `libMonitorX86Emu` 구조 | `inspect_macho` | `ev_96e1089a9873e49d2d50a895ba45c4429e355afae53c0f98f27fab0056895086` | arm64, Hypervisor.framework 의존 |
| 18 | SDK 프레임워크 구조 | `inspect_macho` | `ev_00b1ae261bb68724e3d2f00b5b47321764b2d4574460b8f24d6aa3058249517f` | universal, 1,268 `Prl*` export |
| 19 | 메인 앱 Info.plist | `inspect_plist` | `ev_80877cc6776dde1ac487c1ced2d200a4611f3e70ed37c4175b9ad5e859f4a6c1` | ATS 임의 로드, URL 스킴 http/https/pdfm |
| 20 | 디스패처 Info.plist | `inspect_plist` | `ev_fe534773647dbeea0eaf042bb06875c9b5985577c1bed492a522452046f5a757` | LSUIElement + LSBackgroundOnly, ATS 임의 로드 |

---

## 3. Mach-O 구조 Evidence (17)

| 파일 | arch | UUID | minOS / SDK | 의존성 | Evidence ID |
|---|---|---|---|---|---|
| `Parallels Service` | arm64 | `3D958B06-3CEC-32FA-840B-DFA99B64519F` | 14.0 / 26.0 | 5 | `ev_e3120afb897f6057fd7d8b090e532780df04ec1c3f8eab4c655ccdcad1767ab3` |
| `ParallelsVirtualizationSDK` | arm64 | `93BB79FE-2617-388B-AF75-764A84EFB7DD` | 14.0 / 26.0 | 28 | `ev_00b1ae261bb68724e3d2f00b5b47321764b2d4574460b8f24d6aa3058249517f` |
| `libMonitorArm.dylib` | arm64 | `A270CC00-ABD4-3F83-A5C5-729ECB934B36` | 14.0 / 26.0 | 4 | `ev_6b7f8aa0c9f8c65ba8199d5da80b7dadced25d51d327f9252e1ccd7175a6107d` |
| `libMonitorX86Emu.dylib` | arm64 | `EA9323FA-7A1C-36C1-ADE9-4B9D9B3B3E0B` | 14.0 / 26.0 | 4 | `ev_96e1089a9873e49d2d50a895ba45c4429e355afae53c0f98f27fab0056895086` |
| `libPrlGui.3.dylib` | arm64 | `3E503469-F5FD-32EF-912F-FE69360E3098` | 14.0 / 26.0 | 47 | `ev_db8da96bbcfa38e3a47fb7adde3ba73f950a338663670a7e900dd01673ae4290` |
| `libPrlXmlModel.1.dylib` | arm64 | `70975F22-7FC6-30E2-A6FF-8C955DF3E7F5` | 14.0 / 26.0 | 13 | `ev_77e5faab2a9ea5c7bdb3a19ce0690897ca55b640877f4ef772033cd271cd05ba` |
| `prl_client_app` | arm64 | `73FD9A27-DCCB-35CA-88AB-FAB03A34B60C` | 14.0 / 26.0 | 52 | `ev_f75348391b69e77a08a8e34f40abd01b33f09de34bf0f68ee1cd6f4233f6e745` |
| `prl_convert` | x86_64 | `71DB9222-5734-3BB8-9C0B-EACB925CDB36` | 14.0 / 26.0 | 14 | `ev_5d3d4af367ae7efba15bf92aedc9d22001675cfe3a648ee14b543a05d1692fca` |
| `prl_disk_tool` | x86_64 | `C7653A76-0E4D-3692-972D-B6388871097F` | 14.0 / 26.0 | 14 | `ev_73267ba1c0dfe516952dd883131bbaa6c0afe2e24432e8a9f5dc84ff96fec924` |
| `prl_disp_service` | arm64 | `09F951EC-309F-3BC7-8038-001CBA27D2DB` | 14.0 / 26.0 | 30 | `ev_a8b166b3d886172b926de43cfada520f4197aae3aae44360ba5c4241ae3ff113` |
| `prl_macvm_app` | arm64 | `624E3006-4573-3073-ACEC-7EDFD9DCA88C` | 14.0 / 26.0 | 19 | `ev_7a60ef9c7261517ab78f1f49c4f0f1c35e0ac8822be03c90a45849a6da89dead` |
| `prl_naptd` | arm64 | `CE6E7054-E6F0-38C0-870E-796491EF900A` | 14.0 / 26.0 | 13 | `ev_36a228135fa8b6d726cc1cb6b9401dda7123a4cffa94e2dd509189b9461e8c36` |
| `prl_update_helper` | arm64 | `BEB3B12A-828F-3902-B88B-0658817C8282` | 14.0 / 26.0 | 6 | `ev_c8633c1a4379707fafabf8cba2f2c90bc1aa6ef85e4ba89a349443057b579ed0` |
| `prl_vm_app` | arm64 | `2A150605-341F-3521-AB56-2D8097EBAF9A` | 14.0 / 26.0 | 36 | `ev_b0c4eabf8d84bbe33722ce84fb084049a0622a2ff0d283b08cc8f5f50fa76b7a` |
| `prl_vmarchiver` | arm64 | `CD6AE28C-0156-3316-8C53-CBB162B4D984` | 14.0 / 26.0 | 2 | `ev_db09318839ed485129d6d8c1a96422cbdc8222e376936eb6d77ad828901e84d2` |
| `prlctl` | arm64 | `C1529A28-3A58-37B8-BC9E-C94A872B58BB` | 14.0 / 26.0 | 3 | `ev_1a174d3b2c230a550f9d399ba015502796ce2c674df0dddea0cd86fd9be9aa45` |
| `prlsrvctl` | arm64 | `ADB8D451-DEA5-32F3-ABFE-E55BBF83FD94` | 14.0 / 26.0 | 3 | `ev_b0bc18600ad6b5d4805fc145b048d849e27a398466b8e10de5a432acef9bf62f` |

---

## 4. 코드 서명 Evidence (79)

`inspect_signature`는 `codesign` 계열 메타데이터를 구조화한다. Hardened는 hardened runtime 플래그, Ent.는 entitlement 키 개수다.

| 파일 | Identifier | 형식 | Hardened | Ent. | Evidence ID |
|---|---|---|---|---|---|
| `PDCocoaKit` | `com.parallels.desktop.console.PDCocoaKit` | Mach-O thin (arm64) | ✘ | 0 | `ev_8df85fd99537330ed2a2551ef479bb52a1a994e15f1869fa6600bdb367fb4ac6` |
| `PDControlCenter` | `com.parallels.desktop.console.PDControlCenter` | Mach-O thin (arm64) | ✘ | 0 | `ev_6fbcfb3b79517b297a0050a790b97ad3b873296c7b0cba491279b98ab99d886f` |
| `ParallelsVirtualizationSDK` | `com.parallels.virtualization.sdk` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_e4419c0d7ff9e13c5147789563e4ad66e81503604b8376503cd91edc698cb29c` |
| `ProxyAppCore` | `com.parallels.proxyappcore` | Mach-O thin (arm64) | ✘ | 0 | `ev_9289a3c801eadcdbd0542fb0d7aa20696eeb93c10eb71afacb6e2278caf08f29` |
| `QtConcurrent` | `org.qt-project.QtConcurrent` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_d1a3782eaaedc00c5820a52e314e61673990d4638cb2a3f2b9f8745dcf81ac77` |
| `QtCore` | `org.qt-project.QtCore` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_64f7019f739cbc93791e1ac94ba20a8f04bc4abe93b1051b0302c7426455249e` |
| `QtGui` | `org.qt-project.QtGui` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_35acd43a553552b79dc92627e5ca4b37386e03756b6ae0ec72a7287f64e3e542` |
| `QtMacExtras` | `org.qt-project.QtMacExtras` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_c69cf538efcf1367cf39f2dedcd47ca886920c2efb42bbbc5e7b50d3213759dc` |
| `QtNetwork` | `org.qt-project.QtNetwork` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_2fd2bb87c7fbda5a7dafb4053a402ba491303611ae5289e4019ee9fdce07b70f` |
| `libqgif.dylib` | `libqgif` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_50eea46bdcf79f96ee26621261a1674fcafcda3e75a6db6317a7d88ea1056f9d` |
| `libqjpeg.dylib` | `libqjpeg` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_e148f8d41df5d226ddc20d214b6ec63a27f6ea8aca2c7b9868333bbf42dc4af2` |
| `libqtiff.dylib` | `libqtiff` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_39cf8ab1079773e5c96c865c46ba5d67442ef6923167a881a7ead6dd1cb0c892` |
| `libqcocoa.dylib` | `libqcocoa` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_60ad929b2e2c892fbb40cda66ffea9744ca608891bad8e05e9608078e3be32a1` |
| `libqquicklayoutsplugin.dylib` | `libqquicklayoutsplugin` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_9cc87d3c3a2cfd0cb716dc3104cb8a50a258308b610a71f5ac994f461a290f47` |
| `libqtgraphicaleffectsplugin.dylib` | `libqtgraphicaleffectsplugin` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_0955fda5a80d883eab827b517bc0f70dfc6ee62005cc5b31aaaf9b1f078e3906` |
| `libqtgraphicaleffectsprivate.dylib` | `libqtgraphicaleffectsprivate` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_efadede30313bcd2674e78bfacef2db00cb1a90c62e725e82ae61c97be4886af` |
| `libqtquick2plugin.dylib` | `libqtquick2plugin` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_2528ab3f9316df2e777d31c2e2c48734baa8d31869cc0f27a6521aaefec58855` |
| `libqtquickcontrols2plugin.dylib` | `libqtquickcontrols2plugin` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_a69a3866771d5c7cc6dc78e235c73bd23e6cb273588d784c4f5741fd61a60959` |
| `libqtquicktemplates2plugin.dylib` | `libqtquicktemplates2plugin` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_ded7fe02e3d5383f70fe29f9df99462ed034cebafdf8aa18b55ae399aa57f4f8` |
| `libwidgetsplugin.dylib` | `libwidgetsplugin` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_604ff4de9d4faf8e47db7da9b28bb6ed7d478a66a39b0ff00832d3944b713d2d` |
| `libwindowplugin.dylib` | `libwindowplugin` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_338715eb5b9bef5be0a68489e81b5e27a103d6735a1f2e6e916d2d0ab4ce22dc` |
| `libqsqlite.dylib` | `libqsqlite` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_4c9b356fc30a9b868379f0c0ee908a5d4344e76c74e3b2b334c27dd68050b71f` |
| `libqmacstyle.dylib` | `libqmacstyle` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_c57ea11880082780d1e9ec8cbbd8482367f2427eb1b4da426ac5534d0c0a6187` |
| `QtPrintSupport` | `org.qt-project.QtPrintSupport` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_ba3dea97ce6b27ffd53e0e09a04049f8c6c5c2e70ac9d5a51097fa26ead588f2` |
| `QtQml` | `org.qt-project.QtQml` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_b3f73a2c0d9e419375872a8ac2df85012e9e1751cd89a4b53699e54c59deb989` |
| `QtQmlModels` | `org.qt-project.QtQmlModels` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_d5b183168e3c53fd03888133ec3a9ef0818c00bed51429f7143f59326abef2c4` |
| `QtQmlWorkerScript` | `org.qt-project.QtQmlWorkerScript` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_d75493f13f73b4b87b1794edff9291fa4b6ebe1f78a8276f29db17158b991b27` |
| `QtQuick` | `org.qt-project.QtQuick` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_8602d5659a2d51e2f796966ea1e2c13560c28473c492e2f61a85c2aaeea0bb02` |
| `QtQuickControls2` | `org.qt-project.QtQuickControls2` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_23e1cf280cbd9230239cf1e1f75bf2db2b3ab38a4c1a67ee572420dddb7bd3d7` |
| `QtQuickTemplates2` | `org.qt-project.QtQuickTemplates2` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_732582affb9e38600d7695ff1516fe558452cab079c516ec255fb3342112af5d` |
| `QtQuickWidgets` | `org.qt-project.QtQuickWidgets` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_7c85f16cda6b6124ce06ca5a2d217c969bf7c7cce50091074ca96bfb12ca3505` |
| `QtScript` | `org.qt-project.QtScript` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_c959f531b14e8c7d04c13be973656f2fd7873b4be4ca55a4a0d6357cfa9588e8` |
| `QtSql` | `org.qt-project.QtSql` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_1bce7717db2df9b834ff4053ee261b8bf72a7946b6bc41a74e5d2cda9736ce5c` |
| `QtWidgets` | `org.qt-project.QtWidgets` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_48743e8b5d3c702da3fb9783ceeea9222110d955d120d335809d54d840f09668` |
| `QtXml` | `org.qt-project.QtXml` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_122f1356d48140b17757e48af10f926af7c6b6b37719cf049edb901b339f9c9e` |
| `libAbstractTask.1.dylib` | `com.parallels.desktop.console.common` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_709aba613f9c739e836c04365a98636e646923ef369ff36142dc1301857c3f9e` |
| `libDaApiWrap.1.dylib` | `com.parallels.desktop.console.common` | Mach-O thin (x86_64) | ✘ | 0 | `ev_6b4ab2ae6b91a50dabe729a51f9a9fcd37de831d83b10e705438a8d5f24fc1d5` |
| `libOSMesa.8.dylib` | `libOSMesa.8` | Mach-O thin (arm64) | ✘ | 0 | `ev_6cd2623d60ae47feeb940cd8b6edcfc8ca23bd85337f91c14b6f609a5a68c2e5` |
| `libPrlGui.3.dylib` | `com.parallels.desktop.console.common` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_891769954ff6c81ebd2c8c62367ef2c8b2816401bee1438700eef867b6b9e980` |
| `libPrlSdkCommunication.1.dylib` | `com.parallels.desktop.console.common` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_73a0eb42275106a03fffda621372df73d2e84e26e40b55f0ac06de5f4f5a4909` |
| `libPrlXmlModel.1.dylib` | `com.parallels.desktop.console.common` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_8a590f74968ba70183ae445a5d219f1b5938ac80f295ccd5a455a3286ffda3dc` |
| `libmetalirconverter.dylib` | `com.apple.libmetalirconverter` | Mach-O universal (x86_64 arm64) | ✘ | 0 | `ev_58b0f493ebaa2a25c40f1963717a917386678bb27da6df9bf34baafe899ab285` |
| `libprl_mspack.dylib` | `com.parallels.desktop.console.common` | Mach-O thin (arm64) | ✘ | 0 | `ev_547ec000531cef73f51c461e9dbeb416348178bd97b8f4dce6d2d8e256842146` |
| `libwimlib.1.dylib` | `com.parallels.desktop.console.common` | Mach-O thin (arm64) | ✘ | 0 | `ev_d9eb5ebeaa3455e15b53cecb089019cab5ae213b59b4aa12903b042ec3363e3a` |
| `7z` | `com.parallels.desktop.console.7z` | Mach-O thin (arm64) | ✔ | 0 | `ev_670433ceea56ab45ff114b5dec76362f8024e1265d064e0e90a86d470173cb12` |
| `7z.dylib` | `com.parallels.desktop.console.common` | Mach-O thin (arm64) | ✘ | 0 | `ev_c88607c6b10334930d6142899e0774f63874fb38878de7b557952ef649f6cae7` |
| `prl_macvm_app` | `com.parallels.macvm` | Mach-O thin (arm64) | ✔ | 4 | `ev_cf5cf98ca125f766da66ec2cfe6798a8026e6229f8cc1968da96fbbbbef17c1a` |
| `Parallels Service` | `com.parallels.desktop.console.common` | Mach-O thin (arm64) | ✔ | 0 | `ev_6ec439c21673b2853ed99890fd1ff2f0536152d0e62477d95555f398d8b5d169` |
| `prl_disp_service` | `com.parallels.desktop.dispatcher` | Mach-O thin (arm64) | ✔ | 1 | `ev_00f90db3a1f3a3e762727f94caf377a093dc777f75b8ebc7be22b3afe3ca039d` |
| `prl_vm_app` | `com.parallels.vm` | Mach-O thin (arm64) | ✔ | 7 | `ev_376defa00eeb5cd4704ab0e7c5e47d4e6fb8c18104368cbcdd26325dd7d2877b` |
| `inittool2` | `com.parallels.desktop.console.common` | Mach-O thin (arm64) | ✔ | 0 | `ev_8e2bf28345a31aae9ea6d3c405a7935d4c4b23223e8303710f890ec835190e45` |
| `launchd_user_wrapper` | `com.parallels.desktop.console.common` | Mach-O thin (arm64) | ✔ | 0 | `ev_7f9a0295b21cb40af69a3988557eaa9ebda77614908d98dedb2653b326fc8a78` |
| `libMonitorArm.dylib` | `com.parallels.desktop.console.common` | Mach-O thin (arm64) | ✘ | 0 | `ev_d5f642b77b8951e48c209eb07b339949f4c4be0fcabe5a83a5ecf6c36ce85947` |
| `libMonitorX86Emu.dylib` | `com.parallels.desktop.console.common` | Mach-O thin (arm64) | ✘ | 0 | `ev_edab5a7781adf32fdb6afcbf04953478c212a6d21b78d0d91df9be973b673a1a` |
| `prl_bglaunch` | `com.parallels.desktop.console.common` | Mach-O thin (arm64) | ✔ | 0 | `ev_c7f4ecb6f28e459cadd539f2c58adb458f1c6508c817ad852e398fc458be273a` |
| `prl_client_app` | `com.parallels.desktop.console` | Mach-O thin (arm64) | ✔ | 10 | `ev_6ba40a4bbb1bb934b92f836e5bdaa148437a2f04211f26ffd81411e91523303f` |
| `prl_convert` | `com.parallels.prl-convert` | Mach-O thin (x86_64) | ✔ | 0 | `ev_e7a2c7b7d3c49db92186e34a92e5b85e5a7fe5f07b241db022044e1a5b9df321` |
| `prl_disk_tool` | `com.parallels.disktool` | Mach-O thin (x86_64) | ✔ | 0 | `ev_3903ef31a2f1cd8bb0281f90e336720a63ffbc28e8ef706ca9d2b90c0d32ef41` |
| `prl_esd2iso` | `com.parallels.desktop.esd2iso` | Mach-O thin (arm64) | ✔ | 0 | `ev_30e7c47cf667f5767db5ad597ebf4c4e093f723283101cbe8e5ba3a84d4ee36a` |
| `prl_ls_users` | `com.parallels.desktop.console.common` | Mach-O thin (arm64) | ✔ | 0 | `ev_fd48f3e89559b669961f463bcbf52051458e5aff8bccd2fe76da56ab79eb8a14` |
| `prl_macvm_helper` | `com.parallels.desktop.console.macvmhelper` | Mach-O thin (arm64) | ✔ | 2 | `ev_3bafb626cb54460ea92516b8a3dd39928099338b93f455788c8c988173f70ad4` |
| `prl_naptd` | `com.parallels.naptd` | Mach-O thin (arm64) | ✔ | 0 | `ev_58a02c91976167a986060397239ae40a58e8e9f0d93f646beb01df1289d5b49d` |
| `prl_net_start` | `com.parallels.netstart` | Mach-O thin (arm64) | ✔ | 0 | `ev_84d67e90fec85d3a80b2e8841fd8353d9ec9ee1afee735420693e218ad88e4a4` |
| `prl_osimage_tool` | `com.parallels.desktop.console.common` | Mach-O thin (arm64) | ✔ | 0 | `ev_e318fe9ab63fd8b763c5e66706b13106f9fcefce4a6ffedc0f47233d2f958d33` |
| `prl_packer_inplace` | `prl_packer_inplace` | Mach-O thin (arm64) | ✔ | 0 | `ev_847cd106f5af1d5f5dba0050efd578a9254d9bf2a9787b56db4471ece06834b0` |
| `prl_perf_ctl` | `com.parallels.desktop.console.perfctl` | Mach-O thin (arm64) | ✔ | 0 | `ev_d66b4d18435853726b3b3683ac5a8e66ba72425dd9519e83a56bdf6e01419c72` |
| `prl_ps2pdf` | `com.parallels.desktop.console.prlps2pdf` | Mach-O thin (arm64) | ✔ | 0 | `ev_bbded3baf7cbadb7b012332f61a4b4a098abf85541a3311e3eaf69e78e4067ea` |
| `prl_switchspaces` | `com.parallels.desktop.switch-spaces` | Mach-O thin (arm64) | ✔ | 0 | `ev_d31fc6ea672391a4a2e6e8d5d3ab241c0cf8af148da5f9413c82cf0056225569` |
| `prl_update_helper` | `com.parallels.desktop.console.common` | Mach-O thin (arm64) | ✔ | 0 | `ev_efef5c12d04590b6875c28a156c99601d8dfee5f790ab1bec2d52ea780bba343` |
| `prl_updater_ctl` | `com.parallels.updater` | Mach-O thin (arm64) | ✔ | 0 | `ev_dc176e05ecdc548120adf27389112ea7dec32a0d17ce14f18db06d9bbbc70686` |
| `prl_vmarchiver` | `com.parallels.desktop.console.vmarchiver` | Mach-O thin (arm64) | ✔ | 0 | `ev_7f0ef6cb043693021b48c8144ba9e5d406c7c0a7a01afce3740ae0eee24b3dad` |
| `prlctl` | `com.parallels.desktop.console.prlctl` | Mach-O thin (arm64) | ✔ | 0 | `ev_49ea69a8364b5c258f9014ba68b9062a07e541e588ceb48e1ddf3f0f332617af` |
| `prllog` | `com.parallels.desktop.console.prllog` | Mach-O thin (arm64) | ✔ | 0 | `ev_2b579a645507e308516a65794d32944c441400f283c0c8c9639558519fd16684` |
| `prlsrvctl` | `com.parallels.desktop.console.prlsrvctl` | Mach-O thin (arm64) | ✔ | 0 | `ev_15c8c2c45d4cf7defd16594c62ffee0c7ca0f10e73675fa8c0906137a9b87cdf` |
| `com.parallels.desktop.console.ExeQLPlugin` | `com.parallels.desktop.console.ExeQLPlugin` | Mach-O thin (arm64) | ✔ | 2 | `ev_337533420b8df77d25a708827f22b226211616e5c1a15ee01f42521266c49293` |
| `com.parallels.desktop.console.OpenInIE` | `com.parallels.desktop.console.OpenInIE` | Mach-O thin (arm64) | ✔ | 3 | `ev_3eac82676aa61926e28b289ff290e842e2a8d8e04eadaff1a357e3dd888457a2` |
| `com.parallels.desktop.console.ParallelsMail` | `com.parallels.desktop.console.ParallelsMail` | Mach-O thin (arm64) | ✔ | 2 | `ev_bae58ff98be09f358cb1c6c13cccd2faee5c43324a2530313b8d9a1e7d869ad9` |
| `launchd_wrapper` | `com.parallels.desktop.console.common` | Mach-O thin (arm64) | ✔ | 0 | `ev_32d514586dc5463a301e1b48936ffc2af0235c25b2833d7a746ae24574880830` |
| `libprl_shared_apps.dylib` | `com.parallels.desktop.console.common` | Mach-O thin (arm64) | ✘ | 0 | `ev_9061328f460981b364513221145400e2888e055c4484d90a5fc37d5b4685235b` |

---

## 5. plist Evidence (12)

| plist | CFBundleIdentifier | Evidence ID |
|---|---|---|
| `Frameworks_PDCocoaKit.framework_Versions_A_Resources_Info.plist` | `com.parallels.desktop.console.PDCocoaKit` | `ev_11188c81f5a8285f6593e5fa7c4d82b4b1916012866b9b3662143353fa412e4b` |
| `Frameworks_ParallelsVirtualizationSDK.framework_Versions_11_Resources_Info.plist` | `com.parallels.virtualization.sdk` | `ev_b31c9645bb168d8061f6f753e76e68feb4be49ac4f9c921815477bd95affe9f5` |
| `Info.plist` | `com.parallels.desktop.console` | `ev_80877cc6776dde1ac487c1ced2d200a4611f3e70ed37c4175b9ad5e859f4a6c1` |
| `MacOS_ParallelsLink.app_Contents_Info.plist` | `com.parallels.SharedAppLink.Agent` | `ev_0322a43404e32062976f5b9bce708244420d952a7c49f673f0060c5a270bc6c9` |
| `MacOS_ParallelsMacVM.app_Contents_Info.plist` | `com.parallels.macvm` | `ev_99106043a4a7d1f3041a7cf4266dd45152ec54f4231ea39bb38a6c06b036e494` |
| `MacOS_ParallelsMounter.app_Contents_Info.plist` | `com.parallels.desktop.mounter` | `ev_be3475215e3a4193159bbea32055293c3ce21bd8cec1beb6ff4c5c86f896233c` |
| `MacOS_ParallelsService.app_Contents_Info.plist` | `com.parallels.desktop.dispatcher` | `ev_fe534773647dbeea0eaf042bb06875c9b5985577c1bed492a522452046f5a757` |
| `MacOS_ParallelsVM.app_Contents_Info.plist` | `com.parallels.vm` | `ev_4e4ed51982031f7e8726f2a9765b60265038222ca85a36d9ff82040579f5089a` |
| `MacOS_ParallelsVM.app_Contents_XPCServices_XPCMounter.xpc_Contents_Info.plist` | `com.parallels.xpcmounter` | `ev_e05b6d29a0ef1008787362b74fd138b8e44f7afd4e482dda1362aba63a1a3202` |
| `PlugIns_com.parallels.desktop.console.ExeQLPlugin.appex_Contents_Info.plist` | `com.parallels.desktop.console.ExeQLPlugin` | `ev_55d25f7cf7de1cb70b2565e32cbb6e33c16e46775370d43e433cdc3f7d8e2341` |
| `PlugIns_com.parallels.desktop.console.OpenInIE.appex_Contents_Info.plist` | `com.parallels.desktop.console.OpenInIE` | `ev_9fcdbd4dafb59d4c3ea8ae983ccebb5ab33b0a34c467eaeec8a106a2affadc1b` |
| `PlugIns_com.parallels.desktop.console.ParallelsMail.appex_Contents_Info.plist` | `com.parallels.desktop.console.ParallelsMail` | `ev_714ab174e4e05acb8c4965662f23eeae7e9a9e12af95bcf37db7c7e7910fefae` |

---

## 6. 재현 방법

```bash
# 0) 임시 작업 디렉터리
mkdir -p "$TMPDIR/rea-parallels" && cd "$TMPDIR/rea-parallels"

# 1) 번들 그래프
npx -y rea-agents@6.5.0 inspect_artifact \
  --path "/Applications/Parallels Desktop.app" --integrity-policy fail

# 2) 개별 바이너리
npx -y rea-agents@6.5.0 list_architectures --path "<binary>"
npx -y rea-agents@6.5.0 inspect_macho      --path "<binary>"
npx -y rea-agents@6.5.0 inspect_signature  --path "<binary>"
npx -y rea-agents@6.5.0 inspect_plist      --path "<Info.plist>"
```

REA CLI는 각 호출마다 `evidence_id`, `subject.digest.sha256`, `provider`, `confidence`, `limitations`를 포함한 JSON을 반환한다. 본 문서는 그 JSON에서 구조 필드만 추출했다.

---

## 7. 보조 확인 (REA Evidence ID 없음)

아래 항목은 셸에서 직접 읽은 값이다. 구조적 결론의 근거로 사용했지만 **REA Evidence로 분류하지 않는다**.

| 항목 | 방법 | 사용한 곳 |
|---|---|---|
| `Contents/Resources/exceptions.list` (setuid 6755/4755) | `cat` | README §5, security-review §3 |
| `com.parallels.desktop.launchdaemon.plist` 템플릿 | `read` | architecture §2 |
| `watchdog` bash 스크립트 | `read` | architecture §2, security-review §3 |
| `embedded.provisionprofile` (만료 2044-07-03) | `security cms -D` | README §3 |
| `spctl`/`stapler` 공증 판정 | `spctl -a -vvv`, `stapler validate` | README §3 |
| XPC·엔드포인트·라이선스 키 문자열 | `strings` + `grep` | README §7–8, architecture §4 |
| Qt 5.15.1 확인 | `plutil` (QtCore Info.plist) | README §4 |
| 설치 환경 상태(프로세스/확장/영수증) | `ps`, `systemextensionsctl`, `pkgutil` | README §2.2 |

문자열 기반 항목은 보고서 본문에서 `(문자열 추론)`으로 표기했으며, 이 문서에서도 Evidence로 승격하지 않았다.

---

## 8. 알려진 한계

- `open_binary`/`decompile`/`xrefs` 미수행: Hopper·Ghidra·IDA 부재. 함수 로직, DRM 검증 알고리즘, IPC 메시지 스키마는 **미확인**이다.
- `inspect_artifact`는 심볼릭 링크 79개를 따라가지 않고 기록만 했고, 게스트 도구 ISO 내부는 확장하지 않았다.
- 서명 Evidence는 `codesign` 메타데이터이며, 공증 티켓의 온라인 검증 결과는 별도 도구(`spctl`)로 확인했다.
- 런타임 동작(서비스 설치, VM 부팅, 실제 네트워크 요청)은 관찰하지 않았다.
