const KEYS = {
  accessToken: "zurfur_access_token",
  refreshToken: "zurfur_refresh_token",
  userId: "zurfur_user_id",
  did: "zurfur_did",
  handle: "zurfur_handle",
} as const;

export interface StoredSession {
  accessToken: string;
  refreshToken: string;
  userId: string;
  did: string;
  handle: string | null;
}

export function storeSession(session: StoredSession): void {
  localStorage.setItem(KEYS.accessToken, session.accessToken);
  localStorage.setItem(KEYS.refreshToken, session.refreshToken);
  localStorage.setItem(KEYS.userId, session.userId);
  localStorage.setItem(KEYS.did, session.did);
  if (session.handle) {
    localStorage.setItem(KEYS.handle, session.handle);
  }
}

export function clearSession(): void {
  for (const key of Object.values(KEYS)) {
    localStorage.removeItem(key);
  }
}

export function getAppUrl(): string {
  return import.meta.env.VITE_APP_URL ?? "https://zurfur.app";
}
