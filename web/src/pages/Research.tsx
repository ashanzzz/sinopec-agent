import React, { useEffect, useState } from 'react';
import { apiRequest } from '../api';

export const ResearchPage: React.FC = () => {
  const [status, setStatus] = useState<any>(null);
  const [endpoints, setEndpoints] = useState<any[]>([]);

  const load = async () => {
    const [sRes, eRes] = await Promise.all([
      apiRequest<any>('/api/v1/research/status'),
      apiRequest<any[]>('/api/v1/research/endpoints'),
    ]);
    if (sRes.ok) setStatus(sRes.data);
    if (eRes.ok && eRes.data) setEndpoints(eRes.data);
  };

  useEffect(() => {
    load();
  }, []);

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Research · 协议逆向知识库与网络捕获 (脱敏)</h1>
        <button className="btn btn-outline" onClick={load}>
          刷新研究快照
        </button>
      </div>

      <div className="grid-cards">
        <div className="card">
          <div className="card-label">当前页面 URL</div>
          <div style={{ fontSize: 13, fontWeight: 600, wordBreak: 'break-all' }}>
            {status?.current_page_url || 'https://www.sinopecsales.com/default_corp.html'}
          </div>
          <div style={{ fontSize: 12, color: '#6b7280', marginTop: 4 }}>
            Title: {status?.current_page_title}
          </div>
        </div>

        <div className="card">
          <div className="card-label">当前认证状态</div>
          <div className="card-value">{status?.auth_state || 'UNKNOWN'}</div>
        </div>

        <div className="card">
          <div className="card-label">CONFIRMED 接口数量</div>
          <div className="card-value" style={{ color: '#16a34a' }}>
            {status?.confirmed_endpoints_count ?? 0}
          </div>
        </div>

        <div className="card">
          <div className="card-label">HYPOTHESIS / UNKNOWN 接口</div>
          <div className="card-value" style={{ color: '#d97706' }}>
            {(status?.hypothesis_endpoints_count ?? 0) +
              (status?.unknown_endpoints_count ?? 0)}
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-label">已记录中国石化协议知识库 (docs/research/sinopec.md & SQLite)</div>
        <table>
          <thead>
            <tr>
              <th>状态</th>
              <th>功能名称</th>
              <th>账户类型</th>
              <th>Method</th>
              <th>Endpoint Path</th>
              <th>说明</th>
            </tr>
          </thead>
          <tbody>
            {endpoints.map((ep) => (
              <tr key={ep.id}>
                <td>
                  <span
                    className={`badge ${
                      ep.status === 'CONFIRMED'
                        ? 'badge-green'
                        : ep.status === 'HYPOTHESIS'
                        ? 'badge-yellow'
                        : 'badge-red'
                    }`}
                  >
                    {ep.status}
                  </span>
                </td>
                <td>{ep.endpoint_name}</td>
                <td>{ep.account_mode}</td>
                <td><code>{ep.method}</code></td>
                <td><code>{ep.path}</code></td>
                <td>{ep.notes}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="card">
        <div className="card-label">Research Timeline</div>
        <ul style={{ margin: '8px 0 0', paddingLeft: 18 }}>
          {status?.timeline?.map((t: any, i: number) => (
            <li key={i} style={{ marginBottom: 4 }}>
              <code>{t.timestamp}</code> <strong>[{t.category}]</strong> {t.summary}
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
};
