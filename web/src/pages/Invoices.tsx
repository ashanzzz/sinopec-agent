import React, { useEffect, useState } from 'react';
import { apiRequest } from '../api';

export const InvoicesPage: React.FC = () => {
  const [invoices, setInvoices] = useState<any[]>([]);
  const [selected, setSelected] = useState<any>(null);
  const [downloadMsg, setDownloadMsg] = useState<string | null>(null);

  const load = async () => {
    const res = await apiRequest<any[]>('/api/v1/invoices');
    if (res.ok && res.data) setInvoices(res.data);
  };

  useEffect(() => {
    load();
  }, []);

  const handleDownload = async (id: string) => {
    const res = await apiRequest<any>(`/api/v1/invoices/${id}/download`, {
      method: 'POST',
    });
    if (res.ok && res.data) {
      setDownloadMsg(
        `下载成功：${res.data.path} (SHA256: ${res.data.sha256.slice(0, 16)}...)`,
      );
      load();
    }
  };

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Invoices · 已开发票与文件下载</h1>
        <button className="btn btn-outline" onClick={load}>
          刷新列表
        </button>
      </div>

      {downloadMsg && (
        <div className="card" style={{ borderColor: '#86efac' }}>
          {downloadMsg}
        </div>
      )}

      <div className="card">
        <table>
          <thead>
            <tr>
              <th>开票日期</th>
              <th>发票号码</th>
              <th>类型</th>
              <th>购买方抬头</th>
              <th>金额 (CNY)</th>
              <th>状态</th>
              <th>文件状态</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {invoices.map((inv) => (
              <tr key={inv.id}>
                <td>{inv.invoice_date}</td>
                <td>{inv.invoice_no || inv.id}</td>
                <td>{inv.invoice_type}</td>
                <td>{inv.buyer_name}</td>
                <td>¥ {inv.amount}</td>
                <td>
                  <span className="badge badge-green">{inv.status}</span>
                </td>
                <td>
                  <span className="badge badge-blue">{inv.file_status}</span>
                </td>
                <td style={{ display: 'flex', gap: 8 }}>
                  <button
                    className="btn btn-outline"
                    onClick={() => setSelected(inv)}
                  >
                    查看
                  </button>
                  <button
                    className="btn btn-primary"
                    onClick={() => handleDownload(inv.id)}
                  >
                    下载 {inv.file_format}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {selected && (
        <div className="card">
          <div className="card-label">发票详情 · {selected.id}</div>
          <pre style={{ fontSize: 12, margin: 0 }}>
            {JSON.stringify(selected, null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
};
