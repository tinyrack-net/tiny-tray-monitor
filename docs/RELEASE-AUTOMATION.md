# 릴리스·배포 자동화 계획

`git tag v0.9.0 && git push origin v0.9.0` 한 번으로 GitHub 릴리스와 Microsoft Store 게시까지
끝내는 것이 목표다. [PACKAGING.md](PACKAGING.md) §8의 후속 항목("Store 제출 파이프라인")을
구체화한 문서다.

- 저장소: `github.com/tinyrack-net/tray-monitor`
- 제품: **TinyTrayManager** (Store ID `9MT22CHX25FD`, 유료 USD 1.99 / KRW 2,500)
- 이 문서는 계획이다. 실측하지 않은 항목은 **미검증**으로 표시한다.

---

## 0. 목표와 비목표

| 구분 | 내용 |
| --- | --- |
| 목표 | 태그 푸시 → 빌드·검증·패키징 → GitHub 릴리스 → MSIX Store 제출 → 게시 확인 |
| 목표 | Store 목록 문구(`resources/store/listing.md`)를 제출에 반영 |
| 비목표 | 가격 변경 자동화. 가격은 사람이 정한다 |
| 비목표 | 코드 서명. Store가 제출 패키지에 서명한다(기존 결정, [PACKAGING.md](PACKAGING.md) §2) |
| 비목표 | winget/scoop 매니페스트. 별도 후속 |

## 1. 현재 상태

| 단계 | 자동 | 근거 |
| --- | --- | --- |
| 포맷·clippy·테스트 | O | `format` / `clippy` / `test` job |
| 릴리스 빌드·아이콘 덤프 검사 | O | `build` job |
| installer·MSIX 빌드, 무인 설치·제거 검사 | O | `package` job |
| 태그 → GitHub 릴리스(zip·setup.exe·msix·SHA256SUMS) | O | `release` job |
| 버전 범프·커밋·태그 푸시 | X | 사람 |
| **Store 제출** | **X** | **사람 (Partner Center 웹)** |
| 게시 후 검증·알림 | X | 사람 |

`release` job은 이미 태그(`refs/tags/v*`)로 트리거된다. Store 제출도 **같은 트리거에 얹는 것이
자연스럽다.** 다른 저장소의 "태그 릴리스 = 배포" 관례와 같아진다.

## 2. 결론

- **가능하다.** 공식 경로(Microsoft Store Developer CLI)가 있고, 태그 하나로 릴리스와 Store
  제출을 모두 돌릴 수 있다.
- 이 제품에서 두 지점이 걸린다.
  1. 문서상 **"유료 제품은 미지원"** 경고가 남아 있다. → CLI v0.4.3의 `--priceId`가 유료 제품
     갱신 경로다.
  2. 우리 제품은 **시장별 가격(Pricing v2)** 상태로 보인다. 이 모델에서는 API가 기본 가격을
     제대로 되돌려주지 못해 **가격이 초기화될 수 있다.**
- 그래서 기본 설계는 **"가격은 자동화하지 않는다"** 이고, 제출을 4단으로 쪼갠다:
  **초안 생성 → 가격·패키지 검증 → 사람 승인 → 제출**.
- **P2 완료(§8-6).** Partner Center 계정에 Entra 테넌트가 없어서 자격증명을 만들 수 없다는
  사실을 먼저 찾아냈고, 테넌트를 만들어 앱을 **Manager(Windows)** 역할로 등록했다. 자격증명이 실제로
  동작하는 것까지 확인했다.
- **P2에서 설계 가정이 하나 확정됐다.** 우리 제품은 **Pricing Version 2**이고 `Pricing.PriceId`는
  `"Base"`다. 게다가 sales 리소스는 API에서 더 이상 지원되지 않는다(`SalesUnsupportedWarning`).
  즉 **API로 가격을 읽거나 검증할 수 없다** → 가격을 건드리는 단계는 자동화하지 않는다.

## 3. 공식 경로 (조사 근거)

### 3-1. Microsoft Store Developer CLI (`msstore`)

Cross-platform CLI(Windows/macOS/Linux)로, Partner Center API를 호출한다. GitHub Action
`microsoft/microsoft-store-apppublisher@v1.1`이 이 CLI를 설치하고 인증을 준비한다.

| 명령 | 용도 |
| --- | --- |
| `msstore reconfigure --tenantId --sellerId --clientId (--clientSecret\|--certificateThumbprint\|--certificateFilePath)` | 자격증명 설정 |
| `msstore publish <path> -id <productId> [--priceId TierXXXX] [--noCommit]` | 패키지 업로드(+선택적 커밋) |
| `msstore submission get <productId>` | 현재 제출 JSON 조회 (stdout, 기계 판독용) |
| `msstore submission update <productId> <json>` | 제출 전체 교체 |
| `msstore submission publish <productId>` | 인증 제출(커밋) |
| `msstore submission poll <productId>` | 인증 상태 폴링 |
| `msstore submission delete <productId> [--no-confirm]` | 제출 폐기 |
| `msstore submission rollout get/update/halt/finalize` | 점진 배포(rollout) |

주의: `submission update`는 **제출을 통째로 교체**한다. JSON에 `Pricing.PriceId`가
`Tier<number>` / `Free` / `NotAvailable` 중 하나로 들어 있지 않으면 CLI가 거부한다.

### 3-2. 선행 조건 (1회, 수동)

1. Partner Center 계정에 **Entra ID 테넌트 연결**
2. Entra 앱 등록 → Partner Center `User management → Microsoft Entra applications`에 추가하고
   **Manager 역할** 부여
3. 자격증명 4종 확보: Tenant ID, Client ID, Client Secret(또는 인증서), Seller ID(`94567340`)
4. **앱이 이미 게시되어 라이브여야 한다** — 0.5.0이 라이브이므로 충족

→ **2026-10-21에 1~4를 모두 끝냈다(§8-6).** 이제 이 단계는 반복 작업이 아니라 "만료 시 회전"만 남는다.

### 3-3. 유료 제품

문서에는 아직 다음 경고가 있다.

> App update operations through Microsoft Store Developer CLI is currently supported for
> **free products only**. Paid products will be supported in a future release.

반면 CLI 쪽 문서에는 유료 제품 절차가 구체적으로 적혀 있고, `publish`에
`-pid, --priceId`("Specifies the base price tier to set on the submission, for example
`Tier1012`, `Free`, or `NotAvailable`")가 추가됐다([msstore-cli#143](https://github.com/microsoft/msstore-cli/issues/143)에서
v0.4.3으로 완료 처리). **경고 문구가 낡았을 가능성이 크지만, 실측으로 확인해야 한다(미검증).**

### 3-4. 시장별 가격(Pricing v2) — 진짜 위험

공식 문서 원문:

> When you publish an update, the CLI sends the app's existing `Pricing.PriceId` back unchanged,
> so you don't need `--priceId` for most paid-app updates.
>
> If the Store returns `Base`, an empty price ID, or no pricing object, **the CLI stops** instead
> of submitting pricing that could reset the app to free. **This typically happens when prices are
> managed per market in Partner Center.** In that case, publish from Partner Center, or set a base
> tier explicitly and review the draft before committing it.

그리고 경고문:

> **`--priceId` sets a single base price and doesn't guarantee preservation of per-market prices.**
> Verify all intended market prices before committing. `--noCommit` uploads the package and
> updates the draft; it isn't a dry run.

관련 배경: Store submission API는 **Pricing Version 2** 제품에서 가격을 알 수 없는 tier로
반환한다. 그때는 가격·availability 모듈을 제외한 나머지만 API로 갱신할 수 있다
([msstore-cli#112](https://github.com/microsoft/msstore-cli/issues/112)).

가격 티어(확장 tier, USD):

| Tier 범위 | 가격 범위 | tier당 증가폭 |
| --- | --- | --- |
| Tier1012 – Tier1102 | $0.99 – $9.99 | $0.10 |
| Tier1103 – Tier1282 | $10.49 – $99.99 | $0.50 |
| Tier1283 – Tier1382 | $100.99 – $199.99 | $1.00 |
| Tier1383 – Tier1402 | $209.99 – $399.99 | $10.00 |
| Tier1403 – Tier1414 | $449.99 – $999.99 | $50.00 |
| Tier1415 – Tier1424 | $1,099.99 – $1,999.99 | $100.00 |

검산: 문서 예시 `Tier1052 = $4.99`(= Tier1012 + 40 tier). 따라서 우리 기본가 **USD 1.99 =
Tier1012 + 10 = `Tier1022`**(미검증 — Partner Center의 실제 tier 값과 대조 필요).

우리 제품은 **USD 1.99 base + 240개 시장 기본 그룹**이고 스토어 표시가는 **KRW 2,500**이다.
`MarketSpecificPricings`가 존재하면 `--priceId`가 그 값을 보존한다는 보장이 없다.
**원화 가격이 바뀔 수 있다는 것이 이 계획의 최대 위험이다.**

## 4. 설계

### 4-1. 트리거와 잡 그래프

기존 파이프라인에 `store-publish` 잡을 추가하고, 같은 태그 트리거에 얹는다.

```
push tag v*.*.*
  ├─ format ─┐
  ├─ clippy ─┤
  ├─ test ───┼─ package ─┐
  ├─ build ──┘           ├─ release (GitHub 릴리스 4종)
                         └─ store-publish (Store 제출)
```

- `release`는 기존 그대로 (검증된 경로를 건드리지 않는다).
- `store-publish`는 `package` 아티팩트(`dist/*.msix`)를 그대로 쓴다. 다시 빌드하지 않는다.
- 트리거를 하나로 두면 "릴리스된 패키지 = 제출된 패키지"가 구조적으로 보장된다.

### 4-2. 워크플로 초안

```yaml
  store-publish:
    name: Store publish
    if: startsWith(github.ref, 'refs/tags/v')
    needs:
      - release
    runs-on: windows-2025
    timeout-minutes: 30
    environment: store            # required reviewers 로 사람 승인 게이트
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v7
      - uses: actions/download-artifact@v4
        with:
          name: tray-monitor-packages
          path: dist

      - uses: microsoft/microsoft-store-apppublisher@v1.1

      - name: Reconfigure the Store CLI
        shell: pwsh
        run: |
          msstore reconfigure `
            --tenantId ${{ secrets.AZURE_AD_TENANT_ID }} `
            --sellerId ${{ secrets.SELLER_ID }} `
            --clientId ${{ secrets.AZURE_AD_APPLICATION_CLIENT_ID }} `
            --clientSecret ${{ secrets.AZURE_AD_APPLICATION_SECRET }}

      - name: Guard against a pending submission
        shell: pwsh
        # 진행 중인 제출이 있으면 여기서 멈춘다. 새 제출을 겹쳐 만들지 않는다.
        run: |
          $json = msstore submission get ${{ vars.STORE_PRODUCT_ID }} | ConvertFrom-Json
          # status 가 PendingCommit/Certification/... 이면 throw (필드명은 미검증)

      - name: Upload the package as a draft
        shell: pwsh
        run: |
          $msix = (Get-ChildItem dist -Filter '*.msix' | Select-Object -First 1).FullName
          msstore publish $msix -id ${{ vars.STORE_PRODUCT_ID }} --noCommit

      - name: Verify the pricing and the package version
        shell: pwsh
        # 가격이 초기화됐거나 패키지 버전이 태그와 다르면 여기서 끊는다.
        # 실패하면 초안을 폐기하고 사람이 판단한다.
        run: |
          msstore submission get ${{ vars.STORE_PRODUCT_ID }} |
            Out-File -Encoding utf8 submission.json
          # Pricing.PriceId == Tier1022, MarketSpecificPricings 가 KRW 2500 유지,
          # package version == 태그 버전 을 확인 (검증 스크립트는 별도 작성)

      - name: Commit the submission
        shell: pwsh
        run: msstore submission publish ${{ vars.STORE_PRODUCT_ID }}

      - name: Poll the certification
        shell: pwsh
        run: msstore submission poll ${{ vars.STORE_PRODUCT_ID }}
```

`vars.STORE_PRODUCT_ID`는 Environment 변수로 둔다(시크릿이 아니다).

### 4-3. 시크릿과 환경

| 이름 | 종류 | 값 | 비고 |
| --- | --- | --- | --- |
| `AZURE_AD_TENANT_ID` | secret | 미확보 | Entra 개요의 Tenant ID |
| `AZURE_AD_APPLICATION_CLIENT_ID` | secret | 미확보 | Entra 앱 (client) ID |
| `AZURE_AD_APPLICATION_SECRET` | secret | 미확보 | 인증서 인증이면 대신 `--certificateThumbprint` |
| `SELLER_ID` | secret | `94567340` | 확인 완료 |
| `STORE_PRODUCT_ID` | var | `9MT22CHX25FD` | Environment 변수 |

- 저장소 전역이 아니라 **Environment(`store`)에 넣고 required reviewers를 건다.**
- Entra 앱은 계정 전체 **Manager** 권한을 가진다. 유출은 계정 장악과 같다.
  인증서 인증 + 회전 주기 관리 + 최소 권한 검토를 전제로 한다.

### 4-4. 가드

| 가드 | 이유 |
| --- | --- |
| pending 제출 존재 시 중단 | 제출은 동시에 하나만 유효하다. 지금 `Submission 2`처럼 진행 중이면 충돌한다 |
| 태그 버전 == VERSIONINFO | `release` job이 이미 검사한다 |
| 새 MSIX 버전 > 라이브 MSIX 버전 | Store는 버전 증가를 요구한다 |
| `Identity` Name/Publisher 불변 검사 | 바뀌면 새 제품 취급 + 트레이 승격 초기화 |
| `--noCommit` 고정 | 가격 초기화 사고를 커밋 전에 잡는다 |
| 가격 검증 실패 시 초안 폐기 | 반쪽짜리 제출을 남기지 않는다 |
| Environment 승인 | 되돌리기 어려운 단계(인증 제출) 앞에 사람을 둔다 |

### 4-5. 실패와 재시도

- `store-publish`만 실패하면 `workflow_dispatch`로 **태그 ref를 지정해 그 잡만** 재실행한다.
  GitHub 릴리스는 이미 게시되므로 재빌드하지 않는다.
- 초안이 남아 있으면 `msstore submission delete <productId> --no-confirm`으로 폐기하고 다시
  만든다(**미검증**).
- Store API가 유료 제품을 끝내 거부하면 대안은 두 가지다.
  1. **초안 생성까지만 자동**하고, 가격 확인과 제출은 사람이 Partner Center에서 한다.
     (이 경우에도 손작업의 대부분이 사라진다.)
  2. Partner Center 웹 UI를 브라우저로 자동화한다. **취약하므로 최후 수단**으로만 둔다.

## 5. 단계

| 단계 | 내용 | 완료 조건 |
| --- | --- | --- |
| **P1** | 릴리스 오케스트레이션: 버전 범프 PR 자동화(선택), 태그 푸시 | `release` job이 기존과 동일하게 동작 |
| **P2** | **자격증명 스파이크(읽기 전용)** — Entra 앱 등록, Manager 역할, `msstore submission get` | **완료(§8-6)**: 테넌트·앱·Manager 역할·자격증명 동작 확인, `Pricing` 실측 |
| **P3** | `store-publish` 잡 추가(`--noCommit` + 검증 + 승인 + 커밋) | 태그 하나로 Store 제출까지 완료 |
| **P4** | 목록 문구 자동 반영(`listing.md` → `submission updateMetadata`) | 문구 변경이 제출에 반영되고 가격은 불변 |
| **P5** | 게시 검증·알림·(선택) winget 매니페스트 | 게시 전이 감지와 실패 알림 |

**P2를 먼저 하는 이유**: 결과에 따라 P3이 `--priceId` 경로가 될지, "가격이 있는 제출은 사람이
커밋"으로 남을지가 갈린다. 여기서 갈리면 P3·P4 설계가 달라진다.

## 6. 검증 계획

| 검증 | 방법 | 통과 조건 |
| --- | --- | --- |
| 자격증명이 동작하는가 | 로컬에서 `msstore submission get 9MT22CHX25FD` | JSON이 나온다 |
| 제품 가격 모델 | 위 JSON의 `Pricing` | `PriceId` 값과 `MarketSpecificPricings` 존재 여부 기록 |
| 유료 제품 갱신 가능 여부 | `--noCommit`으로 draft 생성 | 거부되지 않는다 |
| 가격 보존 | draft의 가격을 KRW 2,500과 대조 | 그대로다. 아니면 **자동 커밋 금지** |
| 패키지 버전 | draft의 package version | 태그 버전과 같다 |
| 종단 | 태그 푸시 한 번 | GitHub 릴리스 + Store 제출이 모두 완료 |
| 실패 경로 | 태그를 라이브 이하 버전으로 | 가드에서 중단, 제출 없음 |

## 7. 위험과 한계

| 위험 | 영향 | 대응 |
| --- | --- | --- |
| per-market 가격 덮어쓰기 | 원화 등 시장별 가격 변경 | `--noCommit` 고정 + 검증 실패 시 중단 + 승인 게이트 |
| 유료 제품 미지원 | 자동화 불가 | P2에서 실측. 불가면 초안 생성까지만 자동 |
| 진행 중 제출 충돌 | 제출 실패·꼬임 | pending 가드 |
| Entra 앱 권한 남용 | 계정 장악 | 인증서 인증, Environment 보호, 회전 |
| 인증서·시크릿 만료 | 조용한 실패 | 만료 D-day 점검 |
| 인증 제출은 되돌리기 어렵다 | 잘못된 패키지가 심사로 | 승인 게이트 + 버전 대조 |
| 문서 경고와 실제 동작 불일치 | 설계 오판 | P2 전에는 Store 업로드 자동화를 켜지 않는다 |
| 목록 문구 교체 사고 | 설명·특징 유실 | get → patch → update, 가격 필드 보존, 초안 검토 |

## 8. P2 실측 기록 (2026-10-21)

자격증명을 만들기 전에 확인할 수 있는 것까지 실측했다. **결론: 이 Partner Center 계정에는
연결된 Entra 테넌트가 없고, 그래서 Store submission API 경로가 자격증명 단계에서 막혀 있다.**

### 8-1. 로컬 도구

| 항목 | 결과 |
| --- | --- |
| .NET 9 데스크톱 런타임 | 없었다(8·10만 있었다). `winget install Microsoft.DotNet.DesktopRuntime.9`로 9.0.21 설치 |
| msstore CLI | `winget install --id 9P53PC5S0PHJ --source msstore` → **0.4.3.2**. winget 소스가 아니라 **msstore 소스**에만 있다 |
| 인증 방식 | `reconfigure`에는 tenantId·sellerId·clientId + (clientSecret \| 인증서 \| `--clientAssertion`)만 있다. **인터랙티브 로그인이 없다** → Entra 앱 등록이 필수 |
| 자격증명이 없을 때 | `msstore info` → `crit: MSStore.CLI.Program[0] SellerId is not set.` |

### 8-2. Partner Center 계정 상태

| 확인 | 결과 |
| --- | --- |
| `Account settings → Tenants` | "Current tenant associations" 표가 **비어 있다**. 연결된 Entra 테넌트가 없다 |
| `Account settings → User management` | "You are currently signed in as admin@tinyrack.net. To manage users, sign in with your associated Microsoft Entra ID credentials." 로그인을 시도하면 `account/v3/notauthorized`로 떨어진다 |
| 계정 성격 | `Sign in with Microsoft Entra ID`의 계정 선택기가 **"Sign in with admin@tinyrack.net personal Microsoft account"** 로 표시한다. 즉 **MSA(개인 계정) 기반**이고 Entra 워크 계정이 아니다 |
| `Create Microsoft Entra ID` | `signup.microsoft.com/createaccount?scenario=PartnerCenter`로 이동한다. 단계가 `Account details → Sign-in details → **Payment setup and finish**` 이므로 **결제 수단이 필요하다** |
| `Associate Microsoft Entra ID` | 연결할 기존 테넌트가 있어야 하고, 그 테넌트의 global admin 자격으로 로그인해야 한다 |
| `My access` | 워크스페이스는 Apps and games·Edge·Insights·Earnings 모두 "Access granted". 막는 것은 프로그램 권한이 아니라 **계정 관리(Entra) 권한**이다 |

두 버튼(`Create`·`Associate`)은 활성 상태다. 회색으로 보이는 것은 Fluent의 subtle 버튼 스타일이며
DOM에서 `disabled=false`로 확인했다.

미확인: `Create Microsoft Entra ID`가 이 계정에 테넌트를 **덧붙이는지**, 아니면 별도 워크 계정
온보딩을 시작하는지. 흐름이 `signup.microsoft.com`(결제 포함)이라 "빈 테넌트만 만드는 것"과
같지 않다.

### 8-3. 왜 막히는가

Store submission API는 **Entra 앱 + Manager 역할**로만 인증한다. Entra 앱은 테넌트 안에만
존재하고, 그 테넌트를 Partner Center 계정에 연결해야 한다. 이 계정은 MSA 기반이라 그 테넌트가
없다. 따라서 P3 설계 이전에 **테넌트 확보**가 선행 과제다.

### 8-4. 선택지

| 안 | 내용 | 비용·위험 |
| --- | --- | --- |
| A | 기존 Entra 테넌트를 `Associate Microsoft Entra ID`로 연결 | 이미 테넌트가 있어야 하고 그 global admin 계정이 필요하다. 개인·회사 디렉터리를 Store 배포에 끌어들이는 결과 |
| B | Partner Center `Create Microsoft Entra ID` | **결제 수단 필요**. 절차가 계정 온보딩에 가까워 부작용 확인이 필요하다 |
| C | Entra 관리 센터에서 무료 테넌트를 만든 뒤 A로 연결 | **막힘(§8-5)**. MSA만으로는 테넌트를 만들 수 없다는 것을 실측했다 |
| D | API 자동화를 접고 **초안 생성까지 반자동 유지** | 태그 → GitHub 릴리스는 그대로. Store는 사람이 Partner Center에서 `Start update` |
| E | 계정을 Company/워크 계정으로 이전 | 변경이 가장 크다. 게시된 제품·평점·트레이 승격이 위험하다 |

### 8-5. C안(무료 테넌트 생성) 시도 결과 — 실패

MSA 기반 계정으로는 테넌트를 만들 수 없다. 세 경로를 모두 실측했다.

| 경로 | 결과 |
| --- | --- |
| Entra 관리 센터 (`entra.microsoft.com`) | 로그인은 되지만 포털이 `interaction_required: AADSTS16000: User account ... from identity provider 'live.com' does not exist in tenant 'Microsoft Services'`로 멈춘다. 테넌트 생성 블레이드(`CreateTenantBlade`)는 404 |
| Azure 포털 `Directories + subscriptions` | **`All Directories → No directories found`**. 관리할 수 있는 디렉터리가 0개이고, 현재 디렉터리는 `f8cdef31-a31e-4b4a-93e4-5f571e91255a`(MSA 기본) 뿐이다 |
| Azure 포털 테넌트 생성 (`#create/Microsoft.AzureActiveDirectory`) | **401 No access** |

정리하면 테넌트 생성에는 **워크 계정이나 Azure 구독(결제 수단)** 이 필요하다. 이 계정
(MSA + 결제 수단 없음)에서는 세 경로가 모두 막힌다는 것이 실측 결과다.

남는 현실적 경로:

| 안 | 내용 | 대가 |
| --- | --- | --- |
| B | Partner Center `Create Microsoft Entra ID` | **결제 수단 필요**. 문서상 정식 경로이고 가장 지속 가능하다 |
| B' | Microsoft 365 개발자 프로그램 테넌트를 만들어 연결 | 무료지만 **개발용 테넌트**이고 90일 갱신·회수 위험이 있다. 프로덕션 배포 자격증명으로는 부적합하다(가능한지는 추가 확인 필요) |
| D | API 자동화를 접고 반자동 유지 | 자동화 이득 없음. 계정 변경도 없음 |

### 8-6. B안 실행 결과 + API 실측 (2026-10-21, 완료)

Partner Center `Create Microsoft Entra ID` → `signup.microsoft.com` 흐름으로 테넌트와 업무 계정을
만들고, 그 테넌트에 앱을 등록해 Partner Center에 **Manager(Windows)** 역할로 추가했다.
`msstore reconfigure`가 **"It seems to be working!"** 로 끝나서 자격증명이 실제로 동작한다는 것까지
확인했다.

| 항목 | 값 |
| --- | --- |
| 테넌트 | `tinyrack.onmicrosoft.com` / `18e19524-3424-4990-82d6-c7b1d12a40bd` |
| 업무 계정 | `contact@tinyrack.onmicrosoft.com` (Partner Center Account admin, MFA=TOTP) |
| 결제 계정 | Business name `tinyrack`, 주소는 개인 주소, 카드 = 토스카드 |
| Entra 앱 | `TinyTrayManager Store automation` / clientId `fe09a92e-39bd-4a94-89c8-918f8ce3b4be` |
| 클라이언트 시크릿 | secret ID `7e2115e6-7401-414c-b90d-a45235ea7545`, **만료 2028-10-20** |
| 비밀 저장 | Bitwarden 2건 — 업무 계정(+TOTP), CI용 앱 자격증명 |
| GitHub | Environment `store` 에 secret 4종 + variable `STORE_PRODUCT_ID` |

같은 작업을 다시 할 때 걸리는 지점:

- **주소 검증 경고가 두 번 뜬다.** "We couldn't validate your address" → `Use this address`로 통과한다.
  한국 주소는 이 검증을 자주 통과하지 못한다.
- **카드에 따라 거부된다.** 트레볼로그(Mastercard)는 두 번 거부됐고 토스카드로 성공했다.
- **MFA 등록이 필수다.** "Action required ... set up multifactor authentication". 인증 앱 QR 대신
  `Can't scan the QR code?`의 수동 키를 받아 Bitwarden TOTP(`bw get totp`)로 등록해 해결했다.
- **테넌트는 생성만으로 붙지 않는다.** `Associate Microsoft Entra ID`에서 업무 계정으로 로그인해야
  연결되고, 그 시점에 Partner Center 세션도 업무 계정으로 바뀐다.

#### Pricing 실측 — 자동화 설계를 바꾸는 사실

`msstore submission get 9MT22CHX25FD`로 마지막 게시 제출을 덤프했다.

| 관측 | 값 | 의미 |
| --- | --- | --- |
| `Pricing.PriceId` | **`"Base"`** | 문서가 지목한 그 값이다. 기본 가격이 시장별로 관리된다는 뜻 |
| `Pricing.IsAdvancedPricingModel` | **`true`** | **Pricing Version 2 확정** |
| `Pricing.MarketSpecificPricings` | 2건(`LB`, `EH` = `NotAvailable`) | 실제 시장별 가격은 여기에 없다 |
| `StatusDetails.Warnings` | `SalesUnsupportedWarning: The sales resource is no longer supported. To view or edit the sales data ... use the Dev Center dashboard.` | **API로 가격을 읽거나 쓸 수 없다** |

문서가 경고한 조건(`PriceId == "Base"`)이 우리 제품에서 그대로 재현됐다. 따라서:

- `msstore publish`를 `--priceId` 없이 돌리면 CLI는 "가격을 free로 되돌릴 수 있다"며 **멈춘다.**
- `submission update`는 `PriceId`가 `Tier<number>`/`Free`/`NotAvailable`이어야 하므로 `Base`를 그대로
  보내면 **거부된다.**
- 시장별 가격은 API의 sales 리소스에 있는데 그 리소스가 **더 이상 지원되지 않는다.** 그래서 초안을
  만들어도 **원화 가격이 보존됐는지 API로 검증할 수 없다.**

결론: **API로 가격을 건드리는 단계는 자동화하지 않는다.** 자동화할 수 있는 최대치는
`--priceId Tier1022`(USD 1.99에 해당하는 tier, 역산값 — 미검증)로 초안을 만드는 것까지이고,
커밋은 사람이 Partner Center의 `Pricing and availability`를 눈으로 확인한 뒤에 한다.

덧붙여: 같은 덤프에서 **0.8.0 업데이트가 `Status: Published`, 패키지 `0.8.0.0`** 인 것도 확인했다.
스토어 페이지 가격은 그대로 **KRW 2,500** 이다.

## 9. 결정 필요 / 미검증

해결됨:

- 테넌트 확보 — **B안으로 완료**(§8-6). Entra 앱 + Manager(Windows) 역할 + 자격증명 동작 확인
- 로컬 도구 — 갖춰짐(§8-1)
- 유료 제품에서 `msstore` 동작 — **동작한다**(§8-6)
- 제품 `Pricing`의 형태 — **`PriceId="Base"`, `IsAdvancedPricingModel=true`**(§8-6)

결정 필요:

1. **범위** — 가격을 API로 검증할 수 없으므로 "초안 생성까지 자동 + 사람 커밋"이 사실상 상한이다.
   그대로 갈지, 아니면 Store 제출을 계속 수동으로 둘지
2. **승인 게이트** — Environment `store` 에 required reviewers 를 걸지
3. **인증 방식** — 지금은 클라이언트 시크릿(만료 2028-10-20). 인증서로 바꿀지
4. **버전 범프 자동화** — release-please 류 도입 여부
5. **알림 수단** — 실패·게시 알림을 어디로 보낼지

미검증:

- `--priceId Tier1022`가 실제로 USD 1.99 기본가인지 (문서 예시 `Tier1052 = $4.99`에서 역산)
- `msstore publish --noCommit`이 `PriceId="Base"` 제품에서 어떤 초안을 만드는지, 가격이 어떻게 바뀌는지
- `submission update`로 목록 문구만 교체하는 것이 가능한지(`PriceId` 요구 때문에 어려울 가능성)
- `submission delete`의 정확한 동작
- GitHub Actions 러너에서 .NET 9 설치를 `microsoft-store-apppublisher` 액션이 처리하는지
- 클라이언트 시크릿 만료(2028-10-20) 전 회전 절차

## 10. 참고 자료

- [Publish app updates to Microsoft Store with GitHub Actions](https://learn.microsoft.com/en-us/windows/apps/publish/msstore-dev-cli/github-actions)
- [Microsoft Store Developer CLI commands (MSIX)](https://learn.microsoft.com/en-us/windows/apps/publish/msstore-dev-cli/commands) — §Paid apps and pricing, 티어 표
- [microsoft/msstore-cli](https://github.com/microsoft/msstore-cli) — [#143 유료 앱 갱신](https://github.com/microsoft/msstore-cli/issues/143), [#112 가격 초기화 버그](https://github.com/microsoft/msstore-cli/issues/112)
- [Create and manage submissions (Store submission API)](https://learn.microsoft.com/en-us/windows/uwp/monetize/create-and-manage-submissions-using-windows-store-services) — Pricing Version 2 제약
- `docs/PACKAGING.md` §8 (후속), `.github/workflows/pipeline.yml`
