import React, { useEffect, useState } from 'react';
import { apiRequest } from './api';
import { DashboardPage } from './pages/Dashboard';
import { InvoicePage } from './pages/Invoice';
import { InvoicesPage } from './pages/Invoices';
import { HumanActionsPage } from './pages/HumanActions';
import { SettingsPage } from './pages/Settings';
import { ResearchPage } from './pages/Research';
import { SystemPage } from './pages/System';

type TabKey =
  | 'dashboard'
  | 'invoice'
  | 'invoices'
  | 'human'
  | 'settings'
  | 'research'
  | 'system';

const App: React.FC = () => {
  const [activeTab, setActiveTab] = useState<TabKey>('settings');
  const [researchMode, setResearchMode] = useState(false);
  const [verified, setVerified] = useState(false);
  const [authState, setAuthState] = useState('CHECKING');

  const checkAuth = async () => {
    const res = await apiRequest<any>('/api/v1/auth/check', { method: 'POST' });
    if (res.ok && res.data) {
      const isOk = Boolean(res.data.verified);
      setVerified(isOk);
      setAuthState(res.data.state || (isOk ? 'LOGGED_IN' : 'SESSION_EXPIRED'));
      if (isOk && activeTab === 'settings') {
        setActiveTab('invoice');
      }
    }
  };

  useEffect(() => {
    apiRequest<any>('/api/v1/health').then((res) => {
      if (res.ok && res.data) {
        setResearchMode(Boolean(res.data.research_mode));
      }
    });
    checkAuth();
  }, []);

  const switchTab = (tab: TabKey) => {
    if (!verified && tab !== 'settings' && tab !== 'human') {
      alert('请先完成中国石化短信验证登录，登录认证通过后自动解锁开票与账单模块。');
      setActiveTab('settings');
      return;
    }
    setActiveTab(tab);
  };

  return (
    <div className="app-layout">
      <aside className="sidebar">
        <div className="sidebar-brand">
          Sinopec Agent
          <div style={{ marginTop: 6 }}>
            <span className={`badge ${verified ? 'badge-green' : 'badge-yellow'}`}>
              {verified ? '已登录在线' : `未登录 (${authState})`}
            </span>
          </div>
        </div>

        <button
          className={`nav-btn ${activeTab === 'settings' ? 'active' : ''}`}
          onClick={() => switchTab('settings')}
        >
          1. 登录认证门禁 (Settings)
        </button>
        <button
          className={`nav-btn ${activeTab === 'invoice' ? 'active' : ''}`}
          style={{ opacity: verified ? 1 : 0.55 }}
          onClick={() => switchTab('invoice')}
        >
          {verified ? '' : '[需登录] '}2. 指定时段开票 (Invoice)
        </button>
        <button
          className={`nav-btn ${activeTab === 'invoices' ? 'active' : ''}`}
          style={{ opacity: verified ? 1 : 0.55 }}
          onClick={() => switchTab('invoices')}
        >
          {verified ? '' : '[需登录] '}3. 已开发票与下载 (Invoices)
        </button>
        <button
          className={`nav-btn ${activeTab === 'dashboard' ? 'active' : ''}`}
          style={{ opacity: verified ? 1 : 0.55 }}
          onClick={() => switchTab('dashboard')}
        >
          {verified ? '' : '[需登录] '}4. 运行概览 (Dashboard)
        </button>
        <button
          className={`nav-btn ${activeTab === 'human' ? 'active' : ''}`}
          onClick={() => switchTab('human')}
        >
          Human Actions
        </button>
        {researchMode && (
          <button
            className={`nav-btn ${activeTab === 'research' ? 'active' : ''}`}
            style={{ opacity: verified ? 1 : 0.55 }}
            onClick={() => switchTab('research')}
          >
            Research
          </button>
        )}
        <button
          className={`nav-btn ${activeTab === 'system' ? 'active' : ''}`}
          style={{ opacity: verified ? 1 : 0.55 }}
          onClick={() => switchTab('system')}
        >
          System
        </button>
      </aside>

      <main className="main-content">
        {activeTab === 'dashboard' && verified && <DashboardPage />}
        {activeTab === 'invoice' && verified && <InvoicePage />}
        {activeTab === 'invoices' && verified && <InvoicesPage />}
        {activeTab === 'human' && <HumanActionsPage />}
        {activeTab === 'settings' && <SettingsPage />}
        {activeTab === 'research' && researchMode && verified && <ResearchPage />}
        {activeTab === 'system' && verified && <SystemPage />}
      </main>
    </div>
  );
};

export default App;
