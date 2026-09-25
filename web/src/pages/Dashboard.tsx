import React, { useEffect, useState } from 'react';
import { apiRequest } from '../api';

export const DashboardPage: React.FC = () => {
  const [sys, setSys] = useState<any>(null);
  const [loading, setLoading] = useState(true);

  const refresh = async () => {
    setLoading(true);
    const res = await apiRequest<any>('/api/v1/system');
    if (res.ok) setSys(res.data);
    setLoading(false);
  };

  useEffect(() => {
    refresh();
  }, []);

  const accountModeLabel = () => {
    const mode = sys?.auth?.account_mode;
    if (mode === 'corporate') return '单位账户';
    if (mode === 'personal') return '个人账户';
    return '等待识别';
  };

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Dashboard · 运行概览</h1>
        <button className="btn btn-outline" onClick={refresh}>
          {loading ? '刷新中...' : '刷新状态'}
        </button>
      </div>

      <div className="grid-cards">
        <div className="card">
          <div className="card-label">Backend</div>
          <div className="card-value">
            sinopec-server{' '}
            <span className="badge badge-green">{sys ? 'ONLINE' : 'CHECKING'}</span>
          </div>
          <div style={{ fontSize: 12, color: '#6b7280', marginTop: 6 }}>
            Version {sys?.backend?.version || '0.1.0'} · SQLite WAL
          </div>
        </div>

        <div className="card">
          <div className="card-label">Sinopec Auth</div>
          <div className="card-value">
            <span
              className={`badge ${
                sys?.auth?.verified ? 'badge-green' : 'badge-yellow'
              }`}
            >
              {sys?.auth?.state || 'UNKNOWN'}
            </span>
          </div>
          <div style={{ fontSize: 12, color: '#6b7280', marginTop: 6 }}>
            {sys?.auth?.reason || '等待校验'}
          </div>
        </div>

        <div className="card">
          <div className="card-label">Account Mode</div>
          <div className="card-value">{accountModeLabel()}</div>
          <div style={{ fontSize: 12, color: '#6b7280', marginTop: 6 }}>
            {sys?.auth?.company_name || sys?.auth?.member_account || '通过 /corpgas 自动识别'}
          </div>
        </div>

        <div className="card">
          <div className="card-label">Steel Browser</div>
          <div className="card-value">
            <span
              className={`badge ${
                sys?.steel?.reachable ? 'badge-green' : 'badge-red'
              }`}
            >
              {sys?.steel?.reachable ? 'READY' : 'OFFLINE'}
            </span>
          </div>
          <div style={{ fontSize: 12, color: '#6b7280', marginTop: 6 }}>
            {sys?.steel?.browser_version || sys?.steel?.base_url || '192.168.8.11:13000'}
          </div>
        </div>

        <div className="card">
          <div className="card-label">Last Operation</div>
          <div className="card-value">
            {sys?.last_operation?.operation_type || 'NONE'}
          </div>
          <div style={{ fontSize: 12, color: '#6b7280', marginTop: 6 }}>
            {sys?.last_operation
              ? `${sys.last_operation.phase} · ${sys.last_operation.state}`
              : '尚无历史开票任务'}
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-label">快捷操作</div>
        <div style={{ display: 'flex', gap: 10, marginTop: 10 }}>
          <button
            className="btn btn-primary"
            onClick={async () => {
              await apiRequest('/api/v1/auth/ensure', { method: 'POST' });
              refresh();
            }}
          >
            检测并触发登录验证 (Ensure Auth)
          </button>
          <button
            className="btn btn-outline"
            onClick={async () => {
              await apiRequest('/api/v1/auth/check', { method: 'POST' });
              refresh();
            }}
          >
            同步 Steel Cookie 并校验 (AuthVerifier)
          </button>
          {sys?.steel?.viewer_url && (
            <a
              className="btn btn-outline"
              href={sys.steel.viewer_url}
              target="_blank"
              rel="noreferrer"
            >
              打开 Steel 远程浏览器
            </a>
          )}
        </div>
      </div>
    </div>
  );
};
