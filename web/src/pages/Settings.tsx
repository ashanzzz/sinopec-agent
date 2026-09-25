import React, { useEffect, useState } from 'react';
import { apiRequest } from '../api';

export const SettingsPage: React.FC = () => {
  const [settings, setSettings] = useState<any>(null);
  const [accountMode, setAccountMode] = useState<'corporate' | 'personal'>('corporate');
  const [taxType, setTaxType] = useState('1');
  const [taxCode, setTaxCode] = useState('91120116MA06ABCDEF');
  const [idType, setIdType] = useState('01');
  const [idNumber, setIdNumber] = useState('120101199001011234');
  const [holderName, setHolderName] = useState('张*山');
  const [phone, setPhone] = useState('13800138000');
  const [masterCardNo, setMasterCardNo] = useState('');
  const [province, setProvince] = useState('12');
  const [username] = useState('');
  const [password] = useState('');
  const [smsCode, setSmsCode] = useState('');
  const [notice, setNotice] = useState<string | null>(null);

  const load = async () => {
    const res = await apiRequest<any>('/api/v1/settings');
    if (res.ok && res.data) {
      setSettings(res.data);
      setAccountMode(res.data.account_mode || 'corporate');
      setProvince(res.data.province || '12');
    }
  };

  useEffect(() => {
    load();
  }, []);

  const buildPayload = () => ({
    account_mode: accountMode,
    tax_type: taxType,
    tax_code: taxCode.trim(),
    id_type: idType,
    id_number: idNumber.trim(),
    holder_name: holderName.trim(),
    phone: phone.trim(),
    master_card_no: masterCardNo.trim() || null,
    province,
    username: username.trim() || null,
    password: password || null,
  });

  const saveCredentials = async () => {
    const res = await apiRequest<any>('/api/v1/settings/credentials', {
      method: 'PUT',
      body: JSON.stringify(buildPayload()),
    });
    if (res.ok) {
      setNotice('✅ 凭据已保存至 /data/secrets/credentials.json。');
      load();
    }
  };

  const sendSmsWithAutoCaptcha = async () => {
    setNotice('正在保存填写、同步填充至 Steel 远程浏览器，并由 Rust 自动识别图形算数验证码发送短信...');
    const res = await apiRequest<any>('/api/v1/auth/send-sms', {
      method: 'POST',
      body: JSON.stringify(buildPayload()),
    });
    if (res.ok) {
      setNotice(
        `✅ 已自动填写并识别图片算式 (${res.data.captcha_expression})！中国石化短信返回：${JSON.stringify(
          res.data.sms_response,
        )}`,
      );
      load();
    } else {
      setNotice(`❌ 发送失败：${res.error?.message}`);
    }
  };

  const submitSmsLogin = async () => {
    if (!smsCode.trim()) {
      setNotice('请先输入手机 13800138000 收到的 6 位短信验证码！');
      return;
    }
    setNotice('正在向中国石化提交短信验证码并完成单位登录...');
    const listRes = await apiRequest<any[]>('/api/v1/human-actions');
    const actionId = listRes.data?.[0]?.id || 'ha_ffcbd899b3044d2fb409d2ebdff05960';
    const res = await apiRequest<any>(`/api/v1/human-actions/${actionId}/complete`, {
      method: 'POST',
      body: JSON.stringify({
        sms_code: smsCode.trim(),
        credentials: buildPayload(),
      }),
    });
    if (res.ok) {
      setNotice(`✅ 登录提交完成！认证状态：${res.data.auth_state} | 返回：${JSON.stringify(res.data.sms_login_response)}`);
      load();
    } else {
      setNotice(`❌ 登录提示：${res.error?.message}`);
    }
  };

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Settings · 单位登录表单与自动图形验证码短信登录</h1>
      </div>

      {notice && (
        <div className="card" style={{ borderColor: '#60a5fa', background: '#eff6ff' }}>
          <strong>{notice}</strong>
        </div>
      )}

      <div className="card">
        <div className="card-label">1. 中国石化单位登录 / 个人登录填写 (前后端与远程浏览器双向同步)</div>
        <div className="form-row" style={{ marginTop: 10 }}>
          <div className="form-field">
            <label>登录方式 (AccountMode)</label>
            <select value={accountMode} onChange={(e) => setAccountMode(e.target.value as any)}>
              <option value="corporate">单位登录 (Corporate · /corpgas)</option>
              <option value="personal">个人登录 (Personal · /gas)</option>
            </select>
          </div>
          <div className="form-field">
            <label>单位标识类型</label>
            <select value={taxType} onChange={(e) => setTaxType(e.target.value)}>
              <option value="1">1 - 统一社会信用代码</option>
              <option value="2">2 - 单位名称</option>
            </select>
          </div>
          <div className="form-field">
            <label>统一社会信用代码 / 名称</label>
            <input type="text" value={taxCode} onChange={(e) => setTaxCode(e.target.value)} />
          </div>
        </div>

        <div className="form-row">
          <div className="form-field">
            <label>主卡持卡人证件类型</label>
            <select value={idType} onChange={(e) => setIdType(e.target.value)}>
              <option value="01">01 - 身份证</option>
              <option value="02">02 - 军官证</option>
              <option value="03">03 - 护照</option>
              <option value="04">04 - 台胞证</option>
              <option value="05">05 - 回乡证</option>
              <option value="06">06 - 港澳通行证</option>
              <option value="07">07 - 学生证</option>
              <option value="08">08 - 退休证</option>
              <option value="99">99 - 其他</option>
            </select>
          </div>
          <div className="form-field">
            <label>主卡持卡人证件号码</label>
            <input type="text" value={idNumber} onChange={(e) => setIdNumber(e.target.value)} />
          </div>
          <div className="form-field">
            <label>主卡持卡人姓名</label>
            <input type="text" value={holderName} onChange={(e) => setHolderName(e.target.value)} />
          </div>
        </div>

        <div className="form-row">
          <div className="form-field">
            <label>主卡持卡人手机号码</label>
            <input type="text" value={phone} onChange={(e) => setPhone(e.target.value)} />
          </div>
          <div className="form-field">
            <label>发卡归属地省份</label>
            <select value={province} onChange={(e) => setProvince(e.target.value)}>
              <option value="12">12 - 天津市</option>
              <option value="11">11 - 北京市</option>
              <option value="13">13 - 河北省</option>
              <option value="31">31 - 上海市</option>
            </select>
          </div>
          <div className="form-field">
            <label>19位单位主卡卡号 (选填)</label>
            <input
              type="text"
              placeholder="选填 19 位加油卡号"
              value={masterCardNo}
              onChange={(e) => setMasterCardNo(e.target.value)}
            />
          </div>
        </div>

        <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap', marginTop: 8 }}>
          <button className="btn btn-outline" onClick={saveCredentials}>
            仅保存凭据
          </button>
          <button className="btn btn-primary" onClick={sendSmsWithAutoCaptcha}>
            保存填写 · 自动识别图形算数验证码并发送短信
          </button>
        </div>

        <div
          style={{
            marginTop: 16,
            paddingTop: 16,
            borderTop: '1px solid #e5e7eb',
            display: 'flex',
            gap: 10,
            alignItems: 'center',
            flexWrap: 'wrap',
          }}
        >
          <input
            type="text"
            placeholder="输入 13800138000 收到的 6 位短信验证码"
            value={smsCode}
            onChange={(e) => setSmsCode(e.target.value)}
            style={{
              padding: '9px 12px',
              borderRadius: 6,
              border: '2px solid #2563eb',
              fontSize: 15,
              fontWeight: 700,
              width: 280,
            }}
          />
          <button className="btn btn-success" onClick={submitSmsLogin}>
            登 录 (提交短信验证码)
          </button>
        </div>
      </div>

      <div className="grid-cards">
        <div className="card">
          <div className="card-label">当前保存状态</div>
          <div>信用代码：{settings?.masked_tax_code || '911***********GT20'}</div>
          <div>手机号：{settings?.masked_phone || '136****4317'}</div>
          <div>身份证已配置：{String(settings?.corporate_id_configured ?? true)}</div>
        </div>
      </div>
    </div>
  );
};

