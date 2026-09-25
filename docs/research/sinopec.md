# Sinopec (`sinopecsales.com`) Protocol Research Knowledge Base

Last verified: `2026-09-25`  
Target host: `https://www.sinopecsales.com/`  
Methodology: `Observe -> Hypothesize -> Experiment -> Verify -> Reproduce -> Implement -> Test`

---

## 1. Portal Architecture & Account Modes

| Dimension | Personal (`AccountMode::Personal`) | Corporate (`AccountMode::Corporate`) |
| :--- | :--- | :--- |
| Entry page | `https://www.sinopecsales.com/` (`default.html`) | `https://www.sinopecsales.com/default_corp.html` |
| Context prefix | `/gas/` | `/corpgas/` |
| Login UI | Legacy SSO `/websso/login.action` returns `403 Forbidden` (nginx); personal users are routed to YiJie App / Mini-program (`website/html/wxPrompt.html`) | Active iframe `https://www.sinopecsales.com/corpgas/res/html/login/login_pc.jsp` opened via `showBg('corp')` |
| Monetary unit | Integer **Fen (分)** (`String.prototype.parseFen2Yuan` in `res/js/money.js`) | Integer **Fen (分)** (`preBalance`, `balance`, `cardBalance`) |
| Unauthenticated HTTP status | Redirect script or `memberAccount == ""` | **HTTP `390`** (`<script>window.open('/default_corp.html','_top')</script>`) |
| Profile incomplete HTTP status | `391` (`showZLWS()`) | `391` (`showZLWS()`) |

---

## 2. Authentication & Session Persistence Findings

- **Session Cookies Required (`CONFIRMED`)**:
  - `JSESSIONID` (`Path=/corpgas; Secure; HttpOnly`)
  - `MYSERVERID_corpgas` (`Domain=www.sinopecsales.com; Path=/corpgas; Secure; HttpOnly`, sticky load-balancer affinity cookie)
  - `aliyungf_tc` (`Path=/; HttpOnly`, Alibaba Cloud WAF cookie)
  - `acw_tc` (`Path=/; HttpOnly; Max-Age=1800`, Alibaba Cloud WAF cookie)
  - `LASTMSG` (encrypted last-login form state stored in cookie by `login_pc.jsp` after successful SMS login)
- **Session Expiry Detection (`CONFIRMED`)**:
  - Calling any protected `/corpgas/webjsp/*.json` or `/corpgas/webjsp/*.jsp` endpoint without a valid session returns **HTTP status code `390`** (handled by `onJsonError(reqStatus == 390)` in `default_corp.js`).

---

## 3. Endpoint Inventory

### 3.1 Corporate Session Status Check
- **Function Name**: Corporate Auth Session Check (`logInOrOut`)
- **Status**: `CONFIRMED`
- **Last Verified**: `2026-09-25`
- **Account Mode**: `Corporate` (`Personal` uses `/gas/html/memberLoginAction_logInOrOut.json`)
- **Method & Endpoint**: `POST https://www.sinopecsales.com/corpgas/html/memberLoginAction_logInOrOut.json`
- **Response Schema**:
  ```json
  {
    "basePath": "https://www.sinopecsales.com:443/gas/",
    "bulletin": "hide",
    "ssoServerRootURL": "https://www.sinopecsales.com/websso",
    "memberAccount": "",
    "cardNum": 0
  }
  ```
- **Read-Only**: Yes (Zero business side effects)
- **Rust Implementation**: `SinopecHttpTransport::check_login_or_out`

### 3.2 Account Feature & Login State Flag Check
- **Function Name**: Company Account Mode & Feature Detector (`getCompType`)
- **Status**: `CONFIRMED`
- **Last Verified**: `2026-09-25`
- **Method & Endpoint**: `POST https://www.sinopecsales.com/corpgas/html/memberLoginAction_getcompType.json`
- **Response Schema**:
  ```json
  {
    "openWeixinPay": 1,
    "compType": "show",
    "ecard": 1,
    "dianpingOpen": 1,
    "stkOpen": 1,
    "openIve": 2,
    "eCoupon": 2,
    "corpswitch": 0,
    "isEcouponOpen": 0,
    "loginstate": "N"
  }
  ```
- **Read-Only**: Yes

### 3.3 Corporate Arithmetic Image Captcha
- **Function Name**: Corporate Login Captcha (`YanZhengMaServlet`)
- **Status**: `CONFIRMED`
- **Last Verified**: `2026-09-25`
- **Method & Endpoint**: `GET https://www.sinopecsales.com/corpgas/YanZhengMaServlet`
- **Notes**: Returns an image calculation challenge (`请输入计算结果`); answer is submitted in the `check` field.

### 3.4 Corporate Login SMS Verification Trigger
- **Function Name**: Send Corporate Login SMS (`sendyzm` / `getSrand`)
- **Status**: `CONFIRMED`
- **Last Verified**: `2026-09-25`
- **Method & Endpoint**:
  - Standard: `POST https://www.sinopecsales.com/corpgas/html/loginAction_smsYzm.json`
  - Province-specific fallback (`err == 2`): `POST https://www.sinopecsales.com/corpgas/html/loginAction_gsdSmsYzm.json`
- **Request Form Fields (`application/x-www-form-urlencoded`)**:
  - `tax`: Unified Social Credit Code (`taxtype=1`) or Company Name (`taxtype=2`)
  - `taxtype`: `1` or `2`
  - `idtype`: `01` (身份证), `02` (军官证), `03` (护照), etc.
  - `idno`: Master card holder ID number
  - `name`: Master card holder full name
  - `mobile`: Master card holder 11-digit phone number
  - `check`: Arithmetic captcha result from `YanZhengMaServlet`
  - `jsprovince`: Province code from cookie (`12` for Tianjin, `11` for Beijing)
  - `province`: Selected province code
  - `onceCard`: `0` (all cards) or `1` (single 19-digit card login when query exceeds 10s)
  - `cardno`: 19-digit master fuel card number (required when `onceCard == 1`)

### 3.5 Corporate SMS Login Submit
- **Function Name**: Corporate Login Submit (`loginBySms`)
- **Status**: `CONFIRMED`
- **Last Verified**: `2026-09-25`
- **Method & Endpoint**: `POST https://www.sinopecsales.com/corpgas/html/loginAction_smsLogin.json`
- **Request Form Fields**:
  Same as `loginAction_smsYzm.json`, replacing `check` with `smsYzm` (6-digit SMS code) and optional `tjm` (referral code).
- **Success Behavior**:
  Sets `JSESSIONID`, `MYSERVERID_corpgas`, and `LASTMSG`, then navigates top window to `/default_corp.html`.

### 3.6 Master Card & Account Profile Query (`AuthVerifier`)
- **Function Name**: Master Card Balance & Profile Query
- **Status**: `CONFIRMED`
- **Last Verified**: `2026-09-25`
- **Method & Endpoint**: `POST https://www.sinopecsales.com/corpgas/webjsp/billQueryAction_queryBalance.json`
- **Authentication**: Requires valid `JSESSIONID` + `MYSERVERID_corpgas`. Returns HTTP `390` when unauthenticated.
- **Response Fields (`cardInfo` & `cardMember`)**:
  - `cardInfo.cardNo`: 19-digit card number (`cardNo[3..5] == "03"` -> Electronic Card, otherwise Entity Card; `cardNo[6..8]` or `cardNo[8..10]` -> Province code)
  - `cardInfo.priCard`: `"1"` (Master Card) or `"0"` (Vice Card)
  - `cardInfo.cardHolder`: Holder name
  - `cardInfo.compName`: Company name
  - `cardInfo.compNo`: Customer code
  - `cardInfo.tax`: Tax ID / Unified Social Credit Code
  - `cardInfo.invoiceType`: `"2"` (增值税专用发票) or `"1"` (增值税普通发票)
  - `cardInfo.compType`: `"01"` (单位单用户), `"02"` (单位多用户), `"03"` (个人单用户), `"04"` (个人多用户), `"07"` (不记名客户)
  - `cardInfo.preBalance`, `cardInfo.balance`, `cardInfo.cardBalance`: Balances in integer **Fen (分)**
- **Rust Implementation**: `SinopecHttpTransport::probe_corporate_balance` + `SinopecParser::parse_balance_response`

### 3.7 Bound Fuel Card List & Card Switching
- **Function Name**: Bound Fuel Card List (`qiehuanCards` / `gainCardNo`)
- **Status**: `CONFIRMED`
- **Last Verified**: `2026-09-25`
- **Method & Endpoints**:
  - `POST https://www.sinopecsales.com/corpgas/webjsp/memberOilCardAction_queryMyOilCardList.json`
  - `POST https://www.sinopecsales.com/corpgas/webjsp/billQueryAction_gainCardNo.json`
  - Switch active card: `GET https://www.sinopecsales.com/corpgas/webjsp/memberOilCardAction_qiehuanCardAction.action?t=c&num={cardNo}`
- **Rust Implementation**: `SinopecHttpTransport::query_card_list`

### 3.8 Consumption & Transaction Detail Query
- **Function Name**: Consumption & Recharge Detail (`jymxcx` / `czmxcx`)
- **Status**: `HYPOTHESIS` (Entry URLs confirmed in `carManager.jsp`; inner XHR JSON action requires authenticated capture)
- **Entry URLs**:
  - Consumption detail: `https://www.sinopecsales.com/corpgas/webjsp/query/billDetail.jsp`
  - Recharge detail: `https://www.sinopecsales.com/corpgas/webjsp/query/chargeDetail.jsp`
  - Pre-allocation detail: `https://www.sinopecsales.com/corpgas/webjsp/query/yfpcx.jsp`

### 3.9 Corporate Invoice Quota Query
- **Function Name**: Invoice Quota (`fpedcx`)
- **Status**: `HYPOTHESIS` (Entry URL confirmed in `carManager.jsp`)
- **Entry URL**: `https://www.sinopecsales.com/corpgas/webjsp/invoice/queryAmount.jsp`

### 3.10 Electronic Invoice Issue & Invoice Bill Download
- **Function Name**: Electronic Invoice V2 (`createInvoicev2` & `invoiceBill`)
- **Status**: `UNKNOWN` (Requires Human Demonstration before executing any real submit)
- **Entry URLs**:
  - Issue electronic invoice: `https://www.sinopecsales.com/corpgas/webjsp/invoicev2/createInvoice.jsp`
  - Query & download invoice bills: `https://www.sinopecsales.com/corpgas/webjsp/invoicev2/queryInvoiceBill.jsp`
  - Scheduled invoicing: `https://www.sinopecsales.com/corpgas/webjsp/carManager/quartzInvoice.jsp`

---

## 4. Official Business & Security Constraints (`website/html/service/ywlc.html` & `gnjs.htm`)

- **Account Lockout Threshold (`CONFIRMED`)**:
  - 6 failed login attempts in a single day locks the Sinopec account for the remainder of the day (`ACCOUNT_LOCKED`, unlocks the next calendar day).
  - `sinopec-server` enforces `SINOPEC_MAX_AUTH_ATTEMPTS=3` by default to prevent accidental account lockout.
- **Master / Vice Card Hierarchy (`CONFIRMED`)**:
  - Vice cards (`priCard != "1"`) are automatically bound under their Master Card (`priCard == "1"`).
  - Online recharge (`zxcz`), pre-allocation (`yfp`), and invoice issuance must be initiated from the Master Card (`cardType == '2'` triggers `alert("请使用主卡进行此操作")`).
- **VAT Special Invoice vs Normal Invoice Rules (`CONFIRMED`)**:
  - Cards with `invoiceType == "2"` (`增值税专用发票`) lock the invoice title and tax code (`tax`) to the corporate customer's registered company profile (`invoice_title_locked = true`).
  - Cards with `invoiceType == "1"` (`普通发票`) support standard electronic VAT normal invoices.

---

## 5. Confirmed Electronic Invoice V2 Protocol (`CONFIRMED` on 2026-09-25)

All `invoicev2Action_*.json` endpoints MUST be called via `POST` with `Content-Type: application/x-www-form-urlencoded; charset=UTF-8` and parameters in the **request body** (never in the URL query string `?`, which triggers Struts2 `DataErrorFilter` `<title>数据错误</title>` and invalidates `JSESSIONID`).

1. **Pre-SMS Captcha Validation (`CONFIRMED`)**:
   - `POST https://www.sinopecsales.com/corpgas/html/loginAction_validatejs.json`
   - Form fields: `mobile`, `check`, `idno`, `name`, `idtype`, `onceCard`, `cardno`
   - Returns: `{"result":0}` on valid captcha; must be called immediately before `loginAction_smsYzm.json`.
2. **Query Bound Card & Default Invoice Mode (`CONFIRMED`)**:
   - `POST https://www.sinopecsales.com/corpgas/webjsp/invoicev2Action_queryBindedCardNoList.json`
   - Returns: `{"defaultCardNo":"****0729","invType":"02","invoiceType":2,"pod":{...}}`
   - `invType == "02"` = Consumption-based invoicing (`kplx="10"`), `invoiceType == 2` = VAT Special Invoice (`fplx="02"`).
3. **Query Un-invoiced Transactions & Quota Pool (`CONFIRMED`)**:
   - `POST https://www.sinopecsales.com/corpgas/webjsp/invoicev2Action_queryTransList.json`
   - Form fields: `cardNo`, `merge` (`"0"` detail / `"1"` summary), `startDate` (`YYYY-MM-DD`), `endDate` (`YYYY-MM-DD`), `createWay=1`, `kplx` (`"10"` consume / `"01"` recharge), `fplx` (`"02"` special / `"01"` normal).
   - Returns:
     - `cardInfo`: `invoicelmt` (available invoice quota in Fen), `usedlmtamt` (last issued invoice amount in Fen), `enddate` (`YYYYMMDD`), `invoiceTitle`, `taxNo`, `bankName`, `bankAccount`, `address`, `phone`.
     - `list`: array of un-invoiced items with `transId`, `datetime` (`YYYYMMDDHHmmss`), `nodeName`, `tradeName`, `tradeClass`, `taxrate`, `uninvoiceAmt` (in Fen), `reduceAmt` (in Fen).
4. **Query Issued Electronic Invoices & Download Links (`CONFIRMED`)**:
   - `POST https://www.sinopecsales.com/corpgas/webjsp/invoicev2Action_queryPlainList.json`
   - Form fields: `cardNo`, `startDate`, `endDate`, `pageNo=1`.
   - Returns `list` of issued electronic invoices (`invoiceNo`, `id`, `einvReqKey`, `money` in Fen, `addDate`, and direct download `url`: `https://invoice.sinopec.com/...`).
5. **Query Corporate Invoice Email & Display Config (`CONFIRMED`)**:
   - `POST https://www.sinopecsales.com/corpgas/webjsp/invoicev2Action_queryCompConfig.json`
   - Form fields: `cardNo`.
   - Returns: `{"config":{"mail":"...","addrSellFlag":"Y","addrBuyFlag":"Y","bankBuyFlag":"Y","bankSellFlag":"Y"}, "code":0}`.
