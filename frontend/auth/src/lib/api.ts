const API_URL = import.meta.env.VITE_API_URL ?? "http://localhost:3000";

export interface StartLoginResponse {
  redirect_url: string;
  state: string;
}

export interface CallbackResponse {
  access_token: string;
  refresh_token: string;
  user_id: string;
  did: string;
  handle: string | null;
  is_new_user: boolean;
}

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const res = await fetch(`${API_URL}${path}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...options.headers,
    },
  });

  if (!res.ok) {
    const text = await res.text();
    throw new ApiError(res.status, text || `Request failed: ${res.status}`);
  }

  return res.json() as Promise<T>;
}

export function startLogin(handle: string): Promise<StartLoginResponse> {
  return request<StartLoginResponse>("/auth/start", {
    method: "POST",
    body: JSON.stringify({ handle }),
  });
}

export function exchangeCallback(
  code: string,
  state: string,
  iss?: string,
): Promise<CallbackResponse> {
  const params = new URLSearchParams({ code, state });
  if (iss) params.set("iss", iss);
  return request<CallbackResponse>(`/auth/callback?${params.toString()}`);
}
