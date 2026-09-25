# Authentication & Session Persistence (`AUTH.md`)

## 1. Corporate vs Personal Login on `sinopecsales.com`

- **Corporate (`https://www.sinopecsales.com/default_corp.html`)**:
  - Loads `/corpgas/res/html/login/login_pc.jsp`.
  - Requires:
    1. Unified Social Credit Code (`tax`, `taxtype=1`) or Company Name (`taxtype=2`)
    2. Master card holder ID type (`idtype=01`) and ID number (`idno`)
    3. Master card holder name (`name`)
    4. Master card holder mobile phone (`mobile`)
    5. Arithmetic image captcha (`GET /corpgas/YanZhengMaServlet` -> `check`)
    6. SMS code (`POST /corpgas/html/loginAction_smsYzm.json` -> `smsYzm`)
    7. Final submit (`POST /corpgas/html/loginAction_smsLogin.json`)
- **Session Persistence**:
  - `SteelBrowserDriver::sync_sinopec_cookies` extracts `JSESSIONID`, `MYSERVERID_corpgas`, `acw_tc`, `aliyungf_tc`, and `LASTMSG` from Steel Browser's `/v1/sessions/{id}/context` API and persists them to `/data/auth/sinopec_cookies.json`.
  - `AuthVerifier` calls `POST /corpgas/webjsp/billQueryAction_queryBalance.json`. HTTP `390` indicates an expired session.
