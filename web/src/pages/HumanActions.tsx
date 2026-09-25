import React, { useEffect, useState } from 'react';
import { apiRequest, HumanActionRecord } from '../api';

export const HumanActionsPage: React.FC = () => {
  const [items, setItems] = useState<HumanActionRecord[]>([]);

  const load = async () => {
    const res = await apiRequest<HumanActionRecord[]>('/api/v1/human-actions');
    if (res.ok && res.data) setItems(res.data);
  };

  useEffect(() => {
    load();
  }, []);

  const complete = async (id: string) => {
    await apiRequest(`/api/v1/human-actions/${id}/complete`, {
      method: 'POST',
      body: JSON.stringify({}),
    });
    load();
  };

  const cancel = async (id: string) => {
    await apiRequest(`/api/v1/human-actions/${id}/cancel`, {
      method: 'POST',
    });
    load();
  };

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Human Actions · 人工协作与任务恢复</h1>
        <button className="btn btn-outline" onClick={load}>
          刷新列表
        </button>
      </div>

      <div className="card">
        <table>
          <thead>
            <tr>
              <th>状态</th>
              <th>原因</th>
              <th>说明</th>
              <th>关联任务</th>
              <th>创建时间</th>
              <th>有效截止</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {items.length === 0 ? (
              <tr>
                <td colSpan={7} style={{ textAlign: 'center', color: '#6b7280' }}>
                  当前没有待处理的人工协作请求
                </td>
              </tr>
            ) : (
              items.map((ha) => (
                <tr key={ha.id}>
                  <td>
                    <span
                      className={`badge ${
                        ha.status === 'WAITING'
                          ? 'badge-yellow'
                          : ha.status === 'COMPLETED'
                          ? 'badge-green'
                          : 'badge-red'
                      }`}
                    >
                      {ha.status}
                    </span>
                  </td>
                  <td>{ha.reason}</td>
                  <td style={{ maxWidth: 320 }}>{ha.message}</td>
                  <td>{ha.operation_id || '-'}</td>
                  <td>{ha.created_at.slice(0, 19).replace('T', ' ')}</td>
                  <td>{ha.expires_at.slice(0, 19).replace('T', ' ')}</td>
                  <td style={{ display: 'flex', gap: 6 }}>
                    <a
                      className="btn btn-primary"
                      href={ha.viewer_url || ha.human_action_url}
                      target="_blank"
                      rel="noreferrer"
                    >
                      打开浏览器
                    </a>
                    {ha.status === 'WAITING' && (
                      <>
                        <button
                          className="btn btn-success"
                          onClick={() => complete(ha.id)}
                        >
                          完成
                        </button>
                        <button
                          className="btn btn-danger"
                          onClick={() => cancel(ha.id)}
                        >
                          取消
                        </button>
                      </>
                    )}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
};
