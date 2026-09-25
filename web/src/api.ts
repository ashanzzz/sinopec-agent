export interface ApiResponse<T> {
  ok: boolean;
  data?: T;
  error?: {
    code: string;
    message: string;
    human_action?: HumanActionRecord;
  };
}

export interface HumanActionRecord {
  id: string;
  token: string;
  reason: string;
  status: string;
  message: string;
  human_action_url: string;
  viewer_url?: string;
  operation_id?: string;
  created_at: string;
  expires_at: string;
}

export async function apiRequest<T>(path: string, options?: RequestInit): Promise<ApiResponse<T>> {
  const res = await fetch(path, {
    headers: {
      'Content-Type': 'application/json',
      ...(options?.headers || {}),
    },
    ...options,
  });
  return res.json();
}
