import React, { useEffect, useState } from 'react';
import { apiRequest } from '../api';

export const SystemPage: React.FC = () => {
  const [sys, setSys] = useState<any>(null);

  useEffect(() => {
    apiRequest<any>('/api/v1/system').then((r) => {
      if (r.ok) setSys(r.data);
    });
  }, []);

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">System · 组件与运行时诊断</h1>
      </div>

      <div className="card">
        <div className="card-label">sinopec-server & SQLite</div>
        <pre style={{ fontSize: 12, margin: 0 }}>
          {JSON.stringify(sys?.backend, null, 2)}
        </pre>
      </div>

      <div className="card">
        <div className="card-label">Steel Browser (Unraid 192.168.8.11:13000 / 19223)</div>
        <pre style={{ fontSize: 12, margin: 0 }}>
          {JSON.stringify(sys?.steel, null, 2)}
        </pre>
      </div>

      <div className="card">
        <div className="card-label">Scrapling Research Assistant (Unraid 192.168.8.11:8111)</div>
        <pre style={{ fontSize: 12, margin: 0 }}>
          {JSON.stringify(sys?.scrapling, null, 2)}
        </pre>
      </div>
    </div>
  );
};
