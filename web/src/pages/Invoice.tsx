import React, { useState } from 'react';
import { apiRequest } from '../api';

export const InvoicePage: React.FC = () => {
  const [startDate, setStartDate] = useState('2026-09-01');
  const [endDate, setEndDate] = useState('2026-09-30');
  const [cardId, setCardId] = useState('');
  const [invoiceType, setInvoiceType] = useState('增值税专用发票');
  const [preview, setPreview] = useState<any>(null);
  const [submitMsg, setSubmitMsg] = useState<string | null>(null);
  const [humanUrl, setHumanUrl] = useState<string | null>(null);

  const handlePreview = async () => {
    setSubmitMsg(null);
    setHumanUrl(null);
    const res = await apiRequest<any>('/api/v1/invoices/preview', {
      method: 'POST',
      body: JSON.stringify({
        start_date: startDate,
        end_date: endDate,
        card_id: cardId || null,
        invoice_type: invoiceType || null,
      }),
    });
    if (res.ok) setPreview(res.data);
  };

  const handleCreate = async () => {
    const res = await apiRequest<any>('/api/v1/invoices', {
      method: 'POST',
      body: JSON.stringify({
        start_date: startDate,
        end_date: endDate,
        card_id: cardId || null,
        invoice_type: invoiceType || null,
        confirmed: true,
      }),
    });
    if (res.ok) {
      setSubmitMsg(`开票任务已提交！发票单号：${res.data.invoice_id}，总金额：¥${res.data.total_amount}`);
    } else if (res.error) {
      setSubmitMsg(`[${res.error.code}] ${res.error.message}`);
      if (res.error.human_action?.human_action_url) {
        setHumanUrl(res.error.human_action.human_action_url);
      }
    }
  };

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Invoice · 按日期查询与开票预览</h1>
      </div>

      <div className="card">
        <div className="form-row">
          <div className="form-field">
            <label>开始日期</label>
            <input
              type="date"
              value={startDate}
              onChange={(e) => setStartDate(e.target.value)}
            />
          </div>
          <div className="form-field">
            <label>结束日期</label>
            <input
              type="date"
              value={endDate}
              onChange={(e) => setEndDate(e.target.value)}
            />
          </div>
          <div className="form-field">
            <label>加油卡 (默认主卡)</label>
            <input
              type="text"
              placeholder="****8816 或留空"
              value={cardId}
              onChange={(e) => setCardId(e.target.value)}
            />
          </div>
          <div className="form-field">
            <label>发票类型</label>
            <select
              value={invoiceType}
              onChange={(e) => setInvoiceType(e.target.value)}
            >
              <option value="增值税专用发票">增值税专用发票</option>
              <option value="增值税普通发票">增值税普通发票</option>
            </select>
          </div>
        </div>
        <div style={{ display: 'flex', gap: 10 }}>
          <button className="btn btn-primary" onClick={handlePreview}>
            查询并预览
          </button>
          {preview && (
            <button className="btn btn-success" onClick={handleCreate}>
              真实提交开票 (Create Invoice)
            </button>
          )}
        </div>
      </div>

      {submitMsg && (
        <div className="card" style={{ borderColor: '#93c5fd' }}>
          <strong>提交结果：</strong> {submitMsg}
          {humanUrl && (
            <div style={{ marginTop: 8 }}>
              <a
                className="btn btn-primary"
                href={humanUrl}
                target="_blank"
                rel="noreferrer"
              >
                打开人工协作验证页面 ({humanUrl})
              </a>
            </div>
          )}
        </div>
      )}

      {preview && (
        <>
          <div className="grid-cards">
            <div className="card">
              <div className="card-label">交易数量</div>
              <div className="card-value">{preview.transaction_count} 笔</div>
            </div>
            <div className="card">
              <div className="card-label">总金额 (Decimal)</div>
              <div className="card-value">¥ {preview.total_amount}</div>
            </div>
            <div className="card">
              <div className="card-label">卡 / 省份</div>
              <div className="card-value">
                {preview.card?.masked_card_no || '****8816'} ({preview.card?.province_name || '天津市'})
              </div>
            </div>
            <div className="card">
              <div className="card-label">发票类型 / 能力</div>
              <div className="card-value">{preview.invoice_type}</div>
              <div style={{ fontSize: 12, color: '#6b7280', marginTop: 4 }}>
                可用额度：¥ {preview.quota?.available_amount || '0.00'}
              </div>
            </div>
          </div>

          {(preview.blocking_reasons?.length > 0 || preview.warnings?.length > 0) && (
            <div className="card">
              <div className="card-label">校验说明与警告 (Why can_submit={String(preview.can_submit)})</div>
              <ul style={{ margin: '6px 0 0', paddingLeft: 18 }}>
                {preview.blocking_reasons?.map((r: string, idx: number) => (
                  <li key={`b-${idx}`} style={{ color: '#dc2626', fontWeight: 600 }}>
                    [需处理] {r}
                  </li>
                ))}
                {preview.warnings?.map((w: string, idx: number) => (
                  <li key={`w-${idx}`} style={{ color: '#d97706' }}>
                    [提示] {w}
                  </li>
                ))}
              </ul>
            </div>
          )}

          <div className="card">
            <div className="card-label">待开票交易明细 ({preview.candidates?.length || 0})</div>
            <table>
              <thead>
                <tr>
                  <th>交易单号</th>
                  <th>时间</th>
                  <th>加油站</th>
                  <th>油品</th>
                  <th>卡号</th>
                  <th>金额 (CNY)</th>
                  <th>状态</th>
                </tr>
              </thead>
              <tbody>
                {preview.candidates?.map((tx: any) => (
                  <tr key={tx.remote_id}>
                    <td>{tx.remote_id}</td>
                    <td>{tx.transaction_time}</td>
                    <td>{tx.station}</td>
                    <td>{tx.product}</td>
                    <td>{tx.card_id}</td>
                    <td>¥ {tx.amount}</td>
                    <td>
                      <span className="badge badge-blue">{tx.invoice_state}</span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </div>
  );
};
